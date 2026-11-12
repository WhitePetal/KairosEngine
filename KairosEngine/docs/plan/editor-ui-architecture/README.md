# Kairos Editor UI 架构重设计

> **状态**：提案（未实施）
> **日期**：2026-10-30
> **触发**：现有 `Messager` 消息回调机制与"每窗口一个 Model+View"的 MVC 结构被认为过时、难扩展；作者明确接受完全重写。
> **证据**：[editor-ui-messager-mvc-audit.md](../../research/editor-ui-messager-mvc-audit.md) · [rust-ui-architecture-survey.md](../../research/rust-ui-architecture-survey.md) · [editor-ui-architecture-patterns.md](../../research/editor-ui-architecture-patterns.md) · [editor-ui-engine-integration.md](../../research/editor-ui-engine-integration.md)
> **配套**：[rewrite-plan.md](./rewrite-plan.md) · [ADR 0008](../../adr/0008-editor-ui-architecture.md)

---

## 0. 结论摘要（TL;DR）

**诊断**：现在的结构不是"教科书 MVC"，而是三个结构性问题叠加：

1. **全局消息总线**：57 个 `Message` 变体 + 一个 365 行的 `Context::handle` 巨型 match，所有窗口、所有领域、所有副作用共用一个无类型化路由、无返回值、无错误通道的队列。
2. **God Controller**：`Context::handle`（`ui.rs:410-774`）同时做窗口生命周期、业务逻辑、`World` 变更和 IO；33 处 `get_window_mut::<T>()` 类型擦除下转型。
3. **视图与模型同构**：每个窗口 struct 自己持有 Model 并直接画它；为了从 `&self` 的 `ui()` 里改状态，代码被迫使用 `Cell`、`Arc<Mutex<...>>`、把 `oneshot::Receiver` 塞进消息，以及"对话框持有 `Cell<Option<FnOnce>>` 回调"。

**方案**：**保留 egui 作为视图技术，重写为单向数据流（MVU/Store + Effect）架构**：

```
View（egui，只读状态、只发 Action）
  → Action（纯数据、按领域切片）
  → reduce()（纯函数，更新状态、返回 Effect）
  → Effect（纯数据，由 host 执行 IO/World 变更）
  → 完成事件回到 Action → 下一轮 reduce
```

配套三件事：**Command/Query API**（工具栏/快捷键/命令面板/MCP 共用一套入口）、**Document + Selection 服务**（单一事实来源，替代窗口内各自持有的状态）、**Effect Runner**（异步、引擎变更、磁盘 IO 全部显式化）。

视图层内部再分三层（本轮新增，详见 [widget-layer.md](./widget-layer.md)）：**Theme tokens → Widgets（纯外观，独立 crate，编译期不认领域类型）→ Components（读状态、组合 widget、发 Action）**。这样「一个 .rs 同时带外观和逻辑」被拆成「widget 只画、component 只接线、core 只改状态」，且边界由 cargo 强制。

**收益**：删掉 `Messager`/`Message`/`Drawer`/`UIReader`；核心逻辑可无 GPU/无窗口单测；新增一个面板/资产编辑器是「加一个领域模块 + 一个绘制函数」，不再改中心文件；`docs/plan/kairos-editor-mcp` 的绕行方案（给 `Drawer` 加 `QueryHandler`）不再需要。 本轮已交付**架构原型** [`prototypes/editor-ui/`](../../../prototypes/editor-ui/)：**126 个测试**（125 headless + 1 doctest，5 个 crate），把「core reducer → feature component → 纯 widget → 合成点击 → Action → 真实 reducer → Effect → completion」跑成一条链路；**Panel trait + Registry + Host** 已在窗口里替代写死在 `EditorApp` 的面板字段（加面板 = 注册 factory），且 **`cd prototypes/editor-ui && cargo run` 打开真实窗口 demo**：Project/Inspector/Console/**真实 wgpu 3D Scene 视口**/Widget gallery 五个面板、四槽 tab 停靠，视口走 offscreen RTT→egui TextureId→orbit/pan/zoom/pick 完整交互链路。不进入主 workspace 构建。面板/视口细节见 [panel-viewport-design.md](./panel-viewport-design.md) 与 [ADR 0009](../../adr/0009-editor-panel-host-and-viewport.md)。

---

## 1. 现状：它到底哪里坏了

### 1.1 现状结构

```
EditorRuntime::redraw()                       runtime.rs:237
└─ egui_ctx.run_ui(|ui| {
     render_ui()   → Context::render()        ui.rs:776   // 遍历所有 drawer（含已关闭的）
     handle_ui()   → Context::handle()        ui.rs:410   // 365 行巨型 match，drain Messager
     draw_ui()     → Context::darw()          ui.rs:368   // 遍历抽屉树，各窗口 ui()
   })

Context (ui.rs:326)
├─ messager: Messager { VecDeque<Message> }   // 全局单例总线
├─ drawers: Vec<Box<dyn Drawer>>              // 类型擦除的窗口注册表
├─ ids: TypeIdMap<usize>                      // TypeId → 下标（service locator）
├─ actives: Vec<bool>                         // 窗口可见性（隐藏状态）
├─ tab_tree: DockState<usize>                 // 布局树
└─ layout: EditorLayout                       // 四个"假抽屉"占位
```

数据流：视图在 `ui()` 里 `messager.send(Message::Xxx)` → 当前帧稍后（或下一帧）`Context::handle` 里 match 到某个变体 → `get_window_mut::<具体窗口>()` 下转型 → 调用窗口方法 → 方法里改 Model / 改 `engine.world` / 写磁盘。

### 1.2 `Messager` 的九个具体问题

1. **巨型枚举无领域边界**。57 个变体混了六类完全不同的东西：窗口/标签生命周期（`Open*Tab`、`Close*Tab`）、用户意图（`RenameProjectNode`）、引擎变更（`UpdateSceneWindowSize`、`SceneViewOrbit`）、异步握手（`RegisteSceneWindowViewBind(oneshot::Receiver<...>)`）、磁盘 IO（`DocumentInspectorSave(PathBuf, Handle<Text>, Arc<Mutex<...>>)`）、样式（`UpdateUIStyle`）。这六类的生命周期、失败语义、可测试性完全不同，却共享一个 `VecDeque`。
2. **新增一个功能要改 4~6 处**。例：项目树"删除节点"链路 = `Message` 变体（ui.rs:144）→ 右键菜单发送（project_window/context_menu.rs）→ `handle` match 臂（ui.rs:587-591）→ `ProjectWindow::delete_node`（project_window.rs:403）→ 领域逻辑与磁盘落盘（project_tree/asset_registry）。57 个臂里有 33 处是 `get_window_mut::<T>()`，意味着控制器要知道每个具体窗口类型。
3. **无类型化路由 / service locator**。分发靠 `TypeId` + `Any` 下转型（`get_window_mut`，ui.rs:885-897）；目标窗口不存在时（标签被关掉）只是 `if let Some` 静默丢弃，`Message::SelectProjectNode` 在 Inspector 未打开时不会有任何提示，也不会重放。
4. **时序语义隐式且随调用点变化**。在 `ui()` 里发的消息，要等下一帧 `handle` 才处理（帧内状态滞后）；在 `handle` 里发的消息（如 `RefershPreferenceWindow`，ui.rs:470-473）被同一个 `while let Some` 立即取出（重入）；在 `render()` 里发的消息（`SceneWindowTryReceTextureId`，scene_window.rs:366）在当前帧的 `handle` 处理。三种时序，没有任何机制约束，全靠读代码推断。 更糟的是，egui 0.35 默认 `max_passes = 2`，`Preferences` 里的 `egui::Grid` 首帧会 `request_discard`（preferences_window.rs:229），闭包重跑会让上述时序全部失效：draw 里发的消息可能在同帧第二 pass 被处理，`graphics_commands` 重复累积，`render()` 里创建的 oneshot channel 被注册两次。
5. **没有返回值、没有错误通道**。保存失败只能 `log::error!`；调用方不知道结果。确认对话框只能"再发一条消息"表达取消/确认。
6. **借用检查的绕行工具，而非架构**。`Drawer::ui(&self)` 是不可变借用，视图想改状态只能走 `&mut Messager` 这个唯一可变通道；于是出现：`ProjectWindowModel::force_expand_to: Cell<Option<NodeIndex>>`、`renaming_buffer: Arc<Mutex<String>>`（project_window.rs:49-71）、`MaterialInspectorModel` 的 `Arc<Mutex<Option<SerializedMaterial>>>` / `Cell<bool>` / `Mutex<Option<PreviewState>>`（material.rs:286-311）、`Dialog` 的 `Cell<Option<Message>> + Cell<Option<FnOnce>>`（dialog.rs:14-27）。**消息总线本质上是为可变视图硬造的一层间接层**。
7. **把异步管线也塞进总线**。纹理回绑通道是消息负载（`RegisteSceneWindowViewBind(tokio::sync::oneshot::Receiver<EguiTextureHandle>)`），再用每帧消息轮询（`SceneWindowTryReceTextureId`）；`AudioInspectorTick` 则是拿消息当每帧定时器（audio.rs:305）。消息队列同时承担事件总线、定时器、异步握手线三种职责。
8. **不可测试**。想测一个交互，必须构造整个 `Context` + `Engine`（还要 GPU 窗口）；74 个迁移来的测试全部是数据结构测试（project tree 32、audio inspector 23），没有一个交互/状态机测试。
9. **关闭/隐藏语义混乱**。`close_drawer` 只把 `actives[id] = false`（ui.rs:855-863），drawer 仍留在 `drawers` 里；`render()` 会遍历所有 drawer 包括已关闭的（ui.rs:778）；场景/游戏窗口只能靠"`size=0` 让 play layer 跳过"来掩盖（ui.rs:631-637）；`RefershPreferenceWindow` 会把已关闭窗口的样式页也注册进去。

