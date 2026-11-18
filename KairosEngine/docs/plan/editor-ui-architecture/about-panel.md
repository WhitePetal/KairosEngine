# About 窗口迁移评审：`kairos_editor::AboutWindow` → 0.2.0 `panels/about`

> **状态**：评审 + 实现（2026-11-18）
> **范围**：把 0.1.0 的 `kairos_editor/src/ui/about_window.rs`（181 行）迁到 0.2.0 的 `kairos_editor_app` 面板架构。本文只讨论 About：它在旧实现里**应该做而没做**的事、**做了但不该那样做**的事，以及新架构下它该长成的形状。
> **配套**：[ADR 0008](../adr/0008-editor-ui-architecture.md)（单向数据流）· [ADR 0009](../adr/0009-editor-panel-host-and-viewport.md)（Panel/Registry/Host）· [panel-viewport-design.md](./panel-viewport-design.md) §2 · [widget-layer.md](./widget-layer.md)（Theme/Widget/Component 三层）
> **证据口径**：仓库内 `path:line` 均为本次（2026-11-18）工作区 HEAD 实读；「关窗后无法再打开」的结论是**静态推断**（已给完整调用链），未运行验证。

---

## 0. 结论摘要（TL;DR）

1. **旧 About 不是「一个窗口」，是「五条 label + 两个尺寸字段」**：`ui.heading("KairosEngine")`、`ui.label(VERSION)`、一句描述、`ui.label("TODO: add icon...")`（`about_window.rs:87-105`）。图标、许可、构建信息、链接、复制、关闭按钮全部没有。
2. **它带着四个真实缺陷**：(a) 关掉 tab 后再点 "About Kairos" 打不开（`on_close` 不调 `Drawer::close`，`actives` 仍为 true）；(b) 打开期间**每帧**写一条日志（`:96`）；(c) Preferences 选中 About 页时**每帧** `fs::write`（`:141-163` + `ui.rs:493-499`）；(d) 样式文件缺失 → `create_ui_failed` → **panic 杀编辑器**（`ui.rs:865-869`）。
3. **版本号有三处真源**：旧 About 读 `kairos_editor::consts::VERSION`（=`env!("CARGO_PKG_VERSION")` = 0.1.0，`consts.rs:2`）、0.2.0 标题栏读 `kairos_editor_app::consts::VERSION`（硬编码 `"0.2.0"`，`consts.rs:3`）、workspace 声明 0.1.0（`Cargo.toml:37`）。About 窗口必须与标题栏同源，否则用户报障时贴的版本号是假的。
4. **不能平移**：旧实现依赖 `Drawer`/`Messager`/`Message`/`UIReader`/`&Engine`/`&mut Log`/`show_window` 这些在 0.2.0 里已被删除的东西。新架构把旧的三件套拆成三处：**面板实例字段**（view state）、**`AboutPanelStyle` RON**（外观）、**`Action` → `reduce` → `Effect`**（开关窗口）。
5. **落地形状**：`kairos_editor_component/src/panels/about.rs`（`ABOUT` PanelId + `AboutPanel` + `EditorInfo`）+ `about/style.rs`（`AboutPanelStyle: Style`，`Preferences/Styles/AboutPanel.ron`）；工具栏 "About Kairos" → `LayoutAction::ShowPanel` → 浮动窗口；body 内 Close → `LayoutAction::ClosePanel`。
6. **产品身份由组合根注入**（`PanelHost::new(panels::CATALOG, PanelContext::new(info))`，工厂构造时读 `context.editor`）：组件 crate 不得依赖 app（cargo 强制）；`HostView` 是「每帧动态宿主资源」的缝；`EditorState` 是可序列化状态（没有 action 能改一个常量）。名字/版本/文案/链接由 `kairos_editor_app::consts` 提供。
7. **链接与复制不新增 Action/Effect**：`ui.hyperlink_to` / `ctx.copy_text` 只产出 egui 的 `OutputCommand::{OpenUrl, CopyText}`，app 早已把 `platform_output` 交给 `egui_winit::State::handle_platform_output`（`host.rs:169`），该函数用 `webbrowser` 打开 URL、用 arboard 写剪贴板（`egui-winit-0.35.0/src/lib.rs:1103-1115,1370-1380`，`links` 在默认 feature 里）。面板因此仍然「不碰 IO」，只是发了一条 egui 平台命令。

