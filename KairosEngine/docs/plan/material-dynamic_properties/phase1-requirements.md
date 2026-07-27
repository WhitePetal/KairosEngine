# Material 动态属性系统 — Phase 1：需求分析

## 1.1 背景

当前 Material 属性是硬编码的：只有 `texture: Option<Arc<AssetHandle<TextureAssetsSystem>>>` 一个可变字段。不同 Shader 需要不同参数时无法表达——PBR Shader 需要 `roughness`/`metallic`/`albedo`，Water Shader 需要 `wave_amplitude`/`wave_frequency`，每新增一种 Shader 都要修改 Material 结构体。

## 1.2 目标

将 Material 改造为 **Shader 驱动的动态属性系统**：Material 的属性集合由其引用的 Shader 定义，通过 naga 反射 WGSL 源码自动提取。

## 1.3 架构决策

采用 **Unity/Godot 模式**（Shader 驱动），而非 Bevy 模式（Rust struct 驱动）：

- 属性定义源是 WGSL Shader 文件，不是 Rust 代码
- 运行时通过 naga 解析 WGSL 提取属性声明
- Material 中动态存储属性值（`HashMap<String, MaterialProperty>`）
- Inspector 根据反射结果自动生成编辑 UI

## 1.4 属性生命周期与优先级

| 优先级 | 来源 | 说明 |
|--------|------|------|
| 1（最高） | 运行时覆盖 | 代码中 `material.set_float(...)` 修改 |
| 2 | Material 文件 | `.mat` TOML 中存储的值 |
| 3（最低） | Shader 默认值 | WGSL 中声明的默认值（暂以类型零值代替） |

## 1.5 类型系统

| Rust 类型 | WGSL 类型 | Inspector 默认控件 |
|-----------|-----------|-------------------|
| `f32` | `f32` | DragValue |
| `Vec2` / `[f32; 2]` | `vec2<f32>` | [DragValue × 2] |
| `Vec3` / `[f32; 3]` | `vec3<f32>` | [DragValue × 3] |
| `Vec4` / `[f32; 4]` | `vec4<f32>` | [DragValue × 4] |
| `i32` | `i32` | DragValue (integer) |
| `u32` | `u32` | DragValue (unsigned) |
| `bool` | `bool` | Checkbox |
| `TextureHandle` | `texture_2d<f32>` | 拖拽目标框 |
| `TextureHandle` | `texture_cube<f32>` | 拖拽目标框 |

暂不支持：矩阵、数组、结构体嵌套。

## 1.6 运行时 API

采用 Unity 一致的**静默失败**策略：

```rust
impl Material {
    /// 设置浮点属性。属性名不存在或类型不匹配时静默失败 + log::warn!
    pub fn set_float(&mut self, name: &str, value: f32);
    pub fn get_float(&self, name: &str) -> f32;
    pub fn set_vec4(&mut self, name: &str, value: [f32; 4]);
    pub fn get_vec4(&self, name: &str) -> [f32; 4];
    pub fn set_texture(&mut self, name: &str, handle: Arc<AssetHandle<TextureAssetsSystem>>);
    pub fn get_texture(&self, name: &str) -> Option<&Arc<AssetHandle<TextureAssetsSystem>>>;
}
```

## 1.7 Shader 变更检测

编辑器层（`kairos_editor`）通过 `notify` crate 监听项目目录的文件变更：

```
Zed 中修改 .wgsl 保存
  → notify 检测文件变更
  → kairos_editor::FileWatcher 调用 AssetsServer::reload::<ShaderAssetsSystem>(path)
  → ShaderLoader 重新加载 + naga 重新解析属性
  → modify_count 递增
  → MaterialInspector / RenderPipeline 检测到 modify_count 变更 → 属性协调
```

`kairos_engine`（运行时层）不引入 `notify` 依赖，仅暴露 `AssetsServer::reload()` 公共 API。

## 1.8 属性协调策略

当 Shader 变更时，Material 执行属性协调：

| 场景 | 行为 |
|------|------|
| 新增属性 | 使用 Shader 默认值（类型零值） |
| 删除属性 | 从 Material 中移除，`log::warn!` |
| 同名属性、类型相同 | **保留旧值**（不丢失用户修改） |
| 同名属性、类型变更 | 使用默认值，`log::warn!` |
| Inspector 中有未保存修改 | 保留 dirty 标记，协调后不清除 |

渲染帧检测到变更时仅更新内存中的 `Material` 数据，不写磁盘。

## 1.9 序列化格式

TOML 新格式（不向后兼容旧格式，所有 `.mat` 文件一次性迁移）：

```toml
source_path = "res/materials/pbr.mat"
shader_path = "res/shaders/pbr.wgsl"

[render_state]
cull_mod = "Back"
depth_test = "LessEqual"

[material.properties]
roughness = 0.5
metallic = 0.0
albedo = [1.0, 0.5, 0.2, 1.0]

[material.textures]
albedo_map = "res/textures/albedo.texture"
normal_map = "res/textures/normal.texture"
```

## 1.10 错误处理与降级

| 场景 | 降级策略 |
|------|----------|
| Shader 文件不存在 | 使用 `error_shader.wgsl`（纯紫），忽略所有动态属性 |
| Shader WGSL 语法错误 | naga 解析失败 → 空属性列表 + 日志 |
| 属性类型不匹配 | 使用 Shader 默认值 + `log::warn!` |
| 属性值越界 | Slider 类型 clamp；其他类型允许任意值 |
| Texture 文件不存在 | 使用 `white.texture`（2×2 纯白）降级 |
| Shader 中不存在的属性 | 保留在 Material 中但不在 Inspector 显示 |

## 1.11 不可变设计约束

| 约束 | 说明 |
|------|------|
| Shader 中 Material 参数约定 | 每个 Shader 有且仅有一个 `var<uniform>`，其类型为单层 struct（如 `PerMaterialCBuffer`），包含所有数值属性 |
| sampler 不由 Material 管理 | sampler 由纹理资产的 `format` + `sampler` 配置自动创建 |
| 编辑器文件监听 | 仅在 `kairos_editor` 模块，不侵入 `kairos_engine` |
| 渲染帧 Material 更新 | 仅内存操作，不写磁盘 |

## 1.12 范围外

- Shader Graph / 可视化材质编辑器
- 运行时 Shader 热重载
- Material 实例化（Material Instance / Material Preset）
- Compute Shader 的 Material 支持
- 矩阵、数组、嵌套结构体类型
