# Material Inspector 设计案 — Phase 3：技术选型与坑点

## 3.1 Module A：基础框架

### Style TOML (MaterialInspectorStyle)

```toml
row_height = 24.0
apply_button_height = 32.0
preview_min_height = 200.0
preview_default_size = 512
camera_fov = 60.0
camera_direction = [0.0, 0.0, -1.0]
camera_orbit_speed = 5.0
camera_zoom_speed = 5.0
camera_min_distance = 1.0
camera_max_distance = 100.0

preview_meshes = [
    "res/models/Suzanne.mesh",
]
```

### SerializedMaterialAssetsSystem

新建资产系统，与 `MaterialAssetsSystem` 并行加载同一 `.mat` 文件。
- 注册方式：在 `KairosEngine::new()` 或相关初始化位置 `assets_server.push(SerializedMaterialAssetsSystem::new())`
- Loader：`tokio::fs::read` → `toml::from_slice`（纯数据，无依赖加载，比 MaterialAssetsSystem 的 Loader 简单）
- Inspector::create() 中通过 `assets_server.load::<SerializedMaterialAssetsSystem>(&mat_path)` 获取 handle
- 后续通过 `assets_server.get::<SerializedMaterialAssetsSystem>(handle)` 获取 `&SerializedMaterial`

**坑点**：两个 AssetsSystem 加载同一个文件，需确保文件路径一致。建议使用 `canonicalize()` 或保证路径归一化。

## 3.2 Module B：Shader 下拉栏

### 数据来源

`ProjectPathGraph::find_assets_by_kind(AssetKind::Shader)` 返回所有 `.wgsl` 节点。
- `InspectorCreater::create_from_asseet_kind()` 签名增加 `project_graph: &ProjectPathGraph` 参数
- `MaterialInspector::create()` 中调用 `find_assets_by_kind` 缓存路径列表
- 显示名称取文件名 stem（`path.file_stem()`）

### Shader 切换流程

1. 用户在下拉栏选择新 shader
2. `assets_server.load::<ShaderAssetsSystem>(&new_shader_path)` 触发异步加载
3. 等待 shader handle 可用后（异步），构建新的 `Material { shader: Some(handle), .. }`
4. `assets_server.insert::<MaterialAssetsSystem>(new_material, &mat_path)` 原地更新
5. 设置 `dirty = true`

**坑点**：load 是异步的，shader handle 不会立即可用。需要在 draw() 中等待 handle 就绪后再 insert。

## 3.3 Module C：Texture 拖拽输入

### 拖拽交互分析

目前代码库中没有现成的"从项目树拖拽资产到 Inspector"的实现（现有拖拽系统仅用于 docking tab）。需要新建。

### egui drag-and-drop 方案

egui 0.35 提供 `DragSource` / `DropTarget` 标准模式。

**拖拽数据传递流程**：

```
项目树节点（drag source）             MaterialInspector（drop target）
        │                                      │
        │  1. 鼠标按住节点拖拽                  │
        │  → DragSource::begin(ui)              │
        │  → payload: 节点的 asset_path         │
        │  → 光标变化（使用可配置的光标图案）    │
        ├──────────────────────────────────────►│
        │                                      │  2. 检测到有拖拽进入目标框
        │                                      │  → 高亮目标框（视觉反馈）
        │  3. 鼠标松开                         │
        │                                      │  4. 接收 payload
        │                                      │  → 检查 payload 类型/
        │                                      │     扩展名是否为 .texture
        │                                      │  → assets_server.load()
        │                                      │  → 更新 SerializedMaterial
        │                                      │  → dirty = true
```

### 项目树侧的拖拽支持

- 所有 `ProjectTreeNode` 都可以注册 DragSource（为未来"移动资产"等需求预留）
- 目前实际 payload 只在拖拽 Texture 节点时有效，其他节点拖拽到 Inspector 会被 drop target 过滤掉
- 视觉反馈：光标变化（光标图案路径可配置，默认借用 `Preferences/Textures/audio_icon.png`）
- 需要在 `HierarchyPanel` / `ContentPanel` 的节点渲染处添加 DragSource

### Inspector 侧的拖拽目标框

布局：缩略图预览 + 路径文字 + 清除按钮

```
┌────┬─────────────────────────────┐
│    │ res/textures/xxx.tex   [×]  │  ← 左侧缩略图（通过 handle 获取像素），右侧路径 + 清除按钮
└────┴─────────────────────────────┘
无纹理时显示 "Drag texture here"
```

