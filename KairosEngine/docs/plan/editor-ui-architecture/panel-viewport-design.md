# 面板系统与 3D 视口设计

> **配套**：[ADR 0009](../../adr/0009-editor-panel-host-and-viewport.md)（决策）· [README](./README.md) §3.3-3.5 / §4.3（总架构）· [rewrite-plan.md](./rewrite-plan.md) Phase 1-2（排期）· [widget-layer.md](./widget-layer.md)（L0/L1/L2 分层）
> **状态**：设计稿（2026-11-09），2026-11-12 更新：docking 与面板层已落地（`kairos_editor_docking`、`kairos_editor_core`、`kairos_editor_component`，见 §6.1）；§7「原型证据」在代码全部落地后补，届时统一刷新引用行号与测试数量。
> **范围**：只回答两件事——(a) 面板如何组织：trait/registry/host、身份、状态归属、生命周期、多实例/焦点/关闭/重绘；(b) 3D 场景如何以 **egui + wgpu** 在真实 kairos 引擎里落地：渲染链路、纹理生命周期、交互与拾取、帧阶段、测试与迁移。ADR 0009 已拍板的决策本文只引用、不重复论证；这里给的是可实施细节、代码形状与 `file:line` 证据。
> **证据口径**：仓库内 `path:line` 均为 2026-11-09 工作区 HEAD 实读；原型（`prototypes/editor-ui/`）行号会随并行实现漂移，§7 统一复核。外部 API 事实以本机 cargo registry 的 `egui-0.35.0` / `egui-wgpu-0.35.0` 源码为准（与主 workspace 的 `egui = "0.35.0"` 同版本，见 `kairos_editor/Cargo.toml:50-55`）。

---

## 0. 结论摘要（TL;DR）

1. **面板身份是数据，不是 Rust 类型。** `PanelId(&'static str)` 在 core 里只被当作不透明键（open/focus 集合），`PanelInstanceId { kind, serial }` 从第一天就有；面板实现是 `kairos_editor_component` 里的 `trait Panel` 对象，由 `PanelRegistry` 按 id 造实例、由 `PanelHost` 持有实例与 dock 摆放。**加一个面板 = 实现一个 `Panel` + 组合根注册一行工厂**，不改 core、不改 app、不加 `Message` 变体（ADR 0009 决策 1/2）。
2. **`EditorApp` 只是组合根。** 它持有 `PanelHost` 和状态树，不持有任何具体的 `ProjectPanel`/`InspectorPanel` 字段。重构前的原型曾把两个面板写成字段（ADR 0009 的 Context 引用了当时的 `app.rs:34-35`），2026-11-09 的重构已改成「registry 注册 + PanelHost」；现在 `EditorApp` 只有 `core / panels / host / ui_out / frame_count`（`editor-app/src/app/mod.rs` 的 `EditorApp`），gallery 面板甚至在 host crate 里注册（§7）。
3. **状态四归属**（§2.6）：打开/焦点 → `core::LayoutState`（可持久化、MCP 可查）；dock 树/tab 顺序/分割比例/拖拽瞬态 → host/UI（`kairos_editor_docking`，独立布局文件）；面板会话态（滚动/展开/搜索/重命名缓冲/lock）→ `ViewStates[PanelInstanceId]`；GPU 纹理句柄 → host 的 `HostView`（`ViewportId → egui::TextureId`）；相机位姿 → 引擎 `World`（`EditorCameraController` 是唯一真值）。
4. **3D 视口链路（真实引擎，唯一的 UI→GPU 路径）**：`World` 的 `SceneView/GameView`（size/camera/draws）→ `Extract` 每帧重建 draws → `RenderViewports` 生成 `GraphicsCommand`（离屏 color/depth attachment + render pass）→ `bind_attachment_to_egui` → `present` 里建 texture、`register_native_texture`、oneshot 回传 `EguiTextureHandle` → host 存 `egui::TextureId` → 面板只 `painter.image(tex_id, rect, uv, tint)`。UI crate 永远看不到 wgpu 类型。
5. **拒绝三种替代方案**（§3.4）：`egui_wgpu::CallbackTrait` 画在 egui 自己的 render pass 里，**引擎的 egui pass 没有 depth attachment 且 UI crate 拿不到 `wgpu::Device`**（`kairos_editor/src/runtime.rs:329-335` 传 `None`、`kairos_graphics/src/render_pipeline.rs:916-945`），无法表达场景的离屏 pass；native child window 破坏单窗口/DPI/输入/dock；UI-owned wgpu 资源违反单一路径并制造第二份 pipeline/相机真值。callback 仍适合**轻量、自包含**的预览（单 pass、不需要引擎材质/资产缓存）。
6. **纹理生命周期是现状最大的坑**：`present` 每帧为每个 attachment 新建 wgpu texture（`render_pipeline.rs:386-405`），`BindAttachmentToEgui` 每次执行都重新 `register_native_texture`（`:526-544`），旧 id 靠 `EguiTextureHandle::drop` → mpsc → 下一次 `present` 才 `free_texture`（`:357-359, :576-580`；`egui_texture_handle.rs:18-22`）。生产必须**缓存/复用 viewport attachment texture + 显式 free**，否则要么每帧泄漏注册，要么显示陈旧帧（§3.5）。
7. **相机交互有明确时序契约**：面板只发 `ViewportAction`（`Ensure/Orbit/Pan/Zoom/Pick`），reducer 校验 ticket/尺寸后发 `Effect::{CreateViewport, SyncViewportSize, DestroyViewport, ViewportCameraInput, PickViewport}`；相机输入在 **`BeforeEngineUpdate`** 阶段写入 `SceneViewInput`，由 `camera.rs` 的 `PostUpdate` 系统消费并清零。稳态输入到屏幕是 **1 帧**（UI 帧 N 产生意图 → 帧 N+1 的 update 应用 → 帧 N+1 的 present 渲染并显示；§3.6 给出三个前提）。不满足前提（意图落到 `EngineUpdate` 之后、视口纹理每帧重建、视口 pass 排在 egui 合成之后）才退化为 2 帧。现状 `OrbitState` 只有 orbit/zoom/fly、**没有 pan**（`kairos_editor/src/camera.rs:84-113`），生产要补 pan 方法/输入字段（原型 `CameraInput` 已带 `pan`）。
8. **拾取**：原型用 CPU ray cast（`PickViewport → SceneObjectPicked` 的请求/完成路径与生产一致）；生产必须走 engine query 或 GPU id-buffer + 异步 readback——`DrawCommand` 只有 mesh/material/local_to_world、**没有 Entity**（`kairos_graphics/src/drawer.rs:31-47`），面板里做 ray cast 既拿不到相机也映射不回实体，明确禁止。
9. **帧阶段**：`BeginFrame → ApplyIntents → EngineUpdate → RenderViewports → BuildUi → PostUi(dispatch) → Present`（README §3.3.1 已定义）。`BuildUi` 可能多 pass（egui 0.35 默认 `max_passes = 2`），**只有最后一遍提交 `UiOut`**，绘制闭包里零副作用（§4）。
10. **迁移路径**：按「原型 crate → 生产 crate/模块」映射（§6.1），分 5 个可验收阶段；质量门用 `cargo test-crate <crate>`（见 `docs/agents/testing.md:5-24`），不跑 `cargo test-full`。

---

## 1. 现状：面板为什么长成「按类型索引的中心列表」

### 1.1 生产现状：`Drawer` + `ui::Context`

现在的编辑器 UI 是「一个按类型索引的 drawer 列表 + 一个按枚举变体分发的消息总线」：

```rust
// kairos_editor/src/ui.rs:326-336（实读）
pub struct Context {
    pub messager: Messager,
    layout: EditorLayout,
    ids: TypeIdMap<usize>,          // 面板类型 -> drawers 下标
    drawers: Vec<Box<dyn Drawer>>,  // 注册顺序 = 渲染顺序
    actives: Vec<bool>,             // 关闭 = 只翻 bool，实例还在
    tab_tree: DockState<usize>,     // dock 树里存 drawers 下标
    tab_viewer: KairosTabDrawer,
    dialogs: Vec<Box<dyn dialog::Dialog>>,
    global_styles: GlobalStyles,
}
```

它在四个维度上把「面板身份」和「面板实现」焊死在一起：

| 事实 | 证据 | 后果 |
|---|---|---|
| 面板身份 = Rust 类型（`TypeId`） | `ui.rs:329`（`ids: TypeIdMap<usize>`）、`ui.rs:788-798`（`push_drawer::<T>`）、`ui.rs:831-853`（`show_tab::<T>`） | 同类型只能有一个实例；id 不可序列化，布局持久化/MCP 无法寻址 |
| 关闭 = `actives[i] = false` | `ui.rs:855-863` | 实例与状态不销毁；GPU 视图只能靠「size 写 0」兜底（`ui.rs:631-637`） |
| 渲染顺序 = 注册下标 | `ui.rs:776-786`（`drawers.iter()`）、`ui.rs:332`（`DockState<usize>`） | 面板顺序是隐式语义，dock 树与注册表是两套 id |
| 面板 = 模型 + 视图 | `SceneWindowModel`（`ui/scene_window.rs:54-65`）持有 `rt_handle`/oneshot receiver；`ProjectWindowModel`（`ui/project_window.rs:49-71`）持有树/选中/重命名 | 状态分散、无法 headless 测试、跨面板只能靠 Message 互相猜 |

被这个形态逼出来的 workaround，全部有代码证据：

- **预览渲染挂在 Inspector 上**：`Inspector::render()` 是 trait 的可选方法（`ui/inspector.rs:46-50`），mesh/material 预览各自手工建 attachment、建 oneshot、`bind_attachment_to_egui`（`ui/inspector/mesh.rs:460-556`、`ui/inspector/material.rs:1219-1298`）。它证明「3D 预览 = 一条 viewport 链」，但也说明每个预览都在复制渲染握手逻辑。
- **创建失败直接 panic**：`Context::create_ui_failed` 弹原生框后 `panic!`（`ui.rs:865-869`）。面板工厂必须允许失败并显示错误态，不能杀编辑器。
- **消息总线承载一切**：`Message` 57 个变体、single `handle` 约 365 行（`ui.rs:90-206`、`ui.rs:410-774`），Scene/Game 的尺寸、纹理回绑、相机输入全是变体（`ui.rs:109-127`）；时序上 `render_ui → handle_ui → draw_ui`（`runtime.rs:264-270`）使 render 里发的消息当帧处理、draw 里发的下一帧处理（调研文档 §1.3 已列为顺序 oddity）。

### 1.2 重构前原型的形态：`EditorApp` 字段（历史证据）

2026-10-30 的合并原型为了先跑通链路，`EditorApp` 直接持有两个面板。这段形态已被 2026-11-09 的重构删除，ADR 0009 的 Context 保留了当时的 `editor-app/src/app.rs:34-35`：

```rust
// 重构前（2026-10-30）；现行形态见 §7 与 editor-app/src/app/mod.rs
pub struct EditorApp {
    core: App,
    project: ProjectPanel,
    inspector: InspectorPanel,
    ...
}
```

绘制是「字段 + `if`」：`draw_project` / `draw_inspector` 手写调用，面板开关是 `if layout.is_open(...) { egui::Panel::left(...).show(...) }`，中央视口是 `EmptyState` 占位。它能证明 reducer/Effect/组件链路，但作为最终形态有三个硬伤——这正是本轮重构的动机，现行原型已消除：

1. **单实例**：两个 Scene 视口、两个锁定的 Inspector、多个材质预览都无处安放。
2. **加面板要改 `EditorApp`**：面板字段、绘制函数、开关 `if`、快捷键分支都要动中心文件——退化回「每加功能改核心」。
3. **不可单测**：面板生命周期绑在 `eframe::App` 上，无法在 headless 下断言「打开 → 实例化 → 关闭 → 视口销毁」。现行原型用 `PanelHost` + headless harness 覆盖了这部分（§7）。

### 1.3 三个根因

1. **身份用类型/下标表达**：`TypeId`（`ui.rs:329`）和 `usize`（`ui.rs:332`）都不可序列化、不可跨版本、不能表达「同类第二个实例」。布局持久化、MCP `get_dock_layout`、命令寻址都需要稳定字符串 id（README §3.4「稳定 ID 规则」）。
2. **面板拥有模型**：`Drawer`/面板 struct 同时是视图和状态容器，靠 `Cell`/`Arc<Mutex>` 绕过 `&` 限制（现状 `ProjectWindowModel.force_expand_to`、Material 草稿 `Arc<Mutex<Option<...>>>` 等，见调研文档 §3.2）。状态不再有单一写者，就无法保证「加面板零核心改动」。
3. **面板 = 中心列表下标**：绘制顺序、dock 位置、打开集合是三套不同步的表达（`drawers` / `DockState<usize>` / `Message` 分支），任何新面板都要在每套里登记一次。

**目标形态**（ADR 0009）：面板实现是 trait object，身份是字符串 id，装配是注册表，存活/摆放在 host；状态树在 core，绘制只读 + 只发 Action。

---

## 2. 面板架构

### 2.1 目标形状

```
                    ┌────────────────────────────── kairos_editor_app ──────────────────────────────┐
  用户/菜单/命令/MCP │  EditorApp（组合根）                                                          │
        │           │   ├─ core::App（EditorState + reduce + effects）                             │
        ▼           │   ├─ PanelHost（registry + instances + ViewStates + dock + focus）           │
  Action::Layout ──►│   │    └─ PanelRegistry: PanelId → PanelFactory                          │
  (Open/Focus/      │   ├─ HostView（egui::TextureId / Theme / Log 快照）                          │
   Close)           │   └─ Frame::draw → Panel::draw(&Frame, &mut UiOut)                            │
                    └──────────────────────────────────────────────────────────────────────────────┘
                                   │ Action（唯一回流）
                                   ▼
                        core::reduce → Vec<Effect> → host 执行（IO/Engine/GPU）
```

四个对象，职责单一：

| 对象 | 所在层 | 职责 | 不做什么 |
|---|---|---|---|
| `Panel` | `kairos_editor_component` | `draw(&mut self, ui, frame, out)`：读 `&Frame`、画 widget、往 `UiOut` 推 `Action` | 不碰 `&mut World`、不做 IO、不持有领域状态、不调 `ctx.request_repaint` |
| `PanelRegistry` | `kairos_editor_component`（组合根填充） | `PanelId → PanelFactory`；启动时注册一次；提供 descriptor（标题/默认槽位/是否单例/是否拥有视口） | 不持有活实例 |
| `PanelHost` | `kairos_editor_component`（由 `kairos_editor_app` 持有并驱动） | 持有活实例、`ViewStates`、dock 树、焦点；`sync(&EditorState)` 幂等对齐；按 dock 树顺序绘制 | 不实现面板、不解释领域语义、不做 IO |
| `EditorApp` | `kairos_editor_app` | 组合根 + 帧循环编排 + EffectExecutor + ViewportHost | 不持有任何具体面板类型字段 |

### 2.2 `Panel` trait

ADR 0009 已给出签名（`docs/adr/0009-editor-panel-host-and-viewport.md`，决策 1）：

```rust
// kairos_editor_component::panel（生产目标形状；原型已按此落地，见 editor-components/src/panel/mod.rs）
pub trait Panel {
    fn kind(&self) -> PanelId;
    fn title(&self, frame: &Frame) -> String;
    fn draw(&mut self, ui: &mut egui::Ui, frame: &Frame, out: &mut UiOut);
    fn closable(&self) -> bool { true }
    fn wants_continuous_repaint(&self) -> bool { false }
}

// 原型签名是 Fn(&EditorState) -> Box<dyn Panel>：工厂可用初始状态构造会话态
pub type PanelFactory = Box<dyn Fn(&EditorState) -> Box<dyn Panel>>;
```

设计要点：

- **`&mut self` 只用于面板自己的会话态**（滚动位置、正在编辑的重命名缓冲、拖拽中的临时量）。领域状态一律从 `frame.state` 读、通过 `out.push(Action)` 改。
- **`Frame` 是只读快照 + host 资源访问器**：原型已按 `struct Frame<'a> { state: &'a EditorState, host: &'a dyn HostView }` 落地（`editor-components/src/frame.rs:22-80`），`EmptyHost` 让 headless 测试不接 host；生产再加 `world: &World`（或把 World 访问也收进 `HostView`）。`HostView` 只暴露 theme/纹理 id/log 快照等显示值，不暴露 wgpu。
- **`wants_continuous_repaint`** 是「面板需要动画帧」的显式声明（如播放中的音频波形、实时预览）。默认 `false`，host 聚合决定是否连续 `request_redraw`，避免现状「无条件每帧重绘」（`runtime.rs:384`）被复制到十几个面板。
- **无回调、无泛型参数**：面板之间不互相调用；跨面板协作只能走 Action（与 widget-layer.md §7 反模式 2 一致）。

