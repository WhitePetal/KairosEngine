# Widget / Component 分层设计

> **状态**：提案（未实施；本阶段只定方向 + 原型，不写生产代码）
> **日期**：2026-10-30
> **配套**：[总架构](./README.md) · [重写计划](./rewrite-plan.md) · [ADR 0008](../../adr/0008-editor-ui-architecture.md)
> **调研**：[ui-widget-component-layer-survey.md](../../research/ui-widget-component-layer-survey.md) · [editor-ui-widget-catalog.md](../../research/editor-ui-widget-catalog.md)
> **原型**：[`prototypes/editor-ui/`](../../../prototypes/editor-ui/)（合并后的统一工作区）

---

## 0. 结论（TL;DR）

你的判断是对的：现状每个 `.rs` 文件都是「**外观 + 逻辑**」一体（`ProjectWindow` 622 行里既有树行怎么画、也有资产树怎么改；`material.rs` 1506 行里既有每行样式、也有保存/回滚/预览）。解决办法不是「把文件拆小」，而是**按职责分层，并让编译器强制这条边界**：

```
L0  Design Tokens / Theme      ← 颜色、间距、字号、圆角、行高（唯一外观来源）
L1  Widgets（纯外观 / 无状态）  ← 只认 props，输出事件；不认 Asset/Entity/Document/Action
L2  Components（功能组件）      ← 读 EditorState、组合 Widgets、发 Action；不含像素细节
L3  Panels / Assembly（装配）   ← dock 面板注册、布局、面板实例
```

**关键设计**：L1 独立成 crate `kairos_editor_widgets`，只依赖 egui，**不依赖 core/engine**——于是「widget 里混进业务逻辑」不是靠 review 拦，而是**编译不过**。L2 在 `kairos_editor_ui`，只做「状态 → props、事件 → Action」的适配；适配函数是纯函数，可以脱离 egui 单测（这正是把逻辑从 .rs 里抽出来的落点）。

L2 与总架构（MVU）的关系：总架构里的 `Panel`/component 就是这里的 L2；L1 是它下面新增的一层纯外观库。**Widget 层不改变 store/reducer/effect 设计，只改变「视图内部怎么写」**。

---

### 0.1 这个分层与成熟方案的对应

> 注：React 谱系的 smart/dumb 划分后来被 Dan Abramov 本人收回（「不要教条地二分」）。Kairos 的分层依据**不是「有没有 state 字段」**，而是一条可编译检查的边界：**是否知道领域类型 / 是否发 Action**。widget 不知道 = 纯外观。

| 方案 | 纯外观层 | 功能/逻辑层 | 备注 |
|---|---|---|---|
| Flutter | `StatelessWidget` / 通用 widget | `StatefulWidget` + 应用逻辑 | 保留式 widget 树；egui 不等价 |
| React/Redux | presentational components | container components / hooks | 最经典的「哑组件 vs 容器组件」划分 |
| **Rerun re_viewer** | **`re_ui`（独立 design-system crate）** | `re_viewer` / 各 feature 组件 | 与本提案同构度最高；re_ui 正是独立 crate |
| Zed | `ui` crate（Button/IconButton/Label/List/h_flex） | `editor`/`project`/`workspace` crates | GPUI retained element，但分层思路相同 |
| iced | `iced::widget`（纯渲染 + Message 事件） | 应用 `update`/`view` | widget 只出 Message，业务在 update |
| **Kairos（本提案）** | **`kairos_editor_widgets` crate（L0+L1）** | **`kairos_editor_ui` components（L2）** | widget Output → Component → Action |

共同点（也是本提案的依据）：**纯外观层都是独立 crate/模块，且业务逻辑层只做「数据 → props、事件 → 意图」的翻译**。区别只在语法（Flutter/React 有 retain 树，egui/iced 是立即模式函数），所以不要照抄它们的组件框架，只照抄分层与契约。

## 1. 现状：为什么「一个 .rs 同时带外观和逻辑」会烂

以三个真实文件为例（数据来自 [组件目录审计](../../research/editor-ui-widget-catalog.md)）：