---

## 1. 旧实现审读

### 1.1 它实际做了什么

| 步骤 | 代码 | 事实 |
|---|---|---|
| 建实例 | `about_window.rs:57-67` | `AboutWindow { model: AboutWindowModel { style } }`：**三层 struct 包 3 个字段** |
| 读样式 | `:25-43` | `fs::read_to_string(Preferences/Styles/AboutWindowStyle.toml)`，失败即 `Err`（错误文案写着 "Json"，其实是 TOML，`:28-31`） |
| 开窗 | `ui.rs:800-829` | 首次打开：`AboutWindow::new()` → 失败走 `create_ui_failed`（原生错误框 + `ViewportCommand::Close` + `panic!`，`:865-869`）→ `push_drawer` → `add_window` → `show_window` 用样式里的 width/height `set_size`（`about_window.rs:77-86`） |
| 画 | `:87-105` | `log.info("about_window update")`（**每帧**）→ heading "KairosEngine"（**硬编码，没解析 `consts::APP_NAME`**）→ `label(consts::VERSION)` → separator → 一句描述 → `label("TODO: add icon...")` |
| 样式反射 | `:111-131` / `:133-164` | `get_style_fileds` 给 Preferences 两个 float（width/height）；`update_style` 用 **`style_fields[0]`/`[1]` 下标**读，然后 `toml::to_string` + `fs::write` |
| 关 | `:107-109` | `messager.send(Message::CloseAboutWindow)` → `close_drawer::<AboutWindow>()` 只 `actives[id]=false`（`ui.rs:855-863`） |
| 身份 | `:166-168` | `get_name() -> type_name::<AboutWindow>()` |

### 1.2 功能缺口（该有而没有）

| # | 缺口 | 证据 | 为什么算缺口 |
|---|---|---|---|
| M1 | **没有图标** | `:104` 直接写着 `"TODO: add icon..."` | 仓库早就有 `Preferences/Textures/engine_icon.png`，工具栏已经拿它当 "About" 按钮图标（`toolbar/style.rs:191-209`）；只剩窗口里是空白 |
| M2 | **版本号不是进程的版本号** | `:99` 用旧 crate 的 `CARGO_PKG_VERSION`（0.1.0）；0.2.0 标题栏用 `kairos_editor_app::consts::VERSION`（`"0.2.0"`，硬编码）；workspace 仍是 `0.1.0`（`Cargo.toml:37`） | About 的唯一职责就是「这个进程是谁」，三处真源里它偏偏选了和标题栏不同的那处 |
| M3 | **没有许可/版权** | 全文只有一句产品描述 | 仓库是 `MIT OR Apache-2.0`（workspace `Cargo.toml` 的 `license`），About 是唯一自然的展示位；现在用户只能在两个 LICENSE 文件里找 |
| M4 | **没有构建信息** | 无 | 报障第一问是「Debug 还是 Release、什么平台」；`cfg!(debug_assertions)` + `std::env::consts::{ARCH, OS}` 零成本可得 |
| M5 | **没有链接** | 无 | 仓库/讨论区/Thanks 都只在 README 里；而且旧架构**根本没有打开浏览器或写剪贴板的通道**（`Message` 57 个变体里没有 `System::OpenUrl`） |
| M6 | **没有可复制的版本串** | 无 | 提 issue 要求贴版本；egui 的 `OutputCommand::CopyText` 现成可用 |
| M7 | **body 内没有关闭入口** | 只有 tab 上的 ×（且见 B1，× 之后打不开） | 浮动窗口场景下用户更需要一个明确的 Close 按钮 |
| M8 | **没有错误态** | 样式缺失 → `panic`（`ui.rs:865-869`） | 一个显示版本号的窗口没有理由因为皮肤文件缺失杀掉编辑器；新组件的既有约定是 `StyleFile::load_or_default()` + `warn!` + 继续画（`console.rs:92-104`） |
| M9 | **零测试、零可断言语义** | 181 行 0 测试 | 全是 `ui.label` 裸文本，没有 widget id、没有 `WidgetInfo`，连「版本号画出来了」都无法断言 |