### 2.3 `PanelId` 与 `PanelInstanceId`

```rust
// kairos_editor_core（core 只把它当不透明键；字符串内容属于 UI 词汇）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PanelId(pub &'static str);

// kairos_editor_component（实例身份属于 UI/host；若生产把 LayoutState 改成按实例键，再上移到 core）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PanelInstanceId { pub kind: PanelId, pub serial: u32 }
```

为什么 `PanelId` 是**字符串而不是核心枚举**（ADR 0009 决策 2，这里给证据与代价）：

| 维度 | 枚举 `enum PanelKind` | 字符串 `PanelId(&'static str)` |
|---|---|---|
| 新增面板 | 改 core enum + 所有 `match` | 新增常量 + 注册工厂；core 零改动 |
| 持久化/跨版本 | 变体增删要写迁移 match，旧布局文件枚举名可能变 | 字符串天然稳定；未知 id 在加载时丢弃并 warn，天然前/后兼容 |
| MCP/命令/面板表 | 需要额外枚举→字符串映射 | id 本身就是协议键 |
| 测试 | 中心 match 要跟着改 | 测试可注册 `PanelId("test-panel")` |
| 编译期穷尽性 | 有 | 无（靠注册表完整性测试补） |

- 命名规则：小写 kebab-case、稳定不改（如 `"project"`、`"inspector"`、`"viewport"`、`"console"`、`"scene"`、`"game"`、`"preview.material"`）。**禁止**用 `TypeId`、`usize`、Rust 类型名作为持久化 id（README §3.4 稳定 ID 规则；调研文档已记录项目树重扫后 `NodeIndex` 失效的教训）。
- **内置面板 id 常量可以留在 core，但不要求新增面板改 core**：core 只需要 `PanelId` 类型存 open/focus 集合；原型把内置词汇作为常量放在 `editor-core/src/state.rs:33-40`（`PROJECT/INSPECTOR/VIEWPORT/CONSOLE/...`）。新增面板**不必**加常量：host 侧直接写 `PanelId("gallery")` 即可（原型 gallery 面板即此证明，`editor-app/src/gallery.rs:21`）。唯一例外见 §2.8「谁负责销毁视口」：若 core 保留 viewport-panel 的跨 slice 规则，则 core 需要认识 `PanelId::VIEWPORT`；生产建议用注册表描述符消除它。
- `serial` 是**同 kind 的实例序号**（session 内单调、不回收），布局持久化成 `"scene#0"`、`"inspector#1"`。它不是 `Vec` 下标：关闭/重排/恢复都不会让已存布局指向另一个实例。
- 单例面板（Project/Console/Preferences）由 descriptor 标 `singleton: true`，host 分配 `serial = 0`；试图开第二个 = reducer 忽略或聚焦已存在实例（由 `LayoutAction` 语义决定，不在面板里特判）。

### 2.4 `PanelRegistry`

原型落地形状（`editor-components/src/panel/registry.rs`）：

```rust
pub type PanelFactory = Box<dyn Fn(&EditorState) -> Box<dyn Panel>>;

struct PanelRegistration { slot: DockSlot, factory: PanelFactory }
pub struct PanelRegistry { registrations: BTreeMap<PanelId, PanelRegistration> }

impl PanelRegistry {
    /// 启动期注册；重复注册替换旧项（见规则 2）。
    pub fn register(&mut self, kind: PanelId, slot: DockSlot,
                    factory: impl Fn(&EditorState) -> Box<dyn Panel> + 'static) -> &mut Self;
    /// 未知 kind => None（host 忽略），绝不 panic。
    pub fn create(&self, kind: PanelId, state: &EditorState) -> Option<Box<dyn Panel>>;
    pub fn slot(&self, kind: PanelId) -> Option<DockSlot>;
    /// 按 PanelId 排序的已注册 kind（确定性；serial 分配/tab 顺序依赖它）。
    pub fn kinds(&self) -> impl Iterator<Item = PanelId> + '_;
}
```

生产可把 `(slot, factory)` 扩展成带 `singleton` / `viewport: Option<ViewportId>` 的 `PanelDescriptor`（§2.8 的视口归属需要它），语义不变。

规则：

1. **注册发生在组合根，一次**。生产形态：`kairos_editor_app` 的装配函数里一屏 `register(...)`；原型就是 `EditorApp::new` 里 3 行（`editor-app/src/app/mod.rs`），其中 gallery 面板来自 host crate（证明 panel 不需要 core 知道）。
2. **重复注册 = 启动期最后一次生效**（原型实现是替换）；运行期路径（旧布局、MCP、未知 id）只能降级：忽略 + log，不能崩。
3. **工厂只负责造对象**，不读磁盘、不碰 `World`、不做 IO；需要的数据来自 `&EditorState` + 第一帧 `Frame`。
4. **registry 是菜单/命令/面板列表的唯一数据源**：`PanelId + DockSlot` 用于 tab/菜单/命令面板条目；新增面板的同时菜单自动出现，不需要第二处登记。
5. **可测试性**：registry 是纯数据结构，测试可注册 fake 面板并断言生命周期；原型 `editor-components/tests/panel_host.rs` 即此模式（14 个测试，§7）。
6. **确定性**：`kinds()` 按 `PanelId` 排序（BTreeMap），serial 分配/tab 顺序/测试断言与注册顺序无关。

### 2.5 `PanelHost`

原型落地形状（`editor-components/src/panel/host.rs`；生产由 host 持有同一个对象）：

```rust
pub struct PanelHost {
    registry: PanelRegistry,
    /// 已打开实例。原型按 kind 键（每 kind 一个）；生产改按 PanelInstanceId（§2.8）。
    instances: BTreeMap<PanelId, PanelInstance>,
    /// 每个槽位最近一次 active 的面板（focus 移走后仍保留该槽显示）。
    active: BTreeMap<DockSlot, PanelId>,
    next_serial: u32,
}

impl PanelHost {
    /// ApplyIntents 阶段调用一次：把 core 的 open 集合幂等地对齐到活实例。
    pub fn sync(&mut self, state: &EditorState);
    /// BuildUi 阶段调用：按 left/right/bottom/center 顺序画各槽 tab bar + active body。
    /// 这是**唯一的面板绘制入口**；app shell 从不直接调用具体面板。
    pub fn draw(&mut self, ui: &mut egui::Ui, frame: &Frame, out: &mut UiOut) -> PanelHostOutput;
    pub fn wants_continuous_repaint(&self) -> bool;
    pub fn is_open(&self, kind: PanelId) -> bool;
    pub fn instances(&self) -> impl Iterator<Item = PanelInstanceId> + '_;
    pub fn instance_count(&self) -> usize;
}
```

`sync` 的语义（原型实现，`editor-components/src/panel/host.rs`）：

```rust
pub fn sync(&mut self, state: &EditorState) {
    // 1) 关闭：host 里有、core 里没有 => drop 实例（生产可保留 ViewState）
    self.instances.retain(|kind, _| state.layout.is_open(*kind));
    // 2) 打开：按 registry.kinds() 的确定顺序建实例，serial 单调分配
    for kind in self.registry.kinds() {
        if !state.layout.is_open(kind) || self.instances.contains_key(&kind) { continue; }
        let Some(slot) = self.registry.slot(kind) else { continue };
        let Some(panel) = self.registry.create(kind, state) else { continue }; // 未注册 => 忽略
        let serial = self.next_serial;
        self.next_serial = self.next_serial.wrapping_add(1);
        self.instances.insert(kind, PanelInstance { id: PanelInstanceId::new(kind, serial), panel, slot });
    }
}
```

三条硬规则：

1. `sync` **幂等**：连续两帧同一 `LayoutState` 不产生任何变化；布局从磁盘/MCP 恢复时也只是同一入口。
2. `sync` **只在 ApplyIntents 阶段跑一次**（帧阶段见 §4）；`draw`（BuildUi）不创建/销毁实例、不修改 dock 状态。
3. **删除与创建不对称**：关闭 panel drop 实例；生产把会话态按实例保留（带 LRU 上限，如 32 个），重开同 id 实例可恢复滚动/展开；GPU 视口销毁由 §2.8 的规则走 Effect。

`draw` 的输出 `PanelHostOutput { active_panels, panel_rects, tab_rects, close_rects, wants_continuous_repaint }` 只给 host/测试用（浮层锚定、合成点击输入）；生产绘制不需要回读它。

> **原型简化（§7.4）**：实例按 `PanelId` 键（每 kind 一个）、`active` 按槽位、view state 存在 panel 结构体自身。生产按 `PanelInstanceId` 键 + `ViewStates` 即可；`PanelInstanceId` 已在原型里存在（`panel/mod.rs` 的 `PanelInstanceId`），`panel/mod.rs` 的扩展点注释明确这是 store/sync 的改动、trait/host/tab-bar API 不变。

### 2.6 状态归属

| 状态 | 归属 | key | 可序列化 | MCP 可查 | 例子 |
|---|---|---|---|---|---|
| 打开的实例集合、焦点 | `core::LayoutState`（原型按 `PanelId` 键；生产建议按 `PanelInstanceId`） | `PanelId` → `PanelInstanceId` | 是 | 是 | 「Inspector 开着、焦点在 Scene」 |
| dock 树 / tab 顺序 / 分割比例 / 浮动窗口 / 拖拽中 rect | host + `kairos_editor_docking`（独立布局文件，带 `version`） | `PanelInstanceId` | 是（布局文件） | 只读快照 | 左 320px、中 tab 顺序 |
| 面板会话态（滚动、展开集合、搜索词、重命名 buffer、Inspector lock、列宽） | `ViewStates[PanelInstanceId]` | 实例 | 可选（按需） | 否 | 树展开到 `assets/models` |
| 纯瞬态（hover、popup 开合、光标、拖拽中的 pointer 状态） | egui `Memory`（稳定 `Id`） | egui id | 否 | 否 | 下拉框是否展开 |
| 视口登记（请求尺寸、是否 bound、pending bind/pick ticket、last_error） | `core::ViewportState` | `ViewportId` | 是（意图） | 是 | 「Scene 视口 1280×720、已绑定」 |
| GPU 纹理句柄、相机位姿、深度缓冲 | host `HostView` / 引擎 `World` | `ViewportId` / `Entity` | 否 | 经 World 快照 | `egui::TextureId`、`EditorCameraController` |
| 领域数据（文档、选择、项目树） | core 其余 slice | 各自 id | 是 | 是 | 当前文档、选中资产 |

**判据**（沿用 README §3.4，补两条视口专用）：

- 需要持久化 / 进 undo / 被 MCP 查询 → core。
- 只影响当前帧视觉或交互 → `ViewStates` / egui `Memory`。
- 只在 GPU / 引擎世界里存在（`TextureId`、`wgpu::Texture`、相机组件、draws）→ host / `World`，core 只存稳定 id 与「意图」。
- **同一份数据只能有一个真值**：相机位姿在 `World`，core 的 `ViewportState` 只存「尺寸意图 + 绑定状态」；UI 永不回读相机（否则又是两份真值）。

### 2.7 生命周期与数据流（open → spawn → draw → close）

一次打开到画面出现、再到关闭的完整时序（括号内为阶段，§4）：

```text
[任意线程/入口] 菜单、工具栏、命令面板、MCP、快捷键
  → Action::Layout(ShowPanel { kind }) 或 TogglePanel { kind }   （入口只发 Action）
[PostUi]        core::reduce → LayoutState.open(kind)，focus = kind
                → Effect::PersistLayout（可选，帧末/后台）
[下一帧 ApplyIntents] PanelHost::sync(&state, ctx)
                → registry.create(kind, state)（factory 唯一调用点）
                → 按 registry.slot(kind) 放进对应四槽
                → 生产：ViewStates.entry(instance).or_default()
[BuildUi]       host.draw → PanelChrome + Panel::draw(ui, &frame, &mut out)
                → 视口面板 out.push(ViewportAction::Ensure { viewport, w, h })
[PostUi]        dispatch(Ensure) → reducer 校验/开 ticket → Effect::CreateViewport
[下一帧 BeginFrame] 执行 BeforeEngineUpdate effect：确保相机实体、写 view 尺寸/camera 绑定
[EngineUpdate]  Engine::update：PostUpdate 相机控制器消费输入 → Extract 重建 draws
[RenderViewports] 为每个可见 viewport 生成 GraphicsCommand（attachment + pass + bind）
[Present]       执行渲染图 → register_native_texture → oneshot(EguiTextureHandle) → host inbox
[再下一帧 BeginFrame] host drain oneshot → dispatch(ViewportAction::BindCompleted)
                → 下一帧 BuildUi 起 painter.image 才显示第一帧
[关闭]          Action::Layout(ClosePanel { panel: kind })
                → reducer 移除 LayoutState 项（+ 拥有视口 => DestroyViewport effect，见 §2.8）
[ApplyIntents]  host.sync(layout, ctx)：drop 面板实例、移出槽位；生产保留 ViewStates
[BeginFrame]    DestroyViewport：view.size = 0（Extract 跳过）、drop EguiTextureHandle
[下一次 Present] free_texture 真正回收
```

六条规则：

1. **工厂只在 open 时调用一次**；关闭 drop 实例；`ViewStates` 按实例保留并设上限（避免长时间编辑后无界增长）。
2. **打开/关闭/焦点都只是 Action**，任何入口（MCP、命令、菜单、快捷键、布局恢复）走同一 reducer，不存在「host 里直接改布局」的旁路。
3. **面板对象不碰 GPU/IO**：视口的创建/销毁是 host 执行 Effect 的结果；面板只感知 `frame.host.viewport_texture(viewport)` 是否存在（不存在时画占位/错误态）。
4. **焦点是数据**（core 存 `Option<PanelInstanceId>`），host 把它映射成 dock active tab 与焦点环；**egui 的键盘焦点不复制进 core**（那是纯瞬态，见 §2.6）。
5. **repaint 走 host 聚合**：面板只声明 `wants_continuous_repaint`；`EffectOutcome`/异步完成由 host 唤醒；纹理绑定完成等下一帧需要重绘时由 host `request_repaint`。
6. **每帧一次 sync、一次 draw**；egui 多 pass 时只提交最后一遍的 `UiOut`（§4.4）。

### 2.8 多实例、焦点、关闭、重绘

**多实例**：

- 两个 Scene 视口 = `PanelInstanceId { kind: "viewport", serial: 0/1 }`，各自映射 `ViewportId(1)/ViewportId(2)`，`ViewStates` 与 `ViewportInfo` 都按实例/id 分开——类型上无需改任何东西（原型当前每 kind 一实例；多实例是 store+dock 的迁移项，§7.4）。图省事而用「Scene 面板全局单例 + 一个全局相机」是**禁止**的：多视口是数据，不是结构改动。
- 两个 Inspector：lock 状态在各自 `ViewState`，选择仍读全局 `SelectionState`（单一真值）；lock 只是「不再跟随 selection revision」，不是复制选择。
- 标题用 `Panel::title(&frame)` 实例化（如 `Scene (2)`、`Inspector — crate.material`）；同一 kind 的多个实例在 dock tab 里可区分。

**焦点**：

- core `LayoutState.focus`（原型是 `PanelId`；生产建议 `Option<PanelInstanceId>`）；面板被点击、拖入、命令面板「跳转到」时 reducer 更新 focus。
- 绘制期 host 用 focus 画焦点环，并把 focus 实例设为 dock active tab。
- **键盘仲裁**（与现状 `runtime.rs:496-526` 的「同一按键同时喂 egui 和 InputEngine」不同）：

| 当前状态 | 键盘归属 | 规则 |
|---|---|---|
| egui 有 `TextEdit` 焦点 | egui | 输入法/文本键不进引擎；WASD 不触发相机（否则打字会飞相机） |
| 视口有指针 hover 且刚点击/拖动视口 | 场景视口 | WASD/fly 只发给 `FocusScope::Viewport(instance)` |
| 其他（面板/工具栏/菜单） | UI | 快捷键走 `CommandRegistry`，引擎 `InputEngine` 不喂 |
| 模态打开 | 模态 | 除 Esc/Enter 外全部截断 |

  焦点切换 = reducer 状态（`Action::Layout(FocusPanel)`）+ host 每帧算出的 `FocusScope`；两者都不是全局可变单例。

