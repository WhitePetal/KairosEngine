# Material Inspector 设计案 — Phase 4：代码实现分析

## 4.1 Module A：基础框架

### 4.1.1 SerializedMaterialAssetsSystem

**文件**：新建 `kairos_engine/src/asset_loader/assets/asset/serialized_material.rs`

**结构**：与 `MaterialAssetsSystem` 平行，读取同一 `.mat` 文件但只存储 `SerializedMaterial`。

```rust
// ============================================================
// LoadedEvent / DropEvent
// ============================================================

#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    asset: SerializedMaterial,
}
impl asset::LoadedEvent<SerializedMaterial> for LoadedEvent {
    fn get_index(&self) -> AssetIndex { self.index }
    fn get_asset(self) -> SerializedMaterial { self.asset }
}

#[derive(Debug)]
pub struct DropEvent { index: AssetIndex }
impl asset::DropEvent for DropEvent {
    fn new(index: AssetIndex) -> Self { Self { index } }
    fn get_index(&self) -> AssetIndex { self.index }
}

// ============================================================
// Loader（比 MaterialAssetsSystem 的 Loader 简单——无依赖加载）
// ============================================================

pub struct Loader;
impl Loader {
    async fn load(
        path: PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
    ) -> Result<(), Error> {
        let toml = tokio::fs::read(&path).await?;
        let serialized: SerializedMaterial = toml::from_slice(&toml)?;
        sender.send(LoadedEvent { index: asset_index, asset: serialized }).await?;
        Ok(())
    }
}
impl asset::AssetLoader<LoadedEvent, SerializedMaterial> for Loader {
    fn load_asset(
        &self,
        path: PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
        _denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) {
        tokio::spawn(Self::load(path, asset_index, sender));
    }
}

// ============================================================
// SerializedMaterialAssetsSystem
// ============================================================

pub struct SerializedMaterialAssetsSystem {
    assets: Assets<Self>,
}
impl SerializedMaterialAssetsSystem {
    pub fn new() -> Self {
        let loader = Loader {};
        let assets = Assets::<Self>::new(
            loader,
            consts::SERIALIZED_MATERIAL_ASSETS_CAPACITY,
            consts::SERIALIZED_MATERIAL_ASSETS_LOADED_CHANNEL_BUFFER_SIZE,
            consts::SERIALIZED_MATERIAL_ASSETS_DROP_CHANNEL_BUFFER_SIZE,
        );
        Self { assets }
    }
}
impl AssetsHandler for SerializedMaterialAssetsSystem {
    fn handle_receves(&mut self) { self.assets.handle_receves(); }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
impl Default for SerializedMaterialAssetsSystem {
    fn default() -> Self { Self::new() }
}
impl AssetsSystem for SerializedMaterialAssetsSystem {
    type AssetType = SerializedMaterial;
    type LoadedEvent = LoadedEvent;
    type DropEvent = DropEvent;
    type Loader = Loader;
    fn get_assets(&self) -> &Assets<Self> { &self.assets }
    fn get_assets_mut(&mut self) -> &mut Assets<Self> { &mut self.assets }
}
```

**修改文件**：
- `asset_loader/consts.rs`：添加容量常量
- `asset_loader/assets/asset.rs`：添加 `mod serialized_material` + `pub use`

### 4.1.2 ProjectPathGraph::find_assets_by_kind

**文件**：`kairos_editor/project_path_tree.rs`

```rust
impl ProjectPathGraph {
    pub fn find_assets_by_kind(&self, kind: AssetKind) -> Vec<&ProjectTreeNode> {
        self.graph.node_weights()
            .filter(|n| n.kind == kind)
            .collect()
    }
}
```

### 4.1.3 InspectorCreater 修改

**文件**：`kairos_editor/ui/inspector/creater.rs`

签名增加 `project_graph: &ProjectPathGraph` 参数：

```rust
pub fn create_from_asseet_kind(
    asset_kind: AssetKind,
    path: &Path,
    assets_server: &mut AssetsServer,
    project_graph: &ProjectPathGraph,  // 新增
) -> Result<Box<dyn Inspector>, Box<dyn std::error::Error>> {
    match asset_kind {
        // ... 其他分支不变 ...
        AssetKind::Material => Ok(Box::new(MaterialInspector::create(path, assets_server, project_graph)?)),
        // ...
    }
}
```