### 1.3 可优化 / 坏味道

| # | 问题 | 证据 | 代价 |
|---|---|---|---|
| B1 | **关掉 tab 就再也打不开** | dock 的 × 走 `TabDrawer::on_close`（`tab_drawer.rs:58-65`）**只返回 `Close`、不调用 `Drawer::close`** → tab 被移除，但 `actives[id]` 仍为 `true`；再次 `Message::OpenAboutWindow` 落进 `ui.rs:814-818` 的 `Some(id) + actives` 分支，`find_drawer(id)` 返回 `None` → **静默什么都不做**（静态推断，未运行验证） | 一次关窗即功能失效，直到重启编辑器 |
| B2 | **每帧写日志** | `:96` `log.info("about_window update")` | About 开着 → Console 被灌；`Log` 无界（`messager-mvc-audit.md:300`），关掉 About 再开 Console 会在单帧渲染积压的全部行 |
| B3 | **每帧写盘** | `:133-164` + `ui.rs:493-499` + `ui.rs:477-487`（Preferences 每帧重发整页） | 「开着 Preferences 选中 About 页」≈ 60 次/秒 `fs::write` + `toml::to_string` |
| B4 | **下标式样式反射** | `:133-140` `style_fields[0]`/`[1]` | 字段顺序/数量一变就是运行时 panic 或静默错配，编译器不检查（`editor-ui-widget-catalog.md:237`） |
| B5 | **三层 struct 包 3 个字段** | `:45-55`、`:57-67` | 纯粹的间接层，增减字段要改三处 |
| B6 | **身份是 Rust 类型名** | `:166-168` | 面板身份在新架构里是数据（`PanelId(&"static str")`），类型名不可序列化、不可被 MCP/布局文件寻址 |
| B7 | **标题与尺寸是可编辑的 TOML** | `AboutWindowStyle.toml` 的 `title`；`width/height` 在 Preferences 可改 | 产品文案进了皮肤文件（i18n 无处安放）；窗口尺寸是 dock 的职责，面板不该管 |
| B8 | **每次重开都 `set_size`** | `:77-86`（`show_window` 每次激活都调） | 用户拖过的尺寸被重置；`state` 为 `None` 时 `println!` 到 stdout（`:83`），而不是日志 |
| B9 | **文案与实际格式不符** | `:28-31` "Load AboutWindow Model **Json** Failed"（实际 TOML） | 排障时误导 |
| B10 | **给 Preferences 充数** | `get_style_fileds` 2 个 float + `render()` 返回 `None`（`:174-180`） | 13 个 Drawer 全部实现这两个方法，其中 9 个是空壳（`messager-mvc-audit.md:418-420`）；「每窗口自带样式反射」是这套反射机制的成本 |
| B11 | **VIEW 有副作用** | `ui()` 里 `log.info` | 违反新架构的多 pass 前提（egui 默认 `max_passes=2`，被 discard 的 pass 也执行了绘制闭包） |

### 1.4 为什么这不是一次「平行迁移」

旧 About 的实现依赖 6 样在 0.2.0 里已经不存在的东西：