10. **每帧消息放大已造成真实写盘**：Preferences 打开且选中某页时，每帧无条件 `send(Message::UpdateUIStyle(page))`（preferences_window.rs:404）→ `handle` 调 `drawer.update_style`（ui.rs:493-499）；若选中页是 ToolBar/SceneWindow/AboutWindow，`update_style` 会 `toml::to_string` + `std::fs::write`（tool_bar.rs:305-327 等）——即"开着 Preferences 选 ToolBar 页"会以约 60 次/秒写盘（审计静态推断，未运行验证）。
11. **样式字段靠下标约定，改顺序即 panic/静默错配**：`get_style_fileds` 返回 `Vec`、`update_style` 按 `style_fields[0..n]` 索引读（tool_bar.rs:289-303、scene_window.rs:291-331），Preferences 里还有 `copy_from_slice`（preferences_window.rs:190）。增删/重排字段编译器不检查，运行时才炸——这是"每窗口自带样式反射"的必然代价。
12. **布局索引双源，重开标签可能 panic（未运行验证）**：`EditorLayout` 的 4 个 `Zone{surface,node}` 只在 `InitLayout` 时算一次（layout.rs:235-278），而 `DockState` 删除 leaf 会搬移/弹出节点（tree.rs:677-760）；关闭某 zone 最后一个 tab 后再从工具栏打开同类窗口，`show_tab` 用旧 `NodeIndex` `append_drawer`（ui.rs:839），可能越界 panic 或 "node was not a leaf"。同一个"布局状态"存了两份，必然漂移。

### 1.3 "MVC" 的八个具体问题

严格说现状是 **"窗口 = Model + View，Controller 集中在一个 god match 里"**，比经典 MVC 更糟：

1. **没有应用级 Model**。编辑器状态散在 13 个 `Box<dyn Drawer>`、`DockState`、`Engine.world` 资源、`Log`、`GlobalStyles` 五处。`docs/plan/kairos-editor-mcp/phase3-middleware-architecture.md` 已经把"状态分散"列为 MCP 的第一障碍——这是同一根因的另一个症状。
2. **Controller 是上帝对象**。`Context::handle` 365 行里，既有创建窗口、切换标签，也有 `engine.world.resource_mut::<SceneView>().size = ...`（ui.rs:638-641）、spawn 相机实体（ui.rs:621-630）、写资产文件（`DocumentInspector::save_content` 等）。业务逻辑、渲染器内部细节、UI 生命周期全耦合在一个函数里。
3. **视图直接改模型**。视图不是"渲染函数"，而是"持有可变状态的窗口对象"：`ui(&self)` 里通过 `Cell`/`Mutex` 改字段；`AudioInspector` 在 `draw` 里直接 `log.pop_front()`（console_window.rs:74-80 也在 draw 里弹日志）。
4. **跨窗口隐式耦合**。`UIReader` 让一个窗口读另一个窗口：材质 Inspector 通过 `reader.get_drawer::<ProjectWindow>()`（material.rs:469）读项目树；`TabDrawer` 的签名直接携带 `&Vec<Box<dyn Drawer>>`（tab_drawer.rs:29、docking_tab.rs:292/1121），dock 控件因此知道整个窗口注册表。
5. **选择状态没有单一事实来源**。ProjectWindow 持有 `selected_node: Option<NodeIndex>`；InspectorWindow 持有 `selected: Option<InspectorNodeInfo>`；SceneWindow 的选中在 `Engine.world` 里。三者靠 `Message::SelectProjectNode` 在中心 match 里手工同步（ui.rs:545-556）。
6. **`Drawer` trait 是 kitchen sink**。10 个方法混合生命周期、绘制、渲染命令、样式反射、身份；4 个布局容器（`LayoutLeftContainer` 等，layout.rs:18-187）为占坑实现了全部空方法，再由 `InitLayout` 消息把它们塞进 dock 树再"关闭"（ui.rs:426-457）。
7. **类型擦除堵死了多实例与插件化**。`ids: TypeIdMap<usize>` 一个类型只能有一个实例：两个场景视口、两个 Inspector、锁定的多选 Inspector 都做不了；闭包式构造（`Drawer::create(&World)`）没有应用上下文。
8. **没有 undo/redo 的落点**。编辑是就地修改（材质 Inspector 的 edit-in-place，material.rs:1306），保存靠快照塞消息；没有事务概念，加撤销只能继续加消息。

### 1.4 量化

| 指标 | 数值 |
|---|---|
| `kairos_editor` UI 代码 | 17,625 行 / 54 文件（占 crate 77%） |
| `Message` 变体 | 57 |
| `Context::handle` match 臂 | 57（函数体 365 行） |
| `get_window_mut::<T>()` 调用 | 33 处（全在 `handle`） |
| `messager.send` 调用 | 73 处 / 22 文件（审计口径） |
| `get_inspector_mut::<T>()` 二次下转型 | 10 处（全在 `handle`） |
| `engine.world` 直接读写 | 23 处（全在 `handle`） |
| drawer 生命周期调用（push/show/close） | 23 处（全在 `handle`） |
| `*Model` / `*InspectorModel` 状态载体 | 22 个 |
| `Drawer` impl | 13 个（其中 4 个是空布局容器） |
| `Drawer` trait 方法 | 10 个 |
| 依赖 `TypeId`/Any 下转型 | `get_window_mut` 33 处 + `get_inspector[_mut]` + `UIReader::get_drawer` |
| 交互级 UI 测试 | 0 |

### 1.5 根因：三个结构性错误

1. **全局可写状态**（global mutable state via queue）：任何组件都能发任意消息、任何状态都可能被任意 handler 改。不可推理、不可测试、不可并行。
2. **视图与模型同构**（window struct = model + view）：视图的生命周期被当成模型的生命周期，于是"打开/关闭窗口"和"打开/关闭文档"混为一谈。
3. **控制流藏在回调里**（inverted control flow）：消息是 fire-and-forget 的隐式跳转，没有调用栈、返回值、错误类型，也没有编译器检查"谁处理了它"。

---

## 2. 成熟设计怎么做

> 详细调研见 [rust-ui-architecture-survey.md](../../research/rust-ui-architecture-survey.md)（Rust 框架）与 [editor-ui-architecture-patterns.md](../../research/editor-ui-architecture-patterns.md)（成熟编辑器）。这里只给结论。

### 2.1 范式谱系：MVC 之后是什么

| 范式 | 代表 | 控制流 | 适合场景 | 对 Kairos 的意义 |
|---|---|---|---|---|
| MVC / MVP | 经典 GUI | Controller 回调 | 小工具、表单 | 现状近似（且更集中），已被证明会腐化 |
| MVVM + 数据绑定 | WPF、Slint | 双向绑定 + 信号 | 表单密集、声明式 UI | 双向绑定在 Rust 里要内部可变性，慎用 |
| **MVU / Elm Architecture** | **iced**、Elm | Message → update(Model) → Cmd | 状态机复杂、需要可测试 | **最贴合 Rust：enum + 穷举 match + 纯函数** |
| **Redux / Store + Reducer** | React、egui 大型应用 | dispatch(Action) → reduce → Effect | 大型应用、多模块 | **按领域切片可避免巨型 match** |
| 细粒度响应式 | Leptos、Dioxus、Floem、Slint | 信号依赖图自动重算 | 高频局部更新、声明式 | 与 egui 每帧重绘模型不匹配，只借鉴"派生状态" |
| 命令/动作系统 | Zed、Unreal Slate、VS Code、Godot | Action 注册表 + 派发 | 编辑器/IDE 必备 | **必须抄：菜单/快捷键/命令面板/MCP 共用** |
| 文档/视图分离 | 几乎所有编辑器 | Document 模型 + 多视图订阅 | 编辑器 | **必须抄：现在"窗口即文档"是最大结构错误** |

**关键认知**：这些不是互相排斥的框架，而是同一套"单向数据流"在不同层面的实现。成熟 Rust 编辑器（Zed）和大规模 egui 应用（Rerun）的共同点是：**状态是显式拥有的单棵树，视图是它的函数，副作用是显式的**——而不是"视图对象互相发消息"。

#### 2.1.1 MVC 到底"过时"在哪

不是 MVC 这个词过时，而是**它假设的交互模型**（视图与模型双向同步、Controller 兜住所有交互逻辑）在现代大型 UI 上会结构性腐化，Kairos 的代码是教科书级样本：

