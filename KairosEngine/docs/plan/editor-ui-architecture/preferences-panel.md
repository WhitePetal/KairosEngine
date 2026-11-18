# Preferences 窗口迁移评审：`kairos_editor::PreferencesWindow` → 0.2.0 `panels/preferences`

> **状态**：评审 + 实现（2026-11-19）
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

- `panels::PREFERENCES: PanelId = PanelId("preferences")`，声明在 `panels::CATALOG`。
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
- **控件种类由字段类型决定**：`f32` → Float（DragValue，速度按数量级）、`u8/u16/u32/usize` → Integer（带类型上限）、`bool` → Toggle、`String` → Text、`Color32` → Color、`Vec<T>` → 按下标展开（Profiler 的 `category_palette`），`#[style(skip)]` 排除非 token 字段。
- 旧实现的「字段名 + 手工范围」不见了：名字来自字段名（`toolbar_bg` → “Toolbar bg”），范围来自类型。

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
- `SetField` 不写 console 日志（一次拖动 60 行），`Commit` 写；`describe` 仍为每个 action 提供人读文本。

### 2.5 页面列表：活着的面板

`Frame::skins` 携带 `[PanelSkin]`（`panel/title/file/fields`），由 app 在每帧 `sync` 时、仅当 Preferences 打开时调用 `PanelHost::skins()` 生成。列表因此是：

`About`（打开过才有）、`Console`、`Profiler`、`Preferences` —— 后三者默认在场（Preferences 就是自己）。

**为什么不是「所有组件」**：0.2.0 的样式只在 `Panel::init` 时载入并属于实例；没打开的面板没有实例可改。旧窗口能列出关闭的窗口，是因为它在启动时就实例化了所有 drawer，而这正是 0.2.0 刻意去掉的（「没人打开的面板不构造」）。要覆盖「未打开的面板」，需要把样式从实例里拆出来（catalog §C17 的 `StyleState`），不在本次范围。

**没有 ToolBar 页**：工具栏是 app chrome，不是 `Panel`，没有 `skin()` 入口；它仍然可以通过 `Preferences/Styles/ToolBar.ron` 手改。这是与旧窗口唯一的功能差集，记为后续（把 `ToolBar` 也接进同一个字段协议即可，接口已经就绪）。

### 2.6 版面与交互

```text
┌─ 页面列表 (118) ─┬─ 页面 ────────────────────────────────┐
│ ▍Console         │ Console                                │
│  Profiler        │ Preferences/Styles/ConsoleStyle.ron …  │
│  Preferences     │ ─────────────────────────────────────  │
│                  │ Toolbar bg          [color]            │
│                  │ Toolbar height      [ 20 ]             │
│                  │ …                                      │
│                  │ Icons                                  │
│                  │   Info              [path text]        │
├──────────────────┴────────────────────────────────────────┤
│                                                  [Close]  │
└───────────────────────────────────────────────────────────┘
```

- **两栏**：左侧页面列表（选中行填充 + 2 px accent 竖条），右侧页面在 `ScrollArea` 里滚动；footer 用 `allocate_exact_size` 钉在底部，窗口再矮也先丢页面内容、不丢按钮（有测试）。
- **窄了就换行**：页面宽度不足 `label_width + field_width + 24` 时，控件落到标签下面，而不是被右边缘裁掉——dock 里的 Preferences 可以比浮窗窄得多（有测试）。
- **分组**：点分路径的第一段作为分组标题（`Icons`），行内显示叶子名（`Warning`）；根级 token 不显示分组标题。
- **控件**：Float/Integer → `DragValue`（整数带类型上限、文本右对齐宽度固定）；Toggle → On/Off 按钮（选中填充 accent，与 Console 工具栏一致）；Text → 单行 `TextEdit`（图标路径）；Color → egui 色板按钮。所有控件共用 `control_visuals()` 的 token（rail/hover/active/accent/文字），不会在暗面板里露出 egui 默认灰。
- **字体层级**：标题 15（strong）、正文 12、分组与文件路径 11；色板与 About/Console 同源（`#363636` 底、`#2A2A2A` 分隔线）。
- **没有 Apply/Cancel**：每个交互结束即写，文件就是记录；Close 只关窗。

