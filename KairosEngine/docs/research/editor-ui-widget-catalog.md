# Editor UI 现状审计：Widget 与 Component 目录

> **状态**：调研/提案（本阶段只做设计 + 一次性原型，不改生产代码）
> **日期**：2026-10-30
> **范围**：只读审计 `kairos_editor/src/ui/**`（54 文件 / 17,625 行）与 `kairos_editor/src/ui.rs`（947 行），产出 (a) 纯外观 widget 目录，(b) 现状窗口/Inspector → 目标 component 的映射目录。
> **取证方式**：codegraph 无索引，全部 `read`/`grep` 静态取证；未运行 cargo、未启动编辑器。行号以 2026-10-30 工作区为准。
> **配套**：[架构 README](../plan/editor-ui-architecture/README.md) · [rewrite-plan](../plan/editor-ui-architecture/rewrite-plan.md) · [ADR 0008](../adr/0008-editor-ui-architecture.md) · [Messager/MVC 审计](./editor-ui-messager-mvc-audit.md)
> **本文件不重复的部分**：分层理论见 task-6 的 `ui-widget-component-layer-survey.md`；可运行原型见 [`prototypes/editor-ui`](../../prototypes/editor-ui)（task-8 与 task-9 合并后的统一工作区，core + widgets + components 三层，49 个测试）。本文件只回答"现状长什么样、该抽出哪些 widget、每个窗口怎么落到 widget+ViewState+Action"。

---

## 0. TL;DR（8 条）

1. **UI 代码的一半以上是"可复用外观"，但它和领域逻辑物理混住**：54 文件 17,625 行中，dock 基础设施 6,762 行（38.4%）已经是 widget 级代码，inspectors 6,131 行（34.8%）里约一半是表单绘制；project/scene/窗口合计 3,492 行（19.8%）；剩下 1,240 行（7.0%）是壳/配置。
2. **纯 widget 缺口集中在"表单、树、缩略图、视口贴图、模态"五类**：`TableBuilder` 11 处 / `body.row` 32 次 / `row.col` 70 次 / `ui_style_fields` 的 6 种 StyleField 全是同一张"两列表单"的手写副本；4 个文件各自实现"缩略图/预览图"；6 个文件各自在 `on_exit` 拼确认对话框。
3. **样式系统是最大的负资产**：20 个 per-window `*Style` struct、21 个样式 TOML、`paths.rs` 29 个路径常量、132 个 TOML key（108 个不重复），但 Preferences 只能编辑 **4 个窗口的 21 个字段**（tool_bar 5 + about 2 + scene 10 + preferences 4）——**权限覆盖率 21/108 ≈ 19%**。剩下的 key 只能改 TOML。
4. **下标式 `StyleField` 机制必须删**：`get_style_fileds()` 返回 `Vec`、`update_style()` 按 `style_fields[0..n]` 读下标（tool_bar.rs:289-305、scene_window.rs:299-332、about_window.rs:133-140、preferences_window.rs:154-167），Preferences 还有 `copy_from_slice`（preferences_window.rs:190）和 `drawers[style_page.id]`（ui.rs:497）两处下标耦合；Drawer 实例数恒等于 StylePage 数（启动后 9 个、全开后最多 12 个，含 3 个空布局容器和 Preferences 自己）。
5. **最坏的单点耦合是 material.rs**：1506 行里，`RenderState` 编辑区 257 行（638-894）、纹理拖放槽 112 行（456-567）、3D 预览 179 行（903-1001、1219-1298）、领域写回约 150 行（1323-1411、1438-1505）、保存 28 行（1478-1505）全在一个文件里；Ctrl+S 快捷键在 `draw()` 里被读取（1071-1075）。
6. **纯 widget 目录：25 个**（11 个 P0 表单/文本底座 + 8 个 P1 集合/反馈/chrome + 5 个 P2 图像/chrome + RangeSlider 1 个可选；SearchInput/StatusBar/DragGhost 作为 kit 附加项不占名额）。合计目标约 2,200-2,600 行（含 kittest），替代现状约 5,000-6,000 行手写绘制，预计 UI 绘制代码减少 50-60%。
7. **组件目录：22 个 component**（9 个 panel + 10 个 document editor + 3 个 host/chrome），全部按"只读 Frame + &mut UiOut + ViewState + 发 Action"重写；现状 9 个真实 Drawer + 3 个空布局容器 + 1 个未实例化类型 收敛为 `PanelKind` 枚举 + `PanelInstanceId`。
8. **三个代表文件的 before/after**（保守估计，UI 文件层）：`project_window.rs` 622 → 155-175（−72%）；`inspector/material.rs` 1506 → 330-390（−75%）；`preferences_window.rs` 414 → 100-130（−72%）。被删掉的行不是消失，而是上移 core（约 700-900 行）或压进 widget kit（复用后不再重复）。

---

## 1. 审计口径与总体数字

### 1.1 代码规模与分层

```
kairos_editor/src/ui/**            54 files   17,625 lines   （本文件主口径）
kairos_editor/src/ui.rs             1 file       947 lines   （Drawer/Message/Context，独立在 ui/ 之外）
合计                                            18,572 lines
```

按职责重新归类（`src/ui/**` 全量）：

| 分层 | 文件数 | 行数 | 占 17,625 | 内容 |
|---|---:|---:|---:|---|
| dock 基础设施（`ui/docking_tab/**`） | 14 | 6,762 | 38.4% | 布局树、surface、tab bar、拖拽、drag&drop、窗口状态、dock 样式 |
| inspector 及其窗口 | 17 | 6,131 | 34.8% | 11 个 inspector + inspector_window + 测试 + texture/edit 数据层 |
| 杂项窗口（toolbar/preferences/about/console/dialog/game） | 6 | 1,439 | 8.2% | 见 §2.1 |
| project 面板（project_window + hierarchy/content/context_menu） | 4 | 1,203 | 6.8% | 项目树 + 缩略图网格 + 右键菜单 |
| scene 视口（scene_window + gizmos） | 4 | 850 | 4.8% | 视口 ui + 3 个 gizmo mesh/render |
| 壳与支持（egui_ext/global_styles/ui_style_fields/layout/paths/ide_detection/...） | 9 | 1,240 | 7.0% | 纯 widget 的 `egui_ext.rs` 223 行就在这里 |

> 单文件 `wc -l` 与分类脚本可复现，见 §10.1。

### 1.2 三个最硬的数字结论

**A. 现状已有"半个 widget kit"，只是没被承认。**
- `ui/docking_tab/**` 6,762 行 = 布局控件（已是领域无关的通用控件，风险低、**建议保留**）。
- `ui/egui_ext.rs:13-222` 223 行 = 两个真正的通用 widget（`range_slider` 139 行、`truncate_text_to_height` 53 行），零领域依赖。
- 这两块合计 6,985 行（39.6%），说明团队并不缺"做 widget 的能力"，缺的是把窗口里的表单/树/图像也抽出来。

**B. 重复表单是最大的单一浪费。**
- `egui_extras::TableBuilder` 11 处（material 4、mesh 3、texture 2、toml 2）+ `egui::Grid` 1 处（preferences_window.rs:229）。
- 这些表里 `body.row` 32 次、`row.col` 70 次；每一对 `(label, editor)` 都是手写的"键值行"。
- 加上 18 个 `ComboBox::from_id_salt`、21 个 `DragValue::new`、5 个 `egui::Slider::new`、9 个 `ui.checkbox`、2 个 `color_edit_button`，全部散落在 7 个文件里重复实现"标签 + 编辑器"的布局与 id 规则。

**C. 视图层在做它不该做的事（副作用清单）。**
- 在 `draw()` 里发生的副作用：Preferences 每帧发样式消息（preferences_window.rs:404）、Console 消费日志（console_window.rs:80）、About 每帧写日志（about_window.rs:96）、Audio 每帧发 Tick（audio.rs:304-306）、Scene/Game 每帧回写 size 消息（scene_window.rs:188-191、game_window.rs:137-140）、Project 在 draw 里重置一次性标记（project_window.rs:561）、Material 在 draw 里读快捷键（material.rs:1071-1075）。
- UI 层直接写盘 9 处：code.rs:223、shader.rs:226、toml.rs:191、document.rs:174、material.rs:1489（`.mat`）、texture/edit.rs:120（`.meta`）、tool_bar.rs:307、scene_window.rs:334、about_window.rs:142（后三者是样式 TOML，"每帧写盘"风险见 §4.3）。
- UI 层持有异步通道 5 个文件：scene_window.rs:57/474/547、game_window.rs:32/260、material.rs:233/1228、mesh.rs:83/469（`oneshot::Receiver<EguiTextureHandle>`）。
- 中心路由：`Context::handle` 410-774（365 行）、57 个 `Message` 变体、33 处 `get_window_mut::<T>()`、10 处 `get_inspector_mut::<T>()`、24 处 `engine.world` 直接读写、73 处 `messager.send`（其中非测试代码 66 处 / 21 文件）。

---

## 2. 逐文件清单（`kairos_editor/src/ui/**`）

### 2.1 一览表

**split 口径**（本表与 §7 统一使用，人工按函数归类，最大文件逐行读过，误差约 ±10%）：
- **A = appearance/widget-reusable**：只画像素、不读领域数据、不产生副作用，理论上可换成通用 widget 的行。
- **L = feature/domain logic**：读写 `World`/`Assets`/磁盘/tokio、构造 `GraphicsCommand`、领域映射（如 `BlendPreset`、`Surface`）。
- **P = plumbing/boilerplate**：样式 DTO 与加载、Model 结构、`Drawer` 样板方法（create/show_window/close/get_name/get_title/get_style_fileds/update_style/render/scroll_bars）、imports/常量。