1. **同步逻辑没有归宿**。视图改模型、模型通知视图，这段"同步代码"不属于 M/V/C 任何一方；在 Kairos 里它就是 `Context::handle` 的 365 行 match，于是 Controller 变成上帝对象。
2. **状态可被多处修改**。MVC 没有规定"谁能写模型"，于是视图（`Cell`/`Mutex`）、Controller（`get_window_mut`）、甚至 `render()` 都能写；一旦有并发/异步（资产加载、纹理回绑）就产生竞态和静默丢弃。
3. **组合与嵌套无解**。MVC 面向"一个窗口一个模型"；现代编辑器是面板树 + 多实例 + 跨面板共享模型（选中、文档），MVC 没有对应的结构，只能靠消息/回调串起来。
4. **不可测试**。Controller 的输入是"某个 GUI 事件 + 整个应用状态"，没有可 mock 的边界；Kairos 现状 0 个交互测试。
5. **Rust 所有权放大矛盾**。视图持有模型引用 + 回调修改模型，在借用检查下只能退化成 `Rc<RefCell<_>>`/`Arc<Mutex<_>>`/"发消息让中心改"——即当前实现。

**替代它的不是某个新名词，而是把"控制流"显式化成数据流**：视图产出意图（Action），纯函数决定状态迁移（Reducer），副作用作为值返回（Effect）。MVU/Redux/MVI/命令系统都是这一条主线的不同包装。

#### 2.1.2 机制对照：现状 → 目标

| 现状机制 | 目标机制 | 解决什么 |
|---|---|---|
| `Messager` + `Message` 枚举 | `Action`（按领域切片）+ `reduce()` | 无类型化路由、巨型 match、时序隐式 |
| `get_window_mut::<T>()` 下转型派发 | 领域 reducer 直接改状态切片 | service locator、窗口必须存在 |
| 窗口 struct 持有 Model | `EditorState` 单棵树 + `Document`/`Selection` | 状态分散、跨窗口同步 |
| `ui(&self)` + `Cell`/`Mutex` 绕借用 | `fn(ui, &Frame, &mut UiOut)`（只读状态） | 视图越权改状态、内部可变性 |
| 消息里塞 `oneshot::Receiver` / 每帧 Tick 消息 | `Effect` + `EffectOutcome` Action + 固定 Tick 阶段 | 异步握手/定时器混进总线 |
| `render()` 里发消息/改世界 | `App::render_viewports(&state, &mut engine)` 单独阶段 | 绘制与副作用顺序不可推理 |
| `Cell<Option<FnOnce>>` 回调式 Dialog | `ModalState` 数据 + `Action::Modal` | 回调藏控制流 |
| `Drawer` 10 方法 kitchen sink | `Panel` trait（title/draw/closable） | 空实现样板、假抽屉 |
| 每窗口 `get_style_fileds` 反射 | `CommandRegistry` / `StyleSchema` 注册表 | 偏好设置遍历所有窗口 |
| MCP 给 Drawer 加 QueryHandler | `Query` 快照 + `Command` → `Action` | 外部自动化入口统一 |

### 2.2 Rust 生态的六种答案（细节见调研文档）

| 项目 | 意图派发 | 状态归属 | 副作用 | 可测试性 | 对 Kairos 的可借鉴度 |
|---|---|---|---|---|---|
| **iced** | `Message` enum → `update` | `Program`/应用状态 | `update` 返回 `Task` | 纯 update 极易测 | ★★★★★ 直接采用其 MVU 骨架 |
| **Rerun re_viewer**（大型 egui） | `SystemCommand` + 命令队列 | `StoreHub`/`ViewerContext` + Blueprint | 命令在帧边界执行 | 中等（大量集成测试） | ★★★★★ 同为 egui 编辑器，验证了"store + 命令"可行 |
| **Zed GPUI** | `action!` 宏 + `Context` 派发 | `Entity<T>` 集中管理 | `cx.spawn` 异步任务 | 中 | ★★★★ 编辑器命令系统的最佳参考 |
| **Slint** | 声明式回调 + 属性绑定 | 组件属性树 | 回调 | 中 | ★★ 双向绑定不适合 egui |
| **Xilem/Masonry** | 视图树 + Action | 组件状态 | 上下文对象 | 中 | ★★ 编译期组合思想可借鉴 |
| **Dioxus/Leptos/Floem** | 信号函数调用 | 细粒度信号图 | 异步资源 | 中 | ★ 与立即模式不匹配 |

**结论**：Kairos 应取 **iced 的 MVU 骨架 + Redux 的领域切片 + Zed/Rerun 的命令系统 + 编辑器领域的 Document/Selection 服务**，视图层继续用 egui。不要迁到 iced/Slint——3D 视口、wgpu 纹理嵌入、现有 egui 设计系统（样式 TOML、字体、dock）都是重资产，迁移收益远小于架构改造收益。

### 2.3 成熟编辑器的共同形状

调研 Godot / Unity / Unreal / Blender / VS Code / Zed / Rerun 后，共同点高度一致：

1. **命令/动作注册表**是唯一入口：菜单、工具栏、快捷键、命令面板、自动化（MCP/脚本）都调用同一个 `Command`，命令声明"何时可用"（enabled predicate）。
2. **文档模型与视图分离**：Document（可编辑数据 + dirty 状态 + undo 历史）被多个视图共享（Inspector、代码编辑器、预览）。
3. **选择是全局服务**：选中状态只有一个所有者，视图只订阅；不再由"项目窗口"和"Inspector 窗口"各存一份再手工同步。
4. **Inspector 是注册表驱动的派生视图**：资产类型 → 编辑器 provider，而不是"一个窗口里一堆 `Box<dyn Inspector>`"。
5. **副作用走显式的命令/事务**：保存、导入、撤销、打开对话框都是可记录、可回滚的操作。
6. **布局状态与内容状态分离**：dock 树只存 panel 标识，内容状态在文档/选择里。

#### 2.3.1 每个机制抄谁（可直接对照源码）

| Kairos 目标机制 | 成熟样本 | 抄什么 / 不抄什么 |
|---|---|---|
| 类型化 Action + 单点分发 | Zed `action!` / `#[derive(Action)]`（`Clone+PartialEq+serde`，可从 keymap JSON 反序列化）；Rerun `SystemCommand` + sender trait | 抄"action 是 Rust 类型 + 可序列化"；不抄 Zed 的 retained entity 树 |
| 命令注册表（菜单/快捷键/面板同源） | Unreal `TCommands` + `FUICommandList::MapAction`（can-execute 委托）；Godot `EditorCommandPalette` 注册；VS Code contribution points + `when`；JetBrains `AnAction` + `DataContext` | 抄"元数据 + 现算 enabled"；不抄 Unreal 的宏模板/多套注册 |
| 选择服务 | Godot `EditorSelection`；Unity `Selection`；Rerun `ApplicationSelectionState`（`on_frame_start` 失效解析）；Zed selection 历史与文档 undo 分离 | 抄"全局唯一 + 每帧校验 + 选择历史独立" |
| Inspector 属性面板 | Unity `SerializedObject`（自动 undo/styling）；Unreal `FPropertyEditorModule` customization；Rerun `DataUi` + `ComponentUiRegistry`；Blender RNA 元数据驱动 | 抄"属性模型 + 类型注册表"；不抄"每资产一个 inspector" |
| undo / 事务 | Godot `UndoRedo`（命令产生改动才入历史，命令本身不入）；Blender 只存数据不存 UI；Rerun `BlueprintUndoState`（时间线 + 拖拽合并 + 外部编辑合并） | 抄"文档级事务 + 合并策略"；不抄 Rerun 的整条时间线（太重，phase 1 用命令栈） |
| 布局 | Rerun `ViewportBlueprint` 直接存 `egui_tiles::Tree`，帧末统一应用；VS Code 保存 workbench grid；Godot `EditorDockManager` | 抄"布局是数据 + 帧末应用"；dock 控件本体保留现有实现 |
| 插件/扩展 | Godot `EditorPlugin` 扩展点；VS Code contribution points；Rerun `ViewClassRegistry` | 抄"注册表 + 上下文"，phase 5 再做 |
| 视图无状态 + logic/ui 两段 | eframe 0.37 `App::logic`/`App::ui`；Rerun `logic_impl`/`ui_impl` | 直接照搬命名与阶段边界 |
| 属性/事件通知 | Blender notifier（"失效通知 + 重读"，不推送完整新状态）；Zed observe/subscribe | 引擎→UI 用"失效/脏标记 + 快照重读"，不要推送大对象 |
| 消息/命令分层 | Rerun `SystemCommand`/`UICommand`/`RecordingCommand` 三层 | 这是防止新 `Action` 长成第二个 `Message` 的关键 |

### 2.4 对 Kairos 的取舍

**抄**：MVU/Reducer、Effect、Command Registry、Document/Selection、Inspector provider、undo 事务。
**不抄**：声明式 DSL（Slint/Xilem）、双向绑定、细粒度信号（与 egui 的立即模式模型冲突）、ECS 化 UI 状态（见 §5）。
**保留**：egui + 自研 dock + 样式 TOML + 现有 `Engine.world` 数据。

---

### 2.4 关键外部证据（每条设计主张都有出处）

调研文档里有完整引用，这里只列最硬的几条，说明本设计不是"重新发明"：

