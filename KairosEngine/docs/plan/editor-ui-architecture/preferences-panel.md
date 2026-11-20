# Preferences 窗口迁移评审：`kairos_editor::PreferencesWindow` → 0.2.0 `panels/preferences`

> **状态**：评审 + 实现（2026-11-19）；UI 重做为 Unity Inspector 样式，并加入自有 LDR/HDR 取色器（2026-11-19 续）；颜色值改为 8 字节 linear `Color` + `ColorProperty` 字段策略（2026-11-19 再续）；取色器改为 260 px 浮动窗、HDR 0-Inf/191 曝光基准、Library swatch groups，menu button/浮层菜单对齐 Unity popup field（2026-11-19 完善）；标题栏改为 dock 同款三 traffic-light widget + 居中标题，收起/最大化灰色禁用（2026-11-19）；Float/Int/String/Toggle 值框抽成 `kairos_editor_widgets::field` 的可复用 widget，页面只剩 StyleValue 映射（2026-11-20）；每个控件一个文件，数字改为拖 label scrub，值框皮肤独立成共享样式 `EditorFieldStyle`（2026-11-20 续）
> **范围**：把 0.1.0 的 `kairos_editor/src/ui/preferences_window.rs`（414 行）与它依赖的样式反射通道（`ui_style_fields.rs`、`ui.rs:468-514`）迁到 0.2.0 的面板架构。目标与旧窗口一致：**把各个组件的 `Preferences/Styles/*.ron` 以 UI 控件的形式列出来，直接在 Preferences 里改**。
> **配套**：[about-panel.md](./about-panel.md)（同一套评审口径）· [ADR 0008](../adr/0008-editor-ui-architecture.md) · [ADR 0009](../adr/0009-editor-panel-host-and-viewport.md) · [widget-layer.md](./widget-layer.md) · [editor-ui-widget-catalog.md](../../research/editor-ui-widget-catalog.md) §C17
> **证据口径**：`path:line` 为本次（2026-11-19）工作区 HEAD 实读；「每帧写盘」「多 pass 重复发消息」沿用 `docs/research/editor-ui-messager-mvc-audit.md` 的静态推断口径（该文已给调用链）。

---

## 0. 结论摘要（TL;DR）

1. **旧窗口的功能是「样式字段编辑器」**：左侧页面列表、右侧 `egui::Grid` 的字段表单，改一个值立刻作用于对应窗口。本文保留这个功能，换掉它的机制。
2. **旧机制有四个硬伤**：(a) 字段按 **下标** 读写（`style_fields[0..3]`、`pages[style_page.id]`、`drawers[style_page.id]`）——重排字段就是运行时 panic 或静默写错；(b) body 末尾**每帧**无条件 `send(UpdateUIStyle(page))`（`preferences_window.rs:404`）→ 三个窗口的 `update_style` 直接 `toml::to_string` + `fs::write`，约 60 次/秒；(c) 页面列表是 **Drawer 镜像**：遍历全部 drawer 生成页面，13 个实现里 4 个有字段，还把布局容器和 Preferences 自己列进去；(d) 视图在 `ui()` 里发消息，而 `Grid` 首帧会 `request_discard`（egui-0.35.0/src/grid.rs:454），多 pass 下同一帧重复写盘。
3. **新机制：字段由类型自己给出**。每个 `*Style` derive `StyleFields`（新增 derive 宏），宏从结构体字段生成**点分路径**（`"icons.warning"`、`"toolbar_height"`）和按类型决定的控件种类；新增/重排字段不需要改第二份表，也不会出现「下标对不上」。
4. **改值 = 立即写进面板自己的 live style**：面板实现 `Panel::set_skin_field`，app 的 effect executor 调用它；下一 pass 面板就用新样式重绘 —— 这就是旧窗口的「立即生效」。
5. **落盘 = 一次交互一次**：`StyleAction::SetField` → `Effect::Style(SetField)`（每帧，预览）；`StyleAction::Commit` → `Effect::Style(Persist)`（指针抬起 / 切页 / 关闭面板时一次）。写盘从「帧」变成「手势」，B1 消解。
6. **面板列表 = 活着的面板**：0.2.0 里样式属于面板实例（`Panel::init` 载入），没打开的面板没有实例可改。页面列表因此来自 `PanelHost::skins()` 的实时快照；Preferences 自己也在列表里（可以改自己的皮肤）。
7. **不迁移的东西**：窗口尺寸/页面列表本身不可编辑（B6/B7）；不存在空页与自指页（M9）；没有「打开时重新收集」这一步，页面表是每次绘制的快照。

---

## 1. 旧实现审读

### 1.1 它实际做了什么

| 步骤 | 代码 | 事实 |
|---|---|---|
| 读样式 | `preferences_window.rs:17-45` | `Preferences/Styles/PreferencesWindowStyle.toml`（`paths.rs:8`）：`title/default_width/default_height/grid_space_x/grid_space_y`；读失败返回 `Err`，错误文案写的是 “Json”（`:28-41`），实际是 TOML |
| 建实例 | `:47-80` | `PreferencesModel { style, style_pages: Option<Vec<StylePage>>, selected_id: Option<usize> }`；`selected_id` 初始 `None`（`:60`）→ 首屏 “Select a Setting...” |
| 开窗 | `ui.rs:468-473` | `show_window::<PreferencesWindow>`（首次构造失败走 `create_ui_failed`）→ 再投递 `RefershPreferenceWindow` |
| 收集页面 | `ui.rs:477-487` | 遍历 **所有** `self.drawers`：`drawer.get_style_fileds()` → `StylePage::new(id, drawer.get_name(), fields)`；id 是 `drawers` 的下标 |
| 页面标题 | `preferences_window.rs:150-152` | `type_name::<T>()`，用户看到的是 `kairos_editor::ui::about_window::AboutWindow` |
| 画 | `:194-413` | 左侧 `selectable_label` 页面列表（`:199-212`）→ 竖分隔线 → `ui.vertical_centered` 里 heading + 横线 + `egui::Grid`（`:229-234`）逐字段分派（6 种字段，`:236-401`） |
| 回写 | `:404` + `ui.rs:493-514` | 每帧整页 `send(UpdateUIStyle(page))`；handler 调 `update_style_page` + `drawers[id].update_style(...)`（其中 3 个窗口直接写盘） |
| 选中页 | `:207` + `ui.rs:488-492` | 点列表 → 发 `SetPreferenceWindowSelectedId(page.id)` |
| 关 | `:110-112` + `ui.rs:474-476` | `close()` 会发 `ClosePreferenceWindow`，但 **body 里没有任何按钮调用它**；实际只有 tab 的 × |