| 文件 | 行数 | 外观部分 | 逻辑部分 | 烂点 |
|---|---|---|---|---|
| `project_window.rs` | 622 | 树/内容面板/底部栏的绘制、drag ghost、配色 | 资产树增删改、registry 落盘、watcher 重扫、选中/重命名状态机 | 画到一半调 `self.model.graph.create_node(...)`；`Cell` 为了在 `&self` 里写状态 |
| `inspector/material.rs` | 1506 | 每个字段的手工行布局、缩略图、shader 菜单树、3D 预览 | 加载/序列化 Material、edit-in-place、dirty、保存/丢弃、preview 相机 | `draw` 里直接改运行时材质；`Arc<Mutex<Option<SerializedMaterial>>>` 快照随消息传 |
| `preferences_window.rs` | 414 | 网格表单、样式页列表 | 遍历所有 drawer 反射样式字段、按索引写回、写 TOML | 每帧发 `UpdateUIStyle`，选中 ToolBar 页会每秒写盘 |
| `ui_style_fields.rs` | 138 | — | `Vec<StyleField>` 按下标对应 `update_style` 的 `style_fields[0..n]` | 改字段顺序 = 运行时 panic/静默错配 |

共同点：**外观代码与逻辑代码共享同一份可变状态**，所以既不能复用外观（每个面板一套按钮/行/树），也不能单独测试逻辑（要起 egui + Engine）。分层要解决的就是这两个「不能」。 量化一下「重复」：`egui_extras::TableBuilder` 11 处、`body.row` 32 次、`row.col` 70 次、`ComboBox::from_id_salt` 18 个、`DragValue::new` 21 个、`ui.checkbox` 9 个、`color_edit_button` 2 个，散落在 7 个文件里各自实现「标签 + 编辑器」的行布局；4 个文件各自实现缩略图；6 个文件各自在 `on_exit` 拼确认对话框。

---

## 2. 三层契约

### 2.1 L0：Design Tokens / Theme

- **唯一外观来源**：颜色（bg/panel/text/muted/accent/border/error/success/warning）、间距（xs/sm/md/lg/xl）、字号（caption/body/heading/title）、行高、圆角、图标尺寸、动效时长。
- 现状是「每个窗口一个 `XxxStyle` TOML + 自己的 `*Style::new()` + 偏好设置反射」；目标是**一个 `Theme`**（可含 per-panel override），由 host 每帧准备，widget 只读。
- 映射方式：现有 8 个 `Preferences/Styles/*.toml` 的字段先 1:1 映射进 `Theme`（保证视觉不变），再逐步归并重复 token。
- **widget 不得出现魔法颜色/尺寸**；component 也不得（只有布局决策）。
- 主题读写：偏好设置页编辑 `StyleState` → host 生成新 `Theme` → 全 UI 即时生效（替换现在「按 draw 序反射 `get_style_fileds`」）。
- **优先复用 egui 0.35 自带的主题设施**（调研结论）：`egui::Theme` / `set_style_of` / `widget_style(Classes, WidgetState, ButtonStyle)`。因此 Theme 映射分两层：`StyleState → egui Style + UiTokens（自定义 token 集）`；widget 用 `ui.style()` 的 class/state 取样式，而非手传颜色。

### 2.2 L1：Widgets（纯外观，无状态）

**契约（硬性）**：

1. 只依赖 egui；**不得**出现任何领域类型：`Guid`/`AssetKind`/`NodeIndex`/`Entity`/`Document`/`Action`/`EditorState`。
2. **props 进、事件出**：展示数据用 struct 字段；交互结果用返回值（`egui::Response` 或自定义 `Output`），**不接收回调**（布局闭包 `PanelChrome::body(|ui| ...)` 例外，那只是组合）。
3. **跨帧无状态**：语义状态（展开/选中/值）全部由调用方 props 传入；纯瞬态（popup 开合、滚动、文本光标、hover）允许存 egui `Memory`（稳定 `Id`）。
4. **主题驱动**：所有视觉值来自 `Theme`。
5. 文本一律 `impl Into<WidgetText>`；图标通过 `IconId` + `IconProvider`（避免 widgets crate 依赖资产系统）。
6. 每个交互控件必须设置 `Response::widget_info`（role/label/state）与 accessible label——这是 headless 测试（egui_kittest 按 label/role 查询点击）能写出来的前提；不 panic、不 IO、无全局状态。

**Rust/egui 形态**：

