# kairos_editor UI 架构现状审计：Messager 与状态/MVC

- **任务**：task-1 — Audit current kairos_editor UI architecture (Messager + state/MVC)
- **日期**：2026-10-30
- **仓库 HEAD**：`d1312fa wip(kairos_ecs/kairos_transform): WorldId as SystemParam, lifetime rename, transform require`
- **审计方式**：只读静态审计。按要求先尝试 CodeGraph，但仓库根目录没有 `.codegraph/` 索引，`codegraph explore` 直接报错（CodeGraph isn't available here），因此退回 grep + read。**未运行 cargo build/test**；所有运行时行为判断均为代码路径推断，未验证项已标注。
- **路径约定**：除另行标注外，`文件:行` 均相对 `kairos_editor/src/`；`docs/...` 相对仓库根。
- **计数口径**（后文不再重复）：
  - **变体（variant）**：`Message` 枚举分支，57 个（ui.rs:90-206）。
  - **send 点**：`messager.send(...)` 调用行，共 73 处（22 个文件）。任务书给的 66 是「65 处 `send(Message::` 字面 + 1 处直接 `messages.push_back`」的口径；完整口径见 §2.3。
  - **直插（raw enqueue）**：绕过 `Messager::send`、直接 `self.messager.messages.push_back(...)`，1 处（ui.rs:470-472）。
  - **延迟构造**：`Some(Message::...)` 存进 `ConfirmDialogWindow` 的 `Cell`、稍后由对话框发出，5 处。
  - **arm**：`Context::handle` 中 `match msg` 的分支，57 个（ui.rs:419-772）。

---

## 0. 结论摘要（供 Lead 快速取用）

1. **Messager 本体只有 15 行**（`Messager` + `new` + `send`，ui.rs:289-303），但它事实上是编辑器的全局事件总线：57 个 `Message` 变体、73 个 send 点、1 个直插、5 个延迟构造、22 个文件直接 `send`、57 个 arm 集中在一个 365 行的方法里（ui.rs:410-774）。
2. **Message 没有错误、回复、取消、优先级、来源、序号、合并**：`Messager` 只有一个无界 `VecDeque`（ui.rs:290），`Message` 没有 `Debug`/`Clone`/`Serialize`（ui.rs:90 无 derive）；失败只能 `log::{warn,error}`、弹原生错误框或 `panic!`（ui.rs:865-869）。
3. **投递边界不是「帧」而是 egui pass**：每帧按 `render_ui → handle_ui → draw_ui` 顺序跑一遍（runtime.rs:264-270）；egui 0.35 默认 `max_passes=2`（egui-0.35.0/src/memory/mod.rs:331），`Grid` 首次布局会 `request_discard`（egui-0.35.0/src/grid.rs:454），而 Preferences 窗口里就有 `egui::Grid`（preferences_window.rs:229）。因此「draw 阶段发的消息下一帧处理」只在单 pass 时成立；多 pass 时同一帧会重跑整段闭包，`graphics_commands` 还会重复累积（runtime.rs:254、265）。
4. **状态分散在 6 类载体**：Drawer 内部 Model、engine `World` resource/entity、`DockState`、`Context` 的 `ids/drawers/actives`、egui temp memory、磁盘 toml/`asset_registry`。MCP 计划文档已记录同一结论：「状态分散……没有统一的『编辑器状态』入口」（docs/plan/kairos-editor-mcp/phase3-middleware-architecture.md:13）。
5. **`Context::handle` 是 god controller**：365 行、57 arm、33 次 `get_window_mut` downcast、10 次 `get_inspector_mut` 二次 downcast、23 处 `engine.world` 直接读写、23 次 drawer 生命周期调用（`push_drawer/show_tab/show_window/close_drawer`）、2 次 `dialogs.push`。它同时承担窗口生命周期、tab 生命周期、业务逻辑、引擎写、磁盘 IO、样式注册、对话框编排。
6. **「关闭窗口」是影子状态**：`actives: Vec<bool>`（ui.rs:331）只由 `close_drawer` 翻转（ui.rs:855-863）；drawer 对象永不从 `drawers` 移除（ui.rs:330、788-798）；`render()`（ui.rs:776-786）和样式枚举（ui.rs:477-487）都不检查 `actives`。所以关闭的窗口仍然每帧 `render`、仍然接收消息并执行副作用。
7. **视图就地改 model**：`Drawer::ui(&self, ...)` 签名（ui.rs:262-269）迫使所有「绘制期状态变更」走内部可变性：22 个 `*Model`/`*InspectorModel`（§4.2），`Cell<bool>`、`Cell<Option<..>>`、`Arc<parking_lot::Mutex<..>>` 遍布 inspector（如 material.rs:1043-1055）。消息里甚至直接携带 `Arc<Mutex<Option<SerializedMaterial>>>` 快照来规避对象生命周期（ui.rs:194-205、material.rs:1416-1434）。
8. **每帧消息放大已经发生**：Preferences 打开且选中任一页时，每帧无条件 `send(Message::UpdateUIStyle(page))`（preferences_window.rs:404）→ handle 里 `drawer.update_style`（ui.rs:493-499）；若选中的页是 ToolBar/SceneWindow/AboutWindow，`update_style` 会 `toml::to_string` + `std::fs::write`（tool_bar.rs:305-327、scene_window.rs:333-356、about_window.rs:141-163）——即「开着 Preferences 选 ToolBar 页」会以约 60 次/秒写盘。
9. **布局索引与 DockState 双源**：`EditorLayout` 的 4 个 `Zone{surface,node}` 只在 `InitLayout` 时算一次（layout.rs:235-278），随后 `DockState` 删除 leaf 会搬移节点甚至 pop 掉尾部节点（tree.rs:677-760）。关闭某 zone 里最后一个 tab 之后再从工具栏打开同类窗口，`show_tab` 会拿旧 `NodeIndex` 去 `append_drawer`（ui.rs:839），可能 panic（tree.rs:258-260 越界，或 tree/node.rs:255-258 的 node was not a leaf）。**该路径为静态推断，未运行验证。**
10. **没有 undo/redo、没有 UI 测试**：全仓无 undo/redo 实现；`kairos_editor` 共 74 个 `#[test]`，其中 **0 个**引用 `Context`/`Messager`/`Message`/`Drawer`（§4.4）。历史上曾有 `kairos_test_harness`（bridge/dispatch/input_injector/headless），已在 98f5bca 中整体删除（−2642 行）。
11. **已有 doc 证据**：MCP phase3 文档明确记录状态分散（:13、:20-42）、渲染循环同步痛点（:15、:189、:392）；hierarchy 技术方案明确计划「沿用现有 `Message` 枚举模式」，新增 `Message::SelectHierarchyEntity` 并「各面板 handle() 中响应」（docs/plan/hierarchy/hierarchy-phase3-tech.md:16、58-62）——即后续功能默认会继续往 57 分支的 god method 里加分支。

---

## 1. 帧与所有权地图

### 1.1 每帧调用链

```text
runtime.rs:242   self.editor.update()                       // engine.update()（lib.rs:51-56）
runtime.rs:249   let mut render_pipeline = self.render_pipeline.lock();   // 拿 wgpu 锁
runtime.rs:254   let mut graphics_commands = Vec::<GraphicsCommand>::new();
runtime.rs:264   self.egui_ctx.run_ui(raw_input, |ui| {
runtime.rs:265       graphics_commands.append(&mut self.editor.render_ui());  // ① render
runtime.rs:267       self.editor.handle_ui(ui);                              // ② handle
runtime.rs:269       self.editor.draw_ui(ui);                                // ③ draw
                 });
runtime.rs:272-300 egui platform output / viewport commands
runtime.rs:302-348 tessellate + 构建 egui 的 GraphicsCommand
runtime.rs:351   render_pipeline.present(&self.editor.engine.world, output, graphics_graph);
```

三层转发几乎没有逻辑：`Editor` 只持有 `engine / ui_context / log` 三个字段（lib.rs:19-23）；`Editor::render_ui`（lib.rs:69-71）→ `Context::render`（ui.rs:776-786）；`Editor::handle_ui`（lib.rs:58-60）→ `Context::handle`（ui.rs:410）；`Editor::draw_ui`（lib.rs:62-67）→ `Context::darw`（ui.rs:368，函数名拼写为 `darw`）。

### 1.2 draw 阶段的固定顺序

`Context::darw`（ui.rs:368-408）的绘制顺序是**硬编码**的：

1. **ToolBar**：按 `TypeId` 直接查表绘制（ui.rs:383-386），不走 DockState；toolbar 是「伪 Drawer」。
2. **中央 DockArea**：`egui::CentralPanel` → `DockArea::show_inside`（ui.rs:389-398）→ surfaces 遍历（docking_tab.rs:252-269）→ `show_leaf`（docking_tab.rs:1086-1155）→ `drawer_body`（docking_tab.rs:2285-2373）→ `TabDrawer::ui`（docking_tab.rs:2370）→ `KairosTabDrawer::ui`（ui.rs:220-231）→ `Drawer::ui`。
3. **Dialogs**：最后绘制，`Dialog::draw` 也可 `send` 消息（ui.rs:401-407、ui/dialog.rs:42-70）。