### 1.2 功能缺口（该有而没有）

| # | 缺口 | 证据 | 为什么算缺口 |
|---|---|---|---|
| M1 | **菜单入口未接** | `tool_bar.rs:160-161` 发 `OpenPreferenceWindow`；0.2.0 的 “Edit → Preferences” 曾是 `todo!()`（本次任务） | 迁移的一半是把它接回菜单 |
| M2 | **body 里没有关闭按钮** | `:110-112` 的 `close()` 无调用点 | 浮动窗口场景下用户需要一个明确出口 |
| M3 | **没有「恢复默认值」** | 无 | 改了就落盘（对 3 个窗口而言），手滑无法回退 |
| M4 | **没有字段说明/单位/范围** | 字段名是小写 `&'static str`（`:118-145`） | 用户看到 “grid space x 40”，不知道单位、不知道边界、也无法翻译 |
| M5 | **没有搜索/分组** | 单列表 + 单表格 | 12 个条目里 8 个空页，也没有分类 |
| M6 | **选中页不持久** | 每次打开新实例 `selected_id: None`（`:60`） | 首屏永远是 “Select a Setting...” |
| M7 | **没有撤销/重置单个字段** | 无 | 与 M3 同源 |
| M8 | **零测试** | 414 行 0 测试 | 表格式 UI 完全可以用无头测试钉住 |
| M9 | **空页与自指页** | `ui.rs:477-487` 遍历全部 drawer：3 个 `Layout*Container`、8 个 `Vec::new()`、以及 Preferences 自己 | 页面列表不是「可设置的东西」，而是「窗口对象的镜像」 |

### 1.3 可优化 / 坏味道

| # | 问题 | 证据 | 代价 |
|---|---|---|---|
| B1 | **每帧整页回传 + 每帧写盘** | `:404`（无条件 send）、`ui.rs:493-499`、`tool_bar.rs:305-327` / `scene_window.rs:333-356` / `about_window.rs:141-163` | 「开着 Preferences 选中 ToolBar 页」≈ 60 次/秒 `toml::to_string` + `fs::write` |
| B2 | **视图持有整页 clone 再回写** | `:219` `style_pages[*id].clone()`、`:190` `copy_from_slice` | 回写的是上一帧快照；字段数量一变，`copy_from_slice` 直接 panic |
| B3 | **下标耦合** | `style_fields[0..n]`（各窗口 `update_style`）、`pages[style_page.id]`（`:190`）、`drawers[style_page.id]`（`ui.rs:497`） | 增删/重排字段 = 运行时 panic 或静默错配，编译器不检查 |
| B4 | **VIEW 有副作用** | `ui()` 里 `messager.send`（`:207`、`:404`） | egui 默认 `max_passes=2`，`Grid` 首帧 `request_discard`（egui-0.35.0/src/grid.rs:454）→ 被丢弃的 pass 也会发消息，同帧重复写盘 |
| B5 | **皮肤文件缺失即致命** | `:27-45` → `ui.rs:821-823` → `ui.rs:865-869`（原生错误框 + `ViewportCommand::Close` + `panic!`） | 一个设置窗口没有理由因为皮肤缺失杀掉编辑器 |
| B6 | **窗口尺寸来自样式并每次打开重置** | `:90-97` `state.set_size(default_width, default_height)` | 用户拖过的尺寸在重开时被重置；尺寸属于 dock |
| B7 | **Preferences 可编辑自己的尺寸/间距** | `:114-148`，且 `update_style` 只改内存（`:154-167`，不写盘） | 改的是「下次打开时」的值，当前窗口无变化，重启后丢失 |
| B8 | **文案与实现不符** | `:28-41` 写着 “Json” | 排障误导 |
| B9 | **无校验** | `:154-167` 直接接受任意 `f32` | DragValue 的范围只在 UI 侧；越界/NaN 值无条件落盘 |
| B10 | **三层 struct 包 5 个字段** | `:17-80` | 增减字段要改三处；与 About 的 B5 同源 |

### 1.4 为什么这不是一次「平行迁移」

| 旧依赖 | 0.2.0 现状 |
|---|---|
| `trait Drawer`（`get_style_fileds`/`update_style`/`close`/`show_window`/...） | `trait Panel` + 新的 `skin`/`set_skin_field`/`save_skin` 三个方法，字段由类型自己描述 |
| `&mut Messager` + `Message` | `UiOut` + `Action::Style`（按领域切片） |
| 每窗口自持样式 + `update_style` 写 TOML | 每组件 `StyleFile` RON（`kairos_editor_widgets/src/style.rs:64-142`）+ 面板自己的 `save_skin` |
| `ui.rs` 遍历 `drawers` 生成页面 | `PanelHost::skins()` 的实时快照（只有活着的面板） |
| `Drawer::show_window` 里 `set_size` | dock 的 `WindowState`；Preferences 不再管理任何窗口尺寸 |
| `StyleField` 值类型（`Clone/Copy`）+ 手写下标 | `StyleValue`（core）+ 点分路径 + 类型化读写 |

**关键判断**：功能保留，机制替换。旧窗口的「收集所有窗口的字段」在 0.2.0 里变成「面板自己描述自己」：样式属于组件，组件才知道自己有哪些 token、每个 token 是什么类型。

---

## 2. 新形状

### 2.1 落点：一个 Panel

- `#[crate::panel(PREFERENCES = "preferences", skin = PreferencesPanelStyle)]` 声明在 `panels/preferences.rs` 的 `impl Panel` 上：id 常量、`PanelRegister`、工厂、`inventory` 提交和三个 skin 方法都由这一行生成（`panels::catalog()` 收集）。页面标题取自 `Panel::title`，不再手写。
- **打开**：工具栏 “Edit → Preferences” → `LayoutAction::ShowPanel { PREFERENCES }` → dock 无 tab 时浮成窗口（与 About 同一条路）。
- **关闭**：body 的 Close → `LayoutAction::ClosePanel`；tab × 走 `Panel::on_close`（先提交未保存编辑，再发同一个 action）。