| 主张 | 外部证据 |
|---|---|
| 意图必须是类型化数据、单点分发 | iced 的 `Message` enum → `update`（Elm 架构）；Rerun `SystemCommand`（43 个变体）+ `SystemCommandSender` trait；GPUI `cx.emit` |
| 命令要按作用域分层，否则 enum 会爆炸 | Rerun 同时存在 `SystemCommand` / `UICommand` / `RecordingCommand` 三层（`re_ui` 里 re-export `UICommand`）——**这直接回答"一个 enum 会不会变成第二个 messager：会，所以要分层"** |
| 帧循环拆 logic / ui 两段 | eframe 0.37 的 `App` trait：required `ui` + provided `logic`，`logic` 在窗口隐藏时也会跑；Rerun `logic_impl` / `ui_impl` 同构 |
| 权威状态不能放 egui Memory | egui 官方：`Memory::data` "This is NOT meant to store any important data. Store that in your own structures!" |
| MVC 在 Rust 里会退化 | Raph Levien：*"Architectures that work well in other languages generally don't adapt well to Rust, mostly because they rely on shared mutable state and that is not idiomatic Rust."* |
| 一帧可能跑两次 ui pass | egui `max_passes = 2`（`request_discard` 时重跑）；Rerun `ui_impl` 注释明确提醒"不要假设一帧=一次 pass" |
| 选择要集中 + 每帧失效校验 | Rerun `ApplicationSelectionState` + `on_frame_start` 的 `resolve_item`（失效项降级/剔除） |
| undo 要处理拖拽合并与外部编辑 | Rerun `undo.rs`：`MAX_UNDOS=100`、外部编辑 250ms 合并窗口、slider 拖拽启发式合并 |
| 属性面板用类型注册表 | Rerun `re_data_ui`：`DataUi`/`EntityDataUi` + `add_to_registry`；Bevy 的 `bevy-inspector-egui` 仍在活跃维护 |
| 不要整体换框架 | iced/Slint/Xilem/GPUI 迁移会失去自绘控件与 egui↔wgpu 的零成本集成；Rerun 是"egui 能撑大型编辑器"的存在证明 |

## 3. 目标架构

### 3.1 分层与 crate
```
kairos_editor_widgets  （新 crate，L0+L1 纯外观：只依赖 egui；编译期拿不到任何领域类型）
  theme.rs      Design tokens / Theme（颜色、间距、字号、行高、圆角、图标尺寸）
  icon.rs       IconId + IconProvider（不依赖资产系统）
  atoms.rs      Button / IconButton / Checkbox / Switch / Radio / TextInput / SearchBox
                / SliderRow / NumberInput / ComboBox / ColorSwatch / Badge / Spinner / Label
  chrome.rs     PanelChrome / Toolbar / StatusBar / Breadcrumb / FormSection / EmptyState
  rows.rs       TreeView + TreeRow / ListRow / PropertyRow + PropertyList
  overlays.rs   ContextMenu / Menu / Modal + ConfirmDialog / Toast / Tooltip
  media.rs      Thumbnail + ImagePreview / DragGhost + DropTarget
  gallery/      全部 widget 的 storybook（dev bin；headless 可测）
  tests/        headless egui 测试（合成指针输入，无 GPU/window）

kairos_editor_core     （新 crate，纯逻辑：无 egui / winit / wgpu / World / IO）
  state.rs      EditorState = ProjectState + SelectionState + DocumentsState
                + LayoutState + StyleState + ModalState + StatusState
  action.rs     各领域 Action enum + 汇总路由
  effect.rs     Effect enum（纯数据）+ RequestId + 完成事件
  reduce/       project.rs / selection.rs / documents.rs / layout.rs / style.rs / modal.rs
  document.rs   Document / EditData / dirty / EditTransaction / undo
  command.rs    CommandRegistry（id、label、shortcut、enabled、Action）
  query.rs      只读查询 DTO（供 MCP/自动化，快照语义）
  tests/        reducer 单测（无 GPU、无窗口）

kairos_editor_ui       （新 crate，L2 功能组件：同时依赖 widgets + core，不依赖 host）
  frame.rs      Frame<'a>（state + world + host 资源）、UiOut（收集 Action）
  components/   project / hierarchy / inspector / material / scene / game / console /
                preferences；每个组件 = 布局 + **纯适配函数**
                （state → widget props、widget output → Action）
  view_state.rs ViewStates（会话态，按 PanelInstanceId）
  docking/      现有 dock（保留为布局控件，存 PanelInstanceId）

kairos_editor           （现有 crate 收缩为 host + bin）
  runtime.rs    winit/wgpu/egui 帧循环（改造）
  host.rs       Effect Runner（磁盘 IO、引擎变更、tokio 任务、原生对话框）
  app.rs        App：拥有 EditorState + UI host + Effect 执行器；dispatch/draw 编排
  install.rs    现有 install（保持不变）
  main.rs       入口
```

依赖方向（cargo 强制）：

```
kairos_editor_widgets   （leaf，只依赖 egui）
kairos_editor_core      （leaf，纯领域；只允许 engine 的纯数据类型）
          │                        │
          └────────► kairos_editor_ui ◄───────┘   （components = widgets + core）
                          │
                    kairos_editor（host + bin）
```

**为什么拆 core**：编译期强制「领域逻辑不能碰 egui/World/IO」；单测不需要 GPU/window；MCP 查询可以直接对 `EditorState` 做快照。

**为什么再拆 widgets（本轮新增）**：把「纯外观组件」物理隔离成一个只依赖 egui 的 crate，**widget 里一写业务类型就编译失败**——这比"约定先做组件再做逻辑"可靠得多。同时获得一个可复用的 widget kit（10+ 面板共享，同一种树/行/按钮只有一份实现）和 gallery 视觉评审入口。详见 [widget-layer.md](./widget-layer.md)。

**拆分粒度可以分期**：`kairos_editor_core` 与 `kairos_editor_widgets` 建议从一开始就独立成 crate（两者的边界强制力正是设计价值所在）；`kairos_editor_ui` 前期可以作为 `kairos_editor` 的 module 目录，等稳定后再抽 crate。


### 3.2 核心抽象（代码骨架）

```rust
// ── state.rs ──────────────────────────────────────────────
pub struct EditorState {
    pub project:   ProjectState,    // 资产树、registry 数据、watcher 待处理事件
    pub selection: SelectionState,  // asset: Option<Guid>, scene: Vec<Entity>
    pub documents: DocumentsState,  // DocumentId -> Document
    pub layout:    LayoutState,     // 打开的 panel、焦点、布局树（或仅 open/focus 意图）
    pub style:     StyleState,      // 全局样式值（TOML 的纯数据镜像）
    pub modal:     ModalState,      // Confirm / Error / Input / None
    pub status:    StatusState,     // 状态栏消息、最近错误
}

// ── action.rs ─────────────────────────────────────────────
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Project(ProjectAction),       // SelectAsset / RenameAsset / CreateAsset / ...
    Selection(SelectionAction),   // SelectAsset / SelectEntity / Clear
    Document(DocumentAction),     // Open / Edit / Save / Revert / Undo / Redo / Close
    Layout(LayoutAction),         // TogglePanel / ShowPanel / ClosePanel / FocusPanel / PersistLayout
    Style(StyleAction),           // SetField / Reset
    Modal(ModalAction),           // Open / Confirm / Cancel / Close
    Viewport(ViewportAction),   // Ensure(尺寸/绑定) / Orbit / Pan / Zoom / Pick / *Completed
    Play(PlayAction),             // Play / Pause / Stop
    Effect(EffectOutcome),        // 异步完成 / 失败
    Quit,
    Batch(Vec<Action>),           // 仅同步事务/宏（undo 记录点）；异步串联见 §3.6 的 pending 规则
}

// ── reduce.rs ─────────────────────────────────────────────
/// 唯一的写入口：纯函数。不碰 IO、不碰 World、不碰 egui。
pub fn reduce(state: &mut EditorState, action: Action) -> Vec<Effect> {
    match action {
        Action::Project(a)  => project::reduce(state, a),
        Action::Selection(a)=> selection::reduce(state, a),
        Action::Document(a) => documents::reduce(state, a),
        Action::Layout(a)   => layout::reduce(state, a),
        Action::Style(a)    => style::reduce(state, a),
        Action::Modal(a)    => modal::reduce(state, a),
        Action::Viewport(a) => viewport::reduce(state, a),
        Action::Play(a)     => play::reduce(state, a),
        Action::Effect(o)   => effect::complete(state, o),
        Action::Batch(actions) => actions.into_iter().flat_map(|a| reduce(state, a)).collect(),
        Action::Quit => vec![Effect::Quit],
    }
}

// ── effect.rs ─────────────────────────────────────────────
/// 副作用是"值"：可被构造、比较、记录、mock，不可在 reducer 里执行。
#[derive(Debug)]
pub enum Effect {
    // 磁盘 / IO
    WriteDocument { doc: DocumentId, path: PathBuf, bytes: Vec<u8> },
    LoadDocument { req: RequestId, path: PathBuf, kind: DocumentKind },
    PersistLayout { bytes: Vec<u8> },
    // 引擎 / 渲染（viewport 词表已由原型验证，见 ADR 0009）
    ApplyDocumentPreview { doc: DocumentId, payload: PreviewPayload },
    CreateViewport { ticket: EffectTicket, viewport: ViewportId, width: u32, height: u32 },
    SyncViewportSize { viewport: ViewportId, width: u32, height: u32 },
    DestroyViewport { viewport: ViewportId },
    ViewportCameraInput { viewport: ViewportId, input: CameraInput },
    PickViewport { ticket: EffectTicket, viewport: ViewportId, ndc: [f32; 2] },
    SpawnEditorCamera,   // production: spawn the editor camera entity
    // host / 平台
    FocusPanel(PanelId),
    NativeDialog(NativeDialog),
    SetCursor(CursorIcon),
    Quit,
}

// ── 异步完成走 Action，不再往消息里塞 oneshot ─────────────
#[derive(Debug, Clone, PartialEq)]
pub enum EffectOutcome {
    DocumentLoaded { req: RequestId, result: Result<LoadedDocument, String> },
    DocumentSaved  { doc: DocumentId, result: Result<(), String> },
    ViewportBound,                                    // reducer 用自己记录的 ticket 校验
    ViewportBindFailed { error: String },
    SceneObjectPicked { object: Option<SceneObjectId> },
}
```