```rust
// 原子：实现 egui::Widget，天然只返回 Response
pub struct Button<'a> { label: WidgetText, variant: ButtonVariant, icon: Option<IconId>, .. }
impl egui::Widget for Button<'_> { fn ui(self, ui: &mut egui::Ui) -> egui::Response { .. } }

// 复合：显式 Output（一个控件可能产生多种事件）
pub struct TreeView<'a, Id: Copy + Eq> { rows: &'a [TreeRow<Id>], selected: Option<Id>, .. }
pub struct TreeViewOutput<Id> {
    pub clicked: Option<Id>,
    pub double_clicked: Option<Id>,
    pub toggled_open: Option<Id>,
    pub context_menu: Option<(Id, egui::Pos2)>,
    pub drag: Option<DragEvent<Id>>,   // Started/Moved/Stopped/Consumed
}
impl<Id: Copy + Eq> TreeView<'_, Id> { pub fn show(self, ui: &mut egui::Ui) -> TreeViewOutput<Id> { .. } }
```

**为什么这样分工**：egui 本来就是立即模式 + `Widget` trait；组合就是函数调用，不需要自创「组件框架」（不搞 virtual DOM / children / 泛型 props 系统）。widget 泛型只用于**不透明 id**（`Id: Copy + Eq`），组件侧把 `AssetGuid`/`NodeIndex` 映射成 `RowId(u64/&str)`。

**Widget 目录（首版目标 20-25 个，详见 [catalog](../../research/editor-ui-widget-catalog.md)）**：

| 组 | Widget |
|---|---|
| 原子 | `Button`(`Primary/Secondary/Ghost/Danger`)、`IconButton`、`Checkbox`、`Switch`、`Radio`、`TextInput`、`SearchBox`、`NumberInput/SliderRow`、`ComboBox`、`ColorSwatch/ColorEdit`、`Badge`、`Spinner/ProgressBar`、`Label/Heading`、`Separator`、`Tooltip` |
| 容器/骨架 | `PanelChrome`（标题+工具栏槽+body）、`Toolbar`（item 数据数组）、`FormSection`（可折叠分组）、`StatusBar`、`Breadcrumb`、`EmptyState` |
| 数据行 | `TreeView/ListRow`（缩进+展开+图标+label+suffix+选中+右键+DnD）、`PropertyRow/PropertyList`（label + 编辑槽 + reset、dirty 指示） |
| 浮层 | `ContextMenu/Menu`（item 数据：id/label/icon/shortcut/enabled/checked/分隔/子菜单）、`Modal/ConfirmDialog`（title/body/buttons → confirm/cancel）、`Toast` |
| 媒体 | `Thumbnail/ImagePreview`（`egui::TextureId` + 尺寸）、`DragGhost/DropTarget` |

审计给出的具体目录（[editor-ui-widget-catalog.md](../../research/editor-ui-widget-catalog.md)）共 **25 个 widget**：8 个表单/文本底座（P0）+ 6 个集合/树 + 5 个图像/视口 + 4 个 chrome + 2 个反馈；目标实现约 2,200-2,600 行（含 kittest），替代现状约 5,000-6,000 行手写绘制，**UI 绘制代码预计减少 50-60%**。其中 ~40% 的「widget 能力」现状其实已经存在（dock 6,762 行 + `egui_ext.rs` 223 行的通用 helper），只是没被承认为独立层。

> egui 0.35 已内置 `egui::Modal`、`egui::containers::modal` 等容器；`Modal/ConfirmDialog` widget 直接包装它们，不自绘。

### 2.3 L2：Components（功能组件）

**契约**：

1. 读 `&EditorState`（或更窄的只读投影），**不拿 `&mut` 领域状态**；写只通过 `Action`。
2. 组合 L1 widgets；**不含像素/配色细节**，只做布局决策（顺序、分区、折叠、哪块用哪个 widget）。
3. 会话态（搜索词、展开集合、列宽、lock、重命名输入、drag 中状态）放 `ViewState`，不放 domain、不放 widget。
4. **适配函数是纯函数**（关键）：
   ```rust
   // domain → props（可单测，无 egui）
   fn project_rows(state: &ProjectState, view: &ProjectViewState) -> Vec<TreeRow<RowId>>;
   // widget 事件 → Action（可单测，无 egui）
   fn on_tree_output(out: &mut UiOut, o: TreeViewOutput<RowId>);
   ```
   绘制函数只做 `let rows = project_rows(...); let o = TreeView::new(&rows, ...).show(ui); on_tree_output(out, o);` —— **这就是「把逻辑从 .rs 里抽出来」的落点**：逻辑在纯适配函数里，绘制只剩装配。
5. 面板组件签名（与总架构 §3.5 的 `Panel` 对齐）：
   ```rust
   pub fn project_component(ui: &mut egui::Ui, frame: &Frame, out: &mut UiOut,
                            view: &mut ProjectViewState, instance: PanelInstance);
   ```