### 2.2 字段从哪来：`#[derive(StyleFields)]`

新 crate `kairos_editor_component/macros`（proc-macro）为样式结构体生成两个 impl：

```text
StyleNode::collect(path, out)      // 把本节点的 token（或子节点）追加到字段表
StyleNode::write(path, tail, value) // 按点分路径写一个叶子
StyleFields::style_fields()        // 入口：collect("")
StyleFields::set_style_field(path, value)
```

- 每个字段生成 `"父路径.字段名"` 和一个 `match` 分支；嵌套结构体（`icons`）自己也要 derive，于是 `icons.warning` 自然出现，父结构体不需要知道 `icons` 是分组。
- **控件种类由字段类型决定**：`f32` → Float（DragValue，速度按数量级）、`u8/u16/u32/usize` → Integer（带类型上限）、`bool` → Toggle、`String` → Text、`Color` → Color、`ColorProperty` → Color 或 HDR Color（看 usage）、`Vec<T>` → 按下标展开（Profiler 的 `category_palette`），`#[style(skip)]` 排除非 token 字段。
- 旧实现的「字段名 + 手工范围」不见了：名字来自字段名（`toolbar_bg` → “Toolbar bg”），范围来自类型。
- `#[SharedStyle(CONST = "id", title = "Page")]` 是可选声明：额外生成 id 常量、crate-private seal 和 `SharedStyleDef` 提交；类型只写 `preload`。见 §2.5。

**为什么用 derive 而不是手写字段列表**：手写列表每加一个 token 就要同步一次，正是旧 `get_style_fileds`/`update_style` 漂移的根源。宏在编译期从结构体本身生成，漏写不可能发生。

### 2.3 值怎么写进去：面板自己的 live style

```text
控件改变
  → StyleAction::SetField { panel, path, value }
  → reduce（style slice，纯路由）
  → Effect::Style(SetField)         ← 下一帧 sync 执行
  → PanelHost::set_skin_field(panel, path, value)
  → Panel::set_skin_field → style.set_style_field(path, value)
  → 下一 pass 用新样式重绘
```

- 样式不是 `EditorState`：它属于面板实例（`Panel::init` 里从 RON 载入）。所以 reducer 只做路由（core 不认识 egui 组件），真正写入发生在 app 的 effect executor 里；面板对 `bool` 返回值负责（路径不存在或类型不匹配 → 什么都没发生，记一条 warn）。
- **同一帧内为什么能看到自己的编辑**：Preferences 面板持有该页字段表的本地副本，控件改的是这份副本；宿主上的 live style 由 effect 在下一帧写入，但页面显示不依赖它。代价是若别处改了同一面板的样式（目前没有这种入口），页面要切页后才刷新。

### 2.4 文件什么时候写：一次手势一次

```text
拖动一帧:  SetField  → Effect::Style(SetField)     只改内存
抬起指针:  Commit    → Effect::Style(Persist)      Panel::save_skin → Preferences/Styles/*.ron
切页/关窗: Commit（由面板发）                      同上
```

- 面板的规则：**有未保存编辑 + 指针抬起** → 发 `Commit`；切页前、关闭前各提交一次（`Panel::on_close` 兜底键盘输入）。
- `Panel::save_skin` 只写**当前主题那一半**：先读文件的另一半，替换本半再写回，所以编辑 dark 不会丢掉手改过的 light；文件不存在就用代码默认值补一份。
- shared 样式（Toolbar / Color Picker）走同一套手势边界，但 effect 是 `Effect::SharedStyle`：`SetField` 写 `SharedStyles` 的 live 值，`Persist` 写 `<Style>.ron`，`Reload` 重读文件并 bump revision。它们没有面板实例，所以 action 的目标是 `SharedStyleId` 而不是 `PanelId`。
- `SetField` 不写 console 日志（一次拖动 60 行），`Commit` 写；`describe` 仍为每个 action 提供人读文本。

### 2.5 共享样式页：Toolbar / Color Picker / Field

工具栏和取色器不是 `Panel`（没有实例，也没有 `Panel::init`），但它们的样式同样需要一份 RON、一份 live 值和 Preferences 页。它们走 **shared style** 路径，而不是 `PanelSkin` 路径：

- `kairos_editor_component/src/shared_style.rs` + `shared_style/`：根文件放 `SharedStyle` trait 与 re-export；`catalog.rs` 放 `SharedStyleDef` / `StyleCatalog`（inventory 注册，对应 `#[panel]` 的 `PanelDef`）；`registry.rs` 放 `SharedStyles` 注册表和 `SharedSkin` 字段表；`color_picker.rs` 放取色器 skin。
- `ToolBarStyle` 与 `ColorPickerSkin` 在 `#[derive(StyleFields)]` 上加 `#[SharedStyle(TOOLBAR = "toolbar", title = "Toolbar")]`：宏生成 id 常量、sealed erasure impl 和 `inventory` 提交；类型只写 `impl SharedStyle for <Style> { fn preload(...) }`，`as_any` 不再手写。文件仍是 `Preferences/Styles/ToolBar.ron` / `ColorPicker.ron`（dark + light）。取色器的 42 个运行期 token 先落成 serde 友好的 `ColorPickerSkin`，再由 `tokens()` 投影到 widget 层。值框同理：`EditorFieldStyle`（`shared_style/field.rs`，文件 `Preferences/Styles/EditorField.ron`，标题 Field）持有文字/名称/选中名称/底色/悬停/描边三态/边框宽/圆角/内边距/高度/checkbox 边长，`tokens()` 投影成 widget 的 `FieldStyle`；任何画字段的面板（Preferences，将来的对象 Inspector）都读同一份。
- app 在 `EditorApp::new` 的 `set_theme(Dark)` 之后 `SharedStyles::load(&StyleCatalog::builtin(), theme)` 一次；`Frame::shared` 把 live 值下发给所有面板，工具栏用 `frame.toolbar_style()`，取色器用 `frame.color_picker_style()`，字段用 `frame.field_style()`。
- Preferences 在面板页之后固定追加 shared 页（Toolbar、Color Picker）；字段表来自 `SharedStyles::skins()`，不依赖任何面板实例，所以没有打开面板时也能编辑。
- 写路径是独立的 `Action::SharedStyle(SharedStyleAction::{SetField, EditList, Commit, Reload})` → `Effect::SharedStyle` → `SharedStyles::{set_field, edit_list, save, reload}`。`Commit` 仍然一次手势写一次文件；`Reload` 是显式刷新（页脚 Reload 按钮），reload 会 bump `SharedStyles::revision`，Preferences 下一帧重新取字段表。
- 取色器的 `window_controls`（红黄绿）不进 `ColorPicker.ron`：继续用共享的 `WindowControlsStyle::default()`，和 dock 浮动窗一致，直到 window-controls 自己有 skin。

