# 样式页由类型自己绘制：`StyleRow` + 宏生成

> **状态**：已实现（2026-11-24）。五步全部落地：接口 + 宏生成、`FormRows`、页面切换到类型自绘、删掉字段表路径（`StyleValue`/`StyleNode`/`StyleFields`/`PanelSkin` 已从代码中删除）、收尾（页面快照只切页/换所有者时 clone + 列表整块拖拽 ghost + 真机目视对照）。
> **2026-11-24 后续**：`FormRows` 与行协议已移入 widget kit，行版式成为共享样式 `Form`（见 §9）。
> **范围**：把 Preferences 的「字段表 + 一个通用页面」换成「每个样式类型自己绘制自己的页」，并让将来的对象 Inspector 复用同一套行版式。
> **配套**：[preferences-panel.md](./preferences-panel.md)（现状与已知边界）· [ADR 0008](../../adr/0008-editor-ui-architecture.md) · [widget-layer.md](./widget-layer.md)

---

## 0. 结论

1. `#[derive(StyleFields)]` 额外生成 `StyleRow::row`：结构体 → `sink.group` + 逐字段递归；叶子 → `&mut` 直接交给 Field widget；枚举 → `sink.variant` + 载荷作用域。页面不再需要 `StyleValue`、点分路径、字段快照。
2. 绘制落在一个 `RowSink` 上。正式实现是 `FormRows`（版式 + 选中 + 脏标记 + 提交），测试实现是记录器（本步已随 `style_row.rs` 落地）。
3. 面板自有的样式可以直接 `&mut` 绘制；共享样式（Toolbar/List/Field/Fonts/Modal/ColorPicker）的所有者是 `SharedStyles`，Preferences 只有 `&Frame`，**分层未决**，等真的改到时再定（§6）。

---

## 1. 为什么换

现状（`skin.rs` + `preferences.rs`）的成本，前两条是这次换的直接原因：

1. **每帧快照**：`frame.shared_styles.skins()` 每帧给每个共享样式跑一遍 `style_fields()`，Preferences 打开时约 240 个字段，每个字段带两个 `String`。面板页同理（`PanelHost::skins`）。
2. **路径代数**：`enum_blocks`、元素块归属、list scope、`enum_block` 都靠 `path.starts_with(&format!("{x}."))` 从扁平表还原父子。这个文件已经因此吃过一次亏（`open_groups` 早期按拼接路径比而不是按段比，`proportional.0` 永远匹配不上 `0`）。
3. **两套分派**：`StyleValue` 是值的擦除，`set_style_field` 是按路径的写回；而 Field widget 本来就是 `&mut` 直接借值的（`NumberField<N>`、`EnumField`、`ListField`）。页面这一层是三者里唯一需要擦除的。

字段表不是「构造结构体」——结构体本来就是各面板/注册表构造的；它是给一个不拥有值的通用页面看的快照。换成类型自绘，这三条一起消失。

---

## 2. 接口

> **2026-11-24 后续**：`RowSink`/`StyleRow`/`StyleEnum` 现在定义在 `kairos_editor_widgets::form`（行协议 + 绘制都是 widget kit 的事，且不含任何领域类型），`kairos_editor_component::style_row`/`skin` 以 `pub use` 保留同名路径，derive 生成的代码不用改；`StylePage` 仍留在 component（它需要 core 的 `StyleBlob`）。见 §9。

```rust
/// 绘制侧：一行一个 token。
pub trait RowSink {
    fn number<N: Number>(&mut self, name: &str, value: &mut N);
    fn text(&mut self, name: &str, value: &mut String);
    fn toggle(&mut self, name: &str, value: &mut bool);
    fn color(&mut self, name: &str, value: &mut Color);
    fn color_property(&mut self, name: &str, value: &mut ColorProperty);
    fn variant<E: StyleEnum>(&mut self, name: &str, value: &mut E);
    /// 空 name = 透明作用域（根，或 tuple 载荷）。
    fn group(&mut self, name: &str, body: impl FnOnce(&mut Self));
    /// 当前变体的载荷行：路径接在 enum 名下，布局挂在它的菜单按钮下。
    fn payload(&mut self, name: &str, body: impl FnOnce(&mut Self));
    fn list<T: StyleRow + Default>(&mut self, name: &str, values: &mut Vec<T>);
}

/// 值侧：一个样式值自己走一遍。
pub trait StyleRow {
    fn row(&mut self, name: &str, sink: &mut impl RowSink);
}
```