6. 新功能 = 新 Action + reducer + 一个适配函数/一个组件；不改中心 match、不加消息变体。

#### 2.3.1 「逻辑去哪了」的三段式（before / after）

现状（`material.rs` 的典型写法：绘制、业务判断、IO 全在一个分支里）：

```rust
if ui.button("Apply").clicked() {
    self.save_material(world, &handle, &snapshot);   // IO + 改运行时状态
    self.model.dirty.set(false);                     // 状态藏在视图里
}
```

分层后：

```rust
// L2 component（只接线）：widget 事件 → Action
if Button::primary("Apply").ui(ui).clicked() {
    out.emit(Action::Document(DocumentAction::Save { id: doc.id }));
}

// L2 adapter（纯函数，可单测）：state → widget props
fn material_props(doc: &Document) -> Vec<PropertyRowProps>;
fn on_property_output(out: &mut UiOut, doc: DocumentId, key: PropKey, o: PropertyRowOutput);

// L1 widget（只画）：label + 编辑槽 + reset
let o = PropertyRow::new("Blend", &value_text).show(ui);

// core reducer（唯一改状态的地方）
fn save(state: &mut EditorState, id: DocumentId) -> Vec<Effect> {
    let doc = state.documents.get_mut(id)?;
    doc.pending_save = RequestSlot::new(ticket, doc.edit.clone());
    vec![Effect::WriteDocument { doc: id, path: doc.path.clone(), bytes: serialize(&doc.edit) }]
}
```

**自检五问**（决定代码放哪一层）：

1. 它只决定「怎么画」吗？→ L1 widget。
2. 它知道 `AssetKind`/`Document`/`Guid` 等领域概念吗？→ L2 component/adapter（widget 不该知道）。
3. 它会改变领域状态吗？→ core reducer（component 只能发 Action）。
4. 它读 World / 磁盘 / tokio / 原生对话框吗？→ Effect + host。
5. 它能在没有窗口、没有 GPU 的情况下被单测吗？→ 如果答案是「不能」但它是纯逻辑，说明放错层了。

### 2.4 L3：Panels / Assembly

- `PanelKind` 注册表：每项 = title + 默认位置 + 组件 show fn + closable + 多实例策略。
- `PanelInstanceId` 决定 `ViewState` 用哪份；dock 只存 id（总架构 §3.5）。
- 装配层不写绘制，只接线。

---

## 3. Crate 结构与编译期约束

```
kairos_editor_widgets（leaf，L0+L1；只依赖 egui）    kairos_editor_core（leaf，纯领域；不依赖 egui）
                    │                                             │
                    └──────────────► kairos_editor_ui ◄───────────┘
                                            │
                                      kairos_editor（host + bin）
```

- **widgets crate 的依赖只有 egui** ⇒ 想写 `use kairos_editor_core::...` 直接编译失败。这是本设计最重要的一条：**边界由 cargo 强制，不靠自觉**。
- `kairos_editor_ui` 不依赖 `kairos_editor`（host）⇒ 组件不可能做 IO / 改 World。
- 工程化检查（建议进 CI / deny.toml）：
  - `cargo tree -p kairos_editor_widgets | grep kairos` 必须为空；
  - `kairos_editor_ui` 的依赖里没有 `kairos_editor`；
  - widget 目录禁止 `Cell/RefCell/Mutex/unsafe`（clippy lint + review）。

**推荐目录（生产）**：

```text
kairos_editor_widgets/src/
  lib.rs
  theme/         mod.rs tokens.rs theme.rs（Theme + 从 StyleState 构建）
  icon.rs        IconId + IconProvider
  atoms/         button.rs checkbox.rs text_input.rs numeric.rs combo.rs color.rs label.rs progress.rs
  chrome/        panel.rs toolbar.rs status_bar.rs breadcrumb.rs form_section.rs empty_state.rs
  rows/          tree.rs list.rs property.rs
  overlays/      menu.rs modal.rs toast.rs tooltip.rs
  media/         thumbnail.rs drag_ghost.rs
  gallery.rs     （dev bin：全 widget storybook）

kairos_editor_ui/src/
  frame.rs       Frame<'a> / UiOut / ViewStates
  components/    project.rs inspector/{mod.rs, material.rs, texture.rs, text.rs, ...}
                 scene.rs game.rs toolbar.rs preferences.rs console.rs
  adapters/      纯适配函数：project.rs、inspector/*.rs（state→props、output→Action）
  docking/       现有 dock（存 PanelInstanceId）
```