> **2026-11-20 补充**：字体页 `Fonts` 进 shared 页列表（排在 Color Picker 之后、Toolbar 之前，按 id 排序）。
> `shared_style/fonts.rs` 的 `EditorFonts` 是普通 shared style——`#[derive(StyleFields)]` +
> `#[SharedStyle(FONTS = "fonts", title = "Fonts")]`，id 常量、seal 和 inventory 提交都由宏生成；
> `EditorFonts.ron` 从单值改成 dark/light 成对（`StyleFile<EditorFonts>`），所以既有页机制原样复用。
> 字段是 `proportional.0.name` / `.path` 这类索引 token：`path` 用 `String`（字段编辑器写文本），
> `FontEntry::priority` 暂 `#[style(skip)]`；安装结构 `fonts::FontsConfig` 由 `EditorFonts::config()`
> 在调用点投影（和取色器 `tokens()` 同一套路）。`preload` 调 `install`，所以 app 启动不再单独调
> `fonts::install`，编辑 Commit 后 app 的 `preload` 也会重装字体。
>
> **2026-11-20 补充（列表编辑器）**：`Vec<T>` 字段现在在字段表里生成一个 `StyleKind::List` 节点
> （value = 元素个数），空列表也因此在页面上有表头；写这个整数就是 resize（`Vec<T>::write`，上限
> `skin::LIST_MAX = 64`）。`+`/`−`/拖拽走新的 `ListEdit { Insert/Remove/Move }` 与
> `StyleAction::EditList` / `SharedStyleAction::EditList` → `StyleFields::edit_style_list`；derive
> 转发到 `StyleNode::edit_list`，`Vec<T>` 自己实现。页面按列表画两行表头（名字；计数 + 两格 +/−，
> 右下角），每个元素是 `Element N` 带（手柄 + caret + 蓝色选中 + 斑马线），字段在同一个块里、不额外
> 缩进，gutter 的分割线贯穿。结构变化后页面用一次性 `stale` 标记重读字段表（否则新元素要等换页）。
> 叶子列表（Profiler `category_palette`）已经有表头和增删，但元素行还是普通行，`Element N` +
> 颜色框留在下一轮；`priority` 仍 skip。

### 2.6 页面列表：活着的面板 + 固定 shared 页

`Frame::skins` 携带活着的面板的 `[PanelSkin]`（`panel/title/file/fields`；`title` 是 `Panel::title` 的文本），由 app 在每帧 `sync` 时、仅当 Preferences 打开时调用 `PanelHost::skins(&state)` 生成。`Frame::shared` 携带 `SharedStyles`，Preferences 把它的 `SharedSkin` 页固定追加在面板页之后。列表因此是：

面板页：`About`（打开过才有）、`Console`、`Profiler`、`Preferences` —— 后三者默认在场（Preferences 就是自己）；shared 页：`Toolbar`、`Color Picker`，始终在场。

**为什么不是「所有面板」**：0.2.0 的面板样式只在 `Panel::init` 时载入并属于实例；没打开的面板没有实例可改。旧窗口能列出关闭的窗口，是因为它在启动时就实例化了所有 drawer，而这正是 0.2.0 刻意去掉的（「没人打开的面板不构造」）。shared 样式没有实例，所以它们的页始终可编辑；未打开的面板仍需要把样式从实例里拆出来（catalog §C17 的 `StyleState`），不在本次范围。

**Toolbar 现在是 shared 页**：工具栏不是 `Panel`，但它实现 `SharedStyle`，由 app 启动时加载，Preferences 通过 shared 页编辑；工具栏本身每帧从 `Frame` 读，不再自己加载或持有皮肤。

### 2.7 版面与交互（Unity Inspector 样式）

```text
┌─ 页面列表 (118) ─┬─ 页面 ────────────────────────────────┐
│ ▍Console         │ Console                                │
│  Profiler        │ Preferences/Styles/ConsoleStyle.ron …  │
│  Preferences     │ ─────────────────────────────────────  │
│                  │ Toolbar bg       [████████████████ ◔]   │
│                  │ Toolbar height            [ 20 ]       │
│                  │ …                                      │
│                  │ ▼ Icons                                │
│                  │   Info      [Preferences/…/info.png]   │
│                  │   Warning   [Preferences/…/warning.png]│
│                  │ ▼ Textures                             │
│                  │   Tint      [Preferences/…/tint.png]   │
├──────────────────┴────────────────────────────────────────┤
│                                                  [Close]  │
└───────────────────────────────────────────────────────────┘
```