**调用处修改**：`ProjectWindow::get_selected_node_info()` 中传入 `&self.model.project_path_graph`。

### 4.1.4 MaterialInspector 结构体

**文件**：新建 `kairos_editor/ui/inspector/material.rs`

```rust
pub struct MaterialInspector {
    model: MaterialInspectorModel,
}

struct MaterialInspectorModel {
    style: MaterialInspectorStyle,
    mat_path: PathBuf,
    // 可序列化数据（通过 assets_server get 获取，不直接持有）
    serialized_handle: Arc<AssetHandle<SerializedMaterialAssetsSystem>>,
    // 运行时数据句柄
    material_handle: Arc<AssetHandle<MaterialAssetsSystem>>,
    // 预览网格句柄（可选）
    mesh_handle: Option<Arc<AssetHandle<MeshAssetsSystem>>>,
    selected_mesh_index: usize,
    // 所有 .wgsl 路径（从 ProjectPathGraph 查询，create 时缓存）
    shader_paths: Vec<PathBuf>,
    // 当前选中的 shader 索引
    selected_shader_index: usize,
    // 上次 insert 的 shader handle（异步加载）
    pending_shader_handle: Option<Arc<AssetHandle<ShaderAssetsSystem>>>,
    // 脏标记
    dirty: Cell<bool>,
}

/// 预览状态（参考 MeshInspector::PreviewState）
struct PreviewState {
    egui_texture_id: Option<egui::TextureId>,
    bind_receiver: Option<oneshot::Receiver<egui::TextureId>>,
    pending_drop_id: Option<egui::TextureId>,
    size: (u32, u32),
    camera: SceneCamera,
}
```

### 4.1.5 Style TOML

**文件**：新建 `Preferences/Styles/Inspectors/Material.toml`

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

**修改** `kairos_editor/ui/paths.rs`：
```rust
pub const PATH_MATERIAL_INSPECTOR_STYLE: &'static str =
    "Preferences/Styles/Inspectors/Material.toml";
pub const PATH_ERROR_SHADER: &'static str = "res/shaders/error_shader.wgsl";
pub const PATH_WHITE_TEXTURE: &'static str = "res/textures/white.texture";
```

### 4.1.6 MaterialInspector::create()

```rust
impl Inspector for MaterialInspector {
    fn create(
        path: &Path,
        assets_server: &mut AssetsServer,
        project_graph: &ProjectPathGraph,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let style = MaterialInspectorStyle::new()?;
        let mat_path = path.to_path_buf();

        // 加载序列化材质
        let serialized_handle = assets_server.load::<SerializedMaterialAssetsSystem>(&mat_path);
        // 加载运行时材质
        let material_handle = assets_server.load::<MaterialAssetsSystem>(&mat_path);

        // 查询所有 .wgsl
        let shader_paths: Vec<PathBuf> = project_graph
            .find_assets_by_kind(AssetKind::Shader)
            .iter()
            .map(|n| n.path.clone())
            .collect();
        // 找到当前 shader 的索引
        // （先读取 SerializedMaterial 取 shader_path，然后匹配 shader_paths）

        let model = MaterialInspectorModel {
            style,
            mat_path,
            serialized_handle,
            material_handle,
            mesh_handle: None,
            selected_mesh_index: 0,
            shader_paths,
            selected_shader_index: 0,
            pending_shader_handle: None,
            dirty: Cell::new(false),
        };
        Ok(Self { model })
    }
}
```

> **注意**：所有现有 Inspector 的 `create()` 都需要增加 `_project_graph: &ProjectPathGraph` 参数（用下划线前缀忽略未使用的参数）。

---

## 4.2 Module B：Shader 下拉栏

### 4.2.1 Inspector trait 签名修改

```rust
pub trait Inspector: Any {
    fn create(
        path: &std::path::Path,
        assets_server: &mut AssetsServer,
        project_graph: &ProjectPathGraph,  // 新增
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized;
    // ... draw, on_exit, render 不变 ...
}
```

需要修改所有现有 Inspector 实现（audio, code, directory, document, font, mesh, shader, texture, toml, unknown）的 `create()` 签名，增加 `_project_graph: &ProjectPathGraph` 参数（用下划线前缀忽略未使用的参数）。

### 4.2.2 Shader 下拉栏 UI

在 `draw()` 中：