因此**同一帧内消息的产生顺序 = 这个光栅顺序**（toolbar → dock 内 surface/node/tab 顺序 → dialogs），而消费顺序只由 `VecDeque` FIFO 决定（ui.rs:418）。跨窗口冲突不会按「语义优先级」解决，只按「谁先画」解决。

### 1.3 所有权/生命周期

| 拥有者 | 拥有什么 | 证据 | 生命周期 |
|---|---|---|---|
| `EditorRuntime` | `Editor`、`egui_ctx`、`egui_state`、`Arc<Mutex<Option<RenderPipeline>>>` | runtime.rs:84-96 | 进程 |
| `Editor` | `Engine`、`ui::Context`、`Log` | lib.rs:19-23 | 进程 |
| `ui::Context` | `messager`、`layout: EditorLayout`、`ids: TypeIdMap<usize>`、`drawers: Vec<Box<dyn Drawer>>`、`actives: Vec<bool>`、`tab_tree: DockState<usize>`、`tab_viewer`、`dialogs`、`global_styles` | ui.rs:326-336 | 进程；drawer 只增不减 |
| `Engine` | ECS `World`（含 `SceneView`/`GameView`/`SceneViewInput` resource、相机 entity、asset stores）、输入、时间 | lib.rs:19-23；ui.rs:636-685 | 进程 |
| `DockState<usize>` | 布局树、tab 顺序、leaf 激活、浮动窗口 `WindowState` | docking_tab/dock_state.rs、surfaces.rs:37-41、window_state.rs:4-26 | 进程（**不持久化**，无 serde） |
| 各 Drawer | 自己的 `*Model`（22 个）、样式、编辑缓冲、动态 `Box<dyn Inspector>` | §4.2 | 进程；关闭 ≠ 销毁 |
| `Log` | 无界 `VecDeque<LogMessage>` | kairos_engine/src/log.rs:26 | 进程；只有 Console 绘制时才被消费（console_window.rs:80-82） |
| 磁盘 | 每个窗口一个 style toml（ui/paths.rs:1-46）、`Library/asset_registry.toml`、`.meta`/`.mat` 等 | project_window.rs:282-284 等 | 跨会话（但布局不持久化） |

### 1.4 依赖方向（现状）

```text
runtime ──> Editor ──> ui::Context ──> drawers（13 个 Drawer 实现）
                             │                │
                             │                ├─> engine::World（读 + 经消息写）
                             │                ├─> Messager（写消息）
                             │                └─> 磁盘（样式、资产、注册表）
                             └─> DockState<usize>（tab 内容是 drawer id）
Drawer::ui == &UIReader（可 downcast 任意其他 drawer）+ &mut Messager + &Engine + &mut Log
```

关键点：**ui 层直接依赖 engine 类型**（`Engine`、`World`、`Handle<..>`、`Mesh`、`RenderState`、`ViewportSize`、`GraphicsCommand` 都出现在 `ui.rs` 顶部 import 与 `Message` payload 里，ui.rs:1-64、90-206）；反过来 engine 不依赖 ui（`install()` 在 lib.rs:107-113 只装 asset/camera/syntax）。编辑器没有独立的 editor state 层。

---

## 2. Messager 机制审计

### 2.1 实现（15 行）

```rust
ui.rs:289-291  pub struct Messager { messages: VecDeque<Message> }
ui.rs:294-298  pub fn new()
ui.rs:300-302  pub fn send(&mut self, msg: Message) { self.messages.push_back(msg); }
ui.rs:418      while let Some(msg) = self.messager.messages.pop_front() { match msg { ...57 arms... } }
ui.rs:470-472  self.messager.messages.push_back(Message::RefershPreferenceWindow);  // 唯一绕过 send 的直插
```

特征：无界、单线程（main thread only）、无日志、无 `Debug`、无消息 ID/来源/时间戳/优先级/合并/取消；`Message` 不是 `Clone`，payload 里有 `tokio::sync::oneshot::Receiver`、`Arc<parking_lot::Mutex<..>>`、`Handle<T>`、`Mesh` 等（§2.8），所以既不能复制、也不能序列化、也不能跨线程。

### 2.2 57 个变体的关注点分组

| 组 | 关注点 | 变体数 | 代表变体（ui.rs 行号） | 直接 send 数 |
|---|---|---|---|---|
| A | 启动/退出/布局初始化 | 3 | `CreateToolbar`(91)、`InitLayout`(92)、`QuitEngine`(93) | 3 |
| B | About/Preferences/全局样式 | 7 | `OpenAboutWindow`(94)、`RefershPreferenceWindow`(98)、`UpdateUIStyle`(100) | 6 个 send + 1 个直插 |
| C | tab 打开/关闭（6 种窗口 × open/close） | 12 | `OpenConsoleTab`(101)…`CloseGameTab`(116) | 17 |
| D | 视口/相机/引擎视图状态 | 9 | `UpdateSceneWindowSize`(112)、`SceneViewOrbit`(123)、`RegisteGameWindowViewBind`(119) | 9 |
| E | 项目树/选择/重命名/拖拽 | 11 | `SelectProjectNode`(130)、`CreateProjectNode`(134)、`DragStartProjectNode`(146) | 21 |
| F | Inspector 保存/应用/音频预览 | 15 | `DocumentInspectorSave`(152)…`MaterialInspectorDiscard`(205) | 15 |
| — | dialog 通用发送（不属于单个变体） | — | ui/dialog.rs:49、58 | 2 |
| **合计** | | **57** | | **73** |

观察：
- **C 组 12 个变体是同一件事的手工复制**：每新增一个可停靠窗口就必须加 `OpenXxxTab`/`CloseXxxTab` 两个变体、两个 arm、toolbar 一个菜单项、窗口自己实现 `close()` 发 `CloseXxxTab`（§2.11）。
- **E 组 21 个 send 点**说明项目树是当前消息流量最大的子系统；`SelectProjectNode` 一个变体就有 5 个发送点（hierarchy_panel.rs:167、237、241，content_panel.rs:90、214）。
- 57 个变体全部都有 arm；唯一 0 直接发送者的是 `ExitRenameProjectNode`（ui.rs:140，只有 arm 没有 send 点；见附录 A）。
- 变体命名带子系统前缀（`SceneWindow*`、`GameWindow*`、`MaterialInspector*`…），即 enum 本身在承担「路由表」职责。

### 2.3 send 点分布（73 处 / 22 文件）

| 文件 | send 数 | 主要变体组 |
|---|---|---|
| ui/tool_bar.rs | 9 | A/B/C（Open*/Quit/OpenPref） |
| ui/project_window/context_menu.rs | 9 | E（Create×5、Open、Delete、StartRename） |
| ui.rs | 7 | A + C（Context::new 的启动消息） |
| ui/scene_window.rs | 7 | C/D（Orbit/Zoom/Fly/Size/Bind/TryRece）+ Close |
| ui/inspector/material.rs | 6 | F（Clear/Drop/ChangeShader/ChangeRS/Apply×2） |
| ui/project_window/hierarchy_panel.rs | 5 | E（Navigate×2、Select×3） |
| ui/project_window.rs | 4 | E（Drag×3）+ CloseProjectTab |
| ui/project_window/content_panel.rs | 4 | E（Select×2、Rename、Open） |
| ui/game_window.rs | 4 | C/D + Close |
| ui/inspector/audio.rs | 3 | F（Tick/Toggle/Seek） |
| ui/preferences_window.rs | 3 | B（Close/SetSelected/UpdateUIStyle） |
| ui/dialog.rs | 2 | 通用（转发 `Cell<Option<Message>>`） |
| 其余 11 个文件各 1 | 11 | 各窗口 Close、各 Inspector Save/Apply |
| **合计** | **73** | |

另有：
- 1 个直插：ui.rs:470-472（`OpenPreferenceWindow` 的 arm 里给自己追加 `RefershPreferenceWindow`，同一 while 循环内处理）。
- 5 个延迟构造：`Some(Message::...)` 存进 `ConfirmDialogWindow`（ui/dialog.rs:14-27 的 `Cell<Option<Message>>`），确认/取消按钮再 `send`（ui/dialog.rs:49、58）。位置：code.rs:202、shader.rs:205、toml.rs:163、texture/mod.rs:729、document.rs:153。
- 其中 **shader.rs:205 构造的是 `Message::CodeInspectorSave`**（而非 `ShaderInspectorSave`）——编译器发现不了这种跨窗口消息错配，是「enum 变体之间无类型归属」的直接证据。

### 2.4 集中式 dispatch：`Context::handle`（ui.rs:410-774）

| 度量 | 数值 | 证据 |
|---|---|---|
| 方法行数 | 365 | ui.rs:410-774 |
| `match msg` arm 数 | 57 | ui.rs:419-772 |
| 平均每 arm | 约 6.2 行 | 354/57 |
| 最大 arm | `InitLayout` 33 行 | ui.rs:426-458 |
| `get_window_mut::<T>()` downcast 次数 | 33 | grep `get_window_mut::<` @ ui.rs:410-774 |
| `get_inspector_mut::<T>()` 二次 downcast | 10 | 同上 |
| `engine.world` 直接读写 | 23 | 同上 |
| drawer 生命周期调用（push/show/close） | 23 | 同上 |
| `dialogs.push` | 2 | ui.rs:527-529、550-553 |
| `messager` 引用 | 2（pop_front + 直插 push_back） | ui.rs:418、470-472 |

