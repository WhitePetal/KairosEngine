# UI 控件 vs 组件分层与设计系统调研

> **状态**：调研完成（2026-10-30）
> **范围**：面向 kairos_editor（egui 0.35）的「纯外观 widgets」与「带业务逻辑 components」分层、主题/契约/测试策略，以及 15–25 个 widget 的 kit 契约。
> **证据口径**：全部结论来自一手来源（docs.rs API 文档、crate 源码、官方设计文档）；版本快照：egui/egui_kittest **0.35.0**（docs.rs 2026-06-26）、re_ui **0.41.0**（2026-10-28 发布，依赖 egui 0.37）、Zed **main**（2026-10 抓取）、iced **0.14.0**（2026-10-27）。
> **一句话结论**：成熟 Rust/egui 代码库都不靠"目录名"分层，而靠**依赖方向 + 状态归属 + 输出类型**分层：widget 只读 props、只产出 Response/小事件；组件持有领域语义、只发 Action；tokens 是数据、不是样式代码；测试按"纯渲染快照 → AccessKit 交互 → 画廊人工审查"三级走。

---

## 0. TL;DR（先看这 9 条）

1. **widget 是 builder，不是对象**：egui 的 `Widget` trait 明确"consumes `self`""widgets 是 builders，不是持有状态的对象"，本 trait 的文档即契约（[trait.Widget](https://docs.rs/egui/0.35.0/egui/trait.Widget.html)、[源码 widgets/mod.rs](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/widgets/mod.rs)）。
2. **"展示型/容器型"只是思维工具，不是必须的目录强制**：Dan Abramov 2019 年亲自收回"每个组件都要二分"的建议，警告"almost dogmatic fervor"（[原文](https://medium.com/@dan_abramov/smart-and-dumb-components-7ca2f9a7c7d0)）。对 Kairos：分层依据是"是否知道领域类型/Action"，不是"有没有 `state` 字段"。
3. **Atomic Design 是"层级模型"不是目录规范**：Brad Frost 明说 "atomic design is not a linear process, but rather a mental model"（[ch.2](https://atomicdesign.bradfrost.com/chapter-2/)）。用它命名 widget 目录（atom/molecule）会制造无意义的迁移。
4. **tokens 是数据**：DTCG 定义 token = 人类可读名称 + 值（+ type/description），可分组、可 alias（[DTCG 2025.10](https://www.designtokens.org/TR/2025.10/format/)）。Rerun `DesignTokens` 有 158 个公开字段（[docs](https://docs.rs/re_ui/0.41.0/re_ui/struct.DesignTokens.html)）；Zed 把 theme 独立成 `theme` crate（[theme.rs](https://github.com/zed-industries/zed/blob/main/crates/theme/src/theme.rs)）。Kairos 已有 `core::StyleState` 作为 TOML 纯数据镜像，方向一致。
5. **Rerun re_ui 0.41 已不再用 `&ReUi` 显式传参**：crate 根 re-export/模块列表中没有 `ReUi` 类型；tokens 改为全局单例（`try_set_design_tokens` / `design_tokens_of(theme)` / trait `HasDesignTokens`）（[lib.rs 源码](https://docs.rs/re_ui/0.41.0/src/re_ui/lib.rs.html)、[DesignTokens](https://docs.rs/re_ui/0.41.0/re_ui/struct.DesignTokens.html)）。**任务描述里的 `&ReUi` 是旧版模式，不要照搬。**
6. **真实代码库的"设计系统 crate"只包"可复用视觉 + 通用交互"**：re_ui = "Rerun GUI theme and helpers"（[crate 页](https://docs.rs/re_ui/0.41.0/re_ui/)）；Zed `ui` = "UI primitives and components used to build all of the elements in Zed's UI"（[ui.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/ui.rs)）。但 re_ui 的依赖表里有 `re_entity_db`/`re_log_types`（[crates.io](https://crates.io/crates/re_ui)）——**说明"零领域依赖"是工程目标而非现实，Kairos 应该设更硬的规则**。
7. **组件的"交互出口"是返回值，不是回调**：egui 原生控件返回 `Response`，容器 `show()` 返回 `InnerResponse`/`ModalResponse`（[Ui::scope 源码](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/ui.rs)、[egui::Modal](https://docs.rs/egui/0.35.0/egui/containers/modal/struct.Modal.html)）；Zed 用 `on_click(|event, window, cx|` 回调是因为 GPUI 有配对的 `cx` 生命周期（[button.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/components/button/button.rs)）。egui 没有这套，回调会立刻退化成 `FnMut` + 借用绕行。
8. **可测试性来自 `WidgetInfo` + AccessKit**：egui 自定义控件必须在 `Response::widget_info` 里描述 role/label/state，kittest 才能按 label/role 查询与点击（[response.rs 源码](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/response.rs)、[WidgetInfo](https://docs.rs/egui/0.35.0/egui/struct.WidgetInfo.html)、[egui_kittest](https://docs.rs/egui_kittest/0.35.0/egui_kittest/)）。**不做这一步，headless 测试写不出来。**
9. **画廊（gallery）+ 快照是行业标配，但图像快照要克制**：Zed 有 `component` crate + `RegisterComponent` + "open component preview"（[component.rs](https://github.com/zed-industries/zed/blob/main/crates/component/src/component.rs)）；egui_kittest 官方指南直说"能写普通 Rust/insta 测试就别写图像快照，慢且脆"（[crate 文档](https://docs.rs/egui_kittest/0.35.0/egui_kittest/)）。

---

## 1. 词汇与范式：先统一语言

### 1.1 Presentational / Container（React/Redux 谱系）

Dan Abramov 2015 的原文把组件分为两类（[原文](https://medium.com/@dan_abramov/smart-and-dumb-components-7ca2f9a7c7d0)）：

- **presentational**："concerned with *how things look*"，"no dependencies on the rest of the app, such as Flux actions or stores"，"receive data and callbacks exclusively via props"，"rarely have their own state (when they do, it's UI state rather than data)"；
- **container**："concerned with *how things work*"，"provide the data and behavior to presentational components"。

关键点是他 2019 年加在文首的 update：

> "I don't suggest splitting your components like this anymore. … I've seen it enforced without any necessity and with almost dogmatic fervor far too many times. … Hooks let me do the same thing without an arbitrary division."（[同文](https://medium.com/@dan_abramov/smart-and-dumb-components-7ca2f9a7c7d0)）

**对 Kairos 的翻译**：不要为"每个组件是否持有状态"设目录边界；设一条可编译检查的边界——**widget 模块不 import `kairos_editor_core::state/action`，component/panel 模块才 import**。这是比 React 时代更硬的版本，因为 Rust 可以靠 crate/模块可见性强制。

配套的 React 官方原则同样可迁移：

- "remove state from both of them, move it to their closest common parent, and then pass it down to them via props. This is known as **lifting state up**"（[react.dev](https://react.dev/learn/sharing-state-between-components)）→ Kairos 对应"状态上提到 `EditorState` / `ViewState`，view 只读"。
- "passing props can become verbose and inconvenient when you need to pass some prop deeply … **prop drilling**"（[react.dev](https://react.dev/learn/passing-data-deeply-with-context)）→ 对应 Kairos 的 `Frame`（把 `&EditorState`/`&World`/`&StyleState` 一次性传进面板），而不是 10 个 widget 各带一个 `&EditorState`。

### 1.2 Atomic Design（Brad Frost）

五层定义（[ch.2](https://atomicdesign.bradfrost.com/chapter-2/)）：

| 层 | 原文定义 | 对 UI 库的映射 |
|---|---|---|
| Atoms | "foundational building blocks … form labels, inputs, buttons … can't be broken down any further without ceasing to be functional" | Button/Label/Icon/Checkbox/Slider |
| Molecules | "relatively simple groups of UI elements functioning together as a unit" | TextField+Label+Error、ListItem+Icon |
| Organisms | "relatively complex UI components composed of groups of molecules and/or atoms" | Toolbar、PropertyRow、TreeRow、CommandPalette |
| Templates | "page-level content structure" | 面板骨架（Panel layout） |
| Pages | "specific instances of templates" | 真实面板 + 真实数据 |

Brad Frost 自己强调："**Atomic design is not a linear process**, but rather a mental model to help us think of our user interfaces as both a cohesive whole and a collection of parts at the same time"（[同章](https://atomicdesign.bradfrost.com/chapter-2/)）。

**结论**：Atomic Design 的价值是**给"可组合性层级"提供词汇**，不是给目录结构。Kairos 应采用 3 层（tokens / widgets / components），不要照搬 5 层；否则会出现大量"到底是 molecule 还是 organism"的无意义争论。

### 1.3 MVVM（view vs view-model）

微软的定义："The MVVM pattern helps cleanly separate an application's business and presentation logic from its user interface (UI). Maintaining a clean separation between application logic and the UI helps address numerous development issues and makes an application easier to test, maintain, and evolve."（[MS Learn](https://learn.microsoft.com/en-us/dotnet/architecture/maui/mvvm)）。

MVVM 的 view 是**声明式、可绑定**的；Rust/egui 没有数据绑定层（双向绑定需要内部可变性/信号系统，Kairos 现有代码已被 `Cell`/`Arc<Mutex>` 反噬，见 [README.md §1.2](../plan/editor-ui-architecture/README.md)）。因此 Kairos 应取 MVVM 的"**view 只读 view-model**"部分，丢弃"双向绑定"部分：`EditorState`（≈ 全局 view-model）→ `Frame` 只读投影 → 面板绘制 → `Action`（≈ 命令）。这条已在 [README.md §2.1](../plan/editor-ui-architecture/README.md) 论证过，本调研只补证"view 与 view-model 的接口面"：view 的输入是**只读 props + 主题 token + 本地瞬态**，输出是**事件/命令**。

### 1.4 Flutter：StatelessWidget vs StatefulWidget

- `StatelessWidget`："A widget that does not require mutable state."；它"describes part of the user interface by building a constellation of other widgets"（[API](https://api.flutter.dev/flutter/widgets/StatelessWidget-class.html)）。
- `StatefulWidget`："A widget that has mutable state."，且 state 被明确拆到 `State` 对象里（[API](https://api.flutter.dev/flutter/widgets/StatefulWidget-class.html)）。

**可迁移的三条**：(1) widget 是不可变的配置值，状态在别处（Kairos 的 `Widget` consumes self 完全同构）；(2) 组合（composition）是唯一扩展手段，继承不参与 UI 构建；(3) "widget 自身状态"被框架收进 `State` 对象——egui 的对应物是 `egui::Memory`（按 `Id` 存），但它只该存瞬态（见 §2.4）。

### 1.5 Design tokens（W3C DTCG）

DTCG 草案（**Design Tokens Format Module 2025.10**，Draft Community Group Report，2026-09-08 发布）定义：

> "A (Design) Token is information associated with a human readable name, at minimum a name/value pair." 例如 `color-text-primary: #000000`；token 还可带 **type / description** 等 properties；文件格式支持 group 与 alias/reference（[规范](https://www.designtokens.org/TR/2025.10/format/)、[草案地址](https://www.designtokens.org/tr/drafts/format/)）。

这带来三条工程含义（均可在真实代码库验证）：

1. **token 是数据，可序列化、可 diff、可被工具消费**：Rerun `DesignTokens` 是 `pub struct`（158 字段，含颜色/间距/圆角/字体/语义色），并支持运行时热重载与 `try_set_design_tokens` 覆盖（[DesignTokens](https://docs.rs/re_ui/0.41.0/re_ui/struct.DesignTokens.html)、[try_set_design_tokens](https://docs.rs/re_ui/0.41.0/re_ui/fn.try_set_design_tokens.html)）。
2. **语义层 > 原始值层**：Rerun 同时有 `Color32` 原始色与 `text_default`/`text_subdued`/`selection_bg_fill` 等语义 token；Zed 的 `theme` crate 把 "A theme is a collection of colors used to build a consistent appearance for UI components" 独立成 crate，并再分 `accent/colors/status/syntax/system` 子模块（[theme.rs](https://github.com/zed-industries/zed/blob/main/crates/theme/src/theme.rs)、[styles/colors.rs](https://github.com/zed-industries/zed/blob/main/crates/theme/src/styles/colors.rs)）。
3. **token 不能全塞进 `egui::Style`**：Rerun 文档明说 "Not everything is covered by this. A lot of other design tokens are put straight into the `egui::Style`"（[DesignTokens](https://docs.rs/re_ui/0.41.0/re_ui/struct.DesignTokens.html)）。→ Kairos 应接受"两处存放"：`core::StyleState`（自有语义 token）+ `egui::Style`（框架认识的 spacing/visuals），映射集中在 `ui::style` 一处。

### 1.6 Headless UI / unstyled primitives

Headless UI 的自我定位："**Completely unstyled, fully accessible UI components**, designed to integrate beautifully with Tailwind CSS"（[headlessui.com](https://headlessui.com/)）。它的实质是把"行为/可访问性/状态机"与"视觉"分离；组件库只交付前者。

**映射到 egui**：egui 的 `Widget` trait 本来就是 headless 的——它只规定"allocate space, interact, paint, 返回 `Response`"，没规定任何视觉。因此 Kairos 的 widget kit 天然是 headless 的**行为层**；视觉来自 tokens + `egui::Style`。真正需要设计的不是"headless vs styled"这条轴，而是"**behavior layer 里允不允许出现领域语义**"（答案：不允许，见 §6）。

### 1.7 收敛点与分歧点

**六条收敛**（每个来源都能找到出处）：

| # | 收敛点 | 一手证据 |
|---|---|---|
| C1 | 视觉与逻辑分离；逻辑用状态机/单向数据流表达 | Dan 2015；MVVM；Flutter `State`；iced Elm |
| C2 | widget 是"配置/builder"，无长生命周期状态 | egui Widget doc；Flutter immutable widget；iced `Widget` |
| C3 | 组合优先，props/children 向下，事件/命令向上 | React lifting state；Atomic Design；GPUI `ParentElement` |
| C4 | 主题 = 语义 token 数据（可分组/alias/序列化） | DTCG；Rerun `DesignTokens`；Zed `theme` crate |
| C5 | 可访问性与可测试性绑定：不描述 role/label 就没法测 | egui `WidgetInfo`/`AccessKit`；headless UI "fully accessible" |
| C6 | 组件库需要画廊/示例来防退化 | Zed `component` crate "component preview"；re_ui `re_ui_example`（[ListItem docs](https://docs.rs/re_ui/0.41.0/re_ui/list_item/struct.ListItem.html)） |

**两个分歧**（要在 Kairos 明确取舍）：

- **D1 分层是否教条**：Dan 2019 收回 container/presentational 二分；Brad Frost 说 atomic 非线性。→ Kairos 只保留一条硬边界（widget 不懂 domain），不强制 5 层命名。
- **D2 widget 状态放哪**：Flutter 有 `State` 对象；React 有 hooks；egui 有 `Memory`；GPUI 把 `Element::PrepaintState` 作为框架内瞬态。→ Kairos 按"重启是否该恢复 / 是否要 undo/MCP 可见"划分（[README.md §3.4](../plan/editor-ui-architecture/README.md)），不要一刀切。

---

## 2. egui 0.35 的控件模型（一手源码）

### 2.1 `Widget` trait：consume self 的 builder

源码原文（[widgets/mod.rs](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/widgets/mod.rs)）：

```rust
/// You only need to implement `Widget` if you care about being able to do `ui.add(your_widget);`.
///
/// Note that the widgets ([`Button`], [`TextEdit`] etc) are
/// [builders](https://doc.rust-lang.org/1.0.0/style/ownership/builders.html),
/// and not objects that hold state.
///
/// Tip: you can `impl Widget for &mut YourThing { }`.
#[must_use = "You should put this widget in a ui with `ui.add(widget);`"]
pub trait Widget {
    /// Allocate space, interact, paint, and return a [`Response`].
    ///
    /// Note that this consumes `self`.
    fn ui(self, ui: &mut Ui) -> Response;
}
```

三个可直接用的扩展点：

1. **blanket impl**：`FnOnce(&mut Ui) -> Response` 自动实现 `Widget`。对 Kairos 意味着"widget 也可以是一个函数"，适合一次性/实验性控件，但不适合 kit（无法文档化 props、无法统一 gallery 注册）。
2. **`impl Widget for &mut Thing`**：当 widget 必须驱动外部状态时用（egui 官方建议）。
3. **容器/复合控件**用 `Ui::scope` / `ui.push_id` 返回 `InnerResponse`（[ui.rs 源码](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/ui.rs)）；`push_id` 的文档就是"同 wrapper 循环会 Id clash，用 `push_id` 区分"。→ **widget kit 的每个可复用多实例控件必须有 `id_salt` 参数**。

### 2.2 复合控件的两种成熟形态

- **builder + impl Widget**：egui 内建 `Button` 就是这样——`Button::new(impl IntoAtoms)` 返回 builder，链式方法 `.selected()/.frame()/.small()`，最后 `ui.add(button)`（[button.rs 源码](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/widgets/button.rs)）。注意 0.35 的 `Button` 内部已是 `AtomLayout`，文本/图标统一为 `Atom`/`Atoms`，这是"文本 + 图标 + 状态"统一 props 的官方方向。
- **`show() -> Output` 结构体**：容器类（`Modal`、`Window`、`ScrollArea`、`ComboBox`）走 `show()` 返回专门 Output（`InnerResponse<T>`、`ModalResponse<T>`）。egui 0.35 内置 [`containers::modal::Modal`](https://docs.rs/egui/0.35.0/egui/containers/modal/struct.Modal.html)（`Modal::new(id).show(ctx, |ui| ...) -> ModalResponse<T>`），Kairos 不需要自己实现遮罩/层级/输入拦截。
- **生态验证**：egui 官方扩展 crate `egui_extras` 的 `TableBuilder` 也是同一形态——builder 配置列/滚动/样式，`.header(...) -> Table` 后 `.body(...) -> ScrollAreaOutput<()>` 返回输出；并且 `.id_salt(...)` 的文档明确写 "This is required if you have multiple tables in the same Ui"（[table.rs 源码](https://github.com/emilk/egui/blob/0.35.0/crates/egui_extras/src/table.rs)）。
- **交互结论**：`Response` 是 egui 的"事件出口"（`.clicked()/.changed()/.hovered()` + `on_hover_text` 等）；复合控件应返回 `Output { response, ... }` 而不是在每个内部交互点塞回调。**Kairos 不需要回调式 props。**

### 2.3 主题/token 在 egui 0.35 里怎么传

egui 0.35 有**四条**可用的主题通道（全部一手验证）：

| 通道 | API | 用法/限制 |
|---|---|---|
| 全局 dark/light | `Context::set_theme(ThemePreference)`、`Context::theme()`、`set_style_of(Theme, style)`、`style_of(Theme)`、`all_styles_mut()`（[context.rs 源码](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/context.rs)、[`Theme`](https://docs.rs/egui/0.35.0/egui/enum.Theme.html)） | 适合"主题切换"；Kairos 的 `StyleState` 应一次性映射成两套 `Style`（dark/light），不要每帧改 |
| 局部 style | `Ui::scope` 内 `ui.style_mut()`（[ui.rs 源码](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/ui.rs)）、`StyleModifier`（re_ui 的 `menu_style()` 即此，[menu.rs 源码](https://docs.rs/re_ui/0.41.0/src/re_ui/menu.rs.html)） | 适合"菜单/弹窗/面板"统一微调，避免 per-instance 硬编码 |
| widget 级 class/state | `egui::widget_style`：`WidgetStyle { frame, text, stroke }`、`ButtonStyle`/`LabelStyle`/`CheckboxStyle`/`SeparatorStyle`、`Classes`（静态字符串 class，`HasClasses` trait）、`ROOT_CLASS`/`SELECTED_CLASS`（[模块](https://docs.rs/egui/0.35.0/egui/widget_style/index.html)、[`Classes`](https://docs.rs/egui/0.35.0/egui/widget_style/struct.Classes.html)） | 0.35 新出现的"类 + 状态 → 样式"雏形；**可作为 DTCG token → egui visuals 的桥，但属于较新 API，Kairos 应封装在自己的 `ui::style` 里，避免散落** |
| 显式 token 参数 | `&DesignTokens` 或 `&Style`（Rerun 0.41 之前/内部、re_ui `HasDesignTokens` 的 `tokens()`） | 只在"需要非 egui 语义 token"时使用；**不要把 `&Theme` 塞进每个 widget 签名**（见 §3.2） |

### 2.4 瞬态状态：`Memory` 能做什么、不能做什么

egui `Memory::data` 的源码文档是**权威边界声明**（[memory/mod.rs](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/memory/mod.rs)）：

> "This map stores some superficial state for all widgets with custom `Id`s. This includes storing whether a `CollapsingHeader` is open, how far scrolled a `ScrollArea` is, where the cursor in a `TextEdit` is, etc.
> **This is NOT meant to store any important data. Store that in your own structures!**"
> "This will be saved between different program runs if you use the `persistence` feature."

→ Kairos 规则：**widget 内部**的 hover/focus/scroll/文本光标/折叠动画 = `Memory`/内部状态；**面板级**的展开项/重命名缓冲/滚动位置 = `ui::ViewStates`（key = `PanelInstanceId`）；**领域级** = `core`。三条判据与 [README.md §3.4](../plan/editor-ui-architecture/README.md) 一致。

### 2.5 可访问性与 `WidgetInfo`

`Response::widget_info(|| WidgetInfo::...)` 的源码注释："For accessibility. Call after interacting and potential calls to `mark_changed`."；内部会 `fill_accesskit_node_from_widget_info` + `register_widget_info`，并把 clicked/changed 等转成 `OutputEvent`（[response.rs 源码](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/response.rs)）。

`WidgetInfo` 字段：`typ: WidgetType`（20 种内建类型 + `Other`）、`enabled`、`label`、`current_text_value`/`prev_text_value`、`selected`、`value`、`text_selection`、`hint_text`（[WidgetInfo](https://docs.rs/egui/0.35.0/egui/struct.WidgetInfo.html)、[WidgetType](https://docs.rs/egui/0.35.0/egui/enum.WidgetType.html)）；egui 0.35 依赖 `accesskit ^0.24.1`（[crate 页](https://docs.rs/egui/0.35.0/egui/)）。

→ **widget kit 的完成定义（DoD）里必须有这一条**：任何可点击/可切换/可输入的 widget，都要调用 `widget_info` 并给 `WidgetType` 与 label（或 `WidgetInfo::selected`/`slider`/`text_edit` 等构造器）。否则 kittest 的 `get_by_label`/`get_by_role` 找不到它（§2.6）。

### 2.6 `egui_kittest`：headless 交互 + 可选图像快照

crate 文档原文："Ui testing library for egui, based on **kittest** (an **AccessKit based testing library**)"（[egui_kittest](https://docs.rs/egui_kittest/0.35.0/egui_kittest/)）。核心 API 形态：

```
Harness::new_ui(app)          // 把 |ui: &mut Ui| 包成可测 app
harness.get_by_label("...")   // AccessKit 节点查询（Queryable）
node.click(); harness.run();  // 注入交互、推进帧
harness.fit_contents();       // 缩到最小尺寸
harness.snapshot("name")      // 需 wgpu + snapshot feature
```

（示例直接来自 crate 首页；`Harness` 方法列表含 `run/fit_contents/try_snapshot/snapshot/take_snapshot`，见 [Harness](https://docs.rs/egui_kittest/0.35.0/egui_kittest/struct.Harness.html)）

- **快照配置**：`kittest.toml`（workspace 根）可配 `output_path`、`threshold`（默认 0.6）、`failed_pixel_count_threshold`，并按 windows/macos/linux 分平台；`UPDATE_SNAPSHOTS=true` 更新失败用例，`UPDATE_SNAPSHOTS=force` 全量更新（[crate 文档](https://docs.rs/egui_kittest/0.35.0/egui_kittest/)）。
- **官方劝退**："Whenever possible prefer regular Rust tests or insta snapshot tests over image comparison tests because… relatively slow… brittle since unrelated side effects (like a change in color) can cause the test to fail… images take up repo space."（同页）→ **图像快照只用在"视觉回归真正关键"的 10–20 个 widget，不要全量铺开**。
- **对 0.35 的适配注意**：egui 0.35 默认 `max_passes = 2`（多 pass 会在 `request_discard` 时重跑闭包，Kairos 审计文档已列为必须项，[README.md §3.3.1](../plan/editor-ui-architecture/README.md)）。kittest 的 `harness.run()` 会推进帧，**测试断言要放在最终 pass 之后**；widget 内部不能有"每 pass 执行一次"的副作用。


---

## 3. Rerun `re_ui` 0.41：设计系统 crate 的现实样本

### 3.1 crate 定位与模块地图

`re_ui` 的 crate 文档："Rerun GUI theme and helpers, built around egui."（[crate 页](https://docs.rs/re_ui/0.41.0/re_ui/)）。`lib.rs` 的模块声明直接展示了它的职责边界（[lib.rs 源码](https://docs.rs/re_ui/0.41.0/src/re_ui/lib.rs.html)）：

- **视觉基建**：`design_tokens`、`icons`、`egui_ext`、`ui_ext`、`ui_layout`、`motion`、`markdown_utils`
- **控件**：`button`、`combo_item`、`list_item`、`menu`、`modal`、`tab_bar`、`text_edit`、`time`、`time_drag_value`、`loading_indicator`、`progress_ring`、`section_collapsing_header`、`panel_title_bar`、`filter_widget`、`alert`、`re_form`
- **app chrome**：`command`、`command_palette`、`notifications`、`hot_reload_design_tokens`、`context_ext`、`debug_only`、`drag_and_drop`

`lib.rs` 的 re-export 进一步说明哪些是"公共动词"：`DesignTokens`、`Icon`、`LinkButton`、`CommandPalette`/`CommandPaletteProvider`、`ControlRow`、`TabBar`、`ListItem`、`Modal` 相关类型、`UiExt`/`ContextExt`、`UICommand` 系列等（[同源](https://docs.rs/re_ui/0.41.0/src/re_ui/lib.rs.html)）。

**关键观察**：re_ui **不是纯 atom 库**，它把"通用 chrome（命令面板/通知/快捷键展示）"也收进来了。这直接决定了下文的边界讨论：设计系统 crate 的合理范围 = "多个 feature 共享的视觉与交互基建"，而不是"只允许 8 个 atom"。

### 3.2 token / theme 的传递方式（重要更正）

**0.41 的 token 是全局单例，不是 `&ReUi` 参数。** 证据链：

- 0.41 的模块列表/re-export 列表中没有 `ReUi` 类型（[lib.rs 源码](https://docs.rs/re_ui/0.41.0/src/re_ui/lib.rs.html)）；
- `DesignTokens` 是 `pub struct`，**158 个公开字段**，覆盖颜色/间距/描边/动画时长/语义状态色（[DesignTokens](https://docs.rs/re_ui/0.41.0/re_ui/struct.DesignTokens.html)）；
- 获取方式：`design_tokens_of(theme: Theme) -> &'static DesignTokens`（[fn](https://docs.rs/re_ui/0.41.0/re_ui/fn.design_tokens_of.html)）；
- 扩展 trait：`trait HasDesignTokens { fn tokens(&self) -> &'static DesignTokens }`，为 `egui::Context`/`Style`/`Visuals` 实现（[trait 页](https://docs.rs/re_ui/0.41.0/re_ui/trait.HasDesignTokens.html)）；
- 下游主题覆盖：`try_set_design_tokens(dark, light) -> Result<(), DesignTokensAlreadyInitializedError>`，文档说明"Lets downstream crates ship their own theming (e.g. a tweaked color palette) without forking re_ui"（[fn](https://docs.rs/re_ui/0.41.0/re_ui/fn.try_set_design_tokens.html)）；
- 另有 `hot_reload_design_tokens` 模块与 `design_tokens_of_visuals`（[lib.rs](https://docs.rs/re_ui/0.41.0/src/re_ui/lib.rs.html)）。

同时，`DesignTokens` 文档明说："Not everything is covered by this. A lot of other design tokens are put straight into the `egui::Style`"（[同页](https://docs.rs/re_ui/0.41.0/re_ui/struct.DesignTokens.html)）。Rerun 还把不少 token 直接按 **widget 类别**命名：`button_primary/button_secondary/button_ghost/button_outlined/button_blue/button_opened`、`tab: TabVisuals`、`outlines: Outlines`、`alert_success/alert_info/alert_warning/alert_error: AlertVisuals`、`list_item_hovered_bg/list_item_active_bg/list_item_*_text/icon`（[同页](https://docs.rs/re_ui/0.41.0/re_ui/struct.DesignTokens.html)）。这说明成熟设计系统**允许 token 带 widget 语义**（不是"纯颜色变量"），但用一个集中的 struct 管理。

**给 Kairos 的直接结论**：

1. **不要设计 `struct Toolkit`/`&ReUi`/`&Theme` 作为每个 widget 的必填参数**。re_ui 已经用"全局 style + extension trait"证明了更 egui 化的形态；Kairos 的 UI 层应提供 `UiExt`/`ContextExt` 式 helper + 从 `ui.style()`/`ctx.style()` 读取。
2. **tokens 的单一来源放 `core::StyleState`**，由 `ui::style` 一次性映射为 `egui::Style`（dark/light 各一套）+ Kairos 自有语义字段；widget 不接收 token 参数。
3. **widget 命名的语义 token 可以被允许**（如 `list_item_hovered_bg`），但必须集中在 tokens 类型里；禁止在组件调用点硬编码 `Color32::from_rgb(...)`。

### 3.3 widget 样本：从具名控件看契约

| 组件 | 形态 | 关键契约 | 来源 |
|---|---|---|---|
| `ListItem` | 泛型 `ListItem`（内容由 `ListItemContent` 决定） | 公开 builder 状态：`interactive/selected/active/draggable/drag_target/force_hovered/collapse_openness`；`show_flat`/`show_hierarchical`/`show_hierarchical_with_children` 返回 `Response`；用 `list_item_scope[_with_style]` 统一上下文；内容组合：`LabelContent`/`PropertyContent`/`ButtonContent`/`CustomContent`/`ItemButtons` | [ListItem](https://docs.rs/re_ui/0.41.0/re_ui/list_item/struct.ListItem.html)、[list_item 模块](https://docs.rs/re_ui/0.41.0/re_ui/list_item/index.html) |
| `ModalWrapper`/`ModalHandler` | builder + 调用方持有的 `Option<ModalWrapper>` | 文档原话："A ModalWrapper is typically held only so long as it is displayed, so it's typically stored in an Option"；`ModalHandler::ui(ctx, make_modal, content_ui)` 负责开/关/动画；内部基于 `egui::Modal` | [modal.rs 源码](https://docs.rs/re_ui/0.41.0/src/re_ui/modal.rs.html) |
| `ControlRow` | 简单构造器 | `ControlRow::new(text: impl Into<String>, items: Atoms<'static>)`；"A single row in the controls table"；配套 `re_form` 模块 | [ControlRow](https://docs.rs/re_ui/0.41.0/re_ui/struct.ControlRow.html)、[re_form](https://docs.rs/re_ui/0.41.0/re_ui/re_form/index.html) |
| `menu` | free function + `StyleModifier` | `menu_style() -> StyleModifier` 统一 `interact_size.y=24`/`menu_margin`/`corner_radius=4` 等；`container_context_menu(response, add_contents)` 用 `egui::Popup` + `open_memory`（跨帧开合状态交给 egui 原生 popup） | [menu.rs 源码](https://docs.rs/re_ui/0.41.0/src/re_ui/menu.rs.html) |
| `CommandPalette` | app chrome（非纯视觉） | `CommandPaletteProvider` 供给命令；`CmdRow`/`MatchGroup`/`MatchedCmd`/`paint_command_row`；`UICommand/UICommandSender/CommandEnvironment` 在 `re_ui::command` 中 | [crate 页 structs](https://docs.rs/re_ui/0.41.0/re_ui/)、[lib.rs](https://docs.rs/re_ui/0.41.0/src/re_ui/lib.rs.html) |
| `icons::Icon` | 资源 provider | "An image (PNG or SVG) embedded in the binary at compile time."；常量枚举式图标（ADD/APPLICATION/CHECKED/CLOSE/…） | [icons 模块](https://docs.rs/re_ui/0.41.0/re_ui/icons/index.html) |

**三个可迁移的具体模式**：

1. **"上下文 scope + 无状态 item"**：`list_item_scope` 提供共享 `ListItemStyle`，每个 `ListItem` 只是数据 + builder。Kairos 的 Tree/List 应照此：`TreeScope` 管行高/缩进/选中色，`TreeRow` 只接受数据与状态位。
2. **瞬态状态放调用方**：`Option<ModalWrapper>`（只在显示期间存在）是最干净的"UI 生命周期状态"范例。Kairos 的 `ModalState` 在 core 放"数据"，widget 层放"wrapper/动画状态"，二者不混。
3. **原生机制优先**：modal 用 `egui::Modal`，菜单用 `egui::Popup`，列表用 `Ui::scope`/`push_id`。**Kairos 不应重写这些基础设施**（与 [rewrite-plan.md](../plan/editor-ui-architecture/rewrite-plan.md) Phase 1 "保留 dock，只换泛型参数"同一原则）。

### 3.4 re_ui 与 viewer feature 组件的边界

Rerun 的 `ARCHITECTURE.md` 把 crate 分成：顶层（`re_sdk`/`re_viewer`/…）、**viewer feature crates**（`re_time_panel`、`re_blueprint_tree`、`re_recording_panel`、`re_viewport` 等，每个面板一个 crate）、**viewer support**（"the UI and rendering machinery the views are built on"，含 `re_renderer`、`re_ui`）（[ARCHITECTURE.md](https://github.com/rerun-io/rerun/blob/main/ARCHITECTURE.md)）。`re_ui` 在该表中的描述就是 "Rerun GUI theme and helpers, built around egui"（[同文](https://github.com/rerun-io/rerun/blob/main/ARCHITECTURE.md)）。

**诚实的反例**：`re_ui` 0.41 的依赖里有 `re_entity_db`、`re_log_types`、`re_analytics`、`re_uri` 等（[crates.io](https://crates.io/crates/re_ui)），因为 command palette/notifications/相对时间范围天然要认识 Rerun 的领域类型。**所以"设计系统 crate 零领域依赖"不是行业事实；它是需要主动维护的约束。** 对 Kairos 的建议是：

- `kairos_editor_ui::widgets`：**禁止** `use kairos_editor_core::{state, action}`、禁止 `use kairos_engine::world`；只能依赖 egui + 本层 tokens/icons。
- `kairos_editor_ui::components`（面板内部组合）：允许 `Frame<'_>`/`UiOut`，允许把 widget 事件翻译成 `Action`。
- 若未来出现"命令面板"这种跨域 chrome，单独放 `ui::chrome` 并显式列出领域依赖白名单，不要污染 widgets。

### 3.5 re_ui 的测试基建（可直接抄）

- dev-dependencies：`egui_kittest`（optional，feature `testing`）与 `insta`（[crates.io](https://crates.io/crates/re_ui)）；`testing` 模块只在 feature 开启时编译（[lib.rs](https://docs.rs/re_ui/0.41.0/src/re_ui/lib.rs.html)）。
- `testing` 模块（[文档](https://docs.rs/re_ui/0.41.0/re_ui/testing/index.html)）：`TestOptions::{Gui, Rendering3D}`、`new_harness(option, size) -> HarnessBuilder<T>`、`step_until(predicate)`、超时/轮询常量 `DEFAULT_WAIT_TIMEOUT`/`DEFAULT_POLL_INTERVAL`、`is_ci`/`strict_on_ci`。
- 快照默认值集中在两处：`default_snapshot_options_for_ui()`（threshold 1.0、max_failed_pixels 10，注释解释"egui 在 CPU 上渲染文本，所以阈值跨平台稳定"）与 `default_snapshot_options_for_3d(viewport_size)`（更高容差）；源码注释要求"bump it here rather than in the individual tests"（[testing/mod.rs 源码](https://docs.rs/re_ui/0.41.0/src/re_ui/testing/mod.rs.html)）。
- 内部实现：`egui_kittest::Harness::builder().wgpu()…`，即 **wgpu 离屏渲染 + AccessKit 交互**（[同源](https://docs.rs/re_ui/0.41.0/src/re_ui/testing/mod.rs.html)）。

**Kairos 抄法**：在 `kairos_editor_ui` 建 `test_support` 子模块（仅 dev/feature）集中 `new_harness`、`snapshot_options`、`step_until`；widget 测试只依赖它，不各自 new Harness。

---

## 4. Zed `ui` crate / GPUI：组件库 + 组件画廊

### 4.1 分层：gpui / theme / ui / feature crates

Zed 的 `ui` crate 文档（lib path = `src/ui.rs`）："This crate provides a set of UI primitives and components that are used to build all of the elements in Zed's UI."（[ui.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/ui.rs)、[Cargo.toml](https://github.com/zed-industries/zed/blob/main/crates/ui/Cargo.toml)）。其内部模块：`components`、`styles`、`traits`、`utils`、`prelude`、`component_prelude`（[同上](https://github.com/zed-industries/zed/blob/main/crates/ui/src/ui.rs)）。**领域隔离靠 Cargo 依赖面强制**：`ui/Cargo.toml` 的依赖只有 `gpui`、`theme`、`icons`、`menu`、`component`、`ui_macros`、`documented` 等基础 crate，不含 `project_panel`/`editor`/`workspace` 等任何 feature crate（[Cargo.toml](https://github.com/zed-industries/zed/blob/main/crates/ui/Cargo.toml)）；因此 feature 代码不可能从 `ui` 反查领域状态。

主题被抽成独立 crate `theme`："This crate provides the theme system for Zed. … A theme is a collection of colors used to build a consistent appearance for UI components across the application."（[theme.rs](https://github.com/zed-industries/zed/blob/main/crates/theme/src/theme.rs)）。其内部再分：`registry`/`schema`/`fallback_themes`/`scale`/`ui_density`/`icon_theme` + `styles::{accents, colors, players, status, syntax, system}`（[theme.rs](https://github.com/zed-industries/zed/blob/main/crates/theme/src/theme.rs)、[styles/colors.rs](https://github.com/zed-industries/zed/blob/main/crates/theme/src/styles/colors.rs)）。`ThemeColors` 的字段是**逐个带 doc 的语义色**（`border`、`border_variant`、`border_focused`、`border_selected`…）（[styles/colors.rs](https://github.com/zed-industries/zed/blob/main/crates/theme/src/styles/colors.rs)）。

feature crate 的用法在 `project_panel` 中可直接看到：`use ui::{ContextMenu, DecoratedIcon, ListItem, Tooltip, WithScrollbar, prelude::*};`（[project_panel.rs](https://github.com/zed-industries/zed/blob/main/crates/project_panel/src/project_panel.rs)）——**feature 只 import 组件，不 import 样式值**。

### 4.2 组件的写法与契约（以 Button 为例）

`Button` 定义（[button.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/components/button/button.rs)）：

```rust
#[derive(IntoElement, Documented, RegisterComponent)]
pub struct Button {
    base: ButtonLike,
    label: SharedString,
    label_color: Option<Color>,
    label_size: Option<LabelSize>,
    selected_label: Option<SharedString>,
    start_icon: Option<Icon>,
    end_icon: Option<Icon>,
    key_binding: Option<KeyBinding>,
    alpha: Option<f32>,
    truncate: bool,
    loading: bool,
}
```

要点：

- **props 是纯数据 + 语义枚举**：颜色不直接传 `Hsla`，而是 `Option<Color>`（语义色）/ `ButtonStyle::Tinted(TintColor::Accent)`；`ButtonStyle` 枚举含 `Filled/Tinted(TintColor)/Outlined/OutlinedGhost/...`，`TintColor` 再映射到 `Color::{Accent, Error, Warning, Success}`（[button_like.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/components/button/button_like.rs)）。
- **复合/通用件显式标注风险**：`ButtonLike` 文档："A button-like element that can be used to create a custom button when prebuilt buttons are not sufficient. **Use this sparingly, as it is unconstrained and may make the UI feel less consistent.**"（[button_like.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/components/button/button_like.rs)）——这是"反过度通用组件"的一手依据。
- **事件 = 回调**：`.on_click(|event, window, cx| ...)`（[button.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/components/button/button.rs)）。GPUI 允许，因为 `cx` 提供了"回调期间可安全访问 App 状态"的受限借用；**egui 没有对等机制**，所以 Kairos 应改用返回值（`Response`/`Output`）。
- **横切能力用 trait**：`traits::{clickable, disableable, fixed, styled_ext, toggleable, visible_on_hover}`（[ui.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/ui.rs)、[prelude.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/prelude.rs)），避免每个组件重复 `.disabled()/.selected()` 语义。
- **prelude 收口**：`use ui::prelude::*` 同时提供 gpui 原语、组件（`Button`/`IconButton`/`SelectableButton`、`Label`/`LoadingLabel`、`h_flex`/`v_flex`、`h_group`/`v_group` 等）、theme trait（`ActiveTheme`）与宏（[prelude.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/prelude.rs)）。→ Kairos 可有一个小而稳的 `widgets::prelude`，但**不要把 core 的 Action 放进 prelude**。

### 4.3 GPUI 的 Element / Render / RenderOnce：与 egui 的映射

GPUI 的 `Element` trait（[element.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/element.rs)）：

- "Elements are the workhorses of GPUI. They are responsible for laying out and painting all of the contents of a window."；`Element: 'static + IntoElement`，关联类型 `RequestLayoutState`/`PrepaintState`，方法 `request_layout`/`prepaint`/`paint`。
- 文档明确劝退直接实现："most of the time, you won't need to implement your own elements … it's recommended that you use those [built-in elements] to construct `components`, using the `RenderOnce` trait and the `#[derive(IntoElement)]` macro. Only implement elements when you need to take manual control of the layout and painting process."（[同文](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/element.rs)）
- `Render`：有状态 view 的渲染入口，"An object that can be drawn to the screen. This is the trait that distinguishes 'views' from other entities."，`fn render(&mut self, window, cx) -> impl IntoElement`。
- `RenderOnce`：无状态组件入口，"You can derive `IntoElement` on any type that implements this trait. It is used to construct reusable components out of plain data. … RenderOnce allows you to invoke this pattern, without breaking the fluent builder pattern"，`fn render(self, window, cx)`（**consume self**）。

**映射到 egui**：`Render` ≈ 面板（有借用的 `&Frame` + `&mut UiOut`）；`RenderOnce` ≈ `egui::Widget`（consume self）。结论：Kairos 的 widget kit **只对应 RenderOnce 那一层**；面板不是 widget，面板是"渲染一棵 widget 树的函数"。

### 4.4 画廊与测试：`component` crate

Zed 专门有一个 `component` crate："This module provides the Component trait, which is used to define components for **visual testing and debugging**."（[component.rs](https://github.com/zed-industries/zed/blob/main/crates/component/src/component.rs)）。机制：

- 派生宏 `#[derive(RegisterComponent)]` + `inventory` 分布式注册（`inventory::collect!(ComponentFn)`）；`ComponentMetadata` 含 `preview: fn(&mut Window, &mut App) -> AnyElement`、`scope: ComponentScope`、`status: ComponentStatus`（[component.rs](https://github.com/zed-industries/zed/blob/main/crates/component/src/component.rs)）；
- `Component` trait 文档："Implement this trait to define a UI component. This will allow you to derive `RegisterComponent` on it, in turn allowing you to preview the contents of the preview fn in `workspace: open component preview`."；`ComponentStatus` 有 `WorkInProgress`/`EngineeringReady`/`Deprecated`/`Live`（[component.rs](https://github.com/zed-industries/zed/blob/main/crates/component/src/component.rs)）；
- 画廊布局 `ComponentExample { variant_name, description, element, width }` + `example_group`（[component_layout.rs](https://github.com/zed-industries/zed/blob/main/crates/component/src/component_layout.rs)）；
- 交互/逻辑测试走 `#[gpui::test]` + `TestAppContext`（确定性 executor、可多 context 模拟协作）（[test.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/test.rs)）；`ui` crate dev-deps 开 `gpui/test-support`（[Cargo.toml](https://github.com/zed-industries/zed/blob/main/crates/ui/Cargo.toml)）。

**对 Kairos 的启示**：画廊不是"额外爱好"，而是组件库的**索引**：每个 widget 一个 preview + 一个 status；面板开发时先在画廊中确定视觉，再接 core。测试走 kittest，而不是靠画廊截图人工比对。

### 4.5 Zed 模式取舍表

| Zed 模式 | 对 Kairos | 理由 |
|---|---|---|
| `theme` 独立 crate（语义色 + schema + 多主题注册） | **部分抄**：不新建 crate，`core::StyleState` + `ui::style` 即可 | Kairos 主题只需 dark/light + TOML，不需要主题市场；见 [README.md §3.10](../plan/editor-ui-architecture/README.md) |
| `ui` crate + prelude + components 目录 | **抄结构**：widgets / components 两层 + prelude | 依赖方向可由模块可见性强制 |
| `ButtonStyle`/`TintColor` 语义枚举 | **抄**：variant enum 替代 per-instance 颜色 | 允许 per-instance 颜色 = 样式重复的根源 |
| `on_click` 回调 props | **不抄**：改用 `Response`/`Output` | egui 无 `cx` 受控借用；回调会迫使 `FnMut`/`RefCell` |
| `ButtonLike` 通用底件 + "use sparingly" | **抄警告**：通用件必须有文档边界，且不进 gallery 主推 | 防止"框架化组件" |
| `component` crate 画廊 + status | **抄**：`widgets/gallery` 示例 app + `status` 标注 | 团队评审与迁移期的可视清单 |
| `gpui::test` 多 context 确定性测试 | **映射**：`egui_kittest` 的 `Harness` + reducer 纯测试 | 无需照搬框架 |

---

## 5. iced 0.14：widget = 纯渲染描述，逻辑在 update

`iced` 的定位："a cross-platform GUI library focused on simplicity and type-safety. **Inspired by Elm.**"（[crate 文档](https://docs.rs/iced/latest/iced/index.html)）。widget 模块第一句："Use the built-in widgets or create your own."（[widget 模块](https://docs.rs/iced/latest/iced/widget/index.html)）；内建控件从 `iced_widget` 再导出（Button/Checkbox/Column/Container/TextEditor/…），说明"控件库"与"运行时"分离。

自定义 widget 的契约（`advanced` feature，[Widget trait](https://docs.rs/iced/latest/iced/advanced/widget/trait.Widget.html)）：

- 必选方法只有三个：`size()`、`layout()`、`draw()`；
- 其余全是 provided：`size_hint/tag/state/children/diff/operate/update/mouse_interaction/overlay`；
- `update(...)` 的出口是 `Shell<'_, Message>`——**widget 只能产生 `Message`，不能改应用状态**；
- 官方示例 `custom_widget`（画一个圆）里，`Circle { radius }` 只实现 `size/layout/draw`，没有任何领域逻辑（[example 源码](https://github.com/iced-rs/iced/blob/master/examples/custom_widget/src/main.rs)）。

应用层：`iced::application(new, update, view)`，`view(&State) -> Element<Message>`，`update(State, Message) -> Task`；theme 也是 `fn theme(&State) -> Theme`（[crate 文档 · Theming](https://docs.rs/iced/latest/iced/index.html)）。这正是 [README.md §2.1](../plan/editor-ui-architecture/README.md) 里的 MVU 形态。

**对 Kairos 的两条硬结论**：

1. **"widget 不允许改状态"在 iced 是框架强制的（只能 `Shell<Message>`），在 egui 只能靠约定**。因此 Kairos 必须在类型上人为制造同构约束：widget 的签名里不出现 `&mut EditorState`/`&mut UiOut`/`&mut World`；只出现只读 props + `&mut Ui`。
2. **widget 的"局部状态"不是领域状态**：iced 用 `Tree`（widget 状态树）承载滚动/焦点等；egui 对等物是 `Memory`/`Id`。两边都说明：widget 内部状态只能是"交互瞬态"。


---

## 6. 面向 Kairos 的 widget kit 契约（15–25 个）

> 本节综合前五节的一手证据，给出可直接用于 [rewrite-plan.md](../plan/editor-ui-architecture/rewrite-plan.md) Phase 1 的契约建议。前提：`kairos_editor_ui` 前期可作为 `kairos_editor` 的模块目录，稳定后再抽 crate（[README.md §3.1](../plan/editor-ui-architecture/README.md)）。

### 6.1 三层模型

```
L0 tokens   core::StyleState (TOML 纯数据) ──ui::style 映射──▶ egui::Style/Visuals + UiTokens
L1 widgets  kairos_editor_ui::widgets        只读 props + &mut Ui → Response / Output
L2 components kairos_editor_ui::components   懂 domain：&Frame + &mut UiOut + ViewState → Action
L3 shell    app/runtime                       dispatch/reduce/effects（不认识 widget 具体类型）
```

| 层 | 位置 | 允许依赖 | 禁止依赖 | 状态 | 测试 |
|---|---|---|---|---|---|
| L0 tokens | `core::StyleState` + `ui::style` | `serde`、`egui::Style/Visuals` | widgets/components、`UiOut`、`World` | 可序列化、可持久化、可被 MCP 读 | 普通 Rust 单测（映射函数） |
| L1 widgets | `ui::widgets`（建议单模块 + 子文件） | `egui`、`ui::tokens`、`ui::icons` | `core::{state,action,effect}`、`World`、`UiOut`、`std::fs` | 只允许 `Memory` 级瞬态；props 里的 `&'a T` 只读 | kittest + 可选图像快照；gallery |
| L2 components | `ui::components`（面板内组合） | L0 + L1 + `Frame`/`UiOut`/core 类型 | `EffectExecutor`、IO | `ViewState`（keyed `PanelInstanceId`） | kittest（fake `Frame`，断言 Action） |
| L3 shell | `app.rs`/`host.rs`/`runtime.rs` | 全部 | 不出现具体 widget 类型 | `EditorState` 所有权 | reducer 单测 + 集成 smoke |

**依赖方向是唯一硬约束**：`core ← ui(widgets→components) ← app`。widgets 层不 import core，是可以用 `grep`/CI 检查的（与 [README.md §5](../plan/editor-ui-architecture/rewrite-plan.md) 的质量门一致）。

### 6.2 widgets 的统一签名

推荐三件套（按复杂度选择，不要求所有 widget 长一样）：

```rust
// 形态 A：原子控件 —— builder + impl Widget（egui 原生习惯）
pub struct Button<'a> { /* 只读 props + 语义 variant */ }
impl Widget for Button<'_> { fn ui(self, ui: &mut Ui) -> Response; }        // 直接 ui.add(Button::new(...))

// 形态 B：复合控件 —— builder + show() -> Output
pub struct TreeRow<'a> { /* props */ }
pub struct TreeRowOutput { pub response: Response, pub event: Option<TreeRowEvent> }
impl TreeRow<'_> { pub fn show(self, ui: &mut Ui) -> TreeRowOutput; }

// 形态 C：带内部状态的控件 —— 显式 id + 状态读写（禁止自持跨帧字段）
pub struct TextField<'a> { id_salt: Id, /* props */ }
```

**props 设计规则**：

1. **只读数据 + 语义枚举**：`label: impl Into<WidgetText>`（或 0.35 的 `impl IntoAtoms`）、`variant: ButtonVariant`、`selected: bool`、`enabled: bool`；不传 `Color32`/`f32 spacing`/`&Theme`。egui 0.35 的 `Button::new(impl IntoAtoms)` 就是"文本/图标统一为 Atoms"的官方示例（[button.rs 源码](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/widgets/button.rs)）；`WidgetText`/`RichText`/`Atoms` 都把"排版"延迟到绘制期，因此本地化字符串在调用方构造即可（[WidgetText](https://docs.rs/egui/0.35.0/egui/widget_text/enum.WidgetText.html)、[RichText](https://docs.rs/egui/0.35.0/egui/widget_text/struct.RichText.html)、[Atoms](https://docs.rs/egui/0.35.0/egui/struct.Atoms.html)）。
2. **需要一个稳定身份**：凡是可能多实例/可开合/可滚动/有 popup 的 widget，必须带 `id_salt`（内部 `ui.push_id(id_salt, ...)`）。egui 文档对 `push_id` 的例子就是"同标题循环会 Id clash"（[ui.rs 源码](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/ui.rs)）。
3. **不要回调 props**：egui 的 `Response` 已经是事件出口（`clicked/changed/hovered` + `on_hover_text`）；复合控件用 `Output { response, event }`。回调会引入 `FnMut` 迭代/借用问题，和 Kairos "禁止 `Cell<Option<FnOnce>>` 回调"的既定方向冲突（[README.md §1.2/§3.10](../plan/editor-ui-architecture/README.md)）。
4. **enabled 语义**：简单控件直接用 `ui.add_enabled(cond, widget)`；需要自定义视觉态的控件在 props 里收 `enabled: bool` 并自行设置 `Sense`。不要两套并存。
5. **图标走 provider**：widget 只收 `IconId`（语义枚举）+ size；纹理由 `ui::HostView` 提供（[README.md §3.4](../plan/editor-ui-architecture/README.md)），widgets 不直接 `include_bytes` 资源。参照 re_ui 的 `Icon`（"embedded in the binary at compile time"）与 Zed 的 `IconName`/`Icon` 组件分工（[icons](https://docs.rs/re_ui/0.41.0/re_ui/icons/index.html)、[prelude.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/prelude.rs)）。
6. **文本本地化在调用方**：widget 收 `impl Into<WidgetText>`（或 `IntoAtoms`），**不查翻译表**；组件层从语言资源拿字符串后传入。这样 widget 单测不需要 locale fixture。

**输出事件设计**：优先"只有 `Response`"，当一次交互有多个语义动作时才加 `Output.event`；事件枚举必须小（1–5 个变体）、`#[non_exhaustive]` 可选、不含领域类型（如 `OpenAsset(AssetPath)` 属于组件层翻译，不属 widget 事件）。示例：

```rust
pub enum TreeRowEvent { Select, ToggleExpand, BeginRename, OpenContextMenu, ActivateDefault }
```

**DoD（每个 widget 的完成定义）**：

- [ ] `widget_info` 已设置（role + label/selected/value），kittest 可查询（[WidgetInfo](https://docs.rs/egui/0.35.0/egui/struct.WidgetInfo.html)）；
- [ ] 多实例安全（`id_salt` 或调用方 `Id`）；
- [ ] 无 core 依赖、无回调 props、无颜色/spacing 参数；
- [ ] hover/focus/disabled/selected 四态视觉来自 tokens；
- [ ] gallery 有 preview + 变体示例；
- [ ] 至少 1 个行为测试（点击/键盘/输出事件），可选 1 张快照。

**命名与文件约定**（建议写进 `ui::widgets/mod.rs` 文档头，review 时逐条对照）：

- 类型用名词：`Button`、`IconButton`、`TreeRow`；**不要**加 `Widget` 后缀（会和 `egui::Widget` trait 混淆），**不要**加 `Component` 后缀（层名不进类型名）。
- props/输出/变体：`XxxProps<'a>`、`XxxOutput`、`XxxEvent`、`XxxVariant`、`XxxSize`、`ColorRole`（共享枚举），例如 `ButtonProps<'a>` + `ButtonVariant::Primary`。
- 构造：`Button::new(...)` 收必选 props，builder 方法收可选 props；复合控件 `TreeRow::show(self, ui)`；需要身份的控件把 `id_salt` 放构造参数（`TextField::new(id_salt, ...)`）。
- 文件：每个 widget 一个 singular snake_case 文件（`button.rs`/`icon_button.rs`/`tree_row.rs`），测试放同文件 `#[cfg(test)] mod tests`；`widgets/mod.rs` 只做 `mod`+`pub use`，`widgets/gallery.rs` 与 `widgets/test_support.rs` 独立（后者 `#[cfg(any(test, feature = "test_support"))]`）。
- gallery/preview id：`"button/primary"`、`"tree-row/selected"`（稳定字符串，可直接用作快照文件名前缀）。
- 禁止在 widget 名里出现领域词（`AssetRow`、`InspectorField` 属于 components；widget 层叫 `TreeRow`/`PropertyRow`）。

### 6.3 建议的 24 个 widget 清单

按"原子 → 复合 → 复合容器"分组；数量刻意控制在 24 组（同一行里的相近控件可拆成独立类型，实际类型约 26 个），避免第一版做成框架。

**Atoms（10）**

| widget | 关键 props | 输出 | 状态 | 测试 |
|---|---|---|---|---|
| `Button` | label(atoms)、variant(primary/secondary/ghost/outline)、selected、enabled、icon | `Response` | 无 | click/disabled + 1 快照/变体 |
| `IconButton` | icon、tooltip、selected、enabled | `Response` | 无 | click + tooltip |
| `Toggle` / `Checkbox` | checked、label、enabled | `Response` | 无 | click 改变 checked |
| `RadioGroup<T>` | options、selected | `Response`（内部改为 `Option<T>` 由调用方存） | 无 | 键盘/点击 |
| `TextField` | id_salt、text（`&mut String` 由调用方持有）、placeholder、multiline | `TextEditOutput` 风格 `Output` | `Memory`（光标/选中） | 输入事件 + id 隔离 |
| `NumericField` | value（`&mut f32/i32`）、range、step、precision | `Response` + changed | `Memory`（拖拽中） | 拖拽/键入 |
| `Slider` | value、range、step、orientation | `Response` | `Memory`（拖拽中） | 拖拽 |
| `ComboBox<T>` | id_salt、options、selected、label | `Output { response, event: Option<T> }` | `Memory`（popup 开合） | 选择项 |
| `ColorSwatch` | color、selected、interactive | `Response` | 无 | click |
| `Separator` / `Spinner` / `Progress` | horizontal/vertical、fraction | `Response`（无交互） | 无 | 快照 |

**Molecules（8）**

| widget | 关键 props | 输出 | 状态 | 测试 |
|---|---|---|---|---|
| `SectionHeader` | title、collapsed、help、actions(右槽) | `Output { response, toggled }` | `Memory`（折叠动画） | click 折叠 |
| `TabBar` | tabs、selected、closable、id_salt | `Output { response, event: TabBarEvent }` | `Memory`（hover/drag） | 切换/关闭 |
| `Toolbar` | groups（`&[ToolbarGroup]`）、overflow | `Output`（悬浮展开项） | 无 | 溢出布局 + click |
| `ListItem` | id_salt、selected、hovered、icon、label、trailing | `Output { response, event: ListItemEvent }` | 无（状态由调用方） | 点击/双击/右键 |
| `TreeRow` | `&TreeRowProps`（depth、expanded、selected、leaf） | `TreeRowEvent` | 无 | 展开/选择 + id 隔离 |
| `PropertyRow` | label、value 渲染槽（`impl FnOnce(&mut Ui)` 泛型参数，**渲染槽≠事件回调**） | `InnerResponse` | 无 | 布局/对齐快照 |
| `FormRow` / `ControlRow` | label、items(atoms) | `Response` | 无 | 快照 |
| `SearchField` | text、id_salt、on-change 由调用方检查 | `Output { response, cleared }` | `Memory` | 输入 + 清空 |

**Composites（6，仍属 widgets，但内部组合多个 atom）**

| widget | 关键 props | 输出 | 状态 | 测试 |
|---|---|---|---|---|
| `ModalScaffold` | id、title、min/max 宽高、可滚动 | `ModalScaffoldOutput`（含 `should_close`） | `Option<ModalScaffold>` 由调用方持有（参照 re_ui `ModalHandler`） | 打开/关闭/Escape + 快照 |
| `ContextMenu` | anchor response、items | `Option<MenuItemClick>` | egui `Popup` memory | 右键打开 + 点击项 |
| `MenuBar` | menus、items | `Output` | egui `Popup` memory | 菜单遍历 |
| `CommandPalette` | query、items（`&[CmdItem]`）、selected | `Output { response, accepted }` | `Memory`（输入 + 滚动） | 键入过滤 + 回车 |
| `Table` / `VirtualGrid` | id、columns、rows、row_height、selection | `Output { response, event: TableEvent }` | `Memory`（滚动/列宽） | 滚动/排序 |
| `StatusPill` / `Banner` | severity、text、actions | `Response` | 无 | 快照 + 无交互 |

> 不在 widgets 层的：`DockingTabStrip`（保留现有 dock，只改泛型参数，[rewrite-plan.md §Phase 1](../plan/editor-ui-architecture/rewrite-plan.md)）、`InspectorPanel`/`ProjectPanel`（components/panels）、`ViewportFrame`（component + render handle）。

### 6.4 components 的契约

```rust
pub fn inspector_panel(
    ui: &mut Ui,
    frame: &Frame<'_>,          // &EditorState + &World + tokens/host 资源（只读）
    view: &mut InspectorViewState, // 纯数据、Default；key = PanelInstanceId
    out: &mut UiOut,             // 唯一出口：收集 Action
);
```

规则：

1. **只有 components 允许看到 `EditorState/SelectionState/DocumentsState`**，也只有它能构造 `Action`。widget 事件 → Action 的翻译表写在 component 文件里（可单测）。
2. **components 不持有跨帧状态**：需要跨帧的临时态一律 `ViewState`；需要持久化/undo/MCP 可见的走 core（[README.md §3.4](../plan/editor-ui-architecture/README.md)）。`Cell`/`RefCell`/`Mutex` 禁止出现在面板代码（[rewrite-plan.md §5](../plan/editor-ui-architecture/rewrite-plan.md)）。
3. **components 不直接执行副作用**：保存/IO/引擎变更一律 `Action` → reducer → `Effect`（[README.md §3.2](../plan/editor-ui-architecture/README.md)）。
4. **多实例**：`ViewState` 以 `PanelInstanceId` 为 key；两个 Inspector 各自展开状态不共享；共享的是 core 的 selection/document。

翻译示例：

| widget 输出 | component 翻译 |
|---|---|
| `TreeRowEvent::Select` | `Action::Selection(SelectAsset { path })` |
| `TreeRowEvent::BeginRename` | 不改 core；`view.renaming = Some(path)`，随后 TextField 提交时发 `Action::Project(Rename { from, to })` |
| `ModalScaffoldOutput::should_close` + 确认按钮 | `Action::Modal(Confirm)` → reducer 产生后续 Effect |
| `TableEvent::SortColumn` | `Action::Inspector(SetSort)`（若排序属于持久视图偏好则进 core；否则 ViewState） |

### 6.5 瞬态状态归属决策表

| 状态例子 | 归属 | key / 生命周期 | 依据 |
|---|---|---|---|
| hover/focus/scroll/文本光标/popup 开合/折叠动画 | `egui::Memory`（widget 内部） | `Id`（`push_id` 生成）；重启可丢 | egui Memory 文档："NOT meant to store any important data"（[源码](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/memory/mod.rs)） |
| 面板级展开项、重命名输入缓冲、列宽/排序、选中行 | `ui::ViewStates` | `PanelInstanceId`；`Default`；不持久化（除非单独持久化） | [README.md §3.4](../plan/editor-ui-architecture/README.md) |
| 选中资产/实体、打开文档、dirty/undo、modal 数据、样式值 | `core` | 单例 `EditorState`；可序列化 | 同上；modal 必须数据化（[README.md §3.10](../plan/editor-ui-architecture/README.md)） |
| 相机/视口尺寸/渲染句柄 | `Engine.world` / `ui::HostView` | 引擎/GPU 生命周期 | [README.md §3.4](../plan/editor-ui-architecture/README.md) |
| 每帧派生（可见行、过滤结果） | 不存，帧内计算 | — | 单向数据流；避免缓存失效 |

**判据三问**（源自 README，推荐写进 review checklist）：重启后应恢复吗？需要 undo 或 MCP 读取吗？多个面板/多实例需要共享吗？任一为"是" → core；否则 ViewState；widget 内部交互瞬态 → Memory。

### 6.6 主题与 token 的具体规则

1. Kairos 建一个语义 token 类型（建议 `ui::UiTokens`，字段数 30–60，不要 158）：`bg_panel`、`bg_surface`、`bg_hover`、`bg_selected`、`text_default/subdued/disabled`、`accent`、`danger/warn/success`、`stroke_divider`、`radius_*`、`spacing_*`、`row_height_*`、`icon_size_*`。
2. `core::StyleState`（TOML 纯数据）→ `ui::style::apply_to_ctx(ctx, &style_state)`：生成 dark/light 两套 `egui::Style`（`set_style_of`）+ 两套 `UiTokens`；运行期切换用 `Context::set_theme`（[context.rs 源码](https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/context.rs)、[Theme](https://docs.rs/egui/0.35.0/egui/enum.Theme.html)）。
3. widget 通过 `UiTokens::of(ui)`（extension trait）读取；**props 不接受颜色/间距**，只接受 variant。需要强调色时用 `ColorRole::{Default, Accent, Danger, …}` 枚举。
4. 允许的例外：视口背景/缩略图底色等"内容相关颜色"可以由 component 传显式颜色，但必须从 tokens 派生（`tokens().viewport_background`），不能 `from_rgb`。
5. dark/light + 图标主题（icon theme）都要在 tokens 层有槽位；不要在 widget 里 `if dark { ... } else { ... }`。

### 6.7 测试策略（四级金字塔）

```
L4 面板/组件交互（kittest + fake Frame/UiOut）：关键路径，数量少      ← 每个 Phase 的验收
L3 gallery（examples/widget_gallery）：人工/周期截图，不进 CI golden
L2 widget 行为测试（kittest，AccessKit 查询）：每个 widget 1–3 个
L1 widget 快照（wgpu 离屏）：只给视觉关键/易回归的 10–20 个
```

- **L1**：`Harness::new_ui(|ui| widget.show(ui))`、固定尺寸、`harness.snapshot("widget_name/default")`；快照文件小（≤ 320×200）、阈值统一放 `kittest.toml`；只覆盖"状态 × variant 矩阵"的代表项（如 Button primary/disabled/hovered）。不要给每个面板快照。
- **L2**：用 AccessKit 查询（`get_by_label`/`get_by_role`）+ `click()/type_text()` + `harness.run()`；断言 widget 输出事件或 `Response` 状态。**注意 label 是测试 API**：改文案会破坏测试，所以测试里用英文/固定文案，或优先用 role + index。
- **L3**：gallery 注册所有 widget + status（Ready/WIP/Deprecated），作为设计评审入口；参照 Zed `component` crate 与 `ComponentStatus`（[component.rs](https://github.com/zed-industries/zed/blob/main/crates/component/src/component.rs)）。可定期生成截图报告，但不作为 CI 门。
- **L4**：fake `Frame`（构造最小 `EditorState`）+ `UiOut`，对组件跑 kittest，断言产生的 `Action` 序列；这是 [README.md §3.11](../plan/editor-ui-architecture/README.md) 里的"交互测试"落点。测试 fixture 放 `ui::test_support`。
- **CI 阈值**：借用 re_ui 的形态，在 `ui::test_support` 里提供 `default_snapshot_options_for_ui()`，统一 threshold/max_failed_pixels，避免每个测试各写一套（[re_ui testing 源码](https://docs.rs/re_ui/0.41.0/src/re_ui/testing/mod.rs.html)）。

### 6.8 反模式清单

| # | 反模式 | 为什么坏 | 一手证据 | Kairos 规则 |
|---|---|---|---|---|
| A1 | 领域类型/领域 Action 泄漏进 widget | widget 无法脱离 core 复用，测试要构造整个 `EditorState` | re_ui 自身对 `re_entity_db` 的依赖是"被容忍的例外"（[crates.io](https://crates.io/crates/re_ui)） | widgets 层 CI 禁 import core/world |
| A2 | widget 持有长生命周期状态（展开集合、dirty、选中） | 多实例/多视图共享失效，生命周期与面板错位 | egui Memory 文档"NOT meant to store any important data"；Kairos 现有"窗口即模型"教训（[README.md §1](../plan/editor-ui-architecture/README.md)） | 状态按 §6.5 归属 |
| A3 | props 爆炸（10+ bool/颜色/回调参数） | 调用点不可读，行为组合爆炸 | Zed `Button` 用 `ButtonStyle`/`ButtonSize`/`TintColor` 收口语义变体（[button_like.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/components/button/button_like.rs)） | 超过 ~7 个 props 时提炼 `XxxProps` 结构体/变体枚举 |
| A4 | 过度通用的"框架组件"（万能 Button、万能 Table） | 约束消失，UI 一致性下降 | Zed `ButtonLike`："Use this sparingly, as it is unconstrained and may make the UI feel less consistent."（[button_like.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/components/button/button_like.rs)） | 通用底件必须有文档边界 + 不进 gallery 主推 |
| A5 | 回调 spaghetti（`FnMut` 塞满 props） | egui 里立即退化为 `RefCell/Cell`/闭包捕获，测试困难 | egui 用 `Response` 做出口；Kairos 现有 `Cell<Option<FnOnce>>` 是为借检查绕行（[README.md §1.2](../plan/editor-ui-architecture/README.md)） | widget 不收回调；输出 `Response/Output` |
| A6 | per-instance 样式覆盖（每个调用点传颜色/圆角） | 样式漂移、主题切换失效、偏好设置无法覆盖 | re_ui 把 widget 状态色集中进 `DesignTokens`（[DesignTokens](https://docs.rs/re_ui/0.41.0/re_ui/struct.DesignTokens.html)） | 只允许 variant/role 枚举，token 从 style 读 |
| A7 | widget 直接发全局 Action / 知道 dispatcher | 控制流再次藏进组件，测试无法只测视觉 | MVU 要求"视图产出意图，不执行"（[README.md §2.1](../plan/editor-ui-architecture/README.md)、[iced Widget](https://docs.rs/iced/latest/iced/advanced/widget/trait.Widget.html) 只产出 `Message`） | 只有 component 构造 `Action`，widget 输出自己的小事件 |
| A8 | 给所有 widget/面板拍快照 | 慢、脆、仓库膨胀 | egui_kittest 官方劝退（[crate 文档](https://docs.rs/egui_kittest/0.35.0/egui_kittest/)） | 快照限 10–20 个关键 widget |
| A9 | 重写 modal/menu/popup/dock | 无障碍、层级、输入拦截、多 pass 语义都会踩坑 | egui 0.35 已内置 `Modal`/`Popup`/`MenuBar`（[Modal](https://docs.rs/egui/0.35.0/egui/containers/modal/struct.Modal.html)）；rewrite-plan 也要求保留 dock | 原生机制优先，只做包装/样式 |
| A10 | 组件库同时承担布局引擎/停靠/业务 | 边界消失，任何单点改动影响全局 | re_ui 的边界是 theme+helpers；Zed 的 `ui` 不做布局算法（用 gpui/taffy）（[ARCHITECTURE.md](https://github.com/rerun-io/rerun/blob/main/ARCHITECTURE.md)、[ui.rs](https://github.com/zed-industries/zed/blob/main/crates/ui/src/ui.rs)） | widgets 专注"叶子视觉+交互"；布局交给 `Ui`/面板 |

### 6.9 迁移建议（与 rewrite-plan 对齐）

1. **先做 gallery app**（`examples/widget_gallery.rs`），再逐个移植面板；gallery 的每个 widget 注册 `status`（Live/WIP）。
2. **第一批只做 12 个 widget**：Button、IconButton、Toggle、Checkbox、TextField、NumericField、ComboBox、SectionHeader、TabBar、ListItem、TreeRow、PropertyRow —— 这些是 Phase 1 的 project/hierarchy/inspector/console/preferences/toolbar 的直接依赖（[rewrite-plan.md Phase 1](../plan/editor-ui-architecture/rewrite-plan.md)）。
3. **"重复三次再抽"**：组合件（Table、CommandPalette、ModalScaffold）等至少两个面板出现相同需求后再进入 widgets；先写在 component 里。
4. **旧 UI 冻结期**：旧 `kairos_editor/src/ui/**` 的绘制代码可以搬，但必须按新签名重写（`&Frame`+`&mut UiOut`），并且**先清掉 `Cell`/`Mutex`/`Arc<Mutex>`** 再搬（[rewrite-plan.md §4/§5](../plan/editor-ui-architecture/rewrite-plan.md)）。
5. **widget 验收用 kittest 写**：每搬一个面板前，先给它依赖的 widget 写 L2 行为测试；面板的 L4 测试按 README §3.11 的"headless 交互测试"落地。

---

## 7. 三层契约总表（tokens → widgets → components；附加 shell 列）

| 维度 | L0 tokens | L1 widgets | L2 components / panels | L3 shell |
|---|---|---|---|---|
| 所在 | `core::StyleState` + `ui::style` | `ui::widgets` | `ui::components`、`ui::panels` | `app.rs`/`host.rs`/`runtime.rs` |
| 允许依赖 | 纯数据 + egui Style | egui + tokens + icons | L0/L1 + core 只读类型 + `UiOut` | 全部（编排） |
| 禁止依赖 | UI/领域 | core state/action、World、IO | Effect 执行、IO | 具体 widget 类型（只按 Panel trait 调用） |
| 状态所有权 | 可序列化、单一来源 | 仅 `Memory`/Id 级瞬态 | `ViewState`（keyed `PanelInstanceId`） | `EditorState` + Effect 生命周期 |
| 主题来源 | TOML → egui Style + UiTokens | `ui.style()/`UiTokens extension | 同 L1（透传） | 初始化时 apply 一次 |
| 输出 | — | `Response` / `Output` | `Action`（经 `UiOut`） | dispatch/reduce/effect |
| 测试 | 映射函数单测 | kittest 行为 + 限量快照 | kittest（fake Frame）+ Action 断言 | reducer 单测 + smoke |
| 典型文件数 | 1–2 | 20–30 | 面板数（6–10） | 3–5 |
| 改动频率 | 低 | 中（新增控件） | 高（功能） | 低 |

---

## 8. 给 Kairos 的 10 条规则

1. **widgets 层禁止 import `kairos_editor_core::{state,action,effect}`、禁止 `World`、禁止 IO**；CI 用 `grep`/clippy 检查（与 [rewrite-plan.md §5](../plan/editor-ui-architecture/rewrite-plan.md) 的硬约束同一机制）。
2. **widget 签名 = 只读 props + `&mut Ui` + 返回值**；禁止 `&mut EditorState`/`&mut UiOut`/`&mut World`/`FnMut` 回调。
3. **状态归属三问**（重启恢复？undo/MCP 可见？多面板共享？）→ 是则 core，否则 ViewState，widget 内部交互瞬态才用 `egui::Memory`；详见 §6.5。
4. **tokens 单一来源**：`core::StyleState` → `ui::style` → `egui::Style` + `UiTokens`；widget props 只收 variant/role，不收颜色/间距；dark/light 由 `Context::set_theme`/`set_style_of` 统一切换。
5. **每个 widget 必须 `widget_info` + 稳定 `Id`**（内置 `id_salt`），并进 gallery；不满足 DoD（§6.2）不算完成。
6. **复合控件返回 `Output`，事件枚举小且不含领域类型**；widget 事件 → `Action` 的翻译只写在 component 里，可单独单测。
7. **原生机制优先**：modal 用 `egui::Modal`、菜单用 `egui::Popup`、列表/折叠用 `Ui::scope`/`CollapsingHeader`、dock 保留现实现；只做样式/数据包装，不重写基础设施。
8. **测试金字塔**：每个 widget 1–3 个 kittest 行为测试；图像快照只给 10–20 个易回归 widget（小尺寸、统一阈值）；面板交互测试用 fake `Frame`/`UiOut` 断言 Action；画廊不进 CI golden。
9. **先 12 个、重复三次再抽**：不为"未来可能需要"造通用框架；通用底件（如 `ButtonLike` 式）必须有文档边界并默认不鼓励使用。
10. **本地化与图标穿在边界上**：widget 收 `impl Into<WidgetText>/IntoAtoms` 与 `IconId`；翻译字符串与 `IconId → TextureHandle` 映射分别由 component 层与 `HostView` 提供，widget 不知道语言与资源路径。

---

## 9. 参考来源（一手）

### egui / egui_kittest（0.35.0；Kairos 当前依赖）
- Widget trait 文档：https://docs.rs/egui/0.35.0/egui/trait.Widget.html
- Widget trait 源码（builder/consume self/闭包 impl）：https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/widgets/mod.rs
- Button 源码（Atoms/builder）：https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/widgets/button.rs
- Response / widget_info / AccessKit：https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/response.rs
- WidgetInfo / WidgetType：https://docs.rs/egui/0.35.0/egui/struct.WidgetInfo.html · https://docs.rs/egui/0.35.0/egui/enum.WidgetType.html
- Ui::scope / push_id / add：https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/ui.rs
- Memory::data 边界：https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/memory/mod.rs
- widget_style（WidgetStyle/Classes/WidgetState）：https://docs.rs/egui/0.35.0/egui/widget_style/index.html
- Theme / Context::set_theme/set_style_of：https://docs.rs/egui/0.35.0/egui/enum.Theme.html · https://github.com/emilk/egui/blob/0.35.0/crates/egui/src/context.rs
- egui::Modal：https://docs.rs/egui/0.35.0/egui/containers/modal/struct.Modal.html
- egui_kittest：https://docs.rs/egui_kittest/0.35.0/egui_kittest/ · Harness：https://docs.rs/egui_kittest/0.35.0/egui_kittest/struct.Harness.html
- 版本事实：crates.io egui 最新 0.37.0（https://crates.io/crates/egui）；egui_kittest 0.35.0 依赖 kittest 0.4 / accesskit 0.24.1（https://docs.rs/egui/0.35.0/egui/）

### Rerun re_ui（0.41.0，依赖 egui 0.37）
- crate 首页/模块与 re-export：https://docs.rs/re_ui/0.41.0/re_ui/ · 源码：https://docs.rs/re_ui/0.41.0/src/re_ui/lib.rs.html
- DesignTokens（158 字段）：https://docs.rs/re_ui/0.41.0/re_ui/struct.DesignTokens.html
- HasDesignTokens：https://docs.rs/re_ui/0.41.0/re_ui/trait.HasDesignTokens.html
- design_tokens_of：https://docs.rs/re_ui/0.41.0/re_ui/fn.design_tokens_of.html
- try_set_design_tokens（下游主题覆盖）：https://docs.rs/re_ui/0.41.0/re_ui/fn.try_set_design_tokens.html
- ListItem：https://docs.rs/re_ui/0.41.0/re_ui/list_item/struct.ListItem.html · 模块：https://docs.rs/re_ui/0.41.0/re_ui/list_item/index.html
- modal.rs 源码（ModalHandler/ModalWrapper）：https://docs.rs/re_ui/0.41.0/src/re_ui/modal.rs.html
- menu.rs 源码（menu_style/StyleModifier）：https://docs.rs/re_ui/0.41.0/src/re_ui/menu.rs.html
- ControlRow：https://docs.rs/re_ui/0.41.0/re_ui/struct.ControlRow.html · re_form：https://docs.rs/re_ui/0.41.0/re_ui/re_form/index.html
- icons：https://docs.rs/re_ui/0.41.0/re_ui/icons/index.html
- testing 模块：https://docs.rs/re_ui/0.41.0/re_ui/testing/index.html · 源码：https://docs.rs/re_ui/0.41.0/src/re_ui/testing/mod.rs.html
- Rerun 架构与 crate 边界：https://github.com/rerun-io/rerun/blob/main/ARCHITECTURE.md
- 版本与依赖：https://crates.io/crates/re_ui

### Zed ui / GPUI / theme / component（main，2026-10 抓取）
- ui crate 文档与模块：https://github.com/zed-industries/zed/blob/main/crates/ui/src/ui.rs · Cargo：https://github.com/zed-industries/zed/blob/main/crates/ui/Cargo.toml
- prelude：https://github.com/zed-industries/zed/blob/main/crates/ui/src/prelude.rs
- Button：https://github.com/zed-industries/zed/blob/main/crates/ui/src/components/button/button.rs · ButtonLike/ButtonStyle：https://github.com/zed-industries/zed/blob/main/crates/ui/src/components/button/button_like.rs
- theme crate：https://github.com/zed-industries/zed/blob/main/crates/theme/src/theme.rs · 语义色：https://github.com/zed-industries/zed/blob/main/crates/theme/src/styles/colors.rs
- component（画廊）：https://github.com/zed-industries/zed/blob/main/crates/component/src/component.rs · 布局：https://github.com/zed-industries/zed/blob/main/crates/component/src/component_layout.rs
- GPUI Element/Render/RenderOnce：https://github.com/zed-industries/zed/blob/main/crates/gpui/src/element.rs
- GPUI 测试：https://github.com/zed-industries/zed/blob/main/crates/gpui/src/test.rs
- feature crate 组合示例：https://github.com/zed-industries/zed/blob/main/crates/project_panel/src/project_panel.rs

### iced（0.14.0）
- crate 文档（Inspired by Elm / theming）：https://docs.rs/iced/latest/iced/index.html
- widget 模块：https://docs.rs/iced/latest/iced/widget/index.html
- advanced Widget trait：https://docs.rs/iced/latest/iced/advanced/widget/trait.Widget.html
- advanced 模块：https://docs.rs/iced/latest/iced/advanced/index.html
- 自定义 widget 示例：https://github.com/iced-rs/iced/blob/master/examples/custom_widget/src/main.rs
- 作者哲学（Elm 影响）：https://book.iced.rs/philosophy.html

### 通用 UI 范式
- Dan Abramov, Presentational and Container Components（含 2019 更新）：https://medium.com/@dan_abramov/smart-and-dumb-components-7ca2f9a7c7d0
- React lifting state up：https://react.dev/learn/sharing-state-between-components
- React prop drilling/context：https://react.dev/learn/passing-data-deeply-with-context
- Brad Frost, Atomic Design ch.2：https://atomicdesign.bradfrost.com/chapter-2/
- Microsoft MVVM：https://learn.microsoft.com/en-us/dotnet/architecture/maui/mvvm
- Flutter StatelessWidget：https://api.flutter.dev/flutter/widgets/StatelessWidget-class.html · StatefulWidget：https://api.flutter.dev/flutter/widgets/StatefulWidget-class.html
- W3C DTCG Format 2025.10：https://www.designtokens.org/TR/2025.10/format/ · 草案：https://www.designtokens.org/tr/drafts/format/
- Headless UI：https://headlessui.com/

### 仓库内上下文（非代码）
- [docs/plan/editor-ui-architecture/README.md](../plan/editor-ui-architecture/README.md)（§3.1 crate 布局、§3.4 状态归属、§3.10 样式、§3.11 测试）
- [docs/plan/editor-ui-architecture/rewrite-plan.md](../plan/editor-ui-architecture/rewrite-plan.md)（Phase 1/5、ci 约束）
- [docs/research/rust-ui-architecture-survey.md](./rust-ui-architecture-survey.md)（框架层对比）
- [docs/research/editor-ui-architecture-patterns.md](./editor-ui-architecture-patterns.md)（编辑器层对比）

---

## 10. 局限与未验证项

1. **未运行任何代码**：本文所有 API 结论来自 docs.rs/源码阅读，未编译验证；widget kit 的最终形态由 [prototypes/editor-ui](../../prototypes/editor-ui) 合并原型验证（对应 task-8/9，49 个 headless 测试）。
2. **版本错位**：re_ui 0.41 面向 egui 0.37，Kairos 固定 egui 0.35；`Atoms`/`widget_style`/`Modal` 在 0.35 中确实存在（本文已按 0.35 文档核对），但**不要照搬 re_ui 的 egui 0.37 用法**。egui 最新为 0.37.0（2026-10），升级评估不属本文范围。
3. **Rerun `ReUi` 历史**：任务描述中的 `&ReUi` 在当前 0.41 源码里不存在；本文未考证它是何时/如何被替换为全局 `DesignTokens` 的（未查到公开迁移说明），因此 §3.2 只陈述"现状"，不陈述"历史"。
4. **Zed main 快照**：Zed 仓库结构与文件路径可能随 main 变化（本文按 2026-10 抓取）；GPUI 未发布 crates.io，无法用 docs.rs 固定版本。
5. **未审计 Kairos 代码**：按任务要求只读架构文档，未检查现有 widget 代码；[editor-ui-widget-catalog.md](./editor-ui-widget-catalog.md)（task-7）给出实际代码清单，两者需对照。
6. **图像快照的平台差异**：kittest/re_ui 都靠阈容忍字体渲染差异；Kairos 若上 CI，需要先验证 Linux/macOS 的阈值，本文未实测。
7. **未覆盖**：键盘导航/focus scope 细节、IME、拖拽（drag & drop）在 egui 0.35 的具体 API、虚拟化表格性能，这些应在 widget 原型中单独验证（README §3.3.1 的 focus scope 要求同样适用）。