**关闭**：

- tab 关闭按钮 = `Action::Layout(ClosePanel { panel })`（生产可带 instance）；`closable() == false` 的面板（如 Game 视口在 Play 中）按不可关闭渲染。
- 关闭的副作用分两段：core 先移除布局项并（若拥有 viewport）发 `Effect::DestroyViewport`；host 在 ApplyIntents 里 drop 实例、移出 dock。GPU 回收是下一帧 present 的事（§3.5），面板对象不等 GPU。
- 重新打开：新 ticket、新 panel 实例、新 `CreateViewport`；**相机实体不销毁**（对应 `scene_window.rs:494-520` 的 `spawn_editor_camera` 幂等语义），所以重开保持视角。这是「相机真值在 World」的直接好处。
- **谁负责销毁视口**：原型把跨 slice 规则放在 layout reducer（关闭拥有视口的 panel → `DestroyViewport`，见 `prototypes/editor-ui/crates/editor-core/src/reduce/layout.rs`）。这条规则让 core 认识了 `PanelId::VIEWPORT`，属于知识泄漏。生产建议改为：registry descriptor 声明 `viewport: Option<ViewportId>`；`ClosePanel` reducer 只移除布局并发 `Effect::DetachPanel`，host 执行该 effect 时按描述符发 `Action::Viewport(DestroyRequested)`（host 发 Action 走的是与 completion 相同的唯一入口，单写者不被破坏）。这样「哪些面板拥有视口」是注册表数据，不是 reducer 分支。

**重绘**：

| 触发源 | 旧做法 | 新做法 |
|---|---|---|
| 引擎/视口内容变化 | 无条件 `window.request_redraw()`（`runtime.rs:384`） | 有可见视口且处于播放/相机运动 → 连续帧；静止视口可降频（先保持连续，见 §6.5 风险） |
| 面板动画（音频波形、加载 spinner） | 各面板自己 `ctx.request_repaint` | `wants_continuous_repaint()` 聚合，host 决定 |
| egui 自身（文本光标、hover 动画） | egui 的 repaint callback → `EventLoopProxy`（`runtime.rs:100-109`） | 不变，仍由 host 侧 `set_request_repaint_callback` 转给事件循环 |
| 异步完成（保存/绑定/拾取） | 无统一机制 | host 在 completion 入队后 `request_repaint` |

### 2.9 加一个面板的成本（before / after）

| 步骤 | 现状（`Drawer`） | 目标（`Panel` + registry） |
|---|---|---|
| 实现 | 新增 `*_window.rs` + `impl Drawer`（10 个方法，约 8 个空实现）+ 面板 struct 兼模型 | 新增一个文件：`impl Panel`（3 个必需方法 + 2 个默认方法） |
| 身份 | `TypeId`（自动、不可见、不可序列化） | `PanelId::CONSOLE` 常量（显式、字符串、可序列化） |
| 装配 | `Context::new` 发 7 条 Message（`ui.rs:343-349`）+ `handle` 加 match 分支 + `layout.rs` 假抽屉 | 组合根 `registry.register(...)` 一行 |
| 打开/关闭 | `Message::Open/CloseXxx` 变体 + `handle` arm + `close_drawer::<T>` | 复用 `LayoutAction::{ShowPanel,TogglePanel,ClosePanel,FocusPanel}`；**不加代码** |
| 布局位置 | 改 `EditorLayout` / 手工 `append_drawer` | 注册时的 `DockSlot`（生产可由持久化布局覆盖）；用户可拖 |
| 菜单/命令面板 | toolbar 直接发 Message（没有 registry） | 菜单从 registry 生成；需要命令时再加 `CommandRegistry` 一条（可选） |
| 测试 | 不可测（构造需要 Engine/window/cwd） | headless：注册 fake 面板 → 断言 open/spawn/draw/close；纯函数测试 |
| 核心 crate 改动 | 多处 | **0** |

验收方式（可写成 CI/架构测试）：

- `git diff --stat` 新增面板的 PR 只应出现「新文件 + 组合根一行 +（可选）命令注册一行」。
- `grep -rn "<新 PanelId>" kairos_editor_core kairos_editor_app` 只在组合根命中——原型 gallery 的 `PanelId("gallery")` 就定义/注册在 host crate（`editor-app/src/gallery.rs`、`editor-app/src/app/mod.rs` 的 `EditorApp::new`），core 0 命中。
- `cargo test-crate kairos_editor_core` 不需要因为新增面板而变化——已有 reducer 测试证明 `PanelId` 是不透明数据（原型测试名：`panel_identity_is_opaque_data_so_new_panels_need_no_core_edit`）。

### 2.10 反模式（这个方向做错的常见方式）

1. **`EditorApp` 又长出面板字段**：`project: ProjectPanel`、`inspector: InspectorPanel`——最终会退化成 17 个字段 + 17 段绘制。面板只能通过 registry 进入 host。
2. **PanelId 用枚举/TypeId**：新增面板改中心文件、布局不能跨版本、MCP 不能寻址。字符串是不透明数据，语义属于 UI 词汇。
3. **把 dock 库状态当唯一真值**：`kairos_editor_docking` 只负责渲染/拖拽；open/focus 的真值在 core `LayoutState`，dock 树在 host 并可序列化。否则 MCP/命令/撤销永远看不到布局。
4. **面板持有 `&mut World`/IO/回调**：面板只读 `&Frame`、只写 `UiOut`；需要世界变更就造 Action + Effect。
5. **关闭时不释放 GPU 资源**：靠「size = 0 跳过渲染」只是视觉兜底，纹理/绑定仍占内存；必须走 `DestroyViewport`。
6. **在 draw 里 `request_repaint`/阻塞/panic**：多 pass 会放大成重复副作用；错误必须显示为面板错误态（原型 `ViewportInfo.last_error` 即此模式）。

---

## 3. 3D 视口：egui + wgpu 在真实引擎里的落地

### 3.1 引擎侧渲染链路（事实）

真实编辑器没有 eframe：`kairos_editor` 自己拥有 winit 事件循环和 wgpu surface，`egui` 作为渲染命令的一个节点进入引擎的渲染图。完整链路：

```text
主线程（winit EventLoop + egui::Context + wgpu + World 全在主线程）
│
├─ Engine::update() = world.run_schedule(Main) + clear_trackers()      engine.rs:43-46
│    First      : time_system（每帧推进 Time）                          schedule.rs:194-196, 321-331
│    PostUpdate : editor_camera_controller_system（消费 SceneViewInput） camera.rs:194-204, 217-243
│    Extract    : reset_camera_views + extract_scene_view / extract_game_view  extract.rs:36-106
│                 └─ 清空 draws、由 view.camera 派生 CameraView.view_projection、收集 draws
│
├─ RenderViewports（新架构，取代各窗口的 Drawer::render）
│    for 每个可见 viewport（core::ViewportId）:
│      GraphicsCommand::new(...)                                       graphics_command.rs:31-44
│      create_color_attachment / create_depth_attachment               graphics_command.rs:46-56
│      begin_render_pass(label, color_binds, depth_bind, vp, draws)    graphics_command.rs:64-98
│      draw(mesh, material, local_to_world) × draws                    graphics_command.rs:115-131
│      end_render_pass()                                               graphics_command.rs:100-113
│      bind_attachment_to_egui(color_attachment_id, oneshot_tx)        graphics_command.rs:155-181
│
├─ GraphicsGraph::build(Vec<GraphicsCommand>)                          graph.rs:29-163
│    · 合并多个 command 的图（每个 command 一个子图）                   graph.rs:39-136
│    · ending_nodes = 出度为 0 的节点                                  graph.rs:140
│    · combine_nodes（相邻 pass 可合并，见 LoadClear 规则）             graph.rs:141, 216-259
│    · prune_graph：只保留能到达 ending node 的链                       graph.rs:143, 165-180
│    · optimize_nodes：同 mesh+material 的 draw 合并成 instancing       graph.rs:145, 296-331
│
└─ RenderPipeline::present(&World, surface, graph)                    render_pipeline.rs:345-581
     invalidate_caches(world)                                          :355
     drain 已 drop 的 egui 纹理 id -> free_texture                     :357-359
     每个 attachment: device.create_texture(...)  ← 每帧新建！         :386-405
     每个 depth attachment: device.create_texture(...)                 :424-442
     DFS 执行节点：
       RenderPass            → 真正 begin_render_pass + 画 draws
       BindAttachmentToEgui  → egui_renderer.register_native_texture    :534-538
                              → oneshot.send(EguiTextureHandle)         :540-543
       OutputToFrameBuffer   → 记下本帧要 free 的 egui 纹理列表          :518-525
     queue.submit + poll error scopes + output.present()               :551-574
     回收本帧 drop 的 egui 纹理                                         :576-580
```

两个容易忽略的事实：

- **egui 自己的绘制也在引擎的 render pass 里**：`RenderPipeline::handle_render_pass_node` 里调用 `egui_renderer.render(&mut render_pass, paint_jobs, screen_descriptor)`（`render_pipeline.rs:896-945`）。也就是说「egui 面板」与「场景视口」最终都汇总到同一条 `GraphicsCommand` 列表、同一个 `GraphicsGraph`、同一次 `present`——这正是本设计能成立的原因：**UI 与 3D 视口共享一个渲染图，而不是两套窗口/两个后端**。
- **`RenderViewports` 在 `BuildUi` 之前拼好 `GraphicsCommand`，但 GPU 执行发生在 UI 之后的 `Present`**（帧阶段见 §4）：面板绘制时拿到的是 host 缓存的 `TextureId`，egui 的采样发生在 present 的 egui 合成 pass。因此「1 帧延迟」的关键不是 UI 何时拿到 id，而是 present 里视口 pass 与 egui pass 的**执行顺序**、以及纹理是否**稳定**（§3.6 三个前提）。原型用 eframe 作为宿主替身：`eframe::App::logic` 每帧先调 `render_viewports` 渲染所有 bound 视口，`ui` 里才 BuildUi（`editor-app/src/app/{mod,pipeline}.rs`），「先渲染后绘制」的顺序与生产语义一致；生产只是把这段替换为 `RenderViewports → GraphicsGraph::build → RenderPipeline::present`。

### 3.2 一个新视口从「打开」到「有画面」的 9 步

| # | 阶段 | 发生什么 | 代码事实 |
|---|---|---|---|
| 1 | BuildUi | 面板可见，量出 rect；面板发 `ViewportAction::Ensure { viewport, w, h }`（w/h 已按 `pixels_per_point` 换成物理像素） | 现状等价：`scene_window.rs:144-148`（`rect * pixels_per_point`，`.max(1.0)`） |
| 2 | PostUi | reducer 校验/建 `ViewportInfo` + `RequestKind::BindViewport` ticket，发 `Effect::CreateViewport` | 原型：`editor-core/src/reduce/viewport.rs` `ensure()`；ticket 校验见 `viewport_flow.rs` 测试 |
| 3 | 下一帧 BeginFrame（`When::BeforeEngineUpdate`） | 确保编辑器相机实体存在并绑到 view：`SceneView.camera = Some(entity)`；相机实体只创建一次、不销毁 | 现状等价：`ui.rs:621-629`（OpenSceneTab → `spawn_editor_camera`）+ `scene_window.rs:494-520` |
| 4 | EngineUpdate → Extract | 相机控制器已在本帧前消费输入；Extract 按 size/camera 派生 VP、重建 draws | `camera.rs:217-243`；`extract.rs:43-106` |
| 5 | RenderViewports | 按 `ViewportInfo.size` 建 color+depth attachment、render pass（含 gizmo 第二 pass），末尾 `bind_attachment_to_egui` | 现状等价：`scene_window.rs:358-480`（`GraphicsCommand::new(16, 2, 4, 16)`、两个 pass、`bind_attachment_to_egui` :472-477） |
| 6 | Present | 图执行：attachment 建 texture → register_native_texture → oneshot 发送 `EguiTextureHandle`；submit+present | `render_pipeline.rs:526-544`、`551-574` |
| 7 | 再下一帧 BeginFrame | host drain oneshot → 存 `HostView[viewport] = TextureId`（旧 handle drop ⇒ 旧 id 进 free 队列）→ dispatch `ViewportAction::BindCompleted { ticket }` | 现状等价：`scene_window.rs:365-367, 545-569`（消息 + `try_recv`）；free 队列 `render_pipeline.rs:357-359` |
| 8 | BuildUi | `frame.host.viewport_texture(viewport)` 返回 `Some(TextureId)` → `painter.image(tex, rect, uv, Color32::WHITE)` | 现状：`scene_window.rs:193-200` |
| 9 | 后续帧 | 面板每帧 `Ensure`（幂等：同尺寸零开销）→ 尺寸变化 `SyncViewportSize`；关闭 `DestroyViewport` | 原型 `ensure`/layout reducer；见 §3.9 映射表 |

> 第 7 步的「再下一帧」是**首次创建**的引擎契约：`BindAttachmentToEgui` 节点在 `present` 里执行（`render_pipeline.rs:526-544`），而 `present` 在 UI 之后（`runtime.rs:350-351`），host 只能在下一帧 BeginFrame 收到 oneshot——所以「首帧空白占位 → 第二帧出画面」是正常行为。它与 §3.6 的稳态输入延迟（1 帧）是两回事：创建成本只在第一次绑定/重建时出现。

### 3.3 关键约束（踩错就显示错误/崩溃）

1. **egui pass 没有 depth，UI crate 拿不到 `wgpu::Device`**。引擎给 egui 建 pass 时传 `None` 作为 depth attachment（`runtime.rs:329-335`，`begin_render_pass(..., None, ...)`），egui 的 `Renderer::render` 在这个 pass 内执行（`render_pipeline.rs:916-945`）。`egui_wgpu::CallbackTrait::paint` 拿到的正是**同一个** `wgpu::RenderPass<'static>`（`egui-wgpu-0.35.0/src/renderer.rs:87-120`），因此：callback 里无法做深度测试、无法开自己的 render pass、无法访问引擎的 pipeline/材质/资产缓存；`Callback::prepare` 虽然能拿 device/queue，但那个 device 只存在于 `RenderPipeline` 内部，UI crate 按架构拿不到（research §8 MUST NOT 6）。→ §3.4 拒绝方案 B 的根因。
2. **`CopyAttachmentToEGui` 不可用**：`GraphicsGraph::build` 里它是 `todo!("Not impl CopyAttachmentToEGui")`（`graph.rs:103-105`）。视口只能用 `BindAttachmentToEgui`。
3. **没有终点的链会被 prune 掉**：`ending_nodes = graph.externals(Outgoing)`，只保留能走到 ending node 的子图（`graph.rs:140, 165-180`）。每个 viewport 的 `GraphicsCommand` 必须以 `bind_attachment_to_egui`（或 `output_to_framebuffer`）结尾；只 begin/end pass 而不 bind 的视口会整条链被丢弃，表现为「面板全黑且无错误」。
4. **attachment 纹理每帧重建**：`present` 对每个非 internal attachment 都 `device.create_texture`（`render_pipeline.rs:386-405`），depth 同理（`:424-442`）。注册给 egui 的 view 也随之每帧换新（`:526-544`）。这直接造成两个问题：UI 采样到的 `TextureId` 指向的不是本帧渲染的纹理（稳态输入延迟退化为 2 帧，§3.6 前提 2）；每帧创建/销毁大纹理是纯浪费（多视口时按 §3.5 的量级）。**生产必须改造**（§3.5 末尾给方案）。
5. **render pass 合并有 Load 规则**：相邻 pass 只有在 attachment id 前缀一致、且 next 不是 `LoadClear` 时才合并（`graph.rs:216-259`）。场景的第二个 gizmo pass 用 `Load`（`scene_window.rs:442-446`），所以可合并；如果习惯性给每个 pass 传 `LoadClear`，会把主场景画没。
6. **`size == 0` = 不渲染**：`ViewportSize::is_renderable()`（`view_port.rs:39-41`）为 false 时 Extract 直接返回、`present` 不建 attachment（`extract.rs:91-94`；现状 `scene_window.rs:372-376` 的 `if !size.is_renderable() { return None }`）。关闭视口必须把 size 写 0（现状 `ui.rs:631-637`），否则会渲染一个无人显示的视口。
7. **`register_native_texture` 要求 `Rgba8Unorm`**（`egui-wgpu-0.35.0/src/renderer.rs:767-790`，文档注释 + 实现）。场景 attachment 正好是 `AttachmentFormat::RGBA8UNorm`（`attachment.rs:27-35`；`scene_window.rs:378-383`）。要改 color space 必须自己转换，不能直接把 sRGB/BGRA 格式的 attachment 注册给 egui。
8. **单窗口假设**：runtime 只处理 `ViewportId::ROOT`（`runtime.rs:274-300`），MCP/截图也按单窗口设计。多视口 = 同一窗口内多张纹理，不是多 OS 窗口；多窗口是另一次 runtime 扩展（research §8 MUST 15）。
9. **`TextureId` 只能主线程使用**：host 的 `HostView` 是主线程结构，不 `Send`；后台任务只回传纯数据（research §5.1）。