**异步回流的落点（原型验证后修正）**：host 只负责把 completion 包装成 `Action::Effect(EffectOutcome)` 并 dispatch；**ticket → 语义的映射与陈旧校验由 reducer 拥有的 `RequestSlot` 完成**（如 `Document.pending_save: RequestSlot<SaveReq>`、`ProjectState.loading_requests`、`ViewportState.binding`）。host 侧可以有自己的 inflight 记账，但状态树必须自己持有 request slot——否则 completion 映射逻辑藏在 host 里，无法单测，也容易在"保存中继续编辑"这类竞态上丢 dirty 标记。建议生产实现里直接提供 `RequestSlot<T> { ticket, payload }` 类型，而不是三个手维护字段。

**三条铁律**（对应现状三个根因）：

1. **View 只读状态、只发 Action**：签名是 `fn(ui, &Frame, &mut UiOut)`，拿不到 `&mut EditorState`，不需要 `Cell`/`Mutex`。
2. **Reducer 纯**：不碰 IO/World/egui；所有需要外部世界的事，返回 `Effect`。
3. **Effect 是数据**：由 host 在帧边界执行；执行结果变成新的 `Action`。异步任务不进入状态树。

### 3.3 帧循环与数据流

```
redraw():
  1. exec.pump(&mut inbox)                     // tokio/平台/资产事件 → Action
  2. while let Some(a) = inbox.pop() {          // 状态变更阶段
         let effects = reduce(&mut state, a);
         effects.push_into(&mut outbox);
         if outbox 非空 { exec.run(&mut outbox, &mut engine); }  // 需要 &mut Engine 的副作用
     }
  3. render_viewports(&state, &mut engine)      // 场景/游戏/预览的 GraphicsCommand
  4. egui_ctx.run_ui(|ui| {
         let mut out = UiOut::new(&mut view_states);
         draw_panels(ui, &Frame { state: &state, world: &engine.world, host: &host }, &mut out);
         draw_modals(...); draw_toolbar(...);
         actions = out.actions;
     })
  5. 对 actions 重复步骤 2（本帧内生效，或显式延后一帧；二选一，全局统一）
  6. 按需 request_repaint()
```

与现状的关键差别：
- 绘制阶段**没有** `&mut EditorState`、**没有** `&mut Engine`，也不可能"画的时候顺手改世界"。
- 异步完成是 **Action**（`EffectOutcome`），不是消息里的 `oneshot::Receiver`。
- 副作用位置固定（步骤 2/5 的 effect 执行），不再有"render 里发消息、draw 里发消息、handle 里再发消息"三种时序。
- 绘制顺序 = 面板注册顺序，不再是 `drawers: Vec<Box<dyn Drawer>>` 的下标顺序。

#### 3.3.1 显式帧阶段（替代现在的隐式顺序）

把阶段按 eframe 0.37 / Rerun 的 **logic 段 / ui 段**再归一次类：`BeginFrame → ApplyIntents → EngineUpdate` 是逻辑段（不碰 `egui::Ui`，窗口隐藏时也能跑，可 headless 测试）；`RenderViews → BuildUi → PostUi → Present` 是绘制段。这个命名不是自创，是 egui 官方 `App::logic`/`App::ui` 与 Rerun 的实际结构，照搬即可。

现状的 `render_ui → handle_ui → draw_ui` 顺序（`runtime.rs:264-270`）带来两个隐性事实：`render()` 里发的消息当帧处理、`ui()` 里发的消息下一帧处理；`Engine::update()` 在 UI 之前，UI 写 World 要等下一帧才被消费。新架构把这些变成**有名字、有类型、可测试的阶段**：

| 阶段 | 做什么 | 可碰 | 备注 |
|---|---|---|---|
| 1. `BeginFrame` | drain 异步完成 / watcher / 资产事件 → Action；执行上一帧遗留的 `BeforeEngineUpdate` effect | `&mut Engine`（仅经 EffectExecutor） | 唯一允许写 World 的 UI 相关入口之一 |
| 2. `ApplyIntents` | reduce 所有 Action；执行 state-only effect（FocusPanel/光标/打开模态） | `EditorState` + UiHost | 纯状态迁移 |
| 3. `EngineUpdate` | `Engine::update()`（ECS rails + extract） | `&mut Engine` | 与现状位置一致 |
| 4. `RenderViews` | 按 `ViewportState` 生成场景/游戏/预览的 `GraphicsCommand` | `&EditorState` + `&mut Engine`（只写渲染图） | 取代各窗口的 `render()` |
| 5. `BuildUi` | `egui_ctx.run_ui`：面板只读状态、发 Action；**只在最后一遍 pass 收集** | `&EditorState` + `&World` + `&mut ViewStates` | 闭包必须无副作用，见下 |
| 6. `PostUi` | 收集到的 Action 立即 reduce（UI 状态本帧生效）；产生 `NextFrame`/`BeforeEngineUpdate` effect | `EditorState` | 关闭标签、选择等本帧可见 |
| 7. `Present` | tessellate + present；MCP 查询在此刻取快照 | — | 查询看到的是本帧最终状态 |

**显式规则**：

1. **World 写入时机**：需要引擎在下一帧 extract/camera controller 消费的写（`SceneView.size`、`SceneViewInput`、spawn editor camera），一律建模为 `Effect::When::BeforeEngineUpdate`，在阶段 1 执行。现状的"1 帧延迟"被保留（因为 `Engine::update` 在 UI 前），但变成契约而非巧合。
2. **egui multi-pass 安全**（新发现的必须项）：egui 0.35 默认 `max_passes = 2`，`run_ui` 闭包可能执行多遍（布局 `request_discard` 时）。（实证触发点：`PreferencesWindow` 里的 `egui::Grid` 首帧会 `request_discard`，见 `preferences_window.rs:229` 与 egui 的 `grid.rs`。）因此 `BuildUi` 里**不能**做"每帧一次"的副作用：Action 收集用双缓冲、只提交最后一遍的 `UiOut`；`Log::pop_front`、watcher drain、资产事件、`Engine` 变更全部移出闭包。实现上建议显式限制 pass 数（确认 egui 0.35 API 后设 `max_passes(1)`）或由 `App::frame` 包装"最后 pass 提交"。 现状的具体后果（审计给出）：多 pass 会让 `graphics_commands` 重复累积（scene/game/preview 命令执行两次），并让 `render()` 里的 `Registe*ViewBind` 创建两条 oneshot channel（后一条覆盖前一条，前一条 receiver 被丢弃）。这两类 bug 在新架构里从设计上消失（RenderViews 在 pass 外，Effect 有 RequestId）。
3. **查询快照时机**：MCP/自动化的 query 在 `Present` 前（阶段 6 之后）取，保证"UI 本帧动作 + Engine 本帧更新"都可见；command 在阶段 1/2 之前入队，保证"本帧 World 生效"（替代 MCP 计划里“commands 在 draw 前、queries 在 draw 后”的近似方案）。
4. **退出协议**：`Present` 后处理退出请求（`Effect::Quit` / winit Close）→ 有未保存文档则走 `ModalState` 确认 → `App::shutdown()`（现状 `Editor::on_exit` 是空 TODO，新架构必须定义）。

### 3.4 状态归属规则

| 状态 | 放哪 | 理由 |
|---|---|---|
| 资产树、registry、watcher 待处理事件 | `core::ProjectState` | 领域数据，可序列化、可测 |
| 选中（资产/实体） | `core::SelectionState` | 单一事实来源，视图只读 |
| 打开文档、编辑缓冲、dirty、undo 历史 | `core::DocumentsState` | 与"窗口"解耦；多视图共享 |
| 面板打开/关闭/焦点 | `core::LayoutState` | 可持久化、可被 MCP 查询 |
| dock 分割树、拖拽中的临时 rect、滚动位置、展开状态、重命名输入框缓冲 | `ui::ViewStates`（`PanelInstance` 为 key） | 纯 UI 临时态；重启无需恢复，或单独持久化 |
| 样式值（TOML 的纯数据） | `core::StyleState` | 偏好设置、主题、MCP 可读写 |
| egui 纹理句柄、字体、图标 | `ui::HostView` | GPU 资源不属于领域模型；UI 只拿 plain `egui::TextureId`，core 不存句柄 |
| ECS 实体/组件、渲染资源 | `Engine.world` | 引擎所有；UI 只读 `&World`，变更走 Effect |
| 异步任务、通道、文件句柄 | host 的 `EffectExecutor` | 不属于可序列化状态 |

