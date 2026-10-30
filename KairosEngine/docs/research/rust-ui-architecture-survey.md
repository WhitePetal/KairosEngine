# Rust UI 架构调研：面向 egui 引擎编辑器的替代方案

> **文档定位**：为 kairos_editor UI 架构重写提供的外部一手资料调研。回答一个问题：
> *除了「全局消息队列 + MVC/回调」之外，一个 10k+ LOC 的 egui 引擎编辑器应该采用什么架构？*
>
> **范围**：只调研外部资料，**不检查 KairosEngine 代码**。现状问题（Messager 消息回调、MVC/state 结构）以任务简报为准，本文只给架构层建议与可迁移模式。
>
> **取证日期**：2026-10-30（下文版本号均为当日 docs.rs / GitHub 最新可见版本）。
> **方法**：优先一手来源——框架官方 API 文档（docs.rs / 官方 docs 站）、官方 changelog、框架作者博客、GitHub 源码；第三方理论资料仅用于背景并会标注。
>
> **证据等级标记**：**[源码]** = 直接读到的框架源码/rustdoc 签名；**[官方文档]** = 官方文档站；**[release notes]** = 官方 changelog；**[博客]** = 作者本人博客（一手观点，不是 API）。

---

## 0. 结论摘要（TL;DR）