### 3.4 备选方案对比（为什么是「离屏 attachment + bind」）

| 维度 | **A. 引擎离屏附件 + `BindAttachmentToEgui`（采用）** | B. `egui_wgpu::CallbackTrait` | C. 原生子窗口 / 每视口 `Surface` | D. UI 自持 wgpu 资源 |
|---|---|---|---|---|
| 复用引擎 render graph / 材质 / 资产缓存 | ✅ 同一条图、同一套 pipeline 缓存（`render_pipeline.rs:308-317`） | ❌ 在 egui pass 内画，拿不到图/Cache | ❌ 另一条 surface 渲染路径 | ❌ 第二套缓存 |
| UI crate 是否接触 wgpu | ✅ 完全不接触（只 `TextureId`） | ❌ callback 资源是 wgpu 类型；device 不可得（§3.3-1） | ❌ UI 要管 surface | ❌ 直接违反边界 |
| 多视口（scene/game/material preview） | ✅ 每视口一条 command，图里合并（`graph.rs:39-136`） | ⚠️ 能画多个，但每个都是独立 pass 内容、顺序取决于面板绘制顺序 | ⚠️ 平台相关，视口数=窗口数 | ⚠️ 可行但资源成倍 |
| 嵌入 dock / 与面板同层 | ✅ 就是一张图 | ✅ 但 Z 序/裁剪由 egui 决定 | ❌ 原生窗口盖在 egui 上，层级/滚动/裁剪失控 | ✅ |
| DPI / 尺寸变化 | ✅ size 写 `SceneView.size`，attach 按物理像素建 | ⚠️ 需要在 prepare 里自己重配 | ❌ 每个 surface 单独处理 DPI | ⚠️ 自己做 |
| 输入路由 | ✅ egui 的 rect/response 直接可用 | ✅ 同 | ❌ OS 级输入焦点/坐标转换 | ✅ |
| 拾取 / 回读 | ⚠️ 需扩展引擎（readback） | ❌ 无 attachment 概念 | ⚠️ 平台相关 | ⚠️ 自己做 |
| 首帧/稳态延迟 | 1 帧（需 §3.6 三个前提；现状每帧重建纹理时退化 2 帧） | 1 帧（prepare 当帧） | 1–2 帧 | 1 帧 |
| 与「渲染命令是唯一 UI→GPU 路径」一致 | ✅ | ❌ | ❌ | ❌ |
| 实现成本 | 中：只需扩展「缓存纹理 + readback」 | 低（单 pass 预览） | 高（平台/窗口/输入） | 高（复制引擎能力） |
| 失败模式 | 纹理 id 生命周期（已可测） | 顺序依赖绘制位置；device 不可得 | 平台崩溃、DPI 模糊 | 双设备/双相机真值 |

**结论**：场景视口用 A。**B 不是「完全不能用」，而是只适合轻量、自包含的预览**：单 pass、不需要引擎材质/资产、不需要深度的小图（图标、曲线、缩略图、色卡）——它恰好能利用「当帧 1 帧延迟」的优势。本仓库现有的 mesh/material Inspector 预览需要引擎的 mesh/material/shader 资产（`ui/inspector/mesh.rs:460-556`、`ui/inspector/material.rs:1219-1298`），因此它们属于「视口」而不是「callback」；迁移时进 viewport 注册表（`ViewportId::Preview{...}`），并删掉 `Inspector::render` 钩子（`ui/inspector.rs:48-50`）。

**C/D 的额外否决理由**（都有本仓约束支撑）：

- C（原生子窗口）：runtime 单窗口（`runtime.rs:274-300`）、`resumed` 里只建一个 window（`runtime.rs:452-466`）；winit 子 surface 的创建/销毁/DPI/输入焦点/跨平台行为全部变成编辑器代码，且 UI 必须持有 GPU 资源——违反 research §8 MUST NOT 6 与 README §3.3「UI 只产出 GraphicsCommand」。
- D（UI 自持 wgpu）：需要第二份 `Device/Queue` 或把 `RenderPipeline` 的 device 暴露给 UI crate；两套 pipeline/material 缓存、两套相机状态，设备丢失语义也要各写一遍。现有引擎已把 wgpu 封装在 `RenderPipeline`（`render_pipeline.rs:177-206` 的字段）和 `kairos_graphics` 内，跨出这个边界没有收益。

### 3.5 纹理生命周期、尺寸与 DPI

**所有权模型**（三层，任何一层都不越界）：

| 层 | 拥有什么 | 代码位置 |
|---|---|---|
| core `ViewportState` | 尺寸**意图**、是否 bound、pending bind/pick ticket、last_error | `prototypes/editor-ui/crates/editor-core/src/viewport.rs`（`ViewportInfo`） |
| host `HostView` / `ViewportHost` | `egui::TextureId`、`EguiTextureHandle`（负责 free）、host 侧 render target 句柄 | 生产：`kairos_editor_app` 的 viewport 模块；现状等价：`scene_window.rs:54-65`（`rt_handle` + receiver） |
| engine `RenderPipeline` | `wgpu::Texture` / view / bind group（egui 注册句柄） | `render_pipeline.rs:386-405`、`:526-544` |

**状态机**（原型 core 已按此实现并测试）：

```text
                 Ensure(w,h)
   (absent) ─────────────────► creating(pending_bind=ticket)
        ▲                          │  BindCompleted(Ok)            ┌─ 尺寸又变 ─┐
        │                          ▼                               │  SyncViewportSize（重放）
        │                       bound(size, texture) ──────────────┘
        │                          │  Ensure(new w,h) => SyncViewportSize
        │  DestroyViewport         │
        └──────────────────────────┘（size=0、drop handle、下一次 present free）
   creating ── BindFailed ──► error(last_error, 不自动重试；尺寸变化或用户重试才再 Ensure)
```

**尺寸与 DPI**：

- 面板 rect 是逻辑像素；物理像素 = `round(rect * pixels_per_point)`，两轴 `max(1)`（现状 `scene_window.rs:146-148`）。**尺寸变化要防抖**：dock 拖动时每帧 ±1px 会触发每帧重建 attachment；生产建议只在物理尺寸变化 ≥1px 的稳定帧提交 `SyncViewportSize`（例如 UI 输入停止 1 帧后），或按 4px 量化后建纹理。
- DPI 变化（`ScaleFactorChanged`，`runtime.rs:512-515`）会让同一 rect 得到不同物理像素 → 走同一条 `Ensure → SyncViewportSize`，不需要特判。
- `present` 每帧新建 attachment（`render_pipeline.rs:386-405`）意味着**当前实现下 `TextureId` 指向的 view 会逐帧变化**；现状 `scene_window.rs:472-477` 只在「上一个 receiver 已消费」时重新 bind，于是注册可能滞后 1 帧，叠加 UI 读取滞后 → 最坏显示 2 帧前的场景内容。生产必须二选一：
  1. **缓存 attachment 纹理（推荐）**：给 `RenderPipeline` 增加按 `ViewportTargetId` 索引的持久 color/depth texture（`Attachment` 增加持久 id 字段，类似现有 `bind_internal_id` 机制 `attachment.rs:4-9, 62-70`），只在 create/resize 时重建；egui 注册在 create 时做一次、resize 时用 `update_egui_texture_from_wgpu_texture` 复用同一个 `TextureId`（`egui-wgpu-0.35.0/src/renderer.rs:792-812`）。**这是多视口 + 1 帧延迟（§3.6）的共同前置条件**。原型已按这个形状实现：`ViewportHost::bind` 只在首次/尺寸变化时重建 target（`crates/editor-viewport/src/host.rs:76-98`），`TextureId` 仅在 resize 时变（同文件 `:21-23` 的注释；重建见 `renderer.rs:357-379`）。
  2. 不改引擎则必须每帧重新注册（`RenderPipeline::present` 里对每个视口都 register，代价是每帧创建 bind group/sampler），并保证 host 在同一帧 Present 后更新 `HostView`——比方案 1 差，只作为过渡。
- **free 路径必须显式**：`EguiTextureHandle::drop` 把 id 发到 mpsc（`egui_texture_handle.rs:18-22`），`present` 在下一帧开头统一 `free_texture`（`render_pipeline.rs:357-359`）并在末尾回收本帧 drop 的 id（`:576-580`）。host 在丢弃 `HostView` 条目前必须保证 handle 已 drop，且下一帧 UI 不再引用该 `TextureId`（否则 painter 引用已 free 的 id）。
- **量化成本**（说明为什么必须缓存）：1080p RGBA8 color = 1920×1080×4 ≈ 8.3 MB，D24S8 depth 同样 ≈ 8.3 MB；每视口每帧重新创建 ≈ 16.6 MB/帧，3 个视口 @60fps ≈ 3.0 GB/s 的分配/带宽开销（未计 wgpu 内部开销）。场景 + 游戏 + 材质预览同时可见时这不可接受；缓存后成本只在 create/resize。

### 3.6 延迟预算：输入到屏幕 1 帧（显式契约）

稳态下相机输入在 **1 帧**后可见。原型的实现语义就是如此（`editor-app/src/app/mod.rs` 的帧阶段注释明确写了 one-frame latency；实现是 `fn logic` 先 `render_viewports`、`fn ui` 里 BuildUi + dispatch，下一帧 present 看到结果）：UI 帧 N 产生 camera intent → 帧末 effect 应用到 host camera → 帧 N+1 的 `render_viewports` 渲染 → 帧 N+1 的 present 可见。生产帧循环（§4）实现同一语义：

| 帧 | 发生的事 |
|---|---|
| N（BuildUi/PostUi） | 用户拖动 → 面板发 `ViewportAction::Orbit` → reducer 发 `Effect::ViewportCameraInput`（`When::BeforeEngineUpdate`） |
| N+1（BeginFrame） | 执行 effect：写 `SceneViewInput`（累积语义） |
| N+1（EngineUpdate） | `PostUpdate` 的 `editor_camera_controller_system` 消费输入并清零 → 写 `LocalTransform` → `Extract` 派生新的 VP/draws |
| N+1（RenderViewports） | 生成 `GraphicsCommand` |
| N+1（Present） | 按前提 3：先执行视口 render pass（写入 viewport texture），再执行 egui 合成 pass（采样同一张 texture）→ **屏幕 N+1 即包含输入结果** |

**契约：输入在 UI 帧 N 产生，present N+1 可见 = 1 帧（60Hz 下 ~16ms）。** 实现它有三个前提，缺一退化为 2 帧：

1. **意图在 `EngineUpdate` 之前应用**：`Effect::ViewportCameraInput` 必须是 `When::BeforeEngineUpdate`（§4.2）。若放到 update 之后（例如误用 `NextFrame`），输入要等 N+2 的 update 才进相机。
2. **UI 采样的纹理 = 本帧视口 pass 写入的纹理**：现状每帧新建 attachment、present 期间才注册（`render_pipeline.rs:386-405, 526-544`），UI 的 `TextureId` 指向的不是本帧内容；必须按 §3.5 做持久 viewport texture + 稳定 `TextureId`（原型 `ViewportHost::bind` 已如此，`crates/editor-viewport/src/host.rs:76-98`）。
3. **视口 render pass 先于 egui 合成 pass 执行**：egui 的绘制命令在 BuildUi 里 tessellate，但 GPU 采样发生在 present。`GraphicsGraph` 目前按 DFS 弹出的顺序执行节点（`render_pipeline.rs:458-473`），egui 命令最后追加（`runtime.rs:265` 收集视口命令、`:348` 追加 egui 命令），两个子图之间没有依赖边——执行顺序是实现的偶然结果，**不能依赖**。生产必须显式保证「先写视口、后合成 egui」：例如重新启用 `graph.rs:147-154` 注释掉的 ending-node 排序（让 `OutputToFrameBuffer` 对应的 egui pass 最后执行），或把 present 拆成两次 submit。否则即便纹理稳定，egui 也会采样到上一帧内容。

**退化说明（2 帧，不是设计目标）**：

| 原因 | 表现 | 归属/修法 |
|---|---|---|
| 意图在 `EngineUpdate` 之后应用（`NextFrame`/延迟 dispatch） | 输入晚一帧进相机 | 违反 §4.2 阶段契约 |
| 每帧重建 attachment、present 中才注册（现状 `scene_window.rs` 的握手方式） | UI 采样上一帧纹理 | §3.5 持久纹理缓存 |
| egui 合成 pass 先于视口 pass 执行 | 采样到上一帧内容 | §3.6 前提 3：显式 pass 排序/拆分 submit |

因此：**原型是 1 帧**（持久 target + render-before-UI）；**现状生产代码是 2 帧**（消息在 UI 后处理 + 每帧重建纹理 + 顺序未保证）；设计契约按 1 帧写，2 帧只作为实现缺口的退化态。内容延迟（游戏视口）同理；「材质预览」若只在 Inspector 里静态展示，可在参数变化后主动请求一帧（`request_repaint`），用户感知不到延迟。

### 3.7 多视口

- **注册表化**：`ViewportId` 是稳定 id（原型 `ViewportId(u32)` + `ViewportId::SCENE`，见 `prototypes/.../editor-core/src/viewport.rs`）；生产建议保留「内置常量 + host 实例分配」的形态（`SCENE`、`GAME`、`PREVIEW_MATERIAL`…），预览按 document/asset 分配 id。core 只存 `ViewportInfo`；GPU 资源在 host 按同一 id 索引。
- **每个视口一条 `GraphicsCommand`**，末尾自带 `BindAttachmentToEgui`；`GraphicsGraph::build` 合并所有 command 的图（`graph.rs:39-136`），所以 scene/game/preview 在同一次 present 里完成、共享一份 pipeline/material 缓存。
- **可见性裁剪**：host 的 `RenderViewports` 只为「面板可见 ∧ 已 bound」的视口生成 command；不可见视口不渲染（等价于现状 `size == 0`），重新可见时再 `Ensure`。这避免「三个视口都开着但只有两个 tab 可见」时的浪费。
- **图规模**：每个视口至少 2 个 pass（主渲染 + gizmo/overlay）、2 个 attachment、1 个 bind 节点；`GraphicsCommand::new` 的容量参数只是预分配（现状 scene 传 `16, 2, 4, 16`，`scene_window.rs:363`），不是硬上限。
- **首发范围**：`SCENE` + `GAME` 两个内置视口；材质/网格预览作为 `Preview` 视口在 P2 之后接入（见 §6.2），这样 `Inspector::render` 钩子可以整条删除。

### 3.8 交互模型

#### 3.8.1 hover / focus / pointer capture

面板侧（全部在 `ViewState`，不进 core）：

```rust
// kairos_editor_component::panels::viewport 的 ViewState（形状示意）
pub struct ViewportViewState {
    /// 正在视口里拖拽相机；egui 在按钮释放前会持续 dragged()
    drag: Option<CameraDrag>,
    /// 上一次 pick 的 NDC（用于 hover 高亮/重复点击判定）
    last_pick_ndc: Option<[f32; 2]>,
    /// 焦点环动画等纯视觉量
    focus_blink: f32,
}
enum CameraDrag { Orbit { last: egui::Pos2 }, Pan { last: egui::Pos2 } }
```

规则：

- `ui.allocate_exact_size(available, Sense::click_and_drag())`（现状 `scene_window.rs:145` 同款）拿到 rect/response；
- **hover 只影响滚轮与光标**；**拖动一旦开始就捕获指针**，直到按钮释放，期间指针移出 rect 也继续 orbit/pan（egui 的 `response.dragged()` 天然如此，不要用 `hovered() && dragged()` 组合）；
- 点击（`response.clicked()`，egui 已做拖动阈值判定）才触发 `Pick`；`dragged` 不触发 pick，避免「转完相机顺手选中」；
- 视口被点击时发 `Action::Layout(FocusPanel { instance })`（键盘焦点归属，见 §2.8）。