- **.ron 的嵌套结构就是折叠区域**：点分路径的每一级（`icons`、`icons.textures`）画成 Unity 的 foldout band——整行可点、左侧三角；折叠状态属于视图（`PageState::collapsed`），不写回文件。组头与根 token 左对齐，组内 token 按层级缩进 `group_indent`，左侧画 1 px 树引导线。
- **两栏、左对齐**：标签列从行内边距开始左对齐（不再用 `add_sized` 的居中布局）；值列从同一列起点铺满剩余宽度。`label_width` 是理想值，页面窄了先缩标签列，`field_width` 是值列的下限——不再把控件堆到标签下面（Unity 的 inspector 也保持两列，有测试）。
- **Zebra 行**：每条叶子行铺满整行背景，`row_odd` / `row_even` 交替；组头用 `group_background`，悬停 `group_hover`。`row_gap = 0`，条带连续。
- **Unity 值框**：`control_rail` 暗底、`control_edge` 1 px 框、0 圆角、文字左对齐并留 `field_padding`；悬停时框线变 `control_edge_hover`。Float/Integer 点击进入行内文本编辑（全选、Esc 还原、Enter/失焦提交），拖动仍然是 scrub；Text 直接是单行 `TextEdit`；Toggle 是 Unity 的方形 checkbox（`checkbox_size`）。
- **值框是共享 widget，一个控件一个文件**：`field/style.rs`（FieldStyle）、`field/paint.rs`（底色/描边/勾）、`field/edit.rs`（行内编辑器与 Unity 的选中蒙版）、`field/number.rs`（NumberField + Number trait）、`field/text.rs`、`field/toggle.rs`、`field/label.rs`。NumberField 对任意 Rust 基本类型生效（f32/f64/i8..i64/u8..u64/usize/isize）：显示走类型自己的 Display（f32 不会变成 f64 噪声），写回按类型自身的范围取整截断，样式 token 的额外上限用 `.range(0.0, max)` 收窄。`FieldOutcome::finished` 是「这次键入结束」的信号，页面据此写盘；行内编辑缓冲 `NumberEdit` 由页面持有，整数的拖动余量放在 egui 临时存储里。
- **数字靠名称拖拽**：名称是 `FieldLabel`（Unity 的 scrub handle），悬停变左右箭头光标，按住左右拖改值。拖拽属于 `NumberField`：页面把本帧的名称手势交进去（`dragged_by(&FieldLabelResponse)`），速度、整数余量、范围截断都在 widget 里，页面只是把两个 widget 接起来。值框本身只处理点击进入行内编辑，悬停是文本光标；没有名称的框（列表的 size 框）默认自己就是手柄。
- **选中文字用取色器同款蒙版**：数字编辑框与文本字段全选时不再铺实心 accent，而是 accent 的 62%（`from_rgba_unmultiplied(.., 160)`）+ 不透明 selection stroke，数字仍然可读——和取色器通道值一致。文本字段第一次获得焦点即全选，所以直接打字就是替换。
- 名称列、Zebra 条带与两栏几何仍属于页面的行版面——那是 inspector 的排版，不是控件。
- **颜色字段**：`Color` 是 Unity Inspector 的整条 swatch + 右侧吸管槽（不再在行内重复 hex）；`ColorProperty` 额外带字段策略，HDR 行的 swatch 右上角叠一个对比色 `HDR` 角标。透明色先画棋盘格再叠色。点 swatch 选中该行并打开 picker；点字段名只选中。`Color` 是 8 字节 linear f16×4；HDR/LDR 不是值类型，而是 `ColorUsage`。
- **取色器（自有 widget）**：不是 dock panel，而是字段拥有的浮动工具窗；固定宽 260、高度随内容、标题栏可拖、同时只开一个；标题栏 28 px 与 dock 浮动窗口一致，标题随值类型为 `Color` / `HDR Color` 并在标题栏居中；标题栏左侧是 dock 同款三个 macOS traffic-light 圆点，由共享的 `kairos_editor_widgets::window_controls` widget 绘制（dock 浮动窗也用它）：关闭红色可用，收起/最大化在本窗没有动作，按 Unity 保留为灰色且不接收点击，悬停可用按钮才显示字形。这三个按钮是 egui 自绘 chrome，不是 NSWindow/Win32 原生 caption button；dock 的 `WindowStyle` 本来就刻意全平台统一 macOS 外观，所以切到 Windows 仍是红黄绿，要变成 Windows 右上角原生按钮需要真实 OS 窗口或单独的 Windows skin；232 px 色相环（26 px 环宽，程序生成抗锯齿纹理）+ 约 116 px SV 方块（128² HSV 纹理，消除四顶点网格的 45° 斜线）、通道滑杆与可键入数值、Hexadecimal、HDR 的 Intensity 与 `-2/-1/0/+1/+2` 预设、Swatches 调色板。LDR 模式为 `RGB 0-255` / `RGB 0-1.0` / `HSV`，HDR 为 `RGB 0-255` / `RGB 0-Inf` / `HSV`；HSV 的标签是 H/S/V，S/V/Alpha 用 0–100。LDR 不显示 Intensity，HDR 不显示 Hexadecimal。通道模式按钮与 Swatches 组按钮共用 Unity popup field 样式：`field` 灰底 + 1 px 边框 + 左对齐文字 + 右端 caret，不再是透明文字式 `Button`；通道滑杆 14 px 高、184 px 宽、thumb 7×18，点击某行后 label 用柔和的 `label_active` 蓝、value 框变 accent 描边，数值左对齐并保留 5 px 内边距。编辑数值时选中文字是接近 Unity 的实心蓝蒙版（160/255），并强制用 `selection.stroke` 保留白色数字，不能因为 `Stroke::NONE` 把数字一起隐掉；蒙版重但 value 始终可读。拖动实时 `SetField`，指针抬起/Enter/Esc 时 `Commit`；Esc 先还原到打开时的颜色再关闭，点外部或标题栏左侧关闭圆点关闭。
- **HDR 曝光基准**：Intensity 使用 Unity 的 191/255 基准（`ColorProperty::HDR_BASE_MAX`），最大通道 4.0 打开时约为 2.42 stops，而不是 2.0；`RGB 0-Inf` 直接编辑绝对 linear 通道。
- **吸管**：swatch 右侧与 picker 左上使用从本机 Unity/Tuanjie 包提取的 `EyeDropper.Large` 图标（运行时资源 `Preferences/Textures/color_picker/eyedropper.png`，按皮肤 tint）。点吸管进入拾取态：桌面任意位置（包括编辑器窗口外）实时采样，光标旁跟随一个置顶、点击穿透的放大镜浮层（15×15 像素约 8× 放大，中心格高亮，显示 `#hex` 与 RGB 色块），左键确认、Esc/右键取消；HDR 字段只改基色、保留 Intensity。拾取期间宿主用低层鼠标钩子屏蔽整个桌面的鼠标按键与滚轮（光标移动照常，标题栏的缩小/放大/关闭也不响应，键盘不遮罩），所以确认点击不会误触任何窗口；确认那一下也不会到 egui，字段用 `eyedropper::take_committed` 上报拾取结束，Preferences 立即写盘而不是等下一次指针抬起。宿主实现见 `kairos_editor_app/src/eyedropper/`：Windows 用 `GetCursorPos` + `BitBlt` + GDI 浮层 + `WH_MOUSE_LL` 遮罩；macOS 用 ScreenCaptureKit 取 15×15 像素、每块屏幕一个 AppKit 透明浮层（hex/RGB/色块，左键确认、右键/Esc 取消，字段逐帧跟随），首次使用弹系统屏幕录制授权；无权限、旧系统或采集失败时回退 `NSColorSampler`（无需权限，但只在确认时给色、无右键取消）。Linux 在 X11 下用私有的 `QueryPointer`/`GetImage` + override-redirect 浮层 + `GrabPointer` 遮罩；Wayland（含 XWayland）没有全局指针与跨窗口像素，保留「not available」提示。
- **Swatches**：共享一份 `ColorSwatches`，存在 `Library/ColorSwatches.ron`（运行时用户数据，不随样式文件走）。标题行是折叠箭头 + `Swatches`，右侧是 `Default v` 组菜单按钮，按钮与模式按钮同款 Unity popup field。点击后在按钮下方、向右展开浮层菜单（组列表 + New/Rename/Delete），菜单用 `egui::Area` 而不占 header 布局，因此不会把下方 swatch rect 推下去；每个 action 独占 20 px 一行，选中/悬停使用项目菜单的 Unity 蓝 `#2C5D87`。Rename/New 的输入框也是浮层对话框，不改变网格位置。网格 14 px、1 px gap，末尾 New tile 保存当前颜色，空组显示 `Click to add new preset`。菜单打开时页面滚动偏移保持不变，避免 Rename 输入框触发背景滚动。
- **选中态**：点字段名或值框都会选中该行——字段名变 `label_active`（Unity 的蓝色 label），值框画 `accent` 1 px 外框。点页面空白或标题清除选中；点组头只折叠，不改变选中。
- **滚动条间距**：页面右侧预留 `scrollbar_gap`；值框的右边界停在它左边，图标路径这类长值不会压到 scrollbar 上（有测试）。
- **字体层级**：标题 15（strong）、正文与组头 12、文件路径 12；组头用更亮的 `group` 色 + 全宽底色区分，不依赖粗体（编辑器字体没有单独注册的 bold family）。
- **没有 Apply/Cancel**：每个交互结束即写，文件就是记录；Close 只关窗。