| 旧依赖 | 0.2.0 现状 |
|---|---|
| `trait Drawer`（10 个方法：`ui`/`show_window`/`close`/`get_style_fileds`/`update_style`/`get_name`/`get_title`/`render`/`create`/`on_rect_changed`） | `trait Panel`（`panel.rs:56-117`）：`kind`/`icon`/`title`/`init`/`ui`/`context_menu`/`on_rect_changed`/`closeable`/`on_close`/`allowed_in_windows`/`scroll_bars` |
| `&mut Messager` + `Message::OpenAboutWindow` | `UiOut` + `Action::Layout(LayoutAction::ShowPanel)` → `reduce` → `LayoutEffect` → `EffectExecutor` |
| `UIReader`（面板互相读） | `Frame { state, host }`（只读快照） |
| `&Engine` / `&mut Log` 参数 | 无：面板不碰引擎；日志走 `kairos_log` |
| `show_window` 里 `set_size` 管窗口尺寸 | 尺寸归 `kairos_editor_docking` 的 `WindowState`；app 只决定新窗口的落点（`effects.rs:81-93` `floating_rect`） |
| per-window 样式 TOML + 反射 | per-component `Style` + `StyleFile` RON（`kairos_editor_widgets/src/style.rs:64-142`），`Panel::init` 里载入 |

**结论**：About 必须按新架构重写，而不是搬到 `kairos_editor_app` 里继续用 `egui::Window`。后者会在 0.2.0 里造出第二套窗口模型（dock 之外的浮窗），这正是 ADR 0008/0009 要消灭的东西。

---

## 2. 新形状

### 2.1 落点：一个 Panel，不是 modal

- `panels::ABOUT: PanelId = PanelId("about")`，声明在 `panels::CATALOG`（`PanelDef { id, factory }` 的编译期表，没有启动期注册步骤）；`AboutPanel { info, style }`。
- **打开语义**：工具栏 "About Kairos" → `Action::Layout(LayoutAction::ShowPanel { panel: ABOUT })` → reducer 打开布局并返回 `LayoutEffect::ShowPanel` → `EffectExecutor::show_panel`：dock 里没有它的 tab 且 `allowed_in_windows()` → `open_in_window(id, floating_rect)`（`effects.rs:43-57`）。**这就是 0.2.0 的「About 窗口」**：一个独立浮动窗口，不需要新的 modal 层，也不需要 `egui::Window`。
- **关闭语义**：body 的 Close 按钮 → `LayoutAction::ClosePanel { panel: ABOUT }`；tab 的 × 走 `PanelHostViewer::on_close`（`panel/viewer.rs:78-93`），推的是同一个 action。下一帧 `PanelHost::sync` 回收实例、`EditorApp::sync` 摘掉 dock tab（空浮动窗口随之销毁）。
- **不做的**：不把 About 塞进启动布局（`app.rs:142-150` 的 5 个默认面板），不新增 `ModalState`，不记忆窗口尺寸。

### 2.2 身份从哪来：`EditorInfo` 由组合根注入

```rust
// kairos_editor_component/src/panel/context.rs
pub struct EditorInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub license: &'static str,
    pub repository: &'static str,
    pub discussions: &'static str,
    pub credits: &'static str,
}
```

被否掉的四个位置：

| 候选 | 否掉的理由 |
|---|---|
| 面板里写 `env!("CARGO_PKG_VERSION")` / 硬编码 | 正是 M2：组件 crate 的版本是 workspace 的 0.1.0，和标题栏 / 产品版本号不是一回事；谁编译这些字符串，谁就决定了 About 说什么 |
| `HostView::about()` | `HostView` 的契约是「每帧动态的宿主资源」（日志流、profiler、将来的视口纹理，`frame.rs:96-118`）；静态身份放进去等于给常量开一条每帧通道 |
| `EditorState.info` | core 是「可序列化状态 + 唯一写者 `reduce`」；常量身份没有任何 action 能改，放进去只会让 core 背产品文案 |
| `kairos_editor_core::EditorInfo` | 今天只有一个消费者；等 MCP 的 `get_editor_state` 也要读它时再上移到 core（见 §6） |

