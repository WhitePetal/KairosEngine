# Material 动态属性系统 — Phase 3：功能分析

## 3.1 模块全景

```
┌─────────────────────────────────────────────────────────────┐
│  M1: WGSL 反射 (kairos_engine)                              │
│  naga 解析 → 提取 @group/@binding → ShaderProperty[]         │
├─────────────────────────────────────────────────────────────┤
│  M2: Material 数据模型 (kairos_engine)                       │
│  SerializedMaterial + Material 扩展动态属性存储              │
├─────────────────────────────────────────────────────────────┤
│  M3: 属性协调器 (kairos_engine)                              │
│  Shader 变更 → 合并新旧属性 → dirty 标记                     │
├─────────────────────────────────────────────────────────────┤
│  M4: RenderPipeline 绑定组 (kairos_engine)                  │
│  动态创建 bind group layout → uniform buffer → 上传属性值    │
├─────────────────────────────────────────────────────────────┤
│  M5: MaterialInspector UI (kairos_editor)                   │
│  属性元数据 → egui widget 映射 → 动态 UI 生成                │
├─────────────────────────────────────────────────────────────┤
│  M6: 序列化模块 (kairos_engine)                              │
│  TOML [material.properties] + 纹理路径序列化                 │
├─────────────────────────────────────────────────────────────┤
│  M7: 文件监视器 (kairos_editor)                              │
│  notify → 检测文件变更 → AssetsServer::reload()             │
├─────────────────────────────────────────────────────────────┤
│  M8: 运行时 API (kairos_engine)                              │
│  material.set_float/get_float → 静默失败 → dirty 标记        │
└─────────────────────────────────────────────────────────────┘
```

## 3.2 M1：WGSL 反射

**职责**：解析 WGSL 源码，提取 Material 属性定义

**输入**：WGSL 源码字符串
**输出**：`Vec<ShaderProperty>`

```rust
pub struct ShaderProperty {
    pub name: String,
    pub ty: ShaderPropertyType,
    pub binding_group: u32,
    pub binding_index: u32,
}

pub enum ShaderPropertyType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Int,
    UInt,
    Bool,
    Texture2D,
    TextureCube,
}
```

**naga 反射实现**：

1. `naga::front::wgsl::parse_str()` 解析 WGSL 为 `Module`
2. 遍历 `module.global_variables`
3. 匹配 `AddressSpace::Uniform` → 展开 struct members → 数值属性
4. 匹配 `TypeInner::Image` → 纹理属性
5. 忽略 sampler（由纹理资产自动管理）

**约定**：每个 Shader 有且仅有一个 `var<uniform>`，其类型为单层 struct（参考 Unity SRP Batcher 的 `PerMaterialCBuffer`）。

**新增文件**：`kairos_engine/src/graphics/shader_reflection.rs`（新建）

## 3.3 M2：Material 数据模型

**修改文件**：`kairos_engine/src/graphics/material.rs`

```rust
// 运行时 Material
pub struct Material {
    pub shader: Option<Arc<AssetHandle<ShaderAssetsSystem>>>,
    pub render_state: RenderState,
    pub properties: HashMap<String, MaterialProperty>,
}

pub enum MaterialProperty {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Int(i32),
    UInt(u32),
    Bool(bool),
    Texture(Arc<AssetHandle<TextureAssetsSystem>>),
}

// 序列化格式
pub struct SerializedMaterial {
    pub source_path: PathBuf,
    pub shader_path: PathBuf,
    pub render_state: RenderState,
    pub properties: HashMap<String, SerializedProperty>,
    pub textures: HashMap<String, PathBuf>,
}

pub enum SerializedProperty {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Int(i32),
    Bool(bool),
}
```

**删除**：`Material::texture`、`SerializedMaterial::texture_path`（不向后兼容）

## 3.4 M3：属性协调器

**职责**：Shader 变更时同步 Material 属性

**触发时机**：
- Material 加载时（首次填充默认值）
- MaterialInspector 检测到 shader `modify_count` 变更
- 渲染帧检测到 shader `modify_count` 变更

**协调逻辑**：

```
for new_prop in shader.properties:
    if old.properties 中存在同名 + 类型匹配:
        → 保留旧值
    else:
        → 使用 Shader 默认值（类型零值）

for old_name in old.properties 中不存在于 new:
    → 移除 + log::warn!
```

**dirty 标记**: 协调后保留原有的 dirty 状态，不自动清除