---

## 3. 文件级实现清单

| 文件 | 变更 | 职责 |
|---|---|---|
| `kairos_editor_component/macros/`（新 crate） | 新增 | `#[derive(StyleFields)]`：从结构体字段生成字段表与按路径写入；`#[SharedStyle(...)]` 生成 id 常量、sealed erasure 和 `SharedStyleDef` 提交 |
| `kairos_editor_component/src/skin.rs` | 新增 | `StyleValue` 桥接、`StyleField`/`StyleKind`、`StyleNode`/`StyleFields` 与叶子类型实现、`PanelSkin`、`save_skin`、`humanize` |
| `kairos_editor_core/src/style.rs` | 新增 | `StyleValue` + `SharedStyleId`（core 只认识值，不认识 egui） |
| `kairos_editor_core/src/action.rs` / `effect.rs` / `reduce/{style,shared_style}.rs` | 改/新增 | `Action::Style`、`StyleAction::{SetField,Commit}`、`Effect::Style`、`StyleEffect::{SetField,Persist}`；shared 样式另有 `SharedStyleAction::{SetField,Commit,Reload}` / `SharedStyleEffect::{SetField,Persist,Reload}` |
| `kairos_editor_component/src/panel.rs` | 改 | `Panel::{skin,set_skin_field,save_skin}`（默认无皮肤） |
| `kairos_editor_component/src/panel/host.rs` | 改 | `skins`/`set_skin_field`/`save_skin` |
| `kairos_editor_component/src/shared_style.rs` + `shared_style/{catalog,registry,color_picker}.rs` | 新增 | `SharedStyle`；`SharedStyleDef` / `StyleCatalog`；`SharedStyles` / `SharedSkin`；`ColorPickerSkin`（serde + `StyleFields` + `tokens()` 投影） |
| `kairos_editor_component/src/frame.rs` | 改 | `Frame::skins` + `with_skins`；`Frame::shared` + `with_shared`；`toolbar_style()` / `color_picker_style()` |
| `kairos_editor_component/src/panels/preferences.rs` | 新增/改 | 页面列表（面板页 + 固定 shared 页）、页、分组行、字段表到 field widget 的映射、保存边界、shared 页的 Reload、单测 |
| `kairos_editor_component/src/panels/preferences/style.rs` | 新增/改 | `PreferencesPanelStyle`（自身也 derive，可编辑自己）+ 参考皮肤往返测试；不再投影 `ColorPickerStyle` |
| `kairos_editor_widgets/src/color_picker.rs` | 新增 | 自有的 Unity 风格 LDR/HDR 颜色字段与取色器：色相环、SV 方块、通道滑杆、hex、Intensity、Swatches |
| `kairos_editor_widgets/src/field/` | 新增 | 可复用值框 widget，一控件一文件：`style`（FieldStyle）/ `paint` / `edit`（行内编辑器与 62% 选中蒙版）/ `number`（Number trait + NumberField）/ `text` / `toggle` / `label`（FieldLabel，数字的 scrub handle） |
| `kairos_editor_component/src/shared_style/field.rs` | 新增 | `EditorFieldStyle`（`#[SharedStyle(EDITOR_FIELD = "editor_field")]`）+ `tokens()` 投影 |
| `Preferences/Styles/EditorField.ron` | 新增 | 字段皮肤参考文件（dark + light），与代码默认值逐字段相等的往返测试钉住 |
| `kairos_color/src/linear.rs` | 新增 | `Color`（8 字节 linear f16×4，可超过 1）、`ColorUsage`/`ColorProperty`/`ColorSpace`、sRGB 传输函数与 round-trip 测试 |
| `kairos_editor_component/src/panels/{about,console,profiler}.rs` + `*/style.rs` | 改 | 样式 derive；三个 `Panel` 方法（皮肤快照 / 写字段 / 存文件） |
| `kairos_editor_component/src/panels.rs` | 改 | `PREFERENCES` 条目；面板数量 7 → 8 |
| `kairos_editor_component/src/toolbar.rs` | 改 | `todo!()` → `ShowPanel { PREFERENCES }`；工具栏改为从 `Frame` 读 shared 皮肤，不再自己加载 |
| `kairos_editor_component/src/toolbar/style.rs` | 改 | `ToolBarStyle` derive `StyleFields` + `#[SharedStyle(...)]`；只保留 `preload` |
| `kairos_editor_component/src/lib.rs` | 改 | 导出 `skin` / `shared_style` 类型；`extern crate self` 供 derive 使用；模块文档 |
| `kairos_editor_app/src/{app,effects}.rs` | 改 | 启动时 `SharedStyles::load` + preload；快照 `Frame::skins`（仅 Preferences 打开时）；`Frame::shared` 下发；`Effect::Style` / `Effect::SharedStyle` 执行；`describe` |
| `KairosEngine/Cargo.toml` | 改 | 新成员 `kairos_editor_component/macros` |
| `Preferences/Styles/PreferencesPanel.ron` | 新增 | 参考皮肤（dark + light），与代码默认值逐字段相等的往返测试钉住 |
| `Preferences/Styles/ColorPicker.ron` | 新增 | 取色器的共享参考皮肤（dark + light），与 `ColorPickerSkin` 代码默认值逐字段相等 |