#### 3.8.2 手势 → 相机映射（与 `camera.rs` 逐项对应）

| 手势（原型实现） | 面板换算 | `ViewportAction` 单位 | 引擎侧落点（生产） | 证据 |
|---|---|---|---|---|
| 左键拖动 orbit | `orbit_delta(points) = [-dx*0.008, +dy*0.008]` | `Orbit { delta: [yaw, pitch] }`（**弧度**） | `OrbitState::orbit(dx, dy, tuning, dt)` 期望像素级 delta × tuning（`camera.rs:84-89`）→ executor 需换算，或给 `camera.rs` 加按 CameraInput 的入口（§3.9 细节 2） | 原型 `viewport_panel.rs:22-38`、测试 `orbit_drag_right_turns_yaw_negative_and_drag_down_raises_pitch` |
| 中键拖动 / Shift+左键拖动 pan | `pan_delta(points) = [dx/H, dy/H]` | `Pan { delta }`（**归一化到视口高**） | **需要新增** `SceneViewInput.pan` + `OrbitState::pan`；同样先换算到相机输入单位 | 原型 `viewport_panel.rs:40-44`、测试 `pan_is_normalized_to_viewport_height`；`camera.rs` 无 pan（§3.9） |
| 滚轮 | `zoom_delta(scroll) = scroll * 1/500` | `Zoom { delta }`（**无单位，正 = 拉近**） | `OrbitState::zoom`：distance 随正 delta 减小（`camera.rs:92-97`）→ 语义一致；缩放系数需换算 | 原型 `viewport_panel.rs:46-49`、测试 `scroll_up_maps_to_positive_zoom_in` |
| 左键单击 | 由 rect 归一化、y 翻转 | `Pick { ndc: [x, y] }`（NDC，y 向上） | host 用 `CameraView.view_projection` 求逆做射线（§3.8.4） | 原型 `viewport_panel.rs:60-68`、`ndc_from_pos` 测试 |
| W/A/S/D（**生产扩展，原型未实现**） | 键盘焦点在视口时转成 fly intent | 需新增 `CameraInput.fly: [right, forward]` + 对应 Action | `OrbitState::fly`（`camera.rs:101-113`）+ `SceneViewInput.fly` | 原型只覆盖鼠标输入，§7.4 列为未实现项 |

> **单位契约（原型已定，生产必须沿用语义）**：`CameraInput` 注释与测试明确 `orbit` 是弧度 `[yaw,pitch]`、`pan` 归一化到视口高、`zoom` 无单位且**正 = 拉近**（`editor-core/src/viewport.rs:99-116`；`ViewportAction` doc 见 `action.rs:154-165`）。生产 `kairos_editor::camera` 现有单位是「像素 × tuning × dt」（`camera.rs:84-97`），且没有 pan/fly-input——**这是跨 crate 的单位换算点，必须在 executor 显式完成（或给 camera.rs 增加 `apply_camera_input(CameraInput)` 入口），不能把两套单位混在一条 effect 链里**（§3.9 细节 2）。
>
> 另：现状 `OrbitState` **没有 pan**（`camera.rs:59-138` 只有 orbit/zoom/fly/position/forward/right/transform），`SceneViewInput` 也只有 orbit/zoom/fly 三个字段（`camera.rs:162-170`）。生产要补 `SceneViewInput.pan`、控制器消费、`OrbitState::pan`，并在 `camera.rs` 测试里加「pan 不改变 distance/yaw/pitch」不变量。

#### 3.8.3 egui 与引擎键盘焦点仲裁

现状是**双投递**：`egui_state.on_window_event` 先吃一遍（`runtime.rs:496-501`），随后 `WindowEvent::KeyboardInput` 无条件喂 `engine.input_engine`（`runtime.rs:520-526`），没有「谁有焦点」的概念。新架构：

```rust
// host 每帧计算一次（BeginFrame）：唯一键盘归属
pub enum FocusScope { Ui, Viewport(PanelInstanceId), Modal }

fn focus_scope(egui_ctx: &egui::Context, layout: &LayoutState, modal: &ModalState) -> FocusScope {
    if modal.is_open() { return FocusScope::Modal; }
    if egui_ctx.wants_keyboard_input() { return FocusScope::Ui; } // TextEdit/输入法
    match layout.focus() {
        Some(inst) if is_viewport(inst) => FocusScope::Viewport(inst),
        _ => FocusScope::Ui,
    }
}
```

- `FocusScope::Viewport` 时，WASD/方向键转成 `ViewportAction`；**不**再直接写 `InputEngine`；
- 其他作用域下引擎 `InputEngine` 不接收 UI 按键（游戏视口/Play 模式是另一套：Play 时 game viewport 有独立 scope，见 rewrite-plan Phase 5）；
- 快捷键（Cmd/Ctrl+S 等）永远先走 egui + `CommandRegistry`，仅在非文本焦点时触发；
- 模态打开时全部截断（现状靠 dialog 的点击顺序，不稳定）。

#### 3.8.4 点击拾取：原型 CPU raycast → 生产 GPU id-buffer / engine query

**请求/完成路径**（原型与生产一致，已被 core 测试覆盖）：

```text
面板: ViewportAction::Pick { viewport, ndc }
  → reducer: 若 bound 且无 pending pick → 开 RequestKind::PickSceneObject ticket
  → Effect::PickViewport { ticket, viewport, ndc }
  → host: 求射线/查询 → dispatch(ViewportAction::PickCompleted { ticket, object })
  → reducer: 校验 ticket → selection.selected_scene_object = object（None = 点空白，清选择）
```

- **原型（CPU ray cast）**：host 从 `CameraView.view_projection`（现有 `scene_window.rs:414-421` 即取这个）与相机 transform 反投影，得到 world ray；对原型里的少量对象做 AABB/球测试；返回 `SceneObjectId`。它验证的是**请求/完成/陈旧 ticket** 的路径，不是性能。
- **生产**：两条路，接口不变：
  1. **engine scene query**（建议目标）：`World::pick(viewport, ray) -> Option<Entity>`，由引擎用已有的场景加速结构实现；编辑器把 `Entity` 映射为 `SelectionState` 的稳定选择 id。
  2. **GPU id-buffer + readback**（fallback）：额外渲染一张 `R32Uint` attachment（实体 id），`copy_texture_to_buffer` + `map_async` 异步读回；**不能在帧内阻塞等待**（`present` 已经有一个 `pollster::block_on` 用于 error scope 弹出（`render_pipeline.rs:565`），再叠加 `recv` 会放大延迟）。仓库目前**没有**任何 `copy_texture_to_buffer`/`map_async`（grep 为空），所以这是纯新增能力，需要随 readback 一起设计错误/超时。
- **为什么面板不能自己 ray cast**：面板拿到的是只读 `Frame`（没有相机位姿——相机真值在 World），且渲染用的 `DrawCommand` 只有 `mesh/material/local_to_world`、没有 `Entity`（`drawer.rs:31-47`），命中后无法映射回实体。ray cast 属于 host/engine。
- pick 结果晚 1+ 帧（GPU readback 更晚）：可接受；hover 高亮基于上一次 pick 结果，不要每帧请求。

#### 3.8.5 Gizmo 路线图

| 阶段 | 内容 | 依赖 | 验收 |
|---|---|---|---|
| A（与视口同期） | 选中高亮：pick 后对 `SelectionState` 的实体在场景 pass 后叠加 outline/描边 pass | picking（§3.8.4） | 点选/清空/重选正确；无选中时无 outline |
| B | `core::GizmoState { mode: Translate/Rotate/Scale, space, hover: Option<Axis>, drag: Option<GizmoDrag> }` + 引擎侧 gizmo 几何（轴/环/方块）作为独立 render pass | A + 场景编辑文档 | 模式切换/悬停高亮；gizmo 随选中实体 |
| C | 拖拽交互：pointer ray → hover/active axis；drag 时发 `Action::Gizmo(Begin/Update/Commit)`；`Effect::ApplyTransform` 写 World；commit 进 undo 事务 | B + undo 事务（rewrite-plan Phase 5 的场景编辑） | 拖 1 秒 = 1 个 undo 步；Esc 取消回到起始值 |
| D | 多选/局部空间/吸附；键盘快捷键（G/R/S）走 `CommandRegistry` | C | 与主流编辑器行为对齐 |

原则：gizmo 的**命中/拖拽状态**是 core 领域状态（可 undo、可测试、MCP 可查）；它的**几何渲染**是引擎 pass（与场景共享相机/深度）；面板只负责把指针事件转成 Action——与相机输入同一套模式。

### 3.9 Effect ↔ 真实引擎资源映射

`ViewportAction` / `Effect` 与引擎的逐条对应（生产实现清单）：

| 面板动作（Action） | Effect | 执行阶段 | 真实引擎写入 | 现状等价代码 | 备注 |
|---|---|---|---|---|---|
| `Ensure { viewport, w, h }`（首次） | `CreateViewport { ticket, viewport, w, h }` | `When::BeforeEngineUpdate`（相机/尺寸）+ `RenderViewports`（attachment/bind） | 确保相机实体并 `SceneView.camera = Some(entity)`；首帧 `RenderViewports` 建 attachment、`bind_attachment_to_egui` | `ui.rs:621-629` + `scene_window.rs:494-520` + `:378-477` | 相机实体只建一次、不销毁；完成 = present 注册纹理后 oneshot |
| `Ensure`（尺寸变化） | `SyncViewportSize { viewport, w, h }` | `When::BeforeEngineUpdate` | `SceneView.size = ViewportSize::new(w, h)` | `ui.rs:638-641`；`GameView` 同（`ui.rs:661-663`） | 必须在 `EngineUpdate` 前写，否则本帧 Extract 用旧尺寸（现状消息在 UI 后处理 → 多 1 帧） |
| `Layout::ClosePanel`（拥有视口的面板） | `DestroyViewport { viewport }` | `When::BeforeEngineUpdate` | `SceneView.size = ViewportSize::new(0, 0)`；drop `EguiTextureHandle` | `ui.rs:631-637`；`view_port.rs:39-41`；`extract.rs:91-94` | 下一帧 present free 纹理；相机实体保留，重开保持视角 |
| `Orbit / Pan / Zoom`（+ 生产 `Fly`） | `ViewportCameraInput { viewport, input: CameraInput }` | `When::BeforeEngineUpdate` | 先做单位换算（§3.8.2），再累积写 `SceneViewInput { orbit, pan?, zoom, fly }` | `ui.rs:674-685`；`camera.rs:162-243` | 控制器 `PostUpdate` 消费并清零；无输入帧不写 transform（`camera.rs:239-241`）；原型单位见 `editor-core/src/viewport.rs:99-116` |
| `Pick { viewport, ndc }` | `PickViewport { ticket, viewport, ndc }` | host 异步（`NextFrame`） | 只读相机 `CameraView.view_projection`（`extract.rs:96-103`）+ engine pick/readback | 现状无 | 结果经 `ViewportAction::PickCompleted` 回 reducer；陈旧 ticket 丢弃 |
| 完成回调 | `ViewportAction::BindCompleted / PickCompleted` | `BeginFrame` | —（只改 core 状态） | 现状 `scene_window.rs:365-367,545-569` 的 oneshot try_recv | host 只负责路由，ticket 语义在 reducer（ADR 0008 决策 5） |

三个细节：

1. **`SyncViewportSize` 与 Extract 的关系**：`extract_scene_view` 在 `Extract` 阶段读 `SceneView.size`（`extract.rs:43-51, 85-106`）；`Engine::update` 是 `PostUpdate → Extract` 一次跑完（`schedule.rs:163-183`）。所以尺寸写入必须在 `Engine::update` 之前——这就是 `When::BeforeEngineUpdate` 的硬性理由。现状把尺寸写在 `handle_ui`（UI 阶段、update 之后）里（`ui.rs:638-641` 由 Message 触发），所以 extract 永远慢一帧（research §1.3-2）。
2. **`SceneViewInput` 是累积语义 + 单位换算点**：写入是 `input.orbit = input.orbit + delta`（`ui.rs:674-685` 的现状），控制器每帧消费后清零（`camera.rs:224-226`）。effect 执行时机若晚于 `PostUpdate`，deltas 会留到下一帧被消费（多 1 帧且可能与下一帧输入叠加）——必须遵守阶段契约。**单位换算**：原型 `CameraInput` 是 camera-ready（弧度/归一化/无单位），生产 `OrbitState` 的 orbit/zoom/fly 乘像素级 delta 与 tuning（`camera.rs:84-113`）且无 pan；建议给 `kairos_editor::camera` 增加按 `CameraInput` 语义的入口（新增 pan，内部换算），executor 只写资源，避免两套单位混在一条 effect 链里（§3.8.2 单位契约）。
3. **`ViewportBound` 的时机**：`BindAttachmentToEgui` 在 `present` 里执行（`render_pipeline.rs:526-544`）。host 在下一帧 BeginFrame drain oneshot；因此 reducer 收到 `BindCompleted` 时纹理已注册，可以安全 dispatch。若 present 未发生（surface timeout/occluded），oneshot 仍在 pending，`ViewportInfo.pending_bind` 保持——不要超时重试（§3.10）。

### 3.10 失败与恢复

| 失败 | 表现 | 处理 |
|---|---|---|
| 渲染目标创建/注册失败 | `ViewportBindFailed { error }` | reducer 记录 `last_error`，面板显示错误态 + 重试按钮；**不自动循环重试**（避免每帧失败洪泛） |
| surface lost / outdated / occluded | `get_window_surface` 的错误分支（`runtime.rs:354-369`） | 现有逻辑：resize/忽略；视口纹理随下一帧重建 |
| GPU 设备丢失 | `EditorRuntimeEvent::RenderPipelineCrash` → shutdown（`runtime.rs:475-479`） | 现状无恢复；生产至少进结构化日志 + 错误弹窗，禁止 `panic!`（research §8 MUST NOT 4） |
| 关闭时纹理未 free | 内存增长、egui 引用已 free 的 id | 关闭路径：先 `DestroyViewport`（size=0）→ drop handle → 下一帧 present free；host 同步移除 `HostView` 条目 |
| 尺寸为 0 / 最小化 | `is_renderable() == false` | Extract 跳过、不建 attachment；恢复尺寸时走 `Ensure` |
| 面板关闭后才到的 bind completion | 陈旧 ticket | reducer 校验 `pending_bind == ticket` 才接受，否则忽略（原型测试 `completion_for_a_destroyed_viewport_is_ignored`） |

---

## 4. 帧阶段与原型实现映射

### 4.1 七个阶段

| # | 阶段 | 做什么 | 可碰 | 视口相关 | 可 headless 测 |
|---|---|---|---|---|---|
| 1 | `BeginFrame` | drain 异步完成/纹理 oneshot/watcher/资产事件 → Action；执行上一帧遗留的 `BeforeEngineUpdate` effect | `&mut Engine`（仅经 executor）、host inbox | 相机实体绑定、`SceneView.size`、`SceneViewInput` 写入 | ✅（mock executor） |
| 2 | `ApplyIntents` | reduce 所有 Action；`host.sync(&layout, ctx)`；纯状态迁移 | `EditorState` + host | 消费 `BindCompleted/PickCompleted`、开/关 viewport 注册 | ✅ |
| 3 | `EngineUpdate` | `Engine::update()`：PostUpdate 相机控制 → Extract 重建 draws | `&mut Engine` | 相机位姿在本帧后期（Extract）生效 | ✅（bare World harness） |
| 4 | `RenderViewports` | 为每个「可见 ∧ bound」视口生成 `GraphicsCommand`（attachment/pass/bind） | `&EditorState` + `&Engine`（只读 World） | 视口渲染图 | ⚠️ 可测命令结构（纯数据），像素需 GPU |
| 5 | `BuildUi` | `egui_ctx.run_ui`：`host.draw(ui, frame, out)` + 模态/工具栏；只读、只收集 Action | `&EditorState` + `&World` + `&mut ViewStates` + host | 面板 `Ensure/orbit/pick`、`painter.image` | ✅（egui headless + 合成输入） |
| 6 | `PostUi` | dispatch 收集到的 Action；`When::UiOnly` effect 立即执行；其余按阶段入队；按需 request repaint | `EditorState` | 下一帧 BeginFrame 前把 `ViewportCameraInput` 等排好 | ✅ |
| 7 | `Present` | tessellate → 追加 egui `GraphicsCommand`（`draw_egui` + `output_to_framebuffer`）→ `GraphicsGraph::build` → `render_pipeline.present`；退出协议 | `&World` + wgpu（只在 host/runtime 内） | 渲染图执行、纹理注册、oneshot 发出 | ❌ 需 GPU |