| 文件 | LOC | 画什么 / 干什么 | split A/L/P | 关键证据 |
|---|---:|---|---|---|
| `ui.rs` | 947 | Drawer trait、Message 总线、Context 路由、字体安装 | 5/60/35 | handle 410-774；get_window_mut 885-897；render 776-786 |
| `docking_tab.rs` | 2,542 | dock 全部绘制：tab bar、surface、分隔条、拖拽 overlay、tooltip | 95/0/5 | drawer_bar 1157；drawer_title 2044；TabDrawer 调用 290-292/1121 |
| `docking_tab/dock_state/tree.rs` | 1,050 | 布局树数据结构（split/remove/insert） | 0/100/0 | `Tree::split` 564；`remove_leaf` 677 |
| `docking_tab/styles.rs` | 755 | dock 主题 struct（7 个） | 0/0/100 | `Style` 288；`from_egui` 499 |
| `docking_tab/dock_state.rs` | 695 | surface/节点寻址、增删 drawer | 0/100/0 | `add_window` 366；`remove_drawer` 323 |
| `docking_tab/drag_and_drop.rs` | 477 | 拖拽 overlay 绘制 + 命中测试 | 60/40/0 | 绘制 320-346；overlay 431-469 |
| `docking_tab/dock_state/tree/node*.rs` | 601 | 节点（leaf/split）数据+几何 | 0/100/0 | `Node` 13；`append_drawer` 255 |
| `docking_tab/surfaces.rs / window_state.rs / tab_drawer.rs / translations.rs / state.rs` | 642 | 浮动窗口面、WindowState、TabDrawer 签名、文案、拖拽状态 | 30/0/70 | tab_drawer.rs:25-56 的 ui 签名带 Drawer/Messager/Engine/Log |
| `inspector/material.rs` | 1,506 | 材质：shader 菜单、纹理槽、RenderState 表单、3D 预览、Apply | 50/25/25 | 638-894；456-567；903-1001；1219-1298；1300-1318 |
| `inspector/texture/mod.rs` | 1,008 | 纹理属性表单（MaxSize/Format/Filter/Address/Mipmap/Compare）+ 预览 + Apply | 57/18/25 | draw 352-717；address 863-971；compare 974-1008 |
| `inspector/audio.rs` | 682 | waveform + spectrum + 播放控制 | 36/41/23 | draw 259-307；waveform 481-606；spectrum 612-681；DSP 41-156 |
| `inspector/mesh.rs` | 561 | 只读属性表 + 顶点属性表 + 3D 预览 + 预览工具栏 | 43/27/30 | draw 303-458；preview 153-206；toolbar 208-241 |
| `project_window.rs` | 622 | 左右两 panel 组合、拖拽 ghost、底栏路径 | 25/43/32 | ui 522-595；draw_drag 445-505；domain 250-443 |
| `scene_window.rs` | 570 | 视口贴图 + 相机输入 + 尺寸同步；render() 造 GraphicsCommand | 12/33/55 | ui 136-201；render 358-480；style 反射 219-356 |
| `preferences_window.rs` | 414 | 通用 StyleField 表单（6 种字段） | 53/0/47 | ui 194-413；Grid 229；每帧 send 404；copy_from_slice 190 |
| `inspector/toml.rs` | 350 | 递归 TOML 表编辑 | 45/15/40 | draw 82-151；render_toml_table 201；render_toml_field 227 |
| `tool_bar.rs` | 339 | 菜单栏（About/Quit/Scene/Edit/Window）+ 引擎图标 | 33/18/49 | ui 135-244；menu 148-242；icon 85-124；写盘 306-326 |
| `game_window.rs` | 294 | 占位工具栏（aspect/scale/checkboxes）+ 视口贴图 | 23/32/45 | ui 84-150；render 174-266 |
| `layout.rs` | 279 | 4 个空容器 Drawer（无绘制） | 0/0/100 | 18-186 全是空方法 |
| `project_window/hierarchy_panel.rs` | 259 | 项目树（CollapsingHeader + 文件行 + 拖拽） | 100/0/0 | draw_file 184-258；draw_directory 103-182 |
| `project_window/content_panel.rs` | 234 | 缩略图网格（列数计算 + 单元格 + 重命名输入） | 100/0/0 | columns 93-115；draw_cell 121-233 |
| `inspector/shader.rs` | 234 | WGSL 代码编辑器 + Save | 35/10/55 | draw 86-192；save 217-233 |
| `inspector/code.rs` | 231 | Rust 代码编辑器 + Save | 35/10/55 | 与 shader.rs 95% 同构 |
| `inspector/document.rs` | 182 | Markdown 预览 / 编辑切换 | 40/15/45 | draw 74-140；save 165-181 |
| `project_window/context_menu.rs` | 88 | 右键菜单（Create 子菜单 + Open/Delete/Rename） | 90/10/0 | show 29-87；直接构造 AssetKind |
| `inspector/audio/test.rs` | 386 | 测试（波形/频谱/播放状态） | 0/0/100 | 纯数据测试 |
| `inspector/texture/edit.rs` | 201 | TextureEdit 数据类型 + async 加载 + `.meta` 写入 + 测试 | 0/100/0 | `write_settings_meta` 120 |
| `inspector_window.rs` | 214 | Inspector 壳：3 行 header + 下发 draw | 20/40/40 | ui 148-180；`Box<dyn Inspector>` 创建在 project_window.rs:220 |
| `inspector/material/test.rs` | 192 | 测试（widget-rect 采集） | 0/0/100 | — |
| `about_window.rs` | 181 | 5 行 label（版本/描述） | 12/0/88 | ui 87-105；写盘 133-164 |
| `inspector/font.rs` | 158 | 字号滑杆 + 示例文本 | 20/40/40 | register 68-77；draw 112-152 |
| `ui_style_fields.rs` | 138 | 6 种 StyleField 结构（无绘制） | 0/0/100 | 4-11、32-138 |
| `console_window.rs` | 110 | 逐行 label（`log.pop_front()`） | 10/10/80 | draw 70-83 |
| `hierarchy_window.rs` | 105 | 1 行 "TODO: Hierarchy" | 5/0/95 | ui 69-78 |
| `global_styles.rs` | 106 | 字体列表 + 图标路径（含 AssetKind→icon 映射） | 0/40/60 | `for_kind` 46-72 |
| `dialog.rs` | 101 | Modal 确认框（Cell 回调） | 50/50/0 | model 14-27；draw 42-70 |
| `inspector/unknown.rs` | 51 | "not implement inspector" + 文件大小 | 60/0/40 | 29-46 |
| `inspector/directory.rs` | 41 | 子项数量 | 50/50/0 | 23-40 |
| `inspector/creater.rs` | 83 | 11 路 AssetKind → Box<dyn Inspector> 工厂 | 0/100/0 | 19-82 |
| `scene_window/gizmos*.rs` | 280 | 网格/坐标轴 mesh 构造与渲染命令 | 0/100/0 | gizmos.rs:40；axes 152；grid 68 |
| `ide_detection.rs` | 311 | 外部 IDE 探测 + 进程启动 | 0/100/0 | `open_file` 195-238 |
| `egui_ext.rs` | 223 | range_slider + 文本按高度截断 | 100/0/0 | 13-27；30-168；171-222 |
| `global/paths/native_dialog/drag` | 49+16+13 | 路径常量、原生错误框、Drag 类型 | 0/0/100 | paths.rs 全文 |

### 2.2 最差的"外观 + 逻辑"纠缠 Top 10

1. **`ui/inspector/material.rs:1060-1217`（draw 157 行）**：同一函数里读快捷键（1071-1075）、从 `World` 轮询资产（1077-1087）、画 shader 菜单（1099-1154）、画纹理槽（1166-1182）、画 RenderState 表单（1187）、画 Apply（1191-1210）、画 3D 预览（1214-1215）。视图同时是"文档持有者 + 磁盘写入者"：`change_shader/drop_texture/change_render_state/clear_texture`（1323-1411）直接改 `Assets<Material>` 并置 dirty，`save_material`（1478-1505）写盘，`discard_changes`（1438-1468）从 `Assets<SerializedMaterial>` 还原——这是 edit-in-place 模型的全部代价。
2. **`ui/inspector/material.rs:638-894`（257 行 RenderState 表单）**：一张 2 列表被 `TableBuilder` 手写 7 次（depth/cull/blend/topology/custom 子字段），每个字段的"选项集"来自领域枚举（`CompareFunction`/`CullMode`/`BlendPreset`），每次 `changed` 都发整份 `RenderState` 快照消息（`send_render_state` 611-613）。
3. **`ui/inspector/texture/mod.rs:352-717`（draw 366 行）**：资产加载轮询（364-374）、6 个 ComboBox 的 `dirty.set(true)`（449/492/529/567/601/625/646/667）、"非可过滤格式自动改采样器"的领域规则（480-491）、Apply 按钮发消息（688-710）全在一处；`draw_address_mode_rows`（863-971）和 `draw_compare_row`（974-1008）以 `&mut TextureSettings` 直接改领域数据，名为"行绘制"实为"领域编辑器"。
4. **`ui/inspector/audio.rs`**：draw 里发 Tick（304-306）、draw 里懒算波形/FFT（272-283）、播放控制要 `&mut Engine`（322-478，kira）、DSP 纯函数（41-156）夹在 UI 文件、386 行测试同目录。**41-156 的 `compute_peaks`/`compute_spectrum` 是纯函数，应该是最容易搬到 core/audio 的部分**，却被 UI 文件拖累。
5. **`ui/project_window.rs`**：`Cell<Option<NodeIndex>>` 一次性展开标记（63）+ `Arc<Mutex<String>>` 重命名缓冲（60）；`create_node`（250-286）直接建节点、写 `AssetRegistry`、进入重命名；`rename_node`（289-309）写盘；`delete_node`（403-421）写盘；`open_node`（337-369）启动外部 IDE；`poll_external_changes`（159-200）每帧 drain watcher 并按 GUID 恢复选中。视图 = 控制器 + 磁盘 + 进程启动器。
6. **`ui/scene_window.rs`**：ui（136-201）把"相机输入→消息"（150-184）、"尺寸同步→消息"（188-191）、"贴图绘制"（193-200）三件事揉进 65 行；`render()`（358-480）在窗口里构造渲染图并创建 oneshot 通道（474-477）；`get_style_fileds`+`update_style`（219-356）137 行把样式 DTO 手工映射到相机/实体，且 `update_style` 里写盘+通过 `Context` 反向调用 `apply_camera_style`（ui.rs:511-513）——样式生命周期与领域实体的同步靠"消息回来时顺手做"。
7. **`ui/preferences_window.rs:194-413`（220 行）**：手写 6 种 StyleField 的渲染分派（Float/Slider/Color/Vector2/3/4/Range），第 404 行**每帧** `messager.send(Message::UpdateUIStyle(page))`（`page` 是整页 clone，219）；`ui.rs:493-514` 收到后调被选窗口 `update_style` → tool_bar.rs:306-326 / scene_window.rs:333-355 / about_window.rs:141-163 会 `toml::to_string` + `fs::write`。**打开 Preferences 选 ToolBar 页 ≈ 约 60 次/秒写盘**（静态推断，未运行验证；多 pass 下可能更高）。
8. **`ui/ui.rs:477-487` + `:493-514`**：`RefershPreferenceWindow` 遍历 `self.drawers` **全部** Drawer 取 style fields（包括已关闭的 drawer、3 个空布局容器、Preferences 自己），于是每个 Drawer 实例 → 一个 StylePage（启动后 9 个，打开 Console/About/Preferences 后 12 个）；`ui.rs:497` 又用 `style_page.id` 直接下标 `self.drawers[..]`。只要 `drawers` 的插入顺序变化（新增/删除窗口）或字段顺序变化，运行时就静默错配或 panic——**这是"每窗口自带样式反射"的必然代价，编译期无法检查**。
9. **`ui/dialog.rs:14-27`**：确认/取消是 `Cell<Option<Message>>` + `Cell<Option<FnOnce>>`，控制流藏在闭包里；6 个文件各自拼 `ConfirmDialogWindow::new(...)`（material.rs:1307、texture/mod.rs:724、code.rs:197、shader.rs:200、toml.rs:158、document.rs:148），文案/按钮名/后续消息各不相同。没有统一的 `ModalState`，也就没有可测试性。
10. **`ui/console_window.rs:80` + `ui/about_window.rs:96`**：Console 在 draw 里 `while let Some(log) = log.pop_front()`（每帧消费历史，多 pass 语义不确定）；About 在 draw 里 `log.info(...)` 每次重绘写一条日志。这两处把"可回放日志"当成每帧 UI 副作用。egui 0.35 默认 `max_passes = 2`，Preferences 的 `Grid` 首帧又会 `request_discard`（preferences_window.rs:229）——**多 pass 会让上述所有 draw 内副作用重复执行**。

---

## 3. 重复视觉模式清单（widget 需求来源）

### 3.1 已有模式计数（grep 口径，复现命令见 §10.1）