约定：**绘制文件里不写 `match domain_type` 的业务分支**——业务分支只允许出现在 `core/reduce`（状态迁移）或 `adapters/`（投影与事件映射）；绘制文件只做「取数据 → 调 widget → 收 Output → 丢给 adapter」。

**主题归并的量化依据**（审计数据）：现状有 20 个 per-window `*Style` struct、21 个 TOML、`paths.rs` 30 个路径常量、132 个 TOML key（108 个不重复），但 Preferences 只能编辑 4 个窗口的 21 个字段——**可编辑覆盖率仅约 19%**。这正是「外观配置散落在每个窗口自己的 .rs 里」的代价；收敛成 `Theme` + `StyleState` 后，偏好设置可以覆盖全部 token（按 schema 生成），且 widget 不再各自读 TOML。

- 现状对应关系：把 `global_styles.rs`（字体/图标/颜色）与各 `*Style` TOML 合并成 Theme；`egui_ext.rs`（`UiExt`）里的通用 helper 归入 widgets；`ui_style_fields.rs` 的反射式样式机制删除，改成 Theme 展示/编辑。

---

## 4. 状态归属（三层 + 引擎）

| 状态 | 归属 | 说明 |
|---|---|---|
| Theme tokens / 字体 / 图标集 | host 生成 `Theme`（源是 `StyleState` + 字体/图标资源） | widget 只读；偏好设置改的是 `StyleState` |
| widget 纯瞬态（popup 开合、滚动、光标、hover） | egui `Memory`（稳定 `Id`） | 不持久化、不进 undo |
| 组件会话态（搜索词、展开集合、列宽、lock、重命名 buffer、drag 状态） | `ViewStates`（按 `PanelInstanceId`） | 可持久化但非权威数据 |
| 领域状态（project/documents/selection/layout） | `kairos_editor_core::EditorState` | 单一真值，reducer 才能改 |
| GPU 资源（egui texture handle） | host `HostView` | widget 只收 `egui::TextureId` 等显示值 |
| 引擎世界（ECS/相机） | `Engine.world` | L2 只读 `&World`，变更走 Effect |

**一句话判据**：widget 只认「画什么」（props）；component 负责「数据从哪来、事件到哪去」；core 负责「状态怎么变」；host 负责「世界怎么动」。

---

## 5. 测试策略

| 层级 | 怎么测 | 工具 |
|---|---|---|
| L0/L1 widget | headless 渲染一帧不 panic；交互控件用合成输入（`RawInput` 指针事件）断言 `Response/Output`；未来加 `egui_kittest` 快照 | egui `Context::run`（无需 GPU/window）；可选 `egui_kittest` |
| L1 gallery | 全部 widget 的 storybook 页面（假数据），编译 + headless 跑一遍 | 单独 bin/测试 |
| L2 适配函数 | `state → props`、`output → Action` 纯函数单测（无 egui），覆盖大部分交互逻辑 | 普通 `#[test]` |
| L2 组件 | headless 跑组件 + 断言 `UiOut` 里的 Action 序列 | egui headless |
| L3/集成 | 面板打开/关闭/布局、与 core reducer 联动 | 集成测试 |

要点：**把「逻辑」从绘制文件里赶出去**靠的就是第 3 行——适配函数是纯的，不需要窗口/GPU 就能测；这直接回应现状「0 个交互测试」。 测试基建路线：egui 0.35 自带 `WidgetInfo`/AccessKit；`egui_kittest` 与 egui 同版本（0.35）可按 label/role 查询与点击、也能做快照，但官方建议优先写普通 Rust 断言、克制图像快照（慢且脆）。原型阶段用 `Context::run` + 合成 `RawInput` 同样能做点击断言（见 [`prototypes/editor-ui/`](../../../prototypes/editor-ui/)）。

---

## 6. Before / After：以 ProjectWindow 为例

**现在（622 行）**：`ProjectWindowStyle` + `ProjectWindowModel`(graph/registry/selected/active_directory/renaming/drag/watcher) + 业务方法（create/rename/delete/open/drag）+ `ui()` 里 hierarchy/content/bottom bar 全画 + `Drawer` trait 10 个方法（8 个空实现）。

**之后**：