arm 里被混在一起的关注点（举例）：
- **窗口生命周期**：`OpenAboutWindow`/`CloseAboutWindow`（ui.rs:462-467）、`OpenPreferenceWindow`（468-476）。
- **tab 生命周期 + 引擎副作用**：`CloseSceneTab` 除了 `close_drawer` 还把 `SceneView.size = 0`（ui.rs:631-637）；`CloseGameTab` 同理（655-660）；`OpenSceneTab` 顺带 `spawn_editor_camera(&mut engine.world)`（621-630）。
- **业务逻辑**：`CreateProjectNode`（562-566）→ `ProjectWindow::create_node` 里创建资产、写 `asset_registry.toml`（project_window.rs:250-286）。
- **磁盘 IO**：4 个 `*InspectorSave` arm 直接调 save 函数（ui.rs:686-697）；`TextureInspectorApply` 里 save `.meta`（698-705）。
- **样式注册/应用**：`RefershPreferenceWindow` 遍历**所有** drawers 调 `get_style_fileds`（ui.rs:477-487）；`UpdateUIStyle` 做 DTO 回写 + `drawer.update_style` + `scene_window.apply_camera_style(&mut engine.world)`（493-514）。
- **对话框编排**：`CloseInspectorTab` 先 `on_close` 拿对话框再关闭窗口（ui.rs:524-532）；`SelectProjectNode` 里 project→inspector 的跨窗口同步 + 对话框入列（545-556）。
- **面板间协调**：`AudioInspectorTogglePreview`/`Seek`/`Tick` 通过 `InspectorWindow → get_inspector_mut::<AudioInspector>` 两级 downcast 才拿到对象（ui.rs:607-620、706-712）。
- **帧级 IO**：`handle` 开头先 `project_window.poll_external_changes()`（ui.rs:414-416）——文件 watcher 的消费被绑定在「每帧一次 handle」上。

### 2.5 投递时序语义（关键）

单 pass 名义顺序（runtime.rs:264-270）：`render_ui()` → `handle_ui()` → `draw_ui()`。由此：

| 发送时机 | 被处理的时机 | 对 render 可见 | 对 draw 可见 |
|---|---|---|---|
| `Context::new`（首个 handle 之前，ui.rs:343-349） | 第 1 次 `handle_ui` | 第 1 帧 | 第 1 帧 |
| `Context::render` 内（ui.rs:778-783；scene_window.rs:366、475，game_window.rs:182、261） | **同一 pass** 的 `handle_ui`（render 在 handle 之前） | — | 同 pass 的 `draw_ui` |
| `handle` 内部再次 send（ui.rs:470-472） | **同一 while 循环**（FIFO 排到队尾） | 本帧 render 已过 | 同 pass 的 `draw_ui` |
| `draw_ui` 内（绝大多数 UI 交互） | **下一次** `handle_ui` | 再下一次 `render`（render 在 handle 前） | 下一 pass/frame 的 `draw_ui` |
| `dialogs` 绘制内（ui/dialog.rs:49、58） | 下一次 `handle_ui` | 同上 | 同上 |

推论：
- 一个「draw 中点击 → 发消息」的写操作，对 `render()` 而言有 **2 个 pass 的延迟**（N 帧 draw 发 → N+1 handle 写 → N+1 render 已错过 → N+2 render 才看到）。视口尺寸就是这条路：draw 里发现尺寸变了才发 `UpdateSceneWindowSize`（scene_window.rs:188-191），render 读 `SceneView.size`（scene_window.rs:372-376），所以尺寸变化到画面 attachment 重建之间存在固定延迟。
- **多 pass 例外（未在运行时验证）**：egui 0.35 `run_ui` 在 `request_discard` 时会重跑闭包（egui-0.35.0/src/context.rs:822-860），默认上限 2。Preferences 的 `egui::Grid` 首帧会 `request_discard`（preferences_window.rs:229 + egui-0.35.0/src/grid.rs:454）。此时：
  - pass 1 draw 中发的消息，会在 **pass 2 的 handle** 里被处理——「下一帧」语义被打破；
  - `runtime.rs:254` 的 `graphics_commands` 在闭包外声明、闭包内 `append`（runtime.rs:265），pass 2 会再执行一次 `render_ui`，把 scene/game/preview 的 `GraphicsCommand` **重复累积**；
  - `render()` 的副作用（发 `Registe*ViewBind`、创建 oneshot channel，scene_window.rs:473-477、game_window.rs:259-263）可能在一帧内执行两次，`handle` 会收到两条 `Registe*` 消息，后一条覆盖前一条（ui.rs:642-646），前一 channel 的 receiver 被丢弃。

### 2.6 重入、级联与隐式全局顺序

- **重入**：唯一在 handler 内再入队的是 ui.rs:470-472（`OpenPreferenceWindow` → 追加 `RefershPreferenceWindow`）。while 循环无迭代上限、无「处理中」标志；新增「刷新链」（如 rescan → rebuild → notify）很容易变成自激循环。
- **级联**：`RefershPreferenceWindow` 一次调用会对**全部** 9 个 drawer 调 `get_style_fileds()`（ui.rs:477-487），包括 3 个 fake layout container 和已关闭的窗口；`StylePage::new(id, drawer.get_name(), fields)` 把 `type_name` 全路径作为 UI 列表项显示（如 `kairos_editor::ui::layout::LayoutLeftContainer`）。
- **隐式全局顺序**：同一帧内多个窗口的消息按绘制顺序入队（§1.2）；没有 target 队列、没有消息时间戳。若两个窗口在同一帧对同一状态各发一条消息（如两个面板都发 `SelectProjectNode`），结果取决于 tab 顺序。
- **actor 语义缺失**：`handle` 用 `&mut self` 串行处理，天然单线程；但一旦要做后台 IO/加载完成回调（现在只有 oneshot 轮询，§3.9），没有「回复到消息」的通道。

### 2.7 没有错误、没有回复、没有取消

- `Message` 没有 reply 变体；handler 统一返回 `()`（ui.rs:419-772）；`Messager` 没有 `send_with_reply` 之类 API。
- 失败处理三种，都不回到 UI：
  1. 静默 log：删除失败 `log::warn!`（project_window.rs:419）；material 保存失败 `log::error!`（material.rs:1493-1500）。
  2. 原生系统弹窗：`dialog::error_message_window`（scene_window.rs:337、tool_bar.rs:316、about_window.rs:145、ui.rs:503；`crate::dialog` 在失败时还会 `panic!`，dialog.rs:21-26）。
  3. 直接 panic：`create_ui_failed` 弹框后 `panic!`（ui.rs:865-869）；docking 层大量 `assert!/expect`（如 docking_tab.rs:1098、1103）。
- **保存成功/失败对 UI 不可见**：CodeInspector 的 Save 按钮在 `send` 后立刻 `dirty=false`（code.rs:178-190），Shader 相同（shader.rs:181-192）；Material 的 Apply 消息被处理时先 `inspector.apply()` 清 dirty（ui.rs:752-758），再 `save_material`，失败只 log（material.rs:1478-1506）。即「写入失败但 UI 显示已保存」。
- **对话框的两个取消语义不统一**：`MaterialInspectorDiscard` 是独立变体（ui.rs:205、765-771），而 Code/Shader/Toml/Document 在对话框里只有 save 消息、取消为 `None`（code.rs:201-210 等）；「关闭时保存」与「显式保存」复用同一条变体，无法区分来源。

### 2.8 payload 类型：UI/引擎/异步原语混装

57 个变体中 **28 个带 payload**（29 个 unit），payload 类型分布：

| payload 类型 | 变体数 | 例子 |
|---|---|---|
| `petgraph::graph::NodeIndex` | 7 | `SelectProjectNode(Option<NodeIndex>)`(130)、`CreateProjectNode(NodeIndex, String, AssetKind)`(134) |
| `engine::asset::Handle<T>` | 7 | `DocumentInspectorSave(PathBuf, Handle<Text>, Arc<Mutex<Option<String>>>)`(152)、`MaterialInspectorDiscard(Handle<SerializedMaterial>, Handle<Material>)`(205) |
| `Arc<parking_lot::Mutex<Option<..>>>` | 6 | 4 个 save + `TextureInspectorApply`(178) + `MaterialInspectorApply`(198) |
| `tokio::sync::oneshot::Receiver<EguiTextureHandle>` | 2 | `RegisteSceneWindowViewBind`(113)、`RegisteGameWindowViewBind`(119) |
| `Mesh` | 1 | `ModelInspectorCreateWireframeMesh(182)` |
| `RenderState` | 1 | `MaterialInspectorChangeRenderState(193)` |
| `StylePage`（UI DTO） | 1 | `UpdateUIStyle(100)` |
| `PathBuf` | 2 | `MaterialInspectorChangeShader`(185)、`MaterialInspectorDropTexture`(188) |
| `(u32,u32)` / `f32` 组 | 5 | 尺寸、orbit/zoom/fly |
| `usize`、`String`、`AssetKind` | 3 | `SetPreferenceWindowSelectedId`(99)、`CreateProjectNode`(134) |