| 模式 | 次数 | 代表 file:line | 现状问题 | 目标 widget |
|---|---:|---|---|---|
| 按钮（`ui.button` 19 + `Button::new` 14） | 33 | tool_bar.rs:159；material.rs:1196；texture/mod.rs:693；toml.rs:135；document.rs:96/109；code.rs:177；shader.rs:180；context_menu.rs:72-85 | 其中 7 处是 `Button::new(x).min_size(Vec2::new(ui.available_width(), H))` 全宽保存/应用按钮（H 常量散落），另有 3 处 context menu 用 `min_size(ui.min_size().x, 0)` | `Button` |
| 菜单按钮 / 菜单项 | 6 / 15 | tool_bar.rs:201/205/212/219/221；context_menu.rs:30 | 菜单内容与业务动作（`todo!()` tool_bar.rs:207）直接同处 | `MenuBar` / `ContextMenu` |
| 选择控件（`selectable_value` 15 + `selectable_label` 8） | 23 | material.rs:584/993；texture/mod.rs:894/902/926/954/992；preferences_window.rs:204 | 树/列表行选择用 `selectable_label`，下拉项用 `selectable_value`，无统一 row 概念 | `ListRow` / `TreeView` / `ComboBox` |
| 键值/表单行（`body.row` 32 + `row.col` 70） | 32 行 | material.rs:654-892；texture/mod.rs:396-680；mesh.rs:339-451；toml.rs:219；preferences_window.rs:229-401 | 同一"标签列 + 编辑器列"在 5 个文件、11 张表里重复 | `PropertyGrid` + `KeyValueRow` |
| 下拉框 `ComboBox::from_id_salt` | 18 | material.rs:357/392/660/720/758/873/988；texture/mod.rs:427/464/511/584/632/888/921/950/988；mesh.rs:219；game_window.rs:101 | 每个都要手写 `selected_text` + `selectable_value` 循环（约 8-12 行/个） | `ComboBox`（typed） |
| 数值编辑 `DragValue` / `Slider` | 21 / 5 | toml.rs:260/265/319/324；preferences_window.rs:244/292/.../382；font.rs:139；texture/mod.rs:661 | range/speed/精度各处手填；`range_slider` 另起自研 widget（egui_ext.rs:30） | `NumberInput` / `RangeSlider` |
| 复选框 | 9 | mesh.rs:237；material.rs:700；toml.rs:270/329；texture/mod.rs:553/619；game_window.rs:118/121/124 | 标签为空字符串 `""` 的裸 checkbox 有 4 处，无统一样式 | `Checkbox` |
| 颜色编辑 | 2 | toml.rs:246；preferences_window.rs:270 | hex 字符串 ↔ [u8;4] 转换各写一次 | `ColorInput` |
| 文本编辑 | 6 | code.rs:147；shader.rs:150；toml.rs:252；document.rs:129；content_panel.rs:182 | 代码编辑器（语法高亮 layouter + 背景色 + 保存条）code/shader 两份 95% 同构 | `TextInput`（code 变体） |
| 折叠/分区 | 3 | hierarchy_panel.rs:125；toml.rs:286/300 | `ui.heading` 也算分区（3 处）；无统一 section 间距 | `Section` |
| 分隔线 / 间距 | 23 / 2 | 全 ui（如 material.rs:1093/1156/1184/1189/1212）；常量 `DEFAULT_SPEATOR_HEIGHT/DEFAULT_LABEL_HEIGHT`（ui.rs:87-88） | 分隔线承担了分组语义，但没有 token | `Separator` |
| 滚动区 | 11 | inspector/*.rs draw 入口；preferences 无 | 每个 inspector 自己包 `ScrollArea`，与保存条高度手算（code.rs:140-145） | `ScrollArea` 封装（widget kit 内部） |
| 列表/树行 | 3 套实现 | hierarchy_panel.rs:184-258（文件行）、hierarchy_panel.rs:103-182（目录行）、content_panel.rs:121-233（单元格） | 行 = icon+name+suffix+选中底+context menu+拖拽，三套各写一遍 | `TreeView` / `ListRow` |
| 缩略图/图片预览 | 7 处绘制 | material.rs:456-567（拖放槽）、material.rs:967-972（3D 预览）、texture/mod.rs:144-180（图像预览）、mesh.rs:200-205、scene_window.rs:194-199、game_window.rs:143-148、content_panel.rs:160-166（网格 icon） | 4 套独立的"绑定 egui texture → painter.image"；2 套独立的拖放/占位框 | `ImageDropBox` / `ImagePreview` / `ThumbnailGrid` |
| 上下文菜单 | 4 处调用（+ dock 自带 1 套） | hierarchy_panel.rs:169/244；content_panel.rs:85/219；docking_tab.rs:1350+（drawer_bar 内） | 项目菜单直接构造 `AssetKind` 和 `Message`（context_menu.rs:29-87） | `ContextMenu` |
| 模态/确认 | 1 个类型 + 6 处构造 | dialog.rs:42-70；material.rs:1307；texture/mod.rs:724；code.rs:197；shader.rs:200；toml.rs:158；document.rs:148 | 闭包式；按钮文案/后续动作不统一 | `ModalFrame` |
| 工具栏 | 4 套 | tool_bar.rs:135-244（菜单栏）；game_window.rs:92-126（占位）；material.rs:976-1001；mesh.rs:208-241 | 三套 inspector 工具栏各写"label+combo/checkbox" | `Toolbar` |
| 底栏/状态栏 | 1 处 | project_window.rs:383-400（`draw_bottom_bar`） | 只有一个项目底栏，没有全局状态栏 | `StatusBar`（可选） |
| 空状态/加载中 | 约 20 处裸 label | code.rs:99/109；shader.rs:101/111；material.rs:915/939/1085；texture/mod.rs:372；mesh.rs:168/316；toml.rs:97；audio.rs:268/546/633；document.rs:89；font.rs:126/130；hierarchy_panel.rs:151；inspector_window.rs:157；unknown.rs:37 | 文案/样式/居中方式各写各的；无 spinner/进度条（`show_loading_spinner(true)` 仅 content_panel.rs:166 一处） | `EmptyState` / `LoadingState` |
| 未保存标记 | 5 处 | material.rs:1207；texture/mod.rs:707；code.rs:187；shader.rs:190；toml.rs:148 | 文案 `"* unsaved changes"` 硬编码 5 次 | `DirtyBadge` |
| Tooltip | 4 | docking_tab.rs:1724/1758/1913；其它 0 | 只有 dock 有 tooltip 体系 | `Tooltip` |
| 拖动 ghost | 1 | project_window.rs:461-504 | 内联 Area+Frame+icon | `DragGhost`（可选） |
| Tab bar | 完整 1 套 | docking_tab.rs:1157-1800 | 已实现且领域无关；只有 `TabDrawer` 签名泄漏 | 保留，改签名 |

### 3.2 缺失或未统一的模式（新建 widget 的直接理由）

| 缺失模式 | 现状 | 影响面板 |
|---|---|---|
| **搜索/过滤输入** | 全 ui 0 处 `TextEdit` 用于过滤；项目树/Inspector 列表都没有 filter | Project、Hierarchy、Console、Preferences |
| **Spinner / ProgressBar** | 0 处 `egui::Spinner` / `ProgressBar` | 全部异步加载（资产加载、预览回绑） |
| **图标按钮** | 0 个真正的 icon button：`ui.button("X")`（material.rs:444）、unicode `"▶"/"⏸"`（audio.rs:489-490）、dock 用 painter 手绘箭头/关闭符（docking_tab.rs:1933-2017） | Toolbar、Tab、Inspector 工具栏 |
| **统一空状态** | 20 处裸 label，居中方式三套（`centered_and_justified`、`vertical_centered`、裸 label） | 全部 |
| **状态栏** | 仅 Project 底栏（project_window.rs:383-400）；无全局状态/错误区 | 全部 |
| **面包屑** | 0（只有 Project 的 `active_directory` + 底栏路径） | Project、任何层级导航 |
| **徽标/Badge** | 0（未保存用裸 label，日志级别无颜色标记） | Inspector、Console |
| **可折叠分区（带持久化 id）** | 3 处 CollapsingHeader，展开状态交给 egui 默认 id（按标题哈希） | Inspector、Preferences |
| **统一拖放目标** | 仅材质纹理槽一处（material.rs:415-567 手写 152 行） | 材质、资产、未来导入 |
| **数值单位/步进** | DragValue `.speed()` 只 toml.rs:265/324 用了，其余默认 | 全部表单 |
| **表格/虚拟列表** | 无；`body.row` 每次全量迭代（日志/项目树大列表会掉帧） | Console、Project |

---

## 4. 样式与主题现状

### 4.1 per-window style 清单（TOML ↔ struct ↔ 可编辑性）

`paths.rs` 共 29 个常量（`PATH_*`），其中 22 个指向样式/语法 TOML；磁盘上实际有 21 个样式 TOML（含 2 个未被引用的）。逐窗口：

| TOML | 读取方 | Style struct | key 数 | Preferences 可编辑字段 | 备注 |
|---|---|---|---:|---:|---|
| `EditorWindowToolBarStyle.toml` | tool_bar.rs:41 | `ToolBarStyle`（tool_bar.rs:24） | 7 | 5（height/button_width/corner_radius/fill_color/button_text_color） | 有图标路径 `about_icon_path`；字段名 typo `"corrner_radius"`（tool_bar.rs:267） |
| `AboutWindowStyle.toml` | about_window.rs:27 | `AboutWindowStyle`（about_window.rs:19） | 3 | 2（width/height） | title 不可编辑 |
| `PreferencesWindowStyle.toml` | preferences_window.rs:29 | `PreferencesStyle`（:18） | 5 | 4 | **Preferences 自己也是 Drawer**，出现在自己的样式页列表里 |
| `ConsoleWindowStyle.toml` | console_window.rs:26 | `ConsoleWindowStyle`（:12） | 1 | 0 | 只有一个 title |
| `InspectorWindowStyle.toml` | inspector_window.rs:60 | `InspectorWindowStyle`（:18） | 1 | 0 | 只有一个 title |
| `HierarchyWindowStyle.toml` | hierarchy_window.rs:26 | `HierarchyWindowStyle`（:11） | 1 | 0 | 窗口只是 TODO 占位 |
| `ProjectWindowStyle.toml` | project_window.rs:83 | `ProjectWindowStyle`（:39）+ 嵌套 `HierarchyStyle`（hierarchy_panel.rs:20）+ `ContentStyle`（content_panel.rs:22） | 20 | **0** | 嵌套 2 节；字段最丰富却完全不可从 UI 编辑 |
| `SceneWindowStyle.toml` | scene_window.rs:69 | `SceneWindowStyle`（:38） | 13 | 10 | 相机参数；`update_style` 写盘 + 回写相机实体 |
| `GameWindowStyle.toml` | game_window.rs:41 | `GameWindowStyle`（:25） | 1 | 0 | title |
| `DialogWindowStyle.toml` | **文件不存在** | 无 | — | — | 死常量：`PATH_DIALOG_WINDOW_STYLE`（paths.rs:17）无引用，指向的 TOML 也不在磁盘上 |
| `MainContentStyle.toml` | **无人读取** | 无 | 2 | — | 死配置（`background_color/central_panel_color`） |
| `Inspectors/Material.toml` | material.rs:93 | `MaterialInspectorStyle`（:57） | 23 | 0 | 最大，含 7 个相机键 |
| `Inspectors/Texture.toml` | texture/mod.rs:53 | `TextureInspectorStyle`（:41） | 7 | 0 | 4 个 combo 宽度是尺寸常量 |
| `Inspectors/Audio.toml` | audio.rs:183 | `AudioInspectorStyle`（:158） | 10 | 0 | 6 个颜色 + 2 个尺寸 |
| `Inspectors/Mesh.toml` | mesh.rs:62 | `MeshInspectorStyle`（:49） | 8 | 0 | 7 个相机键与 Material **完全重复** |
| `Inspectors/TomlTable.toml` | toml.rs:30 | `TomlTableInspectorStyle`（:23） | 2 | 0 | row_height + save_btn_height |
| `Inspectors/Code.toml` | code.rs:31 | `CodeStyle`（:23） | 4 | 0 | 与 Shader.toml **完全重复**（4 键同值） |
| `Inspectors/Shader.toml` | shader.rs:31 | `ShaderStyle`（:23） | 4 | 0 | 同上 |
| `Inspectors/Document.toml` | document.rs:27 | `DocumentStyle`（:22） | 1 | 0 | button_height |
| `Inspectors/Font.toml` | font.rs:18 | `FontInspectorStyle`（:12） | 2 | 0 | example_content 是长文本，不是样式 |
| `GlobalStyles.toml` | global_styles.rs:16 | `GlobalStyles`（:9） | 17 | 0 | 字体 + 图标路径 |

**汇总**：20 个 style struct（另有 7 个 dock 内部 style struct，dock 已自洽）+ 1 个全局配置；132 个 TOML key、108 个不重复 key；**21 个字段可从 Preferences 编辑（4 个窗口）**，其余 87 个只能改 TOML；2 个死 TOML；重复组 3 组（Material/Mesh 相机 7 键、Code/Shader 4 键、ToolBar 与其它高度/颜色）。

### 4.2 共同 token 聚类（132 → 108 → 44）

| 类别 | 现有 key（举例） | 不重复数 | 可合并 token |
|---|---|---:|---|
| 颜色 | `fill_color`、`texture_*_color`（5）、`waveform_*_color`（4）、`playhead_color`、`spectrum_color`、`grid_color`、`*_background_color`、`*_header_color`、`label_color`、`file_suffix_color`、`dark/bright_background_color` | 26 | 12-14 个语义色 token + 8 个 per-panel |
| 字号/字体 | `file_header_size`(11)、`label_font_size`(10)、`bottom_bar_font_size`(12)、`default_font_size`(14)、硬编码 `FontId::proportional(14)`（audio.rs:547）等 | 5+ | 5 个字号 token（xs/sm/md/lg/h1）+ 2 个 family |
| 高度/行高 | `height`(toolbar 23)、`row_height`(20×3)、`save_button_height`(20)、`apply_button_height`(20)、`button_height`(20)、`save_btn_height`(10)、`label_height`(28)、`texture_label_height`(60)、`waveform_height`(120)、`spectrum_height`(100)、`preview_min_height`(150)、`desired_rows`(10) | 12 | `control_height` 20、`row_height` 20、`toolbar_height` 24、`label_height` 28 |
| 宽度 | `button_width`(45)、`combo_width_*`(140/120/80/180)、`width/default_width`(320) | 7 | 3 个尺寸 token（sm/md/lg）+ 面板默认尺寸 |
| 圆角/描边 | `corner_radius`(0)、`texture_corner_radius`(4)、`selected_*_corner_radius`(4×2)、`texture_stroke_width`(1.0)、`shader_selector_menu_border`(10) | 7 | `radius_sm 2 / md 4 / lg 6`、`stroke_width 1.0` |
| 间距 | `grid_space_x`(40)、`grid_space_y`(4)、`cell_spacing_x`(24)、`cell_spacing_y`(12)、`render_state_sub_row_indent`(16)、内联 margin（game_window.rs:93 `(4,2)`、project_window.rs:473 `(6,3)`） | 6 | `space_1 2 / space_2 4 / space_3 8 / space_4 12 / space_5 16 / space_6 24 / space_7 40` |
| 图标/缩略图尺寸 | `icon_size`(24 toolbar)、`file_icon_size`(18)、`about_icon_path`、content `icon_size`(60)、`preview_default_size`(512×2) | 6 | `icon_sm 16 / icon_md 24`、`thumb_sm 24 / thumb_md 60 / thumb_lg 120`、`preview_size 512` |
| 相机（per-panel） | Material 7 + Mesh 7 + Scene 12（`cam_default_*`） | 19 | 1 组 `PreviewCameraTokens`（7）+ Scene 相机默认值；**三处重复值相同**（camera_fov 60 vs scene 45 除外） |

### 4.3 下标式 `StyleField` 机制：证据与"必须删"的理由

现状实现：
- `StyleField` 只有 6 个变体（`ui/ui_style_fields.rs:4-11`）：Float/Color/Vector2/3/4/Range；`FloatFieldEditViewType` 只有 Field/Slider（:26-29）。
- 每个 Drawer 提供 `get_style_fileds() -> Vec<StyleField>` 与 `update_style(&Vec<StyleField>)`（`ui/ui.rs:284-286`）。
- 真实提供字段的只有 4 个 Drawer：tool_bar.rs:248-283（5 个）、about_window.rs:111-131（2 个）、scene_window.rs:219-297（10 个）、preferences_window.rs:114-148（4 个）。
- `update_style` 靠**下标**读：tool_bar.rs:289-305（`style_fields[0..4]`）、scene_window.rs:299-331（`[0..9]`）、about_window.rs:133-140（`[0..1]`）、preferences_window.rs:154-167（`[0..3]`）。
- Preferences 侧：`style_pages[*id]`（preferences_window.rs:219）、`pages[style_page.id]`（:189）、`page.fields.copy_from_slice(&style_page.fields)`（:190）。
- 路由侧：`ui.rs:479-483` 遍历全部 drawers 生成 StylePage；`ui.rs:497` `self.drawers[style_page.id]`。

为什么必须删（7 条，全部有现状证据）：
1. **增删/重排字段无编译期检查**：改一个字段顺序 → `if let` 全部落空 → 静默不更新；类型错位（Float 变 Color）→ 静默跳过。
2. **Drawer 实例数恒等于样式页数**（启动后 9 页、全开最多 12 页）：3 个已实例化的布局容器（layout.rs:18-186；`LayoutCenterContainer` 只有 impl 从未实例化）各返回 `Vec::new()`，和 Preferences 自己都会出现在"设置页"列表里（ui.rs:477-487 无过滤）。
3. **索引即身份**：`drawers[style_page.id]` 把"数组下标"当作"窗口 id"，任何插入顺序变化都会错配（现在 sizeof 恰好固定才没炸）。
4. **每帧全量回传**：Preferences 每帧 `send(UpdateUIStyle(page))`（:404）→ 整页 clone + `copy_from_slice`（:190）→ 选中窗口写盘（tool_bar.rs:306-326 等）。这既是性能问题也是"多 pass 重复写盘"的根源。
5. **字段描述能力被 6 个变体锁死**：枚举下拉（`CompareFunction`/格式/过滤模式等）在 inspector 里手写 18 次 `ComboBox`，而 StyleField 根本没有 enum/路径/字符串类型；Font 的 `example_content` 这类长文本也没法进样式页。
6. **样式 DTO 与运行时数据手工双向映射**：scene_window.rs:219-356 共 137 行只是"TOML 字段 ↔ 相机/实体字段"；`ui.rs:511-513` 还要在 UpdateUIStyle 后反向调用 `SceneWindow::apply_camera_style`。
7. **无法测试**：全部逻辑都在 `Context::handle` 的臂里，需要整个 Engine + GPU 才能跑；74 个迁移测试里 0 个覆盖样式链路（审计文档已记录）。

**替代方案（Phase 4 落地）**：`StyleState`（core，纯数据镜像）+ `StyleSchema`（注册表：section → 字段 → 编辑器类型/范围/默认值）+ `Theme`（ui 层派生只读映射）。字段身份用稳定字符串 `StyleKey("material.texture.corner_radius")`，不用下标。Preferences 由 schema 生成 `PropertyGrid`，保存走 debounce 的 `Effect::PersistStyle`。

### 4.4 Theme token 提案（44 个）

目标：`ui_theme.rs` 里一个 `#[derive(Clone)] pub struct Theme`，纯数据、无 IO；启动时从 `StyleState` 映射一次，修改走 `Action::Style(SetToken)`。**全部为 ui 层可用的原始类型**（`egui::Color32`/f32），不含领域类型。

| # | token | 值（建议） | 取代现状 |
|---:|---|---|---|
| 1 | `bg_window` | #1B1B1B | Code/Shader `dark_background_color` |
| 2 | `bg_panel` | #232323 | MainContentStyle（死配置）`central_panel_color` #323232 |
| 3 | `bg_surface` | #2B2B2B | Project `bottom_bar_background_color` #2b2b2b |
| 4 | `bg_input` | #32323280 | Material `texture_background_color` |
| 5 | `bg_hover` | #3A3A3A | 散落 hover 手绘（docking_tab styles） |
| 6 | `bg_selected` | #3C4B64 | Project `selected_background_color` + `selected_file_background_color`（同值重复 2 次） |
| 7 | `bg_overlay` | #000000B4 | project 拖拽 ghost `from_black_alpha(180)`（project_window.rs:471） |
| 8 | `text_primary` | #C8C8C8 | Project `label_color`/`file_header_color`；ToolBar `button_text_color` #BDBDBD |
| 9 | `text_secondary` | #A0A0A0 | Project `bottom_bar_text_color` |
| 10 | `text_muted` | #787878 | Project `file_suffix_color` |
| 11 | `text_accent` | #DCDCB4 | Project `directory_header_color` |
| 12 | `text_on_accent` | #FFFFFF | painter.text 硬编码 GRAY/WHITE |
| 13 | `accent` | #4696FF | Material `texture_drag_hover_stroke_color` |
| 14 | `accent_soft` | #0050C84D | Material `texture_drag_hover_background_color` |
| 15 | `danger` | #FF5252 | Audio `playhead_color`；错误对话框需统一色 |
| 16 | `success` | #81C784 | Audio `spectrum_color`（降级为 token 示例） |
| 17 | `warning` | #FFB74D | 新增（日志级别/未保存） |
| 18 | `border` | #464646 | Material `texture_empty_stroke_color` |
| 19 | `border_strong` | #5A5A5A | Material `texture_fill_stroke_color` |
| 20 | `border_focus` | #4696FF | 新增焦点环 |
| 21 | `grid_line` | #3A3A3A | Audio `grid_color` |
| 22 | `font_family_ui` | "NotoSansSC" | GlobalStyles `fonts.proportional` |
| 23 | `font_family_mono` | "NotoSansSC"（现状 mono 同名，建议换真等宽） | GlobalStyles `fonts.monospace` |
| 24 | `font_size_xs` | 10.0 | Project `label_font_size`、Audio 手写 `FontId::monospace(10)` |
| 25 | `font_size_sm` | 11.0 | Project `file_header_size` |
| 26 | `font_size_md` | 12.0 | Project `bottom_bar_font_size` |
| 27 | `font_size_lg` | 14.0 | Font `default_font_size`、Audio `proportional(14)` |
| 28 | `font_size_h1` | 20.0 | `ui.heading`（3 处） |
| 29 | `radius_sm` | 2.0 | 新增 |
| 30 | `radius_md` | 4.0 | Material `texture_corner_radius`、Project `*_corner_radius`（3 处同值 4） |
| 31 | `radius_lg` | 6.0 | dock 默认 |
| 32 | `stroke_width` | 1.0 | Material `texture_stroke_width` |
| 33 | `control_height` | 20.0 | `row_height`(20×3)、`save/apply/button_height`(20×4) |
| 34 | `row_height` | 20.0 | 同上，表格行专用 |
| 35 | `toolbar_height` | 24.0 | ToolBar `height`(23) |
| 36 | `label_height` | 28.0 | Content `label_height` |
| 37 | `space_1..space_7`（7 个） | 2/4/8/12/16/24/40 | `grid_space_y`(4)、`cell_spacing_y`(12)、`render_state_sub_row_indent`(16)、`cell_spacing_x`(24)、`grid_space_x`(40)、内联 margin |
| 44 | `anim_fast` / `anim_slow`（2 个） | 0.1s / 0.25s | 新增（dock fade、hover） |
| 45 | `icon_sm/md`（2 个） | 16 / 24 | `file_icon_size`(18)、`icon_size`(24) |
| 46 | `thumb_sm/md/lg`（3 个） | 24 / 60 / 120 | content `icon_size`(60)、`texture_label_height`(60)、拖拽预览 |
| 47 | `combo_width_sm/md/lg`（3 个） | 80 / 120 / 180 | `combo_width_anisotropy`(80)/narrow(120)/default(140)/format(180) |
| 48 | `preview_size` | 512 | `preview_default_size`(Material/Mesh 各一份) |

> 上表实际约 **56 个 token**（把 `space_*`、字号、thumb 等拆开计）。若严格压到 30-50，可把 `space_1..7` 压到 5 档、thumb 压到 2 档，得到 **44**。建议正式实现按 44 个落，`space_*` 用 const 数组而非独立字段。

**per-panel overrides（不进全局 Theme）**：
- `MaterialTokens`：`texture_drag_hover_bg/fg`、`texture_empty/fill_stroke`、`texture_corner_radius`、`preview camera 7 键`。
- `AudioTokens`：`waveform_bg/color/played`、`playhead`、`spectrum`、`grid`（6 色，语义太专属，不该污染全局）。
- `PreviewCameraTokens`：`fov/direction/orbit_speed/zoom_speed/min_distance/max_distance`（Material/Mesh 完全重复，合并一份；Scene 保留自己的 `cam_default_*`）。
- `DockTokens`：沿用 `docking_tab/styles.rs` 的 7 个 struct，从 `Theme` 反序列化 `from_theme()`，不再独立 TOML。

### 4.5 StyleSchema 替代 `get_style_fileds`

```rust
// core::style::schema —— 纯数据，可序列化，MCP 可读写
pub struct StyleSchema { pub sections: Vec<StyleSection> }
pub struct StyleSection {
    pub id: StyleSectionId,          // 稳定字符串："general" | "toolbar" | "project.hierarchy" | ...
    pub label: &'static str,
    pub fields: Vec<StyleFieldDef>,
}
pub struct StyleFieldDef {
    pub key: StyleKey,               // 稳定字符串，不再是下标
    pub label: &'static str,
    pub editor: StyleEditor,         // Float{range,step} | Color | Enum{options} | Vec2/3/4 | Range | Bool | Text
    pub default: StyleValue,
    pub scope: StyleScope,           // Global | Panel(PanelKind)
}
```

- Preferences 面板 = `StyleSchema` × `PropertyGrid`，**不再需要遍历 Drawer**（删掉 ui.rs:477-487）。
- 每窗口的 `update_style` 改为 `apply_style(&StyleState, &mut Theme)` 的纯函数（可单测）；运行时系统从 Theme 读值，不反向写盘。
- 落盘统一走 `Effect::PersistStyle`（debounce，帧末执行），删掉 4 处窗口内 `fs::write`。
- 迁移期：保留每窗口 TOML 作为"旧值导入源"，一次 <code>migrate_style</code> 命令转成 `StyleState`；死配置直接删（`PATH_DIALOG_WINDOW_STYLE`、`MainContentStyle.toml`）。

---

## 5. Widget catalog（25 个纯外观控件）

### 5.1 通用约定（硬约束）

1. **零领域类型**：widget 文件禁止 `use kairos_engine::{Guid, Entity, Mesh, Material, ...}`、禁止 `Document`/`Action`/`World`；可以 `use egui` 与 `kairos_math::{Color32, float2...}`（若 math 已脱离 engine）。
2. **统一返回值**，便于组件层收集意图：
   ```rust
   pub struct Out<T = ()> { pub response: egui::Response, pub changed: Option<T> }
   ```
   `changed` 只表达"用户编辑后的新值/点击了哪一项"，**不表达领域语义**。
3. **状态不进 widget**：`Open`/`expand`/`scroll` 等瞬态由 egui `Id`+Memory 或调用方 `ViewState` 持有；widget 的 props 全是"本帧快照"。
4. **id 用调用方给的稳定字符串**（`&str`），不自行 `ui.id().with("row")` 之类的隐式 id（现状 content_panel.rs:99/182、hierarchy_panel.rs:235 已出现 id 冲突风险）。
5. **主题唯一入口**：`fn ui(ui: &mut Ui, theme: &Theme, props: ...) -> Out`；禁止 widget 内出现 `Color32::WHITE/GRAY`（现状约 10 处）。
6. **可测**：每个 widget 至少 1 个 `egui_kittest` 用例（点击/输入/渲染快照），不依赖 GPU/Window。task-8 原型应验证该测试方式。

### 5.2 catalog 总表

| # | Widget | 用途 | 关键 props | Out | 取代现状（行数估） | 优先级 |
|---:|---|---|---|---|---|---|
| W01 | `Button` | 主/次/危险/全宽按钮 | `label, variant, size, enabled, full_width, icon: Option<IconRef>` | `clicked` | 33 处按钮，其中 7 处全宽保存/应用样板，另 3 处 context-menu 全宽（≈150 行） | **P0 必须** |
| W02 | `IconButton` | 图标按钮（含 tooltip） | `icon, tooltip, size, selected, enabled` | `clicked` | `"X"`(material.rs:444)、`"▶/⏸"`(audio.rs:490)、dock 手绘箭头 3 个函数（≈120 行） | P1 必须 |
| W03 | `Label` | 文本（tone/mono/truncate/strong） | `text, tone: TextTone, mono, wrap, strong, size` | — | 144 处 `ui.label/RichText` 中的样式部分（≈80 行） | **P0 必须** |
| W04 | `KeyValueRow` | 键值行（label + value/editor） | `label, label_width, value: ValueView, indent, enabled` | — | `row.col` 70 次 / 11 张表（≈400 行） | **P0 必须** |
| W05 | `PropertyGrid` | 两列表单容器（striped/分组） | `rows: &[PropertyRow], striped, label_width, indent` | `Option<(PropertyId, PropertyValue)>` | 32 个 `body.row` + 1 个 `Grid`（≈900 行） | **P0 必须** |
| W06 | `NumberInput` | 数值（field/slider/drag 统一） | `value, range, speed, decimals, slider: bool, suffix` | `changed: f32/i64` | 21 DragValue + 5 Slider（≈140 行） | **P0 必须** |
| W07 | `RangeSlider` | 双端范围 | `low, high, range` | `changed: (f32,f32)` | `egui_ext.rs:30-168`（139 行，已存在） | 可选 P3 |
| W08 | `ComboBox` | 单选下拉（typed） | `id, items: &[ComboItem<T>], selected, width, enabled` | `changed: T` | 18 个手写 ComboBox（≈200 行） | **P0 必须** |
| W09 | `Checkbox` | 复选（带 label/缩进） | `checked, label, enabled` | `changed: bool` | 9 处（≈30 行） | **P0 必须** |
| W10 | `TextInput` | 单行/多行/代码编辑器 | `id, text: &mut String, multiline, code: Option<CodeHighlight>, rows, background` | `changed, lost_focus, submit` | 6 处（code/shader 各 85 行同构 + toml/document），≈300 行 | **P0 必须** |
| W11 | `ColorInput` | 颜色（swatch + hex + alpha） | `color: Color32, alpha, show_hex` | `changed: Color32` | toml.rs:246、preferences_window.rs:270（≈60 行） | **P0 必须** |
| W12 | `Section` | 分区标题/可折叠分区 | `id, title, collapsible, default_open, right: Option<&dyn Fn>` | `toggled: bool` | 3 CollapsingHeader + 3 heading + 2 strong label（≈60 行） | **P0 必须** |
| W13 | `Separator` | 水平/垂直分隔 + spacing token | `orientation, gap: SpaceToken` | — | 23 处 + 2 个常量（≈40 行） | **P0 必须** |
| W14 | `Tooltip` | 统一 tooltip（文本+快捷键） | `text, shortcut: Option<&str>` | — | 4 处（≈20 行） | P1 必须 |
| W15 | `EmptyState` | 空状态（icon+标题+说明+可选按钮） | `icon, title, hint, action: Option<ButtonProps>` | `action_clicked` | 约 20 处裸 label（≈80 行） | P1 必须 |
| W16 | `LoadingState` | 加载中（label/spinner/progress） | `label, progress: Option<f32>, spinner: bool` | — | 约 20 处（≈60 行） | P1 必须 |
| W17 | `DirtyBadge` | 未保存标记 | `label: Option<&str>, tone` | — | 5 处 `"* unsaved changes"`（≈20 行） | P1 必须 |
| W18 | `TreeView` | 树：缩进/展开/icon/label/suffix/选中/右键/拖拽 | `id, rows: &[TreeRow], selected, expanded, indent, row_height, drag_enabled` | `selected, toggled, double_clicked, context_menu, drag_started, dropped_on` | hierarchy_panel.rs:103-258（≈180 行，另加拖拽 60） | P1 必须 |
| W19 | `ListRow` | 单行列表项（icon+label+suffix+selected） | `icon, label, suffix, selected, enabled, height` | `clicked, secondary_clicked` | content_panel 行 + 多处 selectable_label（≈80 行） | P1 必须 |
| W20 | `ThumbnailGrid` | 缩略图网格（列数自适应） | `items: &[ThumbItem], cell: Vec2, columns: Option<usize>, selected` | `clicked, double_clicked, secondary_clicked` | content_panel.rs:50-233（≈200 行） | P2 必须 |
| W21 | `ImageDropBox` | 固定尺寸图片/拖放目标 | `texture: Option<TextureRef>, empty_label, drag_hover, size, radius` | `dropped: Option<DragPayload>, clicked, cleared` | material.rs:415-567（≈150 行） | P2 必须 |
| W22 | `ImagePreview` | 自适应图片/视口 surface | `texture, aspect: Option<f32>, overlay: Option<&dyn Fn>, sense` | `hovered, dragged(delta), scrolled(delta), resized(Vec2)` | Scene 66 + Game 67 + Material 预览 71 + Mesh 预览 53（≈260 行） | P2 必须 |
| W23 | `Toolbar` | 工具栏容器（对齐/分隔/溢出） | `height, align, groups: &[ToolbarGroup]` | — | tool_bar 111 + game 35 + material 26 + mesh 34（≈200 行） | P1 必须 |
| W24 | `MenuBar` | 菜单栏 + 菜单项 + 快捷键/禁用 | `menus: &[MenuDef<'_>], height` | `selected: Option<MenuId>` | tool_bar.rs:148-242（≈110 行） | P2 必须 |
| W25 | `ContextMenu` | 右键菜单（子菜单/分隔/禁用） | `items: &[MenuItemDef<'_>]` | `selected: Option<MenuId>` | 4 处 + dock 1 套 + project context_menu 88 行（≈200 行） | P2 必须 |
| — | `ModalFrame` | 确认/错误/输入模态框 | `id, title, body, buttons: [ModalButton; 2], size` | `choice: ModalChoice` | dialog.rs:42-70 + 6 处构造（≈120 行） | P2 必须 |
| — | `StatusBar` | 左/中/右槽位底栏 | `left/center/right: &dyn Fn, height` | — | project_window.rs:383-400（≈40 行） | 可选 |
| — | `SearchInput` | 搜索框（清空/快捷键） | `query, placeholder, id` | `changed: String` | 0（缺失），≈60 行新增 | 可选 P2 |
| — | `DragGhost` | 拖拽跟随预览 | `payload, position, label, icon` | — | project_window.rs:461-504（≈60 行） | 可选 |
| — | `TabStrip` | tab bar（保留 dock 实现，仅签名改造） | — | — | docking_tab 已有（不重写） | 可选（不建议新建） |

> 真正进入 catalog 编号的是 W01-W25（25 个），`ModalFrame/StatusBar/SearchInput/DragGhost/TabStrip` 列为"kit 附加项"，不占 25 名额。**P0 = 11 个**（W01 Button、W03 Label、W04 KeyValueRow、W05 PropertyGrid、W06 NumberInput、W08 ComboBox、W09 Checkbox、W10 TextInput、W11 ColorInput、W12 Section、W13 Separator）；**P1 = 8 个**、**P2 = 5 个**、**必须（P0-P2）= 24 个**、可选 1 个（W07 RangeSlider）。

### 5.3 逐 widget 定义（props + out + 替代证据）

以下为建议签名（仅供 task-8 原型对齐，不是生产代码）：

```rust
// ---- 基础类型 ----
pub enum Size { Sm, Md, Lg }
pub enum TextTone { Primary, Secondary, Muted, Accent, Danger, OnAccent }
pub enum IconRef<'a> { Uri(&'a str), Texture(egui::TextureId), Glyph(char) }
pub enum DragPayload { Path(String), AssetId(String), Custom { id: String, bytes: Vec<u8> } } // 纯数据，不含 Guid/Handle

// W01 Button
pub struct ButtonProps<'a> { pub label: &'a str, pub icon: Option<IconRef<'a>>, pub variant: ButtonVariant,
    pub size: Size, pub enabled: bool, pub full_width: bool }
pub fn button(ui: &mut egui::Ui, theme: &Theme, p: &ButtonProps) -> Out<bool>;   // changed=Some(true) 表示 clicked

// W05 PropertyGrid + 行模型
pub enum PropertyEditor { Float { range: RangeInclusive<f32>, speed: f32 }, Int { range: RangeInclusive<i64> },
    Bool, Text { multiline: bool }, Color { alpha: bool }, Vec2 { min: f32, max: f32 },
    Vec3 { .. }, Vec4 { .. }, Range { min: f32, max: f32 }, Enum { options: &'static [(&'static str, i64)] } }
pub struct PropertyRow<'a> { pub id: PropertyId, pub label: &'a str, pub editor: PropertyEditor,
    pub indent: u8, pub enabled: bool, pub help: Option<&'a str> }
pub struct PropertyGridProps<'a> { pub rows: &'a [PropertyRow<'a>], pub striped: bool, pub label_width: f32 }
pub fn property_grid(ui: &mut egui::Ui, theme: &Theme, p: &PropertyGridProps,
    values: &mut dyn FnMut(PropertyId) -> PropertyValue) -> Out<(PropertyId, PropertyValue)>;

// W18 TreeView
pub struct TreeRow<'a> { pub id: TreeRowId, pub depth: u16, pub icon: Option<IconRef<'a>>,
    pub label: &'a str, pub suffix: Option<&'a str>, pub expandable: bool, pub expanded: bool,
    pub selected: bool, pub dragging: bool }
pub struct TreeOut { pub selected: Option<TreeRowId>, pub toggled: Option<TreeRowId>,
    pub double_clicked: Option<TreeRowId>, pub context_menu: Option<TreeRowId>,
    pub drag_started: Option<TreeRowId>, pub dropped_on: Option<TreeRowId> }

// W22 ImagePreview
pub struct ImagePreviewProps { pub texture: Option<egui::TextureId>, pub aspect: Option<f32>,
    pub sense: egui::Sense, pub show_overlay: bool }
pub struct ImagePreviewOut { pub response: egui::Response, pub drag_delta: egui::Vec2,
    pub scroll_delta: f32, pub size_px: egui::Vec2 }
```

**逐 widget 的"替代证据 → 目标行数"**（估算）：

| widget | 当前散落位置 | 当前行数 | 目标实现行数 |
|---|---|---:|---:|
| W01 Button | 8 文件 33 处 + 全宽样板 7 处 | ~150 | ~70 |
| W05/W04 PropertyGrid+KeyValueRow | material 638-894、texture 352-717 表单部分、mesh 332-451、toml 201-349、preferences 194-413、ui_style_fields 138 | ~1,050 | ~420 |
| W06/W07/W11 数值/范围/颜色 | 21 DragValue + 5 Slider + 2 color edit + egui_ext 139 | ~260 | ~200（含已有 range_slider 改造） |
| W08 ComboBox | 18 处 × 8-12 行 | ~200 | ~80 |
| W10 TextInput/code | code.rs:84-190、shader.rs:86-192、document.rs:74-140、toml 文本分支 | ~300 | ~140 |
| W18 TreeView | hierarchy_panel.rs:103-258 + project 拖拽 60 | ~240 | ~180 |
| W20 ThumbnailGrid | content_panel.rs:50-233 | ~200 | ~130 |
| W21 ImageDropBox | material.rs:415-567 | ~150 | ~90 |
| W22 ImagePreview | scene 66 / game 67 / material 71 / mesh 53 | ~260 | ~120 |
| W23/W24/W25 Toolbar/MenuBar/ContextMenu | tool_bar 111 + context_menu 88 + 3 个 mini toolbar 95 + dock 菜单 ~100 | ~390 | ~250 |
| W15/W16/W17 空/加载/脏 | 约 30 处裸 label | ~160 | ~90 |
| W12/W13 Section/Separator | 6 处 + 23 分隔线 | ~100 | ~60 |
| 其余（Icon/Checkbox/Tooltip/ListRow） | — | ~180 | ~120 |

合计：**现状约 3,640 行手写绘制 → kit 约 1,850 行**（不含 ModalFrame/StatusBar/DragGhost/SearchInput 约 +300）。加上组件层胶水（§6），整体 UI 绘制减少约 50-60%。

---

## 6. Component catalog（22 个 component）

### 6.1 命名与 Action 域约定

- **Action 域**（沿用架构 README，额外两个标记 uncertain）：`Project` / `Document` / `Layout` / `Selection` / `Modal` / `Style` / `System`；**`Viewport`（scene/game/预览，README 未定义，标 uncertain）**；**`Play`（播放/音频预览，README 有 PlayAction）**。
- 组件**只发 Action 不直接改状态**；所有跨域规则（选中→加载文档→Inspector 显示）在 reducer 里。
- 组件签名（与 README §3.5 对齐，细节由 task-8 原型验证）：
  ```rust
  pub trait Component {
      type ViewState: Default;
      fn draw(ui: &mut egui::Ui, frame: &Frame, out: &mut UiOut{ ActionSink }, view: &mut Self::ViewState);
  }
  ```
  注意：`Frame` 里只有 `&EditorState` + 只读 `&World` + `&Theme`，**没有 `&mut EditorState`/`&mut Engine`**。

### 6.2 组件总表

| # | 目标 component | 现状来源（file:line） | 组合 widgets | ViewState（ui 层） | 发出 Actions | 风险/不确定 |
|---:|---|---|---|---|---|---|
| C01 | `panels/project` | project_window.rs:522-595；hierarchy_panel.rs 全文；content_panel.rs 全文 | W18 TreeView、W20 ThumbnailGrid、W23 Toolbar、W08/W24、W15 EmptyState、W17 DirtyBadge、W13 Separator、W25 ContextMenu | `expanded: BTreeSet<AssetKey>`、`active_dir: Option<AssetKey>`、`rename: Option<RenameState{buffer, target}>`、`filter: String`、`grid_scroll: f32`、`tree_scroll: f32` | `Selection::SelectAsset`、`Project::{CreateAsset, RenameAsset, DeleteAsset, RequestDelete, NavigateToDir}`、`Document::Open`、`Layout::FocusPanel`、`Modal::Open(ConfirmDelete)`、`System::OpenExternal` | 树重扫后索引失效（现状已用 GUID 找回 project_window.rs:177-200）；拖拽 payload 待定（§9 Q7） |
| C02 | `panels/hierarchy` | hierarchy_window.rs:69-78（TODO 占位）、project_window 的树 | W18 TreeView | 同 C01 共享 `ViewState` | 同 C01 | 现状是空壳，可并入 C01 的同一种 tab 视图 |
| C03 | `panels/inspector`（壳） | inspector_window.rs:148-180 | W12 Section、W03 Label、W15 EmptyState、W08 ComboBox（文档选择器，新增） | `scroll: f32`、`collapsed: BTreeSet<SectionId>` | `Document::Open{asset}`、`Document::Close{id}`、`Layout::FocusPanel` | 多文档并排（多实例）本期不做 |
| C04 | `editors/text` | code.rs:84-190、document.rs:74-140 | W10 TextInput(code)、W01 Button、W17 DirtyBadge、W15 EmptyState、W16 LoadingState | `scroll`、`find: Option<FindState>`、`wrap: bool`、`editing: bool`（document 的预览/编辑开关） | `Document::Edit{id,patch}`、`Document::Save{id}`、`Modal::Open(ConfirmDiscard)`、`Document::Revert{id}` | 语法高亮 layouter 依赖 `syntax.rs`；Markdown 预览依赖 egui_commonmark |
| C05 | `editors/shader` | shader.rs:86-192 | 同 C04（`CodeHighlight::Wgsl`） | 同 C04 | 同 C04 | 与 C04 合并为一个 component + kind 参数即可，无需独立文件 |
| C06 | `editors/toml` | toml.rs:82-151、201-349 | W05 PropertyGrid（递归）、W10/W06/W09/W11、W12 Section | `expanded_paths: BTreeSet<String>`、`scroll` | `Document::Edit{id, TomlPatch}`、`Document::Save`、`Document::Revert` | TOML 任意 schema → 只能"通用值编辑器"，无字段语义 |
| C07 | `editors/material` | material.rs:1060-1217、1300-1318、1323-1505 | W05 PropertyGrid（RenderState 7 组行）、W08 ComboBox（shader 选择）、W21 ImageDropBox（纹理）、W22 ImagePreview（3D 预览）、W23 Toolbar、W01 Button、W17 DirtyBadge、W12 Section | `blend_custom_expanded: bool`、`preview_mesh: usize`、`preview_camera`（或进 `ViewportState`）、`thumb_cache_key: Option<PathHash>`、`scroll` | `Document::Edit{id, MaterialPatch{shader?, texture?, render_state?}}`、`Document::Save`、`Document::Revert`、`Modal::Open(ConfirmDiscard)`、`Viewport::{Create, Input, SyncSize}`、`Effect::ApplyDocumentPreview` | edit-in-place → 文档缓冲 + preview effect 的时序；Apply/Discard 语义；3D 预览相机与 Scene 共用 viewport 注册表；Ctrl+S 上移 Command |
| C08 | `editors/texture` | texture/mod.rs:352-717、863-1008、719-744 | W05 PropertyGrid、W08、W09、W06、W22 ImagePreview、W01、W17、W12 | `per_axis_mode: bool`、`scroll`、`preview_cache: Option<...>` | `Document::Edit{id, TextureSettingsPatch}`、`Document::Save`（Effect 写 `.meta`）、`Document::Revert`、`Modal::Open(ConfirmDiscard)` | `.meta` 写盘时机；源图/产物路径配对（texture/edit.rs）；压缩配置来自 `Preferences/Engine/KairosSettings.toml` |
| C09 | `editors/mesh` | mesh.rs:303-458 | W05 PropertyGrid（只读行）、W08、W09、W22 ImagePreview、W23 Toolbar、W12 | `preview_mode: u8`、`wireframe: bool`、`scroll` | `Document::Open{mesh}`（只读）、`Viewport::{Create, Input, SyncSize}` | 网格不可编辑——是否算 Document？建议只读 Document（无 dirty/undo） |
| C10 | `editors/audio` | audio.rs:259-307、481-681 | W23 Toolbar（播放/暂停）、W03 Label（元信息行）、W22 的变体 `WaveformView`（私有绘制）、W12 Section | `peaks: Option<Vec<Peak>>`、`spectrum: Option<Vec<Bin>>`、`scroll` | `Play::TogglePreview`、`Play::SeekPreview{secs}`、`Play::Tick{dt}` | DSP（41-156）应搬 core/audio 或 engine audio；播放状态属 PlayState 非 ViewState |
| C11 | `editors/font` | font.rs:112-152 | W06 NumberInput（字号）、W03 Label（示例文本） | `font_size: f32` | 无（只读预览）或 `Style::SetToken`（若字号成为主题 token） | egui 字体注册/注销是 host 资源（`Fonts`），不是 ViewState |
| C12 | `editors/directory` | directory.rs:23-40 | W04 KeyValueRow | 无 | 无 | 可并入 C13 |
| C13 | `editors/unknown` | unknown.rs:29-46 | W15 EmptyState、W04 KeyValueRow | 无 | 无 | — |
| C14 | `panels/scene` | scene_window.rs:136-201；render 358-480 移出 | W22 ImagePreview、W15 EmptyState、W02 IconButton（可选视图控件） | 无（相机/尺寸进 `ViewportState`/`World`） | `Viewport::Input{orbit/zoom/fly}`、`Layout::FocusPanel`；Effect `CreateViewport/SyncViewport` | README 明确相机留 World；组件只发输入事件；多实例需 `PanelInstanceId` |
| C15 | `panels/game` | game_window.rs:84-150；render 174-266 移出 | W23 Toolbar、W08 ComboBox、W06 NumberInput、W09 Checkbox、W22 ImagePreview | `aspect: AspectMode`、`scale: f32`、`maximize_on_play: bool`、`stats: bool`、`gizmos: bool` | `Play::{Play, Pause, Stop}`（若实现）、`Viewport::SyncSize`、`Layout::FocusPanel` | 现状控件全为占位（`&mut 1.0_f32`），语义待定（§9 Q6） |
| C16 | `panels/toolbar` | tool_bar.rs:135-244 | W24 MenuBar、W01 Button、W02 IconButton、W23 Toolbar | 无（菜单开合用 egui Memory） | `Command::dispatch(CommandId)`（不是字段级 Action；内部映射为 `Layout::*`/`System::Quit`/`Layout::OpenPanel`） | 菜单项必须来自 `CommandRegistry`，不能继续硬编码 `todo!()`（tool_bar.rs:207） |
| C17 | `panels/preferences` | preferences_window.rs:194-413；ui_style_fields.rs | W19 ListRow（页面列表）或 W18、W05 PropertyGrid、W06/W07/W08/W09/W11、W14 Tooltip、SearchInput（可选）、W12 | `selected_page: Option<SectionId>`、`filter: String`、`scroll` | `Style::SetField{key, value}`、`Style::ResetField{key}`、`Layout::ClosePanel`；Effect `PersistStyle`（debounce） | schema 描述能力（range/step/unit/enum/颜色/向量）；旧 TOML 迁移；多 pass 安全 |
| C18 | `panels/console` | console_window.rs:70-83 | W19 ListRow（日志行）、W06/W08（过滤/级别）、W01（清空）、W16 | `filter: String`、`level: LogLevel`、`follow: bool`、`cursor: u64` | Console 新域（uncertain）：`Console::SetFilter`、`Console::Clear`、`Console::ScrollToEnd`；或纯 ViewState + `System::QueryLogs` 快照 | 现状 `pop_front` 消费日志（不可回放）；新架构日志在 host 环形缓冲 |
| C19 | `panels/about` | about_window.rs:87-105 | W03 Label、W12 Section、W01（可选关闭） | 无 | `Layout::ClosePanel`（或 `Layout::OpenPanel(About)` 反义） | About 是面板还是模态？现状是 Drawer/窗口，建议保留 panel |
| C20 | `docking/workspace`（宿主） | docking_tab.rs 全量 + dock_state/** + window_state.rs | W23/W24（add/close/collapse 按钮内部沿用）、TabStrip 保留 | `DockState<PanelInstanceId>`、`WindowState` map、`drag: Option<DragDropState>` | `Layout::{FocusPanel, ClosePanel, MovePanel, OpenPanel, RestoreLayout}`、`Layout::Persist` | 只改泛型与 `TabDrawer` 签名，**不重写绘制**（2542 行，风险最高） |
| C21 | `status_bar` | project_window.rs:383-400 + 新增全局 | StatusBar、DirtyBadge、W02 | 无 | `System::OpenExternal`（当前文件路径） | 新增组件，无现状可搬 |
| C22 | `modal_layer` | dialog.rs:42-70；native_dialog.rs | ModalFrame、W01 Button | 无（`ModalState` 在 core） | `Modal::{Confirm, Cancel, Close}`；`System::NativeDialog` Effect | 原生对话框仍走 host；错误显示不要再用 `native_dialog::ui_create_error_window` 阻塞 |

### 6.3 ViewState 归属规则（对 §6.2 的约束）

| 状态 | 归属 | 理由 |
|---|---|---|
| 展开/选中/当前目录/重命名缓冲/过滤词/dock 树/窗口 rect | **ui ViewState** | 重启不必须恢复；不需要 MCP/undo |
| 滚动位置、caret、hover、菜单开合 | **egui Memory**（或 ViewState 若需跨 pass 稳定） | 只影响本交互 |
| 文档内容、dirty、undo、pending save | **core DocumentsState** | 可序列化、可测、MCP 可见 |
| 选中资产/实体 | **core SelectionState** | 单一事实来源（现状散在 3 处） |
| 面板开关/焦点 | **core LayoutState** | 可持久化、可被命令查询 |
| 样式值 | **core StyleState** | Preferences/MCP 读写 |
| 播放状态（音频/材质预览） | **core PlayState / ViewportState** | 时间驱动，driven by `Play::Tick` |
| 相机 orbit/zoom/fly、视口 size、渲染图 | **Engine.world**（README §3.4） | 引擎真值；UI 只发 Effect |
| egui 纹理句柄、字体、图标缓存 | **ui HostView** | GPU 资源不是领域模型 |

### 6.4 现状 → 目标 Action 映射（按域）

| 现状 Message（代表） | 目标 Action | 域 | 备注 |
|---|---|---|---|
| `SelectProjectNode`（ui.rs:130/545-556） | `Selection::SelectAsset{asset}` + reducer 跨域发 `Document::Open`/`LoadDocument` | Selection | 现在由中央 match 手工同步 ProjectWindow+InspectorWindow 两处 |
| `NavigateToProjectDirectory` | `Project::NavigateToDir`（或纯 ViewState） | Project（uncertain） | 若算 UI 会话态，可只写 ViewState 不发 Action |
| `Create/Rename/Delete/DragStart/DragStop/DragConsumeProjectNode` | `Project::{CreateAsset, RenameAsset, DeleteAsset, RequestDelete, MoveAsset}` | Project | 删除要 `Modal::Open(ConfirmDelete)`；磁盘走 Effect |
| `OpenProjectNode` | `Document::Open{asset}` / `System::OpenExternal{path}` | Document/System | 现状按 AssetKind 分支（project_window.rs:337-369），应进 reducer |
| `Open/Close*Tab`（12 个变体） | `Layout::{OpenPanel{kind}, ClosePanel{inst}, FocusPanel}` | Layout | 窗口生命周期不再用消息 |
| `UpdateSceneWindowSize / GameWindowSize` | `Viewport::SyncSize{view, size}` → Effect `SyncViewport` | Viewport（uncertain） | 现状每帧发消息（scene:190，game:139） |
| `SceneViewOrbit/Zoom/Fly` | `Viewport::Input{...}` → Effect `When::BeforeEngineUpdate` 写 `SceneViewInput` | Viewport | README 要求 BeforeEngineUpdate 时机 |
| `Registe*ViewBind / *TryReceTextureId`（4 变体） | `EffectOutcome::ViewportBound{req, view}` → `Action::Effect` | Effect/System | oneshot 不进状态树 |
| `Document/Code/Shader/TomlInspectorSave`（4 个，带 `Arc<Mutex<Option<..>>>`） | `Document::Save{id}` → Effect `WriteDocument` | Document | 快照/写盘/清 dirty 都在 reducer+effect |
| `MaterialInspectorChangeShader/DropTexture/ChangeRenderState/ClearTexture` | `Document::Edit{id, MaterialPatch}` | Document | edit-in-place → 文档缓冲 + preview effect |
| `MaterialInspectorApply/Discard`、`TextureInspectorApply` | `Document::Save` / `Document::Revert` / `Modal::{Confirm,Cancel}` | Document/Modal | 关窗确认 = `ModalState::Confirm{SaveThenClose}`（原型已验证竞态） |
| `AudioInspectorTogglePreview/SeekPreview/Tick` | `Play::{TogglePreview, SeekPreview, Tick{dt}}` | Play | Tick 由帧循环固定阶段产生 |
| `ModelInspectorCreateWireframeMesh` | Effect（预览网格派生）或 `Viewport::Create` | Effect/Viewport | 现状消息携带整个 `Mesh`（ui.rs:182），应改为 effect 内部处理 |
| `UpdateUIStyle(StylePage)` | `Style::SetField{key,value}` → Effect `PersistStyle` | Style | 删除 StylePage/下标/每帧回传 |
| `RefershPreferenceWindow` | 删除；Preferences 直接读 `StyleSchema` | Style | 不再遍历 Drawer |
| `QuitEngine` | `System::Quit`（有 dirty 先 `Modal`） | System | README §3.3.1 退出协议 |
| `Open/CloseAboutWindow`、`Open/ClosePreferenceWindow` | `Layout::{OpenPanel,ClosePanel}` | Layout | About/Preferences 作为 panel |
| `InitLayout` / 4 个假容器 | 删除；启动时直接构造 `DockState<PanelInstanceId>` | Layout | layout.rs 279 行全删 |

---

## 7. 三个代表文件 before/after 估算

### 7.1 `ui/project_window.rs`（622 行）

| 现状分块 | 行数 | 去向 |
|---|---:|---|
| style DTO + 加载（35-98，含嵌套 2 个 TOML 节） | 60 | 删除 → `Theme` + `ProjectTokens`（约 20 行 schema） |
| Model 字段（49-71，含 `Cell`/`Arc<Mutex>`） | 23 | 删除 → `ProjectState` + `ProjectPanelViewState` |
| `new()`（100-140，registry 加载 + watcher 启动） | 41 | 移 host（Effect）约 20 |
| 领域操作（151-443：select/navigate/create/rename/open/delete/drag） | ~230 | 上移 `core/project.rs` 约 180；Effect（写盘/IDE）约 120 |
| watcher/重扫/GUID 恢复（159-200） | 50 | `ProjectState` + Effect 约 45 |
| `get_selected_node_info` + `InspectorCreater`（211-239） | 29 | 删除（reducer 按 Selection 打开 Document） |
| 绘制（`ui` 522-595 + `draw_bottom_bar` 383-400 + `draw_drag` 445-505） | 153 | C01 组件胶水约 80（TreeView/ThumbnailGrid/StatusBar/DragGhost 调用） |
| Drawer 样板（508-622） | ~80 | 删除（Panel trait 只剩 3 个方法） |

**after**：
- `ui/panels/project.rs`：**155-175 行**（组件胶水 + 两个 widget 组合 + filter/rename 状态接线）。
- `ui/panels/project_view_state.rs`：30-40 行。
- `ui/widgets/tree_view.rs` / `thumbnail_grid.rs`：进 kit（复用）。
- `core/project.rs`：约 200-240 行（不是删除，是搬家）。
- `host/project_effects.rs`：约 110-140 行（watcher、registry 写盘、IDE 启动、文件系统 CRUD）。
- 风险：watcher 与 registry 写盘的**自激**（现状 refresh 只在新增路径时保存，project_window.rs:183-191，这个细节必须保留）；重扫重建索引 → 组件必须用 `AssetKey(Guid/path)` 而不是 `NodeIndex`；ContentPanel 的多列网格与 TreeView 布局差异大（不能硬套 TreeView 当网格）。

### 7.2 `ui/inspector/material.rs`（1,506 行）

| 现状分块 | 行数 | 去向 |
|---|---:|---|
| style + 加载（56-114） | 60 | 删除 → `Theme` + `MaterialTokens`（schema 约 20） |
| ShaderMenuNode 树 + Thumbnail + PreviewState（121-280） | 155 | 部分进 W24/W21；`PreviewState` 约 60 行并入共享 viewport 绑定（ui 层） |
| Model（286-307，含 4 个 Cell/Mutex） | 25 | 删除 → `MaterialEditData` + ViewState |
| blend 映射 + 两个 combo 行（322-407） | 85 | 删除（`StyleSchema`/PropertyGrid 数据行 + 映射进 core） |
| 纹理拖放槽（409-567） | 158 | W21 ImageDropBox（约 90 行 widget）+ 拖拽逻辑进组件 20 |
| shader 菜单（569-608） | 40 | W24 MenuBar/TreeView 组合约 25 |
| RenderState 表单（638-894） | 257 | PropertyGrid：7 组行数据约 90 行声明 + 组件 glue 60 |
| 3D 预览 draw + toolbar + render（903-1001、1219-1298） | 176 | W22 ImagePreview（共享）约 15 行调用 + 共享 viewport effect（host 约 60） |
| create（1005-1059，资产加载） | 55 | Effect `LoadDocument` + reducer 建文档 |
| draw（1060-1217，含 Ctrl+S、Apply、滚动） | 157 | 组件约 90（属性网格 + ApplyBar + Preview 组合） |
| on_exit + apply/discard message（1300-1318、1413-1431） | ~37 | `ModalState` + Action（删除） |
| 领域变更（1323-1411） | 89 | core reducer（patch 应用）+ Effect（preview/缩略图） |
| 保存（1433-1505） | 73 | Effect `WriteDocument` |
| 其余（imports、apply） | ~15 | — |

**after**：
- `ui/editors/material.rs`：**330-390 行**（属性行数据构造 + shader/纹理/预览组合 + edit→patch glue）。
- `ui/editors/material_view_state.rs`：约 30 行。
- `core/document/material.rs`：约 150-200 行（`MaterialEditData`、patch、dirty/undo）。
- `core/document/render_state_edit.rs`：约 100 行（`RenderState` ↔ 行数据/枚举选项的纯映射，可单测）。
- `ui/viewport/preview.rs`：约 80 行（与 Mesh/Scene 共用的绑定 + 相机 ViewState）。
- `host/material_effects.rs`：约 100 行（保存、缩略图解码、预览渲染命令）。
- 风险：**edit-in-place → 文档缓冲 + preview effect 的时序**（拖动滑块时预览必须即时，但保存要显式）；`Apply/Discard` 语义从"运行时 Material 回滚"变成"文档 revert"；3D 预览相机与 Scene 共用 viewport 注册表后，谁持有 orbit 状态需定（World vs ViewState）；Ctrl+S 上移 `CommandRegistry` 后，组件不再读快捷键。

### 7.3 `ui/preferences_window.rs`（414 行）

| 现状分块 | 行数 | 去向 |
|---|---:|---|
| style DTO + 加载（18-45） | 28 | 删除（schema 自带默认值） |
| Model（47-63） | 17 | 删除 → ViewState |
| Drawer 样板（82-180：get_style_fileds 35 + update_style 14 + 其余） | 99 | 删除（Panel trait） |
| `registe_ui_styles` / `update_style_page`（183-192，含 `copy_from_slice`） | 10 | 删除 |
| `ui()` 194-413（220 行：6 种 StyleField 渲染 + 每帧 send） | 220 | W19 ListRow（页面列表）+ W05 PropertyGrid（schema 驱动）约 70 行 |
| imports/常量 | ~40 | 约 15 |

**after**：
- `ui/panels/preferences.rs`：**100-130 行**（页面列表 + schema → PropertyGrid + 保存接线）。
- `core/style/schema.rs`：约 100-140 行（`StyleSchema`/section/field/scope + 默认值）。
- `core/style/reduce.rs`：约 40 行。
- `host/style_io.rs`：约 50 行（单个 `StyleState` TOML 读写 + debounce）。
- `ui/widgets/property_grid.rs`：进 kit。
- 风险：**schema 表达力**（range/step/unit/enum/颜色/向量/路径，现状 StyleField 只有 6 种）；**旧 21 个 TOML 的兼容迁移**（至少保留 `GlobalStyles` 字体/图标配置；样式值一次性 migrate）；"每帧写盘"必须换成 debounce effect（否则改任何字段都会 ~60 次/秒 IO）。

### 7.4 汇总

| 文件 | before | after（UI 层） | Δ | 搬去 core/host（行） |
|---|---:|---:|---:|---:|
| `project_window.rs` | 622 | 155-175 | **−72%** | ~200-240 core + ~110-140 host |
| `inspector/material.rs` | 1,506 | 330-390 | **−75%** | ~250-300 core + ~160 host + ~120 ui(viewport) |
| `preferences_window.rs` | 414 | 100-130 | **−72%** | ~140-180 core + ~50 host |
| 合计 | 2,542 | 585-695 | **−74%** | ~860-1,110 |

> 口径说明：after 行数只算"目标文件里剩下的组件胶水 + ViewState + 组合调用"；widget 实现行数计入 kit（§5.3 表格），core/host 行数单列，所以总数不是简单相减。**净代码量不会减少 74%，但 UI 文件会减少约 74%，重复绘制会减少约 50-60%。**

---

## 8. Widget kit 构建顺序（按"解锁面板数"排序）

| 阶段 | 内容 | 解锁 | 估算 |
|---|---|---|---:|
| **P0（第 1-1.5 周）** | `Theme` + `Out<T>` 约定 + W01 Button、W03 Label、W04 KeyValueRow、W05 PropertyGrid、W06 NumberInput、W08 ComboBox、W09 Checkbox、W10 TextInput、W11 ColorInput、W12 Section、W13 Separator + `egui_kittest` 基建 | **8 个面板/编辑器**：Inspector 壳、text/document、shader、toml、material(只读表单)、texture(只读表单)、preferences、about | ~1,100-1,300 行 + ~20 交互测试 |
| **P1（第 2 周）** | W02 IconButton、W14 Tooltip、W15 EmptyState、W16 LoadingState、W17 DirtyBadge、W18 TreeView、W19 ListRow、W23 Toolbar + ModalFrame | **+5**：project、hierarchy、console、toolbar、全窗口模态/未保存确认 | ~900 行 |
| **P2（第 3 周）** | W20 ThumbnailGrid、W21 ImageDropBox、W22 ImagePreview、W24 MenuBar、W25 ContextMenu | **+4**：scene 视口、game 视口、材质/mesh 3D 预览、项目 content 网格 | ~800 行 |
| **P3（第 4 周，可并行）** | W07 RangeSlider（改造 egui_ext）、SearchInput、StatusBar、DragGhost、IconSet（11 个 PNG 预加载）、dock `TabStrip` 签名改造 | 体验补齐；dock 不需要重写 | ~500 行 |

**为什么这个顺序**：PropertyGrid/PropertyEditor 一家解锁 5 个 inspector（material/texture/mesh/toml/preferences，现状合计约 2,900 行表单代码）；TextInput+code 变体解锁 4 个文档编辑器（约 1,000 行，code/shader 95% 重复）；TreeView 解锁项目树（约 500 行）；ImagePreview 把 4 套视口/预览绑定收敛成 1 套（约 260 行）。先做"表单 + 文本"能在 2 周内让 Phase 1（项目/层级/Inspector/控制台）具备可移植条件，符合 rewrite-plan 的 strangler 路径。

---

## 9. 开放问题（8 个，需 Lead 决策）

1. **Widget API 形状**：`fn button(ui, theme, &props) -> Out` 还是 `struct Button；impl Widget`？是否直接返回 `egui::Response` 以复用 egui 生态（`add_enabled`、`on_hover_text`、`kittest`）？建议由 task-8 原型给结论；本 catalog 的 props/Out 设计假定"函数式 + Out 包装"。
2. **PropertyValue 边界**：`PropertyEditor` 只接受纯值 enum（f32/bool/String/Color32/[f32;N]/Range），`RenderState`→行数据的映射放**组件 glue**（每编辑器 ~60 行）还是 phase-5 的 `DataUi` 反射注册表？后者能消掉 material 的 ~100 行声明，但要 bring `kairos_reflect`。
3. **Theme 归属**：`Theme` 是 ui 层从 `core::StyleState` 派生的只读视图，还是 ui 自己持有并持久化？MCP 是否需要读 token（→ 必须进 core）？当前审计倾向：值进 core，`Theme` 是 `fn(&StyleState) -> Theme` 的纯映射。
4. **108 个旧 style key 的迁移策略**：哪些并入 44 tokens、哪些变 per-panel overrides、哪些直接删（`PATH_DIALOG_WINDOW_STYLE`、`MainContentStyle.toml` 已死）？是否保留旧 TOML 一个版本周期做导入？
5. **dock 是否纳入 widget kit**：dock 6,762 行（38% UI 代码）已 widget 化，建议只改 `TabDrawer` 签名（去掉 `Drawer`/`Messager`/`Engine`/`Log` 参数，换成 `PanelInstanceId`）并保留内部绘制，不重写。是否同意这个"冻结内部、只换签名"的低风险路径？
6. **Viewport 域**：scene/game/material-preview 的 size/bind/camera 归 `core::ViewportState` 还是继续留 `World`（README §3.4 倾向留 World）？`ImagePreview` 只拿 `TextureId` + size + 输入事件，谁把输入转成 `ViewportAction`（组件 glue vs host）？
7. **TreeView 拖拽 payload**：widget 必须无领域类型。用 `DragPayload { id: String, bytes: Vec<u8> }`（弱类型）还是泛型 `TreeView<T: Clone>`（强类型但 props 变复杂）？项目树需要 `PathBuf`、材质需要纹理路径、未来资产导入需要多文件。
8. **图标体系**：现状 3 种（`file://` URI 图片 11 个 PNG、Unicode 字形 `"▶/⏸/X"`、dock 里 painter 手绘箭头）。是否统一为 `IconSet`（启动加载 → `TextureHandle` 缓存）+ `Icon` 枚举？是否引入图标字体（lucide/egui-phosphor）作为新依赖？CJK 字体（NotoSansSC）与图标 fallback 的顺序需要定。

（候选第 9 问，留给 Lead 视情况合并：**中文/英文文案与 tooltip 的 i18n 策略**——现状 UI 文案中英混杂（"have modify not save" code.rs:198、"not implement inspector"、"* unsaved changes"），dock 已有 `translations.rs` 文案表，新 kit 是否统一走翻译表？）

---

## 10. 附录

### 10.1 复现命令（本次审计实际执行）

```bash
# 文件规模
find kairos_editor/src/ui -name "*.rs" | wc -l          # 54
find kairos_editor/src/ui -name "*.rs" | xargs wc -l | tail -1   # 17625

# 模式计数
grep -rn --include=*.rs -E 'ComboBox::from_id_salt' kairos_editor/src/ui | wc -l   # 18
grep -rn --include=*.rs -E 'body.row' kairos_editor/src/ui | wc -l                 # 32
grep -rn --include=*.rs -E 'row.col' kairos_editor/src/ui | wc -l                  # 70
grep -rn --include=*.rs -E 'ui.button|Button::new' kairos_editor/src/ui | wc -l   # 33
grep -rn --include=*.rs -E 'DragValue' kairos_editor/src/ui | wc -l                # 21
grep -rn --include=*.rs -E 'egui::Slider' kairos_editor/src/ui | wc -l             # 5
grep -rn --include=*.rs -E 'ui.label|RichText::new' kairos_editor/src/ui | wc -l  # ~144
grep -rn --include=*.rs -E 'ui.separator' kairos_editor/src/ui | wc -l            # 23

# 样式
find Preferences/Styles -name '*.toml' | wc -l                                      # 21
grep -rhE '^[a-zA-Z_].*=' Preferences/Styles/*.toml Preferences/Styles/Inspectors/*.toml | sed -E 's/ *=.*//' | sort -u | wc -l   # 108
grep -rn 'get_style_fileds' kairos_editor/src/ui.rs                                 # ui.rs:480
grep -rn 'update_style(&style_page.fields)' kairos_editor/src/ui.rs                 # ui.rs:498