**边界判据**：能序列化、需要 undo/持久化/MCP 读写的 → core；只影响当前帧视觉与交互的 → ViewStates；引擎世界的 → World。

**egui `Memory` 的定位**：egui 的 `Memory`/`Id` 只存控件瞬态（焦点、滚动、折叠、弹窗锚点等「这一帧/这一交互」的信息），不能替代领域状态。判断标准：重启后是否应该恢复？需要 undo/MCP 看到？是 → core；否 → ViewStates/Memory。

**与世界状态的边界（结合引擎调研）**：
- **相机/视口状态留在 World**：`EditorCameraController`（`camera.rs`）是 orbit 状态唯一真值，`SceneView`/`GameView` 的 size/camera 也在 World；core 不复制这些数据，MCP 查询从 World 快照提取，相机输入走 `Effect::When::BeforeEngineUpdate`。把它们搬进 Store 会造成双真值（现状相机在 World、MCP 却假设在 SceneWindow，已经是这种矛盾）。
- **Log 不是 World 资源，也几乎空转**：现状 `Log` 在 `Editor` 字段，唯一生产者是 About 窗口每帧 `info`、唯一消费者是 Console 每帧 `pop_front`；真正的日志走 `log` crate → stderr。新架构把"可回放、可过滤、可 clear、关闭面板不丢"的结构化环形日志放在 host 侧，Console 只读快照（`ConsoleState` 只存查询/游标），不再 `pop_front` 消费历史。
- **稳定 ID 规则**：`Action`/`Query`/布局持久化**禁止**使用 `NodeIndex`/`usize`/`TypeId` 作为身份（项目树重扫会重建索引）；一律用 `Guid` / `AssetPath` / 稳定字符串 `PanelId`。这是现有代码已在踩的坑（项目树重扫后按 GUID 找回选中项，`project_window.rs:177-200`）。
- **键盘焦点仲裁**：现状键盘事件同时给 egui 和 `InputEngine`（`runtime.rs:496-526`），新架构需要显式 focus scope：UI 文本框聚焦时不吃场景 WASD；场景视口 hover/focus 时才把输入投给引擎。

### 3.5 面板、dock 与多实例

> 实现形态（原型已验证，2026-11-09）：**Panel trait + PanelRegistry + PanelHost**；面板**不**写进
> `EditorApp` 字段，`PanelId` 是稳定字符串而不是核心 enum，新增一个面板 = 实现 trait + 注册 factory。
> 完整设计/真引擎映射见 [panel-viewport-design.md](./panel-viewport-design.md)，决策见
> [ADR 0009](../../adr/0009-editor-panel-host-and-viewport.md)。

```rust
// core：身份是数据（新增面板不改 core），布局是可持久化/可查询的领域状态
pub struct PanelId(pub &'static str);            // "project" / "inspector" / "viewport" / ...
pub struct PanelInstanceId { pub kind: PanelId, pub serial: u32 }

// ui（L2）：面板只读 Frame、只发 Action；实例与 view state 由 host 持有
pub trait Panel {
    fn kind(&self) -> PanelId;
    fn title(&self, frame: &Frame) -> String;
    fn draw(&mut self, ui: &mut egui::Ui, frame: &Frame, out: &mut UiOut);
    fn closable(&self) -> bool { true }
    fn wants_continuous_repaint(&self) -> bool { false }
}
pub struct PanelRegistry { /* PanelId -> (DockSlot, factory) */ }
pub struct PanelHost { /* instances + slot/tab layout */ }
```

- **归属**：打开/焦点在 core `LayoutState`（可持久化、MCP 可查询）；dock 树、tab 顺序、分割比例、拖拽瞬态在 host/UI view state；面板会话态（搜索词、展开集合、滚动）跟实例走，不进 core。
- **新增面板成本**：实现 `Panel` + 组合根里 `registry.register(kind, slot, factory)` 一行；不改 `editor-core`、不改 `EditorApp`、不新增 `Message`。原型的 Gallery 面板就注册在 host crate 里，core 完全不知道它。
- **多实例**：`PanelInstanceId(kind, serial)` 从第一天就有；两个场景视口 = `viewport#0`、`viewport#1`（kind 相同、serial 不同），view state 按实例 key 分离，布局 API 不用改。
- **dock**：原型的四槽（left/center/right/bottom）+ tab bar 是 `PanelHost` 的最小实现；生产把现有 `DockState` 泛型化为 `PanelInstanceId` 后接到 `kairos_editor_docking`，`PanelHost` 接口不变（`TabDrawer` 只调 `Panel::draw`，不再携带 `&Vec<Box<dyn Drawer>>`/`&mut Messager`/`&Engine`）。
- **布局持久化**：`Effect::PersistLayout` 序列化 dock 树（`serde`）；启动时恢复 `LayoutState` + dock 布局。

### 3.6 文档、选择、撤销

```rust
pub struct Document {
    pub id: DocumentId,
    pub asset: Guid,                 // 对应的资产身份
    pub path: PathBuf,
    pub kind: DocumentKind,          // Text / Toml / Material / Texture / Shader / Audio...
    pub edit: EditData,              // 当前编辑缓冲（纯数据）
    pub saved: EditData,             // 上次落盘内容（用于 dirty / revert）
    pub history: Vec<EditTransaction>,
}

pub struct EditTransaction { pub label: &'static str, pub before: EditData, pub after: EditData }
```

- **打开文档**：`Action::Document(Open { asset })` → 若已打开则聚焦；否则 `Effect::LoadDocument` → `EffectOutcome::DocumentLoaded` → reducer 建 Document。
- **编辑**：每次字段修改 = `Action::Document(Edit { id, patch })` → reducer 更新 `edit`、按需合并事务；**预览**由 `Effect::ApplyDocumentPreview` 显式写入引擎（不再 edit-in-place 后靠 dirty 标记兜底）。
- **保存**：`Effect::WriteDocument` → `DocumentSaved` → 清 dirty、`saved = edit.clone()`。
- **关闭有未保存修改**：reducer 置 `ModalState::Confirm { confirm: DocumentAction::SaveThenClose { id }, cancel: DocumentAction::DismissClose }` —— 模态框是**数据**，不是持有 `Cell<FnOnce>` 的对象。**不要写成 `Batch([Save, Close])`**：`Save` 是异步 effect，紧随其后的 `Close` 会重新触发未保存确认或丢编辑（原型已实测该竞态）。异步串联必须用显式 pending 状态：reducer 置 `doc.close_after_save = true`，`DocumentSaved` completion 成功后 reducer 再关闭；失败则保持 dirty 并弹错误 modal。规则：**`Batch` 只用于同步 action；「先异步、后动作」必须建模成文档/请求状态机**。
- **选择（Selection）**：全局唯一 `SelectionState`，所有面板读写同一对象；每帧 `BeginFrame` 做一次**失效校验**（实体被删除、资产卸载、面板关闭 → 剔除或降级），对应 Rerun `ApplicationSelectionState` 的 `on_frame_start` + `resolve_item`。选择变化触发多面板同步，但重型派生数据（Inspector 加载文档）延到 logic 段，不在绘制中做。
- **撤销**：分两类命令——`transient`（改 UI 会话态：布局、选择、悬停，**不进 undo**）与 `persistent`（改文档，进 undo）。phase 1 做文档级事务栈：连续输入/拖拽按时间窗合并成一个 undo 点（Rerun 的外部编辑合并窗口 250ms、slider 拖拽启发式合并可作参数）；`MAX_UNDOS` 上限（Rerun 取 100）。`EditTransaction` 存 before/after 快照（文档数据小，可接受）；场景编辑的跨 ECS 事务留到 phase 5，但接口先按 `Action::Batch` + 事务单元留好落点。
- **权威文档 store → 运行时投影**：文档是权威数据，引擎里的运行时材质/预览是从文档**投影**出来的（`Effect::ApplyDocumentPreview` 必须幂等）；这样"保存"只是把文档写回磁盘，"预览"只是投影，两者不互相污染。Rerun 的 store/blueprint 分离是同一思路。

### 3.7 Inspector：provider 注册表，而不是"一个窗口一堆 dyn"

```rust
pub fn inspector_panel(ui: &mut egui::Ui, frame: &Frame, out: &mut UiOut) {
    let Some(doc) = frame.state.documents.active() else { /* empty state */ return; };
    match &doc.edit {
        EditData::Text(text)     => text_editor(ui, doc, text, out),
        EditData::Toml(table)    => toml_editor(ui, doc, table, out),
        EditData::Material(mat)  => material_editor(ui, doc, mat, out),
        EditData::Texture(edit)  => texture_editor(ui, doc, edit, out),
        EditData::Shader(text)   => shader_editor(ui, doc, text, out),
        EditData::AudioPreview(a)=> audio_editor(ui, doc, a, out),
    }
}
```