---

## 3. 文件级实现清单

| 文件 | 变更 | 职责 |
|---|---|---|
| `kairos_editor_component/macros/`（新 crate） | 新增 | `#[derive(StyleFields)]`：从结构体字段生成字段表与按路径写入 |
| `kairos_editor_component/src/skin.rs` | 新增 | `StyleValue` 桥接、`StyleField`/`StyleKind`、`StyleNode`/`StyleFields` 与叶子类型实现、`PanelSkin`、`save_skin`、`humanize` |
| `kairos_editor_core/src/style.rs` | 新增 | `StyleValue`（core 只认识值，不认识 egui） |
| `kairos_editor_core/src/action.rs` / `effect.rs` / `reduce/style.rs` | 改/新增 | `Action::Style`、`StyleAction::{SetField,Commit}`、`Effect::Style`、`StyleEffect::{SetField,Persist}`、路由 |
| `kairos_editor_component/src/panel.rs` | 改 | `Panel::{skin,set_skin_field,save_skin}`（默认无皮肤） |
| `kairos_editor_component/src/panel/host.rs` | 改 | `skins`/`set_skin_field`/`save_skin` |
| `kairos_editor_component/src/frame.rs` | 改 | `Frame::skins` + `with_skins` |
| `kairos_editor_component/src/panels/preferences.rs` | 新增 | 页面列表、页、分组行、控件、保存边界、单测 |
| `kairos_editor_component/src/panels/preferences/style.rs` | 新增 | `PreferencesPanelStyle`（自身也 derive，可编辑自己）+ 参考皮肤往返测试 |
| `kairos_editor_component/src/panels/{about,console,profiler}.rs` + `*/style.rs` | 改 | 样式 derive；三个 `Panel` 方法（皮肤快照 / 写字段 / 存文件） |
| `kairos_editor_component/src/panels.rs` | 改 | `PREFERENCES` 条目；面板数量 7 → 8 |
| `kairos_editor_component/src/toolbar.rs` | 改 | `todo!()` → `ShowPanel { PREFERENCES }` |
| `kairos_editor_component/src/lib.rs` | 改 | 导出 `skin` 类型；`extern crate self` 供 derive 使用；模块文档 |
| `kairos_editor_app/src/{app,effects}.rs` | 改 | 快照 `Frame::skins`（仅 Preferences 打开时）；`Effect::Style` 执行；`describe` |
| `KairosEngine/Cargo.toml` | 改 | 新成员 `kairos_editor_component/macros` |
| `Preferences/Styles/PreferencesPanel.ron` | 新增 | 参考皮肤（dark + light），与代码默认值逐字段相等的往返测试钉住 |

---

## 4. 旧 → 新逐项对照

| 旧 | 新 | 处理 |
|---|---|---|
| `get_style_fileds`/`update_style` + 下标 | `#[derive(StyleFields)]` + 点分路径 | B2/B3 消解：字段与编辑器同源，路径寻址 |
| 每帧 `UpdateUIStyle(page)` | `SetField`（每帧预览）+ `Commit`（一次写盘） | B1/B4 消解 |
| 每窗口 TOML 落盘 | 面板自己的 `save_skin`，交互结束写一次 | 写盘从「帧」变成「手势」 |
| 遍历 `drawers` 生成页面 | `PanelHost::skins()`（活着的面板） | M9 消解：没有空页，没有自指页 |
| `type_name::<T>()` 页面名 | `PanelSkin::title`（“Console”） | 用户文案 |
| 字段名手写、无说明 | 字段名 humanize + 类型决定控件 | M4 消解 |
| 只有 `egui::Grid`（首帧 discard） | 手排的两栏 + `ScrollArea` | 多 pass 安全 |
| 只有 tab × | body 的 Close + tab × | M2 消解 |
| 皮肤文件缺失 → panic | `load_or_default` + `warn!` | B5 消解 |
| 窗口尺寸可在自己的页里改 | 不可编辑；尺寸归 dock | B6/B7 消解 |
| 无测试 | 见 §5 | M8 消解 |