叶子（`f32`…`usize`、`bool`、`String`、`Color`、`ColorProperty`）手写 impl，各自调对应的 sink 方法；`Vec<T>` 调 `sink.list`。宏不按类型 match 控件，只看语法：别名、泛型、别的 crate 的类型都由类型系统解析，加一种叶子只需要加一个 impl。

---

## 3. 宏生成

结构体：

```rust
impl StyleRow for ConsoleStyle {
    fn row(&mut self, name: &str, sink: &mut impl RowSink) {
        sink.group(name, |sink| {
            StyleRow::row(&mut self.toolbar_height, "toolbar_height", sink);
            StyleRow::row(&mut self.icons, "icons", sink);
        });
    }
}
```

枚举：

```rust
impl StyleRow for EditorFontPriority {
    fn row(&mut self, name: &str, sink: &mut impl RowSink) {
        sink.variant(name, self);
        match self {
            Self::First => {}
            Self::Push(payload) => sink.payload(|sink| StyleRow::row(payload, "", sink)),
        }
    }
}
```

规则与现有 derive 一致：`#[style(skip)]` 不生成行；具名载荷字段在 `payload` 作用域内各自成行；单字段 tuple 载荷透明（以空 name 进入，字段直接挂在枚举路径下）。叶子行的名字由宏写死为字段名，嵌套的路径拼接由 sink 维护——生成代码里没有任何路径运算。

---

## 4. 切换后删除的东西（已全部删除）

- `kairos_editor_core::StyleValue`、`ListEdit`（以及 `SharedStyleAction`/`StyleAction` 里的值载荷）
- `skin::StyleNode::{collect,write,edit_list}`、`skin::StyleFields::{style_fields,set_style_field,edit_style_list}`、`StyleField`、`PanelSkin`、`SharedSkin`
- `PanelSkin.fields`、`PanelHost::skins`、`Frame.skins`、`PageState.fields/stale`
- `StyleAction::{SetField,EditList}`、`StyleEffect::{SetField,EditList}`、`SharedStyleAction::{SetField,EditList}`、`SharedStyleEffect::{SetField,EditList}`
- 路径代数：`group_path`、`element_path`、`enum_block`、`collapsed_above`、`open_groups`、`list_stack`、`PageWriter`（约 900 行）
- preferences-panel.md §6 第 14 条（depth/parent）随之作废：没有路径就没有前缀判断

保留：`StyleFile`/`save_skin`（RON 持久化）、主题两半、页面列表、Reset、`Commit` 手势、`humanize`、`LIST_MAX`、`StyleEnum`。

改名：derive 从 `StyleFields` 改为 `StyleRow`（它与 trait 同名，serde 的做法）；宏仍额外生成 `StyleBlob`/`StylePage`（struct）与 `StyleEnum`（enum）。

---

## 5. 前置条件（第 2 步）

1. **先定写回模型（§6）**：它决定 `StyleRow` 要不要 object-safe、`FormRows` 是具体类型还是泛型 sink。定下来之前不动 `PageWriter`。
2. **`FormRows` 抽取**：现在行版式（`row`/`row_at`/`foldout`/`list_header`/`list_footer`/`element_band`，约 600 行）长在 `PageWriter` 里，且是 path 驱动的。抽成一个实现 `RowSink` 的 widget：`FormRows` 持有 `Ui`、样式、`collapsed`/`selected`/`edit`/`dirty`/`list_chrome`，以及提交目标。抽取时先保持行为不变，用现有无头测试钉住。
3. **`NumberField` 幂等拖动**：`drag_value` 现在是 `current + delta`（`number.rs:306-323`），egui 的 pass 被 `request_discard` 重跑时会双加。今天没暴露，因为页面改的是副本、discard 的 `UiOut` 会丢；直接 `&mut` 绘制之后权威值也会被加两次。改成 egui `DragValue` 的做法（drag-start 值存 temp data，从 start 重算）。
4. **多 pass 规则**：`Frame` 的约定是「read Frame, draw, push actions」（`frame.rs:11-17`）。面板自绘自己的 `self.style` 属于面板自己的视图状态，和今天页面改本地副本同类；但提交（写盘）仍然只在指针抬起/失焦时发生。