# 中心路由
awk 'NR>=90 && NR<=206' kairos_editor/src/ui.rs | grep -cE '^    [A-Z][A-Za-z]*'    # 57
grep -c 'get_window_mut::<' kairos_editor/src/ui.rs                                 # 33
grep -c 'get_inspector_mut::<' kairos_editor/src/ui.rs                              # 10
grep -c 'engine.world' kairos_editor/src/ui.rs                                      # 24
```

### 10.2 审计发现的死代码/缺陷线索（供后续清理，不属本任务改动）

| 线索 | 证据 |
|---|---|
| `PATH_DIALOG_WINDOW_STYLE` 无引用 | paths.rs:17；全仓库仅此一处 |
| `Preferences/Styles/MainContentStyle.toml` 无引用 | 无 `PATH_` 常量、无读取 |
| 3 个空布局 Drawer（LayoutLeft/Right/Bottom）恒占 3 个样式页；第 4 个 `LayoutCenterContainer` 只有 impl 从未实例化 | layout.rs:18-186 + ui.rs:426-458 |
| `Message::ModelInspectorCreateWireframeMesh(Mesh)` 把整个 Mesh 塞进消息 | ui.rs:182；mesh.rs:319-323 每帧检查 `wireframe_mesh_handle.is_none()` |
| `game_window` 控件全是占位（`&mut 1.0_f32`、`&mut false`） | game_window.rs:111-124 |
| `tool_bar` 菜单 `New Scene` 是 `todo!()` | tool_bar.rs:206-208 |
| `hierarchy_window` 是 TODO 占位 | hierarchy_window.rs:77 |
| Console `pop_front` 消费日志（不可回放） | console_window.rs:80 |
| About 每次重绘写日志 | about_window.rs:96 |
| Preferences 每帧发整页样式消息 | preferences_window.rs:404 |
| `dialogs` 只在 `darw()` 末尾 retain，关闭语义与窗口隐藏纠缠 | ui.rs:401-408、855-863 |
| `get_style_fileds` 的 typo 字段名 `"corrner_radius"` | tool_bar.rs:267 |
| 文案中英混杂/拼写错误 | code.rs:198 `"have modify not save"`、tool_bar.rs:311 `"Falied"` |

### 10.3 与其它文档的关系（避免重复劳动）

- **task-6（widget/component 分层调研）**：回答"为什么分两层、业界怎么分"；本文件只给**本仓库现状的目录与证据**，两者的分类术语以 task-6 为准，本文的 W/C 编号可直接作为 task-8 原型的输入清单。
- **task-8/9（widget 原型，已合并）**：`prototypes/editor-ui/crates/{ui-widgets, editor-components}` 已完成并与真实 core 类型接通（49 tests，含 7 个 full-stack）；本文 §5.3 的 props/Out 建议、§8 的 P0 顺序即其验收清单；合并时暴露的 core 缺口见原型 README §8。
- **架构 README §3.5 Panel trait / §3.7 Inspector provider / §3.10 样式与模态**：本文件的 C01-C22 是它们的"现状 → 目标"落表；A 编号 Action 域沿用 README，新增 `Viewport`/`Play` 标 uncertain 待 Lead 定。
- **`editor-ui-messager-mvc-audit.md`**：已覆盖 Message/Messager/Context 的架构级问题；本文件只补**视觉/widget 层**的量化与目录，不重复 57 变体/365 行 match 的架构论证（仅引用数字）。

---

*本文件为设计调研产物，未修改任何生产代码；只读审计，未运行 cargo。*