---

## 5. 测试与验收

**core（12 个）**

1. `SetField` → 恰好一条 `Effect::Style(SetField)`；`Commit` → `Persist`；两个 action 都不改 `EditorState`。

**component（140 个）**

2. derive：字段顺序、点分路径、`#[style(skip)]`、类型种类、整数 clamp、颜色 round-trip、`humanize`。
3. 三个真实样式（About/Console/Profiler/Preferences）都能列出字段，且字段路径可达（`toolbar_height`、`icons.info`、…）。
4. 面板：页面列表显示所有活着的皮肤、首屏画第一页的文件路径与字段、嵌套字段有分组标题；点另一个面板换页且**不产生 action**。
5. 点 toggle → 恰好 `[SetField, Commit]`（同一个 release pass），页面本地值同步更新。
6. 切页/关窗时把未保存编辑提交一次；Close 按钮 → `ClosePanel`；没有皮肤时给出说明；180 px 高的窗口里 Close 仍在面板内。
7. Preferences 自己的 `skin()` 可列出 `nav_width` 等 token（可编辑自己）。
8. host：`skins()` 只包含活着的面板；未知路径/错误类型被拒绝；写入后快照反映新值；未打开的面板没有皮肤。
9. 参考皮肤 `PreferencesPanel.ron` 与代码默认值逐字段相等。

**app（20 个）**

10. Preferences 通过 `ShowPanel` 浮成第二个 surface；打开时 `app.skins` 含 Console/Profiler/Preferences，不含未打开的 About；关闭时不快照。
11. `SetField{console, toolbar_height, 30.0}` → Console 的 live style 变成 30；**不写文件**；`Commit` 后 `ConsoleStyle.ron` 里出现 `toolbar_height: 30.0`。

**静态门**：`cargo check --workspace --all-targets` 干净；四个 crate 的 `fmt --check` 干净；clippy 无新增告警。

---

## 6. 已知边界 / 后续

1. **ToolBar 不在页面列表里**：它是 app chrome，不是 `Panel`。接进来的方式很直接（给 `ToolBar` 同样的三个方法，并由 app 把它的 `PanelSkin` 追加到 `Frame::skins`），但工具栏的生命周期在 app 而不是 host，本次没做。
2. **只能编辑已打开的面板**：原因见 §2.5。要覆盖关闭的面板需要 `StyleState`（状态树持有样式文档），属 catalog §C17。
3. **只显示当前主题的那一半皮肤**：面板在 `init` 里按 `ctx.theme()` 选 skin（主题被 app 钉为 Dark）。页面编辑 dark；保存时保留 light。等浅色皮肤启用后再做主题切换。
4. **没有单字段「恢复默认」**：需要 `StyleEffect::ResetField`（把代码默认值与字段路径对齐），等有第二个使用场景再加。
5. **文本字段没有实时提交**：键入触发 `SetField`（宿主下一帧生效），但 `Commit` 依赖指针抬起或关窗；纯键盘流程的保存点只有关窗/切页。
6. **拖拽速度是启发式的**：`speed = clamp(|value| / 50, 0.01, 1.0)`，对大多数尺寸合适；某些 token（颜色分量之外的极小值）可能偏慢，后续可以按字段加 hint。
7. **面板列表的花费**：Preferences 打开时每帧重新快照所有字段（现在约 3 个面板、~180 个字段）。这些分配只在编辑器自己的设置窗口开着时发生；若将来面板/字段数量上来了，改成「页面切换时刷新」即可。

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