```rust
fn draw_shader_row(&self, ui: &mut egui::Ui, assets_server: &AssetsServer) {
    ui.horizontal(|ui| {
        ui.label("Shader:");
        let current_shader = &self.model.shader_paths[self.model.selected_shader_index];
        let display_name = current_shader.file_stem().unwrap_or_default().to_string_lossy();

        egui::ComboBox::from_id_salt("material_shader")
            .selected_text(display_name)
            .show_ui(ui, |ui| {
                for (i, path) in self.model.shader_paths.iter().enumerate() {
                    let name = path.file_stem().unwrap_or_default().to_string_lossy();
                    if ui.selectable_label(i == self.model.selected_shader_index, name).clicked() {
                        // 发送消息切换 shader（不能直接在 &self 中修改，需要通过 messager）
                    }
                }
            });
    });
}
```

### 4.2.2 Shader 切换流程

由于 `draw()` 的 `&self` 不可变，修改状态需要通过 `Messager` 发送消息：

```rust
// ui.rs 的 Message 枚举新增：
MaterialInspectorChangeShader(PathBuf),
// 或更通用：
MaterialInspectorFieldChange(FieldChange),
```

在 `draw()` 中检测到用户选择新 shader 时，发送消息。在 `Context::handle()` 中处理消息时，通过 `get_window_mut::<MaterialInspector>()` 获取可变引用进行修改。

**关键：为什么不能直接在 `draw()` 中修改？** 因为 `Inspector::draw()` 签名为 `&self`。所有可变状态修改通过消息机制传递。

**Shader 切换的异步加载流程**：

```
收到消息 → Context::handle() 中：
  1. assets_server.load::<ShaderAssetsSystem>(&new_shader_path)  → 返回 handle（异步）
  2. 设置 pending_shader_handle = Some(handle)

下一帧 draw()：
  3. 检查 pending_shader_handle 是否已加载（assets_server.get() != None）
  4. 已就绪 → 构建新 Material { shader: Some(handle), texture: ..., render_state: ... }
  5. assets_server.insert::<MaterialAssetsSystem>(new_material, &mat_path)
  6. dirty = true
  7. pending_shader_handle = None
  8. 未就绪 → 显示 "Loading shader..."
```

---

## 4.3 Module C：Texture 拖拽输入

### 4.3.1 项目树侧添加 DragSource

**文件**：`kairos_editor/ui/project_window/hierarchy_panel.rs` 和 `content_panel.rs`

在 `draw_file()` 中的节点 label 上添加 DragSource：

```rust
// 在节点 label 的 response 上
let response = ui.add(label);
let drag_id = ui.next_auto_id();  // 或基于 node 的唯一 id

let drag_response = egui::DragSource::new(drag_id)
    .begin(ui, response);

if drag_response.is_dragging() {
    // 存储 payload：节点的 asset_path（或 path）
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    // 存储拖拽数据到 egui memory
    ui.memory_mut(|mem| {
        mem.data.insert_temp(drag_id, node_data.asset_path.clone().unwrap_or(node_data.path.clone()));
    });
}
```

### 4.3.2 Inspector 侧 DropTarget

在 `draw()` 的 texture 行：

```rust
fn draw_texture_row(&self, ui: &mut egui::Ui, assets_server: &AssetsServer, messager: &mut Messager) {
    let (rect, response) = ui.allocate_rect(...);

    // Drop target 检测
    let drop_id = ui.next_auto_id();
    let drop_response = egui::DropTarget::new(drop_id)
        .show(ui, |ui| {
            // 渲染目标框内容
            if let Some(serialized) = assets_server.get(&self.model.serialized_handle) {
                if let Some(tex_path) = &serialized.texture_path {
                    // 显示缩略图 + 路径 + 清除×按钮
                    self.draw_texture_thumb(ui, tex_path, assets_server);
                } else {
                    ui.label("Drag texture here");
                }
            }
        });

    if drop_response.is_pointer_over() {
        // 高亮目标框
    }

    // 检查是否有拖拽放下
    if response.hovered() && ui.input(|i| i.pointer.any_released()) {
        if let Some(dragged_path) = ui.memory(|mem| {
            // 读取拖拽 payload，检查扩展名是否为 .texture
            // ...
        }) {
            if dragged_path.extension().map_or(false, |e| e == "texture") {
                messager.send(Message::MaterialInspectorChangeTexture(dragged_path));
            }
        }
    }
}
```