- 新增资产编辑器 = 新增 `EditData` 变体 + 一个 `*_editor` 函数 + loader/保存 effect；**不改任何中心 match、不加 Message 变体、不改 Drawer trait**。
- 现在"Inspector 自己从文件读并持有 `Box<dyn Inspector>`"（`InspectorCreater::create_from_asseet_kind`）被 Document 替代；预览渲染（材质/网格 3D 预览）变成"文档 → viewport 注册表"的一项，不再挂在窗口 `render()` 上。
- **属性 UI 的第二次演进（phase 5，可选）**：把"材质/RenderState/组件字段"的编辑行做成**类型 → 编辑函数**注册表（`DataUi` 式，Rerun `re_data_ui` / Bevy `bevy-inspector-egui` 的成熟做法），新类型接入属性面板不改面板代码；可复用 `kairos_ecs` 已有的 reflect 能力。第一版先手写字段行，重复三次以上再抽。

### 3.8 Effect 与异步

- **Effect Runner** 是 host 上唯一能碰 `&mut Engine`、tokio、磁盘、原生对话框的地方；它用 `trait EffectExecutor` 抽象，测试可 mock。
- **异步任务**：`Effect::LoadDocument` 等携带 `RequestId`；host 起 tokio task，完成后把 `Action::Effect(EffectOutcome)` 推回 inbox。**不再有** `oneshot::Receiver` 进入状态或消息。
- **陈旧结果**：completion 带 `RequestId`，reducer 校验当前文档/视口仍持有该 id，否则丢弃（对应现状"消息发晚/窗口关了"的竞态）。
- **每帧 Tick**（音频预览、播放时钟）：由帧循环在固定阶段产生 `Action::Play(Tick { dt })`，不再是一条 `AudioInspectorTick` 消息。
- **引擎 → UI 通知**（资产热重载、watcher）：host 在帧边界 drain（`AssetEvent` / watcher channel）→ 转成 `Action::Project(ExternalChanged)`。引擎侧的 `kairos_ecs::message`（`MessageWriter`/`MessageReader`）继续用于引擎内部系统间通信，但**不**作为 UI 总线。

### 3.9 Command / Query：一套入口服务所有调用方

```rust
pub struct Command {
    pub id: CommandId,                    // 稳定字符串，可序列化
    pub label: &'static str,
    pub shortcut: Option<KeyChord>,
    pub enabled: fn(&EditorState) -> bool,
    pub action: fn(&EditorState) -> Action,   // 由当前状态构造（如"保存当前文档"）
}
pub struct CommandRegistry { commands: Vec<Command> }
```

注意 `action` 必须是**函数**而非常量 `Action` 字段：否则 `SaveActive`/`UndoActive`/`CloseActive` 这类「对当前选中对象做某事」的命令会逼出一堆 `*Active` 变体（原型实测过这个坑）。参数化命令（MCP/脚本）也用 `fn(&EditorState) -> Result<Action, CommandError>` 的形态。

- 工具栏按钮、菜单项、快捷键、命令面板、MCP、脚本全部走 `registry.dispatch(id)` → `Action` → 同一个 reducer。
- **MCP/自动化**：查询 = `fn(&EditorState, &World) -> QueryResponse`（快照 DTO）；命令 = `EditorCommand` DTO → `Action`。`docs/plan/kairos-editor-mcp` 里"给每个 Drawer 加 `QueryHandler`"的中间层设计可以整个删掉——它是在为"状态分散 + 没有应用级 Model"打补丁。
- **既有 MCP 计划需要同步修订**：`phase3-middleware-architecture.md` 的 `Drawer::as_query_handler()`、`handle_asset_server()` 分发、以及“commands 在 draw 前 / queries 在 draw 后”的 hook 位置都建立在“状态分散在 Drawer”的前提上；本架构落地后应改为“Store 快照 + 命令入队”，并把该文档标记为 superseded 部分。
- **命令必须按作用域分层**（否则 `Action` 会长成第二个 `Message`）：Rerun 的做法是同时存在 `SystemCommand`（系统级：打开数据源、截图、退出）/`UICommand`（会话级：布局、面板、焦点）/`RecordingCommand`（数据编辑）。Kairos 对应三层：
  - `UiAction`：面板开关、焦点、布局、悬停——**不可 undo、不进文档**（本提案里就是 `LayoutAction`/`StyleAction`）；
  - `DocumentAction`：文档编辑、保存、回滚——**可 undo、持久化**；
  - `SystemAction`/`EditorCommand`：打开/切换项目、退出、MCP 入口——**对外稳定、可序列化、可审计**。
- **发送方依赖 trait，不依赖具体 App**：面板只拿到 `&mut UiOut`（内部是 `impl ActionSink`），引擎侧系统要发命令时依赖 `trait ActionSink { fn send(&self, Action); }`。这是 Rerun `SystemCommandSender` 的关键经验：**单向依赖 + 可单测**，不产生"面板 ↔ App"循环。

### 3.10 样式、偏好设置、模态

> 外观 token 的落点是独立的 `kairos_editor_widgets`（Theme）：`StyleState` 是权威值，`Theme` 是纯映射，widget 通过 egui 0.35 的 `Theme`/`widget_style`/class 取样式。细节见 [widget-layer.md](./widget-layer.md) §2.1。

- `StyleState` 是 TOML 的纯数据镜像；UI 层把它映射成 egui 样式（`frame.style()`）。
- 偏好页由 `StyleSchema` 注册表生成（替代现在"PreferencesWindow 遍历所有 drawer 调 `get_style_fileds`"）。改字段 = `Action::Style(SetField)` → reducer + `Effect::PersistStyle`。
- 模态/对话框 = `ModalState` 数据 + `Action::Modal`；原生文件对话框走 `Effect::NativeDialog` + 完成 Action。`Cell<Option<Message>>`/`Cell<Option<FnOnce>>` 的回调式 Dialog（dialog.rs）整个删除。

### 3.11 测试策略

| 层 | 工具 | 测什么 |
|---|---|---|
| core reducer | 普通 `#[test]` | 每个 Action 的状态迁移 + 返回的 Effect（纯、毫秒级） |
| core 领域 | 普通 `#[test]` | 项目树/资产 registry/文档事务/undo（迁移现有 74 个测试） |
| ui 交互 | `egui_kittest`（headless，按 accesskit 节点查询和点击，可选快照） | "点删除 → 弹确认 → 点确认 → 节点消失"这类交互 |
| host effect | mock `EffectExecutor` | Effect 执行顺序、错误处理、陈旧 completion |
| 端到端 | 可选 | 启动 → 打开资产 → 编辑 → 保存（无 GPU 模式尽量覆盖） |

现状 0 个交互测试；改造后 reducer 测试可以覆盖现有 Message 链路 80% 的逻辑。

---

## 4. 三个端到端例子（before / after）

### 4.1 重命名项目节点

**现在**（6 处改动、2 个共享可变状态）：
`HierarchyPanel` 双击 → `Message::StartRenameProjectNode(idx)`（`Arc<Mutex<String>>` 缓冲）→ 用户输入 → Enter → `Message::RenameProjectNode` → `handle` → `ProjectWindow::rename_node` → 改 graph + 存 registry；ContentPanel 也发送同一批消息，靠注释"防止 hierarchy 和 content 同时发送导致重复处理"（project_window.rs:290）兜底。

**之后**：
```rust
// view
out.emit(Action::Project(RenameAsset { asset, new_name }));
// reducer
fn rename(state, asset, new_name) -> Vec<Effect> {
    state.project.rename(asset, new_name)?;
    vec![Effect::RenameOnDisk { .. }, Effect::PersistAssetRegistry]
}
```
无共享缓冲（重命名输入缓冲在 `ViewState`）、无消息、无中心 match；重复处理不可能（改名是幂等 action）。

### 4.2 材质编辑 / 保存 / 放弃

**现在**：6 个 Message 变体；`Arc<Mutex<Option<SerializedMaterial>>>` 快照随消息传递；edit-in-place 改运行时 Material 做预览；dirty 用 `Cell<bool>`；关闭时 `Dialog` 持有 `apply_message`/`discard_message`（material.rs:1300-1318），丢弃还要另一个消息回滚运行时材质。

**之后**：
```rust
// 编辑：只改文档缓冲，预览是 Effect
out.emit(Action::Document(Edit { id, patch: FieldPatch::Blend(x) }));
// reducer: 更新 doc.edit、dirty=true、合并事务 → vec![Effect::ApplyPreview { id }]
// 保存
out.emit(Action::Document(Save { id }));       // 若 dirty
// effect: WriteDocument → EffectOutcome::DocumentSaved → 清 dirty
// 放弃
out.emit(Action::Document(Revert { id }));     // doc.edit = doc.saved; → Effect::ApplyPreview
```
预览、dirty、保存失败、关闭确认全部有单一语义；撤销=事务栈，不再需要"丢弃消息"。

### 4.3 场景视口 + 异步纹理回绑

> **原型已实现（2026-11-09）**：`prototypes/editor-ui` 的 `editor-viewport` 用 wgpu offscreen
> Rgba8UnormSrgb + `register_native_texture` 走通「host 渲染 → TextureId → egui image」；core 的
> `ViewportAction/Effect`（Ensure/CreateViewport/SyncViewportSize/DestroyViewport/ViewportCameraInput/
> PickViewport）与 ticket 校验、resize 回放、关闭面板 destroy 都有测试；视口面板在
> `editor-components/src/viewport_panel.rs`，host 在 `editor-viewport/src/{host,renderer}.rs`。
> 生产替换点（引擎 render graph / BindAttachmentToEgui / 纹理缓存 / pass 顺序 / pan / 拾取）见
> [panel-viewport-design.md](./panel-viewport-design.md) §3、§6。