它落在 `kairos_editor_component/src/panel/context.rs`（框架层），不在 `panels/about.rs`：`PanelDef` 的工厂签名是 `fn(&PanelContext) -> Box<dyn Panel>`，而 `panel` 不能反向依赖某个具体面板模块。唯一的注入点因此在构造成本：`PanelHost::new(panels::CATALOG, PanelContext::new(info))`，About 的工厂读 `context.editor`，`AboutPanel::new(info)` 拿到的仍是 app 的那一份。没有闭包捕获，也没有启动期注册。app 侧只有一个常量文件：

```rust
// kairos_editor_app/src/consts.rs
pub const APP_NAME: &str = "KairosEngine";
pub const VERSION: &str = "0.2.0";          // 与标题栏同源，改一处两处都变
pub const DESCRIPTION: &str = "...";
pub const LICENSE: &str = "MIT OR Apache-2.0";
pub const REPOSITORY_URL: &str = "https://github.com/WhitePetal/KairosEngine";
```

**构建信息**（Debug/Release、arch-os）由面板自己算：`cfg!(debug_assertions)` + `std::env::consts`。这是「正在运行的二进制」的属性，组件与 app 同一次编译，不存在第二真源。

### 2.3 外观：`AboutPanelStyle`（RON，dark/light）

`Preferences/Styles/AboutPanel.ron`，实现 `kairos_editor_widgets::style::Style`（`FILE`/`dark()`/`light()`），`Panel::init` 里 `StyleFile::load_or_default()` + 按 `ctx.theme()` 选 skin，失败 `warn!` 后继续用代码默认值（M8）。

**主题由组合根钉死为 Dark**（`EditorApp::new` 调 `ctx.set_theme(Theme::Dark)`，在 `PanelHost::sync` 跑 `init` 之前）。当前引擎的 chrome（dock 的 `Style`、工具栏皮肤、widget 默认值）全是暗色，而面板按 `ctx.theme()` 选皮肤 —— 不钉的话，浅色桌面上就会出现「暗色窗口里嵌一块白色面板」，About 正是这个 bug 的显形处。各组件 RON 里的 light 皮肤保留，等 dock 有了浅色皮肤再一起启用。

放进去的：颜色（背景/标题/副标题/正文/键列/值/链接/分隔线）、字号、logo 尺寸、行高、键列宽、内边距、按钮 token（投影成 `ButtonStyle`）、图标路径（logo + tab icon，`init` 里 `ui_image::preload`）。

**不放进去的**：产品文案（名字/描述/许可/链接）。样式是外观，文案是身份 —— 旧实现把 `title` 放进 TOML（B7），结果是皮肤文件变成了文案真源，i18n 无处落地。

### 2.4 交互：四条通道，各自有主

| 交互 | 通道 | 为什么 |
|---|---|---|
| 打开 | `LayoutAction::ShowPanel` → `LayoutEffect::ShowPanel` → `EffectExecutor::open_in_window` | 布局意图归 core，dock 操作归 app（ADR 0009 决策 3）；面板自己不碰 dock |
| 关闭 | `LayoutAction::ClosePanel` | 与 tab × 完全同一条路径；不新增「关闭自己」的特权 |
| 链接 | `ui.hyperlink_to` → egui `OutputCommand::OpenUrl` → `egui_winit` `webbrowser::open` | 平台输出通道已存在且 app 已在转发（`host.rs:169`）；新增 `SystemAction::OpenUrl` 就要新增 Effect、新 host 执行器，而浏览器窗口不是编辑器状态 |
| 复制版本串 | `ctx.copy_text` → `OutputCommand::CopyText` → `egui_winit` 剪贴板 | 同上 |

四者共同的约束仍然成立：**`ui()` 里零副作用**，只发 `Action` / egui 平台命令。

### 2.5 内容清单（画什么，以及为什么不画别的）

```text
[logo 72px]  KairosEngine                     <- EditorInfo::name
             Version 0.2.0                    <- EditorInfo::version
             Debug - x86_64-windows           <- cfg! + std::env::consts（更淡的一行）
             A pure Rust game engine ...      <- EditorInfo::description（自动换行）
--------------------------------------------
License    MIT OR Apache-2.0                  <- EditorInfo::license
--------------------------------------------
Repository WhitePetal/KairosEngine            <- hyperlink（空串则整行不画）
Discussions ...                               <- hyperlink
Credits    Thanks.md                          <- hyperlink
--------------------------------------------
[Copy version info]                    [Close]
```