### 4.3.3 Texture 切换消息处理

在 `Context::handle()` 中：

```rust
Message::MaterialInspectorChangeTexture(tex_path) => {
    let drawer = self.get_window_mut::<MaterialInspector>();
    if let Some(inspector) = drawer {
        // 更新 serialized material 中的 texture_path
        // assets_server.load::<TextureAssetsSystem>(&tex_path) 加载纹理
        // 构建新 Material + assets_server.insert()
        // dirty = true
    }
}
```

---

## 4.4 Module D：RenderState 编辑

### 4.4.1 wgpu 类型包装

**文件**：`graphics/render_state.rs`（在当前文件中添加）

为以下类型添加包装（参考 `CompareFunction` 模式）：

```rust
// --- Face (CullMode) ---
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum Face {
    None,  // 不剔除
    Front,
    Back,
}
impl Face {
    pub fn label(&self) -> &'static str { ... }
}
impl From<Face> for Option<wgpu::Face> { ... }
impl From<Option<wgpu::Face>> for Face { ... }

// --- PrimitiveTopology ---
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum PrimitiveTopology {
    PointList,
    LineList,
    LineStrip,
    TriangleList,
    TriangleStrip,
}
impl PrimitiveTopology {
    pub fn label(&self) -> &'static str { ... }
}
impl From<PrimitiveTopology> for wgpu::PrimitiveTopology { ... }

// --- BlendFactor ---
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum BlendFactor {
    Zero, One, SrcColor, OneMinusSrcColor, SrcAlpha, OneMinusSrcAlpha,
    DstColor, OneMinusDstColor, DstAlpha, OneMinusDstAlpha,
    SrcAlphaSaturated, // ... 等
}
impl BlendFactor {
    pub fn label(&self) -> &'static str { ... }
}
impl From<BlendFactor> for wgpu::BlendFactor { ... }

// --- BlendOperation ---
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum BlendOperation {
    Add, Subtract, ReverseSubtract, Min, Max,
}
impl BlendOperation {
    pub fn label(&self) -> &'static str { ... }
}
impl From<BlendOperation> for wgpu::BlendOperation { ... }

// --- BlendPreset ---
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BlendPreset {
    Replace,
    Add,
    Multiply,
    AlphaBlend,
    Custom(BlendState),
}
impl BlendPreset {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Replace => "Replace",
            Self::Add => "Add",
            Self::Multiply => "Multiply",
            Self::AlphaBlend => "Alpha Blend",
            Self::Custom(_) => "Custom",
        }
    }
    /// 从 BlendState 推断最匹配的预设
    pub fn from_blend_state(state: &BlendState) -> Self { ... }
    /// 获取预设对应的 BlendState
    pub fn to_blend_state(&self) -> Option<BlendState> { ... }
}
```

### 4.4.2 RenderState 编辑 UI

```rust
fn draw_render_state(&self, ui: &mut egui::Ui, assets_server: &AssetsServer, messager: &mut Messager) {
    // 从 serialized_handle 获取当前的 RenderState
    // Depth Test: ComboBox<CompareFunction>
    // Depth Write: Checkbox
    // Cull Mode: ComboBox<WrappedFace>
    // Topology: ComboBox<WrappedTopology>
    // Blend: ComboBox<BlendPreset>
    //   选中 Custom → 展开子行：
    //     color.srcFactor / color.dstFactor / color.operation
    //     alpha.srcFactor / alpha.dstFactor / alpha.operation
}
```

参考 TextureInspector 的 `draw_address_mode_rows` 模式实现 Custom 展开。

---

## 4.5 Module E：持久化与脏标记

> **按实际实现修订（issue #36）**：初稿中 `MaterialInspectorApply` 为无参单元变体、由 handler
> 回读 Inspector 当前数据。实际实现改为**携带共享状态快照的负载变体**（见 §4.8），原因：
> `set_selected` 在弹出 on_exit 确认对话框时会**立即替换** Inspector，对话框里的 "Apply"
> 点击发生时，handler 通过 `get_inspector_mut` 解析到的已是新 Inspector —— 单元变体会把
> 新 Inspector 的数据写盘（或静默丢失旧编辑）。负载变体携带旧 Inspector 的
> `Arc<Mutex<...>>` 状态克隆，保存始终作用于发起时的数据（同 `TextureInspectorApply` 模式）。