---

## 4. 旧 → 新逐项对照

| 旧 | 新 | 处理 |
|---|---|---|
| `get_style_fileds`/`update_style` + 下标 | `#[derive(StyleFields)]` + 点分路径 | B2/B3 消解：字段与编辑器同源，路径寻址 |
| 每帧 `UpdateUIStyle(page)` | `SetField`（每帧预览）+ `Commit`（一次写盘） | B1/B4 消解 |
| 每窗口 TOML 落盘 | 面板自己的 `save_skin`，交互结束写一次 | 写盘从「帧」变成「手势」 |
| 遍历 `drawers` 生成页面 | `PanelHost::skins()`（活着的面板） | M9 消解：没有空页，没有自指页 |
| `type_name::<T>()` 页面名 | `Panel::title`（“Console”），由宏写进 `PanelSkin` | 用户文案，单一真源 |
| 字段名手写、无说明 | 字段名 humanize + 类型决定控件 | M4 消解 |
| 只有 `egui::Grid`（首帧 discard） | 手排的两栏 + `ScrollArea` | 多 pass 安全 |
| 只有 tab × | body 的 Close + tab × | M2 消解 |
| 皮肤文件缺失 → panic | `load_or_default` + `warn!` | B5 消解 |
| 窗口尺寸可在自己的页里改 | 不可编辑；尺寸归 dock | B6/B7 消解 |
| 无测试 | 见 §5 | M8 消解 |

---

## 5. 测试与验收

**core（12 个 + shared）**

1. `SetField` → 恰好一条 `Effect::Style(SetField)`；`Commit` → `Persist`；两个 action 都不改 `EditorState`。
1b. `SharedStyleAction::{SetField,Commit,Reload}` → 对应 `Effect::SharedStyle`；同样不改 `EditorState`。

**component**

2. derive：字段顺序、点分路径、`#[style(skip)]`、类型种类、整数 clamp、颜色 round-trip、`humanize`。
3. 三个真实样式（About/Console/Profiler/Preferences）都能列出字段，且字段路径可达（`toolbar_height`、`icons.info`、…）。
4. 面板：页面列表显示所有活着的皮肤、首屏画第一页的文件路径与字段；点另一个面板换页且**不产生 action**。
5. 点 toggle → 恰好 `[SetField, Commit]`（同一个 release pass），页面本地值同步更新。
6. 切页/关窗时把未保存编辑提交一次；Close 按钮 → `ClosePanel`；没有皮肤时给出说明；180 px 高的窗口里 Close 仍在面板内。
7. Preferences 自己的 `skin()` 可列出 `nav_width`、`row_odd` 等 token（可编辑自己）。
8. host：`skins()` 只包含活着的面板；未知路径/错误类型被拒绝；写入后快照反映新值；未打开的面板没有皮肤。
9. 参考皮肤 `PreferencesPanel.ron` 与代码默认值逐字段相等。
9b. `StyleCatalog::builtin()` 注册 Toolbar / Color Picker；`SharedStyles::skins()` 给出两页字段表；`ColorPicker.ron` 与 `ColorPickerSkin` 默认值逐字段相等；`tokens()` 保持 0.7 边框、13 px 标题和共享 traffic lights。
9c. 所有面板关闭时 shared 页仍然存在且可编辑；shared 编辑发 `Action::SharedStyle(SetField)`/`Commit`；Reload 按钮发 `Reload`，reload 后 `revision` 让页面重新取字段表。
10. foldout：每个嵌套 struct 是整行可点的组头；点 `icons` 只隐藏该组，点 `icons.textures` 只隐藏 Tint，动作列表为空。
11. Zebra：叶子行按 `row_odd` / `row_even` 交替；组头用 `group_background`，不参与交替。
12. 对齐：不同宽度的 root 标签左边缘一致；组内标签按 `group_indent` 逐级缩进；值列左边缘不随深度改变；窄面板仍是两列且在窗口内。
13. 选中：点字段名 → `label_active` 蓝色 + 值框 `accent` 外框；点页面空白 → 清空。
14. 数字编辑：点数值框进入行内编辑，输入替换选中值，Enter 结束并额外发 `Commit`；Esc 还原。
15. 滚动条：长页面里图标路径值框的右边界停在 `scrollbar_gap` 左侧。
16. 颜色字段：LDR 行不出现 `HDR`，`ColorProperty` 的 HDR usage 行出现角标；点 swatch 打开自有 picker，点 Swatches 里的黑块 → 恰好 `[SetField(Color), Commit]`，且行仍保持选中。
17. 取色器：LDR 标题 `Color` 且无 `Intensity`；HDR 标题 `HDR Color` 且有 `Intensity`/预设；色相环点击改变颜色；通道数值可键入；hex 可替换颜色；`RGB 0-255` 模式切换生效。
18. 浮动窗：picker 固定宽 260（232 wheel / 26 ring / 20 row / 14 swatch / 14 track），标题栏拖动会移动；点外部关闭；Esc 还原并关闭；同时只开一个。
19. HSV/ HDR：HSV 标签为 H/S/V 且 S/V/Alpha 为 0–100；HDR 菜单含 `RGB 0-Inf`，`RGB 0-Inf` 可键入超过 1 的绝对 linear 值；HDR 不显示 Hexadecimal。
20. Swatches：New tile 追加当前颜色；组菜单可新建/选择/重命名/删除，菜单与 Rename 对话框均浮在网格上方且不改变 swatch rect；`ColorSwatches` 经 `Library/ColorSwatches.ron` 往返。

**app**