版面预算：默认浮动窗口 537x344（`floating_rect` 在 1280x820 下取 42%），减去 dock 窗口标题栏 28 px 后可用 316 px；正文自然高度约 230 px，因此**不会出现滚动条**（有测试钉住，见 §5）。这也是把 Build 从独立行下移到身份块第三行的原因：Build/License 各占一行的旧版式在 460 px 宽的窗口里要 ~300 px，压到边缘。

- **不画 git commit**：仓库没有 build script，没有可信来源；编一个假 hash 比不画更糟。
- **不画 GPU/后端名**：那是引擎的 `wgpu::Adapter` 信息，属于引擎查询（将来走 `HostView`/引擎接口），不是面板该伸手去拿的。
- **不画「检查更新」**：没有更新服务器。

**按钮不是文字**：`Copy version info` / `Close` 用 `ButtonStyle` 的 fill + 1 px 描边（`button_fill`/`button_border`，hover 变亮、按下变暗）。只靠文字颜色的按钮在面板底色上会被当成 label —— 本迁移为此给 widget kit 加了 `ButtonStyle::stroke`（默认 `Stroke::NONE`，菜单栏/工具栏的平铺按钮外观不变）。

---

## 3. 文件级实现清单

| 文件 | 变更 | 职责 |
|---|---|---|
| `kairos_editor_component/src/panel/context.rs` | 新增 | `EditorInfo` + `PanelContext`：工厂的构造期上下文（框架层，不认识具体面板） |
| `kairos_editor_component/src/panel/catalog.rs` | 新增 | `PanelDef`/`PanelCatalog`：编译期面板表，替代运行期 `PanelRegistry` |
| `kairos_editor_component/src/panels/about.rs` | 新增 | `AboutPanel`（`Panel` impl）、`DEF` 工厂、布局与交互、单测 |
| `kairos_editor_component/src/panels/about/style.rs` | 新增 | `AboutPanelStyle: Style` + `AboutIconPaths` + `button_style()` 投影 |
| `kairos_editor_component/src/panels.rs` | 改 | `ABOUT` id；`register_default` 换成 `panel_catalog!` 宏（一个条目同时生成 `mod`、id 和 `CATALOG` 行）；模块 doc 说明 About 是第一个真面板 |
| `kairos_editor_component/src/lib.rs` | 改 | 导出 `EditorInfo`/`PanelContext`；模块 doc 增一行 About |
| `kairos_editor_component/src/toolbar.rs` | 改 | `todo!()` → `LayoutAction::ShowPanel { ABOUT }` |
| `kairos_editor_component/tests/panel_host.rs` | 改 | catalog 完整性、7 个默认 kind、About 图标存在性、未打开的面板不被构造 |
| `kairos_editor_widgets/src/button.rs` | 改 | `ButtonStyle::stroke`（默认 `Stroke::NONE`）：对话框按钮的 1 px 描边；菜单栏/工具栏按钮不受影响 |
| `kairos_editor_widgets/src/ui_image.rs` | 改 | 修掉一个 Windows-only 断言（`file:///D:/…` 反解后不是 `Path::is_absolute`），与本迁移无关但挡着这个 crate 的测试门 |
| `kairos_editor_widgets/src/fonts.rs` | 改 | `EditorFonts.ron` 改走 `style_root()`：新增的 app 测试会调用 `EditorApp::new`（它会装字体），原实现把该文件写进进程工作目录 |
| `kairos_editor_app/src/consts.rs` | 改 | 产品身份常量（名字/版本/文案/许可/链接） |
| `kairos_editor_app/src/app.rs` | 改 | 构造 `EditorInfo`，包成 `PanelContext` 注入 `PanelHost`；集成测试：`ShowPanel` 让 About 浮起来 |
| `Preferences/Styles/AboutPanel.ron` | 新增 | 参考皮肤（dark + light），与代码默认值逐字段相等的往返测试钉住 |