| 位置 | 内容 | 估算 |
|---|---|---|
| `core::ProjectState` + reducer + effects | 资产树增删改、registry 落盘、watcher 应用、选中语义 | 从 `ProjectWindow` 迁出 ~200-250 行（可测） |
| `ui/components/project.rs` | 布局 + 适配函数：`project_rows()` / `on_tree_output()` + `PanelChrome + SearchBox + Breadcrumb + TreeView + StatusBar` | ~120-180 行 |
| `ViewState` | `expanded`、`search`、`renaming`、`drag` | ~40-60 行 |
| widgets | PanelChrome/TreeRow/SearchBox/Breadcrumb/ContextMenu/StatusBar/Menu | 复用，0 行业务 |
| 删除 | `Drawer` impl、style struct 的绘制字段、逐行手绘树行 | −150~200 行，且其他 10 个面板同样受益 |

其他面板同理（审计保守估计）：`inspector/material.rs` 1506 → 330-390 行（−75%）；`preferences_window.rs` 414 → 100-130 行（−72%）；`project_window.rs` 622 → 155-175 行（−72%）。删掉的行不是消失：约 700-900 行上移 core（可测的领域逻辑），其余压进 widget kit（复用后不再重复）。

各面板的 widget 映射：`material.rs` 的字段行 → `PropertyRow/PropertyList/SliderRow/ComboBox/ColorSwatch/TextureDropTarget`；`preferences_window.rs` → `FormSection + PropertyRow` 由 schema 生成；`tool_bar.rs` → `Toolbar + MenuBar + IconButton`。

---

## 7. 反模式（这个方向做错的常见方式）

1. **widget 里出现领域类型**（`AssetKind`/`Guid`）→ 复用性归零、逻辑渗回。→ crate 边界强制。
2. **widget 接收回调闭包**（`on_click: impl FnMut(&Domain)`）→ 回调地狱回归、生命周期/借用爆炸。→ 返回值/Output 结构。
3. **widget 持有跨帧状态**（内部 `expanded: HashSet`）→ 状态双源、无法持久化/测试。→ 语义状态走 props，瞬态走 Memory。
4. **props 爆炸**（一个 widget 15 个参数）→ 拆成 2-3 个 widget 或引入子组件；单 widget props 目标 ≤ 8-10 个。
5. **自造组件框架**（virtual DOM / 泛型 children / 响应式绑定）→ egui 里组合就是函数调用，别造框架。
6. **样式当 props 传**（`color: Color32` 到处传）→ 视觉不一致、主题失效。→ 变体枚举 + Theme token。
7. **组件直接改状态 / 做 IO** → 回到现状。→ 只 emit Action。
8. **每个组件各自缓存大投影**（每帧重建 10k 行树 props）→ 大数据列表需要 revision 缓存（在 logic 段计算，绘制只读）；先保证正确，再按 profiling 加缓存。

---

## 8. 迁移策略（仅方向，本阶段不动生产代码）

1. **原型验证（本轮）**：`prototypes/editor-ui/` —— 一个 workspace 三个 crate，egui 0.35 下验证 widgets crate 纯度、theme、复合 Output、无头测试，以及 **widget 点击 → Action → 真实 core reducer → Effect** 的完整链路（详见原型 README）。
2. **Phase 0.5（重写计划的插入阶段）**：生产实现 `kairos_editor_widgets`（Theme + 首版 20 个 widget + gallery bin）+ `kairos_editor_ui` 骨架；用 gallery 做视觉评审；把现有 `global_styles`/`*Style` 映射成 Theme（视觉 1:1）。
3. **Phase 1**：**先迁 ProjectPanel**（widget 需求最集中、验证 TreeView/ContextMenu/StatusBar），再迁 Inspector 系列（验证 PropertyRow 抽象），最后 Toolbar/Preferences/Console。
4. 旧面板在对应新面板验收后再删；widgets 与旧样式可以短期共存（Theme 映射保证外观不变）。
5. 每阶段质量门：`cargo test-crate kairos_editor_widgets`（无 GPU）、`cargo test-crate kairos_editor_ui`、`cargo check --workspace --all-targets`。

---

### 8.1 原型验证结果（2026-10-30，合并后）

[`prototypes/editor-ui/`](../../../prototypes/editor-ui/) 是**合并后的统一原型工作区**：一个 `[workspace]`、一份 Cargo.lock、一份 README、**五个 crate（core / ui-widgets / components / viewport / app）、126 个测试**（125 headless + 1 doctest；其中 7 个 full-stack、14 个 panel host、11 个 viewport slice），全部离线编译运行（egui 0.35 + wgpu 29 来自本机 cargo cache）：