---

## 6. 写回：面板和共享样式是同一个问题（2026-11-24 修正）

**修正上一版的一个错误判断**：上一版写「面板自有的样式可以直接 `&mut` 绘制」。实际上 Preferences 页拿不到任何所有者的 `&mut`：

- 面板实例在 `App` 的 `PanelHost::instances` 里（`panel/host.rs:24-31`），页面拿到的是 `Frame.skins` 快照；
- 共享样式在 `SharedStyles` 里，页面拿到的是 `Frame.shared_styles: &SharedStyles`；
- `Frame` 是只读的（`frame.rs:129-147`），`EffectExecutor` 只有 `host/dock/shared_styles/project`（`app.rs:377-397`）；
- 唯一能拿 `&mut` 的是 app 执行 effect 的那一段，和绘制不在同一个 pass。

所以「类型自绘」必须先定：**页面画的是谁的值，改了怎么送回去**。三条路：

### A. typed clone + 逐字段 `SetField`（推荐）

- 页面选页时拿一份 `Box<dyn StylePage>`（`self.style.clone()`；`StylePage` 是 `StyleRow` 的 object-safe 适配，`fn draw(&mut self, rows: &mut FormRows)`），每帧画这份 clone。
- sink 写值时记录 `(path, StyleValue)`（数字叶子由 `Number` 侧提供到 `StyleValue` 的映射），页面把它们转成现有的 `StyleAction::SetField`；owner 的 `set_style_field` 原样保留。
- 动作/effect/`PartialEq`/现有测试全部不动；删掉的是 `style_fields()`、字段表、每帧快照、页面里的路径代数。
- 代价：clone 在 pass 里被改，被 `request_discard` 丢弃的 pass 会多算一次拖动 → **`NumberField` 幂等是硬前提（§5.3）**。

### B. typed clone + 整体 replace

动作载荷变成不透明 blob（`Box<dyn Any>`）。`Action` 目前有 `Clone`/`Debug`/`PartialEq`（测试直接 `assert_eq!`），blob 会把这三个全部打断，或者逼 core 认识组件 trait。不推荐。

### C. 可变 `Frame`

`Frame` 里放 `&mut PanelHost` / `&mut SharedStyles`。破坏只读约定与 `Copy`，丢弃 pass 的写入不回滚。改动面最大。不推荐。

**决定（2026-11-24）：A2 —— 页面把整份 typed clone 交回去，仍然走 action → effect。**

- `StyleAction::Replace { panel, skin: Skin }`（shared 同样）→ `Effect::Style(StyleEffect::Replace)` → executor 调 `PanelHost::replace_skin` / `SharedStyles::replace`；`Commit` 仍是独立 action，顺序天然正确（先换值、后写盘）。
- `Skin` 是 core 里唯一的**非纯数据载荷**。`Skin` 包 `Box<dyn StyleBlob>`，`StyleBlob` 提供 `clone_blob/as_any/into_any/equals/debug_blob`，由 `#[derive(StyleFields)]` 为每个 struct 生成；`Action`/`Effect` 的 `Clone + Debug + PartialEq` 就靠这三样满足（两个 `Skin` 比较的是样式值本身，所以测试里的 action 列表仍然读得懂）。
- 为什么不是侧通道：`UiOut` 的约定是「视图只 push action」。侧通道虽然少改 core，但让一次写回绕过 reducer/effect，失去日志与测试位置。代价是 core 的 `style.rs` 文档要改写：跨边界的形状现在有两种（`StyleValue` 单 token、`Skin` 整份），其中 `Skin` 是唯一不做解释的载荷。
- 已落地：`StyleBlob`（core）、`Skin`（core）、`StyleAction/SharedStyleAction::Replace`、对应 effect 与 reduce 分支、executor 分支、`describe`/`is_quiet`、derive 生成的 `StyleBlob`/`StylePage`、`Panel::{skin_value, skin_file}`（trait 的 provided `skin_page`/`skin_meta` 由它们拼出页面快照）、`Panel::replace_skin`、`PanelHost::{page_metas, page, pages, replace_skin}`、`SharedStyles::{page_metas, page, pages, replace}`。
- 页面值的来源：app 在 Preferences 打开期间持有快照（`Frame.pages`），面板切页时从快照 clone 一页；快照在页面集变化或所有者被 `Replace`/`Reset` 时刷新，稳态帧只重建元数据列表做比较（`PanelHost::page_metas` / `SharedStyles::page_metas`，不拷贝样式值）。
- 新的自绘路径完全不需要 `StyleValue`/`StyleNumber`/`FormChange`；`StyleValue` 只剩旧页面（字段表）在用，第 3 步切页后随之删除（第 4 步已完成）。