含义：
- 消息层**同时**是：UI 事件板、命令 API、渲染线程间 handoff（oneshot）、文件保存参数包、样式 DTO 传输。任何「给 MCP/自动化/脚本复用」的尝试都绕不开这些引擎类型（phase3 文档的 Query/Command 方案正是为了绕开它，docs/plan/kairos-editor-mcp/phase3-middleware-architecture.md:98-159）。
- `StylePage` 等 UI 专属类型和 `Handle<Material>` 等引擎类型出现在同一个 enum，说明没有稳定的边界。

### 2.9 分发机制：TypeIdMap + downcast + service locator

- `Context.ids: TypeIdMap<usize>`（ui.rs:329）是**单例注册表**：`push_drawer::<T>` 用 `TypeId::of::<T>()` 记账（ui.rs:788-798），每个类型最多一个实例；`show_tab`/`show_window` 用 `ids.get(&TypeId)` 判断「是否创建过」（ui.rs:807-808、835-836）。
- 三个取用入口：`Context::get_window_mut::<T>`（ui.rs:885-897，handle 里 33 次）；`Context::_get_window::<T>`（ui.rs:871-883，已无调用者）；`UIReader::get_drawer::<T>`（ui.rs:310-324，全仓**仅 1 处**使用：material.rs:468-470）。
- **二次 downcast**：`InspectorWindow::get_inspector{,_mut}::<T>`（inspector_window.rs:116-128）在 `Box<dyn Inspector>` 上再 `Any` downcast；handle 里 10 次。
- `Drawer: Any`（ui.rs:255）与 `Inspector: Any`（inspector.rs:24）都用 trait object + downcast 做动态派发，编译期无法检查「谁处理哪条消息」。
- `UIReader` 名义上是跨窗口只读访问，实际形态是**service locator**：3 个字段（`global_style`、`ids`、`drawers`，ui.rs:305-309）+ 1 个方法；docking 层还绕过方法直接访问 `reader.drawers`（docking_tab.rs:292、312、329、850、1121、2334、2361），窗口直接读 `reader.global_style`（project_window.rs:550、575）。
- 唯一一处真正的跨窗口业务读是拖拽状态：material inspector 从 `UIReader` 掏出整个 `ProjectWindow` 再读 `get_dragging()`（material.rs:468-470、project_window.rs:371-373）。这次访问当年需要一次 28 文件、+310/−318 的提交（`d69be0e`，§4.3）。

### 2.10 消息空转与丢失

- **空转（关闭后仍处理）**：`get_window_mut` 不检查 `actives`（ui.rs:885-897），且 drawer 永不删除（§3.10）。所以关闭 Inspector tab 后，`SelectProjectNode` 仍会走到 inspector：会 `on_exit` 旧 inspector、重建新 inspector（inspector_window.rs:99-114），后者可能同步读文件/资源（project_window.rs:211-239 → `InspectorCreater`）。同理关闭 Inspector 后 `MaterialInspectorApply` 仍会保存（ui.rs:752-764）。
- **丢失（对象尚未创建）**：`get_window_mut` 返回 `None` 时 arm 是静默 no-op（如 ui.rs:546-555 的 `if let Some(...)` 链）。drawer 只在收到 `Open*` 时才 `push_drawer`；因此任何「先发命令、后开窗口」或测试路径都会无声丢消息。
- **无回压**：`Messager` 无界；唯一限流是 draw 每帧发一次（`UpdateUIStyle`、`AudioInspectorTick`、`TryReceTextureId`）。若未来在 handler 里做长任务，队列只会在边界 pass 继续增长。
- **每帧消息（已存在）**：`UpdateUIStyle`（Preferences 选中页时每帧发送，preferences_window.rs:404，handler 含 3 处可能写盘）；`AudioInspectorTick`（音频预览播放时每帧发送，audio.rs:303-306，handler 做播放位置计算 audio.rs:363-380）；`SceneWindowTryReceTextureId` / `GameWindowTryReceTextureId`（render 里只要 receiver 还在就每帧发送，scene_window.rs:365-367、game_window.rs:181-183）。
- **消息顺序与画面顺序耦合**：`DragConsumeProjectNode` 由 `ProjectWindow::ui` 在「检测到自己处于 `Drag::Stoped`」时发送（project_window.rs:530-532）——即拖拽状态的清理依赖「下一帧 ProjectWindow 被画出来」。窗口关闭时该状态不会被清理。

### 2.11 具体功能变更成本（现状下「加一个功能」要动哪里）

| 场景 | 需要改的点 | 文件数 | 备注/证据 |
|---|---|---|---|
| 项目树新增一条命令（如 Duplicate） | `Message` 新变体（ui.rs:130-150 区，1-3 行）+ handle 新 arm（5-15 行）+ `ProjectWindow` 业务方法（20-40 行，asset_registry + project_path_graph + save，参考 project_window.rs:250-286）+ 右键菜单项（context_menu.rs，3-8 行）+ 若联动 Inspector 再加 ui.rs 同步 | **4-5** | 2 个 god 文件必须动；无单测可覆盖（§4.4） |
| 新增 Inspector 动作（如 mesh 导出） | `Message` 变体 + arm + save/apply 函数 + 按钮 + 关闭时对话框构造（`Some(Message::...)`） | **3-4** | 参考历史提交 5879b94（material Discard）：5 文件 +253/−5 |
| 新增一个可停靠窗口 | `Drawer` impl 10 个方法（样板：console_window.rs 110 行中约 90 行为样板）+ 2 个变体 + 2 个 arm + toolbar 菜单项 + `close()` 发 Close 消息 + style struct/toml + `paths` 常量 | **6-8** | 见 §4.2 |
| 打开第二个 Scene/Inspector 实例（多视口） | 改 `TypeIdMap<usize>`/`drawers`/`actives`/`DockState<usize>` 的单例假设、`Message` payload 需带实例 id、33 次 `get_window_mut` 调用点、12 个 open/close arm、`UIReader`、`KairosTabDrawer::title/ui` | **结构性重构** | 当前不可表达 |
| undo/redo | 需要为 57 条消息里的变更类操作建立逆操作/命令日志；状态分散在 22 个 Model + `World`；无现有 hook | **结构性重构** | 全仓无 undo/redo 实现 |
| MCP/headless 外部命令 | `Message` 不可序列化、payload 含 `Handle/oneshot/Arc<Mutex>`；需要另建协议（phase3 文档已规划） | **新协议层** | docs/plan/kairos-editor-mcp/phase3-middleware-architecture.md:98-159、:189、:392 |
| 样式字段增删 | `get_style_fileds` 的 `Vec` 顺序 + `update_style` 的**按索引读取** `style_fields[0..n]`（tool_bar.rs:289-303、scene_window.rs:291-331、about_window.rs:133-139、preferences_window.rs:154-167）；`copy_from_slice`（preferences_window.rs:190）会 panic | **2-3** | 改字段顺序 = 运行时 panic 或静默错配，编译器不检查 |
| Hierarchy 的 entity 选择（已规划） | 按 docs/plan/hierarchy/hierarchy-phase3-tech.md:16、58-62 的计划：新增 `Message::SelectHierarchyEntity` + 「各面板 handle() 中响应」——即再加变体 + arm | **2+** | 文档明确「沿用现有 Message 枚举模式」 |

### 2.12 已有文档证据

- **「状态分散」**：MCP phase3 文档把「编辑器状态散落在 KairosEngine → ui_context → 各 Drawer 内部，没有统一入口」列为第一个问题（docs/plan/kairos-editor-mcp/phase3-middleware-architecture.md:13），并画出状态分布树（:20-42），其中 `ui_context` 下列出了 `ProjectWindow.model`、`InspectorWindow.model`、各窗口、`DockState`。
- **渲染循环同步**：同一文档要求 Command 在本帧 draw 前处理、Query 在 draw 后执行（:189、:392），即承认「消息/状态/帧序」已经是需要显式设计的问题；并记录「Drawer 访问不到（私有字段）」的风险（:404）。
- **扩展路径**：hierarchy 方案明确沿用 `Message` 枚举（docs/plan/hierarchy/hierarchy-phase3-tech.md:16），新增变体并「各面板 handle() 中响应」（:58-62）——与现状「只有 1 个 handle」的事实不符（只有 `Context::handle` 一个 57-arm 方法，不存在「各面板 handle」），说明文档假设与代码结构已出现偏差。

---

## 3. 状态 / MVC 审计

### 3.1 现状不是 MVC，而是「god controller + 单例窗口」

| MVC 角色 | 现状落点 | 问题 |
|---|---|---|
| Model | 22 个 `*Model`/`*InspectorModel`（§4.2）+ engine `World` | 同一业务对象的状态被拆到两处（如 ProjectWindow 的 `asset_registry/project_path_graph` vs engine 的 `Assets<>`；SceneWindow 的 style vs engine 相机组件） |
| View | `Drawer::ui(&self, ...)`（ui.rs:262-269） | `&self` 却必须改状态 → 内部可变性遍地（§3.3）；view 还直接 `reader` 读别的窗口 |
| Controller | `Context::handle`（365 行 / 57 arm） | 同时是窗口管理器、命令分发器、业务逻辑、引擎写入口、磁盘 IO、样式注册中心、对话框编排器 |
| 「选中」这一最接近共享 model 的概念 | `ProjectWindow.selected_node`（project_window.rs:54）+ `InspectorWindow.selected: Option<InspectorNodeInfo>`（inspector_window.rs:47-53） | 手工同步一次（ui.rs:545-556），之后无失效/通知机制（§3.8） |