- 目标框检测拖拽 payload，过滤非 `.texture` 扩展名
- 拖拽悬停时高亮目标框
- 纹理路径更新后同步到运行时 Material（同 shader 切换流程）

### 坑点
- egui 的 drag-and-drop 是 UI 线程内操作，数据通过 `egui::Id` + payload 传递
- 需要确保项目树节点能区分"点击选中"和"拖拽开始"（`Sense::click_and_drag()` 处理）
- 拖拽光标使用 egui 预设 `CursorIcon::Grabbing`（egui 不支持自定义 OS 光标图案）

## 3.4 Module D：RenderState 编辑

### wgpu 类型包装

参考 `CompareFunction` 的模式：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum WrappedBlendFactor {
    Zero, One, SrcColor, OneMinusSrcColor, ...
}

impl From<WrappedBlendFactor> for wgpu::BlendFactor { ... }
impl WrappedBlendFactor { pub fn label(&self) -> &'static str { ... } }
```

`BlendState` 的预设模式需要定义一个项目级类型：

```rust
pub enum BlendPreset {
    Replace,
    Add,
    Multiply,
    AlphaBlend,
    Custom(BlendState),  // 展开子字段
}
```

编辑交互（参考 TextureInspector 的 Address Mode + Per Axis 模式）：
1. 主下拉栏显示预设名（Replace / Add / Multiply / Alpha / Custom）
2. 当前 BlendState 不属于任何预设时，自动显示为 Custom
3. 选中 Custom 时下方展开子行，展示 6 个 dropdown：
   - color.srcFactor / color.dstFactor / color.operation
   - alpha.srcFactor / alpha.dstFactor / alpha.operation
4. 选中预设时隐藏子行，直接设置对应的 BlendState 值

**注意**：当前 `RenderState` 用 wgpu 原生类型直接序列化（`Serialize/Deserialize` 由 wgpu 提供）。改为包装类型后需要保持向后兼容——即现有的 `.mat` 文件在新代码下仍能反序列化。

## 3.5 Module E：持久化与脏标记

Apply 按钮逻辑：
1. 从 `SerializedMaterialAssetsSystem` 获取当前的 `SerializedMaterial`
2. 调用 `SerializedMaterial::save_to_file()`（已有实现，见 `serialize_asset/material.rs`）
3. 重置 `dirty = false`

Ctrl+S 快捷键：在 `draw()` 中检测 `ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S))`，触发 Apply 逻辑。

关闭确认对话框：复用 `ConfirmDialogWindow`，同 TextureInspector 的 `on_exit()` 实现。

## 3.6 Module F：3D Preview

参考 MeshInspector 的实现：
- 在 `render()` 中创建临时 color attachment + depth attachment
- `command.begin_render_pass()` 绘制网格 + 当前材质
- `command.bind_attachment_to_egui()` 将渲染结果传给 egui
- 在 `draw()` 中用 `ui.painter().image()` 显示

预览网格切换：
- 网格 handle 需要是 `AssetHandle<MeshAssetsSystem>`，从 Style TOML 的路径加载
- 下拉栏切换时重新 `assets_server.load()` 新网格

**坑点**：
- Preview attachment 的大小需要跟随面板 resize（参考 MeshInspector 的 `preview.size` 在 draw() 中更新）
- 多 preview 共存的场景：材质 inspector 和 mesh inspector 同时打开时，各自有独立的 attachment

## 3.7 Module G：降级资源

### error_shader.wgsl

纯紫色输出：

```wgsl
@vertex fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
    let pos = array(vec2f(-1, -1), vec2f(3, -1), vec2f(-1, 3));
    return vec4f(pos[vi], 0.0, 1.0);
}

@fragment fn fs() -> @location(0) vec4f {
    return vec4f(1.0, 0.0, 1.0, 1.0); // 纯紫
}
```

注意：error_shader 在渲染管线中已有类似概念（编译出错时的 purple fallback），但那是 GPU 级别的降级（bind group 层面）。这里的 error_shader 是资产级别的降级——当 `.mat` 引用的 shader 文件不存在时使用。

### white.texture

2×2 纯白纹理：创建一个 2×2 RGBA8(255,255,255,255) 像素数据，通过现有纹理导入流程生成 `.texture` + `.texture_bin`。

### Fallback 逻辑

在 MaterialInspector 的 shader/texture 切换时，检查目标文件是否存在（`path.exists()`）：
- 不存在 → 用降级资源路径替代
- 存在 → 正常加载

**坑点**：降级资源本身也可能缺失。需要确保 `error_shader.wgsl` 和 `white.texture` 在引擎初始化时就被加载并缓存，防止无限递归 fallback。