### 4.5.1 Apply 按钮

按实际实现（`draw()` 底部，仅 dirty 时可用，注册 test-harness widget rect）：

```rust
let changed = self.model.dirty.get();
ui.vertical_centered(|ui| {
    ui.push_id("apply_button", |ui| {
        let apply_btn = egui::Button::new("Apply").min_size(Vec2::new(
            ui.available_width(),
            self.model.style.apply_button_height,
        ));
        let resp = ui.add_enabled(changed, apply_btn);
        // #[cfg(feature = "test-harness")] 记录 rect/egui Id（同 TextureInspector）
        if resp.clicked() {
            messager.send(self.apply_message());
        }
        if changed {
            ui.label("* unsaved changes");
        }
    });
});
```

### 4.5.2 Ctrl+S 快捷键

在 `draw()` 顶部检测：

```rust
if self.model.dirty.get() && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
    messager.send(self.apply_message());
}
```

> **按实际实现修订（跨平台）**：初稿/issue #36 为 `modifiers.ctrl`；实现改为
> `modifiers.command` —— egui 中 macOS 上映射为 ⌘、Windows/Linux 上映射为 Ctrl，
> 符合各平台保存快捷键习惯（代码库先例：`docking_tab.rs` 同样使用 `modifiers.command`）。

### 4.5.3 Apply 消息处理

在 `Context::handle()` 中（按实际实现）：

```rust
Message::MaterialInspectorApply(path, serialized_handle, shader_path, texture_path, render_state) => {
    // 静态方法：从快照构造 SerializedMaterial → save_to_file()，
    // 成功后同步资产系统缓存；失败静默打 log、返回 false
    let saved = MaterialInspector::save_material(
        &mut engine.assets_server, &path, &serialized_handle,
        &shader_path, &texture_path, &render_state,
    );
    // 仅保存成功才重置 dirty；且仅当前 Inspector 路径匹配时
    // （对话框触发时当前 Inspector 可能已是另一个资产）
    if saved
        && let Some(inspector) = self.get_window_mut::<InspectorWindow>()
        && let Some(material_inspector) = inspector.get_inspector_mut::<MaterialInspector>()
    {
        material_inspector.apply(&path);
    }
}
```

### 4.5.4 on_exit() 确认对话框

```rust
fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
    if !self.model.dirty.get() {
        return None;
    }
    // 确认 = Apply（保存后关闭），取消 = Discard（丢弃修改）
    // Discard 携带 MaterialInspectorDiscard 消息：编辑是 edit-in-place，
    // 必须将运行时 Material 还原为持久化状态（SerializedMaterial 缓存
    // 仅在保存成功时同步，始终与磁盘一致，作为还原源）
    let dialog = ConfirmDialogWindow::new(
        "Unsaved material changes".into(),
        "Apply the changes before leaving?".into(),
        "Apply".into(),
        "Discard".into(),
        Some(self.apply_message()),
        Some(self.discard_message()),
        None::<fn()>,
        None::<fn()>,
    );
    Some(Box::new(dialog))
}
```

---

## 4.6 Module F：3D Preview

### 4.6.1 draw_preview()

```rust
fn draw_preview(&self, ui: &mut egui::Ui, dt: f32) {
    let mut guard = self.model.preview.lock();
    let preview = guard.get_or_insert_with(|| {
        // 从 Style TOML 读取预览网格路径，创建 PreviewState
    });

    // 接收 bind 完成的 egui texture_id
    // 分配预览 rect
    // 渲染 image
    // 处理 orbit/zoom 交互
}
```

### 4.6.2 render()

```rust
fn render(&self) -> Option<GraphicsCommand> {
    // 参考 MeshInspector::render() 的完整实现：
    // 1. 创建 color attachment + depth attachment
    // 2. begin_render_pass
    // 3. command.draw(mesh_handle, material_handle, IDENTITY)
    // 4. end_render_pass
    // 5. command.bind_attachment_to_egui(...)
}
```

### 4.6.3 预览网格切换

下拉栏数据来自 Style TOML 的 `preview_meshes` 列表，切换时：

```rust
// 通过 messager 发送切换网格的消息
// Context::handle() 中:
//   assets_server.load::<MeshAssetsSystem>(&new_mesh_path)
//   更新 model.mesh_handle
```