**不变**：`kairos_editor_core`（0 行）、`kairos_editor_docking`（0 行）。`kairos_editor_widgets` 只加了 `ButtonStyle::stroke` 一个可选 token（默认无描边，平铺的菜单/工具栏按钮外观不变）——这正是「加一个面板 = 一个 impl + 一个 `DEF` + `CATALOG` 一行」的验收点，widget kit 的扩展只发生在确有必要时。

---

## 4. 旧 → 新逐项对照

| 旧 | 新 | 处理 |
|---|---|---|
| `AboutWindow`/`AboutWindowModel`/`AboutWindowStyle`（3 个 struct） | `AboutPanel` + `AboutPanelStyle` | B5 消解：panel 持 view 数据，style 是独立可序列化类型 |
| `fs::read_to_string` + `toml::from_str`（硬失败） | `StyleFile::load_or_default()` + `warn!` | M8 消解 |
| `get_style_fileds`/`update_style`（下标 + 写盘） | 删除 | B3/B4/B10 消解：Preferences 将来按 schema 编辑 style，不再「每窗口反射」 |
| `show_window` 里 `set_size` | 删除 | B8 消解：尺寸归 dock，新窗口落点由 `floating_rect` 决定 |
| `Message::OpenAboutWindow`/`CloseAboutWindow` | `LayoutAction::{ShowPanel, ClosePanel}` | 统一到 layout 域，和 Console/Inspector 同一条路 |
| `type_name::<AboutWindow>()` | `PanelId("about")` | B6 消解 |
| `label(consts::VERSION)`（旧 crate 版本） | `EditorInfo::version`（app 注入，与标题栏同源） | M2 消解 |
| `"TODO: add icon..."` | `style.icons.logo` + `ui_image::paint` | M1 消解 |
| 每帧 `log.info` | 无 | B2/B11 消解 |
| 只有 tab × | body 内 Close 按钮 + tab ×（同一条 action） | M7 消解；B1 在新架构里不存在（第 2 次 `ShowPanel` 会建新实例并浮起来） |
| —— | License / Build / 链接 / Copy | M3-M6 补齐 |

## 5. 测试与验收

**组件单测（`cargo test-crate kairos_editor_component`）**

1. 注入身份后，名字/版本/许可都出现在本帧的 `TextShape` 里（M2/M3/M9）。
2. 点 Close → `UiOut` 里恰好一条 `LayoutAction::ClosePanel { ABOUT }`。
3. 点 Copy → `platform_output.commands` 里一条 `CopyText`，内容含名字与版本。
4. 点仓库链接 → `platform_output.commands` 里一条 `OpenUrl`，URL 正确，且 `UiOut` 为空（M5）。
5. `Preferences/Styles/AboutPanel.ron` 能解析且逐字段等于 `StyleFile::<AboutPanelStyle>::defaults()`（参考皮肤不会静默漂移）。
6. 正文自然高度 ≤ 316 px（460/537 两种宽度）——默认浮动窗口不会一开就出滚动条、把 Close 挤到视野外。
7. 页脚两个按钮**画了填充与描边**（不是纯文字）——见 §2.5。
8. `tests/panel_host.rs`：默认 kind 7 个，About 的 tab 图标文件存在。

**app 集成测**

9. `ShowPanel { ABOUT }` → 出现第二个 surface（浮动窗口），主 surface 的 tab 数不变；再 `ClosePanel` → 回到 1 个 surface。
10. 系统是浅色主题时，`EditorApp::new` 之后 `ctx.theme()` 仍是 `Dark`（主题钉死，面板不会选中 light 皮肤）。

> 注意：`kairos_editor_app` 只有 bin target，`cargo test-crate kairos_editor_app`（内部是 `cargo test --lib …`）会报 "no library targets found"。用 `cargo test -p kairos_editor_app --bins --tests --no-fail-fast`。