### 3.2 状态到底存在哪（清单）

| 状态 | 载体 | 证据 | 谁能写 |
|---|---|---|---|
| 布局树/tab 顺序/激活 leaf | `Context.tab_tree: DockState<usize>` | ui.rs:332、390；docking_tab/dock_state.rs | DockArea（draw 期）+ handle 的 open/close |
| 浮动窗口位置/大小/minimized | `WindowState`（每个 `Surface::Window` 一份） | docking_tab/window_state.rs:4-26；surfaces.rs:37-41 | docking 层 + `Drawer::show_window`（仅初始一次） |
| drag&drop 临时态 | `State` 存进 egui temp memory | docking_tab/state.rs:8-16、20-36；docking_tab.rs:220-233 | docking 层 |
| 「窗口是否打开」 | `Context.actives: Vec<bool>` | ui.rs:331、796、810-813、838-840、860-861 | 仅 `show_*`/`close_drawer` |
| drawer 实例 | `Context.drawers: Vec<Box<dyn Drawer>>` + `ids` | ui.rs:329-330、788-798 | 只增不减 |
| 布局分区索引 | `EditorLayout{left,center,right,bottom: Zone}` | layout.rs:193-233 | 仅 `InitLayout`（ui.rs:426-458） |
| 项目树/选中/重命名/拖拽/watcher | `ProjectWindowModel` 10 个字段 | project_window.rs:49-71 | window 方法（handle 转调）+ `Cell` 在 draw 期写（:63、561） |
| Inspector 选中快照 + 动态对象 | `InspectorWindowModel.selected: Option<InspectorNodeInfo>` | inspector_window.rs:47-53、99-114 | handle 的 `SelectProjectNode` 等 |
| 各 Inspector 编辑缓冲/dirty/预览 | `*InspectorModel`（22 个 Model 之一）+ `Cell`/`Arc<Mutex>` | material.rs:286-300、1043-1055；texture/mod.rs:74-90；document.rs:33-44 | draw 期直接写（§3.3） |
| 视口尺寸 | `SceneView.size` / `GameView.size`（engine resource） | ui.rs:636-663；scene_window.rs:188-191 | handle |
| 相机输入/轨道 | `SceneViewInput`（engine resource） | ui.rs:674-685；camera.rs | handle |
| 相机实体/intrinsics | engine `World` entity 组件 `Camera`/`EditorCameraController`/`CameraView` | scene_window.rs:494-543 | handle（经 SceneWindow 方法） |
| 资产数据 | engine `Assets<Text/Toml/SerializedMaterial/...>` + `AssetServer` | material.rs:1478-1506 等 | handle（save/apply/discard） |
| 全局字体/图标 | `Context.global_styles: GlobalStyles` | ui.rs:335；global_styles.rs:7-12 | 仅启动加载 |
| 每个窗口样式 | 各自 `*Style` + `Preferences/Styles/*.toml` | ui/paths.rs:1-46；各 window `*Style::new` | `update_style` 直接写文件（3 处） |
| 日志 | `Log.logs: VecDeque`（无界） | kairos_engine/src/log.rs:26 | 各窗口 `&mut Log`；仅 Console 绘制时消费（console_window.rs:80-82） |
| 关于窗口「每帧 log」 | draw 期副作用 | about_window.rs:96（每帧 `log.info`） | draw |

### 3.3 视图就地改 model（`&self` + 内部可变性）

`Drawer::ui` 与 `Inspector::draw` 都拿 `&self`（ui.rs:262-269、inspector.rs:33-40），但绘制期产生的状态必须落盘，于是：

- `ProjectWindow::ui` 里 `self.model.force_expand_to.set(None)`（project_window.rs:561；字段是 `Cell<Option<NodeIndex>>`，:63）；
- `MaterialInspector::draw` 里 `self.model.thumbnail.lock() = None`（material.rs:484）、`self.model.dirty.get()/set(...)`；
- `DocumentInspector::draw` 通过 `content: Arc<Mutex<Option<String>>>` 直接编辑缓冲（document.rs:118-146）；
- `AudioInspector::draw` 直接读 `self.model.is_playing` 并发 tick 消息（audio.rs:303-306）；
- 拖拽视觉/状态：`ProjectWindow::draw_drag` 同时发消息（project_window.rs:445-505）。

数量级：22 个 Model 结构体（§4.2），其中至少 6 个含 `Cell`/`RefCell`/`Arc<Mutex>`；`MaterialInspectorModel` 单结构体就有 3 个 `Arc<Mutex<..>>` + 3 个 `Cell`（material.rs:1043-1055）。

**结果**：同一帧里「谁改了模型」无法从函数签名看出来；view 函数可以产生 IO、发命令、改状态、注册纹理通道。审查/测试没有可插的缝。

### 3.4 跨窗口读：UIReader 的实际用法

- 定义 3 字段 + 1 方法（ui.rs:305-324），构造于 `Context::darw`（ui.rs:376-380）。
- 使用统计：`get_drawer::<T>()` **1 处**（material.rs:468-470，读 `ProjectWindow` 的拖拽状态）；`reader.global_style` 2 处（project_window.rs:550、575）；`reader.drawers` 直接被 docking 层使用 7 处（docking_tab.rs:292、312、329、850、1121、2334、2361）。
- 结论：UIReader 不是「状态访问 API」，而是「把整个 drawer Vec + TypeIdMap 暴露给绘制层」的逃生舱。跨窗口数据流的真实数量很小（1 条业务读），但**结构上不设限**：任何 drawer 都可以 downcast 任何其他 drawer 并读其全部可见方法（如 `get_dragging()` 返回 `&Option<Drag<PathBuf>>`，project_window.rs:371-373）。

### 3.5 `Context::handle` 之外的「控制器」：每个窗口的 `pub fn xxx(&mut self, ...)`

窗口只把「查/改自己 model 的方法」暴露成 `pub fn`，由 handle 调；这些方法里又混入引擎写与 IO：
- `ProjectWindow::create_node/rename/delete` 内部直接 `asset_registry.save()`（project_window.rs:282-284、306、417）；
- `SceneWindow::spawn_editor_camera/apply_camera_style` 直接写 `World`（scene_window.rs:494-543）；
- `MaterialInspector::change_shader/drop_texture/apply/discard` 直接 `Assets` 读写（material.rs:1341-1414、1470-1506）；
- `AudioInspector::toggle_playback/seek_and_play/tick_playback` 直接操作 kira/`World`（audio.rs:322-380）。

也就是说「Controller」其实分散在 `handle` + 9 个窗口的 `pub fn` 两层，**没有统一的事务边界**：一次用户动作可能跨「消息 → handle arm → 窗口方法 → engine world → 磁盘」。失败/回滚/撤销都没有挂载点。

### 3.6 `Drawer::create(&World)` 类型擦除与两条创建路径

- `Drawer::create(world: &World) -> Result<Self, ...> where Self: Sized`（ui.rs:256-259）：trait object 上**不可调用**（`where Self: Sized`），所以 `show_tab` 里写死 `T::create(&engine.world)`（ui.rs:846）。
- 两条不一致的创建路径：`show_tab<T>` 用 `T::create(&engine.world)`（ui.rs:846）；`show_window<T>` 接收 `impl FnOnce() -> Result<T, _>`（ui.rs:800-829），例如 `AboutWindow::new`（ui.rs:463）。
- 没有窗口工厂/注册表；`Open*` 消息 = 手工 `show_tab::<具体类型>`（ui.rs:515-544、621-673）。
- 创建失败 → `create_ui_failed` 弹原生错误框并 `ui.send_viewport_cmd(Close)` 然后 `panic!`（ui.rs:865-869）——UI 创建失败 = 进程终止。
- `Drawer` trait 有 10 个方法（`create`、`show_window`、`ui`、`render`、`close`、`scroll_bars`、`get_name`、`get_title`、`get_style_fileds`、`update_style`，ui.rs:255-287），13 个实现（9 个真实窗口 + 4 个 layout 容器）。其中大量方法是空实现（§4.2）。

### 3.7 「布局容器」是假 Drawer + `InitLayout` hack + `actives` 影子状态

`Message::InitLayout` 的 arm（ui.rs:426-458）在做：

1. `push_drawer::<LayoutLeftContainer>`、`LayoutRightContainer`、`LayoutBottomContainer` 三个**空 Drawer**（ui.rs:427-432；实现见 layout.rs:23-146，全部方法 no-op）；
2. 用它们的 id 作为 `Node::leaf_with(vec![id])` 的占位，调用 `EditorLayout::init_layout` 做 3 次 split（ui.rs:434-439；layout.rs:235-278）；
3. 从各 zone leaf 里 `retain_drawers` 掉这些占位 id（「保留空 leaf 结构」，ui.rs:441-452）；
4. 然后 `close_drawer` 标记它们不活跃（ui.rs:454-457）。