1. **不要用无类型的全局消息总线做意图分发**。主流 Rust UI 框架的共识是「意图必须是类型化的数据（enum/struct）」：iced 是 `Message` enum 进 `update`（[docs.rs/iced](https://docs.rs/iced/0.14.0/iced/)），Rerun 是 `SystemCommand` 40+ 变体进 `send_system`（[源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/command_sender.rs)），GPUI 是 `cx.emit(Event)`（[源码](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app/context.rs)）。类型化意图 = 可穷举 match、可日志/可回放、可序列化测试。
2. **架构上最接近 Kairos 的目标形态是 Rerun re_viewer**：大规模 egui 应用、`re_ui` 设计系统 + `SystemCommand` 命令通道 + `StoreHub`（数据 store）与 blueprint（编辑器配置 store）分离 + `ApplicationSelectionState` + 基于时间线的 undo + `DataUi` 反射属性 UI。**它是「egui 能做到什么」的最强存在证明**（[re_viewer_context 0.41 文档](https://docs.rs/re_viewer_context/0.41.0/re_viewer_context/)）。
3. **帧循环要拆成「logic（每帧都跑，包含窗口隐藏时）+ ui（只在 egui pass 跑）」两段**。这是 Rerun 的实际实现（`app/logic.rs` 的 `logic_impl` + `app/ui.rs` 的 `ui_impl`，[源码](https://docs.rs/crate/re_viewer/0.41.0/source/src/app/logic.rs)），并且 egui 官方的 `eframe::App` trait 现在也长成这样：required `ui(...)` + provided `logic(...)`（[docs.rs/eframe](https://docs.rs/eframe/latest/eframe/trait.App.html)）。照搬即可，不需要自创。
4. **状态必须分层，且「权威状态」不能放 egui `Memory`**。egui 自己的文档明说 `Memory::data` "This is NOT meant to store any important data. Store that in your own structures!"（[docs.rs/egui Memory](https://docs.rs/egui/latest/egui/struct.Memory.html)）。推荐三层：**Editor/文档 store（权威、可 undo、可序列化）→ ViewState（布局/选择/折叠等会话态）→ egui Memory（纯暂态 UI 状态）**。
5. **MVC 在 Rust/egui 下会退化成「Controller 同时持有 &mut model 和 &mut UI」**，借用检查器逼你用 `Rc<RefCell<>>` 或全局 `static`，于是拿到运行期 borrow panic 和隐式耦合。Raph Levien 的原话："Architectures that work well in other languages generally don't adapt well to Rust, mostly because they rely on shared mutable state and that is not idiomatic Rust"（[博客](https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html)）。egui 的 immediate mode 本身已经把「回调的控制权」反转成「每帧调用应用代码」，所以**编辑器侧真正需要的不是 MVC 的 Controller，而是「意图 → 状态转换 → 帧末投影」的流水线**。
6. **MVU/Elm Architecture 是 Rust 里最省心的分发骨架，但要小心中年膨胀**：iced 官方自述 "Inspired by Elm"（[docs.rs/iced](https://docs.rs/iced/0.14.0/iced/)）；Raph Levien 承认 Elm 架构"不需要共享可变状态"，但批评"显式消息类型 + dispatch 很啰嗦，而且 Elm 架构对组件化支持差"（[博客](https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html)）。→ 对编辑器，**采用 MVU 的「单点 dispatch + 纯转换」思想，但不要学 iced 的整棵树重建/单一 State 关联类型**。
7. **Reactive signals 阵营（Dioxus/Leptos/Floem）适合「细粒度依赖追踪」，不适合直接套到引擎编辑器**：核心收益是省去手写 diff，代价是 `Copy signal` + 内部可变性 + 隐式依赖图，跟 Rust 所有权模型以及外部引擎数据源（每帧可能被 engine/网络线程改）耦合成本高。Dioxus 自述 "ergonomic Copy runtime for data with local subscriptions"（[docs.rs/dioxus-signals](https://docs.rs/dioxus-signals/latest/dioxus_signals/)）。
8. **Bevy 的编辑器生态是反例教材**：官方 `bevy_editor_prototypes` 已 **archived**（"Bevy is still actively working on an Editor ... Foundational work continues apace in the main repository"，[README](https://github.com/bevyengine/bevy_editor_prototypes)）；社区 `bevy_editor_pls` 最后一版 0.10.0 停在 2024-10-14（[crates.io](https://crates.io/crates/bevy_editor_pls)）；而 **`bevy-inspector-egui` 仍在活跃维护（2026-10-17 更新，[crates.io](https://crates.io/crates/bevy-inspector-egui)）**——说明「编辑器 UI 层独立于引擎 ECS UI、用 egui 做反射式属性面板」是被验证可行的路线。
9. **egui 生态的成熟基建可以直接用**：`egui_tiles`（Tile/Tree/Behavior，crate owner 含 rerunio，[docs.rs](https://docs.rs/egui_tiles/latest/egui_tiles/)）或 `egui_dock`（DockState/Surface/Tree/TabViewer，[docs.rs](https://docs.rs/egui_dock/latest/egui_dock/)）做停靠布局；`egui_kittest` 做基于 AccessKit 的 headless UI 测试 + 快照（[docs.rs](https://docs.rs/egui_kittest/latest/egui_kittest/)）；`re_data_ui` 的 `DataUi` 是「反射式属性面板」的可抄范本（[docs.rs](https://docs.rs/re_data_ui/0.41.0/re_data_ui/)）。
10. **不建议整体换框架**。iced/Slint/Xilem/GPUI 各自优秀，但一个已经基于 egui 的引擎编辑器迁移到它们意味着重写所有自绘控件、失去 egui 与引擎渲染的零成本集成；调研结论是 **保留 egui，替换其上的应用架构**（模式 1–8，见 §12）。

---

## 1. 调研方法与证据口径

- **框架版本快照**（2026-10-30）：iced 0.14.0、eframe/egui 0.37.0、Rerun 0.41.0（re_viewer / re_viewer_context / re_ui / re_data_ui）、Slint（docs.slint.dev latest）、Xilem/Masonry 0.4.0、GPUI（Zed `main` 分支源码 + zed.dev 文档）、dioxus-signals 0.7.10、leptos 0.8.22、Floem（GitHub main README）、Bevy 0.20.1。
- **优先一手**：rustdoc 签名、crate 源码（docs.rs 的 `/source/` 页面）、官方 changelog、作者博客。
- **不引用**：没有出处的二手博客结论；所有引文均为原文英文（避免转述失真），中文为解释。
- **注意版本漂移**：Rust UI 框架 API 变动极快（尤其 iced 0.13→0.14、eframe 0.37 的 `App` 重构），本文所有结论都带版本号；落地时应以当期 docs.rs 为准。
- **Rerun 的源码引用口径**：本文引用 docs.rs 的发布版 `/source/` 页（与 crates.io 发布包一致）；GitHub 仓库中这些文件的路径随版本迁移过（早期 `crates/re_viewer_context/src/`，后期在 `crates/viewer/` 下），因此不直接引用 main 分支 URL，以免路径失效。


---

## 2. 理论：从 MVC 到 MVU 的谱系

### 2.1 MVC 的起源与它当初解决的问题

MVC 由 Trygve Reenskaug 于 1979 年在 Xerox PARC 提出，原始报告 *Models–Views–Controllers* 现存于 Zenodo（[zenodo.org/records/3676092](https://zenodo.org/records/3676092)）。它要解决的是**同一份模型数据需要多个视图**（同一数据在列表/表单/图表中同时展示）时，如何避免视图与模型互相写死。

但 MVC 的经典形态有一个隐含前提：**Controller 持有一份对 Model 的引用，并直接调用 Model 的方法；View 观察 Model 的变化**。在 Smalltalk 里这靠「对象间消息 + 观察者通知」实现；在 C++/Java 桌面时代靠「指针 + 回调注册」实现。**在 Rust 里这两样都正好是难点**：共享 `&mut` 不可行，回调闭包捕获引用会撞生命周期，于是退化成 `Rc<RefCell<T>>` 或全局单例。

> **结论**：MVC 不是「错了」，而是它的实现前提（共享可变对象图 + 观察者回调）跟 Rust 的所有权模型正面冲突。若继续用 MVC 词汇描述，会不自觉地把「Controller 直接改 Model 再手动 `request_repaint`」这类隐式副作用带进来。

### 2.2 MVVM：双向绑定是它的核心卖点，也是它在 Rust 里的死穴

Microsoft 对 MVVM 的定义："The MVVM pattern helps cleanly separate an application's business and presentation logic from its user interface (UI)."（[learn.microsoft.com](https://learn.microsoft.com/en-us/dotnet/architecture/maui/mvvm)）。MVVM 的收益来自 **数据绑定 + 变更通知**：View 绑定 ViewModel 的属性，ViewModel 实现 `INotifyPropertyChanged`，框架自动刷新。它还需要 "two-way binding"，让 UI 的编辑直接回流到 ViewModel。

在 Rust + egui 里模拟这套东西的代价：
- 需要为每个属性手写 getter/setter + 变更通知（或引入宏）；
- 双向绑定 = 两处 `&mut` 同时存在 → 内部可变性 `RefCell`/`Mutex` → 运行期 borrow 冲突，编译器帮不上忙；
- egui 是 immediate mode，**根本不存在需要被通知的持久 View 对象**（见 §4），MVVM 的一半机制没有落点。

### 2.3 MVI / 单向数据流：意图（Intent）先于状态变更

MVI（Model-View-Intent）的经典论述见 Hannes Dorfmann 2016 年的 Android 文章（[hannesdorfmann.com](http://hannesdorfmann.com/android/model-view-intent/)）：把用户交互抽象成 `intent()`（产生 Action/Command）→ `model()`（唯一允许改状态的地方，返回新 model）→ `view()`（只读渲染），并强调 "we want an unidirectional data flow and immutability to establish predictable states"。

MVI 与 Redux 同源。Redux 把这条原则写成了三条铁律（[redux.js.org](https://redux.js.org/understanding/thinking-in-redux/three-principles)）：
1. **Single source of truth**——"The global state of your application is stored in an object tree within a single store."
2. **State is read-only**——"The only way to change the state is to emit an action, an object describing what happened."
3. **Changes are made with pure functions**——"Reducers are just pure functions that take the previous state and an action, and return the next state."

Redux 文档还顺带给出了对编辑器极有价值的论据：状态集中成树后，"Undo/Redo, for example - can suddenly become trivial to implement"（[同上](https://redux.js.org/understanding/thinking-in-redux/three-principles)）。

> **对 editor 的启示**：把「用户意图」（点按钮、改属性、拖节点）变成**显式的、可命名的数据**，是 undo/日志/远程协作/宏录制的共同前提。这正是全局消息队列做不到的——它只传 `Box<dyn Any>` 或字符串，丢掉了意图的全部结构。

### 2.4 Elm Architecture（Model-View-Update）：Rust UI 里最常见的骨架

Elm 官方指南对 TEA 的定义（[guide.elm-lang.org/architecture](https://guide.elm-lang.org/architecture/)）：
- **Model** — the state of your application
- **View** — a way to turn your state into HTML
- **Update** — a way to update your state based on messages

副作用不在 `update` 里直接做，而是作为数据返回给 runtime：程序 "also send Cmd and Sub values to the runtime system"（[guide.elm-lang.org/effects](https://guide.elm-lang.org/effects/)）——`Cmd` 是「要做的一次性副作用」，`Sub` 是「持续的订阅」。

iced 是 Rust 里对 TEA 最忠实的实现，官方 crate 文档第一句就是 "iced is a cross-platform GUI library focused on simplicity and type-safety. Inspired by Elm."（[docs.rs/iced](https://docs.rs/iced/0.14.0/iced/)）。

### 2.5 CQRS / 命令分发：为什么编辑器要区分「命令」与「查询」

CQRS（Command Query Responsibility Segregation）由 Greg Young 提出、Martin Fowler 记录（[martinfowler.com/bliki/CQRS.html](https://martinfowler.com/bliki/CQRS.html)）：核心是 "you can use a different model to update information than the model you use to read information"。

Fowler 同时给了重要警告：**"beware that for most systems CQRS adds risky complexity"**。对编辑器 UI 的正确取法不是完整的 CQRS（读写分离 + 事件溯源 + 最终一致），而是其中两个轻量思想：
1. **命令是显式对象/枚举**：`SetTransform { entity, pos }`、`DeleteNode { id }`，而不是 `messager.send(...)`；
2. **命令有单一入口和单一处理器**：所有命令在帧循环的一个点被消费、校验、应用、记录（Rerun 的 `SystemCommand` 就是这个形态，见 §5）。

### 2.6 Effect systems：副作用作为返回值，而不是隐式调用

Rust UI 里「effect」的落点非常具体：
- **iced**：`update` 返回 `Task<Message>`；`Task::perform(future, Message::Xxx)` 把异步结果再变成消息（[0.13 文档的 Concurrent Tasks 一节](https://docs.rs/iced/0.13.1/iced/)）；
- **Xilem**：`async_action` 把异步事件接回 view 树（[docs.rs/xilem](https://docs.rs/xilem/latest/xilem/)）；
- **GPUI**：`Context::spawn` 返回 `Task<R>`，任务在 foreground/background executor 上跑，实体通过 `WeakEntity` 避免泄漏（[Zed glossary](https://zed.dev/docs/development/glossary)、[context.rs 源码](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app/context.rs)）。

把 effect 变成数据/返回值的收益：**状态转换函数变成纯函数，副作用可 mock、可丢弃、可批量合并**。这直接决定了可测试性。

### 2.7 为什么 Rust 的 ownership + enum 让 MVU/reducer 更自然

- **enum 是最便宜的和类型**：`enum Message { AddNode(NodeId), SetValue(ItemId, f32), ... }` + `match` 让编译器强制穷举；新增一种意图，所有未处理的 `match` 会报错。这正是 iced 的 `Message` 模式（[docs.rs/iced](https://docs.rs/iced/0.14.0/iced/)）。
- **reducer 签名的借用关系干净**：`fn update(state: &mut State, msg: Message) -> Task<Message>`——只有一个 `&mut` 指向根状态，不存在「多个 Controller 同时持有子状态」的借用冲突。iced 在 0.13 就用这个签名（[0.13.1 文档](https://docs.rs/iced/0.13.1/iced/)），0.14 的 `Program` trait 把它固化为 `fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message>`（[docs.rs trait.Program](https://docs.rs/iced/0.14.0/iced/trait.Program.html)）。
- **反向证据**：Raph Levien（Xilem 作者）把这个问题讲得最透——"Architectures that work well in other languages generally don't adapt well to Rust, mostly because they rely on shared mutable state and that is not idiomatic Rust, to put it mildly."（[博客 2022](https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html)）。
- **但也有代价**：同一篇博客指出 Elm 架构 "the need to create an explicit message type and dispatch on it is verbose, and the Elm architecture does not support cleanly factored components as well as some other architectures"，并引用 Elm 文档的警告 *"actively trying to make components is a recipe for disaster in Elm."*（[同上](https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html)）。**这是给 Kairos 的关键提醒：MVU 的「单一 Message 枚举」在 10k+ LOC 后会膨胀，必须按领域切分（见 §12 模式 1/4），而不是一个大 enum 打天下。**

### 2.8 为什么纯 MVC / Drawer 风格在 egui 编辑器里会崩

把上面几条合起来，能够精确指出「全局消息总线 + MVC」这套结构的失效机制（与具体实现无关）：

| 失效机制 | 根因 | 外部佐证 |
|---|---|---|
| 回调持有 `&mut` 或 `Rc<RefCell>`，运行期 borrow panic / 泄漏 | Rust 所有权 + 回调生命周期 | egui README 把 immediate mode 的优势直接写成 "You don't have to worry about a lingering callback calling something that is gone."（[README](https://github.com/emilk/egui/blob/main/README.md)） |
| 全局队列丢意图结构，接收方只能 `match` 字符串/`Any` 后 downcast | 无类型消息 | Redux：action 必须是 "an object describing what happened"，才能被 log/serialize/replay（[redux](https://redux.js.org/understanding/thinking-in-redux/three-principles)） |
| 谁都能往总线发消息，依赖关系不可见、初始化顺序敏感 | 隐式依赖图 | GPUI 用 Entity 所有权 + 显式 `observe/subscribe/emit` 取代回调网（[官方 ownership 文档模块](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/_ownership_and_data_flow.rs)） |
| 状态被多处写、无法回答「现在到底谁是真源」 | 多写者 | Redux 的 single source of truth；Rerun 用 `StoreHub` 收敛（见 §5.3） |
| UI 与业务逻辑交织，无法 headless 测试 | Controller 里直接画 UI | Rerun 的 logic/ui 拆分 + eframe 官方 `App::logic`（见 §4.3） |
| 编辑器 undo/redo 无从下手 | 变更不可合并、不可命名 | Rerun `BlueprintUndoState` 时间线 undo（[源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/undo.rs)） |

> **一句话**：全局消息总线是「把类型系统丢掉、把依赖关系藏起来、把顺序交给运行期」的架构；它换来的松耦合在编辑器里是负资产，因为编辑器恰恰需要强顺序、可回放、可撤销。

---

## 3. iced：Rust 里最完整的 Elm/MVU 实现

### 3.1 核心 API（0.14 实测签名）

0.14 的 `Program` trait 是「一屏应用」的核心（[docs.rs/iced/0.14.0/iced/trait.Program.html](https://docs.rs/iced/0.14.0/iced/trait.Program.html)）：

```rust
pub trait Program: Sized {
    type State;
    type Message: Send + 'static;
    type Theme: Base;
    type Renderer: Renderer;
    type Executor: Executor;

    fn name() -> &'static str;
    fn settings(&self) -> Settings;
    fn window(&self) -> Option<Settings>;
    fn boot(&self) -> (Self::State, Task<Self::Message>);
    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message>;
    fn view<'a>(&self, state: &'a Self::State, window: Id) -> Element<'a, Self::Message, Self::Theme, Self::Renderer>;

    // 可选
    fn title(&self, _state: &Self::State, _window: Id) -> String { ... }
    fn subscription(&self, _state: &Self::State) -> Subscription<Self::Message> { ... }
}
```

几个容易记错的点（已核对 0.14 文档与 changelog）：
- `update` 的返回值是 **`Task<Message>`**，不是 `Command`；`Task::none()` 表示无副作用。
- `boot` 也返回 `(State, Task)`，初始化副作用与更新副作用同一套类型。
- **0.13 的用法不是 trait**：0.13.1 文档的入口是 `iced::application(title, update, view).run()` 形式的 builder；0.13.0 changelog 明确列出 "`Program` API. [#2331](https://github.com/iced-rs/iced/pull/2331)"、"`Task` API. [#2463]"、"`Daemon` API and Shell Runtime Unification. [#2469]"（[CHANGELOG](https://github.com/iced-rs/iced/blob/master/CHANGELOG.md)）。写方案时不要写「iced 0.13 的 Program trait」，准确说法是：**0.13 引入 Program/Task/Daemon API，0.14 的 `Program` 才是 trait 形态**。

### 3.2 Task / Subscription：副作用与外部事件的两条正规通路

- **`Task<Message>`**：一次性异步工作。0.13 文档示例（[docs.rs/iced 0.13.1 的 Concurrent Tasks 一节](https://docs.rs/iced/0.13.1/iced/)）：

```rust
fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::FetchWeather => Task::perform(fetch_weather(), Message::WeatherFetched),
        Message::WeatherFetched(weather) => { state.weather = Some(weather); Task::none() }
    }
}
```

- **`Subscription<Message>`**：被动事件源（定时器、键盘、窗口事件、流）。`Subscription` 是按 `State` 重新计算并 diff 的，等价于 Elm 的 `Sub`。

### 3.3 0.14 的新能力（对编辑器有直接价值）

CHANGELOG 的 0.14.0 段（[链接](https://github.com/iced-rs/iced/blob/master/CHANGELOG.md)）：
- **Reactive rendering**（#2662）——按需重绘；
- **Time travel debugging**（#2910）——框架级时间旅行，说明其状态是可快照/可回放的；
- **Animation API**（#2757）；
- **Headless mode testing**（#2698）+ **First-class end-to-end testing**（#3059）。

落地物：`iced_test` crate 提供 `Emulator`（"Run your application in a headless runtime"）、`Ice`（"A shareable, simple format of end-to-end tests"）、`Simulator`（"Run a simulation of your application without side effects"）、`screenshot()` 与 `Selector` trait（[docs.rs/iced_test](https://docs.rs/iced_test/latest/iced_test/)）。

### 3.4 对编辑器的评价

**可抄的**：
- `update(state, message) -> Task` 的**单点状态转换**与**副作用外置**；
- `Subscription` 的「按状态声明式订阅 + 自动 diff」思想（可自建简化版：每帧根据当前编辑器模式返回需要的外部事件源集合）；
- `Message` enum + 穷举 match 的强约束。

**别抄的**：
- **单一 `Message` 枚举 + 单一 `State` 关联类型**：Raph Levien 已指出 Elm 架构的组件化缺陷；一个引擎编辑器有 20+ 面板、几十种文档类型，单枚举会变成 500+ 变体的怪物；
- **整棵 view 树每帧重建**：egui 的 immediate mode 本来就更适合「每帧调用」，不需要 iced 的 retained widget tree 与 diff 成本；
- **框架自带的窗口/事件循环**：Kairos 已有自己的 wgpu/winit 集成，iced 的 runtime 无法直接嵌入。

> 结论：**借 iced 的「MVU 骨架」，不借 iced 的框架**。Rerun 没有采用任何 MVU 框架，而是用命令模式达到同样的解耦（见 §5）。

### 3.5 iced 的组件化困境（风险项）

iced 没有官方的「组件」抽象，社区实践是「把子状态 + 子消息 + 手写 map」组合起来：父 `update` 里把 `ChildMessage` 映射成父 `Message`。这在 10k LOC 后会带来大量样板。Raph Levien 对 Elm 系架构的这一条批评（[博客](https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html)）在 iced 生态里同样成立。

---

## 4. egui：框架给了什么、没给什么

### 4.1 immediate mode 的本质

egui README 的定义（[README](https://github.com/emilk/egui/blob/main/README.md)）：

> "egui is a simple, fast, and highly portable immediate mode GUI library for Rust."

immediate mode 与 retained mode 的差别（官方按钮例子）：retained GUI 里你创建按钮、加入 UI、安装 on-click handler；immediate mode 里你每帧「显示按钮并立刻拿到交互结果」，所以 "there is no need for any on-click handler, nor to store any reference to it"（[README](https://github.com/emilk/egui/blob/main/README.md)）。

对架构的直接影响：
- **没有持久的 Widget 对象**，因此没有「Widget 持有回调」的问题；
- **UI 代码就是每帧执行的普通函数**，可以直接 `&mut` 应用状态——这是 egui 最大的开发效率来源，也是最大的架构陷阱（见 §4.4）；
- egui 的 Goals 里明写 **"Pure immediate mode: no callbacks"**，以及 **"egui is not a framework. egui is a library you call into, not an environment you program for."**（[README](https://github.com/emilk/egui/blob/main/README.md)）。**这句话是整份调研的支点：egui 不提供应用架构，架构必须自己定。**

### 4.2 egui 提供了哪些「应用级」原语

| 原语 | 用途 | 证据 |
|---|---|---|
| `egui::Context` | 帧输入、`request_repaint`、共享 `Memory`、主题、字体、多 pass | [README 的 Integrations 一节](https://github.com/emilk/egui/blob/main/README.md) |
| `egui::Memory` | 跨帧暂态：窗口位置、滚动位置、CollapsingHeader 开合、TextEdit 光标 | [docs.rs/egui Memory](https://docs.rs/egui/latest/egui/struct.Memory.html) |
| `Memory::data`（`IdTypeMap`） | 按 `Id` 存 widget 级状态 | 同上 |
| `Context::request_repaint` | 外部线程/异步任务通知 UI 重绘 | [eframe App 文档](https://docs.rs/eframe/latest/eframe/trait.App.html) |
| `eframe::App` | 官方框架层：`ui` + `logic` 两段式 | 同上 |

**Memory 的正确边界**（官方原话，必须划线）：
> "The data that egui persists between frames. This includes window positions and sizes, how far the user has scrolled in a ScrollArea etc."
>
> 对 `data` 字段："This is NOT meant to store any important data. Store that in your own structures! Each read clones the data, so keep your values cheap to clone."
>
> —— [docs.rs/egui Memory](https://docs.rs/egui/latest/egui/struct.Memory.html)

翻译成架构规则：**Memory 是「UI 暂态缓存」，不是「应用状态仓库」**。把选择、文档、undo 栈塞进 Memory 是反模式（见 §13）。

### 4.3 帧循环：logic + ui 两段式

这是本次调研**最具可操作性**的一条。egui 官方 eframe 最新文档（0.37.0）里，`App` trait 已经是：

```rust
pub trait App {
    // Required method
    fn ui(&mut self, ui: &mut Ui, frame: &mut Frame);

    // Provided methods
    fn logic(&mut self, ctx: &Context, frame: &mut Frame) { ... }
    fn save(&mut self, _storage: &mut dyn Storage) { ... }
    ...
}
```

文档原文（[docs.rs/eframe](https://docs.rs/eframe/latest/eframe/trait.App.html)）：
- `ui`："Called each time the UI needs repainting, which may be many times per second."
- `logic`："Called once before each call to Self::ui, and additionally also called when the UI is hidden, but egui::Context::request_repaint was called. You may NOT show any ui or do any painting during the call to Self::logic."
- 以及 "To force a repaint, call egui::Context::request_repaint at any time (e.g. from another thread)."

> 注意：eframe 0.37 的 `App` 是 `ui` 为 required、`logic` 为 provided，和早期版本只有 `update(ctx, frame)` 不同。**初始化/升级 eframe 时必须重新对齐这一签名。**

Rerun 的 `AppCore` 几乎就是同一套（且 Rerun 的 `ui_impl` 参数里就是 `&eframe::Frame`，说明它确实跑在 eframe 上）：
- `re_viewer/src/app/logic.rs`：`/// Called before each call to ui, but ALSO when the app is hidden (occluded, minimized, …) if something has called request_repaint. We put things here that are unrelated to the UI, and that we still want to happen if the application is hidden.`（[源码](https://docs.rs/crate/re_viewer/0.41.0/source/src/app/logic.rs)）
- `re_viewer/src/app/ui.rs`：`pub(super) fn ui_impl(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame, app_blueprint: &AppBlueprint<'_>, ...)`（[源码](https://docs.rs/crate/re_viewer/0.41.0/source/src/app/ui.rs)）

**为什么对编辑器重要**：引擎编辑器里大量工作（资源加载进度、文件监听、后台编译、网络同步、动画预览）**必须在窗口最小化时也推进**；把它们塞进 `ui()` 会导致「窗口隐藏 = 任务停摆」。logic/ui 拆分同时带来可测试性：logic 段不碰 `egui::Ui`，可以在 headless 下直接跑。

### 4.4 隐式状态：egui 让「直接在 UI 里改状态」变得太容易

egui 示例的写法是：

```rust
if ui.button("Increment").clicked() { age += 1; }
```

这在 demo 里是优点，在 10k+ LOC 编辑器里是三种事故的来源：
1. **业务逻辑散落在绘制代码里**，无法测试、无法复用（菜单项、快捷键、命令面板需要同一套动作）；
2. **没有 intent 记录**，undo/宏/协作无从下手；
3. **借用冲突**：面板 A 需要 `&mut Document`，面板 B 也需要；同一帧里只能拆成多次 `ui.data_mut` 或提前 clone。

**官方自己给的答案就是「把重要状态放到你自己的结构里」**（§4.2 的 Memory 引文），所以架构设计要主动加一层「意图缓冲」：
`UI 只产生 Command/Vec<Command>，帧末统一 apply`。这也正好是 Rerun 的 `SystemCommand` 模式。

### 4.5 egui 的测试设施

`egui_kittest`（egui 官方仓库下的 crate，0.37 与 egui 同版本）：
- 基于 AccessKit 的 UI 测试库：`Harness` 可以 `get_by_label` 找到控件、`click()`、`run()` 推进帧；
- 支持 `harness.snapshot("readme_example")` 图像快照测试；
- 每次 `Harness::run` 后自动检查「每个输入控件都有 accessible name」，对编辑器这种按钮极多的 UI 是白送的回归网。
- 证据：[docs.rs/egui_kittest](https://docs.rs/egui_kittest/latest/egui_kittest/)。

---

## 5. 最接近的同类：Rerun re_viewer（大规模 egui 应用深拆）

> Rerun Viewer 是「用 egui 写的大型专业编辑器」的最强存在证明。下面全部基于 0.41.0 的 docs.rs 源码页与 rustdoc，可逐条复核。

### 5.1 Crate 分层：关注点物理隔离

| crate | 职责 | 证据 |
|---|---|---|
| `re_viewer` | 应用外壳：`App`/`AppCore`、帧循环、命令处理、保存/截图、URL 历史 | [docs.rs/crate/re_viewer 源码目录](https://docs.rs/crate/re_viewer/0.41.0/source/src/) |
| `re_viewer_context` | 共享上下文与「系统状态」：`SystemCommand`、`CommandSender`、`StoreHub`、`ApplicationSelectionState`、`ViewerContext`、undo | [docs.rs/re_viewer_context](https://docs.rs/re_viewer_context/0.41.0/re_viewer_context/) |
| `re_ui` | 设计系统：`DesignTokens`、icons、list item、menu、modal、`CommandPalette`、`ControlRow`、时间控件、测试工具 | [docs.rs/re_ui](https://docs.rs/re_ui/0.41.0/re_ui/) |
| `re_data_ui` | 反射式属性 UI：`DataUi` / `EntityDataUi` / `AppUi` + 组件 UI 注册表 | [docs.rs/re_data_ui](https://docs.rs/re_data_ui/0.41.0/re_data_ui/) |
| `re_viewport_blueprint` | 视口（多视图布局）的 blueprint 模型 | `re_viewer` 的 `app_state.rs` 中 `use re_viewport_blueprint::ViewportBlueprint;`（[源码](https://docs.rs/crate/re_viewer/0.41.0/source/src/app_state.rs)） |

> 对 Kairos 的直接映射：**kairos_editor 内部也应该有 `editor_ui（设计系统）` / `editor_context（共享状态与命令）` / `data_ui（属性反射）` / `app（外壳与帧循环）` 的 crate 级切分**，而不是全在一个 crate 里互相引用。

### 5.2 命令系统：`SystemCommand` + `SystemCommandSender`

`re_viewer_context/src/command_sender.rs` 定义（[源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/command_sender.rs)）：

```rust
/// Commands used by internal system components
#[derive(strum_macros::IntoStaticStr)]
pub enum SystemCommand {
    ActivateApp(re_log_types::ApplicationId),
    CloseApp(re_log_types::ApplicationId),
    LoadDataSource(LogDataSource),
    ...
}
```

- 0.41.0 的 rustdoc 列出 **43 个 variant**（[enum.SystemCommand](https://docs.rs/re_viewer_context/0.41.0/re_viewer_context/enum.SystemCommand.html)），覆盖：加载/关闭数据源、增删 Redap server、打开设置、截图、设置路由/焦点/选择、时间控制、blueprint 的 undo/redo、通知等；
- 发送侧是一个 trait，而不是全局函数：

```rust
pub trait SystemCommandSender {
    fn send_system(&self, command: SystemCommand);
}
impl SystemCommandSender for CommandSender { ... }
```

见 [trait.SystemCommandSender](https://docs.rs/re_viewer_context/0.41.0/re_viewer_context/trait.SystemCommandSender.html)。

- 通道本体在 `command_sender.rs` 里成对出现：`CommandSender` / `CommandReceiver` / `command_channel()` / `SystemCommandSender`（在 `re_viewer_context` 的 re-export 列表中可见，[lib.rs 源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/lib.rs)）；
- 命令的消费点在 app 层：`re_viewer/src/app/command_handling.rs`（[源码](https://docs.rs/crate/re_viewer/0.41.0/source/src/app/command_handling.rs)，文件内 `use re_viewer_context::{RecordingOrLocalTable, SystemCommandSender as _, TimeControlCommand, ...};`）。

**可迁移的关键设计**（比「有个 enum」更重要）：
1. **发送方只依赖 trait（`SystemCommandSender`），不依赖具体通道**：任何面板/系统都能发出命令，但不需要知道 App 的存在。这让「面板 → 命令」是单向依赖，可单测；
2. **命令是「系统级」的**：命名就叫 `SystemCommand`，注释写 "Commands used by internal system components"——**把 UI 局部状态改动（面板开合、输入框内容）和系统级动作严格分开**。Kairos 设计里应显式区分：`UiCommand`（纯 UI/会话态）/ `EditorCommand`（文档编辑、可 undo）/ `SystemCommand`（打开项目、保存、退出）；
3. **命令在帧循环里统一消费**，而不是 getter/setter 满天飞——这给了 undo、日志、命令面板、快捷键统一入口。

### 5.3 状态分层：`StoreHub`（权威数据）vs Blueprint（编辑器配置）

- `StoreHub`（`re_viewer_context/src/store_hub.rs`）聚合 `EntityDb`、`StoreBundle`、`BlueprintUndoState`、`TimeControl`、`ViewClassRegistry` 等（[源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/store_hub.rs) 的 use 列表 / docs 页 `pub use self::store_hub::StoreHub;`）。
- 编辑器侧「配置」被放进 **blueprint**：Rerun 让 blueprint 拥有**独立的数据存储与独立的时间线**（`blueprint_timeline()`、`LatestAtQuery::latest(...)`，见 [undo.rs 源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/undo.rs)）。也就是说：**「数据是什么」和「这次用哪个视图、什么时间轴、哪个选择」是两个 store**。
- `ViewerContext` 的文档甚至写了使用纪律："Never use [`ViewerContext`] where [`StoreViewContext`] would suffice."（[源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/viewer_context.rs)）——用类型系统/文档约束「谁能看到多少上下文」。

**关于 StoreSubscriber（任务简报点名的旧机制）**：旧版 Rerun（如 rerun 0.11 / re_viewer 0.16）确实有 `StoreSubscriber` trait（含 `on_events` 回调，[docs.rs/rerun 0.11](https://docs.rs/rerun/0.11.0-rc.2/rerun/trait.StoreSubscriber.html)），并有 PR 将其从 `StoreView` 更名为 `StoreSubscriber`（[PR #4234](https://github.com/rerun-io/rerun/pull/4234)）。但 0.41 的 `re_viewer_context` 中该 trait 已不在公开 API（docs.rs 返回 404），当前由 `StoreViewContext` / `QueryContext` / `ViewContext` 承担「视图只读访问 store 数据」的职责——**说明该机制历经多轮改名；若按旧版资料设计，需要重新对齐 0.41 的类型名**。

**对 Kairos 的启示**：引擎里「场景/资源/ECS 世界」是权威数据，编辑器的**视图配置（布局、可见性、gizmo 模式、选择、时间轴）**应是第二个 store（可序列化、可 undo、可 profile 化），不要把视图配置塞进 ECS 组件或 `ui.data`。

### 5.4 选择状态：`ApplicationSelectionState`

`re_viewer_context/src/selection_state.rs`（[源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/selection_state.rs)）：

```rust
#[derive(Default)]
pub struct ApplicationSelectionState {
    /// The selected items. Write to this with [`crate::SystemCommand::set_selection`].
    selection: ItemCollection,
    /// Has selection changed since the previous frame?
    selection_changed: Option<SelectionSource>,
    ...
}
```

配套：
- 每个视图的 `ViewContext` 里带 `selection_state`；选择是 `ItemCollection`（一组 `Item`，可包含 entity/component/data-result 等不同类型，见 rustdoc 的 `Item`/`ItemCollection`/`ItemContext` 页面，[re_viewer_context 0.41 索引](https://docs.rs/re_viewer_context/0.41.0/re_viewer_context/)）；
- 有 `on_frame_start` 钩子用于**解析失效项**：文档注释 "resolve_item decides the fate of each currently selected item: returning None drops it, while returning Some(item) keeps it — possibly replacing it with a different item (e.g. downgrading a no-longer-valid data result to a plain entity selection)."（[源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/selection_state.rs)）；
- 提供 `SelectionChange` / `SelectionHighlight` / `HoverHighlight` 等类型。

**可抄的**：编辑器全局选择状态应该是**单一对象 + 每帧开头做有效性校验（entity 被删除后自动剔除）**，而不是每个面板各自维护 `selected_entity: Option<Entity>`。后者在实体被删除/重载后必然出现「悬空选择」。

### 5.5 Undo：时间线式，而不是简单的命令栈

`re_viewer_context/src/undo.rs`（[源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/undo.rs)）的关键注释：

> "We store the entire edit history of a blueprint in its store.
> When undoing, we move back time, and redoing move it forward.
> When editing, we first drop all data after the current time."

实现细节（同样是可抄的设计决策）：
- `MAX_UNDOS: usize = 100`；
- `EXTERNAL_EDIT_SETTLE_TIME: Duration = Duration::from_millis(250)`：外部编辑（JS/gRPC）快速连续发生时合并成一个 undo 点；
- 交互式拖拽（slider）通过启发式合并：注释写 "When the user drags a slider or similar, we get new events recorded on each frame. The user presumably wants to undo the whole slider drag, and not each increment of it. So we use a heuristic to estimate when such interactions start/stop, and add them to this set."；
- 外部编辑入口 `on_external_blueprint_edit(egui_ctx)` 会 `request_repaint()`，并明确说 "egui can't see that the user is interacting"（外部系统改数据时，egui 需要被显式唤醒）。

**对 Kairos 的启示**：引擎编辑器的 undo 必须处理「连续拖拽合并成一个 undo 点」「外部（脚本/网络/引擎）修改也要进 history」——这两条在纯命令栈里很难做对，需要 undo 单元定义 + 合并策略。

### 5.6 反射式属性 UI：`re_data_ui`

`re_data_ui` 0.41 的 traits（[docs.rs](https://docs.rs/re_data_ui/0.41.0/re_data_ui/)）：
- `AppUi`："Types implementing AppUi can display themselves in an egui::Ui using only an AppContext."；
- `DataUi`："Types implementing DataUi can display themselves in an egui::Ui."；
- `EntityDataUi`："Similar to DataUi, but for data that is related to an entity (e.g. a component)."；
- 函数 `add_to_registry` / `register_component_uis`——**按类型注册 UI**，新组件类型接入属性面板不需要改面板代码。

**对 Kairos 的启示**：属性面板不要写 `if type == Transform { ... } else if type == Material { ... }` 的巨型 match；做「类型 id → 编辑 UI 函数」的注册表，支持内置类型 + 引擎侧/插件侧注册，同时保留「谁可以编辑」（只读/可写/多选混合）的信息。

### 5.7 设计系统：`re_ui`

crate 文档第一句：**"Rerun GUI theme and helpers, built around egui."**（[docs.rs/re_ui](https://docs.rs/re_ui/0.41.0/re_ui/)）。可见的模块/类型覆盖：icons、design tokens（`DesignTokens`、`try_set_design_tokens`）、list item（第二代 `ListItem`）、menu/modal/notifications、`re_form`、syntax highlighting、时间控件（`TimeDragValue`、`TabBar`），以及 **命令面板基础设施**：`CommandPalette`、`CommandPaletteProvider`、`CmdRow`、`CommandEnvironment`、`CommandKind`、`CommandScope`、`BoundCommand`（[docs.rs/re_ui](https://docs.rs/re_ui/0.41.0/re_ui/)）。

> 注意 `re_ui` 里还 还提供了 `RecordingCommand` / `UICommand`（[command_sender.rs 源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/command_sender.rs) 的 `use re_ui::{RecordingCommand, RecordingCommandSender, UICommand, UICommandSender};`）——**说明 Rerun 的命令不是单层，而是按作用域分了层**（system / UI / recording）。这直接回答了「一个 enum 会不会爆炸」：会，所以要分层。

### 5.8 帧循环与 App 结构（汇总）

- `re_viewer/src/lib.rs`：`pub use app::{App, AppCore};`、`pub(crate) use app_state::AppState;`（[源码](https://docs.rs/crate/re_viewer/0.41.0/source/src/lib.rs)）；
- `re_viewer/src/app_state.rs`：`AppState` 的 use 列表几乎就是「一个编辑器需要哪些状态」的清单——`AppContext`、`ApplicationSelectionState`、`BlueprintUndoState`、`CommandSender`、`DragAndDropManager`、`FocusTarget`、`Item/ItemCollection`、`Route`、`SelectionChange`、`StorageContext`、`StoreHub`、`TimeControl`、`ViewClassRegistry`、`ViewStates`、`ViewerContext`、`ViewportUi`（[源码](https://docs.rs/crate/re_viewer/0.41.0/source/src/app_state.rs)）；
- `AppState` 本身带 `#[derive(serde::Deserialize, serde::Serialize)]`（注释 `// TODO(#11737): Remove the serde derives since almost everything is skipped.`）——**会话状态整体可序列化**（窗口布局、路由等）；
- 帧循环：`logic_impl()`（含 purge memory、gc blueprints、下载/ingest 数据）→ `ui_impl()`（绘制）；两者都能从源码页读到（[logic.rs](https://docs.rs/crate/re_viewer/0.41.0/source/src/app/logic.rs)、[ui.rs](https://docs.rs/crate/re_viewer/0.41.0/source/src/app/ui.rs)）；
- `ui_impl` 里还有一句重要注释："In some (rare) circumstances we run two egui passes in a single frame. This happens on call to `egui::Context::request_discard`."——**不要假设「一帧 = 一次 ui pass」**，状态变更点要幂等（[ui.rs](https://docs.rs/crate/re_viewer/0.41.0/source/src/app/ui.rs)）。

---

## 6. Slint：声明式 + 响应式属性，是「另一个世界」

### 6.1 语言模型

- 定位："Slint is an open-source declarative GUI toolkit to build native user interfaces for Rust, C++, JavaScript, or Python apps."（[GitHub](https://github.com/slint-ui/slint)）
- **属性（property）**：`in` / `out` / `in-out` / `private`，可以带绑定表达式；读取属性得到当前值（[Properties 文档](https://docs.slint.dev/latest/docs/slint/reference/language/properties/)）。
- **回调（callback）**：`callback hello;` 声明，调用即触发，用 `=>` 绑定 handler。原文："A callback is a member that carries an invocation from one place to another. It's declared with the callback keyword, invoked by calling it, and reacted to by a handler set with the => arrow."（[Callbacks 文档](https://docs.slint.dev/latest/docs/slint/reference/language/callbacks/)）
- **全局单例（global）**："A global is a singleton declared at the top level of a .slint file with the global keyword. It holds properties, callbacks, and functions that are shared across the whole project rather than belonging to an element."（[Globals 文档](https://docs.slint.dev/latest/docs/slint/reference/language/globals/)）

### 6.2 Rust 侧生成代码

`.slint` 编译成 Rust struct，通过 `ComponentHandle` 访问（[Slint 的 Rust 文档](https://docs.slint.dev/latest/docs/rust/slint/)）：
- 每个顶层 property：`set_<name>()` / `get_<name>()`；
- 每个顶层 callback：`invoke_<name>()` / `on_<name>(impl Fn(<Args>) + 'static)`；
- 每个 global：`global<T: Global<Self>>()`；
- 生命周期纪律（官方原文）："A strong reference should not be captured by the closures given to a callback, as this would produce a reference loop and leak the component. Instead, the callback function should capture a weak component."（同上）

### 6.3 评价：适不适合引擎编辑器？

**适合**：工具类/设置类/启动器类的声明式 UI；需要 designer 与代码分离的团队；嵌入式/低资源目标。

**不适合 Kairos 的核心编辑器**：
- 引擎编辑器 ≈ 大量自绘控件（视口、gizmo、曲线、时间轴、图表、材质预览），这些必须用 GPU 坐标/自定义绘制，Slint 的声明式控件模型帮不上；
- 迁移成本 = 重写全部界面 + 引入 `.slint` 编译步骤 + 与 egui 生态（egui_tiles/kittest）割裂；
- 回调模型仍然是「handler 注册」，正是本文建议摆脱的形态（虽然 Slint 的 property binding 比手写回调安全）。

**可借的一点**：`in/out/in-out` 属性语义与「global 单例配置」对「编辑器设置项」设计有参考价值；`slint-snapshot` crate 提供截图测试（[docs.rs/slint-snapshot](https://docs.rs/slint-snapshot/latest/slint_snapshot/)）。

---

## 7. Xilem / Masonry：view 树 + Widget trait + action + lens

### 7.1 Masonry 是「不预设应用架构」的 widget toolkit

Masonry 文档（[docs.rs/masonry](https://docs.rs/masonry/latest/masonry/)）关键句：

> "Masonry gives you a platform-independent manager, which owns and maintains a widget tree. It also gives you tools to inspect that widget tree at runtime, write unit tests on it, and generally have an easier time debugging and maintaining your app."
>
> "The framework is not opinionated about what your user-facing abstraction will be: you can implement immediate-mode GUI, the Elm architecture, functional reactive GUI, etc., on top of Masonry."

Masonry `Widget` trait（[trait.Widget](https://docs.rs/masonry/latest/masonry/core/trait.Widget.html)）的关键成员：
- `type Action: Any + Debug`；
- `fn on_pointer_event(...)` / `on_text_event(...)` / `on_access_event(...)`；
- `fn property_changed(...)`；
- `fn layout(...)` / `fn paint(...)`；
- `fn with_props(...)` / `with_id(...)`。

**注意它的 action 模型**：widget 内部把自己产生的 action 通过 `Action` 关联类型上抛，由上层 AppDriver 处理——**这跟 Rerun 的 `SystemCommand` 是同构的**：子组件只描述「发生了什么」，不直接改全局状态。

### 7.2 Xilem 是 Masonry 之上的 reactive 层

Xilem crate 文档（[docs.rs/xilem](https://docs.rs/xilem/latest/xilem/)）的 counter 示例：

```rust
#[derive(Default)]
struct Counter { num: i32 }

fn app_logic(data: &mut Counter) -> impl WidgetView<Counter> + use<> {
    flex(Axis::Vertical, (
        label(format!("{}", data.num)),
        text_button("increment", |data: &mut Counter| data.num += 1),
    ))
}

fn main() -> Result<(), EventLoopError> {
    let app = Xilem::new_simple(Counter::default(), app_logic, WindowOptions::new("Counter app"));
    app.run_in(EventLoop::with_user_event())?;
    Ok(())
}
```

要点：
- **应用状态是任意 `'static` Rust 类型**；根组件 `app_logic(&mut State) -> impl WidgetView<State>` 每帧生成 view 值，框架把 view 映射到 Masonry widgets 并做增量更新；
- **lens**：`lens` adapter 用于「把组件的状态缩小到父状态的某个字段」；**memoize** 用于跳过未变化子树（[docs.rs/xilem](https://docs.rs/xilem/latest/xilem/)）；
- `async_action` 用于异步事件回传；
- 官方自述 "Xilem can currently be considered to be in an alpha state."（同上）。

### 7.3 对 Kairos 的价值

- **「子视图只拿自己那部分状态」**（lens）是解决「面板需要 &mut 全局数据」的正解，可以借其思想做 **ViewState 的分片借用**（例如把 `EditorState` 拆成若干可独立借用的 `Store`，每个面板只 `&mut` 自己需要的 store）；
- **memoize/key 思想**可借鉴到 egui：给昂贵的面板加 `ui.id_salt`/缓存；
- 但 Xilem 本身**不适合直接用作引擎编辑器**：alpha、控件生态小、自绘能力与 egui 的 `Painter` 相比不成熟；且它与 Masonry 的 retained 树绑定，无法嵌入 Kairos 现有渲染循环。

---

## 8. Zed GPUI：Entity/Context + 事件 + async task

### 8.1 所有权模型

Zed 官方 glossary（[zed.dev/docs/development/glossary](https://zed.dev/docs/development/glossary)）：
- **App**："A singleton that holds full application state, including all entities. App is not Send, so it exists only on the thread that created it (usually the main/UI thread)."
- **Context**："A wrapper around App with specialized behavior for a specific Entity. You can think of it as (&mut App, Entity<V>)."
- **AsyncApp**："An owned version of App for async contexts… still not Send."
- **Task / Executor**："A future running (or scheduled to run) on a background or foreground executor. Unlike regular futures, tasks do not need .await to start."

官方 `_ownership_and_data_flow.rs` 文档模块把这套模型讲得更细（[源码](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/_ownership_and_data_flow.rs)）：
- 每个 model/view（统称 entity）由 `App` 拥有；`Entity<T>` 只是「id + 类型标签 + 引用计数句柄」，**不拥有状态**，要访问状态必须同时持有 `&App`；
- 状态变更通过 `entity.update(cx, |state, cx| { ... })`；变更后调用 `cx.notify()` 通知观察者；
- 观察者用 `observe`，事件用 `subscribe` + `cx.emit(Event)`（entity 需要实现 `EventEmitter<Event>`）。

### 8.2 `cx.listener`：把「事件回调」适配回「持有 Entity 状态」

GPUI 的 UI 事件回调签名通常是 `Fn(&E, &mut Window, &mut App)`，拿不到具体 View 的状态；`Context::listener` 是官方适配器（[context.rs 源码](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app/context.rs)）：

```rust
#[inline(always)]
pub fn listener<E: ?Sized>(
    &self,
    listener: impl Fn(&mut T, &E, &mut Window, &mut Context<T>) + 'static,
) -> impl Fn(&E, &mut Window, &mut App) + 'static {
    let view = self.entity().downgrade();
    move |event: &E, window: &mut Window, cx: &mut App| {
        invoke_listener(&view, window, cx, &|view, window, cx| {
            listener(view, event, window, cx);
        });
    }
}
```

它内部拿的是 `WeakEntity<T>`，回调触发时再 upgrade——**避开了「闭包捕获强引用导致泄漏」与「回调在实体销毁后仍被调用」两大经典问题**。

### 8.3 事件是 effect，不是同步调用

`Context::emit` 的实现（[context.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app/context.rs)）：

```rust
pub fn emit<Evt>(&mut self, event: Evt)
where T: EventEmitter<Evt>, Evt: 'static,
{
    let event = self.event_arena.alloc(|| event).map(|it| it as &mut dyn Any);
    self.app.pending_effects.push_back(Effect::Emit {
        emitter: self.entity_state.entity_id,
        event_type: TypeId::of::<Evt>(),
        event,
    });
}
```

即 `emit` 先把事件压入 `pending_effects`，在本轮 effect cycle 结束时统一分发；`App::defer` 的文档也明说 "Schedules the given function to be run at the end of the current effect cycle"（[app.rs 源码](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app.rs)）。**这是「事件驱动但保持单向、可批处理」的工程实现**，对 egui 编辑器同样适用：UI pass 里只收集命令，帧末统一 apply。

### 8.4 async 模型

- `Context::spawn` 返回 `Task<R>`，闭包收到 `WeakEntity<T>` + `&mut AsyncApp`；`App::spawn` 类似，收到 `&mut AsyncApp`；另有 `spawn_in(window, ...)` 拿 `AsyncWindowContext`（[context.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app/context.rs)、[app.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app.rs)）；
- 任务在 foreground/background executor 上跑；`Task` "do not need .await to start"，需要结果时才 await（[glossary](https://zed.dev/docs/development/glossary)）；
- 测试：GPUI 有 `#[cfg(any(test, feature = "test-support"))] pub mod test` 与 `gpui_macros::test`（[gpui.rs 源码](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/gpui.rs)）。

### 8.5 对 Kairos 的价值

GPUI 本身不可用（它自成渲染/平台栈，Zed 自用，且非 egui）。但它的三个设计可直接搬：
1. **`Context::listener` 式适配器**：egui 里对应「把 `&mut EditorState` 的字段借用包装成可传给按钮闭包的弱句柄」；egui 有 `ui.data_mut` + `Id` 可做类似事，但更推荐显式命令（§12 模式 1）；
2. **`emit` 延迟到 effect cycle 末尾**：避免 UI pass 中改动正在被遍历的状态；
3. **`Entity` + 类型化事件**：编辑器里「文档保存完成」「导入进度」等跨面板通知可以做成类型化事件 + 订阅，而不是字符串消息。

---

## 9. Reactive signals 阵营：Dioxus / Leptos / Floem

### 9.1 Dioxus signals

- `dioxus-signals` 自述："Dioxus Signals is an ergonomic Copy runtime for data with local subscriptions."（[docs.rs/dioxus-signals](https://docs.rs/dioxus-signals/latest/dioxus_signals/)）
- **所有 signal 都实现 Copy**，"even if the inner value does not implement copy"，方便移进 future/children；
- **local subscriptions**："Signals will only subscribe to components when you read from the signal in that component."——依赖追踪是运行期自动完成的；
- 提供 `use_signal`、computed/memo、`spawn` 等（同上）。

**评价**：对「面板间共享少量状态」非常顺手；但依赖图是隐式的，**对引擎编辑器最难受的是「每帧世界可能被外部改」**——signals 的自动订阅假定「写都经过 signal」，而引擎侧（物理步进、渲染线程、导入线程）的写入不走 signal，需要手动 bridge/失效，容易漏更新。可用于工具窗口/设置面板，不建议作为整个编辑器的状态骨架。

### 9.2 Leptos

- 0.8 的文档把核心概念列得很清楚：`RwSignal` / `ReadSignal` / `WriteSignal`、`Memo`、`Effect` / `RenderEffect`、`Resource`（异步加载）、`Action`（异步变更）、`reactive_graph`（[docs.rs/leptos](https://docs.rs/leptos/latest/leptos/)）；
- 定位是 **full-stack web 框架**（CSR/SSR/islands），与桌面引擎编辑器场景不重合。可借 `Resource/Action` 对「异步数据加载/变更」的类型化表达。

### 9.3 Floem

- README："A native Rust UI library with fine-grained reactivity and great ergonomics."（[GitHub](https://github.com/lapce/floem/blob/main/README.md)）
- 典型写法：`let mut counter = RwSignal::new(0);` + `Button::new("Increment").action(move || counter += 1)` + `Label::derived(move || format!("Value: {counter}"))` + `floem::launch(counter_view)`（同上）；
- `Label::derived` 体现 fine-grained reactivity：标签只在其依赖的 signal 变化时更新。

**评价**：Lapce 出品，思路是「Rust 原生 + 细粒度响应式」，适合编辑器类应用（Lapce 就是代码编辑器）。但它自成一套 view/widget 体系，无法嵌入 egui；**可借其「派生视图（derived view）+ 细粒度失效」思想优化 egui 的昂贵面板**，但不要整体迁移。

---

## 10. Bevy：ECS 驱动 UI、observer，以及编辑器生态的教训

### 10.1 Bevy 的观察者系统（push-based）

`bevy::ecs::observer` 模块文档（[docs.rs/bevy 0.20.1](https://docs.rs/bevy/latest/bevy/ecs/observer/index.html)）：

> "Observers are a push-based tool for responding to Events. The Observer component holds a System that runs whenever a matching Event is triggered."

- 实体可挂 `Observer` 组件；事件触发时运行对应 system；
- 观察者 system 通过 `On` 系统参数拿到事件（例如 `On<Add, T>` 一类泛型形态，具体以当期文档为准），另有 `TriggerContext`、`ObservedBy`、`ObserverDescriptor` 等类型（同上）；
- 这是「push 通知」而非「轮询」，适合「组件变化 → UI 刷新」这类反应式需求。

### 10.2 Bevy UI 的本质

`bevy_ui` 文档（[docs.rs/bevy/ui](https://docs.rs/bevy/latest/bevy/ui/)）："This UI is laid out with the Flexbox and CSS Grid layout models." UI 节点是 ECS 实体（`Node` 组件），布局由 ECS 中的 UI 布局系统计算，交互由 `Interaction` 等组件表达。

**含义**：Bevy UI 默认把 UI 状态放进 ECS 世界，用 system 驱动。它对「游戏内 HUD」很合适，但对「引擎编辑器」问题很大：
- 编辑器 UI 的「权威状态」在 ECS 里，undo/序列化/多窗口/布局持久化都要绕 ECS；
- UI 布局每帧重算，文本/输入法/拖拽等编辑器高频交互在 ECS 体系里实现成本高。

### 10.3 编辑器生态的教训（重要反例）

| 项目 | 状态 | 证据 |
|---|---|---|
| `bevy_editor_prototypes`（官方） | **已归档（archived）**；官方称编辑器继续在 bevy 主仓推进（.bsn asset、UI、camera controllers），社区实验转向 jackdaw | [README](https://github.com/bevyengine/bevy_editor_prototypes) |
| `bevy_editor_pls`（社区） | 最后一版 0.10.0，**2024-10-14** 之后再无发布（截至 2026-10-30） | [crates.io](https://crates.io/crates/bevy_editor_pls) |
| `bevy-inspector-egui`（社区） | **仍在维护**，2026-10-17 有更新；用 egui 做反射式 inspector | [crates.io](https://crates.io/crates/bevy-inspector-egui) |

**结论**：Bevy 官方编辑器路线反复调整，但「**编辑器 UI 用 egui 独立实现、只把 ECS 当被编辑的数据源**」这条社区路线（bevy-inspector-egui / jackdaw）反而是活得最久的。这从反面支持 Kairos 的选择：**保留 egui，把编辑器架构做对**，而不是把编辑器塞进引擎的 UI 体系。

---

## 11. 横向对比表

> 评分口径：**10k+ LOC 引擎编辑器**（多面板、文档模型、undo、命令面板、自绘视口、异步资源）。★=弱，★★★★★=强。均为定性判断，依据见各节证据。

| 框架 / 架构 | intent dispatch（意图分发） | state ownership（状态归属） | async / effects | testability | 10k+ LOC 编辑器适配 |
|---|---|---|---|---|---|
| **Rerun re_viewer（egui 应用架构）** | `SystemCommand` enum（43 variants）+ `SystemCommandSender::send_system` + 分层命令（UI/Recording/System） | `StoreHub`（数据 store）+ blueprint（编辑器配置 store）+ `AppState`（会话态，可 serde） | eframe 帧循环；异步任务 + `request_repaint` 唤醒；logic 段跑在隐藏时 | egui_kittest（同生态）；logic/ui 拆分后 logic 可单测 | ★★★★★（存在证明） |
| **egui + eframe（仅框架）** | 无（自己定） | 无（自己定；Memory 仅暂态） | `request_repaint` + 外部线程；无内置 task 系统 | `egui_kittest`（AccessKit + 快照 + 可访问性检查） | ★★（框架不提供架构，需自建） |
| **iced 0.14** | `Message` enum + `Program::update` | 单一 `Program::State` 关联类型；无组件级状态 | `Task<Message>` + `Subscription<Message>`（一等公民） | `iced_test`：Emulator / Ice e2e / Simulator / screenshot | ★★★（MVU 标杆，但单 State/单 Message 扩展性差） |
| **Slint** | callback（`on_*` / `invoke_*`）+ property binding | 声明式组件树 + global 单例；Rust 侧持 `ComponentHandle` | `slint::spawn_local` / event loop | `slint-snapshot` 截图；生成的 handle 可单测逻辑 | ★★（声明式工具 UI 好；自绘编辑器控件弱） |
| **Xilem / Masonry** | Widget `Action` 上抛 + `app_logic(&mut State)` | 任意 `'static` State；Masonry 集中管理 widget tree | `async_action`；tokio 集成 | `masonry_testing` crate；widget tree 可在运行期检查 | ★★（alpha；控件生态小） |
| **Zed GPUI** | `cx.emit(Event)` + `observe/subscribe`；listener 适配器 | `App` 拥有所有 `Entity`；`Entity<T>` 句柄 | `Task` + foreground/background executor + `spawn` | `gpui::test` 宏 + test-support 模块 | ★★★（模型优秀，但绑定 Zed 自身渲染/平台栈） |
| **Dioxus signals** | 事件 handler + signal 写入 | 组件树 + `Signal<T>`（Copy + 内部可变） | `spawn` / futures；无内建 executor 概念 | 组件测试依赖其虚拟 DOM/渲染器生态 | ★★（隐式依赖图与外部世界写入不匹配） |
| **Leptos** | 事件 + signal/action | 组件树 + `RwSignal`；`reactive_graph` | `Resource` / `Action` | Web 测试栈为主 | ★（Web 全栈框架，不适用桌面引擎） |
| **Floem** | 事件 handler + `RwSignal` 写入 | view 树 + signal | 有限 | Lapce 实战验证；测试设施一般 | ★★（自成体系，无法嵌入 egui） |
| **Bevy UI + observer** | `Event` + `Observer`（push） | ECS World（唯一真源） | ECS system / 异步 asset | ECS 测试容易；UI 测试难 | ★★（引擎内 HUD 合适，编辑器不合适） |

---

## 12. 可迁移到 egui 编辑器的 8 个模式

每个模式给 **What / Why / When-not / 证据**。落地优先级按 1→8。

### 模式 1：类型化命令 + 单点分发（SystemCommand 模式）

- **What**：定义 `enum EditorCommand { ... }`（或分层 `UiCommand / EditorCommand / SystemCommand`），UI 只 `sender.send(cmd)`；帧循环（或 logic 段）单点 drain、校验、apply。
- **Why**：可穷举 match、可日志/回放、可 undo、可接命令面板/快捷键/宏；替代无类型全局消息队列。Rerun 的 `SystemCommand` 是 43 变体的成熟样本（[enum.SystemCommand](https://docs.rs/re_viewer_context/0.41.0/re_viewer_context/enum.SystemCommand.html)）；iced 的 `Message` enum 是同一思想的框架内实现（[docs.rs/iced](https://docs.rs/iced/0.14.0/iced/)）。
- **When-not**：命令类型 < 20 个、且没有 undo/测试需求的小工具可以省掉这层；**一旦需要 undo/命令面板，就必须有**。
- **关键细节**：发送者依赖 trait `CommandSender`（如 `SystemCommandSender::send_system`，[trait 文档](https://docs.rs/re_viewer_context/0.41.0/re_viewer_context/trait.SystemCommandSender.html)）而不是具体 App，避免循环依赖。

### 模式 2：logic / ui 两段式帧循环

- **What**：每帧先跑 `logic()`（不碰 `egui::Ui`：drain 命令、推进异步、同步引擎状态、计算派生数据），再跑 `ui()`（纯绘制 + 收集新命令）。
- **Why**：窗口隐藏时任务仍推进；logic 可 headless 测试；绘制代码不含业务逻辑。Rerun `logic_impl` 注释（[源码](https://docs.rs/crate/re_viewer/0.41.0/source/src/app/logic.rs)）+ eframe 0.37 `App` trait（[docs.rs/eframe](https://docs.rs/eframe/latest/eframe/trait.App.html)）。
- **When-not**：极小工具不必拆分；一旦引入后台资源加载/网络/编译就必须拆。
- **关键细节**：两段之间用「命令队列 + 脏标记」衔接；`request_repaint` 放在状态确实变化的地方（参考 Rerun `on_external_blueprint_edit` 的 `request_repaint` 用法，[undo.rs](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/undo.rs)）。

### 模式 3：三层状态分离（权威 store / 会话 ViewState / egui Memory）

- **What**：
  1. **Editor/文档 store**：权威、可 undo、可序列化（场景、资源引用、蓝图配置）；
  2. **ViewState**：布局、停靠树、选中/悬停、面板开关、过滤器、最近打开——可序列化但不必进 undo；
  3. **egui Memory**：只放「纯 UI 暂态」（滚动位置、折叠状态、输入框草稿），如需要可整体 serde。
- **Why**：egui 官方明说 Memory "NOT meant to store any important data. Store that in your own structures!"（[docs.rs/egui Memory](https://docs.rs/egui/latest/egui/struct.Memory.html)）；Rerun 把 `AppState` 做成 `serde` 持久化（[app_state.rs](https://docs.rs/crate/re_viewer/0.41.0/source/src/app_state.rs)），而选择/undo 在 context 层（[selection_state.rs](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/selection_state.rs)、[undo.rs](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/undo.rs)）。
- **When-not**：不要为了「分层」把每个字段都包一层 store；按「变更频率 × 是否需要持久化 × 是否需要 undo」划分。

### 模式 4：纯 reducer + 显式 effect 返回值

- **What**：`fn reduce(state: &mut State, cmd: Command) -> Vec<Effect>`（或 `Task`），reducer 内不做 I/O、不发网络、不碰 UI；effect 由 logic 段执行。
- **Why**：可单测、可回放、可批处理。iced 的 `update -> Task`（[trait.Program](https://docs.rs/iced/0.14.0/iced/trait.Program.html)）、Elm 的 `Cmd`（[guide.elm-lang.org/effects](https://guide.elm-lang.org/effects/)）是标准形态。
- **When-not**：egui 没有现成 runtime，需要自建 `Effect` 枚举 + 执行器（推荐只做最小子集：`SaveFile` / `LoadAsset` / `SpawnTask` / `Repaint` / `EngineCommand`）。
- **关键细节**：effect 执行结果要能**回流成命令**（iced 的 `Task::perform(fut, Message::Xxx)` 模式），否则 effect 系统会变成第二套隐式通路。

### 模式 5：权威文档 store 与运行时投影分离

- **What**：编辑器维护自己的「文档/蓝图 store」（可 serialize、可 undo、可 diff），运行时（ECS 场景）是从 store **投影**出来的产物；命令只改 store，再由系统在合适时机把差异应用到引擎。
- **Why**：Rerun 的 store vs blueprint 分离（数据 store + 独立 blueprint store + `blueprint_timeline()`，见 [StoreHub 源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/store_hub.rs)、[undo.rs](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/undo.rs)）是同一思路；也可让编辑器在引擎未启动时仍能打开/编辑文档。
- **When-not**：编辑器只是「引擎的调试面板」、所有编辑都必须实时作用于运行时，可以省；但一旦要 undo/多文档/离线编辑就需要。
- **关键细节**：投影必须是**幂等 + 可校验**的（Rerun 的 `on_frame_start` 解析失效 selection 是同类思想）。

### 模式 6：集中式 Selection / Hover 状态 + 每帧有效性解析

- **What**：全局唯一 `SelectionState { items: ItemCollection, changed: Option<SelectionSource> }`；所有面板读写同一对象；每帧开头校验失效项（实体删除、资源卸载、视图关闭）。
- **Why**：避免「每个面板各自 `Option<Entity>`」造成的悬空选择与不一致；Rerun 的 `ApplicationSelectionState` 文档写明 "Write to this with SystemCommand::set_selection" 且有 `on_frame_start` 解析钩子（[源码](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/selection_state.rs)）。
- **When-not**：不需要全局选择的小工具；或选择语义完全不同（比如文本编辑器的光标选区）。
- **关键细节**：选择变化要能触发「多面板同步」（Inspector 跟随选择），但**不要在选择变化时立刻重算重型数据**——放到 logic 段。

### 模式 7：Undo/Redo 作为一等公民（命令栈或时间线）

- **What**：`UndoStack` 记录「可撤销单元」；交互式拖拽/连续输入合并成一个单元；外部修改（脚本/网络/引擎）也要进 history。
- **Why**：编辑器刚需；Rerun 的经验：`MAX_UNDOS = 100`、`EXTERNAL_EDIT_SETTLE_TIME = 250ms` 合并外部编辑、拖拽启发式合并（[undo.rs](https://docs.rs/crate/re_viewer_context/0.41.0/source/src/undo.rs)）；Redux 文档也指出状态集中后 "Undo/Redo ... can suddenly become trivial"（[redux](https://redux.js.org/understanding/thinking-in-redux/three-principles)）。
- **When-not**：**不要**用「每帧 snapshot 整个状态」实现 undo（除非状态很小且你确认可接受内存/CPU）。
- **关键细节**：先把命令分成 `transient`（不改文档：视图/选择）与 `persistent`（改文档）两类，只对后者入 undo。

### 模式 8：注册表驱动的命令 + 属性 UI（CommandRegistry + DataUi）

- **What**：
  - 所有命令在一个 `CommandRegistry` 注册（id、标题、快捷键、可用性条件、scope），菜单/工具栏/命令面板/快捷键**从注册表生成**，不手写三份；
  - 属性面板用「类型 → 编辑 UI 函数」注册表（`DataUi` 式），新组件/新资源类型不改面板代码。
- **Why**：
  - Rerun `re_ui` 有 `CommandPalette` / `CommandPaletteProvider` / `CommandEnvironment` / `CommandKind` / `CommandScope`（[docs.rs/re_ui](https://docs.rs/re_ui/0.41.0/re_ui/)）；
  - `re_data_ui` 有 `DataUi` / `EntityDataUi` / `AppUi` + `add_to_registry` / `register_component_uis`（[docs.rs/re_data_ui](https://docs.rs/re_data_ui/0.41.0/re_data_ui/)）。
- **When-not**：命令 < 30 个时可以手写菜单；一旦超过就必须注册表化，否则「快捷键和菜单不同步」是必然 bug。
- **关键细节**：命令注册表 + `SystemCommand` 是两个层次：**注册表**负责「如何呈现/快捷键」，**命令**负责「执行什么」；UI 面板通过 `sender` 发命令。

---

## 13. 反模式清单（结合外部证据）

1. **全局消息队列传 `Box<dyn Any>` / 字符串**：丢掉意图结构，无法 undo/日志/回放；对照 Redux 的 action 定义与 Rerun 的 `SystemCommand`（§5.2）。
2. **回调注册表（callback bus）**：生命周期问题 + 隐式依赖；egui README 把 "no callbacks" 作为设计目标并解释其优势（[README](https://github.com/emilk/egui/blob/main/README.md)）。
3. **MVC Controller 化**：Controller 持有 `&mut Model` 又调 `&mut Ui`，Rust 借用检查器下只能退化成内部可变性；Raph Levien 的批评（§2.7）。
4. **把应用状态放 `egui::Memory`**：官方明确反对（§4.2）。
5. **每个面板各自维护选择/当前文档**：导致多面板不同步、悬空引用；正解是集中 SelectionState（§5.4、模式 6）。
6. **在 `ui()` 里直接做 I/O / 启动任务 / 改引擎 ECS**：会让「隐藏窗口」时停摆、UI pass 可能跑两次（Rerun 关于 `request_discard` 的注释，§5.8）。
7. **undo 用全量快照**：编辑器状态大时不可行；Rerun 用时间线 + 合并启发式（§5.5）。
8. **UI 代码里写 `if type == ...` 巨型 match 画属性**：不可扩展；用注册表（§5.6）。
9. **为了「响应式」引入隐式依赖图**：引擎外部写入不走 signal 时必出 stale 数据（§9）。
10. **过早整体迁移到 iced/Slint/Xilem/GPUI**：迁移成本远超收益，且自绘控件/渲染集成要重写（§3.4、§6.3、§7.3、§8.5）。

---

## 14. 分阶段落地建议（供 Lead 设计文档取舍）

> 目标：**保留 egui 渲染层，重建应用架构**。每阶段都能独立合并、独立验证。

| 阶段 | 内容 | 验收标准 | 依据 |
|---|---|---|---|
| **P0 基线** | 盘点现有 Messager 的消息类型、订阅者、状态写入点；把每条消息归类为「UI 会话态 / 文档编辑 / 系统动作」 | 得到消息类型清单 + 每类状态的真源说明 | 本调研 §5.2 的分层思想 |
| **P1 命令层** | 引入 `CommandSender`（trait）+ 分层命令枚举；UI 里所有 `messager.send(...)` 改为 `send_command(...)`；logic 段单点 drain | 命令可日志；新增命令不改发送方；编译期穷举 | 模式 1、模式 4 |
| **P2 帧循环** | 拆 `logic()` / `ui()`；后台任务、资源加载、外部同步移到 logic | 窗口隐藏时后台任务仍推进；logic 段可 headless 测试 | 模式 2 |
| **P3 状态分层** | 建 `EditorStore`（权威）+ `ViewState`（布局/选择）+ 清理 `ui.data` 里的业务状态；`AppState` 可 serde | 重启后恢复布局；关掉引擎仍能打开文档 | 模式 3、模式 5 |
| **P4 选择与 undo** | 集中 `SelectionState`（含失效解析）；`UndoStack` + 连续编辑合并 + 外部编辑入栈 | 多面板选择同步；拖拽属性滑块只产生 1 条 undo | 模式 6、模式 7 |
| **P5 注册表** | `CommandRegistry`（菜单/快捷键/命令面板同源）+ `PropertyUiRegistry`（DataUi 式） | 新增面板/组件不改注册表调用方；快捷键与菜单一致 | 模式 8 |
| **P6 布局与测试** | 接入 `egui_tiles`/`egui_dock` 做可序列化布局；`egui_kittest` 加 headless 冒烟 + 快照 | 布局可保存/恢复；核心面板有截图回归 | §4.5、§5.7、[egui_tiles](https://docs.rs/egui_tiles/latest/egui_tiles/)、[egui_dock](https://docs.rs/egui_dock/latest/egui_dock/) |

**不建议的阶段**：把 UI 迁移到 iced/Slint/GPUI；把 ECS 当编辑器 UI 状态容器（§10.3）。

---

## 15. 风险与未解问题

1. **Rerun 的架构不能直接依赖**：`re_viewer_context` 依赖 `re_chunk`/`re_entity_db`/`re_ui` 等整套（见 [Cargo 依赖列表](https://docs.rs/crate/re_viewer_context/0.41.0)），抽离成本高；只可「读源码抄模式」。
2. **egui 版本漂移**：eframe 0.37 已把 `App` 从 `update(ctx)` 改成 `ui/ui+logic`（§4.3）；egui 每几个月 breaking change，架构设计要隔离 egui 类型（面板只通过 trait/命令交互）。
3. **「一帧可能跑两次 ui pass」**（Rerun 注释提到 `request_discard`，§5.8）：逻辑不能依赖「每帧只执行一次」；命令 drain 要幂等/可重入。
4. **10k+ LOC 后 enum 膨胀**：必须分层（UI/文档/系统），并考虑用「每领域一个 reducer + 组合」而不是单一巨型 match（Raph Levien 对 Elm 组件化的批评，§2.7）。
5. **可测试性天花板**：egui 是 immediate mode + AccessKit，`egui_kittest` 能测交互与快照（§4.5），但「视觉正确性」仍依赖截图阈值；建议把业务逻辑从 ui 中抽出，靠 logic 段单测兜底。
6. **不要为了「架构先进」引入 signals 隐式依赖**：引擎外部写入会破坏自动订阅假设（§9.1）。
7. **本文未覆盖**：具体 crate 的 license/维护活跃度评估、Kairos 现有代码的具体重构工作量、UI 设计系统细节（这些属于 Lead/其他 task）。

---

## 附录 A：关键 API 速查（已核对版本）

| 框架 | 关键 API | 版本 / 链接 |
|---|---|---|
| iced | `trait Program`：`boot -> (State, Task)`、`update(&self, &mut State, Message) -> Task`、`view -> Element`、`subscription -> Subscription` | 0.14.0，[docs.rs](https://docs.rs/iced/0.14.0/iced/trait.Program.html) |
| iced | `Task::perform(future, Message::X)`、`Task::none()` | 0.13/0.14 文档 Concurrent Tasks 一节，[docs.rs/iced 0.13.1](https://docs.rs/iced/0.13.1/iced/) |
| iced | `iced_test::Emulator / Ice / Simulator / screenshot / Selector` | 0.14.0，[docs.rs/iced_test](https://docs.rs/iced_test/latest/iced_test/) |
| egui/eframe | `trait App { fn ui(&mut self, ui, frame); fn logic(&mut self, ctx, frame); }`、`Context::request_repaint` | 0.37.0，[docs.rs/eframe](https://docs.rs/eframe/latest/eframe/trait.App.html) |
| egui | `Memory { options, data: IdTypeMap, caches, to_global }`；"NOT meant to store any important data" | 0.37.0，[docs.rs/egui Memory](https://docs.rs/egui/latest/egui/struct.Memory.html) |
| egui | `egui_kittest::Harness`：`get_by_label/click/run/snapshot` + 可访问性检查 | 0.37.0，[docs.rs/egui_kittest](https://docs.rs/egui_kittest/latest/egui_kittest/) |
| egui 生态 | `egui_tiles::Tree/Behavior/Tile`；`egui_dock::DockState/TabViewer` | [docs.rs/egui_tiles](https://docs.rs/egui_tiles/latest/egui_tiles/)、[docs.rs/egui_dock](https://docs.rs/egui_dock/latest/egui_dock/) |
| Rerun | `enum SystemCommand`（43 variants）、`trait SystemCommandSender { fn send_system(&self, SystemCommand); }` | 0.41.0，[enum](https://docs.rs/re_viewer_context/0.41.0/re_viewer_context/enum.SystemCommand.html)、[trait](https://docs.rs/re_viewer_context/0.41.0/re_viewer_context/trait.SystemCommandSender.html) |
| Rerun | `StoreHub`、`ViewerContext`、`ApplicationSelectionState`、`BlueprintUndoState`、`CommandSender` | 0.41.0，[re_viewer_context docs](https://docs.rs/re_viewer_context/0.41.0/re_viewer_context/) |
| Rerun | `DataUi / EntityDataUi / AppUi` + `add_to_registry` / `register_component_uis` | 0.41.0，[docs.rs/re_data_ui](https://docs.rs/re_data_ui/0.41.0/re_data_ui/) |
| Rerun | `re_ui`：`DesignTokens`、`CommandPalette`、`CommandKind/CommandScope`、list item、modal 等 | 0.41.0，[docs.rs/re_ui](https://docs.rs/re_ui/0.41.0/re_ui/) |
| Slint | `in/out/in-out property`、`callback` + `=>`、`global`；Rust 侧 `set_/get_/invoke_/on_` + `ComponentHandle` | [Properties](https://docs.slint.dev/latest/docs/slint/reference/language/properties/)、[Callbacks](https://docs.slint.dev/latest/docs/slint/reference/language/callbacks/)、[Globals](https://docs.slint.dev/latest/docs/slint/reference/language/globals/)、[Rust API](https://docs.slint.dev/latest/docs/rust/slint/) |
| Xilem | `Xilem::new_simple(State::default(), app_logic, WindowOptions)`；`app_logic(&mut State) -> impl WidgetView<State>`；lens/memoize | 0.4.0，[docs.rs/xilem](https://docs.rs/xilem/latest/xilem/) |
| Masonry | `trait Widget { type Action; ... }`、`on_pointer_event/property_changed/layout/paint` | 0.4.0，[trait.Widget](https://docs.rs/masonry/latest/masonry/core/trait.Widget.html) |
| GPUI | `App`、`Context<T>`、`cx.listener`、`cx.emit`、`cx.notify`、`cx.spawn -> Task`、`Entity<T>` | [glossary](https://zed.dev/docs/development/glossary)、[context.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app/context.rs)、[ownership doc](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/_ownership_and_data_flow.rs) |
| Dioxus | `Signal<T>`（Copy）、`use_signal`、local subscriptions | 0.7.10，[docs.rs/dioxus-signals](https://docs.rs/dioxus-signals/latest/dioxus_signals/) |
| Leptos | `RwSignal/ReadSignal/WriteSignal`、`Memo`、`Resource`、`Action`、`reactive_graph` | 0.8.22，[docs.rs/leptos](https://docs.rs/leptos/latest/leptos/) |
| Floem | `RwSignal::new`、`.action(...)`、`Label::derived`、`floem::launch` | [README](https://github.com/lapce/floem/blob/main/README.md) |
| Bevy | `Observer` 组件、`On` 系统参数、push-based 事件 | 0.20.1，[bevy::ecs::observer](https://docs.rs/bevy/latest/bevy/ecs/observer/index.html) |

## 附录 B：理论引用清单

- MVC 起源：Reenskaug, *Models–Views–Controllers* (1979)，[Zenodo 存档](https://zenodo.org/records/3676092)。
- MVVM：[Microsoft Learn: Model-View-ViewModel](https://learn.microsoft.com/en-us/dotnet/architecture/maui/mvvm)。
- MVI：[Hannes Dorfmann, Model-View-Intent on Android (2016)](http://hannesdorfmann.com/android/model-view-intent/)。
- Elm Architecture：[guide.elm-lang.org/architecture](https://guide.elm-lang.org/architecture/)、[Commands & Subscriptions](https://guide.elm-lang.org/effects/)。
- Redux 三原则：[redux.js.org/understanding/thinking-in-redux/three-principles](https://redux.js.org/understanding/thinking-in-redux/three-principles)。
- CQRS：[Martin Fowler, CQRS (2011)](https://martinfowler.com/bliki/CQRS.html)。
- Rust UI 架构论述：[Raph Levien, Xilem: an architecture for UI in Rust (2022)](https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html)；[Towards a unified theory of reactive UI (2019)](https://raphlinus.github.io/ui/druid/2019/11/22/reactive-ui.html)。

---

## 附录 C：给 Lead 的一句话结论

> **架构方向：保留 egui 渲染，采用「类型化命令 + logic/ui 两段帧循环 + 三层状态 + 集中选择/undo + 注册表驱动」的 Rerun 式应用架构；MVU 只借其 reducer 思想，不借其单 Message/单 State 骨架；不迁移框架。**