| crate | 依赖 | 内容 | 测试 |
|---|---|---|---|
| `editor-core`（`editor-core-prototype`） | 零外部依赖 | `EditorState` + reducer/Effect/文档/undo/命令；含 `ProjectState::demo()` 与 create/delete/rename 领域动作 | 20（1 lib + 19 flow） |
| `ui-widgets`（`ui-widgets-prototype`） | 只依赖 egui | 13 atom + 13 compound + Theme/Icon + headless harness；purity 测试扫描领域类型 | 14（8 atoms + 5 compounds + 1 purity） |
| `editor-components`（`editor-components-prototype`） | core + widgets + egui | `Frame`/`UiOut` + Project/Inspector 组件：真实 `ProjectState.assets` → 树 props、真实 `Document` → 属性行；只发真实 `Action` | 15（8 纯映射 + 7 full-stack） |

**合并比分开多证明了什么**：

1. **一条真实链路**：`App::new()` → dispatch 真实 Action → 用 headless harness 渲染组件 → 合成 click/double-click/drag → 收集 `UiOut` → `app.dispatch` 真实 reducer → 断言 `EditorState` + `Vec<Effect>` → `App::complete`。旧的 fake_state/actions 已删除，组件产出的就是 core 的 `Action`。
2. **跨 slice 规则在 UI 路径上成立**：点资产行 → `SelectAsset` → reducer 同时改 selection/layout.focus/loading 并发 `FocusPanel`+`LoadAsset{ticket}`；completion 再发 `SpawnPreview`。
3. **异步保存语义没被 UI 破坏**：拖 slider → `SetMaterial` → dirty；Save → `Effect::SaveDocument{ticket,bytes}`；`App::complete(DocumentSaved)` 后才清 dirty。
4. **widget 层确实无领域**：组件从 `AssetEntry.path` 推导文件夹树、把 `TreeOutput` 翻译成 Action；widget 只认 `u64` row id 与字符串。

**接入真实 core 后发现的缺口**（生产 Phase 1/3 要解决，原型用权宜方案）：

- core 没有项目树：组件按 path 推导文件夹；`CreateAsset { parent: Option<AssetId> }` 无法寻址没有 id 的推导文件夹（生产建议 parent 用 path 或稳定 `FolderId`）。
- `DocumentAction::Open` 直接携带 content（ADR 的设计是异步 `LoadDocument` → completion）；原型双击只能按 kind 伪造占位内容。
- `TreeRow` 的 `u64` id 与 `AssetId`/无 id 文件夹不对齐（原型用 FNV-1a(path) 生成 + 反查）；widget kit 后续应接受 opaque id 或由 view 预留映射。
- 文档编辑是整块 `SetMaterial`/`SetText`，没有字段级 patch（undo 粒度/合并需要）。
- egui `Slider` 是 `Sense::drag` 控件，合成 click 不改值 → harness 新增 `double_click`/`drag`；且 `changed()` 只在中间帧出现，full-stack 测试需逐帧 dispatch 或累积 Action（reducer 对等值编辑幂等）。
- toolbar 仍直接发 Action，没有走 core 的 `CommandRegistry`；生产应让 toolbar item id == `CommandId`。

- **窗口 demo（2026-11-09 追加）**：原型新增 `editor-app`（eframe 0.35/wgpu）作为 host 替身，`cd prototypes/editor-ui && cargo run` 打开真实窗口：同一套 core reducer、components、widget kit 在窗口里跑，点击资产/拖 slider/Save/Quit 都走真实 Action → reducer → Effect（SaveDocument 写临时文件、LoadAsset/SpawnPreview 同步 completion、Quit 关窗）；截图见原型 README §2.5。
**未证明**：视觉观感/截图（headless 无像素）、真实 dock/多实例/wgpu 视口/`EffectExecutor`/World 边界、真实磁盘持久化、IME/剪贴板等输入。


## 9. 开放问题（需要作者拍板）

> 前 8 条来自现状审计 [editor-ui-widget-catalog.md](../../research/editor-ui-widget-catalog.md) §9，后 3 条是本设计新增；建议按顺序拍板，前 4 条影响 Phase 0.5 开工。