与现状的映射：现状 `runtime.rs:264-270` 的 `render_ui → handle_ui → draw_ui` 是隐式阶段；新架构把「视口渲染（4）」从 `run_ui` 闭包里拿出来（现在在闭包里通过 `render_ui()` 追加 commands，`runtime.rs:265`），把「命令处理（2/6）」与「绘制（5）」分开，把「present（7）」显式化。

### 4.2 `When`：每个 effect 声明生效时机

```rust
// kairos_editor_app::executor
pub enum When {
    /// 只影响 UI 状态；PostUi 内立即执行（FocusPanel、光标、打开模态）。
    UiOnly,
    /// 必须在 Engine::update 之前写 World（SceneView.size / camera / SceneViewInput）。
    BeforeEngineUpdate,
    /// 只影响下一帧（后台任务提交、持久化、原生对话框）。
    NextFrame,
    /// 帧末（日志 flush、统计、退出收尾）。
    EndOfFrame,
}
```

规则：

- effect 的执行阶段在构造时确定，**不允许执行器自己猜**；需要多个阶段的 effect 拆成多个（例如 CreateViewport = BeforeEngineUpdate 的相机绑定 + RenderViewports 的图构建）。
- 同一个 effect 只执行一次；执行器是唯一能拿 `&mut Engine` 的地方（README §3.8）。
- `BeforeEngineUpdate` effect 由上一帧 PostUi 产生、下一帧 BeginFrame 执行——这是「1 帧延迟」的唯一合法来源，禁止在 BuildUi 里顺手写 World。
- `NextFrame` 完成（IO/GPU）后以 `Action::Effect(EffectOutcome)` 回队列，不直接改状态。

### 4.3 原型 `editor-app` 的阶段映射（eframe 只是宿主替身）

原型没有真实引擎：`eframe` 只是 throwaway 宿主的 window backend，阶段按生产语义排列，但 `EngineUpdate` 段不存在（由 `editor-viewport` 的相机/场景代替），`Present` 由 eframe 后端完成。落地代码在 `prototypes/editor-ui/crates/editor-app/src/app/`（`mod.rs` 帧入口 + 组合根、`pipeline.rs` 阶段、`effects.rs` 执行器、`shell.rs` 顶栏/快捷键/modal）：

```rust
// prototypes/editor-ui/crates/editor-app/src/app/mod.rs（impl eframe::App；实读）
impl eframe::App for EditorApp {
    /// 阶段 4：每帧一次、在 UI pass 之外渲染所有 bound 视口（= RenderViewports）
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_count += 1;
        self.render_viewports();          // app/pipeline.rs；遍历 ViewportState
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.build_ui(ui);                // 阶段 2：shell + PanelHost::draw -> UiOut
        if !ui.ctx().will_discard() {     // 阶段 6：只在最后一遍 pass 提交
            self.pump(ui.ctx());          // 阶段 3+4：dispatch + 执行 Effect
        }
        // 阶段 7：eframe 后端 present；面板要求连续重绘时 build_ui 里 request_repaint_after(16ms)
    }
}
```

阶段映射（按落地模块核对；原型 2026-11-10 重构后按职责拆成 `app/` 子模块）：

| 设计阶段（生产） | 原型对应（`editor-app/src/app/`） | 说明 |
|---|---|---|
| BeginFrame | 无独立函数；`impl eframe::App::logic` 是每帧一次的入口；快捷键在 BuildUi 里采集（`shell.rs` 的 `handle_shortcuts`） | eframe 每帧回调即帧边界；completion 在 `pump` 内同步返回 |
| ApplyIntents | `pipeline.rs::build_ui` 内 `panels.sync(&state)`（只对齐实例）+ `pump` 里 `App::dispatch` reduce | 原型把「同步布局 + 面板绘制」放同一段；生产拆成 ApplyIntents / BuildUi 两段 |
| EngineUpdate | 无 | 由 `editor-viewport::ViewportHost` 的相机/场景代替；生产必须在 RenderViewports 之前跑 `Engine::update` |
| RenderViewports | `pipeline.rs::render_viewports`：遍历 `ViewportState`，每个 bound 视口 `ViewportHost::render`，在 `logic`、UI pass **之前** | 与生产「先渲染后合成」的语义一致（§3.6 前提 3）；不依赖具体面板 id（多视口是数据） |
| BuildUi | `pipeline.rs::build_ui`：每 pass 重置 `UiOut`、shell、`PanelHost::draw` + modal | 与生产一致；多 pass 语义见 §4.4 |
| PostUi / 执行 effect | `pipeline.rs::pump` + `effects.rs::apply_effect`，只在 `!will_discard()` 时执行 | 原型 effect 同步完成（保存写临时文件、视口 bind/pick 同帧回填），生产有异步 ticket |
| Present | eframe wgpu 后端 | 生产是 `GraphicsGraph::build` + `RenderPipeline::present` |

**eframe ↔ 生产宿主（已定，不再二选一）**：eframe 只是原型的 throwaway 宿主。生产 `kairos_editor_app` **接管 `runtime.rs` 的 winit + wgpu 事件循环与 `RenderPipeline::present`**；eframe 不得拥有 surface/present（否则与引擎的渲染图/帧序冲突，§3.1）。迁移时接口不变：`PanelHost` / `HostView` / `Frame` / `UiOut` 原样使用，只把「原型的 `ViewportHost`（自有 `queue.submit`）」替换为「`RenderViewports` 生成 `GraphicsCommand` + `RenderPipeline::present` 执行 + `bind_attachment_to_egui` 回传 `TextureId`」。原型的阶段/面板 API 是生产的预演，不是另一套架构。

### 4.4 多 pass 安全与重绘

- egui 0.35 的 `run_ui` 一帧内可能跑多 pass（默认 `max_passes = 2`，`request_discard` 时重跑；调研文档已记录 `egui::Grid` 首帧触发，见 research §1.3-3）。因此 `BuildUi` 闭包必须**零持久副作用**：

```rust
// host 侧（伪代码）
let mut out = UiOut::new();
ctx.run_ui(raw_input, |ui| {
    out = UiOut::new();                     // 每 pass 重建，丢弃上一 pass 的收集结果
    host.draw(ui, &frame, &mut out);        // 只读 + 只 push
    draw_modals(ui, &frame, &mut out);
});
// 闭包返回后 out 是最后一遍 pass 的结果 => 提交
dispatch(out.take());
```

- **禁止**在 BuildUi 里做：`Log::pop_front`、watcher drain、资产事件处理、`Engine` 写入、纹理创建/释放、计数自增。这些全部在 BeginFrame/ApplyIntents/Present。
- **重绘策略**：host 聚合 `wants_continuous_repaint`、pending effect 数、egui 自身 repaint 请求；当前 runtime 的无条件 `request_redraw`（`runtime.rs:384`）在迁移期保留（保证 MCP 帧语义），但生产目标是有视口/动画才连续、空闲降频（见 §6.5 风险）。

---

## 5. 测试策略

### 5.1 分层与工具

| 层 | 测什么 | 工具/命令 | 现状/原型 |
|---|---|---|---|
| core reducer（纯） | open/close/focus、多实例、Viewport 的 Ensure/尺寸/bind/pick ticket、陈旧 completion、effect 阶段 | `#[test]`；`cargo test-crate kairos_editor_core` | 原型 `editor-core/tests/viewport_flow.rs` 已有 11 个测试（§7） |
| panel host（纯 + headless egui） | 注册/工厂唯一调用、sync 幂等、打开→实例化→关闭 drop、多实例、焦点映射、未知 id 降级 | `egui::Context::default()` + `run_ui`（无需 GPU/window，research §6.3）；`cargo test-crate kairos_editor_component` | 原型 14 个（`editor-components/tests/panel_host.rs`，§7） |
| 面板绘制/交互 | 合成 `RawInput` 点击/拖拽 → 断言 `UiOut` 的 Action 序列；视口面板的 Ensure/Orbit/Pick | 同上（原型 harness `Context::run` 已用于 widget/components） | 原型 `viewport_panel` 7 个（单位/NDC）+ components 8+7；§7 |
| 相机数学（纯） | orbit/zoom/fly/pan 的公式与 clamp；无输入不改 transform | `#[test]` + bare `World` + 手工 stage（`camera/test.rs:1-15, 68-98`） | 原型 `editor-viewport` camera 16 个；生产现状 3 个 camera 测试；补 pan（§3.8.2） |
| World 级相机/Extract 契约 | `SceneViewInput` 写入 → `PostUpdate` 消费清零 → `Extract` 派生 VP/draws；`SceneView.size` 门控 | bare `World`（不建 `Engine::new()`、不碰 wgpu，`camera/test.rs:1-15`） | 可复用现有 harness |
| 视口 host（`editor-viewport`） | 离屏渲染目标创建/尺寸/销毁、纹理注册、相机输入、CPU raycast 正确性 | 纯逻辑部分 `#[test]`；GPU 部分 `#[ignore]` smoke | 原型 35 个（camera+pick 纯逻辑，§7）；GPU 仅窗口 smoke |
| GPU smoke（手动/CI 可选） | 真设备上：create→texture id、resize、destroy→free、渲染一帧无 validation error、画面非全黑 | `#[ignore]` 测试 + 环境变量开关；不在 `cargo test-crate` 默认路径 | 原型暂无 headless GPU 测试；`cargo run` 窗口 smoke + 截图（§7.3） |
| 集成（无 GPU） | 七阶段顺序、effect 按 `When` 执行、多 pass 只提交最后一遍 | mock `EffectExecutor`/`ViewportHost`，记录阶段事件 | 原型 app 阶段无自动化测试（窗口 smoke 覆盖）；生产补 |
| 手动 smoke | 开窗、选中、orbit/zoom/pan、resize、关闭重开、布局恢复 | `cd prototypes/editor-ui && cargo run` | 已跑通并截图（§7.3） |

### 5.2 必须提供的 test seam

1. **core 纯构造**：`EditorState::new()` / `App::new()` 不依赖 egui/wgpu/cwd（原型已满足，`editor-core/src/lib.rs:64-71`）。
2. **`HostView` trait**：原型已落地（`editor-components/src/frame.rs:27-49`，`EmptyHost` 默认）；测试用 `TestHost` 返回 `TextureId::User(7)` + 日志行（`editor-components/tests/panel_host.rs:97-110`），面板即可 headless 绘制。
3. **`EffectExecutor` trait**：在 host 侧抽象 IO/Engine/GPU；测试用 `MockExecutor` 记录 effect 顺序并注入 completion（ADR 0008 决策「EffectExecutor 测试可 mock」）。
4. **`ViewportHost` 抽象**（host 内部）：原型 `editor-viewport` 的 `ViewportHost` 是具体类型（`editor-viewport/src/host.rs:37`），API 已是 `bind/resize/render/texture_id/destroy/apply_camera_input/pick/set_selected`；生产建议 trait 化，测试可断言「关闭面板 → destroy 被调用一次」而不需要 GPU。
5. **bare `World` harness**：相机/extract 契约完全可以在无 `Engine::new()`（音频设备）下测（`camera/test.rs:1-15` 已示范）。

### 5.3 GPU smoke 设计

- 位置：`editor-viewport`（原型）或 `kairos_editor_app` 的 viewport 模块（生产）里的 `#[ignore]` 测试，环境变量如 `KAIROS_GPU_TESTS=1` 才运行。
- 断言（不需要读像素也能覆盖大部分风险）：
  1. 创建离屏 target → 拿到 `TextureId`；重复 create 不重复注册；
  2. resize → 旧 id 被 `update_egui_texture_from_wgpu_texture` 复用或新 id 注册、旧 id free 恰好一次；
  3. destroy → `free_texture` 恰好一次、`HostView` 条目消失；
  4. 渲染一帧 → error scope 无 validation error（当前引擎用 `pollster::block_on(scope.pop())` 收集，`render_pipeline.rs:561-569`）；
  5. 可选读回：对 1×1 清屏色做 `copy_texture_to_buffer` 断言像素（需要新增 readback，见 §3.8.4）。
- **不放进默认 `cargo test-crate`**：默认测试必须无 GPU/window（research §6.5 的明确要求）。

### 5.4 证明不了什么（诚实边界）

- **像素级正确性 / 视觉观感**：headless 无像素；原型截图只能人工看。后续可引入 `egui_kittest` 快照（同 egui 0.35），但官方建议克制（widget-layer.md §5）。
- **真实 DPI / 多显示器 / IME / 剪贴板**：需要真窗口。
- **真实延迟 / vsync / CPU 占用**：1 帧契约可以在逻辑上用帧计数/顺序测试钉住（§3.6），但真实帧时间、vsync 与呈现延迟需要 profiling。
- **dock 拖拽/多实例的视觉与手感**：需要手动 smoke。
- **GPU 驱动行为 / device lost / resize 竞态**：smoke 只能覆盖正常路径。
- **真实引擎资产/材质就绪时序**：原型视口用 demo 对象（cube/网格），不加载真实资产。

### 5.5 质量门

按 `docs/agents/testing.md:5-24` 的约定：**改哪个 crate 就测哪个**，不跑 `cargo test-full`（~`183s`，是 merge gate，`:72-76`）。`kairos_editor_core` / `kairos_editor_component` 为**独立 crate**（ADR-0008 方案，cargo 强制边界，§6.1），已于 2026-11-12 创建：core 承载布局/状态测试，component 承载面板框架与面板绘制测试，`kairos_editor_app` 只保留组合根集成测试（dock reconcile / 整帧绘制）。注意 `kairos_editor_app` 是 bin-only crate，`cargo test-crate kairos_editor_app` 的 `--lib` 会报 no library targets，用 `cargo test -p kairos_editor_app --bins --tests`。

```sh
# runtime log（三级、终端+文件、sink 扩展）
cargo test-crate kairos_log

# 领域（core）
cargo test-crate kairos_editor_core        # reducer/viewport slice

# UI（面板/组件）
cargo test-crate kairos_editor_component          # PanelHost/面板绘制/适配函数

# widget kit
cargo test-crate kairos_editor_widgets

# dock 控件
cargo test-crate kairos_editor_docking

# app/host（帧阶段、EffectExecutor、ViewportHost 纯逻辑）
cargo test-crate kairos_editor_app

# 改了公共 API/被依赖 crate 时
cargo check --workspace --all-targets

# 原型（独立 workspace，不进主构建）
cd prototypes/editor-ui && cargo test && cargo run   # 手动窗口 smoke
```

---

## 6. 生产迁移清单

### 6.1 原型 → 生产映射（按当前主 workspace 的 crate 布局）

| 原型（`prototypes/editor-ui/`） | 生产目标 | 现状 | 说明 |
|---|---|---|---|
| `editor-core-prototype` | **`kairos_editor_core`（独立 crate）** | 已建（2026-11-12，仅状态层） | `EditorState`/`LayoutState`/`PanelId`/`PanelInstanceId` 与 layout 的 `Action`/`reduce` 已迁入；其余 reducer/effect/command/viewport slice 仍留原型，等领域逻辑长出来再落这里。cargo 强制边界：不得依赖 egui/winit/wgpu/`World`。 |
| `editor-components-prototype`（`Frame`/`UiOut`/面板/组件） | **`kairos_editor_component`（独立 crate）** | 已建（2026-11-12） | 已有 `Panel`/`PanelRegistry`/`PanelHost`/`TabViewer` 适配 + `Frame`/`UiOut`/`HostView` + `panels/` 内置面板（先迁 demo 面板，真面板按 P1 逐页迁）；L2：读 core + 用 widget kit，不依赖 host（`kairos_editor_app`）、不做 IO；cargo 强制边界。 |
| `ui-widgets-prototype` | **`kairos_editor_widgets`** | 已存在（workspace 成员，骨架 `theme.rs`） | L0/L1：只依赖 egui + `kairos_color`；widget 里出现领域类型即编译失败 |
| `editor-viewport-prototype` | **`kairos_editor_app` 的 host 侧 viewport 模块**（`ViewportHost`；将来若值得再独立成 crate） | 未建 | 唯一封装 wgpu/离屏渲染的 UI 侧模块；UI crate 不依赖它 |
| `editor-app-prototype` | **`kairos_editor_app`** | 已存在（骨架，当前用 eframe 脚手架） | 组合根 + 帧阶段 + EffectExecutor + ViewportHost + 事件循环/窗口。生产接管 winit/wgpu 循环，eframe 只属于原型（见下） |
| 原型 `PanelHost` 的四槽布局（Left/Center/Right/Bottom + tab） | **`kairos_editor_docking`** 的 DockState/dock 控件（最终替身） | 已存在（`DockState<Id>` + `TabViewer` + `Docker` 单入口；app 侧 `PanelHost` 已接线） | 四槽是原型的**最小替身**，用于先证明面板 API；最终 `PanelHost` 内部把四槽换成 docking 的 `DockState<PanelInstanceId>`。docking 只负责渲染/拖拽；tab 内容由 app 的 `TabViewer`（`PanelHost`）提供，**open/focus 意图在 `kairos_editor_core::LayoutState`，布局树在 app `DockState` 并可序列化** |
| 旧 `kairos_editor`（Drawer/`ui::Context`） | 最终收缩为 runtime/engine 胶水并退役 | 现有 | 迁移完成前保留；不再接新功能（rewrite-plan §0「旧 UI 冻结」） |