---

## 7. 分阶段与验收

| 步骤 | 内容 | 验收 |
|---|---|---|
| 1（已做） | `RowSink`/`StyleRow` + derive 生成 + 记录器测试 | 结构体/枚举/列表/tuple 透明/`skip` 的生成行与 `style_fields()` 的路径、顺序逐一相等 |
| 2（已做） | object-safe `StylePage` + `FormRows`（`form.rs`）+ `NumberField` 幂等 + A2 写回（`Skin` 进 action/effect） | 已过：`form::tests` 3 个（toggle 改到 typed copy、`dyn StylePage` 绘制）、`style_row::tests` 4 个、`number.rs` 多 pass 拖动、`a_whole_skin_replaces_the_live_value`、`a_whole_style_replaces_the_panels_live_skin`；`kairos_editor_component` 265 passed（4 个 Fonts 相关失败是先于本次工作的） |
| 3（已做） | 面板页与共享页都切到 typed page：`Panel::skin_page` / `SharedStyleSealed::page`（宏生成）→ `Frame.pages` → `PageState` 持 `Box<dyn StylePage>`，`FormRows` 绘制，变化时发 `Replace` + `Commit`；页面列表用 typed page 覆盖同 key 的表页 | 已过：`a_typed_page_draws_itself_and_hands_back_the_whole_value`（导航→绘制→toggle→Replace(Skin)+Commit）、`a_real_style_page_draws_its_lists_and_enum_payloads`（Fonts 列表与枚举载荷）、`a_shared_style_page_draws_itself`（List 共享页）；生产路径全部 typed |
| 4（已做，2026-11-24） | 删表页路径：`PageWriter`、`draw_value`、`PageState.fields`、`Panel::skin/set_skin_field`、`PanelSkin.fields`、`PanelHost::skins`、`Frame.skins`、`StyleNode`/`StyleFields` 的字段表部分、`StyleValue`/`ListEdit`；把表页测试迁移到 typed 页（`preferences.rs` 重写为 49 个自绘页测试） | 已过：`kairos_editor_component` 249 + 7 + 10、`kairos_editor_core` 全绿、`kairos_editor_app` 80 + 4；`StyleValue`/`StyleNode`/`StyleFields`/`PanelSkin` 不再出现在三个 crate 里 |
| 5（已做，2026-11-24） | 收尾两件事：① 页面快照不再每帧整份 clone——每帧只比较元数据（id/title/file），页面集变化才整份重建，`Replace`/`Reset` 由 executor 只刷新那一页，面板切页只 clone 选中页；`Panel` 的 seam 改为 `skin_value`/`skin_file`，trait provided `skin_page`/`skin_meta` 拼出快照项。② `FormRows` 列表拖拽浮层：被拖块的整块（band + 行）由同一份 `element_block` 代码画进 tooltip 层的子 `Ui`，再用 `Ui::with_visual_transform` 平移到指针（布局不动，父 `Ui` 用 `advance_cursor_after_rect(child.min_rect())` 前进同样的高度），所以行数/高度/嵌套结构不会漂；子 `Ui` `disable()` 成半透明、无输入，`in_ghost` 只阻止嵌套 list 误跑 `finish`；邻居按 `ListChromeState::shift` 整体滑到预览槽。早期版本用「页面底色盖住原位」的做法会盖掉滑进来的邻居，已删除。顺带修正 `Preferences/Styles/EditorFonts.ron` 里枚举变 unit variant 前的过期形状（`priority: Push((...))` → `priority: Push`），pre-existing 的 Fonts 参考皮肤测试恢复 | 已过：`a_steady_frame_reuses_the_page_snapshot`、`the_page_snapshot_follows_panels_opening_and_closing`、`an_edit_refreshes_its_page_in_the_snapshot`、`a_reset_refreshes_its_page_in_the_snapshot`、`the_page_list_and_its_per_page_copies_share_their_metadata`、`pages_are_listed_and_replaced_through_the_host`（元数据与单页拷贝）、`dragging_an_element_lifts_its_whole_block`（整块随指针、band 与行同偏移、无底色覆盖、邻居上移）；组件 251 + 7 + 10、app 84 + 4、core / widgets 191 / macros 9 全绿 |