## 3.5 M4：RenderPipeline 动态绑定组

**修改文件**：`kairos_engine/src/graphics/render_pipeline.rs`

**现状**：硬编码 bind group layout（group 0 = VP，group 1 = texture + sampler）

**改造**：
- 替换 `create_texture()` 为 `create_material_bind_group()`
- 根据 `ShaderAsset.properties` 动态构建 `BindGroupLayoutDescriptor`
- 使用 `encase` crate 按 std140 布局序列化 uniform buffer
- 缓存策略：

```rust
struct MaterialBindGroupKey {
    material_id: usize,
    material_modify_count: u64,
}

struct MaterialBindGroupCache {
    bind_group: BindGroup,
    bind_group_layout: BindGroupLayout,
    uniform_buffer: Buffer,
}

HashMap<MaterialBindGroupKey, MaterialBindGroupCache>
```

## 3.6 M5：MaterialInspector UI

**修改文件**：`kairos_editor/src/ui/inspector/material.rs`

**改造**：在现有 RenderState 编辑区下方，新增动态属性面板

UI 生成逻辑：
```
对每个 shader.property:
    match (type, metadata):
        (Float, Range{min, max}) → egui::Slider
        (Float, _)               → egui::DragValue
        (Vec3/Vec4, Color)       → egui color_picker
        (Vec2/Vec3/Vec4, _)      → [DragValue; N]
        (Texture2D, _)           → 拖拽目标框（复用现有交互）
        (Bool, _)               → egui::Checkbox
        (Int, _)                 → egui::DragValue (speed=1)
```

**属性按 group 分组显示**（依赖 M5.1 元数据）

## 3.7 M5.1：属性元数据

**职责**：从 WGSL 注释中提取 UI 提示信息

**方案**：WGSL 注释标注（类似 Godot hints）

```wgsl
struct PerMaterialCBuffer {
    // @range(0.0, 1.0) @slider @group "PBR"
    roughness: f32,
    
    // @color @group "PBR"
    albedo: vec4<f32>,
}

// @texture @group "Textures"
@group(2) @binding(1) var albedo_map: texture_2d<f32>;
```

**元数据 tags**：

| Tag | 作用 |
|-----|------|
| `@range(min, max)` | 显示 Slider 替代 DragValue |
| `@slider` | 同上（显式声明） |
| `@color` | vec3/vec4 → ColorPicker |
| `@group "Name"` | Inspector 中分组显示 |
| `@texture` | 标记为纹理属性 |

## 3.8 M6：序列化

**职责**：动态属性的 TOML 持久化

不向后兼容，所有现有 `.mat` 文件在实现时一次性迁移。

## 3.9 M7：文件监视器

**新增文件**：`kairos_editor/src/file_watcher.rs`

```rust
pub struct FileWatcher {
    watcher: notify::RecommendedWatcher,
}

impl FileWatcher {
    pub fn new(root_dir: &Path) -> Self;
    pub fn poll(&mut self, assets_server: &mut AssetsServer);
}
```

按扩展名路由：
- `.wgsl` → `assets_server.reload::<ShaderAssetsSystem>()`
- `.texture_bin` → `assets_server.reload::<TextureAssetsSystem>()`

## 3.10 M8：运行时 API

```rust
impl Material {
    pub fn get_float(&self, name: &str) -> f32;
    pub fn set_float(&mut self, name: &str, value: f32);
    pub fn get_vec4(&self, name: &str) -> [f32; 4];
    pub fn set_vec4(&mut self, name: &str, value: [f32; 4]);
    pub fn get_texture(&self, name: &str) -> Option<&Arc<AssetHandle<TextureAssetsSystem>>>;
    pub fn set_texture(&mut self, name: &str, handle: Arc<AssetHandle<TextureAssetsSystem>>);
    // ... 其他类型的 get/set 对
}
```

静默失败策略，与 Unity 一致。

## 3.11 模块依赖关系

```
M1 (WGSL反射)     M2 (数据模型)
     │                   │
     └───────┬───────────┘
             ▼
        M3 (属性协调器)
             │
        ┌────┴────┐
        ▼         ▼
  M6 (序列化)  M4 (Pipeline)
        │         │
        └────┬────┘
             ▼
        M5 (Inspector UI)
             │
             ▼
        M5.1 (元数据)  M7 (FileWatcher)
             │              │
             ▼              ▼
        M8 (运行时 API)
```