> **实现修正（issue #37）**：实际实现改为 `create()` 时预加载全部
> `preview_meshes` 句柄 + 下拉栏直接写本地 `Cell<usize>` 索引（同
> MeshInspector `preview_mode` 的本地 Cell 模式），不走
> `MaterialInspectorChangePreviewMesh` 消息。原因：预览网格集合在 create 时
> 即可确定，预加载后切换无异步空窗，也无需 Context::handle 往返；§4.8 中的
> `MaterialInspectorChangePreviewMesh(usize)` 变体因此未加入 Message 枚举。
> 另外，切换网格时会按新网格 AABB 重新取景（预览相机重置为默认朝向）。

---

## 4.7 Module G：降级资源

### 4.7.1 error_shader.wgsl

**文件**：新建 `res/shaders/error_shader.wgsl`

```wgsl
@vertex
fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {
    let pos = array(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
    return vec4f(pos[vi], 0.0, 1.0);
}

@fragment
fn fs() -> @location(0) vec4f {
    return vec4f(1.0, 0.0, 1.0, 1.0); // 纯紫
}
```

### 4.7.2 white.texture

2×2 纯白纹理。通过现有纹理导入流程生成（或手动编写 `.texture` + `.texture_bin`）：

- `.texture` TOML：width=2, height=2, format=RGBA8Unorm
- `.texture_bin`：12 bytes（4 pixels × 4 bytes = 16 bytes for RGBA8 → 2x2=4 pixels, each R=255,G=255,B=255,A=255）

### 4.7.3 Fallback 逻辑

在 shader/texture 切换时，以及 `draw()` 中获取数据时：

```rust
fn resolve_shader_path(&self, path: &Path) -> PathBuf {
    if path.exists() {
        path.to_path_buf()
    } else {
        PathBuf::from(paths::PATH_ERROR_SHADER)
    }
}

fn resolve_texture_path(&self, path: &Path) -> PathBuf {
    if path.exists() {
        path.to_path_buf()
    } else {
        PathBuf::from(paths::PATH_WHITE_TEXTURE)
    }
}
```

> **注意**：降级资源本身需要在引擎初始化时就被加载。可以在 `MaterialInspector::create()` 中对降级资源路径调用 `assets_server.load()` 预加载，确保它们始终可用。

---

## 4.8 Message 枚举扩展

**文件**：`kairos_editor/ui.rs`

```rust
pub enum Message {
    // ... 现有消息 ...

    // Material Inspector
    // 携带 (.mat 路径, serialized 句柄, shader 路径, 纹理路径, render_state)
    // 共享快照：即使 Inspector 之后被替换，保存仍作用于发起时的数据
    // （同 TextureInspectorApply 模式；取代初稿的无参单元变体）
    MaterialInspectorApply(
        PathBuf,
        Arc<AssetHandle<SerializedMaterialAssetsSystem>>,
        Arc<parking_lot::Mutex<Option<PathBuf>>>,
        Arc<parking_lot::Mutex<Option<Option<PathBuf>>>>,
        Arc<parking_lot::Mutex<Option<RenderState>>>,
    ),
    MaterialInspectorChangeShader(usize),           // 新 shader 索引
    MaterialInspectorChangeTexture(Option<PathBuf>), // 新纹理路径 / None=清除
    MaterialInspectorChangeRenderState(Box<RenderState>),
    MaterialInspectorChangePreviewMesh(usize),       // 预览网格索引
}
```

---

## 4.9 实施顺序

### 第一批次：A + G（1 session）

1. 创建 `error_shader.wgsl` + `white.texture`（+ `.texture_bin`）
2. 创建 `SerializedMaterialAssetsSystem`
3. 实现 `MaterialInspectorStyle`
4. 实现 `MaterialInspector` 最小骨架（`create` + 空的 `draw`)
5. 修改 `ProjectPathGraph` + `InspectorCreater`
6. 修改 `paths.rs`

### 第二批次：B + C + D（可并行，1-2 session 每个）

- **B**：Shader 下拉栏 + 切换逻辑 + 消息处理
- **C**：项目树 DragSource + Inspector DropTarget + 缩略图
- **D**：wgpu 类型包装 + RenderState 编辑 UI

### 第三批次：E + F（1 session）

- **E**：Apply 按钮 + Ctrl+S + dirty + on_exit
- **F**：PreviewState + draw_preview + render()