### 第 4 步顺带修掉的 `FormRows` 缺陷（原版式抽取时漏掉的）

1. **列表元素路径少了列表名**：`StyleRow::row(item, index)` 进入元素 struct 时没有把列表名压进路径，元素行 id 变成 `0.name` 而不是 `proportional.0.name`。修法：`list` 进入元素前 `self.path.push(name)`，元素本身以空名字走（透明 group），因此既不加第二层 band，也不产生重复 id。
2. **`ListChrome::begin/open/finish` 没接**：`bands` 跨 pass 累积、容器框不画、**拖拽排序的 `Move` 永远不发出**。现在 `FormRows::new` 调 `begin`，每个 list 头之后调 `open`，footer 之后调 `finish` 并应用 `Move`。
3. **没有被行消费的点击不会清选中**：`FormRows::clicked()` 现在被 `typed_rows` 使用，页面空白处的按下会清掉高亮。

> **顺序说明**：原计划先抽 `FormRows`；实际先落接口与宏生成，因为 `RowSink` 的方法签名就是 `FormRows` 的边界——先定接口再抽版式，避免版式 API 反过来限制生成代码。第 2 步按 `RowSink` 实现 `FormRows`，再改 `PageWriter`。

### 第 5 步的目视对照（2026-11-24）

真实编辑器（1280×848，dark，面板以浮动窗口打开，与用户路径一致）启动并打开 Preferences，抓了 Console / Fonts / List / Color Picker 四页（截图在 `target/visual-check/`，gitignored）：

- 行距、斑马纹、折叠组 band 与缩进、标题/文件名/分隔线、页脚分隔线与 Reset 位置都和旧表页一致；
- enum 行是带箭头的下拉（Fonts 页的 `Priority`）；列表有计数框、元素 band（拖拽把手 + 折叠 caret）、容器框与 +/− 按钮；颜色行是 swatch + 吸管按钮。

没有发现需要改版式的回归。拖拽 ghost 另用脚本化鼠标（CGEvent down → 分步 move → 按住截图 → up）在真机上抓了 mid-drag 截图：整块（band + 三行）跟随指针半透明浮起、邻居滑到预览槽、指针抬起后列表正确重排；回归由第 5 步的 `dragging_an_element_lifts_its_whole_block` 钉住（整块随指针、band 与行同偏移、无底色覆盖、邻居上移）。

---

## 8. 风险

- **编译期约束变宽**：derive 现在无条件生成 `StyleRow`，任何字段类型都必须有实现（或 `#[style(skip)]`）。新增叶子类型要同时加 impl；错误信息是 trait bound 不满足。
- **id 稳定性**：行 id 仍是 `(页面, 路径)`，`FormRows` 必须用同一套，否则测试与焦点会漂。
- **枚举载荷**：生成代码里 `sink.variant` 之后才 match 载荷，切换变体后本 pass 画的是新变体的行；页面不再需要 `stale` 重读。
- **拖动预览（已做，2026-11-24）**：`FormRows` 用 `element_block` 把被拖块整块画进 tooltip 层的子 `Ui`（`disable()` 半透明、无输入），`with_visual_transform` 平移到指针，父布局按子 `Ui` 的 rect 前进——行数/高度/嵌套都由同一份绘制代码得出，drop 数学看到的仍是真实高度；邻居按 `ListChromeState::shift` 整体平移（band、行、id 一起）。被拖块不再在原地画任何东西，所以不会盖住滑进来的邻居。回归测试见第 5 步。
- **视觉回归**：表页测试的像素断言（斑马纹、缩进、列宽、值框填充）已随测试迁移；真实编辑器的目视对照见第 5 步的验收记录（2026-11-24：启动编辑器、打开 Preferences 截图对照）。