**宿主形态（已拍板）**：`kairos_editor_app` **接管现有 winit + wgpu + `RenderPipeline::present` 事件循环**；**eframe 只是原型（throwaway）的 window backend，生产不得让 eframe 拥有 surface/present**。理由：真实渲染路径是 `GraphicsCommand → GraphicsGraph::build → RenderPipeline::present`，UI 只拿 `TextureId`（§3.1）；eframe 的 surface/帧序会与引擎的渲染图冲突。迁移映射：原型的 `eframe::App::{logic, ui}` 内阶段（§4.3）→ 生产的 `runtime.rs` `redraw()` 内阶段（§4.1）；原型的 `ViewportHost`（自有 `queue.submit`）→ 生产的 `RenderViewports` + `BindAttachmentToEgui` + 引擎 present；`PanelHost`/`HostView`/`Frame`/`UiOut` 接口不变。

**原型与生产 crate 的现状（2026-11-09）**：原型侧已全部落地（逐项位置与测试见 §7）；主 workspace 里只有三个新 crate 的骨架——`kairos_editor_widgets`（`theme.rs` 骨架）、`kairos_editor_app`（eframe 脚手架）、`kairos_editor_docking`（空）；`kairos_editor_core` / `kairos_editor_component` 当时尚未创建（2026-11-12 已拆出，见下一段）。因此 P1 的第一件事是把原型代码按上表搬进生产 crate，而不是从零设计。

**2026-11-12 更新（docking/PanelHost 已落地；历史中间态，已被下面两段取代）**：`kairos_editor_docking` 已重构为 **id 布局 + `TabViewer` seam**（`DockState<Id>` 只存稳定 id/分割树/tab 顺序；`Docker::show(ui, &mut viewer)` 是唯一绘制/交互入口；close/focus 走 viewer 回调，不在 dock 内部改布局）。`kairos_editor_app` 已有 `Panel` / `PanelInstanceId` / `PanelRegistry` / `PanelHost`（实现 `TabViewer`）、`LayoutState`（open/focus）和 4 个 demo 面板；每帧 `host.sync → dock tab reconcile → Docker::show`。按 owner 决定**不创建独立 headless core/UI crate**：`EditorState` 与面板 trait 暂留 app，面板通过 `PanelContext` 直接读写具体 app 状态（无 `Frame`/`UiOut` 抽象）；`kairos_editor_core` / `kairos_editor_component` 仍可作为后续可选项。

**2026-11-12 拆分（core / component 落地）**：面板层已按本节映射表拆成两个 crate：`kairos_editor_core`（`EditorState`/`LayoutState`/`PanelId`/`PanelInstanceId`，零依赖、不碰 egui）与 `kairos_editor_component`（`Panel` trait / `PanelRegistry` / `PanelHost` / `TabViewer` 适配 / `panels/` 内置面板）。`kairos_editor_app` 只剩组合根：注册表、默认 dock 布局、工具栏、窗口后端。依赖方向 `core → component → app` 由 cargo 强制。

**2026-11-12 晚（Frame/UiOut 已迁回）**：原型的 `Frame`/`UiOut` 已按原型落到生产：面板只读 `Frame { state: &EditorState, host: &dyn HostView }`、把 `Action` 推进 `UiOut`，自身不再碰 `&mut EditorState`；`kairos_editor_core::{Action, reduce}` 是状态树唯一写入口。每帧 `sync → Docker::show(ui, &mut host.viewer(&mut out, frame)) → pump`，`pump` 只在最后一个 pass（`!ui.ctx().will_discard()`）执行、逐条 `reduce`。关闭 tab 是意图而非副作用：`CloseResponse::Close` 只让 dock 摘掉 tab，viewer 推 `LayoutAction::ClosePanel`，reducer 关布局，下一帧 `PanelHost::sync` 回收实例。宿主资源（console log）走 `HostView` 而不是 `EditorState`。`docs/research/` 与 ADR 中的历史名称 `kairos_editor_ui` 指同一层，生产命名为 `kairos_editor_component`。

**2026-11-12 日志（`kairos_log` 落地，ADR 0010）**：新增 runtime crate `kairos_log` —— 进程级 logger，实现 `log::Log`（引擎里既有的 `log::*` 调用无需改动自动汇入），Info/Warning/Error 三级（debug/trace 有意丢弃），默认 stderr + `Library/Logs/KairosEngine.log`，`KAIROS_LOG_LEVEL` / `KAIROS_LOG_FILE` / `KAIROS_LOG_TERMINAL` 覆盖；额外 sink 走 `Sink` trait + `attach()`。编辑器侧**不拆** `kairos_editor_log`：console 历史是 host 资源，`kairos_editor_component::ConsoleLog` 实现 `Sink`（1000 行环，关面板不丢），`kairos_editor_app` 持有 `Arc<ConsoleLog>` 并 attach；`HostView::console_lines` 改为回调快照 `&mut dyn FnMut(&[ConsoleLine])`，行带 `Level` 供面板按主题着色。旧 `kairos_engine::log::Log`（`&mut` 实例、stderr-only、被旧 ConsoleWindow `pop_front` 消费）暂留给旧 `kairos_editor`，随 P5 退役删除。

**2026-11-12 晚（Console 面板对齐 Unity，ADR 0010 实现 note）**：`ConsoleLog` 条目升级为结构化 `ConsoleLine { id, level, time, message, target, location, stack }`；`kairos_log::Record` 记录调用点并（`RUST_BACKTRACE` 开启时）抓取 backtrace，`Record::stack_trace()` 裁掉 logger/std/winit/eframe 噪声后输出 Unity 风格 `symbol (at file:line)` 帧。Console 面板按真实 Unity/Tuanjie Console 重做：20 px 工具栏（Clear/Collapse/Error Pause/Editor、搜索框、Info/Warning/Error 计数开关）、38 px 两行交替底色行、26 px 真图标、选中行下方 130 px 调用堆栈详情面板；图标取自本机 Tuanjie/Unity 序列化资源，UI 字体用 Unity 自带 Inter（OFL）。几何/调色板/行为记录在 `docs/research/unity-console/`，落地截图 `21_*`/`22_*`。

**2026-11-13（面板资源生命周期 + 可序列化样式 + 运行时图标/字体）**：新增 `Panel::init(&mut self, ctx)`——`PanelHost::sync(layout, ctx)` 在实例创建后、首次绘制前调用一次，面板在这里加载自己的 UI 资源；不再在 app 启动时加载所有面板资源。Appearance 改成 **per-component style**：`ConsoleStyle` 移到 `kairos_editor_component::panels`，在 `Panel::init` 从 `Preferences/Styles/ConsoleStyle.ron` 加载（一个文件含 dark/light 两套，颜色序列化为 `"#RRGGBB"`）；`kairos_editor_widgets::style` 提供 `Style` trait（`FILE`/`dark()`/`light()`）和泛型 `StyleFile<S>`（load/save/defaults）；样式颜色用共享的 `kairos_color::Color32`（hex 字符串的解析/序列化由该 crate 提供，见下条），`kairos_editor_component` 的 `export_styles` example 负责把代码默认值写回 RON。全局 `Theme` 移除：各面板自己加载 style，因此改 RON + 重开面板即可换样式，不需要重编译。Console 图标仍是运行时文件（`Preferences/Textures/console/`，路径属于 style），`Panel::init` 通过 `kairos_editor_widgets::ui_image` 调用 egui 的 `egui_extras` file/image loader 异步预加载（后台线程读盘 + 解码 + `request_repaint`），绘制时采样 loader 缓存，不在 `ui()` 里读盘。字体也改成运行时文件：`EditorFonts.ron` + `Preferences/Fonts/Inter-*.otf`，app 启动时 `fonts::install` 一次；编辑器 chrome 图标/字体刻意不走 `kairos_asset`（不是项目资产、无 import/meta、需在 asset server 之前可用），项目资产缩略图以后单独走 `AssetServer` 句柄。

**2026-11-13（`kairos_color` 抽取）**：颜色类型从 `kairos_engine::math::color` 抽成独立 crate `kairos_color`：`Color32`（straight sRGBA）、hex `#RRGGBB`/`#RRGGBBAA` 解析/格式化、serde（RON 里直接写颜色字符串）；`Color32ParseError` 与 serde impl 合并进 `color.rs`，`converts.rs` 删除。crate 保持 UI 无关：egui/syntect 转换由消费者实现——新编辑器在 `kairos_editor_widgets/src/color.rs` 提供 `Color32Ext::to_egui`（扩展 trait，绘制处调用；widgets 因此新增一个 UI 无关的叶子依赖 `kairos_color`），旧编辑器在 `kairos_editor/src/color.rs` 提供 `to_egui`/`to_syntect` 并替换原 `.into()` 调用点（orphan rule 不允许在消费者 crate 里写 `From<Color32> for egui::Color32`）。`kairos_engine::math::Color32` 仍 re-export 同一类型。原 `kairos_editor_widgets::style::hex_color` serde adapter 删除；`ConsoleStyle.ron` 文件内容不变（导出脚本重新生成后 git diff 为空）。

### 6.2 分阶段落地清单

| 阶段 | 交付 | 关键文件（生产） | 验收 |
|---|---|---|---|
| **P1 面板框架** | `Panel`/`PanelId`/`PanelInstanceId`/`PanelRegistry`/`PanelHost` + 四槽 dock + Project/Inspector/Console 只读面板；`EditorApp` 变组合根 | `kairos_editor_core`（LayoutState/panel ids）、`kairos_editor_component/panel*.rs`、`kairos_editor_app/{app,install}.rs` | open/close/focus/多实例测试；「新增一个测试面板只改组合根」；`cargo test-crate` 全绿 |
| **P2 视口** | `core::ViewportState` + `ViewportHost`（离屏 target、tex 注册、相机输入、CPU picking）+ scene 视口面板；补 pan/fly | `kairos_editor_core/viewport.rs`、`kairos_editor_app/viewport_host.rs`、`kairos_editor_component/panels/viewport.rs`；`camera.rs` 补 pan | orbit/zoom/pan/fly；resize；关闭→free；重开；bind 完成/陈旧 ticket；GPU smoke |
| **P3 dock + 布局持久化** | 四槽替身换 `kairos_editor_docking`；布局文件 `version` + 未知 id 丢弃；MCP layout 快照 | `kairos_editor_docking`、`kairos_editor_app/layout_io.rs` | 重启布局保持；旧布局不崩；dock 拖拽回归测试 |
| **P4 拾取 + gizmo A/B** | scene query 或 id-buffer readback 接入；选中高亮；gizmo 状态与几何（A/B） | `kairos_editor_core/gizmo.rs`（新）、engine pick API、editor-viewport gizmo pass | 点选/清空/重选；hover outline；gizmo 模式切换 |
| **P5 收尾删旧** | 删 `Drawer`/`Messager`/`Message`/`UIReader`/`Context`/假 layout/旧 scene/game 窗口；`kairos_editor` 退役 | 见 §6.3 | grep 无残留；`cargo test-crate` 全绿；手动 smoke 清单全过 |

### 6.3 删除清单（迁移完成后）

```text
kairos_editor/src/ui.rs
  Message（57 variants）、Messager、UIReader、Drawer、Context::{handle,darw,render}
  show_tab/show_window/push_drawer/close_drawer/create_ui_failed/get_window_mut
  DockState<usize>/EditorLayout 假抽屉（layout.rs）
kairos_editor/src/ui/scene_window.rs、game_window.rs
  Drawer::render / Registe*ViewBind / TryReceTextureId / Update*WindowSize
  SceneViewOrbit/SceneViewZoom/SceneViewFly 三条 Message（改 ViewportAction）
kairos_editor/src/ui/inspector.rs
  Inspector::render() 预览钩子 → 改为 ViewportId::Preview 视口
kairos_editor/src/ui/docking_tab/**
  已由 kairos_editor_docking 的新实现替代（id 布局 + TabViewer；旧模块待 P5 删除）
kairos_editor/src/ui/layout.rs、dialog.rs
  假抽屉删除；回调式 Dialog 由 core::ModalState 替代
kairos_editor/src/lib.rs
  Editor{engine, ui_context, log} 与 ui_context 字段；on_exit 的 TODO 由 shutdown 协议替代
```

保留（不动）：`camera.rs` 的 `EditorCameraController/SceneViewInput` 系统、`extract.rs`、`view_port.rs`、`kairos_graphics` 的 render graph/render_pipeline（只做「缓存纹理 + readback」的增量扩展）。

### 6.4 架构不变量（建议进 AGENTS/CI）

| 不变量 | 检查 |
|---|---|
| core 不碰 UI/GPU/IO | `grep -rn "egui\|winit\|wgpu\|unsafe" kairos_editor_core/src` 为空；`cargo tree -p kairos_editor_core \| grep -E "egui\|wgpu"` 为空 |
| widgets 只依赖 egui + kairos_color | `cargo tree -p kairos_editor_widgets \| grep kairos \| grep -v kairos_color` 为空 |
| ui crate 不依赖 host/IO | `cargo tree -p kairos_editor_component \| grep kairos_editor_app` 为空；无 `std::fs`/`tokio` |
| 唯一 wgpu 边界 | 只有 `kairos_editor_app` 的 viewport 模块与 `kairos_graphics` 出现 `wgpu::`；面板/组件代码中 0 处 |
| 面板身份稳定 | 无 `TypeId`/`usize` 作为面板/视口 id；布局序列化只用 `PanelId/PanelInstanceId/ViewportId` |
| 加面板零核心改动 | 新增面板 PR 的 `git diff --stat` 只含新文件 + 组合根注册行；`grep <PanelId> kairos_editor_core` 为空 |
| UI→GPU 单一路径 | UI 只产出 `Action`/`GraphicsCommand`/`TextureId`；无 `wgpu::Device` 逃逸 |
| 面板无内部可变性 | 面板/组件代码无 `Cell/RefCell/Mutex/Rc` |

### 6.5 风险与回滚

| 风险 | 缓解 |
|---|---|
| 相机/布局双真值 | 相机只在 World；布局 open/focus 只在 core；host 只做映射（§2.6 判据） |
| 每帧重建 attachment 导致多视口不可用 | P2 前先做「持久 viewport texture 缓存」（§3.5）；把「>2 视口」作为性能验收项 |
| 多 pass 重复副作用 | BuildUi 零副作用 + 只提交最后 pass；测试用 mock executor 记录 effect 次数 |
| 布局文件跨版本 | `version` 字段 + 未知 PanelId 丢弃 + 默认布局兜底；**禁止**把 dock 库内部格式当持久化格式 |
| 旧 UI 共存期双写 | feature flag（编译期 `new_editor_ui` 或环境变量）；旧 UI 只读，写路径只在新架构（rewrite-plan §6） |
| 1 帧契约被实现细节破坏（纹理/顺序） | 用帧计数 + pass 顺序测试钉住 §3.6 三个前提；2 帧只允许出现在退化表里，出现即视为回归 |
| dock 迁移范围失控 | P1 先用四槽替身验证面板 API，P3 再换 docking；不重写 dock 交互（rewrite-plan 风险表同款） |
| GPU smoke 不稳定 | `#[ignore]` + 环境变量开关，默认测试集零 GPU 依赖 |

---

## 7. 原型证据（2026-11-09 冻结）