后果：
- `Context.drawers` 永远多 3 个无意义条目（启动后共 9 个 drawer，其中 3 个是 layout 占位；注意 `Message::OpenConsoleTab` 启动时**没有**发送，ui.rs:343-349）；
- Preferences 的样式页列表会把它们当窗口列出来（ui.rs:477-487，`get_name` 返回 `type_name::<LayoutLeftContainer>()`，layout.rs:53-56）；
- `LayoutCenterContainer`（layout.rs:21、148-187）**定义了但从未实例化/注册**（grep 仅 layout.rs 自身命中），是 40 行死代码；
- `actives` 是 DockState 之外的第二个「可见性」真相：DockState 负责「tab 在不在树里」，`actives` 负责「要不要 append」。二者由 draw 期的 X 按钮（docking_tab.rs:282-305、1494-1517、1574-1582）与下一 pass/frame 的消息分别更新，中间窗口有一个 pass 的不一致；
- `close_drawer::<T>` 只置 `actives[id]=false`（ui.rs:855-863），**不**从 DockState 删除——删除由 DockArea 的 `to_remove` 完成（docking_tab.rs:282-340）；即「关闭」需要两套机制同时生效。

### 3.8 一致性漏洞（可举证的四个）

1. **删除选中资产后 Inspector 继续显示**：`DeleteProjectNode` arm 只调 `project_window.delete_node`（ui.rs:587-591），后者只清自己的 `selected_node/active_directory`（project_window.rs:403-421），没有通知 `InspectorWindow`；`InspectorNodeInfo` 是快照（name/kind/path/guid/inspector，inspector_window.rs:24-45），不会失效。
2. **外部 rescan 后 Inspector 过期**：`poll_external_changes → refresh_from_disk` 会重建项目图并按 GUID 找回选中（project_window.rs:159-200），但 Inspector 的 `path/inspector` 不随之更新；若文件被外部删除，树更新而 Inspector 仍指向旧句柄。
3. **拖拽状态的消费者只有 Material Inspector**：拖拽 state 存在 `ProjectWindowModel.drag`（project_window.rs:64、371-373、423-443），消费方是 material.rs:468-470；`DragConsumeProjectNode` 的清理依赖 ProjectWindow 下一帧被绘制（project_window.rs:530-532）——窗口未绘制/隐藏时状态悬空。
4. **布局 Zone 索引 vs DockState 搬移**（§0-9）：`EditorLayout` 只在 `InitLayout` 计算（layout.rs:235-278），而 `Tree::remove_drawer` 在 leaf 为空时会 `remove_leaf` 并搬移/pop 节点（tree.rs:837-846、677-760）。`show_tab` 的 `self.tab_tree[zone.surface][zone.node].append_drawer(*id)`（ui.rs:839）于是可能：越界 panic（tree.rs:258-260）、或落到 `Node::Empty`/split 上 panic（tree/node.rs:255-258）。**静态推断，未运行验证；建议 Lead 在真机复现：关闭 Hierarchy tab（左 zone 唯一 tab）→ 工具栏再开 Hierarchy。**

### 3.9 `render()` 的副作用与排序

- `Context::render`（ui.rs:776-786）在**所有** drawer 上调用 `Drawer::render`，不检查 `actives`、不检查 dock 可见性；顺序 = `drawers` 插入顺序（toolbar、layout×3、scene、game、project、inspector、hierarchy、console…），与 UI 布局无关。
- `render` 有副作用：发消息（scene_window.rs:366、475；game_window.rs:182、261）；轮询 oneshot（scene_window.rs:553-569、game_window.rs:277-293）；创建 oneshot channel 并 `bind_attachment_to_egui`（scene_window.rs:473-477、game_window.rs:258-263、material.rs:1225-1232、1285-1295）。
- **关闭的 Inspector 仍在渲染 3D 预览**：`InspectorWindow::render` 直接 `self.model.selected.as_ref().and_then(|info| info.inspector.render())`（inspector_window.rs:204-213），`MaterialInspector::render` 只要 `model.preview` 存在就构建完整 `GraphicsCommand` 并返回 `Some`（material.rs:1219-1299）；关闭 tab 不清 `selected`（§3.10），因此每帧继续建 attachment/通道。这是「render 不看可见性」最直接的浪费。
- render 位于 handle 之前（runtime.rs:265 vs 267）：render 看到的是本帧 handler 执行前的 world；handler 的写入最早下一帧 render 才生效；多 pass 时会重复 render（§2.5）。
- `graphics_commands` 在 `run_ui` 闭包外声明、闭包内 append（runtime.rs:254、265），多 pass 时重复；`present` 在闭包外执行一次（runtime.rs:351），重复的 commands 全部进入 `GraphicsGraph::build`。

### 3.10 窗口生命周期：永不销毁、关闭靠消息

- drawer 创建后永远留在 `Context.drawers`/`ids`/`actives`（`push_drawer` ui.rs:788-798 无删除 API）；`close_drawer` 只翻 bool（:855-863）。
- 关闭 tab 的完整路径（X 按钮）：`docking_tab.rs:1494-1517`（右键菜单 Close）或 `:282-305`（`TabRemoval`）→ `KairosTabDrawer::on_close`（ui.rs:233-242）→ `drawer.close(messager)` **发 Close 消息** → Dock 立即 `remove_drawer`。下一 pass/frame handle 才 `close_drawer`（actives=false）并执行引擎副作用（如 SceneView.size=0，ui.rs:631-637）。
- **未保存确认对话框依赖「对象残留」**：`Message::CloseInspectorTab` arm 先 `inspector.on_close(ui.ctx())` 拿对话框，再关闭（ui.rs:524-532）；对话框的 Apply 消息在之后被处理时，仍能从 `drawers` 里找到已「关闭」的 Inspector 并保存（ui.rs:752-764）。如果未来实现「关闭即销毁」，这条路径会立刻断掉。
- 窗口没有 close request 的否决语义：`close_drawer` 无条件执行；`DialogState::Closed` 只是从 `Context.dialogs` 里移除（ui.rs:401-407）。

### 3.11 持久化边界

- **布局不持久化**：`DockState`/`Surface`/`WindowState` 无 serde（docking_tab/dock_state.rs、window_state.rs 无 `Serialize`），关闭编辑器后 tab 布局全部丢失；`EditorLayout` 也不落盘。
- **样式持久化散在 `update_style` 里直接写文件**：scene_window.rs:333-356、tool_bar.rs:305-327、about_window.rs:141-163 三处 `toml::to_string` + `std::fs::write`；而 Preferences 自身 `update_style` 只改内存不写盘（preferences_window.rs:154-167）。
- **两套注册表**：磁盘 `Library/asset_registry.toml`（project_window.rs:104-113、282-284）+ 引擎内 `Assets<>` 缓存（material.rs:1478-1506）/ `AssetServer`；`ProjectWindowModel.project_path_graph` 是第三份文件树视图。
- **没有会话/编辑器状态序列化入口**：即便想把「打开的项目、选中项、窗口布局」存盘，也没有一个对象能代表它们。

---

## 4. 度量

### 4.1 规模

| 项 | 数值 | 证据 |
|---|---|---|
| `kairos_editor/src` 总行数 | 22,841（75 个 rs 文件） | find + wc |
| 其中测试文件 | 1,888 | `*/test.rs` |
| 非测试代码 | 20,953 | 计算值 |
| `ui/` 子树 | 17,625 | 含 docking_tab 全量 |
| `ui/docking_tab*` 子树 | 7,435（占 editor 32.6%） | docking_tab.rs 2,542 + tree.rs 1,050 + styles.rs 755 + dock_state.rs 695 + drag_and_drop.rs 477 + … |
| `ui.rs` | 947 | — |
| └ `Message` enum | 117 行 / 57 变体 | ui.rs:90-206 |
| └ `Context::handle` | 365 行（= ui.rs 的 38.5%） | ui.rs:410-774 |
| └ `darw` | 41 行 | ui.rs:368-408 |
| └ `render` | 11 行 | ui.rs:776-786 |
| └ `Drawer` trait | 33 行 / 10 方法 | ui.rs:255-287 |
| 最大单文件 | docking_tab.rs 2,542 | — |
| 次大 | inspector/material.rs 1,506 | — |

### 4.2 抽象/实现统计