---

## 9. 后续（2026-11-24）：`FormRows` 移入 widget kit，行版式成为共享样式

### 问题

`form.rs` 里的 `FormRows` 直接拿 `&PreferencesPanelStyle` 取行高、列宽、斑马纹和组头颜色：**「表单行怎么画」被绑在 Preferences 一个面板的皮肤上**。于是

- 行版式没有通用性：将来的对象 Inspector（同样是「值自己画成行」）要么继承 Preferences 面板的几何，要么复制一份；
- `fn color` / `fn number` / 折叠带 / 页码带这些方法本来就不含面板知识，却和面板类型放在同一个文件里；
- 想单独测「行怎么排」要构造整个 Preferences 皮肤。

### 拆分

| 归属 | 内容 |
|---|---|
| `kairos_editor_widgets::form`（新） | 行协议 `RowSink`/`StyleRow`/`StyleEnum`、`humanize`，以及绘制侧 `FormRows`、跨帧状态 `FormState`/`FormEdit`、行版式 token `FormStyle`。布局、斑马纹、折叠带、列表 chrome + 整块拖拽 ghost、选中/编辑 buffer、dirty/changed 标记都在这里；不认任何领域类型。 |
| `kairos_editor_component::shared_style::form`（新） | `EditorFormStyle`：行版式的 serde 皮肤（斑马两灰、标签/值列宽与间距、行高、组头底色/悬停/字号/高/缩进、行内边距），`Preferences/Styles/EditorForm.ron`，Preferences 自动多出 `Form` 页。和 Field/List 一样由 shell 所有，面板从 `Frame::form_style()` 取，在调用点 `tokens()` 投影成 `FormStyle`。 |
| `kairos_editor_component`（保留） | 页面机制：`StylePage`（object-safe + core `StyleBlob`）、`#[derive(StyleRow)]` 宏、`SharedStyle` 注册、`Replace`/`Commit`/`Reset`、`LIST_MAX`（作为 `FormRows::new` 的 cap 参数传入，策略不跟 widget 走）。`style_row`/`skin` 用 `pub use` 重新导出 widget 的 trait，所以 derive 生成的路径与 `#[derive(StyleRow)]` 的调用点都不变；宏只把 `StylePage::draw_page` 的 `FormRows` 路径指向 `kairos_editor_widgets::form`。 |
| `panels/preferences`（收缩） | 只剩面板 chrome：页列表、标题与文件行、分隔线、内边距、页脚 Reset；`PreferencesPanelStyle` 删掉 13 个行版式 token，`PreferencesPanel.ron` 同步收缩。 |

### 为什么这么放

- **删除测试**：删掉 `FormRows`，行版式、列表 ghost、选中/编辑状态会重新散落回每个画表单的面板——它确实在承担复杂性，不是 pass-through。
- **crate 边界即约束**：行协议只用到 `f32`/`String`/`bool`/`Color`/`ColorProperty` 和 `StyleEnum` 的变体表，没有一个领域类型，所以它可以整体放进只依赖 egui + `kairos_color` 的 widget crate；而 `StylePage` 需要 `kairos_editor_core::style::StyleBlob`，不能进去，于是页面机制留在 component。这条切线和 §2 里「绘制侧 / 值侧」的分法正好重合。
- **二期一致性**：行版式与 Field/List/ColorPicker 一样是 widget 家的皮肤——一个 RON 文件重排整个编辑器的属性行；Preferences 自己的 `PreferencesPanel.ron` 不再兼职当「全编辑器表单皮肤」。

### 验收

- `cargo test-crate kairos_editor_widgets`：195 passed（新增 4 个 `form` 单测：两帧无输入不脏、toggle 点击写回、斑马纹、`humanize`）。
- `cargo test-crate kairos_editor_component`：251 passed；新增 `shared_style::form` 投影/参考皮肤/斑马纹 3 个测试、`style_row::tests::a_boxed_page_draws_itself_into_the_form_widget`，以及 `panels::preferences::tests::the_form_rows_draw_from_the_shared_skin`（替换注册表里的 `Form` 皮肤会在下一帧重新给行上色，证明页面读的是共享皮肤而不是本地常量）。
- `cargo check --workspace --all-targets` 通过。