1. **Widget API 形状**（原型已给结论）：**原子用 `impl egui::Widget` 返回 `egui::Response`**（13 个 atom 全部可 `ui.add(...)`，复用 egui 生态很顺手）；**复合控件用 `show(self) -> Output`**（不要 `&mut self`，原型 13 个 compound 全部无状态）；Output 里可带交互 rect（测试/浮层锚定用）。
2. **PropertyValue 边界**：`PropertyRow` 只接受纯值 enum（f32/bool/String/Color32/[f32;N]/Range），`RenderState → 行数据` 的映射放组件 glue（每编辑器 ~60 行）还是 phase 5 的 `DataUi` 反射注册表？→ 建议先手写 glue，重复三次再上反射。
3. **Theme 归属**：`Theme` 是 ui 层从 `core::StyleState` 派生的只读视图，还是 ui 自己持有？MCP 是否要读 token（要读就必须进 core）？→ 建议：**值进 core `StyleState`，`Theme` 是 `fn(&StyleState) -> Theme` 纯映射**；且 egui 0.35 已内置 `egui::Theme`/`set_style_of`/`widget_style`，实现上优先把 token 映射成 **egui Style + 自定义 `UiTokens`**（host 每帧 `set_style_of` / 存 `ctx.data`），widget 通过 `ui.style()` 的 class/state 取样式，减少自定义传递机制。
4. **108 个旧 style key 的迁移策略**：哪些并入 ~44 tokens、哪些变 per-panel overrides、哪些直接删（已确认 2 个死配置）；是否保留旧 TOML 一个版本周期做导入？
5. **dock 是否纳入 widget kit**：dock 6,762 行（38% UI 代码）已经 widget 化。→ 建议**冻结内部、只换 `TabDrawer` 签名**（去掉 `Drawer/Messager/Engine/Log`，换 `PanelInstanceId`），不重写。
6. **Viewport 域**：scene/game/material-preview 的 size/bind/camera 归 `core::ViewportState` 还是继续留 World？→ 总架构已倾向**留 World**（`EditorCameraController` 唯一真值），`core` 只存「打开/尺寸/绑定状态」；widget 只拿 `TextureId + size`。输入→`ViewportAction` 的转换放组件 glue。
7. **TreeView 拖拽 payload**：widget 无领域类型。→ 建议 `DragPayload { id: DragId, bytes: Vec<u8> }`（弱类型 + 组件侧解码），或泛型 `TreeView<T: Clone>`（强类型但 props 复杂）。需要按项目树（PathBuf）/材质（纹理路径）/未来多文件导入评估。
8. **图标体系**：现状 3 种（file:// URI 图片、Unicode 字形、painter 手绘）。→ 建议统一 `IconSet`（启动加载 → `TextureHandle` 缓存）+ `Icon` 枚举，先不引入图标字体新依赖；CJK fallback 顺序单独定。
9. **（新）Widget 层的 crate 名与目录**：`kairos_editor_widgets` vs `kairos_ui_kit`；L2 用 `components` vs `views`。
10. **（新）Gallery 形态**：dev bin（可手动看）还是仅测试？→ 建议 bin + `#[ignore]` 快照测试预留。
11. **（新）i18n**：现状文案中英混杂且有拼写错误（`"have modify not save"`、`"Falied"`）。widget 文本用 `WidgetText` 已够；组件/面板标题是否统一走翻译表（dock 已有 `translations.rs`）？
12. **（新）headless 交互测试策略**：原型证明「自研 harness（3-pass 点击）+ Output 带 hit rects」可行且快（25 测试 0.04s），但测试与布局 rect 耦合、production Output 要留测试字段。备选：所有交互控件暴露显式 stable `Id`，测试用 `Context::read_response(Id)`（不改 Output API）；或引入 vendor 的 `egui_kittest`（按 label/role 查询 + 快照，最健壮但多一个 dev 依赖）。→ 建议：**先加 stable Id 方案**，production 只保留必要 rect，`egui_kittest` 作为后续可选项。
13. **（新）编辑 buffer 归属**：`TextInput/Slider` 吃 `&mut String/f32`，MVU 下 component 必须每帧把 domain 值 clone 进本地 buffer，再发 `Edit` Action。buffer 放 `ViewState`（UI 侧）还是文档（domain 侧）？→ 建议：**文本类编辑 buffer 放 ViewState，文档只在 Edit Action 时更新**；但需保证输入不丢字（egui TextEdit 有自己的内部状态，配合 stable Id 可实现），material/数值类可即时发 Action。