**现在**：`OpenSceneTab` → `SceneWindow` 创建 oneshot channel → `Message::RegisteSceneWindowViewBind(receiver)` → 每帧 `render()` 里发 `Message::SceneWindowTryReceTextureId` → `handle` 里 `try_rece`；窗口大小、相机输入、关闭全部走消息。

**之后**：
```rust
// 打开面板（layout 是 core 数据）
Action::Layout(LayoutAction::ShowPanel { panel: PanelId::VIEWPORT })
// 面板首帧上报物理尺寸并请求绑定（幂等）：
Action::Viewport(ViewportAction::Ensure { viewport, width, height })
  → Effect::CreateViewport { ticket, viewport, width, height }
// host 创建 render target，完成后：
Action::Effect(EffectOutcome::ViewportBound)      // reducer 用自己记录的 ticket 校验
// 每帧：views 读 frame.host.viewport_texture(viewport)（egui::TextureId）；
// 尺寸变化 → Ensure → Effect::SyncViewportSize；相机 → Effect::ViewportCameraInput；
// 点击 → Effect::PickViewport → SceneObjectPicked
// 关闭 → Action::Layout(ClosePanel) → Effect::DestroyViewport；陈旧 completion 按 ticket 丢弃
```

---

## 5. 备选方案与取舍

### 方案 A：把编辑器状态放进 ECS World（World resource + system 当控制器）

- 优点：复用现成 schedule/Message 系统；MCP 可通过系统读；无第二套状态。
- 缺点：UI 状态必须 `Send + Sync`；draw 阶段无法拿 `&mut World`（只能读快照或拆 resource），反而更别扭；UI 状态与游戏数据混在一起，序列化/测试/热重载都变重。
- 结论：**不推荐**。引擎世界的数据归引擎；编辑器 UI 领域模型独立，两者在 Effect 边界转换。

### 方案 B：MVU + Store + Effect（本提案）

- 优点：Rust 友好（enum+match+所有权）、可测试、egui 友好（立即模式 + 外部 store）、与 MCP/命令系统天然统一。
- 缺点：需要 discipline（view 不越权、reducer 不碰 IO）；Action/Effect enum 需要按领域切分否则会膨胀。
- 结论：**推荐**。

### 方案 C：细粒度响应式（Floem/Dioxus/Leptos 风格信号）

- 优点：局部更新、依赖自动追踪、代码局部性好。
- 缺点：与 egui 每帧重绘/立即模式不匹配；Rust 信号需要 `Rc<RefCell>` 或在框架内做 arena；3D 视口嵌入成本高。
- 结论：**不整体采用**；可在 core 里给派生数据加缓存（在 MCP 查询/复杂派生视图上按需使用），但不作为 UI 架构。

### 方案 D：增量修补现状（保留 Drawer，加 trait/服务）

- 典型做法：给 `Drawer` 加 `QueryHandler`（MCP 方案）、把 `Messager` 换成 typed queue、给窗口加 `&mut self`。
- 为什么走不通：根因是"窗口=模型"和"全局可写状态"，trait 修不掉；MCP 方案自己已经遇到"状态分散"的墙。每加一个能力都要在 10 个窗口上复制样板。
- 结论：**只适合作为过渡期止血**（见 rewrite-plan 的 strangler 选项），不解决根本问题。

| 维度 | A ECS 化 | **B MVU（推荐）** | C 响应式 | D 增量修补 |
|---|---|---|---|---|
| Rust 契合度 | 中 | **高** | 中 | 低（越修越拧） |
| egui 契合度 | 低 | **高** | 低 | 高 |
| 可测试性 | 中 | **高** | 中 | 低 |
| 重构成本 | 高 | 中 | 高 | 低（但持续付利息） |
| MCP/命令统一 | 中 | **高** | 中 | 低 |
| 长期上限 | 中 | **高** | 高 | 低 |

---

## 6. 风险与反模式（实施时盯住）

0. **Widget / Component 边界腐化**（新）：widget 里出现领域类型或回调闭包、component 里直接改状态/做 IO、样式当 props 传。→ 用 crate 边界 + `Output` 结构 + 只发 Action 三条硬约束挡住，详见 [widget-layer.md](./widget-layer.md) §7。

1. **Action enum 膨胀成第二个 `Message`**：必须按领域拆 enum + 每个领域独立 reducer 模块；任何 reducer 超过 ~300 行就拆。
2. **"顺手在 view 里改状态"回潮**：`Frame` 只给 `&EditorState` + `&World`，编译器强制；code review 禁止 `UnsafeCell`/`RefCell` 出现在面板代码。
3. **Effect 顺序依赖**：effect 由 host 按序执行；有依赖的操作用 `Action::Batch` 表达，不要依赖"先发哪个 effect"。
4. **过度抽象**：不要造通用框架/反射式属性系统；先 hand-written 面板，重复出现三次再抽。
5. **丢失 GPU 侧状态**：纹理句柄不进 core；host `HostView` 只暴露 `egui::TextureId`（原型已验证），core 不依赖 egui/wgpu。
6. **dock 控件重构范围失控**：phase 1 保留现有 dock 实现，只换 `PanelInstanceId` 和 `TabDrawer` 签名；不要顺手重写 2.5k 行 dock。
7. **迁移期双写**：新旧架构并存期，旧 `ui` 模块冻结、只修 bug；新入口用 feature flag 切换，避免两边同时演进。

---

## 7. 开放问题（需要作者决策）

1. **重写 vs 增量**：是否接受"新 crate 并行、旧 UI 冻结、按面板迁移"的 strangler 路径？（推荐）
2. **文档范围**：phase 1 只覆盖资产类文档（text/toml/material/texture/shader），场景编辑文档（实体/组件/层级）是否同期纳入？（推荐 phase 5）
3. **undo 范围**：只做文档级撤销，还是场景/层级编辑也要事务撤销？（后者成本高，建议分期）
4. **多实例**：是否需要多场景视口/多 Inspector 实例？（架构已支持，实现成本在 dock 侧）
5. **游戏 UI**：`kairos_ui`（引擎的游戏内 UI）与编辑器 UI 是否要共享这套 store/命令模型？（建议先不动，引擎侧保持 ECS 风格）
6. **命令/查询的序列化**：MCP/脚本是否需要稳定的 `Action` JSON 协议？（若是，`Action`/`Query` 的 DTO 需要定版本）
7. **样式系统**：是否保留每面板 TOML + 反射式的 `get_style_fileds`，还是收敛成全局 `StyleState` + schema？（推荐后者，phase 4）
8. **帧循环形态**：保留连续 `request_redraw`（对 MCP/帧语义最友好，但 CPU 持续跑满），还是事件驱动 + 正确 honor `repaint_delay`？（现状 `runtime.rs:384` 无条件再要一帧；新架构需要先定这个，因为它决定 MCP 的“每帧”语义。）
9. **`Engine::update()` 位置**：保持现状（UI 之前，UI 写 World 有 1 帧延迟）还是移到 UI 之后（消除延迟，但改变 extract/present 与 UI 读 World 的语义）？推荐**保持现状**并显式化 `BeforeEngineUpdate` effect，风险最低。
10. **Log 方案**：自研环形缓冲 + 显式 logger，还是 `tracing` capture layer（与 env_logger 有重复初始化冲突）？推荐前者 + `log` crate bridge。
11. **相机状态归属**：已建议保留在 World（`EditorCameraController` 唯一真值），MCP 查询从 World 快照提取。是否同意？（若不同意，需要把相机状态迁入 core，并同步改 MCP 查询。）

---

## 8. 参考

> Widget/Component 分层设计见 [widget-layer.md](./widget-layer.md)；其调研与组件目录见 [ui-widget-component-layer-survey.md](../../research/ui-widget-component-layer-survey.md) 与 [editor-ui-widget-catalog.md](../../research/editor-ui-widget-catalog.md)；合并原型见 [prototypes/editor-ui/](../../../prototypes/editor-ui/)。

- 现状审计：[editor-ui-messager-mvc-audit.md](../../research/editor-ui-messager-mvc-audit.md)
- Rust UI 调研：[rust-ui-architecture-survey.md](../../research/rust-ui-architecture-survey.md)
- 编辑器模式调研：[editor-ui-architecture-patterns.md](../../research/editor-ui-architecture-patterns.md)
- 引擎集成约束：[editor-ui-engine-integration.md](../../research/editor-ui-engine-integration.md)
- 重写计划：[rewrite-plan.md](./rewrite-plan.md)
- 决策记录：[ADR 0008](../../adr/0008-editor-ui-architecture.md)
- 现有 MCP 方案（将被本架构取代其中间层）：[phase3-middleware-architecture.md](../../plan/kairos-editor-mcp/phase3-middleware-architecture.md)
- 可运行原型：[`prototypes/editor-ui/`](../../../prototypes/editor-ui/)（五 crate：`editor-core` 状态/reducer/effect/viewport slice + `ui-widgets` 纯外观 kit + `editor-components` Panel trait/Registry/Host 与面板 + `editor-viewport` wgpu RTT/camera/pick + `editor-app` 帧阶段/host；`cargo test` **126 个测试**，含 7 个 full-stack + 14 个 panel host + 11 个 viewport slice；`cargo run` 打开真实窗口，截图见原型 README）