| 项 | 数值 | 证据 |
|---|---|---|
| `impl Drawer for` | 13（9 真实窗口 + 4 layout 容器） | grep `impl Drawer for` |
| 真实窗口 Drawer | 9：ToolBar、About、Preferences、Console、Inspector、Hierarchy、Project、Scene、Game | 同上 |
| `impl Inspector for` | 11 | ui/inspector/*.rs |
| `*Model` 结构体 | 22 | §3.2 |
| `*Style` 结构体 | 36 | grep |
| style 加载样板 `pub fn new() -> Result<Self, _>` | 32 | grep（各窗口/Inspector 各一份） |
| per-window style toml 常量 | 11 个窗口 + 9 个 Inspector | ui/paths.rs:1-46 |
| `show_window` 实现 | 13；其中 11 个空实现，仅 About/Preferences 真正使用 | about_window.rs:77-86、preferences_window.rs:90-97 |
| `render` 实现 | 13；10 个直接 `None`，Scene/Game 真正建 command，InspectorWindow 转发 | ui.rs:776-786 |
| `get_style_fileds` 实现 | 13；4 个非空（ToolBar/Scene/About/Preferences），9 个 `Vec::new()` | 各文件 |
| `update_style` 实现 | 13；3 个直接写 toml 文件 | §3.11 |
| layout 容器的纯样板 | layout.rs:23-187 共 165 行，40 个方法实现几乎全 no-op | layout.rs |
| `_get_window` 死代码 | ui.rs:871-883（0 调用） | grep |

### 4.3 变更成本实证（git 历史）

| 提交 | 内容 | 规模 | 说明 |
|---|---|---|---|
| `d69be0e` | 重构 drag 数据传输：创建 UIReader，通过 reader 跨 ui 安全访问数据（Ref issue #32） | **28 文件，+310/−318** | 为让 MaterialInspector 读 ProjectWindow 的拖拽状态，需要引入 UIReader 并改 docking_tab.rs 109 行、material.rs 116 行、ui.rs 58 行、project_window.rs 97 行 |
| `5879b94` | material inspector Discard 后恢复运行时 Material | **5 文件，+253/−5**（ui.rs +20、material.rs +50、integration 测试 +174 等） | 一个「关闭时丢弃修改」的功能，需要新增 Message 变体 + arm + 对话框 + 测试 |
| `0e7be34` | audio inspector 重构（waveform/spectrum/seek） | **31 文件，+1650/−940**（ui.rs +35、audio.rs +422、inspector_window.rs +30、creater.rs +10…） | 大子系统重构；其中 UI 消息层只占约 35 行，说明「消息层本身不重，重的是它与 UI/引擎的耦合面」 |
| `98f5bca` | 删除 `kairos_test_harness`（bridge/dispatch/headless/input_injector/ws_server…） | **25 文件，−2,642** | 曾经的 UI 自动化/headless 尝试被整体移除；当前无替代品 |

### 4.4 测试覆盖

- `kairos_editor` 共 **74** 个 `#[test]`：project_path_tree 32、audio 23、material 4、ide_detection 3、camera 3、project_watcher 2、texture 2、dock tree 1、syntax 1、editor_assets 2、lib 1、其它。
- **0 个**测试引用 `Context`/`Messager`/`Message`/`Drawer`（grep 结果为空）。
- 没有任何测试覆盖：57 个 dispatch arm、消息时序、窗口开关、`actives`/DockState 一致性、布局 Zone 索引、样式回写、对话框流程。
- 纯数据层有测试（dock tree 的 `test_tabs_iter`，docking_tab/dock_state/tree/test.rs:1-22；material save 失败路径，material/test.rs:99-140；audio waveform 数学等）——这证明「可测的部分被抽出来了，UI 编排层完全没测」。
- 历史 `kairos_test_harness`（headless/bridge/dispatch/input_injector）已被删除（§4.3），当前无 UI 自动化入口；这也是「6-8 文件才能加一个窗口」却没人敢改的原因之一。

---

## 5. 若继续加功能，预测的失败模式（具体场景）

1. **Undo/Redo 无法落地**：没有命令日志/逆操作，变更散在 57 个 arm + 9 个窗口 `pub fn` + engine `World` 原地写（§3.5）。要做 undo 只能给每条变更消息补逆操作（视口尺寸、相机、选择、资产重命名、材质、样式、布局——横跨 6 类载体），或整体改为命令对象；两种都要重写 `handle`。
2. **多视口/多实例撞上单例假设**：`TypeIdMap` + `drawers[id]` + `DockState<usize>` + `actives` 全是「每类型一个」（ui.rs:329-331、788-798）；`Message` 里没有实例 id。第二个 Scene 预览、prefab 编辑、双 Inspector 都要求结构性改动（33 个 downcast 点、12 个 open/close arm、`KairosTabDrawer`）。
3. **MCP/自动化/脚本无协议可复用**：`Message` 不可序列化（无 derive，ui.rs:90），payload 含 `oneshot::Receiver`/`Arc<Mutex>`/`Handle`（§2.8）；外部命令必须另起一套（phase3 文档已开始设计，:98-159）。
4. **跨窗口一致性继续腐化**：选中态现在靠一次手工同步（ui.rs:545-556），删除/重命名/外部 rescan 都不失效 Inspector（§3.8-1/2）；新增 Hierarchy entity 选择（计划中）会变成第三个「选中」真相源；每加一个数据源就要在 `handle` 里再加一次手工同步。
5. **布局索引失效导致 panic**（§3.8-4）：`EditorLayout` 只算一次 + DockState 删 leaf 会搬移/pop 节点；`show_tab` 直接索引旧 `NodeIndex`。触发条件：关闭某 zone 最后一个 tab 再从工具栏打开同类窗口；这是普通用户操作，不是边界 case。
6. **每帧消息放大继续扩散**：`UpdateUIStyle`（preferences_window.rs:404）已是 60 msg/s 且造成每帧 toml 写盘；`AudioInspectorTick`（audio.rs:305）、`TryReceTextureId`（scene_window.rs:366、game_window.rs:182）同类。任何「实时预览/拖拽/播放/自动保存」需求都会走同一条「draw 里每帧 send」的路，而 `Messager` 没有合并/节流。
7. **render pass 重复 / 关闭窗口仍渲染**：多 pass 时 `graphics_commands` 重复累积（runtime.rs:254/265），`render()` 里的 oneshot 注册会重复；关闭的 Inspector 仍每帧构建 3D 预览 `GraphicsCommand`（inspector_window.rs:204-213 + material.rs:1219-1299）。加更多带预览的 Inspector（mesh/texture/shader/animation）会等比放大。
8. **新窗口/新消息的固定税**：`Drawer` 10 方法 + 2 个 Message 变体 + 2 个 arm + toolbar 菜单 + close 消息 + style struct/toml + paths +（若带 render）oneshot 协议 + Preferences 样式页。参考 console_window.rs（110 行中约 90 行样板）；每加一个就多一行 57→58 的 god method。
9. **样式反射的索引 panic**：`update_style` 按 `style_fields[0..n]` 取值（tool_bar.rs:289-303 等）、`copy_from_slice`（preferences_window.rs:190）。增加/重排一个字段 → 运行时 panic 或静默错配；编译器不检查。且已存在「shader 的未保存对话框发 `CodeInspectorSave`」这类跨变体错配的先例（shader.rs:205）。
10. **日志/内存无界**：`Log` 无界（log.rs:26），只有 Console 绘制时才被消费（console_window.rs:80-82，`while let Some(log) = log.pop_front() { ui.label(...) }`）；AboutWindow 每帧写日志（about_window.rs:96）。打开 About 一段时间再打开 Console，会在单帧里渲染积压的全部 label（每帧 60 条 × 停留秒数），存在卡死/内存风险。
11. **测试/回归护城河缺失**：0 个 UI 测试（§4.4），而 `handle` 每加一个 arm 都可能触碰与引擎/布局/对话框的隐式约定；修改后只能整 crate 手工点。随着 arm 数增加，回归风险线性上升。
12. **会话恢复不可用**：布局、打开的项目、选中项、窗口状态均无统一序列化入口（§3.11），「恢复上次工作区」这类 UX 需求无法实现。

---

## 6. Open questions（留给 Lead 做设计决策；本文不提供方案）

1. **单实例窗口约束是否是产品长期约束？** 多 Scene 视口、双 Inspector、prefab 编辑是否在 1.0 范围内？这决定是否需要实例化 id/工厂，还是可以继续 `TypeIdMap` 单例。
2. **「编辑器状态」的边界**：选中项、当前项目、编辑模式（编辑/运行/暂停）、undo 栈，应该归 engine `World`（resource）、独立 `EditorState`，还是继续分散在 Drawer？MCP phase3 的 QueryHandler 方案与本次重写是否合并为同一套抽象？
3. **Message 的语义需求**：命令是否需要 ①失败/回执 ②可取消 ③外部（MCP/脚本/测试）可构造 ④可序列化录制；还是只服务 UI 内部？这决定 enum + queue 是否够用。
4. **投递边界**：是否接受「UI 输入在 pass 内、命令在 pass 之间」的现有语义？多 pass（egui 0.35 `max_passes=2`、Grid/Resize 触发 discard）是否要显式处理（例如把 render 副作用移出 `run_ui` 闭包）？
5. **渲染与 UI 的边界**：`Drawer::render`（返回 `GraphicsCommand` + 发消息 + 轮询 oneshot）是否应继续挂在窗口 trait 上，还是收敛为独立的 render-graph 构建阶段？关闭窗口是否应停止其 render？
6. **docking 层的归属**：7,435 行 vendored `docking_tab`（fork）是否保留、升级上游 `egui_dock`、或换成 `egui_tiles`？`DockState<usize>` 把 drawer id 当 tab payload，是否还要继续耦合 `Context` 内部索引？
7. **样式系统去留**：`get_style_fileds/update_style` 的反射式 DTO + 每窗口 toml 直写是否仍是产品需求？还是改为集中样式资源（Theme/Style）并有统一持久化？这直接影响 Drawer trait 方法数（10 个里有 2 个是样式反射）。
8. **布局/会话持久化**：是否需要保存/恢复 dock 布局与窗口状态（`DockState`/`WindowState` 当前无 serde）？若需要，布局的真相源与 `EditorLayout` Zone 索引如何避免 §3.8-4 的失效？
9. **测试策略**：是否接受「headless 驱动 UI（往队列注入命令、断言状态）」作为重写目标？当前 `Message` 不可构造/不可 Debug/`Context::handle` 需要 `&egui::Ui`（ui.rs:410），这是可测试性的硬约束；重写是否以「可 headless 测试」为验收标准？
10. **迁移策略**：是保留 `ui.rs` 的 `Message` 兼容层（旧窗口逐个迁移）还是一次性替换？历史提交（§4.3）显示跨窗口改动一次就 28 文件，渐进迁移的成本需要提前评估。

---

## 附录 A：`Message` 57 变体全表

（enum 行 / arm 行 均指 ui.rs 行号；send 为该变体的直接 `messager.send` 调用数；触发方为发送文件）

| 组 | 变体 | enum 行 | arm 行 | send | 触发方 | 备注 |
|---|---|---|---|---|---|---|
| A | CreateToolbar | 91 | 420 | 1 | ui.rs | 启动 |
| A | InitLayout | 92 | 426 | 1 | ui.rs | 33 行最大 arm，含 fake drawer hack |
| A | QuitEngine | 93 | 459 | 1 | ui/tool_bar.rs | handle 里用 `ui.send_viewport_cmd` |
| B | OpenAboutWindow | 94 | 462 | 1 | ui/tool_bar.rs | show_window 路径 |
| B | CloseAboutWindow | 95 | 465 | 1 | ui/about_window.rs | |
| B | OpenPreferenceWindow | 96 | 468 | 1 | ui/tool_bar.rs | arm 内直插 Refersh |
| B | ClosePreferenceWindow | 97 | 474 | 1 | ui/preferences_window.rs | |
| B | RefershPreferenceWindow | 98 | 477 | 0（+1 直插） | ui.rs:470-472 | 遍历全部 drawer 取样式 |
| B | SetPreferenceWindowSelectedId | 99 | 488 | 1 | ui/preferences_window.rs | |
| B | UpdateUIStyle | 100 | 493 | 1 | ui/preferences_window.rs:404（每帧） | 样式 DTO 回写 + 3 处写盘 |
| C | OpenConsoleTab | 101 | 515 | 1 | ui/tool_bar.rs | |
| C | CloseConsoleTab | 102 | 518 | 1 | ui/console_window.rs | |
| C | OpenInspectorTab | 103 | 521 | 2 | ui/tool_bar.rs, ui.rs | 启动时也发 |
| C | CloseInspectorTab | 104 | 524 | 1 | ui/inspector_window.rs | 先 on_close 再 close |
| C | OpenHierarchyTab | 105 | 533 | 2 | ui/tool_bar.rs, ui.rs | |
| C | CloseHierarchyTab | 106 | 536 | 1 | ui/hierarchy_window.rs | 关闭后 zone 索引可能失效 |
| C | OpenProjectTab | 107 | 539 | 2 | ui/tool_bar.rs, ui.rs | |
| C | CloseProjectTab | 108 | 542 | 1 | ui/project_window.rs | |
| C | OpenSceneTab | 109 | 621 | 2 | ui/tool_bar.rs, ui.rs | arm 里 spawn 相机 |
| C | CloseSceneTab | 110 | 631 | 1 | ui/scene_window.rs | arm 里置 size=0 |
| C | OpenGameTab | 115 | 652 | 2 | ui/tool_bar.rs, ui.rs | |
| C | CloseGameTab | 116 | 655 | 1 | ui/game_window.rs | arm 里置 size=0 |
| D | UpdateSceneWindowSize | 112 | 638 | 1 | ui/scene_window.rs | engine resource 写 |
| D | RegisteSceneWindowViewBind | 113 | 642 | 1 | ui/scene_window.rs:475 | render 内发 |
| D | SceneWindowTryReceTextureId | 114 | 647 | 1 | ui/scene_window.rs:366 | render 内发 |
| D | UpdateGameWindowSize | 118 | 661 | 1 | ui/game_window.rs | |
| D | RegisteGameWindowViewBind | 119 | 664 | 1 | ui/game_window.rs:261 | render 内发 |
| D | GameWindowTryReceTextureId | 120 | 669 | 1 | ui/game_window.rs:182 | render 内发 |
| D | SceneViewOrbit | 123 | 674 | 1 | ui/scene_window.rs:156 | 写 SceneViewInput |
| D | SceneViewZoom | 125 | 678 | 1 | ui/scene_window.rs:160 | |
| D | SceneViewFly | 127 | 682 | 1 | ui/scene_window.rs:182 | |
| E | SelectProjectNode | 130 | 545 | 5 | hierarchy_panel.rs, content_panel.rs | arm 里 project→inspector 手工同步 |
| E | NavigateToProjectDirectory | 132 | 557 | 2 | hierarchy_panel.rs | |
| E | CreateProjectNode | 134 | 562 | 5 | context_menu.rs（5 个菜单项） | 业务 + 写盘 |
| E | RenameProjectNode | 136 | 567 | 1 | content_panel.rs | |
| E | StartRenameProjectNode | 138 | 572 | 1 | context_menu.rs | |
| E | ExitRenameProjectNode | 140 | 577 | 0 | — | 无发送者（死变体候选） |
| E | OpenProjectNode | 142 | 582 | 3 | context_menu.rs, content_panel.rs | 按类型分发（含 IDE 启动） |
| E | DeleteProjectNode | 144 | 587 | 1 | context_menu.rs | 不通知 Inspector |
| E | DragStartProjectNode | 146 | 592 | 1 | project_window.rs:455 | |
| E | DragStopProjectNode | 148 | 597 | 1 | project_window.rs:458 | |
| E | DragConsumeProjectNode | 150 | 602 | 1 | project_window.rs:531 | 清理依赖窗口被绘制 |
| F | DocumentInspectorSave | 152 | 686 | 1（+对话框） | document.rs | payload: Path/Handle/Arc<Mutex> |
| F | CodeInspectorSave | 157 | 692 | 1（+对话框） | code.rs | shader.rs:205 错发此变体 |
| F | TomlInspectorSave | 162 | 689 | 1（+对话框） | toml.rs | |
| F | ShaderInspectorSave | 167 | 695 | 1 | shader.rs | |
| F | AudioInspectorTick | 172 | 706 | 1 | audio.rs:305（每帧 while playing） | 播放位置推进 |
| F | AudioInspectorTogglePreview | 174 | 607 | 1 | audio.rs:491 | 两级 downcast |
| F | AudioInspectorSeekPreview | 176 | 614 | 1 | audio.rs:603 | 两级 downcast |
| F | TextureInspectorApply | 178 | 698 | 1（+对话框） | texture/mod.rs | arm 里先 apply 再 save .meta |
| F | ModelInspectorCreateWireframeMesh | 182 | 713 | 1 | mesh.rs | payload 含 `Mesh` |
| F | MaterialInspectorChangeShader | 185 | 720 | 1 | material.rs | 引擎 Assets 写 |
| F | MaterialInspectorDropTexture | 188 | 728 | 1 | material.rs | 引擎 Assets 写 |
| F | MaterialInspectorClearTexture | 190 | 736 | 1 | material.rs | |
| F | MaterialInspectorChangeRenderState | 193 | 744 | 1 | material.rs | |
| F | MaterialInspectorApply | 198 | 752 | 2 | material.rs:1074、1204 | 保存失败只 log |
| F | MaterialInspectorDiscard | 205 | 765 | 1 | material.rs | |

---

## 附录 B：一句话证据索引（给 Lead 写设计文档时引用）

- 队列与分发：ui.rs:289-303、418、470-472、788-798、800-863、885-897
- 57 变体：ui.rs:90-206；57 arm：ui.rs:419-772
- 时序：runtime.rs:242、249、254、264-270、351；lib.rs:58-71
- 多 pass：egui-0.35.0/src/context.rs:822-860、memory/mod.rs:331；preferences_window.rs:229
- 状态分散：docs/plan/kairos-editor-mcp/phase3-middleware-architecture.md:13、20-42、189、392、404
- 扩展路径（继续加 Message）：docs/plan/hierarchy/hierarchy-phase3-tech.md:16、58-62
- 布局双源：layout.rs:235-278；tree.rs:677-760、837-846；ui.rs:839；tree/node.rs:255-258
- render 副作用：ui.rs:776-786；inspector_window.rs:204-213；material.rs:1219-1299；scene_window.rs:365-367、473-477
- 每帧消息：preferences_window.rs:404；audio.rs:303-306；scene_window.rs:188-191
- 样式回写：ui.rs:477-514；tool_bar.rs:289-333；scene_window.rs:291-356；about_window.rs:133-164；preferences_window.rs:154-190
- 日志：kairos_engine/src/log.rs:26；console_window.rs:80-82；about_window.rs:96
- 历史成本：d69be0e（28 文件）、5879b94（5 文件）、0e7be34（31 文件）、98f5bca（−2642 行）