> 原型代码于 2026-11-09 冻结（`prototypes/editor-ui/` 独立 workspace，不进主构建）。以下数据为本机实测（macOS arm64，rustc/cargo 1.97.1，全部 `--offline`）；完整命令输出与测试清单见原型 README §6-§7。

### 7.1 逐项落地位置（本文设计 → 原型代码）

| 本文设计 | 原型落地位置 | 状态 |
|---|---|---|
| Panel trait / PanelInstanceId / DockSlot / PanelRegistry / PanelHost / PanelHostOutput | `editor-components/src/panel/`：`mod.rs`（PanelInstanceId/Panel/DockSlot）、`registry.rs`（PanelFactory/PanelRegistry/editor_defaults）、`host.rs`（`sync`/`draw`/`draw_slot`/`draw_panel_body` + PanelHostOutput） | ✅ |
| Frame / HostView / EmptyHost / UiOut | `editor-components/src/frame.rs:27-129`（`Frame::with_host` :72） | ✅ |
| 内置面板（Project/Inspector/Console） | `editor-components/src/{project,inspector,console}_panel.rs`；`PanelRegistry::editor_defaults`（`panel/registry.rs`） | ✅ |
| 视口面板（Ensure/Orbit/Pan/Zoom/Pick + `painter.image`） | `editor-components/src/viewport_panel.rs:51-231`：单位换算 :22-49、NDC :60-68、`painter.image` :100-105、输入 :113-165 | ✅ |
| core viewport slice（ViewportId/ViewportInfo/CameraInput/ViewportState） | `editor-core/src/viewport.rs:34-178` | ✅ |
| viewport reducers（Ensure 幂等/ticket/关闭销毁） | `editor-core/src/reduce/viewport.rs:12-186`；关闭规则 `editor-core/src/reduce/layout.rs:40-53` | ✅ |
| ViewportAction / Effect 词表 | `editor-core/src/action.rs:139-180`、`editor-core/src/effect.rs:75-107` | ✅ |
| host 视口渲染器（离屏 target/camera/pick/egui 注册） | `editor-viewport/src/{host,renderer,camera,pick,error}.rs` + `scene.wgsl`：`ViewportHost:37-194`、`create_target:204-218`、`create_color_target:404-448`（sRGB attachment + `Rgba8Unorm` egui view）、`recreate:357-379`、`Drop:390-394` | ✅ |
| app 帧阶段 + EffectExecutor + 组合根 | `editor-app/src/app/`：`mod.rs`（`EditorApp` + `impl eframe::App` + registry 装配）、`pipeline.rs`（`render_viewports`/`build_ui`/`pump`）、`effects.rs`（`apply_effect`）、`shell.rs`（顶栏/快捷键/modal）、`seed.rs` | ✅ |
| host 资源（`HostView` 实现）与 gallery 面板 | `editor-app/src/host.rs:18-73`、`editor-app/src/gallery.rs:21-146`（host crate 注册的 panel，core 不知道） | ✅ |
| 四槽布局替身 | `PanelHost::draw`（`panel/host.rs`）：left/right/bottom 用 `egui::Panel`、center 用 `CentralPanel`，tab 由 `ui-widgets` 的 `TabBar` 画；`draw_panel_body` 是唯一 `Panel::draw` 调用点 | ✅（生产换 `kairos_editor_docking`） |

### 7.2 测试与构建（实测）

`cargo test --offline`：**125 passed / 0 failed；+1 doctest passed（另有 1 个 doc 示例 ignored）；0 warning**。全部 headless（不建 wgpu device、不开窗口）。合计 126 tests = 125 headless + 1 doctest。

| crate / suite | 数量 | 覆盖 |
|---|---|---|
| `editor-core` lib + `core_flow.rs` | 1 + 19 = 20 | 选择/编辑/保存+completion/关闭 modal/undo/revert/command/异步/quit/layout/project mutation |
| `editor-core/tests/viewport_flow.rs` | 11 | Ensure 幂等、bind ticket、bind 中 resize 回放、camera 仅 bound 后转发、pick round-trip/去重/stale、bind 失败、关闭面板销毁 target + 重开新 ticket、PanelId 是数据 |
| `editor-components` lib（viewport_panel 单位/NDC） | 7 | `orbit_drag_right_turns_yaw_negative_and_drag_down_raises_pitch`、`pan_is_normalized_to_viewport_height`、`scroll_up_maps_to_positive_zoom_in`、NDC 四角/中心/越界/零尺寸 |
| `editor-components/tests/component_actions.rs` | 8 | 纯映射（树/工具栏/inspector） |
| `editor-components/tests/full_stack.rs` | 7 | 点击→Action→真实 reducer→Effect→completion |
| `editor-components/tests/panel_host.rs` | 14 | registry 生命周期、sync drop、focus 决定 active tab、tab→FocusPanel、close→ClosePanel、新 PanelId 零 core 改动、panel body Action、repaint 聚合、HostView 资源、内置面板 smoke |
| `editor-components/tests/viewport_panel.rs` | 3 | 视口面板 headless UI：click→Focus+Pick（NDC≈原点）、drag→Orbit（弧度/方向）、面板自身不改 selection |
| `ui-widgets`（atoms 8 + compounds 5 + tab_bar 6 + purity 1） | 20 | 控件一帧渲染、合成点击、TabBar 交互、纯度扫描（无领域/IO 类型） |
| `editor-viewport` | 35 | camera：pitch/distance clamp、orbit 累积、zoom 乘性、pan 目标平面、view/projection 不变量、非有限输入防御；pick：NDC→ray、ray-AABB、ray-plane、命中/遮挡/空点 |
| `editor-app` bin | 0 | 窗口 smoke 覆盖（见 7.3） |

关键测试名（设计契约的直接证据）：

- viewport slice：`ensure_creates_target_once_and_binding_is_ticket_validated`、`resize_while_binding_is_replayed_after_bind_completion`、`camera_intents_are_forwarded_only_after_binding`、`pick_round_trip_updates_scene_selection_and_ignores_stale_tickets`、`closing_the_viewport_panel_destroys_the_target_and_reopen_recreates`、`panel_identity_is_opaque_data_so_new_panels_need_no_core_edit`。
- panel host：`sync_creates_one_instance_per_open_registered_kind_in_registry_order`、`sync_drops_instances_of_closed_panels_and_reopen_gets_a_new_serial`、`brand_new_panel_kind_can_be_registered_and_shown_without_touching_the_store`、`active_panel_follows_layout_focus_and_only_it_is_drawn`、`tab_click_emits_focus_panel_action`、`close_click_emits_close_panel_action`、`frame_with_host_exposes_host_resources_and_frame_new_uses_empty_host`。
- 相机/拾取：`idle_input_never_changes_the_camera`、`zoom_is_multiplicative_and_moves_the_eye_closer`、`pan_moves_the_target_in_the_camera_plane`、`center_ray_hits_the_cube`、`sky_ray_misses_everything`。
- 视口面板 UI 契约（headless harness）：`viewport_panel_click_emits_focus_and_pick_ndc`、`viewport_panel_drag_emits_orbit_with_radians`、`viewport_panel_never_touches_scene_selection_itself`。

构建/静态检查（本机复跑）：`cargo build --offline` 0 warning；`cargo clippy --offline --all-targets -- -D warnings` 通过；`cargo doc --no-deps --offline` 通过、0 warning。

### 7.3 窗口 smoke（实跑）

启动日志（与原型 README §6 一致）：

```text
wgpu viewport host ready (engine stand-in).
Prototype started: registry panels + wgpu offscreen viewport + real reducers.
CreateViewport ViewportId(1) at 624x546 -> bound
```

截图（均已存在并人工核对内容）：

- `prototypes/editor-ui/docs/screenshot-main.png`（2558×1639）：Project / Scene 3D 视口（raymarch demo：棋盘地面 + cube）/ Inspector / Console + 中央 tab（Widget gallery | Scene）；Console 里可见上述 smoke 日志。
- `prototypes/editor-ui/docs/screenshot-gallery.png`（2558×1639）：host 注册的 Gallery 面板（13 atoms + 13 compounds 展示），Cmd+G 开关。
- 旧 `screenshot-interaction.png` 已删除；原型 README 只引用上述两张（`docs/` 目录核验只有两张图片）。

窗口内实际验证到的链路：启动与 5 个面板渲染、四槽 tab 切换、Gallery 面板 Cmd+G 开关、Scene 面板关闭/重开（`DestroyViewport` / 新 `CreateViewport`）、Project 树点击/搜索、Inspector 显示、顶栏 Save 等按钮链路（console 有对应 effect 日志）。

**没有声称**在窗口里自动验证了视口 orbit/pick：GUI 自动化脚本对 3D viewport 的点击注入不可靠（System Events 点击只稳定命中按钮/标签）。视口输入契约改由两层证据覆盖：① headless 面板测试（click→Focus+Pick NDC、drag→Orbit 弧度/方向、read-only selection）；② `editor-viewport` 在临时 crate 里用真实 wgpu device 做的 bind→input→render→pick→resize→destroy 全链路 smoke（本机 Apple M4 Pro，pixel 回读核对棋盘/cube/选中高亮）。两条合起来覆盖了「egui 输入 → Action → reducer → Effect → host camera/raycast → TextureId」的每条边。

### 7.4 原型 ↔ 生产差异（迁移时必须替换）

| # | 方面 | 原型（已验证） | 生产要求 | 证据/落点 |
|---|---|---|---|---|
| 1 | 宿主 | eframe（`logic` 渲视口、`ui` 画 UI），eframe 拥有 present | `kairos_editor_app` 接管 winit+wgpu+`RenderPipeline::present`；eframe 不进生产 | `app/mod.rs` + `app/pipeline.rs`；§4.3/§6.1 |
| 2 | 视口渲染 | 独立 raymarch pipeline + 自有 `queue.submit` | 引擎 `GraphicsCommand`/`GraphicsGraph`/`RenderPipeline::present` + `bind_attachment_to_egui` | `editor-viewport/src/renderer.rs:239-287`；§3.1 |
| 3 | 纹理生命周期 | 持久 target，resize 时 free+re-register（`TextureId` 会变） | 按 `ViewportTargetId` 持久缓存 + resize 复用 `TextureId` + 显式 free；egui pass 顺序保证 | `renderer.rs:204-448`；§3.5/§3.6 前提 2/3 |
| 4 | 相机 | `OrbitCamera` 纯数学；`CameraInput` camera-ready 单位（弧度/归一化/正=拉近） | `EditorCameraController` 在 World、无 pan、单位不同 → 补 pan + 单位换算入口 | `camera.rs:183-210`、`editor-core/src/viewport.rs:99-116`；§3.8.2/§3.9 |
| 5 | 拾取 | CPU raycast（demo 场景 AABB/plane） | engine scene query 或 GPU id-buffer+readback；`DrawCommand` 无 Entity | `editor-viewport/src/pick.rs`；§3.8.4 |
| 6 | 布局/多实例 | 四槽固定；每 kind 一实例；view state 在 panel struct | `kairos_editor_docking` DockState；`PanelInstanceId` 多实例；keyed `ViewStates` | `panel/host.rs`；§2.8 |
| 7 | effect 执行 | host 内同步完成（保存写临时文件、bind/pick 同帧回填） | 异步 ticket/EffectExecutor；bind 完成跨 1 帧（present 之后） | `app/effects.rs`；§3.9 |
| 8 | 键盘相机 | 未实现 WASD/fly（只有鼠标四件套） | 保留 fly 需加 `CameraInput.fly` + 对应 Action + `FocusScope` 仲裁 | §3.8.2/§3.8.3 |
| 9 | 布局持久化 | `Effect::PersistLayout` 只写日志 | serde + `version` + 未知 PanelId 丢弃 + 恢复 | §6.2 P3 |

### 7.5 仍未证明 / 下一步

- **1 帧延迟没有自动化帧计数测试**：原型的实现（`logic` 先渲染、`ui` 里应用 intent、持久 `TextureId`）与该语义一致，但没有测试直接断言「帧 N 输入 → 帧 N+1 可见」；建议在原型或生产加帧计数测试钉住 §3.6 前提 1/2。
- **生产侧前提 3（视口 pass 先于 egui pass）无法在原型验证**：需要引擎层测试（`GraphicsGraph` 执行顺序或拆 submit）。
- **GPU 无 headless 测试**：`cargo test` 不建 device；像素、device lost、真实 DPI 只有窗口 smoke 覆盖。
- **多实例/dock 拖拽/布局持久化/MCP/命令面板/IME/多窗口**：未实现（§5.4、原型 README §8/§9）。
- **真实引擎材质/资产**：原型场景是 raymarch demo，不加载真实资产；§3.5 的纹理成本量化还没在真引擎测过。

---

## 8. 参考

**本仓库**

- [ADR 0009：面板是注册表 + 3D 视口是 host 拥有的 render target](../../adr/0009-editor-panel-host-and-viewport.md)
- [ADR 0008：Editor UI architecture](../../adr/0008-editor-ui-architecture.md)
- [README：Kairos Editor UI 架构重设计](./README.md) §3.3-3.5（帧循环/状态归属/面板）、§4.3（场景视口 + 异步纹理回绑）
- [rewrite-plan.md](./rewrite-plan.md) Phase 1-2、§5 质量门、§6 回滚
- [widget-layer.md](./widget-layer.md) §2.3/§2.4（L2/L3）、§4（状态归属）、§5（测试）
- [editor-ui-engine-integration.md](../../research/editor-ui-engine-integration.md) §1（帧循环）、§2.7（渲染命令管线）、§5（线程约束）、§6（测试）、§8（MUST/MUST NOT）
- [editor-ui-architecture-patterns.md](../../research/editor-ui-architecture-patterns.md) §7（Docking/布局持久化）、§10 建议 7（布局树 + PanelRegistry）、§10 建议 9（immediate-mode 纪律）
- [docs/agents/testing.md](../../agents/testing.md)（`cargo test-crate` 约定）

**引擎代码（本文引用的关键路径）**

- 帧循环/UI 顺序：`kairos_editor/src/runtime.rs:237-394, 496-526`；`kairos_editor/src/lib.rs:19-113`
- 旧 UI：`kairos_editor/src/ui.rs:90-206, 326-336, 776-786, 855-869`；`ui/scene_window.rs:358-569`；`ui/game_window.rs:174-293`
- 相机：`kairos_editor/src/camera.rs:84-243`
- 视图资源：`kairos_graphics/src/view_port.rs:22-116`；`kairos_graphics/src/extract.rs:36-106`；`kairos_graphics/src/drawer.rs:31-70`
- 渲染命令/图：`kairos_graphics/src/graphics_graph/graphics_command.rs:22-241`；`graphics_graph/graph.rs:29-331`；`graphics_graph/graphics_node.rs:85-113`；`attachment.rs:4-83`
- present/纹理注册/回收：`kairos_graphics/src/render_pipeline.rs:345-581, 896-945`；`egui_texture_handle.rs:3-22`
- 引擎 update/调度：`kairos_engine/src/engine.rs:43-46`；`kairos_engine/src/schedule.rs:140-183`

**原型代码（§7 证据）**

- Panel 框架/面板：`prototypes/editor-ui/crates/editor-components/src/{panel,frame,viewport_panel,project_panel,inspector_panel,console_panel}.rs`
- viewport slice/reducers：`prototypes/editor-ui/crates/editor-core/src/{viewport.rs,reduce/viewport.rs,reduce/layout.rs}`
- host 视口：`prototypes/editor-ui/crates/editor-viewport/src/{host,renderer,camera,pick}.rs`
- app 帧阶段/组合根：`prototypes/editor-ui/crates/editor-app/src/{app,host,gallery}.rs`
- 测试与实测输出：`prototypes/editor-ui/crates/*/tests/*`、原型 README §6-§7

**外部库（本机 cargo registry，版本与 workspace 一致）**

- `egui-wgpu-0.35.0/src/renderer.rs`：`CallbackTrait`（:87-120）、`depth_stencil_format` 默认 `None`（:186-190）、`register_native_texture`（:771-790）、`update_egui_texture_from_wgpu_texture`（:792-812）、`free_texture`（:752-756）
- `egui-0.35.0/src/context.rs`：`run_ui` 多 pass（:773-860）；`memory/mod.rs:331`（`max_passes` 默认 2）
- 与调研文档一致的外部参考（Rerun `ViewportBlueprint`/`egui_tiles`、Godot `EditorDock`、Zed `DockStructure`）见 [editor-ui-architecture-patterns.md](../../research/editor-ui-architecture-patterns.md) §7/§11，此处不再重复引用。