**静态门**：`cargo check -p kairos_editor_component --all-targets`、`cargo check -p kairos_editor_app --all-targets`；`cargo tree -p kairos_editor_component | grep kairos_editor_app` 必须为空（分层不变量）。

## 6. 已知边界 / 后续

1. **版本号单一真源（需要 owner 决策）**：把 `kairos_editor_app::consts::VERSION` 改回 `env!("CARGO_PKG_VERSION")` 并把 `[workspace.package] version` 提到 `0.2.0`，版本号就只剩一处。本次**不动**：这会把仍未退役的 `kairos_editor`（0.1.0 shell）一起改名，是一次跨 crate 的发版决定，不属于 About 迁移。
2. **`W04 KeyValueRow`**：本次的「键—值」行是 about.rs 里的私有 helper。等第二个消费者（Preferences/Inspector 表单）出现时再升到 `kairos_editor_widgets`，避免为一个消费者设计通用 API。
3. **`Panel::preferred_window_size`**：About 理想尺寸约 420×300，当前吃 `floating_rect` 的 42%（1280×820 下约 537×344）。为一个面板加 trait 方法不划算，等第二个需要固定尺寸的面板一起做。
4. **Preferences 编辑器**：About 的 skin 目前只能改 RON；schema 化的样式编辑随 Preferences 面板一起落。
5. **`View → About` 菜单项**：Unity 放在 Help 下；本仓库菜单栏还没有 Help 分类，先挂在 "Kairos" 菜单（与旧版一致）。
6. **浅色皮肤仍是休眠状态（需要 owner 决策）**：About 的主题已由组合根钉为 Dark（见 §2.3），所以现在不会出现白色面板；真正启用浅色需要 dock 的 `Style` 与工具栏皮肤一起做浅色版，否则又是「暗窗嵌白板」。各组件 RON 里的 light 段先留着。
7. **tab body 的 style 不继承 app 的全局设置**：`Docker::show_body` 用 `Ui::new` 从 ctx 的全局 style 造 body Ui（`kairos_editor_docking/src/docker.rs:1558-1562`），因此 `EditorApp::ui` 里的 `selectable_labels = false`（`app.rs:251`）传不到面板里 —— 表现在 dock 面板的文本可被拖动选中。About 在自己 `ui()` 里关掉；根治要在 dock 或 `PanelHostViewer` 一侧统一关，属 dock 侧修复。

## 7. 证据

- 旧实现：`kairos_editor/src/ui/about_window.rs:19,25-43,45-55,57-67,77-86,87-105,107-109,111-131,133-164,166-168,170-172,174-180`
- 旧窗口生命周期：`kairos_editor/src/ui.rs:94-95,462-467,800-829,855-863,865-869`
- 旧样式反射入口：`kairos_editor/src/ui.rs:477-487,493-499`
- 旧 tab 关闭：`kairos_editor/src/ui/docking_tab/tab_drawer.rs:58-65`（`OnCloseResponse::Close`，不调 `Drawer::close`）
- 旧菜单入口：`kairos_editor/src/ui/tool_bar.rs:160-161`
- 旧样式文件：`Preferences/Styles/AboutWindowStyle.toml`（`title/width/height`）
- 新面板协议：`kairos_editor_component/src/panel.rs:56-117`、`panel/host.rs:44-72`、`panel/viewer.rs:78-93`
- 新开关窗口：`kairos_editor_core/src/action.rs:22-34`、`effect.rs:28-38`、`reduce/layout.rs:10-38`、`kairos_editor_app/src/effects.rs:43-93`
- 新样式协议：`kairos_editor_widgets/src/style.rs:64-142`、`kairos_editor_component/src/panels/console.rs:92-110`
- 平台输出转发：`kairos_editor_app/src/host.rs:169`、`egui-winit-0.35.0/src/lib.rs:1103-1115,1370-1380`
- 版本真源：`Cargo.toml:37`、`kairos_editor/src/consts.rs:1-2`、`kairos_editor_app/src/consts.rs:1-3`