21. Preferences 通过 `ShowPanel` 浮成第二个 surface；打开时 `app.skins` 含 Console/Profiler/Preferences，不含未打开的 About；关闭时不快照。shared 样式在 `EditorApp::new` 里 load + preload；`Effect::SharedStyle` 的 SetField 写进 `SharedStyles`，Commit 写 `ColorPicker.ron`/`ToolBar.ron`。
22. `SetField{console, toolbar_height, 30.0}` → Console 的 live style 变成 30；**不写文件**；`Commit` 后 `ConsoleStyle.ron` 里出现 `toolbar_height: 30.0`。

**静态门**：`cargo check --workspace --all-targets` 干净；四个 crate 的 `fmt --check` 干净；clippy 无新增告警。

---

## 6. 已知边界 / 后续

1. **ToolBar / Color Picker 是 shared 页，不是 `PanelSkin` 页**：它们没有面板实例，由 `SharedStyles` 注册表持有，Preferences 固定追加它们的页。将来第三个 chrome 样式（例如 window controls）只需 `#[derive(StyleFields)]` + `#[SharedStyle(...)]` 并写 `preload`，页面和保存路径自动出现（若面板要读它，再在 `Frame` 加一个 typed accessor）。
2. **面板页仍只能编辑已打开的面板**：原因见 §2.6。要覆盖关闭的面板需要 `StyleState`（状态树持有样式文档），属 catalog §C17；shared 页不受这个限制。
3. **只显示当前主题的那一半皮肤**：面板在 `init` 里、shared 在 `SharedStyles::load` 里按 `ctx.theme()` 选 skin（主题被 app 钉为 Dark）。页面编辑 dark；保存时保留 light。等浅色皮肤启用后再做主题切换（shared 需要重新 load/选半套）。
4. **没有单字段「恢复默认」**：需要 `StyleEffect::ResetField`（把代码默认值与字段路径对齐），等有第二个使用场景再加。
5. **键盘提交点**：键入触发 `SetField`（宿主下一帧生效）；`Commit` 在指针抬起、行内数字编辑结束（Enter / 失焦）、color hex/数值的 Enter/Esc、关窗或切页时发生。纯键盘编辑文本字段（不点鼠标）仍要等失焦/关窗才写盘。
6. **拖拽速度是启发式的**：`speed = clamp(|value| / 50, 0.01, 1.0)`，对大多数尺寸合适；某些 token（颜色分量之外的极小值）可能偏慢，后续可以按字段加 hint。
7. **页面列表的花费**：Preferences 打开时每帧重新快照所有字段（现在约 3 个面板 + 2 个 shared 页、~240 个字段；shared 字段表由 `SharedStyles::skins()` 即时生成）。这些分配只在编辑器自己的设置窗口开着时发生；若将来面板/字段数量上来了，改成「页面切换时刷新」即可。
8. **吸管按平台走各自的原生通路**：widget 侧只有会话与 `eyedropper::Update` 契约（sRGB 字节 + Picking/Committed/Cancelled + 会话 id），不认平台；宿主 `kairos_editor_app/src/eyedropper/{win32,macos,linux}.rs` 负责取像素与放大镜。Windows 走 `GetCursorPos` + `BitBlt`（屏幕 DC）+ GDI 浮层，**不依赖 framebuffer readback**，因此不需要接 egui 的 screenshot 请求；拾取期间用 `WH_MOUSE_LL` 钩子遮罩桌面鼠标按键与滚轮（移动放行，否则系统光标会被冻住；捕获窗口自己的标题栏按钮也盖不住，见 ADR 0014）；确认/取消的按键由钩子记录（丢弃的事件 `GetAsyncKeyState` 读不到）。macOS 用 ScreenCaptureKit 的 `SCScreenshotManager` 按 `pointPixelScale` 取 15×15 像素，每块屏幕一个透明 AppKit 浮层；浮层消费确认点击，所以同样要吞随后的 release，字段在 Picking 帧逐帧跟随。首次使用触发系统屏幕录制授权；无权限、旧系统（<14）或采集失败时回退 `NSColorSampler`（无需权限，但只在确认时给色、无右键取消）。Linux 在 X11 下用私有 X 连接 `QueryPointer` + `GetImage`（15×15 块）+ override-redirect 浮层 + `GrabPointer`（只选按键事件，移动放行）实现同样的行为；Wayland 没有全局指针与跨窗口像素（只能走 ScreenCast portal），XWayland 的 root window 也不含 Wayland 客户端的像素，因此两者都报告不可用。确认后的写盘靠 widget 的 `take_committed` 结束信号；Windows/X11 的确认点击被遮罩，宿主还要把随后的 release 吞掉（两帧无按键后解除，见 ADR 0014）。
9. **HDR 数据面**：`Color` / `ColorProperty` 已经端到端打通并有组件测试；`StyleValue::HdrColor` / `StyleKind::HdrColor` 仍是皮肤协议里的线性颜色 kind。现有面板皮肤都是 LDR，所以正常 Preferences 页面暂时看不到 HDR 行；它会在材质/发射色 token 接入后出现。

---

## 7. 证据

- 旧实现：`kairos_editor/src/ui/preferences_window.rs:17-45,47-63,65-80,90-97,99-112,114-148,150-152,154-167,182-192,194-413,404`
- 旧样式字段：`kairos_editor/src/ui/ui_style_fields.rs:1-138`
- 旧通道：`kairos_editor/src/ui.rs:468-476,477-487,488-492,493-514,800-829,855-863,865-869`
- 旧样式文件：`Preferences/Styles/PreferencesWindowStyle.toml`（`paths.rs:8`）
- 旧写盘：`about_window.rs:141-163`、`tool_bar.rs:305-327`、`scene_window.rs:333-356`；审计 `docs/research/editor-ui-messager-mvc-audit.md:26,258,381`
- 多 pass：`egui-0.35.0/src/grid.rs:454`、`context.rs:822-860`；审计 `editor-ui-messager-mvc-audit.md:21,188`
- 新字段协议：`kairos_editor_component/macros/src/lib.rs`、`kairos_editor_component/src/skin.rs`、`kairos_editor_core/src/style.rs`
- 新面板：`kairos_editor_component/src/panels/preferences.rs`、`preferences/style.rs`、`panels.rs`
- 新 host/app：`panel.rs`（三个方法）、`panel/host.rs`、`frame.rs`、`kairos_editor_app/src/effects.rs`、`app.rs`
- 样式协议：`kairos_editor_widgets/src/style.rs:64-148`