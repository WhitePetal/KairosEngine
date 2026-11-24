# ProjectPanel 迁移重构计划

> **状态**：已拍板（D1–D15 全部落定，见 [§7](#7-决策记录已拍板) 与 §7.2）。**M0 / M0.5 / M1 / M2 已落地**（M1 分 M1a core + M1b app 两刀，M2 分 M2a core+widget / M2b 面板两刀）。M1 开工前对方案做了一次复核，补上了 5 处会现场发明的设计空白、3 处自相矛盾，结论已并入 §2.2 / §2.3 / §2.5 与 §3 的 M1 一节。每个里程碑的实测与已知行为记在对应小节末尾。M2 已按「开工前复核定稿」定稿并开工（M2a/M2b 两刀，见 M2 一节），交付后又按真机验收反馈做了 7 条修正（见 M2 一节末尾的「M2 验收修正」）。**M3 已复核并对齐定稿（T1–T16，含弹窗/类型表/图标三处分界），拆 M3a core+app+widget / M3b 面板两刀，M3a 四层已落地（画面几乎不变）、M3b 面板功能已落地（右键菜单 / 行内重命名 / 删除确认 / 多选与键盘 / 拖拽移动 / 类型图标），并已按真机验收第二轮反馈做了 7 条修正（弹窗背板 / 文件夹区域落点 / 拖拽自滚动 / 多选保持 / 幽灵 / 纹理缩略图 / 多选 anchor）、第三轮反馈做了 3 条修正（离线文件夹的区域框 / 自动滚手感 / 弹窗标题条改用项目共享的红色圆圈关闭按钮），**M3 已收口（2026-11-22）**（见 M3 一节）。**M4 已重定义为「编辑器布局持久化」**（2026-11-25 用户拍板 D13：布局落盘、面板会话态不落盘，原 M4 方案从计划删除），开工前复核定稿见 M4 一节；**实现已落地（2026-11-25），待真机验收**（见 M4 落地状态）。 **新需求（2026-11-25，用户提出）**：`Edit ▸ External Tools` 面板与 Rust 脚本图标已插入本计划——面板见 §2.14（P1–P3 已拍板），图标见 §2.15（已交付）；M5 相应拆为 M5a/M5b（见 M5 一节）。
> **范围**：`kairos_editor_core`（项目领域）/ `kairos_editor_app`（扫描、监听、文件操作、持久化、打开方式探测与启动）/ `kairos_editor_component`（面板）/ `kairos_editor_widgets`（树控件）；旧 `kairos_editor` 只冻结，最后退役
> **配套**：[ADR 0008](../../adr/0008-editor-ui-architecture.md)、[ADR 0009](../../adr/0009-editor-panel-host-and-viewport.md)、[rewrite-plan](../editor-ui-architecture/rewrite-plan.md)（Phase 0.5 写明「只迁 ProjectPanel 一页」）、[panel-viewport-design](../editor-ui-architecture/panel-viewport-design.md) §2.6 状态归属、§6.2 P1、[widget-layer](../editor-ui-architecture/widget-layer.md) §2.2/§5
> **可抄的形状**：`prototypes/editor-ui/crates/editor-components/src/project_panel.rs`（派生函数 + 行渲染 + action 映射）、`prototypes/editor-ui/crates/editor-core/src/reduce/project.rs`（data-in / effects-out）
> **旧实现**：`kairos_editor/src/{project_path_tree.rs, asset_registry.rs, project_watcher.rs, ui/project_window/*}`

---

## 0. 结论

### 0.1 你给的骨架成立

5 个阶段的**依赖顺序是对的**：先树、再面板、再持久化、再类型、最后搜索。我不会推倒重来，而是把它落成 9 个可独立验收的里程碑（M0–M8），每一步都写明**代码落在哪个 crate**、**验收怎么量**。

### 0.2 要改的 5 处

1. **第 1 步不是「重构 ProjectTree」，而是「把树拆成三层」**：core 里的纯模型 + app 里的扫描/监听 + 删除 GUID 身份。只重构数据结构，会把 `std::fs` 调用和原生弹窗一起留在原地（现状：`project_path_tree.rs:395` 建目录时直接弹错误框，`:406-410` 五种类型是 `todo!()`，右键建个纹理就 panic）。
2. **第 2 步之前缺一整层接缝**：新 app 目前**没有**「host 异步结果写回 state」的通道（`app.rs:250` 的 `pump` 只吃 UI 产出的 action），也**没有**树控件。不做 M0 + TreeView，面板会重新长出旧那一套 `Arc<Mutex>` / `Cell`（旧 `ProjectWindowModel` 里有 3 个 `Cell`、2 个 `Arc<Mutex>`）。
3. **第 3 步的「持久化」拆开是两件事**：(a) **编辑器布局**（打开的面板、dock 停靠树与分割比例、tab 顺序/焦点、浮动窗口、Project 面板左右比例）→ 新增布局文件（M4，D13）；**面板会话态（展开/选中/当前目录/滚动/搜索）不落盘**；(b) 面板皮肤 → 已由 `#[panel(skin = ...)]` 解决；(c) GUID 注册表 → **删的是"编辑器自己发号"的那份**（唯一消费者是 Inspector 显示一行文本：`ui/inspector_window.rs:170`）；**资产身份本身要保留**，只是应该搬进 `.meta`，见 §2.11 与 D11。
4. **你把「文件/文件夹的增删改查」列进了后端组成，但 5 步里没有任何一步在排它**。必须补 M3，而且它要早于类型任务——类型任务会新增创建入口、拖拽移动、删除确认，全靠这套协议。
5. **「引入各种类型的资源」需要一个前置 seam**：`FileTypeRegistry`（扩展名 → 类型、图标、后缀标签、打开方式、是否伴生文件）。没有它，每个类型任务都会去改树和面板，第 4 步就会从 5 个任务变成 5 次重构。

### 0.3 要补的 5 块

- **文件操作**（建/改名/移动/删除 + 系统回收站 + 失败可见），见 M3；
- **外部变更**（watcher，已有实现要搬家，不是新写），见 M1；
- **多选与拖拽**（右键菜单之外的真实使用路径），见 M3；
- **性能上限与截断**（后台扫描、节点上限、深度上限、状态栏告警），见 M1/§5；
- **打开方式的配置面板（External Tools，2026-11-25 新需求）**：现在打开由 OS 关联 + 硬编码嗅探决定，用户看不见也改不了；见 §2.14 / M5b。

### 0.4 工期

| 阶段 | 估算（单人） |
|---|---|
| M0.5 资产布局迁移（D12） | **0.5–1 人日**（跨 `kairos_asset`/`kairos_engine`/`kairos_graphics`） |
| M0 + M1–M6（面板本体，不含类型任务） | **16–22 人日 ≈ 3.5–5 周**（M3 复核后 M3 由 3–5 调为 5–6；M4 定稿后由 1.5–2.5 调为 2.5–3.5；M5 拆 M5a/M5b 后 M5a 时长不变） |
| M5b External Tools 面板（新需求，2026-11-25） | **2.5–3.5 人日**（面板 / 菜单 / 皮肤 / 测试；M5a 的 seam 已含在上一行） |
| M7 资产身份与引用维护 | **5–8 人日**（跨 `kairos_asset`/`kairos_graphics`） |
| M8 收尾（冻结 + 文档） | **0.5–1 人日** |
| **合计（不含类型任务）** | **24–34.5 人日 ≈ 5–7 周** |
| M5a 的 seam（已含在 M0–M6 里） | 2–2.5 人日 |
| M5 的每个类型任务（另计） | 0.5–2 人日 / 类型 |

前提：保留现有 dock、字体/样式资源与资产加载，不重写引擎侧。

### 0.5 决定摘要（D1–D15 已拍板，细节见 §7 / §7.2）

- **D1 左右两栏，第一天就定型**：右栏先只放面包屑 + 空白占位，网格/缩略图后补；分栏比例进持久化。这样布局、分割条、导航动作一次到位，后面只填内容。
- **D2 单根 + Library 特殊待遇**：仍然是**一个根**（`Library` 就是项目里的目录）；展示上它固定排最后、前面一条分隔线、默认折叠；忽略规则从项目根的 `.kairosignore` 读，不作用于它。**搜索按 Unity 分范围**：默认 `In Assets`（不含 Library），可切 `In Library`。
- **D3 持久化只存「用户意图」，不存树**：`asset_registry.toml` 是**身份表，不是加速缓存**，它一条 `read_dir` 也省不掉（§2.7.1）；M1 结束后用真实工程测一次扫描耗时，再决定要不要索引缓存（§2.7.2）。
- **D4 删除编辑器注册表，身份搬进 `.meta`**：现状引用是纯路径（`.mat` 存纹理路径、`AssetPath`/`Handle` 全路径）；改名安全的引用按 Godot 形态做，落点在资产系统（§2.11），**并入本计划的 M7**。
- **D5 回收站 + 默认确认**：文件和文件夹默认都弹确认（Unity 行为），`FileType::delete_confirm` 只用于个别类型关掉。
- **D6 单选起步**，M3 加多选 + 树内移动。
- **D7 搜索支持 `t:` 类型过滤语法**（Unity 风格，见 §2.9）。
- **D8 旧 crate 整体退役时再删**：本计划不动 `kairos_editor/src` 一行代码（M8 只写文档）。
- **D9 保持 cwd**，但按 §2.12 的三条纪律写；将来接「打开工程」时只动 app 侧，不返工面板。
- **D10 一开始就上 `.kairosignore`**：项目根、gitignore 风格子集、缺失时 seed 默认文件；不再另设编辑器级忽略配置。
- **D11 引用维护并入**：作为 M7，5–8 人日，跨 `kairos_asset`/`kairos_graphics`/`app`/`component`；M3 先埋「移动/删除以簇为单位」的地基。
- **D12 成品根搬进 `Library/`**：作为 M0.5，直接现在搬（`imported_assets/Default` → `Library/imported_assets/Default`），换掉 `ScanPolicy` 里的「跳过成品根」特例。
- **D13 持久化范围 = 编辑器布局**：存 dock 布局（打开的 panel、分割树/比例、tab 顺序/焦点、浮动窗口位置尺寸）+ Project 面板左右比例；**面板会话态（展开/选中/当前目录/滚动/搜索）不落盘**，原 M4 的 `project_view.toml` 方案删除。理由：布局是用户立刻感知的收益；会话态细节低收益，架构文档本就把它归 `ViewStates`（可选、按需）。未来「打开的 World/Scene」另立文档/会话需求。
- **D14 External Tools 面板（新需求，2026-11-25 已拍板）**：`Edit ▸ External Tools` 打开，按文件族记录/设置打开应用；嗅探结果是默认、用户设置优先、未知类型默认系统「打开方式」；见 §2.14。
- **D15 Rust 脚本图标（2026-11-25 已交付）**：`.rs` 的类型图标从 Unity 的 C# 页（`#`）改为 Rust 页（`R` + Unity 绿，dark `#276A2A` / light `#2E7D32`）；保留页面外形，只换字符语义；见 §2.15。

---

## 1. 现状：代码里真实的样子

### 1.1 两个编辑器、四个 crate

`cargo run` 现在跑的是 `kairos_editor_app`（`kairos_editor_app/Cargo.toml:8` 的 `default-run`），它**不依赖** `kairos_editor`。旧编辑器是另一个 bin（`kairos_editor/src/main.rs`），属于「旧 UI 冻结、逐 phase 删除」的那一半。

| crate | 角色 | 能做什么 | 不能做什么 |
|---|---|---|---|
| `kairos_editor_core` | 状态树 + 唯一写入者 | 纯数据、`reduce()`、`Action`/`Effect` | egui、IO、线程、锁 |
| `kairos_editor_widgets` | 纯外观 | egui + `kairos_color` | 领域类型 |
| `kairos_editor_component` | 面板 | 读 `Frame`、画、推 `Action` | 依赖 app、做 IO（皮肤文件除外） |
| `kairos_editor_app` | 组合根 + host | dock、egui 上下文、引擎 | 面板内部逻辑 |

这张表就是本计划所有「这段代码写哪里」的判据。

### 1.2 旧 ProjectWindow 的行为契约

旧实现一共 35 个测试（`project_path_tree/test.rs`），覆盖的行为是这次迁移必须逐条对齐或明确放弃的：

| 能力 | 旧行为 | 新计划 |
|---|---|---|
| 扫描 | 全量递归，构造时同步扫（`ProjectPathGraph::new_at_root`） | 后台全量扫 + generation（M1） |
| 节点身份 | GUID（`Library/asset_registry.toml`） | 相对路径（M1，D4） |
| 显示范围 | 目录 + 8 种已知扩展名，`.meta` 隐藏，未导入的 `.png` 隐藏，未知文件直接丢（`asset_registry.rs:336-366`） | 全部文件，类型的「隐藏/伴生」由注册表决定（M1/M5，D2） |
| 排序 | 「目录在前」，同级按 `read_dir` 顺序，展示时每帧重排（`sorted_children`） | 建树时一次排好，目录在前 + 不区分大小写（M1） |
| 展开 | `CollapsingHeader` + 一次性 `force_expand_to` | core 的 `expanded` 集合 + 行控件（M2） |
| 导航 | 左侧树 + 右侧内容网格（`project_window.rs:544`）+ 底部路径栏 | 树 + 面包屑 + 状态栏（D1） |
| 创建 | 目录 + 3 类文本文件；其余 `todo!()` | 文件夹 + 注册表里的文件模板，走校验 + effect（M3） |
| 改名 | 仅 stem；目录改名**不更新子孙路径**；同名判重拿 stem 比全名（bug） | 全名编辑 + 校验 + 重新扫描（M3） |
| 删除 | 直接 `remove_file`/`remove_dir_all`，无确认、无回收站 | 回收站 + 确认 + 可见错误（M3，D5） |
| 选中恢复 | 重扫后按 GUID 找回 | 按路径找回，失效则回退最近祖先（M1；跨重启不恢复，D13） |
| 外部变更 | 300ms 去抖 watcher，任何变更全量重扫 | 同样搬进 app，增加 generation 丢弃过期结果（M1） |
| 拖拽 | 内部 `Drag<PathBuf>` + 幽灵图 | 树内移动（M3）；拖到视口另立（§9） |

### 1.3 旧实现的缺陷（这次不要带过来）

| 位置 | 现象 |
|---|---|
| `project_path_tree.rs:139` | `read_dir` 失败静默 `return`：权限不足、路径过长都表现为「文件夹是空的」 |
| `project_path_tree.rs:107` | 每次 refresh `graph.clear()` 重建，所有 `NodeIndex` 失效，只能靠 GUID 抢救选中 |
| `project_path_tree.rs:260` | `find_by_path` 用 `ends_with` 匹配：`models/a.png` 会命中 `res/models/a.png` |
| `project_path_tree.rs:395, 406-410` | 建节点直接做 fs + 弹原生对话框；5 种类型 `todo!()`（panic） |
| `project_path_tree.rs:469-475` | 改名判重拿「新 stem」比「同级全名」，`a.png → b` 时永远判不重 |
| `project_path_tree.rs:528-532` | 目录改名只改自己的 path，子孙节点的 path 保持旧前缀（重扫前全部错） |
| `project_path_tree.rs:566` | `remove_dir_all` 直接删，顺序失败会留下半删状态 |
| `asset_registry.rs:101, 353-361` | `.meta` 与「未导入的源文件」被结构性隐藏：用户看不到自己的图，也看不到为什么看不到 |
| `ui/global_styles.rs:55` | 纹理节点的图标就是它自己（`AssetKind::Texture => node.path.to_str()`） |

### 1.4 新 app 的三个空洞

1. **没有项目概念**：`EditorState` 只有 `layout` + `project_name: String`（`core/state.rs:68-82`），没有 root、没有树、没有选择。
2. **没有 host → state 的回写通道**：action 只从 `UiOut` 来（`app.rs:250-267`），effect 只往 dock/host 走（`effects.rs:37-44`）。扫描结果、watcher、文件操作结果**无处可去**。
3. **没有树控件**：`widgets` 里只有 button/field/menu/splitter/text/...，`TreeView` 还停在设计稿（`widget-layer.md:112`）。`panels/project.rs` 是 39 行的 demo（`:1` 明确写着 "demo body until the asset-browser component is ported"）。

### 1.5 原型里已经跑通、可以直接抄的

- `project_panel.rs`：`folder_paths` / `project_nodes` / `tree_rows` / `actions_from_tree` 四个**纯函数**把「状态 → 行 → action」切开，绘制只剩装配；
- `reduce/project.rs`：`ProjectAction::{CreateAsset, DeleteAsset, RenameAsset}` → 改状态 + 返回 `Effect::PersistProject`；`AssetsDiscovered` 收扫描结果；
- `panel/host.rs` 的 `Panel`/`PanelHost`/`Viewer`（生产侧已落地）。

**不能直接抄的两处**：原型把可见行全量物化（`rows: &[TreeRow]`），10 万行时每帧几 MB 分配；原型用 FNV 哈希做行 id（生产侧可以在建树时算一次）。见 §2.6。

### 1.6 打开方式的现状（2026-11-25 新需求的前置事实）

| 事实 | 证据 | 后果 |
|---|---|---|
| 新编辑器只有一条打开路径：`open::that_detached` 交给 OS 关联；失败写进 Project 状态栏 | `kairos_editor_app/src/project.rs:282-290`、`kairos_editor_core/src/project.rs:224` | 用哪个应用打开由 OS 决定，面板/用户看不到也改不了 |
| `FileType` 表还没有 `open` 字段；打开分发是 M5 的事 | `kairos_editor_core/src/project/file_type.rs:1-14` | 打开策略既不在类型表里，也没有消费方 |
| 旧编辑器有 IDE 嗅探：7 个已知 IDE，先查 PATH 再查平台固定路径，`OnceLock` 缓存，按列表顺序取第一个 | `kairos_editor/src/ui/ide_detection.rs:29-168,195-234` | 行为已验证；但结果不可见、不可改，`OnceLock` 让会话中新安装的 IDE 永远发现不了 |
| 旧编辑器按类型分发：脚本/着色器/文档/TOML/字体走 IDE 嗅探；纹理/网格/材质/音频与未知类型未实现 | `kairos_editor/src/ui/project_window.rs:334-369` | 「有的类型有默认启动方式、有的弹系统窗」是两套隐式规则叠加的结果，不是一张可配置的表 |
| 打开是 `ProjectEffect::Open { root, path }`，app 解释 | `kairos_editor_core/src/project.rs:294`、`kairos_editor_app/src/project.rs:282` | 打开策略只能写死在 app；要可配置，必须先让「用哪个程序」成为 core 可解析的数据 |
| 机器相关的设置没有落点：`Preferences/` 是提交进仓库的样式/字体，`Library/` 是不提交的运行期数据 | `kairos_editor_app/src/layout.rs:41`、`.gitignore:10-13` | IDE 绝对路径不能进仓库；设置文件要和布局文件一样放 `Library/`（P1） |

---

## 2. 目标设计

### 2.1 归属表

| 关注点 | 归属 | 理由 |
|---|---|---|
| 树的形状、排序、可见行、选择/展开/过滤规则 | `core::project` | 纯函数，可单测，可回放 |
| 扫描策略（跳过谁、上限、深度） | `core::project::ScanPolicy`（数据） | 策略是数据，不是 `if`；测试可注入 |
| 遍历文件系统、监听、读/写文件、开进程、读编辑器设置 | `app::project` | core 不许 IO |
| 打开方式设置与解析（嗅探结果 / 用户覆盖 / 未知类型策略） | `core::external_tools`（数据 + 纯解析）+ `app::external_tools`（嗅探、进程、设置文件） | 优先级可单测；启动进程与读写设置是宿主 IO |
| 忽略规则的匹配语义 | `core::project::scan` | 规则是纯函数，配置读取在 app |
| 树/行/重命名编辑器/拖拽 | `component::panels::project` | 读 `Frame`，推 `Action` |
| 行控件的通用外观（缩进、展开三角、选中、hover） | `widgets::tree` | 只懂 `TreeRow` 数据，不懂项目 |
| 面板皮肤 | `component::panels::project::style` | RON，`Preferences/Styles/ProjectStyle.ron` |
| GUID / 资产注册表（编辑器自己发号的那份） | **删除**；资产身份搬进 `.meta` | 见 §2.11 / D4 / D11 |

### 2.2 数据模型（core）

```rust
// core/src/project/tree.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u32);

/// 相对项目目录（cwd）的路径，永远用 '/' 分隔；树根为空串。
/// 是树的**唯一身份**：重扫前后同一个文件得到同一个 key。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind { Folder, File }

pub struct Node {
    pub path: RelPath,         // 唯一真相：`name()` 从它派生最后一段，不重复存一份字符串
    pub kind: NodeKind,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>, // 已排序：目录在前，然后按不区分大小写的名字
    pub row_id: u64,           // FNV-1a(path)，建树时算一次，给 egui 当行 id（跨重扫稳定）
}

impl Node {
    /// 行标签 / 改名编辑器的初值。派生而不是字段：10 万节点省一份字符串，
    /// 也少一处可能与 `path` 不一致的状态。
    pub fn name(&self) -> &str;
}

pub struct ProjectTree {
    nodes: Vec<Node>,
    by_path: HashMap<RelPath, NodeId>,   // 只做查找；有序遍历用不上（孩子有序是 per-node Vec）
    /// 顶层系统目录（当前只有 `Library`）。面板靠 `is_system_dir` 画它前面的分隔线，
    /// M6 的 `In Library` 搜索范围靠 `is_under_system` 判前缀。存根路径而不是给每个
    /// Node 加 bool：后代是否在系统子树里，用前缀判即可。
    system_roots: BTreeSet<RelPath>,
    /// 每次 assemble +1。**不要和 `ScanState::generation` 混**：那是请求号（丢弃旧结果用），
    /// 这是内容修订号（面板行缓存按它失效）。
    revision: u64,
    truncated: bool,
}

impl ProjectTree {
    pub fn revision(&self) -> u64;
    pub fn is_system_dir(&self, path: &RelPath) -> bool;   // 恰好是某个 system root
    pub fn is_under_system(&self, path: &RelPath) -> bool; // 前缀匹配，含自身
}
```

**三个不变量**（写成测试，不是注释）：

1. `nodes[0]` 是唯一的根：`path == ""`、`parent == None`；
2. `children` 有序且都指向更深的节点；`by_path` 与 `nodes` 一一对应；
3. 扫描结果按路径**字节序**给出时，`assemble` 是 O(n) 单趟（父路径一定是子路径的前缀）。
   **签名是 `assemble(entries, &system_dirs)`**——系统目录是 policy 的事实，但排序与
   `system_roots` 要在建树时定死，树本身不该反过来依赖 policy。重复路径（非 UTF-8 名字
   lossy 之后撞车）在这里丢弃并 warn，绝不 panic。

```rust
// core/src/project/scan.rs —— 只有数据，没有 fs
pub struct ScanEntry { pub path: RelPath, pub is_dir: bool }

pub struct ScanPolicy {
    /// 忽略规则（D2）：来自项目根的 `.kairosignore`。
    pub ignore: IgnoreRules,
    /// 系统目录：不受 ignore 影响 + 根级排序固定最后 + 独立预算（当前只有 `Library`）。
    pub system_dirs: Vec<String>,
    /// 系统目录的节点预算，超出不再深入（默认 20_000）。
    pub system_dir_budget: usize,
    pub max_nodes: usize,      // 200_000（不含系统目录预算）
    pub max_depth: u32,        // 64
    pub follow_dir_symlinks: bool, // false（防环）
}

/// 忽略规则只影响「是否进入快照」，不决定「是否显示」。
/// 一条规则可以是目录名（任意深度匹配），也可以是相对路径前缀（含 '/' 时）。
pub struct IgnoreRules { rules: Vec<IgnorePattern> }

impl IgnoreRules {
    /// 引擎内置兜底（`.git` 等点目录、`target`）；`.kairosignore` 存在时以它为准。
    pub fn defaults() -> Self;
    /// 匹配一个目录：名字大小写不敏感（Windows/macOS 语义）。
    pub fn is_ignored(&self, rel: &RelPath, name: &str) -> bool;
}
```

**三件不同的事，别混在一起**（这是 D2 的答案）：

| 概念 | 作用 | 在哪 |
|---|---|---|
| **系统目录（`Library`）** | 正常参与扫描；根级排最后 + 前面一条分隔线；独立节点预算；不受 ignore 影响 | `ScanPolicy::system_dirs` + `ProjectTree::ordered_children` |
| **忽略（ignore）** | 这个目录**根本不进快照**：不扫、不显示、不搜索 | `ScanPolicy::ignore`（`.kairosignore`） |
| **隐藏（hidden）** | 进了快照，默认不画（点文件、`.meta` 伴生） | `ProjectView::show_hidden` + `FileType::companion` |

**`Library` 只是项目里的一个目录**（D2 修正：不做第二个根）。它有三条特殊待遇，全部收敛在 `core::project`：

1. 根级排序固定最后：`ordered_children` 对顶层用 `(is_system, !is_dir, lowercase_name)` 排序，面板看到 `is_system_dir` 就在它前面画 1px 分隔线；
2. 不受忽略规则影响（用户写 `*` 也不该让它消失）；
3. 独立节点预算（默认 2 万，超出不再深入并标 `truncated`），避免 `Library/Logs` 这类产物吃掉主扫描的预算。

**成品根会搬进 `Library/`（M0.5，D12）**：`imported_assets/Default` 变为 `Library/imported_assets/Default`，于是扫描策略里**不再需要任何"跳过成品根"的字段**——它天然落在 `Library` 这个系统目录的预算里。这就是"先搬、以后不重构"的收益：`ScanPolicy` 少一个概念，面板少一条特例。

**忽略规则从哪来（D10 修正：一开始就上 `.kairosignore`）**：项目根的 `.kairosignore`，gitignore 风格子集——

- 一行一条，`#` 开头为注释，空行忽略；
- 不含 `/` 的模式匹配**任意层级的名字**（如 `target`）；含 `/` 的模式从项目根算**路径前缀**（如 `res/generated`）；结尾 `/` 表示只匹配目录；
- 每段支持 `*`/`?`；**v1 不支持 `!` 取反**（要放开某个匹配，改规则本身）；
- 文件缺失时用内置兜底（`.*` 点目录、`target`）并在首次打开时**就地补写一份带注释的默认文件**（与 `Preferences/Styles/*.ron` 的 seed 行为一致），这样规则只有一处真相、也方便提交进版本库；
- 与「显示隐藏文件」无关：被忽略的目录连扫都不扫。

```rust
// core/src/project/view.rs —— 「用户意图」，运行期状态（不落盘，D13）；本帧才有效的放面板里
pub struct ProjectView {
    pub expanded: BTreeSet<RelPath>,
    pub active: Option<RelPath>,        // 面包屑 / 内容区当前目录
    pub selection: BTreeSet<RelPath>,   // M3 多选
    pub anchor: Option<RelPath>,        // 选择手势的锚点（Shift 区间/键盘起点），随 selection 一起修剪与重映射
    pub sort: SortKey,                  // v1 只有 Name
    pub show_hidden: bool,              // 点文件与 .meta 伴生
    /// 左右两栏的分割比例（D1）：右栏从第一天就存在，只是先空着。
    pub tree_split: f32,                // 0.15..=0.6，默认 0.28；reducer 与面板都再收一次像素下限
}

// core/src/project/mod.rs
pub struct ProjectState {
    pub name: String,          // 面板标题/顶栏（替换 project_name）
    pub root: PathBuf,         // 绝对路径或相对 cwd；只在 IO 边界用
    /// 扫描策略：忽略规则 + 系统目录 + 上限。读 `.kairosignore` 是 app 的 IO，
    /// 但策略本身是 core 的数据——app 读到之后发 `ProjectAction::PolicyLoaded` 写回来，
    /// reducer 再从 state 里把它放进 `ProjectEffect::Scan`（M1）。
    pub policy: ScanPolicy,
    pub tree: ProjectTree,
    pub view: ProjectView,
    pub scan: ScanState,
    pub error: Option<ProjectError>, // 最近一次可见失败（M1 起由 `Scanned.errors` 写入）
}

pub struct ScanState {
    pub generation: u64,               // 递增；旧结果直接丢
    pub running: bool,
    pub last_ms: u64,
    pub truncated: bool,
    /// 重扫后要落到的路径（改名/新建后用）
    pub focus_after_scan: Option<RelPath>,
}
```

### 2.3 扫描协议

```
boot: ProjectService::new(root) 读/seed .kairosignore → Action::Project(PolicyLoaded { policy })
      顺序写死在 EditorApp::new：PolicyLoaded 与 ScanRequested{Startup} 按这个顺序进 inbox
UI / watcher / 文件操作完成 / 手动刷新
        │  Action::Project(ScanRequested { reason, focus })
        ▼
reduce: generation += 1; scan.running = true
        │  Effect::Project(Scan { root, policy: state.project.policy.clone(), generation })
        ▼
app::ProjectService: 后台线程 DFS（单根：忽略规则 + 深度 + 双预算）
        │   · 主预算 max_nodes：系统目录之外的节点
        │   · 系统目录预算：进入 Library 时切换，超出不再深入并标 truncated
        │   · 每进一个目录查一次 cancel token（watcher 连发时旧扫描尽快退出）
        │  线程结束 → 有序 Vec<ScanEntry>（按路径字节序）
        │            + errors（read_dir 失败的目录，按目录聚合，上限 8 条）
        ▼
Action::Project(Scanned { generation, entries, truncated, elapsed_ms, errors })
        │
reduce: generation 不匹配 → 丢弃；匹配 → tree.assemble(entries, &policy.system_dirs)
        → 剪掉失效的 expanded/selection，active 回退最近祖先 → errors 落进 state.project.error
```

**为什么是「扫描只走目录、组装在 core」**：旧实现把 `analyse_path`、GUID 分配、`.meta` 配对全塞在遍历里，于是每条规则都只能靠改遍历代码来测。拆开后扫描器是一个 20 行 DFS，规则全是纯函数测试。

**为什么是全量**（不做懒加载）：懒加载 + 搜索 + 计数三者互相打架，且 v1 的规模（≤ 5 万）全量扫 < 100ms。预留触发条件：监控到 > 20 万节点或网络盘 > 1s，再引入「展开时扫描」（见 §5）。

### 2.4 host → state 回写（新接缝，M0）

这是新 app 目前缺的那一层，也是后面所有异步功能的地基：

```rust
// app/src/app.rs
pub struct EditorApp {
    // ...
    /// host → state 的入口。扫描结果、watcher、文件操作结果、持久化失败都进这里。
    inbox: VecDeque<Action>,
}

// app/src/host.rs::render —— 一个引擎帧一次，在 egui pass 之外
self.app.begin_frame(&self.egui_ctx);
//   1. ProjectService::poll() → inbox（上一帧以来完成的宿主工作）
//   2. inbox 排空 → reduce → pending_effects
//   3. 实例/tab 对账（PanelHost::sync + dock）
//   4. 执行 pending_effects；产出的 action 进 inbox，**下一帧**才被 apply
let out = self.egui_ctx.run_ui(raw_input, |ui| self.app.ui(engine, ui)); // 只画
// committed pass 的 UI action 在 ui() 内 pump（effect 下一帧执行）
```

M0 落地时把宿主阶段从 `ui()` 里挪到了 `begin_frame`，并由 host 层在 `run_ui` 之前调用一次：egui 一帧可能跑多遍 pass（还可能丢弃），宿主工作不能跟着 pass 的次数走。

副作用执行器拿到服务：

```rust
// app/src/effects.rs
pub(crate) struct EffectExecutor<'a> {
    host: &'a mut PanelHost,
    dock: &'a mut DockState<PanelInstanceId>,
    shared: &'a mut SharedStyles,
    project: &'a mut ProjectService,   // 新增
    now: Instant,                      // 新增：持久化去抖用
}

impl<'a> EffectExecutor<'a> {
    /// 返回本次 effect 产生的 action（进 inbox），而不是直接改 EditorState。
    pub(crate) fn apply(&mut self, effect: Effect, ctx: &Context) -> Vec<Action> {
        // ...
    }
}
```

**规则**：effect 可以**产生** action（扫描完成、文件操作完成），但绝不修改 `EditorState`。这些 action 是**下一帧**的输入，不在执行 effect 的那一帧回灌 reducer：跨帧边界切断了 action↔effect 环，因此宿主阶段不需要轮数预算。这与现有 `Effect::Style` 的「执行器只动宿主资源」一致。

### 2.5 变更协议（M3）

「请求 / 完成」分开命名，和原型一致：

```rust
pub enum ProjectAction {
    // 查询与视图
    ScanRequested { reason: ScanReason, focus: Option<RelPath> },
    Scanned { generation: u64, entries: Vec<ScanEntry>, truncated: bool, elapsed_ms: u64, errors: Vec<String> },
    // watcher 不单设变体：外部变更就是 ScanRequested { reason: ExternalChange, focus: None }
    Expand { path: RelPath },
    Collapse { path: RelPath },
    Navigate { path: RelPath },            // 面包屑/双击目录
    Select { paths: Vec<RelPath>, mode: SelectMode },
    SetSort(SortKey),
    SetShowHidden(bool),
    PolicyLoaded { policy: ScanPolicy },   // boot 读 .kairosignore 之后；改规则时也走这条

    // 变更（请求 → effect → 完成）
    CreateRequested { parent: RelPath, name: String, kind: CreateKind },
    RenameRequested { path: RelPath, name: String },
    MoveRequested { paths: Vec<RelPath>, target: RelPath },
    DeleteRequested { paths: Vec<RelPath> },
    FsCompleted { op: FsOpId, result: Result<FsOutcome, String> },

    // 打开（M5a：reducer 先按 §2.14 解析出 ResolvedTarget）
    OpenRequested { path: RelPath },
    RevealRequested { path: RelPath },
}

pub enum ProjectEffect {
    Scan { root: PathBuf, policy: ScanPolicy, generation: u64 },
    Fs(FsOp),                                            // create/rename/move/trash
    Open { root: PathBuf, path: RelPath, target: ResolvedTarget },
    Reveal { root: PathBuf, path: RelPath },
}

pub struct FsOp { pub id: FsOpId, pub root: PathBuf, pub companions: Vec<&'static str>, pub kind: FsOpKind }
pub enum FsOpKind {
    Create { parent: RelPath, name: String, kind: CreateKind },
    Rename { from: RelPath, to: RelPath },
    Move { from: Vec<RelPath>, to: RelPath },
    Trash { paths: Vec<RelPath> },
}

// M3 定稿 T14/T15：语义来自类型表，不写死在 fs 层
pub enum CreateKind {
    Folder,
    File { extension: &'static str, contents: &'static str },   // 来自 CreateEntry
}

// M3 定稿 T6：完成必须带「改了什么」，否则 reducer 无法 focus、也无法重映射前缀
pub struct FsOpId(pub u64);
pub struct FsOutcome {
    pub created: Option<RelPath>,
    /// 每个被改名的条目一对；rename 一对、move N 对（reducer 按对重写前缀）
    pub moved: Vec<(RelPath, RelPath)>,
    /// 进回收站的条目（M3a 落地）；trash 没有后继路径可重写，
    /// 重扫时由 settle 把它从 selection 剪掉、active 回退到最近的存活祖先
    pub trashed: Vec<RelPath>,
}
```

**reducer 里的关键规则**（都要有测试）：

- `Scanned` 的 generation 不匹配 → 整条丢弃（旧结果绝不覆盖新树）；
- 重扫后 `expanded`/`selection` 里不存在的路径被剪掉；`active` 失效则回退到最近的仍存在的祖先；
- `RenameRequested` 校验通过后**不改树**，只发 effect；完成 → `ScanRequested { focus: Some(新路径) }`。乐观改树会与 watcher 打架，而扫描很便宜；
- `Select { paths, mode }` 三种 `SelectMode`：`Replace` / `Toggle`（Ctrl）/ `Range`（Shift，`paths` 恰为 `[anchor, focus]`，区间由 reducer 用 `visible_rows` 算）；**只有单路径的 `Replace` 移动 `active`**（M3 定稿 T5；多路径 Replace 是 Ctrl+A 这类选择手势，不是导航，M3b 落地时补的细化）；**anchor 存在 core 的 `ProjectView` 里**（M3b 验收修正 #7）：每次点击/键盘移动定 anchor、Shift 区间不移动它、创建/改名成功后跟到新行、删除/外改由 `settle` 修剪、改名/移动按前缀重映射——面板不再自己记 anchor，所以关标签页再打开和一次重扫都不会让 Shift+点击从过期行开始；`Range` 的 anchor 被折叠隐藏时退化为「选中 focus 并重新 anchor」；
- 所有 `*Requested` 在入口做**同一套名字校验**（`project/name.rs`），失败返回 `error`，不发 effect。

**名字校验**（纯函数，一张表）：空名、`<>:"/\|?*`、控制字符、Windows 保留名（`CON`/`PRN`/`AUX`/`NUL`/`COM1..9`/`LPT1..9`）、结尾点或空格、超长（组件 255 字节）、同级大小写不敏感重名、改名为自身、根节点不可改不可删。每条一个测试。

### 2.6 面板 UI（M2）

D1 定稿：**左右两栏从第一天就存在**，右栏先只有面包屑 + 占位，网格/缩略图在 M9 补内容，布局本身不返工。

```
┌ Project ─────────────────────────────────────────────────────────────┐
│ 折叠全部 │ 刷新 │                                 │ ◉ │ 工具栏 20px
├──────────────────────────────┬───────────────────────────────────────┤
│ ▾ res                        │ KairosEngine / res / models           │ ← 内容头 20px
│   ▾ models                   │                                       │   （独立灰底 + 下边线）
│     ▣ Suzanne          png   │            （内容区占位）              │ 分割条可拖
│     ▣ readme           md    │        网格视图在 M9 填充               │
│   ▸ shaders                  │                                       │
│ ▸ scripts                    │                                       │
│ ──────────────────────────── │                                       │ ← 分组线（不可拖）
│ ▸ Library                    │                                       │ 固定最下、默认折叠
├──────────────────────────────┼───────────────────────────────────────┤
│ 12 items · truncated         │                  （图标大小滑条 M9）    │ 状态栏 18px
└──────────────────────────────┴───────────────────────────────────────┘
```

- **工具栏 M2 只放真能用的三项**：`折叠全部` / `刷新` 两个文字段 + **隐藏文件的眼睛开关**（新建 ▾ / 删除等 M3 有真实动作再加，不做灰按钮占位）；每个控件后画 1 px chrome 线（Console/Profiler 工具栏同一规则），眼睛前另加一条——没有分割线，文字按钮读起来就是标签。眼睛是 Unity 的 scene-visibility eye（开 = 显示隐藏项、叉 = 过滤），图标本身即状态，**不加计数文本**：计数会随目录变、M6 的搜索框要占这条横带，tooltip 写清动作就够；
- 分割条用现成的 `widgets::splitter`：拖拽中的宽度**面板本地持有**（`Option<f32>`，`None` = 跟随 `ProjectView::tree_split`），松手只发一条 `SetTreeSplit`；两边宽度进 `tree_split`（M4 写入布局文件）；
- **`Library` 固定排最后**（D2）：模型里它就是顶层的一个普通目录，只是 `ordered_children` 把它排到最后、面板在它前面画一条 1px 分隔线；默认折叠，展开状态同样进 `expanded`；不受忽略规则影响，但有独立节点预算；
- 搜索**不进入 `Library`**：里面是引擎产物，搜出来只会是噪音（要查导入产物请直接展开它）；
- 右栏 v1 是「内容头 + 空状态」：内容头是**独立灰底**（`header_bg`）的 20 px 条 + 1 px 下边线，面包屑画在里面、每一级可点 = `Navigate`；显示目录 = `view.active`，而**单击树行就会更新它**（文件夹 → 自身，文件 → 父目录：`ProjectTree::content_dir`），双击只负责展开/打开，所以内容头不再等双击。空状态写明下一阶段做网格；
- 状态栏按两栏**分割**（分割线在列分界处，用 splitter 的 idle 色——暗色皮肤下 `status_bg` 与 `separator` 同色，用 `separator` 会看不见）：左半边（Hierarchy）是当前 `active` 目录的直接子项数 + `truncated` 告警；右半边（Content）留给 M9 的图标大小滑条，现在只在报错时显示 error。**选中路径不再显示**：内容头已经跟着单击走，重复一遍没有信息量。

**widget 契约**（`widgets/src/tree.rs`）：

```rust
// widgets/src/tree.rs —— M2 定稿后的契约（M2a 实现它）
pub struct TreeStyle {
    pub row_height: f32,        // 20
    pub indent: f32,            // 14
    pub expander: f32,          // 13：三角的命中框（图形本身 8x8）
    pub icon: f32,              // 18
    pub text_size: f32,         // 12
    pub suffix_size: f32,       // 11
    pub text: Color32,
    pub suffix: Color32,
    pub selected_bg: Color32,
    pub selected_radius: u8,    // 4
    pub hover_bg: Color32,
    pub expander_color: Color32,
    pub separator: Color32,
    pub separator_width: f32,   // 1
    pub bg: Color32,
}
impl TreeStyle {
    pub fn dark() -> Self;
    pub fn light() -> Self;
}

pub struct TreeRow<'a> {
    pub depth: u16,
    pub label: &'a str,
    pub suffix: Option<&'a str>,
    pub icon: Option<&'a str>,      // Preferences/Textures 下的相对路径
    pub expander: Option<Expander>, // None = 文件；三角是独立命中区
    pub selected: bool,
    pub separator_above: bool,      // 系统目录（Library）行上方的 1px
}

pub struct TreeOutput<Id> {
    pub clicked: Option<Id>,          // 单击行 = 选中
    pub double_clicked: Option<Id>,   // 双击行 = 打开
    pub expander_toggled: Option<Id>, // 三角 = 展开/收起（与 clicked 互斥）
    pub hovered: Option<Id>,
    pub focused: bool,                // 树有键盘焦点，面板据此吃方向键
}

impl<'a, Id: Copy + Eq + Hash> TreeView<'a, Id> {
    /// rows 只给 id（前序）；行内容按需回调，只对可见行调用。
    pub fn new(
        id_salt: impl Hash,
        rows: &'a [Id],
        style: &'a TreeStyle,
        row: impl Fn(Id) -> TreeRow<'a> + 'a,
    ) -> Self;
    /// 把这一行滚进可视区（键盘移动选中项、reveal 之后）。
    pub fn scroll_to(self, id: Id) -> Self;
    pub fn show(self, ui: &mut egui::Ui) -> TreeOutput<Id>;
}
```

**M3 扩展（已落地，T1/T2/T3）**：`TreeOutput` 再增 `secondary_clicked` / `secondary_pos`、`drag_started`、`dragging`、`drop_target`、`drag_stopped`、`edit_committed` / `edit_cancelled`（derive 因此从 `Eq` 降为 `PartialEq`，因为 `Pos2` 不是 `Eq`）；`TreeView` 增 `highlight(id)`（行底色 + 描边）与 `highlight_region(id)`（文件夹行 + 可见子树的整块区域框，M3b 验收修正 #2）、`editing(id, &mut String)`；`TreeStyle` 增 `drop_fill` / `drop_stroke`（`ProjectStyle` 与 `ProjectStyle.ron` 同步）；`TreeRow` 增 `thumbnail: Option<TextureId>`（宿主缩略图，M3b 验收修正 #6）。拖拽是**树级会话**存在 egui memory（M3b 验收修正 #3）：源行被自动滚动移出可见范围后，松手仍然报 `drag_stopped`；拖拽贴边时 widget 自己滚 ScrollArea，并翻译被 egui 在拖拽期间忽略的滚轮增量。widget 只报交互；payload / 幽灵 / 落点规则与 buffer / 校验 / 错误文案仍归面板。

三处必须和原型不同（否则到了 2 万行会痛）：

1. **只对可见行回调**：`ScrollArea::show_rows` 给可见区间，widget 只对区间内行调用 `row(id)`。原型把所有行物化成 `Vec<TreeRow>`，全展开时每帧 MB 级分配。
2. **行 id 来自 `Node.row_id`**（建树时算一次），不在绘制期做哈希。
3. **不做 `PanelChrome`**：标题栏属于 dock（`Panel` trait 只管 body，见 `panel.rs:77`），面板 body 从工具栏开始 —— 和 Console 一致。

**面板自己持有的视图状态**（关掉标签页就丢，这是对的）：搜索框文本、重命名 buffer、删除确认、拖拽中的路径、滚动定位。**core 持有**：`expanded`/`active`/`selection`/`sort`/`show_hidden`。判据：**关掉面板再打开还该在的 → core；只影响这一帧的 → 面板**。这条规则有硬理由：`PanelHost::sync` 的 `retain` 会直接 drop 关闭的面板实例（`panel/host.rs:52`），面板里存不住任何东西。滚动定位因此是面板态，但**重开标签页必须补回来**：新实例首帧把 `selection`（没有则 `active`）交给 `TreeView::scroll_to`，否则状态还在、视口却回到顶部。

**绘制**：`TreeView` 只画行；右键菜单（`menu::context_menu` + `submenu` + `menu_item_enabled`）与确认弹窗（`widgets::modal`）由面板画 —— 面板有 `&mut Ui` 和 `&mut UiOut`，widget 没有 `Action` 的概念。**弹窗只由 Esc 关闭**（点背板不消失，M3b 验收修正 #1）：确认框是对用户停下来问的问题，不能被一次误点当成「否」。**行内重命名例外**（M3 定稿 T3）：它必须落在行内几何上，所以由 widget 画（`TreeView::editing(id, &mut String)`），buffer、校验和错误文案仍归面板。**行显示名不含扩展名**：文件行画主名（stem），扩展名是右侧后缀列；改名编辑器也只编辑主名，提交时面板把原后缀拼回去 —— 用户拍板「文件名不要展示后缀，避免重命名时把后缀给改了」。

**交互规则（M2 定稿，照 Unity/Tuanjie）**：一行 = 三角（独立命中区）+ 图标 + 名字 + 后缀。**单击选中并移动内容头**（写 `selection`，同时 `active` = 命中项的目录：文件夹 → 自身，文件 → 父目录；不展开、不导航）、**双击打开**（文件夹 → `Navigate`，文件 → `OpenRequested`）、**三角只切换展开**（不选中、不导航）。键盘只在 `TreeOutput::focused` 为真时生效：↑↓ 移动选中、Home/End 首尾、`←` 已展开则折叠否则选父、`→` 未展开则展开否则选第一个子、Enter 打开。重命名/拖拽/右键菜单的通道 **M3 才加**，M2 不预留空变体（见 M2 定稿 N3）。

**视觉来源（N5）**：行图标取自本机 Tuanjie 编辑器资源包，Lanczos 缩到 64² 提交在 `Preferences/Textures/project/{,light/}`（`folder.png` / `folder_open.png` / `file.png`），重跑脚本 `scripts/extract_tuanjie_icons.py`。M2 文件行统一用泛型 `file` 图标；按类型的图标（texture/mesh/material/audio/...）在 M5 从同一个包抽。工具栏的隐藏文件开关也用同一个包：`d_scenevis_visible@2x` / `d_scenevis_hidden@2x`（light 用无 `d_` 版），32²（16 px 的 2×）。

### 2.7 持久化（M4，D13）

持久化**只存编辑器布局**：打开的面板、dock 停靠树（分割方向/比例）、每个 leaf 的 tab 顺序与当前 tab、焦点、浮动窗口（位置/尺寸/最小化/最大化），外加 Project 面板左右比例。**不存**：面板会话态（展开/当前目录/选中/anchor/滚动/搜索词）、项目树、GUID、忽略规则（编辑器设置，见 §2.2）、World/Scene 文档（未来的会话能力）。

```
Library/editor_layout.toml —— 可删、可重建；在项目根下，不进 git
version = 1
focus = "scene"

# 主 surface：前序（preorder）节点表；split 的两个子节点依次跟在本节点后面
[[main.nodes]]
kind = "horizontal"
fraction = 0.78

[[main.nodes]]
kind = "leaf"
tabs = ["scene"]
active = 0

[[main.nodes]]
kind = "leaf"
tabs = ["inspector", "hierarchy"]
active = 0

# 浮动窗口：位置/尺寸恢复后由 egui 的 constrain_to 保证回到可视区
[[windows]]
position = [1180.0, 80.0]
size = [480.0, 360.0]
minimized = false
maximized = false
[[windows.nodes]]
kind = "leaf"
tabs = ["console"]
active = 0

[panels.project]
tree_split = 0.28
```

- **身份**：tab 里存稳定的面板 kind 字符串（`PanelId` 的 `&'static str`）。当前每种面板只有一个实例；将来多实例（`viewport#0`）再引入 `kind#serial`，文件里的字符串位已留好。
- **写**：`EditorApp::begin_frame` 对 dock 取「可持久化快照」（不含每帧重算的 rect/viewport/scroll），与上一帧比较，变化后 **500ms 去抖** → 临时文件 + `rename` 原子替换；`Layer::on_exit` 再 flush 一次。失败只 warn（用户偏好数据，不进状态栏）。
- **读**：boot 时读一次。面板 kind 不认识 / 版本不认识 / 解析失败 / 节点形状非法 → 整份忽略 + warn，退回 `default_dock`，绝不阻塞编辑器；合法但为空（用户关掉了所有面板）→ 尊重它，工具栏 Window 菜单可以重开。
- **恢复顺序**：由文件里的 tab 推导要开的面板集合（唯一真值是 dock 树，不另存一份 open 列表）→ `PanelHost::sync` 建实例 → 用 live 实例重建 `DockState` → 文件里没有、但 layout 要求打开的面板追加到当前 leaf。浮动窗口坐标交给 egui `constrain_to`，换显示器后不会跑到屏外。
- **不设 root 守卫**：布局文件里没有项目路径，项目目录被拷贝/移动后继续用同一份布局是无害的；它就在项目根的 `Library/` 下，天然按根键化（D9）。
- **为什么触发在 host 侧而不是 `Effect::PersistLayout`**：dock 的拖 tab / 分割 / 浮动窗口在 `Docker::show` 内部直接改 `DockState`，不产生 action（只有关闭走 viewer 回调）；core 的 `LayoutState` 只存 open/focus。布局树本来就是 host 状态，所以由 app 自己取快照，不经过 reducer。
- **External Tools 设置不在这份文件里**：打开方式是编辑器设置而不是布局；v1 存 `<root>/Library/external_tools.toml`，同样可删、不进 git（§2.14，P1）。
- **为什么 TOML、为什么不是 sqlite**：文件 KB 级、要人手可读可 diff；树的查询都在内存里。**项目树本身永不落盘**——它靠重扫重建，见 §2.7.1/§2.7.2 的分析。

#### 2.7.1 为什么项目树不落盘：`asset_registry.toml` 是什么，不是什么

这是 D3（不持久化项目树）里最需要说清的一点。旧注册表（`GUID ↔ 路径`）**不是用来加速重建树的**，它一条 `read_dir` 也省不掉：

- 旧 `ProjectPathGraph::new_at_root` 每次都完整递归遍历目录树；registry 只是在遍历途中给每个路径查/配一个 UUID（HashMap 查找），**遍历成本一分不少**；
- 它解决的唯一问题是**身份**：重扫会 `graph.clear()` 重建全部 `NodeIndex`（`project_path_tree.rs:107`），选中项只能靠 GUID 找回；
- 而且它有额外成本：每次刷新要维护两张 HashMap，每次变更要重写整份 TOML，写进 `Library/` 还会反过来叫醒 watcher（旧代码为此专门加了「路径数没变就不落盘」的判断，`asset_registry.rs:305-312`）。

所以：**对树而言（D4），这份文件没有剩余价值**——删掉它不会让启动变慢，也不会让重扫变慢。注意这里说的只是「树的身份」；**资产身份与引用**是另一件事，见 §2.11。

#### 2.7.2 真正能加速重建的是什么（按需，M1 之后定）

决定重建成本的是**遍历 + stat 次数**，不是身份表。可选手段按代价排序：

| 手段 | 收益 | 代价 | 何时做 |
|---|---|---|---|
| 后台线程（M1 已做） | UI 不卡 | 小 | **默认做** |
| 忽略目录（D2） | 直接少遍历一大片 | 小 | **默认做** |
| 懒加载（展开才扫子目录） | 首帧只扫一层 | 中：与搜索/计数冲突 | 实测首屏 > 300ms 再做 |
| 索引缓存（磁盘存 path+mtime，启动时校验） | 大工程首屏快 | 大：失效语义、跨机器、写盘时机 | 实测 > 1s 且懒加载不够时再做 |

如果真到第 4 行，它的形状是明确的：`Library/project_index.toml`（或 sqlite）存 `path + mtime + size`，**校验失败的条目按目录整体回退为真实扫描**，删掉文件必须能当没做过 —— 且它属于「索引」不属于「身份」，和 D4 无关。**M1 结束先量一次**（启动扫描耗时、5 万/20 万节点），数据说话。

**为什么 TOML 不是 sqlite**：数据量是 KB 级；要人手看得懂、能进 git diff、能拿 diff 判断回归；sqlite 要引入 `rusqlite`/`libsqlite3-sys`（跨平台构建成本），换不来任何查询能力 —— 树的查询都在内存里。**什么时候改用 sqlite**：真的要做「全项目文件内容全文搜索」时，那是独立立项（索引、忽略规则、增量更新），不是面板状态。

### 2.8 类型 seam（M5）

```rust
// core/src/project/file_type.rs —— 纯数据（**M3 已落地**；open 按 §2.14 在 M5a 落地、referenceable 留 M7）
pub struct FileType {
    pub id: &'static str,             // "texture" | "mesh" | ...
    pub label: &'static str,          // 行尾后缀，如 "png" / "mat"
    pub extensions: &'static [&'static str],
    pub icon: &'static str,           // 文件名（如 texture.png），根目录由皮肤给（M3/T16）
    pub companion: bool,              // .meta/.uid 之类：默认隐藏
    pub referenceable: bool,          // 可被别的资产引用 → M7 会给它发 guid
    pub delete_confirm: bool,         // D5：默认 true（Unity 行为），按类型可关
    pub open: OpenDefault,            // 未覆盖时的默认策略（嗅探 / 系统 / 内部视图）；见 §2.14
}

/// 默认打开策略（§2.14.3）：类型表的数据，不是用户设置。
pub enum OpenDefault {
    /// 代码/文档：嗅探第一个可用 IDE；检测不到就落到 fallback。
    Sniffed(OpenFallback),
    /// 系统关联 / 系统「打开方式」。
    System(OpenFallback),
    /// 编辑器内部视图（Inspector/Preview），随 M5 每类任务落地。
    Internal(InternalOpen),
}
pub enum OpenFallback { SystemDefault, SystemPicker }

pub struct FileTypeRegistry { types: Vec<FileType>, by_ext: HashMap<String, usize> }
impl FileTypeRegistry {
    pub fn lookup(&self, name: &str) -> Option<&FileType>; // 大小写不敏感，最长扩展名优先
    pub fn is_companion(&self, name: &str) -> bool;
}
```

- 默认表 = 现存的 8 个家族（png / glb / mat / audio / wgsl / rs / md / ttf+otf）+ `meta` 伴生 + fallback；
- 图标走 T16 的类型图标表（`FileType.icon` 文件名 + 皮肤给根目录）；旧 `Preferences/Textures/*_icon.png` 不再作为行图标来源；脚本（`.rs`）的 Rust 图标是项目自绘，见 §2.15；
- 加一个类型的代价 = 表里一行 + 一个打开处理器 + 测试。**验收方式**：用测试替身注册一个假类型，断言树、行、打开分发都不需要改 —— 如果改了三处，说明 seam 没做对；
- 引擎侧的 loader 注册表（`AssetLoader::extensions`）是未来的第二真相源，v1 不接（core 不能依赖 `kairos_asset`，要接就经 app 注入）；
- **打开模型（2026-11-25 新需求）**：原文的 `open: OpenWith` 拆成 `OpenDefault`（类型表默认）+ `OpenTarget`（用户设置）+ `ResolvedTarget`（effect 载荷），详见 §2.14.3。

### 2.9 搜索（M6）

**D7 定稿：Unity 风格的过滤语法**（`t:` 类型过滤），不是「一个输入框只做子串匹配」。

语法（`core/src/project/search.rs` 的纯解析器，可单测）：

| 写法 | 含义 |
|---|---|
| `readme` | 名字子串匹配（大小写不敏感）；含 `/` 时改为相对路径子串匹配 |
| `t:material` / `t:mat` / `t:mat,pptx` | 按类型过滤；认 `FileType::id`、`label`、任一扩展名，逗号 = 或 |
| `t:texture -t:meta` | 逐词 AND；`-` 前缀 = 排除 |
| `p:res/models` | 只在路径里匹配（名字不算） |

- **搜索范围（Unity 同款）**：搜索框左侧一个范围选择器，`In Assets`（默认）= 只搜普通目录，`In Library` = 只搜 `Library`；对应 `SearchScope::{Assets, Library}`，属于面板视图状态；
- 空查询 = 不过滤；命中项的祖先自动展开（`In Library` 时 Library 根自动展开），`Esc` 恢复过滤前的展开集合；`Ctrl+F` 聚焦搜索框；
- 解析结果 `Vec<Term>` 与 `SearchScope` 一起放进面板视图状态（不进 core 的 `ProjectView`：它们是本帧视图，不是持久意图）；`visible_rows` 接收 `(&SearchQuery, SearchScope)`；
- 面板缓存 `Vec<NodeId>`（M2 定稿 N2：缓存在面板，core 只给纯函数），失效条件 = `(tree.revision, expanded, show_hidden, query, scope)`；
- 性能：O(n) 单趟（10 万节点 ~1ms），无索引、无去抖；解析只做一次（查询字符串没变就不重解析）；
- **不做内容搜索**：需要读文件、忽略规则、后台索引、结果列表（「跳到第 N 行」），是另一个面板的活，单独立项 + ADR。

### 2.10 模块布局

```
kairos_editor_core/src/
  project/mod.rs           ProjectState、ProjectAction、ProjectEffect 的再导出
  project/tree.rs          NodeId / RelPath / Node / ProjectTree / visible_rows
  project/scan.rs          ScanEntry / ScanPolicy / ScanReason（只有数据）
  project/view.rs          ProjectView / SortKey / SelectMode
  project/search.rs        查询解析 + 匹配（M6，纯函数）
  project/file_type.rs     FileTypeRegistry + CreateEntry（M3 数据表；OpenDefault M5a、guid M7）
  project/name.rs          名字校验（M3）
  external_tools.rs        OpenSettings / OpenTarget / effective_open（§2.14，M5a）
  reduce/project.rs        reducer（新增 slice，注册到 reduce.rs）
  reduce/external_tools.rs external_tools 切片 reducer（M5a）
  tests/project_*.rs

kairos_editor_app/src/
  project/mod.rs           ProjectService（poll / execute）
  project/scanner.rs       后台扫描线程 + generation
  project/watcher.rs       从 kairos_editor/src/project_watcher.rs 搬来（notify-debouncer-full）
  project/fs.rs            建/改名/移动/回收站，错误转字符串
  layout.rs                编辑器布局文件（`Library/editor_layout.toml`）读写 + 去抖 + 原子写 + 启动恢复
  project/ignore.rs        .kairosignore 解析（缺文件 seed 默认） + 规则匹配入口
  project/identity.rs      guid ↔ path 索引（读侧车、可重建缓存，M7）
  external_tools/mod.rs    ExternalToolsService：boot 读设置 / poll 检测与浏览结果 / 执行 launch 与 persist
  external_tools/detect.rs IDE 嗅探（从 `kairos_editor/src/ui/ide_detection.rs` 搬，工作线程，Windows 不闪控制台）
  external_tools/settings.rs `Library/external_tools.toml` DTO + 原子写 + 版本/大小校验
  external_tools/launch.rs SystemDefault / SystemPicker / Program 三条执行路径

kairos_editor_component/src/
  panels/project.rs        装配：工具栏 + [树 | 面包屑+占位] + 状态栏 + 弹层
  panels/project/style.rs  ProjectStyle（RON，dark/light）
  panels/project/rows.rs   core 状态 → TreeRow 的纯映射 + 行缓存
  panels/project/content.rs 右栏占位（M9 换成网格，接口先定）
  panels/external_tools.rs  面板：检测列表 + 每类型打开方式表 + 未知类型开关 + 错误行（M5b）
  panels/external_tools/style.rs ExternalToolsPanelStyle（RON，dark/light）

  kairos_editor_widgets/src/tree.rs   TreeView / TreeStyle / TreeRow / TreeOutput

  kairos_editor_docking/src/{tree,docker}.rs   布局快照/恢复最小 API（`Tree::from_nodes` / `DockState::from_parts` / `DockState::surfaces`）
```

新依赖（只进 app）：`notify-debouncer-full`、`toml`、`serde`、`trash`、`open`；外加 `native-dialog = "0.9.6"`（Browse 用，已在 workspace 锁文件里，旧编辑器同款）—— 其余版本与 `kairos_editor/Cargo.toml:39-49` 现有条目一致。M7 不引入新依赖：`kairos_asset` 已经依赖 `uuid`（`kairos_asset/Cargo.toml`）。

**M0.5 会改到的 crate**：`kairos_asset`（成品根常量、processor 日志路径）、`kairos_engine`（`kairos_paths`/`kairos_game` 的加载字面量）、`kairos_graphics`（白纹理字面量）——只改常量与字面量，不改行为。

**M7 会改到的 crate（本计划唯一的跨层改动）**：`kairos_asset`（`.meta` 增加 `guid` + 读写 API）、`kairos_graphics`（材质引用变 `AssetRef`，向后兼容旧 `.mat`）、`kairos_editor_app`（索引）、`kairos_editor_component`（Inspector 三态显示）。

### 2.11 资源身份与引用维护（D4）

先把话说准：**旧 `asset_registry` 确实是 guid ↔ path 索引**，我的意思只是「它不加速重建树」（§2.7.1）。真正要决定的是：**资产的持久身份放哪、引用怎么维护**。

#### 2.11.1 现状：Kairos 的引用是纯路径

| 引用场景 | 现在怎么存 | 依据 |
|---|---|---|
| 运行时加载资产 | `AssetPath`（路径）→ `AssetId`（类型 + 路径）→ `Handle` | `kairos_asset` 的 server/loader |
| 材质引用纹理 | `texture_path: Some("res/textures/white.texture")`，**路径字符串** | `kairos_graphics/src/material/test.rs:124` |
| 资产设置 / 加工方式 | 资产旁的 `.meta` 边车（loader 名 + settings + `AssetAction`） | ADR 0002 |
| 编辑器选中项、Inspector 显示 | 旧 `AssetRegistry` 的 GUID | 旧 `ProjectWindow` / `InspectorNodeInfo` |

也就是说：**身份在编辑器手里，被引用的资产自己不知道自己是谁**。后果：换台机器、换个 cwd、或者用户直接改文件夹名，注册表就对不上了；它也无法被资产系统使用（assets 只认路径）。

#### 2.11.2 三种主流做法（原样对照）

| 引擎 | 身份载体 | 引用存什么 | 改名/移动怎么办 | 代价 |
|---|---|---|---|---|
| **Unity** | 每个资产旁的 `.meta` 里的 GUID | GUID | `.meta` 跟着文件走；丢了 `.meta` = 引用永久断 | 必须提交 `.meta`；工具必须成对移动 |
| **Godot** 4.4+ | 纯文本资源用 `.gd.uid` 边车（一行 `uid://…`）；场景/导入资源本来就在文件头带 UID；`ResourceUID` 维护 uid↔path 缓存 | UID（`uid://`） | UID 跟文件走；缓存丢了可以重建 | `.uid` 必须提交；4.4 之前是纯路径，一移就断 |
| **Unreal** | 没有持久 UUID，靠 `AssetRegistry` 扫描 `.uasset` 建的路径索引 | **路径**（soft object path） | 改名/移动时在旧路径留 **redirector**，引用经它跳到新路径；再跑「Fix Up Redirectors」批量改写并清掉 | 迁移期会攒一堆重定向器，需要显式修复步骤 |
| **Bevy / 现状 Kairos** | 无 | 路径 | 断 | 零成本，也零保障 |

参考：[Godot `ResourceUID`](https://docs.godotengine.org/en/latest/classes/class_resourceuid.html)、[Godot 4.4 全资源 UID（PR #97352）](https://github.com/godotengine/godot/pull/97352)、[Unreal Redirectors](https://dev.epicgames.com/documentation/en-us/unreal-engine/redirectors)、[Unity asset metadata](https://docs.unity.com/en-us/engine/6000.6/manual/assets-and-media/import-assets/asset-metadata)。

归纳成两条路线：

- **稳定 ID 路线（Unity / Godot）**：身份写在资产旁边（`.meta` / `.uid`），引用存 ID，移动工具负责把 ID 一起搬；
- **路径 + 工具修复路线（Unreal / Blender）**：引用就是路径，靠重定向器或「查找缺失文件」批量重链。

#### 2.11.3 建议：走 Godot 形态，但落点在资产系统

**结论：持久身份应该放进 `.meta`，而不是编辑器注册表。** 理由三条：

1. 我们已经有 `.meta` 边车（ADR 0002），`AssetMeta` 加一个 `guid: Option<Uuid>`（带 `serde default`）是增量改动，不需要新文件类型；旧侧车仍可读，新侧车需要版本容忍（`META_FORMAT_VERSION` 的读侧放宽）。
2. **引用存 `guid + 路径兜底`**（Godot 的形态）比 Unity 的纯 guid 稳：侧车被误删时还能按路径找回，而 Unity 丢 `.meta` 就永久断。
3. 索引变成**派生物**：扫描 `.meta` 就能重建 `guid ↔ path`，可以缓存、可以删、可以提交给别的工具用 —— 和今天"编辑器发号、存一份 TOML"有本质区别。

**D11 定稿：并入本计划，作为 M7。** 它确实要改 `kairos_asset`（meta 格式、guid 分配与写回、`AssetMetaMinimal`）、材质格式与 Inspector 引用 UI —— 跨 3 个 crate，所以排在 M5 类型接入之后、M8 收尾之前。M3 先埋三件**地基**（成本几乎为零），M7 才有干净的落点：

| # | 地基 | 为什么现在做 |
|---|---|---|
| G1 | 树/面板身份用**路径** | 面板选中、展开、面包屑都是"这一帧的文件系统视图"，不需要持久 ID |
| G2 | 改名/移动时**伴生 `.meta` 一起搬** | 否则将来就算加了 `guid`，改个名也把身份弄丢 |
| G3 | 删除时**整簇进回收站**（主文件 + 伴生侧车） | 同上；也避免"删了图留下孤儿 meta" |

**两件事不冲突**：面板按路径工作，和资产按 `guid` 引用是正交的。将来加 `guid` 时，面板一行都不用改——**这正是"必须考虑"的正确形态：现在就把"移动/删除要成簇"做成硬规则，身份后补**。

**顺序为什么是 M7 而不是 M3**：guid 只对「可引用类型」有意义，而可引用类型是 M5 定义的；先做类型、再做身份，能把「给谁发 guid」收敛到 `FileType::referenceable` 一个判据上。

### 2.12 为「打开工程」预留的纪律（D9）

D9 选「保持 cwd」，但**现在有三条纪律必须遵守**，否则将来接「打开工程」会返工：

1. **根是数据，不是环境推论**：`ProjectState.root` 存绝对路径，core 里所有路径都是它下面的 `RelPath`；除 app 的 IO 边界外，任何地方都不出现 `std::env::current_dir()`。
2. **配置按根键化**：编辑器布局文件在 `<root>/Library/editor_layout.toml`（文件里没有项目路径，不需要 root 守卫，但位置天然按根）；未来的「最近打开」列表也按根键化；External Tools 设置 v1 同样按根（`<root>/Library/external_tools.toml`，§2.14 P1）。
3. **切换工程 = 一条 action**：`ProjectOpened { root }` → 清空 `ProjectState`、重置 generation、`ScanRequested`、重读 `<新 root>/Library/editor_layout.toml`。面板只认 `frame.state.project.root`，一行都不用改。

**真正贵的地方不在面板，在引擎**：`kairos_asset` 的 source root 是进程级的（ADR 0004：cwd + `KAIROS_ASSET_ROOT` 覆盖），换工程要重配 asset server / 处理器 / watcher。那是引擎侧的一整套改动，所以「打开工程」不进本计划 —— 但上面三条保证它将来只需要动 app + 引擎，不碰面板。

### 2.13 词表（写进 `kairos_editor/CONTEXT.md`）

**Project**：编辑器的资产工作区，一个根目录 + 它的文件树 + 用户意图（展开、选中、当前目录）。**System dir**：正常扫描但有三条特殊待遇的目录（当前只有 `Library`）——根级排最后、带分隔线、独立预算、不受 ignore 影响。**ProjectTree**：某一时刻文件系统结构的快照，身份是相对路径。**ScanPolicy**：扫描规则（忽略 + 系统目录预算 + 节点/深度上限）。**ProjectView**：树之上的用户意图（展开/选中/当前目录/排序/分栏比例），运行期状态、**不落盘**（D13）。**Editor Layout**：打开的面板集合 + dock 停靠树（分割方向/比例）+ tab 顺序/焦点 + 浮动窗口位置尺寸；M4 落盘的只有它和 `tree_split`。**Layout file**：`<root>/Library/editor_layout.toml`，可删可重建、带 `version`。**FileType**：一个扩展名族的显示与打开方式，不含任何文件操作。**Cluster**：一个资产的主文件 + 它的伴生侧车（`.meta` 等），移动/删除以簇为单位。**AssetId / Guid**：资产的持久身份（写在 `.meta` 里，见 M7），与树的路径身份正交。**External Tools**：按文件族记录和覆盖打开应用的编辑器设置（§2.14）。**Open default**：`FileType` 表里的默认打开策略（嗅探 / 系统默认 / 系统选择器 / 内部视图）。**Effective open target**：一次打开实际使用的目标——用户覆盖 > 嗅探结果 > 类型默认，由 `effective_open()` 纯函数解析并随 `ProjectEffect::Open` 传给 app。**Sniffing**：扫描 PATH 与平台固定路径检测已安装 IDE，作为「跟随默认」的候选来源。

### 2.14 External Tools 面板（2026-11-25 新需求）

> **状态**：需求由用户提出（2026-11-25）；**P1–P3 已于 2026-11-25 拍板（§2.14.9）：设置存 `Library/external_tools.toml`、Browse 用 `native-dialog`、`SystemPicker` 先做 Windows**。按本节方案执行，M5a / M5b 均可开工。

**需求原文**（用户 2026-11-25）：

> 我们现在一些类型文件有默认的启动方式(通过嗅探的方式), 还有一些则会调系统弹窗来选启动方式。我想要有一个 Panel，专门来记录和设置这些文件的启动应用，就像 Unity/Tuanjie 的 External Tools。这个 Panel 的打开路径就是 Edit -> External Tools。

**一句话**：把「这个文件会用哪个程序打开」从 OS 关联 + 硬编码嗅探，变成一张**按文件族**的、可见、可改、可恢复默认的设置表。

#### 2.14.1 现状（2026-11-25 HEAD 实读）

| 事实 | 证据 | 后果 |
|---|---|---|
| 新编辑器只有一条打开路径：`open::that_detached` 交给 OS 关联；失败写进 Project 状态栏 | `kairos_editor_app/src/project.rs:282-290`、`kairos_editor_core/src/project.rs:224` | 用哪个应用打开由 OS 决定，面板/用户看不到也改不了 |
| `FileType` 表还没有 `open` 字段；打开分发是 M5 的事 | `kairos_editor_core/src/project/file_type.rs:1-14` | 打开策略既不在类型表里，也没有消费方 |
| 旧编辑器有 IDE 嗅探：7 个已知 IDE，先查 PATH 再查平台固定路径，`OnceLock` 缓存，按列表顺序取第一个 | `kairos_editor/src/ui/ide_detection.rs:29-168,195-234` | 行为已验证；但结果不可见、不可改，`OnceLock` 让会话中新安装的 IDE 永远发现不了 |
| 旧编辑器按类型分发：脚本/着色器/文档/TOML/字体走 IDE 嗅探；纹理/网格/材质/音频与未知类型未实现 | `kairos_editor/src/ui/project_window.rs:334-369` | 「有的类型有默认启动方式、有的弹系统窗」是两套隐式规则叠加的结果，不是一张可配置的表 |
| 打开是 `ProjectEffect::Open { root, path }`，app 解释 | `kairos_editor_core/src/project.rs:294`、`kairos_editor_app/src/project.rs:282` | 打开策略只能写死在 app；要可配置，必须先让「用哪个程序」成为 core 可解析的数据 |
| 机器相关的设置没有落点：`Preferences/` 是提交进仓库的样式/字体，`Library/` 是不提交的运行期数据 | `kairos_editor_app/src/layout.rs:41`、`.gitignore:10-13` | IDE 绝对路径不能进仓库；设置文件要和布局文件一样放 `Library/`（P1） |

**结论**：需求不是「再加一个设置窗口」，而是补上打开路径里缺的三件事：**类型表里的默认策略**（现在空白）、**可配置的用户覆盖**（现在没有）、**两者的解析优先级**（现在隐含在 app 的 `open::that_detached` 和旧编辑器的硬编码分支里）。

#### 2.14.2 目标行为

1. **一个面板**：`Edit ▸ External Tools` 打开（与 Preferences/About 同一条 `LayoutAction::ShowPanel` 路径；无 dock tab 时浮成窗口，可拖回 dock）。面板本身是普通 `Panel`，随 M4 布局持久化，不需要新存储。
2. **按文件族一行**：每个 `FileType` 一行 + 一行「未知类型」。行内显示类型图标/名称/扩展名、当前**生效**打开方式、来源（`用户设置` / `嗅探` / `类型默认`）、状态（路径存在 / 缺失 / 未检测到 IDE）。
3. **可设置**：从检测到的应用里选、Browse 自选程序、系统默认、每次询问、恢复默认。改完立即生效（下一次双击文件就用它），不要求重启。
4. **嗅探只负责「默认值」**：启动时后台检测已安装 IDE，面板显示检测结果；用户选择覆盖嗅探，选择「恢复默认」回到嗅探。
5. **未知类型保持现状**：默认走系统「打开方式」弹窗（`SystemPicker`）；用户可以把开关改成「系统默认」。
6. **失败必须可见**：配置的程序路径丢失、启动失败、设置写盘失败，都要在面板/状态栏显示；**不静默换成别的打开方式**（用户明确设置了 A，就不能偷偷打开 B）。
7. **嗅探/打开不阻塞 UI 帧**：检测在工作线程；启动进程只 spawn 不等待。
8. **「记录」的边界**：系统「打开方式」弹窗里用户选了什么，OS 不会回传给进程（ShellExecute 只返回成功/失败），所以面板无法自动记录那一次临时选择；要固定打开方式，必须从检测列表或 `Choose program…` 设置一次。系统默认（`SystemDefault`）同理只显示为「System default」，不尝试反查 OS 关联到了哪个程序。

#### 2.14.3 模型：三张表，三个所有者

原计划的 `OpenWith` 是一个枚举同时承担三件事；本需求把它拆开，因为所有者不同：

```rust
// ① 静态数据：kairos_editor_core/src/project/file_type.rs
/// 一个文件族「没有被用户覆盖时」的默认打开策略。
pub enum OpenDefault {
    /// 代码/文档：嗅探第一个可用 IDE；没有检测到就落到 fallback。
    Sniffed(OpenFallback),
    /// 系统关联（OS 默认程序）或系统「打开方式」弹窗。
    System(OpenFallback),
    /// 编辑器内部视图（Inspector/Preview），随 M5 的每类任务落地。
    Internal(InternalOpen),
}
pub enum OpenFallback { SystemDefault, SystemPicker }

pub struct FileType {
    // ... id / label / extensions / icon / companion / thumbnail / delete_confirm ...
    pub open: OpenDefault,
}

// ② 用户意图：kairos_editor_core/src/external_tools.rs（唯一持久化部分）
pub struct OpenSettings {
    /// 未知类型是否每次弹系统选择器（默认 true，保持现状）。
    pub ask_unknown: bool,
    /// 按文件族覆盖；缺省 = 跟随类型默认。
    pub overrides: BTreeMap<OpenKey, OpenTarget>,
}
pub enum OpenKey { Type(&'static str), Unknown }   // 落盘为 "script" / "*"
pub enum OpenTarget { SystemDefault, SystemPicker, Program(ProgramSpec) }
pub struct ProgramSpec {
    pub label: String,        // 面板显示名："Cursor" / "My Editor"
    pub path: PathBuf,        // 绝对路径或 PATH 中的命令名；原样 spawn，不走 shell
    pub args: Vec<String>,    // "$FILE" 替换为绝对路径；不含 "$FILE" 时把路径追加为最后一个参数
}

// ③ 宿主执行载荷：reducer 解析后随 effect 传给 app
pub enum ResolvedTarget { Internal(InternalOpen), Program(ProgramSpec), SystemDefault, SystemPicker }
pub enum InternalOpen { Inspector, Preview }   // 具体随 M5 每类任务定
pub enum OpenSource { User, Sniffed, TypeDefault, AskUnknown, Internal }
pub struct EffectiveOpen {
    pub key: OpenKey,
    pub target: ResolvedTarget,
    pub source: OpenSource,       // User | Sniffed | TypeDefault | AskUnknown | Internal
    pub warning: Option<String>,  // 配置的程序缺失等；面板黄字，打开时按失败处理
}
```

面板 → reducer → app 的协议（与 Project 同款 request / completion）：

```rust
pub enum ExternalToolsAction {
    Loaded { settings: OpenSettings },        // boot 读文件后
    Detected { apps: Vec<DetectedApp> },      // 检测完成
    DetectionFailed { message: String },
    DetectRequested,
    SetTarget { key: OpenKey, target: OpenTarget },
    SetArgs { key: OpenKey, args: Vec<String> },
    ClearTarget { key: OpenKey },             // = Follow default
    SetAskUnknown(bool),
    BrowseRequested { key: OpenKey },         // 面板要系统文件选择器
    ProgramPicked { key: OpenKey, path: PathBuf },
    PersistFailed { message: String },
}
pub enum ExternalToolsEffect {
    Detect,
    Browse { key: OpenKey },
    Persist { root: PathBuf, settings: OpenSettings },
}
```

路由：aggregate 增加 `Action::ExternalTools(..)` / `Effect::ExternalTools(..)`，`EditorState` 加 `external_tools: ExternalToolsState` 字段；面板只读它、写 action，app 的 executor 处理 `Detect` / `Browse` / `Persist`。

**解析优先级**（`core::external_tools::effective_open()`，纯函数，一张表一测试）：

| # | 条件 | 结果 | 来源徽标 |
|---|---|---|---|
| 1 | 该类型有 `overrides` 条目 | 用户设置的目标 | `用户设置` |
| 2 | 无覆盖、`OpenDefault::Sniffed` 且检测到至少一个 IDE | 检测列表顺序第一个 | `嗅探：<IDE>` |
| 3 | 无覆盖、`OpenDefault::Sniffed` 但未检测到 IDE | `OpenFallback`（v1 = `SystemPicker`） | `类型默认` |
| 4 | 无覆盖、`OpenDefault::System(..)` | 表中的 `SystemDefault` / `SystemPicker` | `类型默认` |
| 5 | 无覆盖、`OpenDefault::Internal(..)` 且该构建有内部视图 | 内部视图 | `类型默认` |
| 6 | 类型未知（没有 `FileType`） | `ask_unknown ? SystemPicker : SystemDefault` | `未知类型` |
| 7 | 配置的 `Program` 路径缺失 | 保留配置、`warning` 置位；打开拒绝并报错 | `用户设置` + 警告 |

**替换原文的 `OpenWith`**：§2.8 写 `open: OpenWith`，M5 原文写「`OpenWith` 五类」但注释只列了四类（`ExternalEditor | SystemDefault | Inspector | Preview`），两处本来就不一致。本节用 `OpenDefault`（静态表）+ `OpenTarget`（用户设置）+ `ResolvedTarget`（effect 载荷）替代；§2.5 的 `ProjectEffect::Open { .., with: OpenWith }` 同步改为 `target: ResolvedTarget`。M5a 落地时以本节为准，不再引入名为 `OpenWith` 的枚举。

**默认类型表**（v1，M5a）：

| 类型 | 默认 | 理由 |
|---|---|---|
| script (.rs)、shader (.wgsl)、document (.md) | `OpenDefault::Sniffed(SystemPicker)` | 旧编辑器对脚本/着色器/文档就是嗅探 IDE；没有 IDE 时让系统弹窗选，和用户描述的现状一致 |
| texture (.png)、mesh (.glb)、material (.mat)、audio | `OpenDefault::System(SystemDefault)` | 这些类型的内部预览/Inspector 还没落地；先交给 OS 关联，不阻断工作 |
| font (.ttf/.otf) | `OpenDefault::System(SystemDefault)` | 字体没有「用 IDE 打开」的语义 |
| companion (.meta) | 不显示、不打开 | 伴生文件不是资产入口 |
| 未知类型 | `SystemPicker`（`ask_unknown` 默认 true） | 保持现状：系统「打开方式」弹窗 |

> 每类任务（M5 之后）把 `OpenDefault` 从 `System(..)` 改成 `Internal(..)` 时，面板与解析器都不需要改：行数据来自类型表，解析器只认三个枚举。

#### 2.14.4 嗅探（app，M5a）

- 从旧 crate 的 `kairos_editor/src/ui/ide_detection.rs` 搬到 `kairos_editor_app/src/external_tools/detect.rs`，**不照抄 `OnceLock`**：启动时检测一次 + 面板「重新检测」再检测一次；结果进 `ExternalToolsAction::Detected`。
- 候选表是数据（`const CANDIDATES: &[IdeDescriptor]`），顺序即嗅探优先级：

| id | 显示名 | CLI | 说明 |
|---|---|---|---|
| `cursor` | Cursor | `cursor` | 旧表第一优先 |
| `code` | VS Code | `code` | |
| `code-insiders` | VS Code Insiders | `code-insiders` | |
| `zed` | Zed | `zed` | |
| `windsurf` | Windsurf | `windsurf` | |
| `idea` | IntelliJ IDEA | `idea` | Rust 插件用户 |
| `fleet` | Fleet | `fleet` | |
| `rustrover` | RustRover | `rustrover` | **新增**；JetBrains 的 Rust 专用 IDE |

- 候选顺序沿用旧表（Cursor 第一），RustRover 追加在末尾；面板里用户可任意覆盖，所以顺序只影响「跟随默认」的初值。
- 检测在工作线程 `external-tools-detect` 跑：每候选先 `--version`（或 `--help`）探 PATH，再查固定路径（沿用 `%USERNAME%` 展开）；Windows 用 `CREATE_NO_WINDOW` 避免每个候选闪一次控制台；单个候选失败只 debug 日志，最终「一个都没检测到」才是一条可见提示。
- 结果只存在内存（`ExternalToolsState.detected`），**不落盘**：路径会因升级/卸载变化，落盘只会过期。用户选择某个嗅探结果时，存的是当时的 `ProgramSpec` 快照，所以检测结果消失也不影响已配置的行。
- 面板顶部显示检测列表（名称 + 路径）；空列表时提示「未检测到 IDE，脚本类将使用系统『打开方式』」。

#### 2.14.5 打开执行（app，M5a）

`kairos_editor_app/src/external_tools/launch.rs`，三条路径：

| target | 实现 | 平台说明 |
|---|---|---|
| `SystemDefault` | `open::that_detached(absolute)`（沿用 M2） | 三平台 |
| `SystemPicker` | Windows：`rundll32 shell32.dll,OpenAs_RunDLL <absolute>`；调用前校验路径存在 | v1 只在 Windows 实现（P3）；macOS/Linux 没有等价的「选择应用后打开文件」入口，回退 `SystemDefault` 并在面板标「该系统不支持每次询问」 |
| `Program(spec)` | `std::process::Command::new(path).args(substituted).spawn()`，detached；Windows 的 `.cmd/.bat` 用 `cmd /C` 包裹，`CREATE_NO_WINDOW` | 三平台；**绝不拼 shell 字符串** |

- `$FILE` 替换发生在 argv 元素内部（例如 `["-g", "$FILE"]` → `["-g", "C:/proj/a.rs"]`），不是把整个命令拼成一行；没有 `$FILE` 时把文件路径追加为最后一个参数。
- 启动失败（找不到可执行文件、OS 拒绝、权限不足）→ `ProjectAction::Failed { message }`，复用 Project 面板状态栏；External Tools 面板同时显示该行警告。
- 不做「失败后自动换下一个应用」：配置是用户意图，静默改变打开方式比失败更难排查；「恢复默认」是显式操作。
- 路径校验：`Program.path` 存在性在设置加载/保存时检查一次，缺失时行内显示警告而不是删除配置（用户可能只是换了盘符/重装）。

#### 2.14.6 设置文件（app，M5a）

`<root>/Library/external_tools.toml`，与 `editor_layout.toml` 同族：可删、可重建、**不进 git**、watcher 已过滤 `Library/`（M1 的系统目录规则），不会自激重扫。

```toml
# External Tools written by Kairos. Machine-specific; delete to reset.
version = 1
ask_unknown = true

[open.script]
kind = "program"
label = "Cursor"
path = "C:/Users/me/AppData/Local/Programs/cursor/resources/app/bin/cursor.cmd"
args = ["$FILE"]

[open.texture]
kind = "system_default"
```

- **DTO 在 app**（core 无 serde，与 `layout.rs` 的分工一致）：app 的 `settings.rs` 定义 serde 结构、读写、原子替换、64KB 上限、版本/坏文件回退（与布局文件同款纪律）；core 只收 `OpenSettings`。
- 缺失 → 默认设置；首次打开面板时**不**自动写文件（用户没改就不落盘，与布局文件「干净启动不写文件」一致）；写坏 → 整份忽略 + warn，不阻塞编辑器。
- 写时机：设置变化即写（点击/选择是一次手势）；参数文本输入 500ms 去抖；`Layer::on_exit` flush。写失败 → 面板错误行 + `kairos_log::warn`，内存设置继续生效（下次可重试）。
- 类型 id 在升级中消失：读侧忽略该条并 warn，写侧按当前 DTO 重写（配置丢失可接受；类型表删除本就意味着不再有该类型）。
- **为什么放 `Library/` 而不是 `Preferences/`**：`Preferences/` 是提交进仓库的编辑器外观（样式/字体/语法高亮），External Tools 的绝对路径因机器而异，提交会立刻在别人机器上失效；`Library/` 是已经确立的 per-project 运行期数据位置（D9/D13）。P1 已拍板 v1 按项目存；将来若升用户级，只动 `settings.rs` 的路径。

#### 2.14.7 面板 UI（M5b）

```text
┌ External Tools ───────────────────────────────────────────────────────────┐
│ Detected applications                                             [Detect] │ ← 区块头 20px
│  Cursor        C:/Users/…/cursor.cmd                                        │
│  VS Code       C:/Users/…/code.cmd                                          │
│  (none detected — script/shader/document fall back to the system picker)    │
├───────────────────────────────────────────────────────────────────────────┤
│ Open with                                                      [Reset all]  │
│  Type            Open with                         Source          Status   │
│  ▣ Script        [Cursor ▾]                        Sniffed         ✓        │
│  ▣ Shader        [Cursor ▾]                        Sniffed         ✓        │
│  ▣ Document      [Follow default ▾]                Type default              │
│  ▣ Texture       [System default ▾]                Type default              │
│  …                                                                          │
│  ○ Other files   [Ask every time ▾]                Unknown                   │
├───────────────────────────────────────────────────────────────────────────┤
│ <error / status line: save failed, missing program, detection failed…>     │
└───────────────────────────────────────────────────────────────────────────┘
```

- 入口：`toolbar.rs` 的 Edit 菜单在 Preferences 后加一条 `External Tools` → `LayoutAction::ShowPanel { panel: panels::EXTERNAL_TOOLS }`；面板声明 `#[crate::panel(EXTERNAL_TOOLS = "external_tools", skin = ExternalToolsPanelStyle)]`，标题 `External Tools`。
- 顶部区块 = **检测结果**（只读列表）+ `Detect`。`Detect` 发 `ExternalToolsAction::DetectRequested`；检测中按钮禁用并显示 `Detecting…`。选择程序入口在行内菜单的 `Choose program…`（打开系统文件选择器 `native-dialog 0.9.6`，已在 workspace 锁文件里、旧编辑器同款），选中后只影响那一行。
- 中间区块 = **打开方式表**，每行一个 `FileType`（按类型表顺序；`companion` 不显示）+「Other files」。行内 menu button 的选项：
  - 每个检测到的应用（写 `Program` 快照，label = 应用名）；
  - `System default`；`Ask every time`（`SystemPicker`）；
  - `Follow default`（删除覆盖，回到类型默认；只有存在覆盖时出现）；
  - `Choose program…`（等价于该行的 Browse）。
- 当前值由 `effective_open()` 算出来：若被覆盖，显示覆盖目标 + `用户设置` 徽标；否则显示解析结果 + `嗅探`/`类型默认`。状态列：路径存在 ✓；配置路径缺失 ⚠（黄）+ 行内 tooltip；检测不到 IDE 时脚本行显示「类型默认」而不是错误（不是用户配置的问题）。
- 交互：菜单选择后立即写设置（一次点击一次落盘），不需要 Apply；参数编辑只对自定义 `Program` 出现（缩进一行，`Arguments` 文本框，默认 `$FILE`，500ms 去抖）；`Reset all` 有确认弹窗（复用 `widgets::modal` 与 T13 的皮肤），单行 Reset 不需要确认（重新选一次即可撤销）。
- 面板皮肤 `ExternalToolsPanelStyle`（`Preferences/Styles/ExternalToolsPanel.ron`）：沿用 Console/Project 的暗色 token 语言，约 20 个标量（背景、区块头、表头、行文字/次要文字、hover、选中、警告/错误色、分隔线、行高、状态行）；`#[derive(StyleRow)]` 自动进 Preferences 页。**不新造主题系统**。
- tab 图标 v1 不做（`Panel::icon` 返回 `None`，标题文字 tab）；要加时从 Tuanjie 资源包抽一张（同类新增一张图，不在本需求范围）。
- 面板不持有设置副本：读 `frame.state.external_tools`，写动作；关闭重开/重启后设置从文件恢复，面板无「脏」状态。

#### 2.14.8 归属与里程碑（M5a / M5b）

| 层 | M5a（打开分发 + 嗅探 + 设置） | M5b（面板） |
|---|---|---|
| core | `FileType::open`、`external_tools` 切片（state/action/effect/reducer）、`effective_open` 纯解析、`ProjectEffect::Open { target }` | 无（面板只读 state） |
| app | `external_tools/{mod,detect,settings,launch}.rs`；启动顺序（读设置 → 起检测 → inbox）；`native-dialog` 依赖；`ProjectEffect::Open` 执行 | 把检测/浏览完成的 action 接进 inbox；设置变更写盘（若 M5a 未含）；无新逻辑 |
| component | 无可见变化：`OpenRequested` 仍只发路径，解析在 core | `panels/external_tools.rs` + `style.rs` + RON；Edit 菜单项；面板测试 |
| widgets | 无 | 复用 `menu`/`button`/`field`/`modal`；不新增 widget |

**为什么先 M5a**：面板只是解析结果的可视化；如果先做面板，`OpenDefault`/`effective_open` 都还没有，面板会现场发明数据结构（与此前 M3a/M3b 的教训一致）。

#### 2.14.9 已拍板项（P1–P3，2026-11-25）

| # | 问题 | 拍板结果（2026-11-25 用户确认） |
|---|---|---|
| **P1** | 设置存哪 | **`<root>/Library/external_tools.toml`**：per-project、机器相关、不进 git（原选项 a）。 |
| **P2** | Browse 怎么选程序 | **系统原生文件选择器 `native-dialog`**（原选项 a；已在 workspace 锁文件里，旧编辑器同款）。 |
| **P3** | 「每次询问」的平台范围 | **先做 Windows**（原选项 a）：只有 Windows 实现系统选择器；macOS/Linux 回退 `SystemDefault` 并在面板标注。 |

> P1–P3 已拍板，M5a / M5b 按此执行，不再阻塞。P1 若将来改为用户级设置，只动 `settings.rs` 的路径，不改解析器与面板。

#### 2.14.10 验收 / 测试 / 风险

**M5a 验收**（headless 为主）：
- 测试替身注册一个 `foo` 类型，`open: Sniffed(SystemPicker)`：无覆盖时解析为 `SystemPicker`；放一个假 `Program` 覆盖后解析为该程序；删除覆盖后回到默认；配置路径不存在 → 解析结果保留 + `warning` 置位；未知文件名走 `ask_unknown` 两个分支。
- 设置 DTO：round-trip、坏 TOML / 版本不认识 / 超过大小上限 → 默认设置 + warn、原子写、去抖/flush、`Library/` 写入不触发重扫（复用 M1 的 watcher 测试口径）。
- 嗅探：候选表顺序、`%USERNAME%` 展开、同一应用去重、PATH 与固定路径两种命中（用临时可执行文件替身，不启动真 IDE）；Windows 下 `CREATE_NO_WINDOW` 的构造断言（不依赖真实进程）。
- 启动：临时 `.cmd`/shell 脚本写 marker 文件 → `Program` 带 `$FILE` / 不带 `$FILE` 两种 argv 断言；缺路径 → `ProjectAction::Failed`；`SystemDefault` 只做 smoke（不在测试里真的开编辑器）。
- 打开链：`OpenRequested` → reducer 解析 → effect 带 `ResolvedTarget`；app 执行后失败回 `Failed`。

**M5b 验收**（手动 smoke + harness）：
- `Edit ▸ External Tools` 打开面板（无 tab 时浮窗；拖进 dock 后 tab 标题正确；重启后随布局恢复）；
- 面板列出检测到的 IDE 与全部文件族；把 Script 设为检测到的 IDE → 回 Project 面板双击 `.rs` 用该 IDE 打开；把 Texture 设为自定义程序 → 双击 `.png` 用该程序打开；
- 设置 `Ask every time` 后双击未知扩展名文件 → 系统「打开方式」弹窗；改回 `System default` → 走 OS 关联；
- 故意把配置路径改坏 → 面板黄字警告；双击文件 → 状态栏显示失败，**不**偷偷换应用；
- 重启编辑器 → 设置仍在；删掉 `Library/external_tools.toml` → 回到默认 + 嗅探；
- 选择某 IDE 后把它从检测列表移除（模拟重装）→ 行显示路径缺失警告；`Follow default` 可恢复；
- 写盘失败（把 `Library/` 设只读/占位）→ 面板错误行可见，设置仍在内存生效；
- 面板帧耗时（Profiler）：空转 < 1ms；检测在后台，打开面板不卡顿。

**风险**：

| 风险 | 缓解 |
|---|---|
| 探测进程闪控制台/拖慢启动 | 工作线程 + `CREATE_NO_WINDOW` + 每候选超时（`--version` 最长 ~500ms 级）；M5 已把「Windows 不闪控制台」列为要求 |
| 配置路径含空格/参数注入 | argv 直传、`$FILE` 按元素替换、绝不拼 shell；路径存在性校验 + 启动失败可见 |
| 设置静默丢失 | 原子写 + 写失败进面板错误；读取坏文件回退默认；测试覆盖 |
| 项目内容注入可执行路径 | 设置只从 `Library/`（不提交、不参与资产源）读，不从项目资产/`.meta` 里读；打开必须由用户手势触发 |
| 嗅探结果过期/误导 | 不落盘、启动重测、面板显式 `Detect`；已选应用存路径快照，检测消失不影响已有配置 |
| P1 的 per-project 设置导致多工程重复配置 | 若用户经常切工程，再升用户级；只动 `settings.rs` 的路径，不改解析器/面板 |

### 2.15 Rust 脚本图标（2026-11-25 新需求，已交付）

**需求原文**（用户 2026-11-25）：

> 另一个需求就是我要你基于当前的 scripts icon(Unity的脚本 icon)重新设计一个 icon 出来，把 icon 里的 "#" 换成 "R" 或其他你觉得会更好的样子, 即从代表 C# 换成代表 Rust。

**结论**：保留 Unity/Tuanjie 的「折角页面 + 语言字形」构图，把页面里的 `#`（C#）换成 **R**（Rust），R 沿用原 C# `#` 的 **Unity 绿**（dark `#276A2A` / light `#2E7D32`）。2026-11-25 用户两次反馈：红色 R 不协调 → 改 Unity 同款绿；R 过大 → 缩到 `#` 的约 85% 高、同样 8.3° 斜体、墨迹 0.92× `#`。只换字形，不新增品牌色。理由：

1. 这是 18 px 的树行图标（64 px 资产）；Rust 齿轮 logo 的齿在 18 px 会糊成一团，而「页面 + 单字符」是 Unity 脚本图标已经教给用户的语法，只换字符语义最省认知成本。
2. Unity 用 `#` 代表 C#（sharp），Rust 没有等价的单字符符号；R 是 Rust 最短、最不会认错的语言标记。

**交付**：

| 文件 | 内容 |
|---|---|
| `Preferences/Textures/project/types/script.png` | dark skin 的 Rust 脚本图标（64×64 RGBA） |
| `Preferences/Textures/project/types/light/script.png` | light skin 版本（同形：白页面 + 灰边框） |
| `scripts/extract_tuanjie_icons.py` | 从抽取表移除 `cs Script Icon`：脚本图标改为项目自绘，重跑脚本不会覆盖 |
| `Preferences/Textures/project/README.md` | provenance 改为「页面外形沿用 Tuanjie `d_cs Script Icon`，R 字形为项目原创，颜色沿用原 C# 绿」，记录色值 |

**不新增类型、不改代码**：`FileType::icon` 仍是 `"script.png"`，`.rs` 家族不变；M5 的类型表/面板不需要知道「图标是自绘的」。如果将来重新支持 C#，加一个 `script_cs.png` 和一个新 `FileType` 行，不覆盖这张图。

**实现细节**（交付时实测）：页面取自 Tuanjie 256×256 源图，R 在 1024×1024 上绘制后 Lanczos 降到 64×64；R 与 `#` 一样倾斜 **8.3°（右上→左下，同原 `#` 方向）**。按 256 源尺度，R 高 120（=30.0 px@64）、宽 108（=27 px@64，含斜体位移）、笔画 24（=6.0 px@64），64 px 下字形外框中心 (31.0, 32.5)，原 `#` 为 (30.5, 33.0)——比 `#` 矮约 14%，视觉上更收敛。R 墨迹面积约为 `#` 的 0.92×。颜色沿用原 `#` 的绿：dark `#276A2A`（`rgb(39, 106, 42)`）、light `#2E7D32`（`rgb(46, 125, 50)`）。对比度（WCAG 2.x）：对 dark 页 `#F0F0F0` = 5.79:1，对 light 页 `#FFFFFF` = 5.13:1，均高于图形对象 3:1、也高于正文文本 4.5:1。

**验收**：dark/light 两张 64² 图在 18 px 显示尺寸下可辨认 R；放大检查无残留 `#` 像素/抠图锯齿；`git diff --stat` 只涉及上表 4 个文件（外加本计划文档）；`cargo test-crate kairos_editor_component` 不受影响（图标路径不变）。

---

## 3. 里程碑

每个里程碑的完成定义（DoD）都包含：编辑器可运行 / 改动 crate 的 `cargo test-crate` 全绿 / `cargo check --workspace --all-targets` 通过 / 手动 smoke 清单勾完。**不跑 `cargo test-full`**（见 `docs/agents/testing.md`）。

### M0 — 接缝与状态骨架（0.5–1 天）

**目标**：把「项目」变成状态树里的一个 slice，并把异步回写通道打通，**画面零变化**。

**交付**
- `EditorState.project: ProjectState`，删除 `project_name`（3 个测试 + inspector demo 面板跟着改）；
- `Action::Project` / `Effect::Project` + `reduce/project.rs`（先只处理 `ScanRequested` → `running = true`）；
- `EditorApp.inbox` + `begin_frame` 宿主阶段（每帧一次，在 egui pass 之外）+ `ProjectService` 空壳；
- `EffectExecutor` 增加 `project` 与 `now`；`pump` 的 `describe()` 增加 Project 分支（高频 action 要像 `StyleAction::SetField` 一样静音，避免每帧刷 console）。

**验收**：编辑器启动、行为与现在完全一致；新增两条测试：inbox 保序、effect 产出的 action 在**下一帧**被 apply。

> **实现说明（M0 落地时修订）**：采用「跨帧完成」——effect 产出的 action 一律进 inbox，下一帧 `begin_frame` 才 apply；不在同帧反复 settle，因此没有 8 轮预算，action↔effect 环由帧边界切断。代价是宿主同步产生的完成晚一帧（扫描线程的完成走 `poll`，poll 到即 apply）。M0 的 `ProjectService::execute` 同步返回一个空结果的 `Scanned`，让「请求 → effect → 完成」协议端到端可测；M1 只把函数体换成后台线程 + `poll`，app 侧不动。

**风险**：静音规则写错 → console 被刷屏。用「action → 是否记录」的显式函数，而不是在 `pump` 里加 `matches!`。

### M0.5 — 资产布局迁移：`imported_assets` → `Library/`（0.5–1 天，D12）

**目标**：把成品根从项目顶层搬进 `Library/`，让后续所有代码都少一条特例（"先搬、以后不重构"）。

**为什么必须先做**：只要 `imported_assets` 还在项目顶层，`ScanPolicy` 就得有一个"跳过成品根"的字段，面板就得解释"为什么这个目录不显示"。搬到 `Library/` 之后，它天然被系统目录的预算覆盖，这个字段和这条解释一起消失。

**交付**
- 仓库：`git mv imported_assets Library/imported_assets`（产品是提交进仓库的；只有 `/imported_assets/log` 被 gitignore）；
- `.gitignore`：`/imported_assets/log` → `/Library/imported_assets/log`（`/Library/Logs`、`/Library/Profiler` 两条保持不变）；
- 常量/字面量/数据（共 10 处；复核时从 6 处补到这里）：`AssetOptions::DEFAULT_PROCESSED_FILE_PATH`（`kairos_asset/src/install.rs:136`）、processor 的 `LOG_PATH`（`kairos_asset/src/processor/log.rs:142`）、`kairos_engine::kairos_paths::PATH_WHITE_TEXTURE`、`kairos_graphics::render_pipeline::PATH_WHITE_TEXTURE`、`kairos_game.rs` 的两处 `load` 字面量、旧编辑器 `ui/inspector/material.rs:89` 的 `DEFAULT_PREVIEW_MESH`、**`res/materials/material.mat` 的 `texture_path`**、**`Preferences/Styles/Inspectors/Material.toml` 的 `preview_meshes`**（后两处是数据/配置，不补会静默丢纹理和预览）、旧编辑器 `product_directory_is_hidden` 的测试夹具；外加 `bake_assets` 的文档注释；
- 与 D8 的边界：旧 crate **只改上面那一处路径字面量**，其余一行不动——D8 冻结的是逻辑，不是常量。旧 `project_path_tree` 的测试用 tempdir 自建根，不受搬迁影响；
- 语义确认（写进 ADR 0017）：`processed_subtree_exclude` 取成品路径相对源根的第一段，现在会返回 **`Library`** —— 即「资产源永不包含 `Library`」，asset watcher 也不再盯它（正是我们想要的）；
- 文档：新 ADR `0017-processed-root-under-library.md`；给 ADR 0004/0005 加一行「成品根位置已被 0017 取代，其余结论不变」；`.gitignore` 注释、`CONTEXT.md` 里提到 `imported_assets` 的地方同步；
- 编辑器侧：`ScanPolicy` 里**不再有** `skip_top_level`（M1 直接按最终形态写）。

**验收**
- `cargo run --bin bake_assets` 重新生成到 `Library/imported_assets/Default`；
- 编辑器/引擎仍能加载白纹理、`Ball.glb`、`Plane.glb`（`cargo run` 起来看到场景不丢资源）；
- `cargo test-crate kairos_asset` / `kairos_engine` / `kairos_graphics` 全绿（测试里的 `imported_assets` 期望值一并改）。

**风险**：仓库里提交的二进制产品要整体搬迁（一次性 diff 较大，但内容不变）；如果有人在别的分支上有未合并的 `imported_assets` 改动，需要 rebake 而不是手搬。

> **落地状态（完成）**
>
> - 10 个成品 `git mv` 到 `Library/imported_assets/Default/`，10 处字面量/数据、`.gitignore`、文档注释同步；ADR 0017 新增，ADR 0004/0005/0007 加交叉注记。旧编辑器只动了 `DEFAULT_PREVIEW_MESH` 与一处测试夹具（规则本身没变），76 个测试全绿。
> - `cargo run --bin bake_assets` 在**新路径**下重新生成：已提交的 10 个成品**逐字节不变**（`git diff -- Library` 为空），说明搬迁没有动内容。
> - 编辑器 rebake 后启动无任何 asset / texture / mesh 加载错误。
> - **顺带发现（不是 M0.5 引入的）→ 已按 ADR 0018 处理**：`bake_assets` 会把可见树里所有能被 loader/processor 认领的文件加工一遍（`docs/` 截图、`Preferences/` 图标、`target/doc` 的 rustdoc 图）。处理方式：**排除规则统一到 `.kairosignore`，且 bake 归属编辑器侧**（`kairos_editor_app/src/bin/bake_assets.rs`，命令名不变）——引擎只导出 `bake_world`/`run_bake`，不再读项目配置。现在 bake 跳过 `.cargo`/`.zed`/`target`；可见目录照旧产出成品（本仓库 230 个、约 330 MB，不提交，也**不做额外丢弃处理**）。已提交的 10 个成品逐字节不变。

### M1 — ProjectTree v2：模型 + 扫描 + 监听 + .kairosignore（2.5–3.5 天，分两刀）

**目标**：树在全 app 里存在且可信，面板只多一块只读读数（M2 换成真面板）。

> **M1 开工前复核定稿（5 条补充 + 3 处修正）**——计划原文这几处是空白或自相矛盾，实现按这里来：
>
> | # | 决定 | 为什么现在必须定 |
> |---|---|---|
> | R1 | `ProjectState.policy: ScanPolicy` + `Action::Project(PolicyLoaded)`；boot 顺序写死在 `EditorApp::new`：读/seed `.kairosignore` → 起 watcher → inbox 依次 `PolicyLoaded`、`ScanRequested{Startup}` | `Effect::Scan{policy}` 得有人产出；读 ignore 是 app 的 IO，策略却是 core 的数据 |
> | R2 | `assemble(entries, &system_dirs)`；`ProjectTree::system_roots` + `is_system_dir`/`is_under_system` | 分隔线（M2）与 `In Library` 搜索范围（M6）都要问树，树不能反过来依赖 policy |
> | R3 | `Scanned.errors: Vec<String>`（按目录聚合，上限 8）→ `ProjectState.error` | M0 已承诺「失败是状态不是日志」；只打 warn 会让 M2 的状态栏没得画，等于重现旧实现 `read_dir` 静默失败 |
> | R4 | **watcher 用同一套 `IgnoreRules` 过滤事件**（不止 `Library/` 前缀），扫描线程每目录查 `cancel` token | `target/` 有 5.7 万个文件，编辑器开着跑 `cargo build` 会每 300ms 触发一次全量重扫；只丢 `Library/` 挡不住 |
> | R5 | `Scanned` 的接受条件是 `running && generation == scan.generation`，接受后清 `running`；`poll` 按到达顺序全喂 inbox | 旧扫描的 completion 被 generation 挡住；重复 delivery 被清掉的 `running` 挡住——两条都要有测试 |
>
> 修正：`ScanPolicy` 按 §2.2 完整交付（含 `max_nodes`/`max_depth`/`follow_dir_symlinks`）；`Node` 不存 `name`（派生）、`by_path` 用 `HashMap`；树侧 `generation` 改名 `revision`。

**M1a — core 纯模型（1–1.5 天）**

- `RelPath`/`NodeId`/`Node`/`ProjectTree`：`assemble(entries, &system_dirs)`（字节序单趟，父一定在前缀之前）、`ordered_children`（根级 `(is_system, !is_dir, lowercase_name)`）、`node`/`find`/`is_system_dir`/`is_under_system`/`revision`/`truncated`；
- `ScanPolicy`（完整字段）+ `system_dirs` 内置 `["Library"]` + `IgnoreRules`：内置兜底**只含点条目与 `target`**（Q3 定稿：不预置 `Preferences`/`.codegraph`，等真需要再改默认值）；名字 / 路径前缀两种模式，`*`/`?`，大小写不敏感，分隔符归一；
- `visible_rows(&expanded, sort, show_hidden)`：O(n) 前序，只产出可见行（M2 的行缓存消费它；M1 只有测试和 debug 读数用）；
- reducer：`PolicyLoaded`、`ScanRequested`（带 policy 发 effect）、`Scanned`（assemble + 剪枝 `expanded`/`selection`/`active` + `error` + `focus_after_scan` 揭示）；
- 测试：三条不变量、排序/目录优先/大小写 tie-break、非 UTF-8 lossy 撞车丢弃、超限与深度截断、`Library` 排最后且不受 ignore、系统目录预算、`visible_rows` 与展开集合、`IgnoreRules` 表驱动（默认值/名字/前缀/大小写/分隔符/坏模式）、reducer 的 generation 丢弃与剪枝。

**M1b — app 扫描 + 监听（1.5–2 天）**

- `project/scanner.rs`：后台线程（每请求一线程 + `mpsc`，`cancel: Arc<AtomicBool>` 每目录查一次）、单根 DFS（忽略 + 深度 + 双预算）、`elapsed_ms`、`errors` 聚合；
- `project/watcher.rs`：从 `kairos_editor/src/project_watcher.rs` 搬（`notify-debouncer-full` 0.7，300ms），**事件先过 ignore 谓词**（`target`/点条目/`Library/`），只留值得重扫的批次；`std::sync::mpsc` 替 `crossbeam-channel`，`kairos_log` 替 `log`；
- `project/ignore.rs`：读项目根 `.kairosignore`，缺失时就地 seed（与 `Preferences/Styles/*.ron` 同一套 seed 行为），坏行 warn + 跳过；
- `ProjectService`：`new(root)` 返回策略并起 watcher；`poll` 排空扫描完成与 watcher 事件 → `Scanned` / `RescanRequested`；`execute(Scan)` 起线程；
- `EditorApp::new` 的 boot 顺序（R1）；
- 组件：`panels/project.rs` 的 39 行 demo 换成只读读数（节点数 / Library 数 / 上次耗时 / truncated / 前 200 条 `visible_rows` 纯文本），M2 整段替换；
- 测试：tempdir fixture（含被忽略目录、深层目录、系统目录预算、不可读目录→errors）、watcher 事件过滤（`target/` 下的写不产生 rescan、`Library/` 下的写不产生 rescan、普通文件写产生一次）、boot 只发一次扫描、ignore seed 幂等、坏行容忍；watcher 去抖不靠 sleep 断言时序（断言「多次事件只产生一次 rescan」）。

**验收**（M1a 绿 + M1b 绿之后一起勾）

- 启动日志出现 `project scan: N nodes (library M) in X ms`；
- 在编辑器外新建/删除文件，300ms 内树刷新（debug 读数里的节点数变化）；
- 5 万节点 fixture 扫描 < 300ms（debug 构建放宽）：**不写成普通单测**——NTFS 上建 5 万个文件比扫它们慢一个数量级，会把 `cargo test-crate` 从 8s 拉到分钟级。用 `#[ignore]` 测试或一次性脚本，M1 收尾手动跑一次，把数字写回 §5；
- 在 `Library/` 里写文件（日志）**不触发** Project 重扫；在 `target/` 里写文件同样不触发（console 里看不到 `rescan`）；
- 编辑器跑起来截图：Project 面板读数显示节点数，树的排序/隐藏行为与测试一致。

> **落地状态（M1 完成，验收逐条勾）**
>
> - 文件：core 的 `project/{tree,scan,ignore}.rs` + `reduce/project.rs`；app 的 `project/{mod,scanner,watcher,ignore}.rs`；`panels/project.rs` 换成只读读数；`EditorApp::new` 拆出 `with_root`（测试用 tempdir 作根，不在仓库里扫/监听）。
> - 启动日志：`project policy: 1 system dirs, 2 ignore rules` → `project scan requested (Startup)` → `project scan #1: 1398 nodes (library 20) in 11 ms`。
> - 外部变更：编辑器外新建一个文件 → 同秒内 `project scan #2: 1399 nodes` 落地（< 300 ms）。
> - 噪声过滤：在 `Library/Logs/` 连写两次、在 `target/` 写一次 → **0 次 rescan**（scan 号不变）；随后正常写根目录文件 → 恰好 +1 次。
> - 截图：Project 面板显示 33 行顶层（`.git`、`target`、点目录都不在其中），`Library` 排在最后。
> - 50,251 节点 fixture：**42 ms**（dev profile，见 §5）。
> - 已知行为（不是缺陷）：一次外部写入偶尔被 debouncer 拆成两个批次，于是连扫两次（各约 10 ms，结果一致）。要合并的话在 `ProjectService::poll` 侧收拢即可，M2 之后视实测再定。

### M2 — 面板 v1：左右两栏 + 树 + 导航（3.5–5 天，分 M2a / M2b 两刀）

**目标**：Project 面板变成**左右两栏**的真实面板（D1）：左树能展开/选中/导航/打开，右栏是面包屑 + 空白占位；树底部固定排最后的 `Library` 与分隔线（D2）；**只读**（不改文件系统）。

> **M2 开工前复核定稿（2026-11-22，用户已拍板 6 条）** —— 计划原文这些地方会「现场发明」，实现一律按这里来：
>
> | # | 决定 | 为什么 |
> |---|---|---|
> | N1 | **导航照 Unity/Tuanjie**：行左端是**独立的展开/收起三角**；**单击 = 选中 + 内容头跟随**（写 `selection`，同时 `active` = 命中项的目录：文件夹 → 自身，文件 → 父目录）；**双击 = 打开**（文件夹 → `Navigate` + 展开；文件 → 系统默认程序）。三角与行是两个命中区 | 参考编辑器的行为；「单击选、双击开」要求三角和行各管一件事。单击移动内容头是 Unity 的浏览行为（验收修正 #3/#4），双击仍只负责展开/打开 |
> | N2 | **可见行缓存归面板**（`Vec<NodeId>` + 失效键），core 只留 `visible_rows` 纯函数 | M6 的 query/scope 是面板视图状态，core 里的缓存服务不了它；放面板 = M6 不用搬 |
> | N3 | **TreeView 用最小 API**：不做 `renaming` / `drag_started` / `dropped_on` / 右键菜单通道；F2/Delete 归 M3 | 没有消费者的通道不可测，M3 一定会按真实需求重画 |
> | N4 | **M2 的打开 = 系统默认程序**（`open` crate）；IDE 探测 + `OpenWith` 分发 + `ui/ide_detection.rs` 搬迁归 M5 | `OpenWith` 是 `FileType` 的字段，M5 才有消费者；提前搬 311 行只会返工（2026-11-25：接口改为 `OpenDefault`/`OpenTarget`/`ResolvedTarget`，见 §2.14.3） |
> | N5 | **图标取自本机 Tuanjie 编辑器资源包**（`d_Folder Icon` / `d_FolderOpened Icon` / `d_DefaultAsset Icon`），Lanczos 缩到 64²，提交到 `Preferences/Textures/project/{,light/}`；重跑脚本 `scripts/extract_tuanjie_icons.py` | 现存 `Preferences/Textures/*.png` 是 2048²，而 egui_extras 的图片加载器忽略 size hint（不降采样）→ 一张 16 MB 纹理；64² 一张 16 KB。M2 文件行统一泛型 file 图标，按类型图标归 M5 |
> | N6 | **先定稿再开工**；M2 拆 **M2a**（core + widget + 纯映射，画面零变化）/ **M2b**（面板装配 + 皮肤 + 打开 + 键盘） | 与 M1 同款刀法：M2a 100% headless 可验收 |
>
> **补齐的 8 处**（原文空白或自相矛盾，在这里定死）：
>
> 1. **TreeView 的视觉入口**：widgets 新增 `TreeStyle`（行高/缩进/三角/图标/字号/颜色/选中底色/分隔线），component 把 `ProjectStyle` 映射成它。仓库没有 `Theme`，也没有第二个画树的 widget——**不要**为此发明主题系统。
> 2. **键盘焦点与滚动**：`TreeOutput::focused` 回答「树有没有键盘焦点」，面板只在 focused 时吃方向键；选中项移动后用 `TreeView::scroll_to(id)` 滚进可视区。关标签页再打开时，新面板实例首帧也用它对 `selection`（没有则 `active`）做一次揭示（验收修正 #2）。
> 3. **`tree_split` 的写回时机**：拖拽期间面板本地持有 `Option<f32>`（`None` = 跟随 core 值），且**每帧把 `Splitter` 改过的宽度存回它**——布局在 `show_in` 之前就算好了，不存回的话下一帧又用 core 的旧值重算，表现为「光标会变但拖不动」（验收修正 #1）；`drag_stopped` 时发**一条** `SetTreeSplit`，然后把本地值清回 `None` 跟随 core；reducer 里 clamp（0.15..=0.6，另有两侧像素下限）。**不要**每帧发 action。
> 4. **系统目录分隔线**：面板问 `ProjectTree::is_system_dir(path)`，在那一行上方画 1px；`Library` 默认折叠 = 初始 `expanded` 不含它，不写特例。
> 5. **状态栏语义**：整行在列分界处**分割**（分割线用 splitter 的 idle 色；暗色皮肤下 `status_bg` 与 `separator` 同色）。左半边（Hierarchy）的 `N items` = 当前 `active` 目录的**直接子项数**（不是可见行数，也不是全树节点数）+ `truncated` 告警；右半边（Content）预留给 M9 的图标大小滑条，当前只在报错时显示 `error`。**不显示选中路径**（内容头已经跟着单击更新，重复）。
> 6. **面包屑根**显示 `ProjectState.name`，每一段可点 = `Navigate`。
> 7. **工具栏只放 M2 真能用的**：`折叠全部` / `刷新`（= `ScanRequested{Manual}`）/ `隐藏文件`。新建/删除等按钮等 M3 有真实动作再加，不做灰按钮占位。
> 8. **面板 `scroll_bars()` 必须返回 `[false, false]`**：dock 默认会在面板外再套一层 `ScrollArea`（`docker.rs:1562`），而这里两栏各自滚动 + 通栏状态栏；Profiler/Preferences 是同一先例。

**M2a — core + widgets + 纯映射（1.5–2 天，画面零变化）**

- **core**（`ProjectAction` 新增，全部是纯状态迁移）：
  - `Select { path: Option<RelPath> }` —— 替换选择集，`None` 清空；不在树里的路径忽略；命中时把 `active` 设为该路径的目录（`ProjectTree::content_dir`：文件夹 → 自身，文件 → 父目录），所以单击就让内容头与状态栏跟随；
  - `ToggleFolder { path }` —— 三角 / `→` 的「展开」那一半；非文件夹或不存在则忽略；
  - `Navigate { path }` —— 双击文件夹 / 点面包屑：`active = path`、把 path 与其祖先放进 `expanded`、`selection = {path}`；
  - `OpenRequested { path }` —— 不改状态，只发 `ProjectEffect::Open { root, path }`；
  - `SetShowHidden(bool)`；`CollapseAll`（清 `expanded`，保留 `active`/`selection`）；
  - `SetTreeSplit(f32)` —— clamp 到 0.15..=0.6，NaN 忽略；
  - `Failed { message }` —— 宿主工作的可见失败（M2 先给「打开失败」用），写 `state.project.error`；
  - `ProjectEffect::Open { root, path }`（M2 只有「系统默认」一个实现）。
- **core 测试**：每个 action 一条以上（非法/不存在路径、clamp、NaN、`CollapseAll` 保留 active、`Navigate` 展开祖先、`Failed` 落 error）。
- **widgets**（`kairos_editor_widgets/src/tree.rs`）：契约见 §2.6 的 M2 定稿版（`TreeStyle` / `TreeRow` / `TreeOutput` / `TreeView`）。
- **widgets 测试**（headless，`Context::run` + 合成指针）：20 万行时行回调次数 ≤ 可见行数 + 2（虚拟化真的生效）；三角命中只报 `expander_toggled` 不报 `clicked`；行单击/双击；选中底色与分隔线出现在 painter 上；图标路径缺失不 panic。
- **component** `panels/project/rows.rs`（纯函数 + 单测，不碰 egui）：`Node + ProjectTree + ProjectView + ProjectIcons → TreeRow`；`suffix` = 扩展名（M5 换成 `FileTypeRegistry`）；行缓存 `ProjectRows { rows: Vec<NodeId>, key: (revision, show_hidden, expanded) }`。

**M2b — 面板装配 + 皮肤 + 打开 + 键盘（2–3 天，可见）**

- **component**：工具栏 20px（折叠全部 / 刷新两个文字段 + 隐藏文件眼睛开关，控件间 1 px chrome 线）→ `Splitter::vertical` 覆盖式分割左右（比例来自 `view.tree_split`，有像素下限）→ 左树 `TreeView` / 右栏内容头（独立灰底 20px + 面包屑）+ 空态 → 按两栏分割的状态栏 18px。`panels/project/style.rs` 的 `ProjectStyle`：
  - 继承 `ProjectWindowStyle.toml [hierachy]` 的 6 个 token：`file_header_color #c8c8c8`（文件夹与文件同色，Unity 2022 同款；legacy 的 `directory_header_color #dcdcb4` 淡黄不沿用）、`selected_file_background_color #3c4b64`、`file_suffix_color #787878`、`file_header_size 11`（后缀字号）、`file_icon_size 18`、`selected_file_background_corner_radius 4`；旧 TOML 里**没有**的（行高 20、缩进 14、工具栏、状态栏、面包屑、hover、分隔线、splitter 三色、内容头灰底与高度 `header_bg`/`header_height`）按 Console 的调色板与量测补齐；
  - 新增 `icons: ProjectIcons { folder, folder_open, file }`（dark/light 两套路径）+ `tab_icon`（`Preferences/Textures/tab_icons/project.png`）；
  - `impl ProjectStyle { fn tree(&self) -> TreeStyle }`；`#[derive(StyleFields)]` 让 Preferences 自动可编辑；文件 `Preferences/Styles/ProjectStyle.ron`。
- **app**：`ProjectEffect::Open` 实现（`open::that_detached`，失败 → `ProjectAction::Failed`），依赖 `open = "5.4.0"`（与旧 crate 同版本）；`describe()` 补新 action 文案；`is_quiet` 不需要新增（`SetTreeSplit` 每次拖拽只发一条）。
- **键盘**（仅 `TreeOutput::focused`）：↑↓ 移动选中（可见行序）、Home/End 首尾、`←` 已展开则折叠否则选父、`→` 未展开则展开否则选第一个子、Enter 打开选中项。
- **测试**：component 用 Console 的 harness 法（`Context::run` + 合成点击）断言「点行 → `Select`、点三角 → `ToggleFolder`、双击文件夹 → `Navigate`、双击文件 → `OpenRequested`」；`ProjectStyle` RON round-trip；空树 / 单行 / `Library` 分隔线三个装配 case。

**验收（手动 smoke）**：单击 `res/models` → 只选中，**内容头与左半状态栏立即跟到 `res/models`**（不展开、不导航）→ 双击 → 左树同步展开 → 双击 `Suzanne.png` → 系统默认程序打开 → 拖分割条比例正确、松手不回弹、右栏不溢出 → `Library` 排在最后、上方有 1px 分组线（**它只是分组线，不能拖**），展开看到 `Library/imported_assets` → **点右侧眼睛**后 `.meta`/点文件出现、图标从叉眼变睁眼且按钮保持选中底 → 工具栏刷新 → 日志出现 `project scan requested (Manual)` → 关掉 Project 面板再打开，展开/选中还在且树滚回选中行 → 20 万行全展开拖动滚动条不卡（Profiler 看 UI 线程 < 8ms；widget 测试里另有「行回调 ≤ 可见行数」的硬断言）。

**风险**：`TreeView` API 被 M3 的改名/拖拽撑破。缓解（已改）：M2 不预留空通道，但 widget 保持「行内绘制 + 全行命中区 + 三角独立命中区」，M3 加通道只改 widget 内部和一个调用点。

> **落地状态（M2 完成，验收逐条勾；含 2026-11-22 真机验收修正）**
>
> - **文件**：widgets 新增 `tree.rs`（虚拟化行控件 + 三角独立命中 + 焦点/滚动，11 条 headless 测试）；core 的 `ProjectAction` 新增 8 个变体 + `ProjectEffect::Open`，`reduce/project.rs` 8 个 arm，`ProjectTree::content_dir`，`tests/project.rs` +20 条（16 + 验收修正 4）；component 的 `panels/project.rs` 整篇替换（工具栏/分割/树/内容头/状态栏/键盘）与 `panels/project/{style,rows,content}.rs`（33 条测试，含合成点击、拖拽与绘制断言）；app 的 `ProjectEffect::Open` 真实实现（`open::that_detached`，失败回 `Failed` 落状态栏）+ `open = "5.4.4"`。
> - **测试**：`cargo test-crate kairos_editor_core` 38（project 切片）+ 其余全绿；`kairos_editor_widgets` 152（tree 11）；`kairos_editor_component` 215 + 17 集成；`cargo check --workspace --all-targets` 通过。
> - **运行（真机 smoke）**：启动日志 `project policy: 1 system dirs, 2 ignore rules` → `project scan #1: 1417 nodes (library 20) in 14 ms`；Project 面板显示工具栏（折叠全部 / 刷新 + 眼睛开关，控件间 chrome 线）、Unity 文件夹图标、灰底内容头、`34 items` 左半状态栏、可拖的垂直分割线。**验收轮复验（新构建）**：拖分割条 287 → 367 px 松手后保持；单击 `kairos_audio` → 内容头变 `KairosEngine / kairos_audio`、左半状态栏变 `2 items`；像素采样确认工具栏分割线、内容头 `#2F2F2F`、状态行分界线。**眼睛开关复验**：点一下 → 状态栏 `34 items` 变 `36 items`（根目录两个被隐藏的 dot 文件出现），按钮从叉眼（工具条底色）变睁眼（选中底 `#464646`），两态像素差 400 px。
> - **图标**：从本机 Tuanjie 2022.3 编辑器资源包抽 `d_Folder Icon` / `d_FolderOpened Icon` / `d_DefaultAsset Icon` 及 light 版，Lanczos 缩到 64²（<2 KB/张）提交在 `Preferences/Textures/project/`，重跑脚本 `scripts/extract_tuanjie_icons.py`，来源说明见该目录 README。**理由实测**：现存 `Preferences/Textures/*.png` 是 2048²，egui_extras 0.35 的加载器忽略 size hint（`ImageCrateLoader::load(..., _: SizeHint)`），一张就是 16 MB 纹理。
> - **皮肤**：`Preferences/Styles/ProjectStyle.ron`（dark + light，34 个 token）首次启动就地 seed；`#[derive(StyleFields)]` 让 Preferences 面板自动可编辑；「仓库里的 RON 必须能解析」有测试兜底（加 token 忘改 RON 会红）。
> - 已知行为与偏离：
>   1. legacy 的 `directory_header_color #dcdcb4`（淡黄文件夹文字）不沿用，文件夹与文件同色——Unity 2022 同款；M2b 的 token 说明已改。
>   2. `tree_split` 只在分割条 `drag_stopped` 时写一条 action（拖拽中的宽度是面板本地态）；双击分割条复位未做（M4 持久化时一起定）。
>   3. **egui 坑（记一笔）**：工具栏按钮用 `ui.put` 会把布局游标重锚到按钮高度（20）而不是预留的 21，所以面板 body 必须从工具栏返回的 row 下沿推导，不能用 `available_rect_before_wrap()`。测试里首帧也要先空跑一帧，egui 的命中测试依赖上一帧的 widget rect。
>   4. `Library` 排在根级最后、上方有 1 px 分组线（有测试覆盖）；**它是 Unity/Tuanjie 的系统目录分组线，不是分割条，不能拖**。它在可视区下方，需要滚动才看得到。
>   5. **面板重建（关标签页再打开）不保留滚动位置**，所以新实例首帧用 `selection`（没有则 `active`）喂 `TreeView::scroll_to` 补回；这条以前只在键盘移动时用。
>   6. 手动 smoke 的 4 项已在 2026-11-22 真机验收轮点过，并引出下面的 7 条修正；修正后拖分割条、单击联动、关闭重开定位都有 harness 测试兜底。
> - 未做（按计划）：M3 的建/改名/移动/删除、右键菜单与拖拽；M5 的类型图标与 IDE 打开；M6 搜索。
>
> **M2 验收修正（2026-11-22，真机 7 条反馈 + 隐藏文件眼睛 1 条追加）**
>
> | # | 反馈 | 处理与验证 |
> |---|---|---|
> | 1a | 左右分栏的分割条：光标会变，但拖不动 | 布局在 `Splitter::show_in` 之前就算好，拖拽结果又被丢掉 → 每帧把改过的宽度存回面板 `split`；release 仍只发一条 `SetTreeSplit`。harness 测试「按下 → 拖 60 px → 松开」断言下一帧内容头左沿 = 拖到的位置、release 只有一条 0.355。 |
> | 1b | `Library` 上方那条线拖不动 | 那是系统目录分组线（`separator_above`）；Unity/Tuanjie 同样不可拖，不是 bug（见已知行为 4）。 |
> | 2 | 关掉面板再打开，状态在但没定位到上次选中行 | 面板加 `reveal_selection` 首帧标志：把 `selection`（没有则 `active`）交给 `TreeView::scroll_to`；80 行测试断言重开后深行回到视口。 |
> | 3 | 单击选中不更新 Content Header 路径 | `ProjectTree::content_dir`（文件夹 → 自身，文件 → 父目录）+ reducer `Select` 写 `active`；core 4 条测试。 |
> | 4 | 单击选中不更新左下 `x items` | 同 #3：计数本来就是 `active` 目录的直接子项数，`active` 跟随单击后自然跟着变。 |
> | 5 | Content Header 与内容区同色，分不出 Header | 新增 `header_bg`（暗 `#2F2F2F` / 亮 `#BDBDBD`）+ `header_height 20`，底边 1 px chrome 线；绘制断言测试 + 像素采样确认。 |
> | 6 | 右下角路径多余；状态行该按两栏分割 | 删除选中路径；状态行在列分界处分割（线用 `splitter_idle`——暗色下 `status_bg` 与 `separator` 同色会看不见）；右半预留给 M9 图标大小滑条，当前只显示 error。 |
> | 7 | 工具栏三个按钮没有分割线、看不出是按钮 | 每个控件后 1 px chrome 线 + 右对齐组前一条（Console/Profiler 工具栏同规则）；绘制断言测试 + 像素采样确认（x=85/141 两条 + 眼睛前一条）。 |
> | 8 | 追加：`Hidden files` 改成 Unity/Tuanjie 的眼睛图标 | 取 Tuanjie 资源包的 `d_scenevis_visible@2x` / `d_scenevis_hidden@2x`（light 用无 `d_` 版）缩到 32²，开 = 睁眼 + 选中底、关 = 叉眼；**评估后不加计数**（图标即状态，计数随目录变，M6 搜索要占这条横带），tooltip 写动作；harness 测试断言没有文字、选中态有底色、再点回 `SetShowHidden(false)`；真机复验 34→36 items、两态像素 400 px 不同。 |

### M3 — 变更：建 / 改名 / 移动 / 删除（5–6 天，分 M3a / M3b 两刀）

**目标**：面板可以完整编辑文件系统，每个失败都看得见。

> **M3 开工前复核定稿（2026-11-22，用户已拍板）**——计划原文这几处会「现场发明」，实现一律按这里来（与 M1 的 R1–R5、M2 的 N1–N8 同性质）：
>
> | # | 决定 | 为什么现在必须定 |
> |---|---|---|
> | **T1** | **TreeView 补 M3 契约**：`TreeOutput` 增 `secondary_clicked` + `secondary_pos`、`drag_started`、`dragging`、`drop_target`；`TreeView::editing(id, &mut String)` + `edit_committed` / `edit_cancelled`；`TreeView::highlight(id)` 画落点高亮（M3b 验收修正 #2 再加 `highlight_region(id)` 与 `TreeRow::thumbnail`，#3 把拖拽升级为树级会话 + 自滚动；第三轮：区域框在文件夹行滚出视口后仍从视口顶边开始画） | 行的 `Response` 在 widget 内部就被消费（`tree.rs:329`），行内 `RowLayout` 是私有的 → 右键/行内改名/拖拽全部卡在这里；**没有这条契约，M3b 必然绕开 widget 自己画行** |
> | **T2** | **拖拽 = widget 只报交互，面板持有 payload / 幽灵 / 落点规则**；幽灵画在 `Order::Tooltip` 子层并用 `TSTransform` 跟随指针（落地为单视口下的 Area；M3b 验收修正 #2 落点改为「指针所在行的最近祖先文件夹」，#5 幽灵改成单行「图标 + 主名」） | widgets 不许懂领域类型（`RelPath`）；dock 的 tab 拖拽已是同款（`docker.rs:1320-1350`），照抄视觉、不抄它的一帧 consume 握手 |
> | **T3** | **行内重命名在 widget 内画**（`TreeView::editing`），buffer / 校验 / 错误文案归面板；提交取消照 `NumberField::draw_editor`（`number.rs:287-297`）：Enter 或失焦提交、Esc 还原；**校验失败不关闭编辑器**。**buffer 是主名（stem），不是全名**：行本身只显示主名（扩展名在后缀列），提交时面板把原后缀拼回去（文件夹不拼），所以改名不可能改掉文件类型 | 原文说「重命名编辑器由面板画」，但面板拿不到行内 rect（`RowLayout` 私有）；这是唯一会在写的时候推倒重来的地方。用户验收后补：文件名不展示后缀、改名锁后缀 |
> | **T4** | **`menu::context_menu` 新增封装**（`Popup::context_menu` + 同一套 `MenuConfig` / `UiStackInfo`，`show_popup` 改为共用它） | `show_popup` 现在是私有的；绕开它直接用 egui 默认弹层会得到第二套菜单样式 |
> | **T5** | **多选形状**：`Select { paths: Vec<RelPath>, mode: SelectMode }`，`SelectMode::{Replace, Toggle, Range}`；Range 的 `paths` 恰好是 `[anchor, focus]`，区间由 reducer 用 `visible_rows` 算；**只有单路径 Replace 移动 `active`**（单击联动内容头的 M2 行为不变；多路径 Replace = Ctrl+A，不动内容头）；**anchor 在 core 的 `ProjectView`**（M3b 验收修正 #7），`Range` 的 anchor 被折叠隐藏时退化为选中 focus 并重新 anchor | 破坏性改动：core 4 条测试 + 面板 3 处 `Select` 调用 + harness 断言一起改；Range 归 core 才可 headless 测 |
> | **T6** | **完成载荷完整**：`FsOpId` 计数器在 `ProjectState`；`FsOutcome { created, moved: Vec<(from, to)> }`；`FsOp` 还带 `companions`（成簇扩展名，来自类型表）；reducer 据此设 `focus_after_scan`，**并按每一对重写 `expanded` / `selection` / `active` 的前缀** | 原文只写 `Result<FsOutcome, String>`，没定义 `FsOutcome`；不重映射前缀 → 改目录名后它自己折叠（验收「改名目录后子孙仍可选中」只算半条） |
> | **T7** | **移动遇同级同名：面板预检（树快照）→ 阻塞式提示弹窗 → 不移动**；core 仍保留自嵌套 / 选中集去重 / 名字表硬校验；TOCTOU 由 `fs.rs` 失败回落状态栏错误 | 用户拍板「弹窗提醒、禁止移动」；也避免「搬一半」 |
> | **T8** | **create → 自动进入重命名**：面板本地 `pending_create: Option<RelPath>`，首个扫描完成后路径存在则开始编辑、否则清掉；编辑中路径消失则取消。**不进 core** | 验收要求它，但协议没有回传通道；按 M2 归属规则它是面板态（不该跨标签页复活） |
> | **T9** | **删除走工作线程**（复用 scan 的 poll 循环 + `FsCompleted`；实际用 `Vec<TrashJob>` 而不是单 job，并发删除不能互相取消），create/rename/move 同步。fs 边界规则：`create_new` 绝不 truncate 覆盖、拒绝 `..`、非 UTF-8 路径先做回环保守校验、Windows `rename` 目标存在必失败、仅大小写改名先实测再决定是否两段式 | `trash` 5.4 在 Windows 走 COM/`IFileOperation`（默认特性 `coinit_apartmentthreaded` 自己管 apartment）；放独立线程既避开主线程 COM 模式冲突，也让删大目录不卡宿主帧 |
> | **T10** | **键盘与 Reveal 补进交付**：F2 改名、Delete 删除、Ctrl+A 全选、Esc 取消；`ProjectEffect::Reveal`（Windows `explorer /select,`，其余 `open(目录)`）；「复制路径」纯面板 `ctx.copy_text` | M2 定稿 N3 明确写「F2/Delete 归 M3」，M3 原文漏了；菜单有「在资源管理器中显示」却没有 effect 实现 |
> | **T11** | **错误可见性复用状态栏右半边**，不再做第二条失败提示条。生命周期用 `ProjectError::clears_on_scan` 区分：扫描错误与一次性 host 失败（打开失败）由下一次干净 `Scanned` 清掉；**文件操作失败不被它自己触发的重扫清掉**，由下一次成功的操作清除 | M2 已经在那里画 `project.error`（`panels/project.rs:586-593`）；不加这一位的话，重命名失败的消息会被紧随的重扫在 2 帧内擦掉 |
> | **T12** | **测试补 widget / component 两层**：右键只报 `secondary_clicked`、拖拽生命周期、行内编辑三种结局（提交/取消/校验失败）、Ctrl/Shift 三种 `Select`、确认框 → `DeleteRequested`；harness 补 modifier / 右键 / 拖拽事件构造（现在 `press/release/key` 把 `Modifiers::NONE` 写死） | M3 最危险的代码原本没有一条测试 |
> | **T13** | **弹窗 = 新窗口件 + shell 共享皮肤 + 面板本地状态**：`widgets::modal` 包装 `egui::Modal`（不自绘背板/焦点陷阱；**标题条用共享的 `window_controls` 红色圆圈关闭按钮，和 Esc 是同一个 Dismissed，点背板不关**，M3b 验收修正 #1 + 第三轮关闭按钮）；`Preferences/Styles/Modal.ron` 经 `Frame::modal_style()` 下发（Preferences 页面自动出现）；对话框状态是**面板本地数据枚举**；**不引入 `core::ModalState`**（那是将来「退出确认 / 保存失败」的 app 级模态，届时复用同一个 widget） | 0.2.0 还没有任何弹窗设施；架构文档已规定做法（`editor-ui/README.md:235,295-324`、`widget-layer.md:132,137`：「modal 用 egui::Modal，只做样式/数据包装」）。M3 做掉其中可复用的那一半，不重写基础设施 |
> | **T14** | **`FileTypeRegistry` 数据表提前到 M3**：字段 `id / label / extensions / icon / companion / thumbnail / delete_confirm`（`open` 留 M5、`referenceable` 留 M7）；`ProjectState.file_types`；`visible_rows(tree, view, types)` 接管伴生隐藏（删掉 `tree::is_hidden_name`）；表里含 9 个家族 + 2 条 CreateEntry | M3 需要它的三个答案（成簇判据 / 删除确认 / 新建模板）；提前 = 消掉 3 处占位，并顺带解掉 M5 的「不动 tree.rs」与 `is_hidden_name` 位置的矛盾 |
> | **T15** | **Create 模型 = 意图表 + 类型表**：`CreateEntry { menu, label, default_stem, target: Folder \| File { extension, contents } }`；M3 只有两行：`Create ▸ Folder`（New Folder）、`Create ▸ Document ▸ Markdown`（New Document，内容 `"# "`）；**`.toml` 不建类型行、无创建入口**，未知类型走 fallback（泛型 file 图标 + 裸扩展名） | 用户拍板「先做 Folder 和 Markdown」；格式 ≠ 类型（`.mat` 也是 TOML），所以菜单挂在意图上，将来加类型只加一行数据 |
> | **T16** | **类型图标用本机 Tuanjie 内置资源**（实测 8/8 全有，dark + light；M3b 验收修正 #6：纹理行不再用纹理族图标，改为宿主缩略图缓存画纹理本身），按现有 `scripts/extract_tuanjie_icons.py` 抽到 `Preferences/Textures/project/types/{,light/}`；`document` 用 `d_TextAsset Icon`（**没有** `d_Document Icon`）、texture 用 `d_Texture Icon`（棋盘格）；**不手绘** | 用户拍板「先看 Tuanjie，没有才自己做」；实测全部命中（见下表） |
>
> **M3 定稿补充 1 — 弹窗的视觉与交互（已用真实 egui 输出做对照评审，选定 C）**
>
> `egui::Modal` 只提供居中 / 背板遮罩 / 输入拦截 / Esc / `should_close()`，**外观 100% 由我们的 token 决定**。评审渲染了 4 个方案（`0` = 不换 Frame 的 egui 默认，`A` = 菜单家族，`B` = 窗口家族 + 蓝确认，`C` = 窗口家族 + 中性按钮 + 红 Delete），**选定 C**，理由：
>
> - 编辑器里没有任何"填充蓝"的按钮先例（工具栏/面板按钮是透明 + hover 填充），蓝底会引入新视觉语言；
> - 红是唯一语义色（危险），危险按钮是 Dialog 的唯一强调点；
> - 标题条 + 1 px 描边 + 3 px 圆角 + 阴影让它和 dock 浮动窗口同族，而不是"一个大菜单"。
>
> C 的 token（`ModalStyle::dark()`，`widgets/src/modal.rs` 已实现，6 条 headless 测试）：背板 `black @ 47%`、主体 `#383838`、描边 `#1E1E1E 1px`、圆角 3、阴影 `(2,3) blur 12 @ 67%`、标题条 `#3C3C3C` 22 px + 底边 1 px `#2B2B2B`、标题 13 px `#DEDEDE`、正文 12 px `#B4B4B4`、内边距 `14/12`、按钮 24 px 高 · 圆角 2 · `#4A4A4A`/`#5A5A5A` + `#232323` 描边、危险 `#8A3B33`。交互：**Enter = 第一个非危险按钮**，危险按钮只能点；Esc / 点背板 = 取消（不动文件）；按钮顺序 `[Cancel, Delete]`。
>
> 两个已知坑（已写进实现与测试）：① **egui 会给新建 Area 做淡入**，首帧截图/断言必须把时钟跳过 fade，否则整层是半透明的（仓库的 menu 测试注释同款）；② **`egui::Modal` 只挡指针命中，挡不住 `ui.input()` 的原始键盘事件**，所以对话框打开时必须短路面板自己的键盘处理（行内改名、方向键、Delete），Esc 交给对话框消费；③ `egui::Modal` 默认居中且**没有拖动能力**，所以 widget 自带 `show_draggable`：标题条 `Sense::drag()` 改 Area 的 anchor 偏移，offset 由调用方持有（位置是视图态）、越界 clamp 到屏幕内，普通 `show` 仍居中。
>
> **M3 定稿补充 2 — 类型图标出处（Tuanjie 2022.3.62t6 实测）**
>
> | 家族 | 扩展名 | dark 资源名 | light | 说明 |
> |---|---|---|---|---|
> | texture | png | `d_Texture Icon` | `Texture Icon` | 棋盘格；不需要自绘 |
> | mesh | glb | `d_PrefabModel Icon` | `PrefabModel Icon` | Project 窗口用的模型图标（另有 `d_Mesh Icon` 偏 Inspector） |
> | material | mat | `d_Material Icon` | `Material Icon` | |
> | audio | ogg, wav, audio | `d_AudioClip Icon` | `AudioClip Icon` | |
> | shader | wgsl | `d_Shader Icon` | `Shader Icon` | |
> | script | rs | **项目自绘 Rust 图标**（§2.15） | 同左 | 页面外形沿用 `d_cs Script Icon`；`#` 换成 `R` + Unity 绿（dark `#276A2A` / light `#2E7D32`）（2026-11-25） |
> | document | md | `d_TextAsset Icon` | `TextAsset Icon` | **没有 `d_Document Icon`**；.md/.txt/.json 在 Unity 里就是 TextAsset |
> | font | ttf, otf | `d_Font Icon` | `Font Icon` | |
>
> 源尺寸 64² 或 256²，按现有脚本 Lanczos 到 64²（与已提交的 folder/file 图标同规格）；**脚本图标是例外**：Rust 版为项目自绘，不再从 bundle 抽取（§2.15）；落点 `Preferences/Textures/project/types/` + `.../types/light/`；仓库里"暗色皮肤只有一个"（`app.rs` 把主题钉死 Dark），light 版本先提交、暂不消费。
>
> **已落地**：脚本表 13→12 条（脚本图标 2026-11-25 改为项目自绘，不再抽取；其余 5 张通用 + 7 家族 ×2 皮肤），16 张 64² 图仍在 `Preferences/Textures/project/types{,/light}/`；`FileType.icon` 只存文件名，`ProjectStyle::type_icon_root` 给目录（dark/light 各一份），面板按类型表解析、未注册家族回退通用 file 图标；`Preferences/Textures/project/README.md` 记了 provenance 与重跑命令。
>
> **M3 定稿补充 3 — 两处文档自相矛盾的修正**
>
> 1. §2.6 原文「重命名编辑器、右键菜单、删除确认都由面板画」→ 改为：**右键菜单 / 确认弹窗由面板画**（面板有 `&mut Ui` 与 `UiOut`）；**行内重命名由 widget 画**（面板给 buffer 与校验），理由见 T3。
> 2. §6 风险表原文「M2 就留 `renaming`/`drag`/`dropped_on` 通道」→ 与 M2 定稿 N3（明确**没留**）矛盾，改为：「M3 一次性把契约补齐（T1）；撑不住就改 widget API，**不在面板里绕过 widget 自己画行**」。
>
> **两刀怎么切**
>
> | 刀 | 内容 | 验收方式 |
> |---|---|---|
> | **M3a（2.5–3 天，画面几乎不变）** | core：`file_type.rs`（T14）、`name.rs`（§2.5 校验表）、成簇判据、`FsOpId/FsOpKind/FsOutcome`、`Create/Rename/Move/Delete` reducer、多选形状（T5）、移动冲突/自嵌套/去重纯查询；app：`fs.rs`（create/rename/move + 成簇 + Windows 细节）、trash 工作线程、`Reveal`；widgets：树通道（T1/T2）、`modal.rs`、`menu::context_menu`；component：`shared_style/modal.rs` + `Frame::modal_style()`、`rows.rs` 改读注册表（标签/图标仍回退到泛型，视觉不变） | `cargo test-crate` 三个 crate 全绿；校验表逐条、tempdir 的 fs 矩阵、reducer 规则表、widget 交互测试 |
> | **M3b（2.5–3 天，可见）** | 右键菜单（Create ▸ Folder / Document ▸ Markdown、Rename、Copy Path、Reveal、Delete）、行内重命名、多选（Ctrl/Shift + 键盘 F2/Delete/Ctrl+A/Esc）、拖拽移动 + 幽灵 + 落点高亮、两个对话框（删除确认 / 移动被阻塞）、类型图标接入（T16）、真机 smoke | 手动 smoke 清单 + component harness（点击/拖拽/右键/确认） |
>
> 先把 M3a 做完的理由：风险最高、返工最贵的两块（widget 契约 + fs 层）恰好都不可见，各自有 headless 验收线之后，M3b 只剩装配与打磨；M2「一次做完再验收」返工 7 条的教训在这里。

**交付（M3a）**
- core：`FileTypeRegistry`（9 家族 + 2 条 CreateEntry，见 T14/T15）、`name.rs` 校验表、`Create/Rename/Move/Delete` 的 reducer 与 `FsOpId`/`FsOutcome`、多选 `Select`、移动预设校验（自嵌套 / 去重 / 同级冲突查询）；
- app：`fs.rs` 执行 create（`create_new` + 模板内容）/ rename / move，删除走 `trash`（失败则报错，绝不静默永久删除）；**删除在独立工作线程**（`trash` 在 Windows 走 COM/IFileOperation，线程名 `project-trash`，`Mode::Inline` 测试下同步跑），完成后仍以 `FsCompleted` 回 reducer，reducer 不需要区分它来自线程还是 syscall；成簇规则（伴生侧车随行，**只对文件生效**：目录不吃同名侧车，避免误删/误改名一个恰好叫 `foo.meta` 的无关文件）；每个操作完成 → `FsCompleted` → `ScanRequested { focus }`（delete 的 focus 是 `None`：路径已不存在，指向父目录等于替用户选中一个目录）；`Reveal`；
- widgets：`TreeOutput` / `TreeView` 的 M3 通道、`modal.rs`、`menu::context_menu`；
- component：`ModalStyle` 的 RON 皮肤与 `Frame::modal_style()`；`rows.rs` 走注册表。

**交付（M3b）**
- component：右键菜单、行内重命名、多选与键盘（含 Shift+↑↓ 扩展）、拖拽、删除确认弹窗、移动被阻塞弹窗、文件夹子树区域高亮、失败提示（复用状态栏）；纹理行画文件自己的宿主缩略图（M3b 验收修正 #6）；
- 资源：Tuanjie 类型图标抽取 + `ProjectStyle` 的图标路径接入；
- 文档/脚本：`scripts/extract_tuanjie_icons.py` 加一张表；`Preferences/Textures/project/README.md` 记录新来源。

**验收（手动 smoke，M3b 收口）**：树里文件名只显示主名、扩展名在右侧后缀列 → 建文件夹 → 自动进入重命名 → 输入含 `/` 被拒并提示（编辑器不关闭）→ 改名目录后子孙仍可选中且目录不折叠 → 改名 `Suzanne`（编辑器里看不到 `.png`）后文件仍是 `.png`、`Suzanne.png.meta` 同步改名 → 删除确认框可用标题条拖动、点弹窗外面**不会**关掉（Esc 或标题条红圈才关）→ Ctrl+点击多选、Shift+点击区间、Shift+↑/↓ 扩展 → 多选拖到另一个目录，幽灵是单行「图标 + 主名」（不换行、超长截断）、目标文件夹整棵可见子树有区域框 → 拖到展开文件夹的任意子行也能移入 → 拖到树的顶部/底部附近自动滚动（贴边越近越快、慢到能停在想要的行）、拖拽中滚轮仍能滚 → 把一个展开文件夹滚到视口上方，再拖到它的子行：区域框从视口顶边开始画 → 弹窗标题条左侧的红色圆圈关闭按钮（系统窗口按钮）与 Esc 都能关，点弹窗外仍不关 → 多选移动完成后**仍是多选** → 拖同名项 → 弹窗「禁止移动」且什么都没发生 → 纹理行（`.png`）显示纹理本身的缩略图，其余类型显示类型图标 → 删除弹确认、进回收站、回收站里能找回来（主文件与侧车都在）→ 只读目录上删除失败有提示而不是静默 → 关标签页再打开，多选/展开/anchor 还在（Shift+点击仍从上次选中的行开始）。

**测试**：校验表逐条（`name.rs`）；fs 操作在 tempdir（含目录带子孙、仅大小写改名、目标已存在、只读父目录、伴生侧车随行、模板内容、目录改名/删除不带走同名侧车）；reducer 的 generation/冲突/前缀重映射/删除去重；widget 的右键/拖拽/行内编辑；component 的菜单/确认/多选动作；`trash` 在 CI 上不可用（无桌面回收站的 Linux 容器）时测试按“失败且条目仍在”通过并注释原因——D5 禁止降级成永久删除，所以失败就是正确行为。

**风险**：Windows 语义（仅大小写改名、保留名、尾点空格、路径占用）全部收敛到 `name.rs` + `fs.rs`；widget 契约若被撑破，改 widget API 而不是在面板里绕开；`trash` 的 COM apartment 由独立工作线程隔离。

**已知行为（接受，不做抑制）**：每个 fs 操作会触发两次扫描（`FsCompleted` 一次 + watcher 一次）。M1 已有同类「一次写入被 debouncer 拆两批」的已知行为，两次扫描各约 10 ms、结果一致；要合并的话在 `ProjectService::poll` 侧收拢，留给 M6 之后按实测决定。

> **落地状态（M3 已收口，2026-11-22：M3a 四层 + M3b 面板 + 第二轮 7 条 + 第三轮 3 条验收修正）**
>
> - **第 1 批（已落地）**：`core::project::file_type`（9 家族 + 2 条 CreateEntry，`lookup` 最长扩展名优先、伴生/隐藏/确认/标签/图标查询）、`core::project::name`（8 条规则 + 边界）、`ProjectState.file_types`、`visible_rows(tree, view, types)`（删掉 `tree::is_hidden_name`）、`core::project::fs`（`FsOpId/FsOp/FsOpKind/CreateKind/FsOutcome`）、多选 `Select { paths, mode }`（Replace/Toggle/Range 三种 reducer 规则）、`Create/Rename/Move` reducer（名字表/同级判重/自嵌套/选中集去重/扩展名补齐）、`FsCompleted`（前缀重写 + 焦点 + 重扫 + `clears_on_scan` 错误生命周期）、`RevealRequested`、`app::project::fs`（create 用 `create_new`、rename/move 拒绝覆盖、任何平台都两段式做仅大小写改名、`.meta` 成簇随行、拒绝 `..`/非 UTF-8 回环），以及面板/行缓存/描述文案的调用点更新。
> - **第 2 批（已落地）**：
>   - **删除**：`ProjectAction::DeleteRequested`、`FsOpKind::Trash`、`FsOutcome.trashed`、reducer `delete_requested`（选中集去重、根节点不进回收站、trash 的 `focus` 为 `None` 让重扫自己剪 selection / 回退 active）、`app::project::fs::trash`（`trash 5.4.0`，失败报错绝不降级成永久删除；只对文件带侧车）、删除工作线程 `project-trash`（`trash` 在 Windows 的 COM apartment 由 crate 自己初始化，不把 host 线程切进那种模式）、`ProjectService::poll` 收完成（`Vec<TrashJob>` 允许并发删除、互不取消，完成仍是普通 `FsCompleted`）。
>   - **widgets 树通道（T1/T2/T3）**：`TreeOutput` 增 `secondary_clicked` / `secondary_pos`、`drag_started`、`dragging`、`drop_target`、`drag_stopped`、`edit_committed` / `edit_cancelled`（derive 因 `Pos2` 降为 `PartialEq`）；`TreeStyle` 增 `drop_fill` / `drop_stroke`（`ProjectStyle` 与 `ProjectStyle.ron` 同步补 `drop_fill` / `drop_stroke` / `drop_stroke_width`）；`TreeView::highlight(id)`、`TreeView::editing(id, &mut String)`（Enter/失焦提交、Esc 取消、校验失败保持编辑器并续编；处理了 egui lost-focus 是两帧信号导致的重复提交）。
>   - **`menu::context_menu(ui, style, salt, pos, open, content)`**：行 Response 在 widget 内部被消费，用不了 `Popup::context_menu`；改用 `Popup::new + PopupAnchor::Position + open_bool`，与 `show_popup` 共用同一套 `MenuStyle` / `MenuConfig` / `UiStackInfo` 装配（点菜单项不自动关、点外部或 Esc 由 egui 复位 `open`）。
>   - **弹窗皮肤（T13）**：`component::shared_style::ModalSkin`（36 个标量 token：Stroke → color+width、Shadow → offset/blur/spread+color、Margin/Vec2/CornerRadius 拆标量，`tokens()` 投影回 `widgets::modal::ModalStyle`）、`Preferences/Styles/Modal.ron`、`SharedStyles::modal()/modal_style()`、`Frame::modal_style()`（缺皮肤时回退 `ModalStyle::dark()`）；Preferences 页面自动出现，没有改 app 或面板。
> - **M3b 第 1 批（已落地，可见）**：行右键菜单（`menu::context_menu`，位置来自 widget 的 `secondary_clicked/secondary_pos`）——Create ▸ Folder / Document ▸ Markdown（菜单由类型表的 `CreateEntry` 驱动，创建后自动进入行内改名，T8）、Rename、Copy Path（`ctx.copy_text` 项目相对路径）、Reveal、Delete（先弹确认）；行内重命名（`TreeView::editing`：Enter/失焦提交、Esc 取消，名字表拒绝时**保持编辑器**并把原因画在状态栏右侧）；删除确认弹窗（`widgets::modal` + `Frame::modal_style()`，`[Cancel, Delete]`，Enter = Cancel，确认后发 `DeleteRequested`）；菜单 / 编辑器 / 对话框打开时短路面板键盘（T13 坑 2）。测试 +10：菜单绘制、Create 子菜单、改名提交、非法名保持打开、Esc 取消、待建行自动改名、确认框、Enter 取消、Copy Path、命令派发。
>   - **验收修正 1（弹窗可拖动）**：`widgets::modal` 增 `show_draggable(ui, style, &mut offset)` —— 标题条 `Sense::drag()` 拖动、`CursorIcon::Grab/Grabbing`、越界 clamp 保证按钮始终可达；offset 归调用方（对话框位置是视图态，重开面板不继承），删除确认框已接入。测试 +1（拖动跟手 + 越界 clamp）。
>   - **验收修正 2（文件名不展示后缀）**：树行只画主名、扩展名列在右侧后缀列（`rows::display_name`；文件夹保留全名，所以叫 `v1.2` 的文件夹不会变成 `v1`）；行内改名的 buffer 也只放主名，提交时面板把原后缀拼回去（文件夹不拼），于是**改名不可能改掉文件类型**；后缀仍是入口外的唯一真相（滤镜/图标 M5 用）。测试 +3（行只显示主名、文件改名保留 `.md`、显示名规则）。
> - **M3b 第 2 批（已落地，可见）**：多选与键盘（Ctrl+click = Toggle、Shift+click = 从 anchor 起的 Range、Shift+↑/↓/Home/End 扩展、F2 改名、Delete 删所选、Ctrl+A 全选可见行、Esc 清选择；**anchor 在 core 的 `ProjectView`**，随选择手势/创建/改名/删除/移动维护；多路径 Replace 不动内容头，core 只让单路径 Replace 移动 active）；拖拽移动（widget 报 drag_started/dragging/drop_target/drag_stopped，面板持 payload / 幽灵 / 落点规则；落点 = 指针所在行的最近祖先文件夹、必须非自身子树、同一父目录视为 no-op；`TreeView::highlight` + `highlight_region` 画行与子树区域框，目标是「指针所在行的最近祖先文件夹」）；幽灵是 tooltip 层的 Area（单视口下等价于 T2 的 TSTransform 方案，已注记）；移动被阻塞弹窗（T7：目标已有同名条目 → Modal 提示、什么都不动）；类型图标（T16：8 家族 dark/light 抽到 Preferences/Textures/project/types{,/light}，脚本表 13 条；FileType.icon 改为文件名 + ProjectStyle::type_icon_root 由皮肤给目录，dark/light 各指一份；面板按类型表解析、未注册家族回退通用 file 图标）。测试 +11（多选两种、F2/Delete/Ctrl+A/Esc、拖拽 move、幽灵+落点高亮、冲突弹窗、自嵌套不移动、按类型图标）。
> - **M3b 验收修正（2026-11-22 第二轮真机 7 条）**
>
> | # | 问题 | 修正 |
> |---|---|---|
> | 1 | 弹窗出现后点空白处会被关闭 | `widgets::modal` 不再把 backdrop click 当答案：只有 Esc（或 `Ui::close`）会 `Dismissed`，背板点击被吞掉。`ModalChoice::Dismissed` 的语义同步改成「关闭」。测试：背板点击不关、Esc 关。 |
> | 2 | 拖拽只能精确落在文件夹那一行；展开的文件夹子树占满视口后就够不到 | 落点改为**指针所在行的最近祖先文件夹**（Unity/Tuanjie 的行为）；widget 增 `TreeView::highlight_region(id)`，把文件夹行 + 可见子树画成一个区域框。测试：落在文件行 → 目标为其目录、区域框跨 3 行；widget 的区域/单行描边测试。 |
> | 3 | 拖到顶部/底部附近不能自动滚；拖拽中滚轮失效 | 拖拽升级为**树级会话**（egui memory）：源行被自动滚动移出可见范围后松手仍报 `drag_stopped`；拖拽贴边时 widget 自己滚 ScrollArea；egui 在 widget drag 期间忽略 wheel，widget 自己翻译 `smooth_scroll_delta`。测试：自动滚到源行消失后松手仍结束拖拽；拖拽中滚轮能滚到远端行。 |
> | 4 | 多选移动后变成单选 | `settle` 的 reveal 只在 focus **不在** selection 里时才替换选择：移动先把每个 `(from,to)` 前缀重写进 selection，reveal 的 focus 是最后一对的目的地、本来就在里面，于是整组选择保留；create 的 focus 是全新行，仍然替换。测试：两条多选移动后 scan 完成仍两条且 anchor = 最后移动项。 |
> | 5 | 幽灵只有换行文件名 | 幽灵改成**单行「图标 + 主名」**：图标与行一致（纹理用缩略图、文件夹用文件夹图标、多选用通用文件图标），主名超长截断不换行。测试：单条幽灵显示主名而不是 `readme.md`。 |
> | 6 | 纹理行统一用一个类型图标 | `FileType` 增 `thumbnail` 标记（只有 `texture` 家族为 true）；宿主新增 `HostView::project_thumbnail(ctx, root, relative)` 与 `app::project::thumbnail::Thumbnails`：工作线程 `image` 解码 + 32 px 下采样、UI 线程上传纹理、按绝对路径缓存、watcher 报外部变化时 `clear()`；行（`TreeRow.thumbnail`）与幽灵都用纹理本身，解码完成前回退族图标。测试：解码/非图片/超大文件/缓存生命周期 + 行投影 + 幽灵文本。 |
> | 7 | 新建 A 后在其内新建 B，Shift+点击 A 选到父目录和兄弟、排除 B | 根因是 anchor 存在**面板态**，创建/改名后没有更新，Shift 从过期行起算。修正：anchor 移入 core 的 `ProjectView.anchor`，由 `Replace`/`Toggle`/`Range`/`Navigate`/创建 reveal/删除修剪/前缀重映射共同维护；`Range` 的 anchor 被折叠隐藏时退化为选中 focus 并重新 anchor；键盘 ↑↓/Home/End 从 anchor 起算、Shift 走 Range；F2 以 anchor 为目标。测试：点击/切换 anchor、Escape 清 anchor、连续 Shift 保 anchor、隐藏 anchor 退化、创建后 anchor 跟到新行、嵌套新建后 Shift 选中 `{A,B}`、多选移动保多选、删除后 anchor 回退。 |
>
> - **M3b 验收修正 2（2026-11-22 第三轮真机 3 条）**
>
> | # | 问题 | 修正 |
> |---|---|---|
> | 8 | 文件夹行滚出 Hierarchy 视口后，落点区域框不显示 | widget 先按 `rows` 找到目标下标与深度；目标行在视口上方时区域框从视口顶边开始，并按可见行深度判断子树在视口内结束还是继续到视口底；折叠文件夹的区域框仍只有自己一行（`RegionBox.extend` 只在展开时允许越过最后一行）。测试：目标行滚出视口仍有一个从顶边开始的框；折叠文件夹的框等于其行。 |
> | 9 | 贴边自动滚太快、感受不到「越贴边越快」 | 速度改成按帧时间缩放（60/144 Hz 一致），最大 260 pt/s（60 Hz 约 4 px/帧），强度用进入边缘带深度的平方：刚进入几乎不动、贴到边才最快；边缘带 22 → 30 pt。测试沿用「自动滚到源行消失后松手仍结束拖拽」。 |
> | 10 | 既然 Esc 能关弹窗，就该有和它等价的关闭按钮 | 不用自绘 ×：标题条改用项目已有的系统窗口按钮 widget `window_controls`（与 dock 浮动窗口 / 颜色选择器同一个组件），红色圆圈关闭灯在标题条左侧，另外两盏保持灰色禁用（`WindowControlsEnabled::CLOSE_ONLY`）；点击红圈与 Esc 都返回 `ModalChoice::Dismissed`，drag 区域从三盏灯右侧开始。交通灯颜色/几何沿用共享 `WindowControlsStyle` 默认值，不新增皮肤 token（`ModalSkin` 仍是 36 个标量）。测试：红圈用共享 close 色画出，点击返回 `Dismissed`。 |
>
> - **M3 收口（2026-11-22）**：三轮真机验收反馈均已修正，M3 改动随本里程碑提交；下面的手动 smoke 清单保留为回归清单（不是未完成项）。
> - 测试：core 141 条（`project` 71，含删除 5 条、多路径选择不动内容头 1 条、第二轮 anchor/多选/移动保持 8 条）、app 70 条（`project::fs` 12 条 tempdir + `project::thumbnail` 4 条）、widgets 183 条（tree 28、menu 7、modal 10）、component 251 + 7 + 10 条（M3b 两批 +21，第二轮 +6）；`cargo check --workspace --all-targets` 绿；本轮改动文件的 `rustfmt --check`（`skip_children=true`）干净——仓库里还有 M0–M2 留下的未格式化文件，整 crate `cargo fmt` 会顺带改它们，本里程碑不纳入（也不把它们算进 diff）。`trash` 在无桌面回收站的 CI 上按“失败且条目仍在”通过并注释原因（D5 禁止降级成永久删除）。
> - 偏差记录：`FsOutcome` 用 `moved: Vec<(from, to)>` 而不是单个 `renamed`——多选移动需要重写每一对前缀；`FsOp` 增加 `companions` 字段，让 `app::project::fs` 不硬编码 `.meta`（成簇判据仍来自类型表）；**侧车只对文件生效**（目录是单个条目，避免误删/误改名恰好叫 `foo.meta` 的无关文件）；T9 原写“复用 scan 的单 job”，实际用 `Vec<TrashJob>`；`TreeOutput` 的拖拽通道固定 6 个字段（T1 原文的 `dragging` 是 bool、`drop_target` 只在拖拽中/松手帧有意义），行内编辑的 Enter/失焦/取消三种结局由 `edit_committed` / `edit_cancelled` 两个 bool 报，面板据此决定是否停止传 `editing`。 M3b 第二轮另记：弹窗只由 Esc 关闭（不跟 egui `should_close` 的背板点击）；纹理缩略图走 `HostView::project_thumbnail` 宿主 seam，而不是 `egui_extras` 的图片加载器（后者会把 4K 纹理全尺寸解码/上传，塞进 18 px 行图标）；拖拽会话与自滚动归 widget（egui 的 ScrollArea 在 widget drag 期间不处理滚轮）；选择 anchor 归 core 的 `ProjectView`，不是面板态。

### M4 — 编辑器布局持久化（2.5–3.5 天，D13）

**目标**：关掉编辑器再打开，**布局一模一样**（哪些面板开着、dock 怎么分、tab 顺序/焦点、浮动窗口在哪、Project 左右比例）；删掉布局文件或写坏它只是回到默认布局。**面板会话态（展开/选中/滚动/搜索）不落盘**（D13；原 M4 的 `project_view.toml` 方案已删除）。

> **M4 开工前复核定稿（2026-11-25，用户已拍板）** —— 原文这一节整体重写，落地一律按这里来：
>
> | # | 决定 | 为什么 |
> |---|---|---|
> | **L1** | **范围 = 全量 dock 布局**：打开的 panel 集合（由 dock 树推导）、主 surface 停靠树（分割方向/比例）、每 leaf 的 tab 顺序与 active tab、焦点、浮动窗口位置/尺寸/min/max；外加 **Project 面板 `tree_split`** | 布局是用户立刻感知的收益；D1 早已拍板「分栏比例进持久化」。面板会话态不存（D13） |
> | **L2** | **文件 `Library/editor_layout.toml`，`version = 1`**；TOML + 前序（preorder）节点表；面板身份是稳定 kind 字符串（将来多实例用 `kind#serial`）；**不设 root 守卫**（文件里没有项目路径，位置天然按根）；`.gitignore` 加 `/Library/editor_layout.toml*`（含原子写的隐藏临时文件）；读侧 64KB 上限 | 文件 KB 级、可读可 diff；按根是 D9 纪律；字符串身份才能在面板增删/版本升级时降级而不是崩 |
> | **L3** | **恢复规则**：boot 读一次 → 版本不认识 / 解析失败 / 节点形状非法 / **tab kind 不认识** → 整份忽略 + warn，退回 `default_dock`；合法但为空 → 尊重；由 tab 推导 `LayoutState.open`（不另存 open 列表）→ `PanelHost::sync` → 用 live 实例重建 `DockState` → 文件里没有但 layout 要开的 panel 追加到当前 leaf | 单一真值（dock 树）；未知 id 不崩；空布局可从 Window 菜单恢复 |
> | **L4** | **写回 = host 侧帧快照比较**（不是 `Effect::PersistLayout`）：`begin_frame` 取可持久化快照（不含每帧重算的 rect/viewport/scroll）→ 与上一帧比 → 500ms 去抖 → 临时文件 + `rename` 原子替换；`Layer::on_exit` flush；启动后的快照作基线（开机不回写）；失败只 warn | dock 的拖 tab / 分割 / 浮动窗口在 `Docker::show` 内部直接改 `DockState`，不产生 action；布局树本来就是 host 状态 |
> | **L5** | **docking 只出纯 Rust 的最小 API**：`Tree::from_nodes`（校验 heap 形状/空节点/active 越界/孤立节点）、`DockState::from_parts`、`DockState::surfaces()`；**不引 serde** | 文件格式归 app、docking 保持依赖轻（crate 文档本来就写「布局可被 app 序列化」）；恢复前必须校验，坏文件不能把 docker 搞 panic |
> | **L6** | **Project 面板**：`tree_split` 写进布局文件的 `[panels.project]`；**双击分割条复位 0.28**（M2 已知行为 #2 的遗留：widget 已有 `reset_to`，面板没接） | 一个标量，属于布局不属于「细节」；双击复位本次一起收口 |
> | **L7** | **原 M4 删除**：`ViewRestored` / `PersistView` / `Library/project_view.toml` / expanded+selection+anchor 落盘 / Ctrl+A 上限，全部从计划移除（D13） | 用户拍板「持久化不需要记录那么多细节」；这些属 `ViewStates`（可选、按需），将来真需要再立项 |
> | **L8** | **工期 2.5–3.5 天，一刀**；顺序：docking API + 测试 → app 布局读写/去抖/恢复协调 + 测试 → boot/`on_exit` 接线 → Project `tree_split` + 双击复位 → 手动 smoke → 落地状态 | 与 M1–M3 同款：先 headless 后接线；恢复协调是主要风险，必须有 app 集成测试 |
>
> **两条设计说明（会进 M8 的 ADR）**：
> 1. **持久化触发放在 host 侧**：文档草图里的 `Effect::PersistLayout` 假设布局树在 core；本实现里 dock 树归 host（`LayoutState` 只有 open/focus），拖拽/分割直接改 `DockState` 且不产生 action，所以由 app 在 `begin_frame` 取快照比较。这是对草图的合理偏离，不是遗漏。
> 2. **恢复失败一律整份回退**：坏文件、未知 version、未知 panel kind、节点形状非法都退回 `default_dock` + warn；只有「合法但为空」被尊重。v1 不做逐 tab 裁剪（那需要重排树，收益低）。

**交付**
- `kairos_editor_docking`：`tree.rs` 的 `Tree::from_nodes`（含形状校验）、`docker.rs` 的 `DockState::from_parts` / `DockState::surfaces()`；
- app `layout.rs`：`LayoutFile` DTO（serde + toml；`version` / `main` / `windows` / `focus` / `panels.project.tree_split`）、`load`（大小/版本/shape 校验）、`save`（原子写 + 头注释）、`LayoutPersist`（500ms 去抖、同值去重、`flush`）、`capture`（dock → DTO，跳过每帧数据）、`restore`/`dock`（校验 + tab 映射到 live `PanelInstanceId`）；
- `EditorApp`：`with_root` 启动顺序改造（读布局 → 决定 open 集合 → `boot_state` → `host.sync` → 重建 dock / 回退 `default_dock` → 追加缺失面板 → 快照基线）；`begin_frame` 的 observe/poll；`on_exit` flush；`EditorLayer` 实现 `Layer::on_exit`；
- `.gitignore`：`/Library/editor_layout.toml*`；
- 面板：`tree_split` 恢复（boot 写 `state.project.view.tree_split`，clamp 到 0.15..=0.6）+ 双击分割条复位（发一条 `SetTreeSplit(0.28)`）；
- 文档：本 README 的 §2.7 / §2.10 / D1 / D3 / D13 / §8 / §9 / M8，以及 `rewrite-plan.md` Phase 4 的交叉注记。

**验收（手动 smoke）**
- 重摆布局（左右分栏 + 挪 tab + 拖比例 + 浮动窗口 + 切 active tab）→ 退出重启 → 完全一致；
- 关掉一个面板 → 重启仍关；从 Window 菜单开新面板 → 重启后仍在；
- 把 `Library/editor_layout.toml` 改成乱码 / 未知 version → 默认布局启动 + 一条 warn，不阻塞；
- 文件里写一个不存在的 panel kind → 整份忽略退回默认（v1 的降级策略）；
- 把浮动窗口坐标改成屏外 → 重启后仍在可视区；
- 退出前 200ms 内改完布局 → 立即关窗 → 重启仍是新布局（`on_exit` flush）；
- 删掉布局文件 → 默认布局；把面板全关掉 → 重启仍是空的，Window 菜单能重开；
- `git status` 看不到布局文件；写布局文件时 console 的 `scan #` 不变（`Library/` 被 watcher 过滤）；
- Project 面板左右比例重启保持；双击分割条回 `0.28`。

**测试**
- docking：`from_nodes` 合法/非法矩阵（根 Empty、leaf 无 tab、active 越界、孤立非空节点、子节点缺失）、`from_parts`（main 必须在 0、空输入拒绝）、`surfaces()` 次序；用真实 dock 操作建树再快照/重建，断言 tab 顺序、active、分割方向/比例、窗口几何；
- app：DTO round-trip；坏 TOML / 版本不认识 / 超过大小上限 / 节点形状非法 → 默认布局 + warn；未知 kind 整份忽略；空布局合法；`capture` 不含 rect/viewport/scroll；假 `Instant` 的去抖（500ms 前不写、之后写一次、同值去重、`flush` 立即写）；原子写不留半文件；boot 集成测试（tempdir 预写布局 → `EditorApp::with_root` 后断言 dock 树/tab/焦点/窗口/`tree_split`）；保存路径集成测试（改 dock → `begin_frame` → `on_exit` → 文件内容）；
- component：双击分割条发一条 `SetTreeSplit(0.28)` 且下一帧分界在 0.28；现有拖拽/单击测试全绿；
- 回归：现有 app/core/widgets/component 测试全绿。

**风险**
- 恢复协调是主要复杂度：未知/重名 tab、空布局、版本升级新增面板、窗口坐标跨显示器 → 用「整份忽略 + 追加缺失 + egui constrain」三条兜底，不猜；
- dock 是 host 状态，持久化触发与文档草图 `Effect::PersistLayout` 不同 → 记录理由（L4），M8 的 ADR 写清楚；
- app crate 无 lib target → 验证用 `cargo test -p kairos_editor_app --bins`（不是 `cargo test-crate`）。

> **落地状态（实现完成，2026-11-25；待真机验收）**
>
> - **实现**：`kairos_editor_docking` 增 `Tree::from_nodes`（heap 形状/空节点/active/孤立节点校验 + 节点上限）、`DockState::from_parts` / `DockState::surfaces`；app 新增 `layout.rs`（`LayoutFile` DTO：`version` / `main` / `windows` / `focus` / `panels.project.tree_split`，前序节点表；`load` / `save` 原子写 + 64KB 上限 + 文件头注释；`capture` / `restore`；`RestoredLayout::dock`；`LayoutPersist` 500ms 去抖 + 同值去重 + `flush`）；`EditorApp::with_root` 改为「读布局 → 决定 open 集合 → `boot_state` → `host.sync` → 重建 dock / 回退 `default_dock` → `reconcile_dock` → 快照基线」，`begin_frame` observe/poll，`on_exit` flush；`EditorLayer` 实现 `Layer::on_exit`；`.gitignore` 加 `/Library/editor_layout.toml*`；Project 面板接 `Splitter::reset_to` + 双击发一条 `SetTreeSplit(0.28)`；`ProjectView` / `ProjectState` 的文档改为运行期状态（D13）。
> - **测试**：docking `dock_state` 6→10（快照/重建 round-trip、4 条非法节点矩阵、`from_parts` 顺序/空输入）；app 96 passed + 1 ignored（`layout` 模块 14 条：capture/restore round-trip、空 dock、未知/重复 panel、malformed tree、版本、无 live 实例、split 与 tree_split clamp、磁盘 round-trip、坏文件、去抖/flush、disabled writer；app 集成 5 条：boot 恢复、坏文件回退、干净启动不写文件、退出 flush、缺失 tab 追加）；component 251→252（双击复位）；core 行为未改；`cargo check --workspace --all-targets` 绿；本轮改动文件 `rustfmt --edition 2024 --config skip_children=true` 干净（仓库里仍有更早的未格式化文件，不纳入）。
> - **与定稿的偏差 / 已知行为**：
>   1. 布局文件里的 panel 身份只写 kind（`"scene"`）。当前 `LayoutState` 每种 panel 只有一个实例，serial 跨重启无法稳定对齐；将来多实例时再写 `kind#serial`，加载按「精确 serial → 同 kind 首个」回退（`layout.rs` 注释记录）。
>   2. v1 的恢复失败策略是**整份回退**（未知 kind / 坏树 / 版本不认识都丢），不做逐 tab 裁剪——简单且不会静默改变布局。
>   3. `begin_frame` 每帧取快照（真实工程约 10 个 leaf，成本可忽略）；去重用「上次观察到的快照」，只有变化才重启 500ms 去抖。
>   4. 快照基线来自实际 dock：没有布局文件的干净启动**不会**生成文件（有测试），第一次打开不会把默认布局写进仓库。
>   5. `About` / `Preferences` 等浮动窗口也随布局保存/恢复（不按面板做特例）；「关闭全部面板」的合法空布局被尊重，可用 Window 菜单重开。
>   6. 布局文件写在 `Library/`，watcher 按系统目录过滤，不会自激重扫（M1 的系统目录事件测试已覆盖；M4 未新增 watcher 测试）。
> - **待办**：真机 smoke（上面 9 条验收清单）。本轮验证全部走 headless / 集成测试，未启动 GUI。

### M5 — 类型 seam + 打开方式（2026-11-25 拆 M5a / M5b；2–2.5 天 + 2.5–3.5 天 + 每类 0.5–2 天）

**目标**：加一个类型不再触碰树和面板；同时把「文件用哪个程序打开」从隐式嗅探变成 External Tools 设置（§2.14）。

> **M5 修订（2026-11-25，新需求）**：原 M5 只写「`OpenWith` 五类 + IDE 探测从旧 crate 搬来」；按 §2.14 拆成两刀：**M5a**（core 解析 + app 嗅探/启动/设置，画面无变化）、**M5b**（External Tools 面板，可见）。M5a 先做，因为面板只是解析结果的视图。**P1–P3 已于 2026-11-25 拍板，M5a / M5b 均可开工。**

**M5a — 打开分发 + 嗅探 + 设置（2–2.5 天）**

**交付**
- core：`FileType::open: OpenDefault`（§2.8 / §2.14.3）；`external_tools` 切片（`ExternalToolsState` / `ExternalToolsAction` / `ExternalToolsEffect` / reducer）与 `effective_open()` 纯解析；`ProjectEffect::Open { root, path, target: ResolvedTarget }`；`referenceable` 仍留 M7；
- app：`external_tools/mod.rs`（boot 读设置 + 起检测、`poll` 收检测/浏览/启动结果、`execute` 执行 effect）、`detect.rs`（旧 `ui/ide_detection.rs` 的 7 个候选 + **RustRover**；工作线程；Windows `CREATE_NO_WINDOW`；无 `OnceLock`）、`settings.rs`（`Library/external_tools.toml` DTO + 原子写 + 版本/大小/坏文件回退）、`launch.rs`（`SystemDefault` / `SystemPicker` / `Program` 三条路径，`$FILE` 按 argv 替换、不拼 shell）；`Browse…` 用 `native-dialog`（P2 已拍板）；
- app 启动顺序：读设置 → `ExternalToolsAction::Loaded` → 起检测 → 原有 `PolicyLoaded` / `ScanRequested`；打开 effect 的失败仍回 `ProjectAction::Failed`；
- `.gitignore`：加 `/Library/external_tools.toml*`（与布局文件同规则：可删、可重建、不进 git）；
- 测试：§2.14.10 的 core / app 清单。

**验收**：headless 全绿；未配置时 `.rs` 用嗅探到的 IDE 打开、无 IDE 时弹系统选择器；未知类型弹系统「打开方式」；把 `Library/external_tools.toml` 手写一条覆盖，打开行为随之改变；删掉文件回默认。

**M5b — External Tools 面板（2.5–3.5 天）**

**交付**
- component：`panels/external_tools.rs`（检测列表 + 每类型表 + 未知类型开关 + 错误行，§2.14.7）、`panels/external_tools/style.rs` + `Preferences/Styles/ExternalToolsPanel.ron`（`#[derive(StyleRow)]`，dark/light）；`#[crate::panel(EXTERNAL_TOOLS = "external_tools", skin = ExternalToolsPanelStyle)]`；`panels.rs` 增加 `mod` / `pub use`；
- toolbar：Edit 菜单加 `External Tools` → `ShowPanel`；工具栏测试补一条「点 Edit → External Tools 产出 ShowPanel」；
- app：面板的动作接进 inbox（检测 / 浏览完成 / 设置写盘），保存失败进面板错误；
- 测试：面板 harness（行渲染、menu 选择、Reset、Detect、浏览取消、错误态）、`Edit` 菜单、设置的启动集成（tempdir 预写文件 → boot 后解析生效）。

**验收**：§2.14.10 的手动 smoke 清单；`cargo test-crate kairos_editor_core` / `kairos_editor_widgets` / `kairos_editor_component` 绿；`cargo test -p kairos_editor_app --bins` 绿；`cargo check --workspace --all-targets` 绿。

- **之后每类一个任务**：纹理（缩略图 + `.meta` 是否有 / 是否已导入）、网格（预览）、材质（Inspector）、音频（试听）、着色器/代码（IDE）、文档（Markdown 只读）。凡依赖「文档/预览」的能力，都依赖 Inspector 与视口迁移，不在本计划排期。**每类任务把该类型的 `OpenDefault` 从 `System(..)` 改成 `Internal(..)` 即可，面板与解析器不改。**

**原文其余交付（保留）**：component：未注册类型显示通用图标（不隐藏）；「只看资产」开关（用注册表过滤，D2）；M5 的类型图标/新家族数据沿用 T16 的 `FileType.icon` + `ProjectStyle::type_icon_root`，不为 External Tools 新增图标。

**验收（类型 seam）**：测试替身注册一个 `foo` 类型 → 树里出现带图标的行 → `Enter` 走它自己的处理器 → 删掉这个类型后行为回到通用文件。整个过程不改 `tree.rs` / `panels/project.rs`。

### M6 — 搜索与过滤语法（2–3 天）

**交付**：`core::project::search` 的查询解析器（§2.9 语法：子串 / `t:` / `p:` / `-` 排除，纯函数 + 表驱动测试）、搜索框（Ctrl+F / Esc）+ **范围选择器（In Assets / In Library，Unity 同款）**、命中项祖先自动展开、过滤时禁用「折叠祖先」、结果计数进状态栏、类型过滤 chips（M5 的 `FileTypeRegistry` 驱动）。

**验收**：`t:mat` 只剩材质、`t:texture -t:meta` 排除伴生文件、`res/mod` 走路径匹配（改一个字母就不匹配）、`In Assets` 搜不到 `Library` 里的文件而 `In Library` 只搜那里、10 万节点下单字符输入单帧完成过滤（无去抖、无卡顿）、`Esc` 后展开状态回到过滤前、过滤中点刷新不丢输入。

### M7 — 资产身份与引用维护（5–8 天，D11 并入）

**目标**：改名/移动后引用不断；身份写在资产旁的 `.meta` 里，索引是派生物（§2.11.3）。

**交付**

1. **`kairos_asset`（+ ADR 0016）**
   - `AssetMeta` 增加 `guid: Option<AssetGuid>`（`#[serde(default)]`，为空时 RON 省略）；`AssetMetaMinimal` 同样带可选 `guid` 并暴露「不依赖 loader 类型」的读取 API；
   - `AssetGuid(uuid::Uuid)`（`kairos_asset` 已依赖 `uuid`，无需新依赖）；**不复用**旧编辑器的 `Guid`（旧 crate 冻结）；
   - 写回策略：只 patch `guid` 字段的 read-modify-write + 原子替换，避免和 processor 写 `processed_info` 打架（ADR 要写明「单一字段单一写者」）；
   - 分配时机：`ensure_guid(path)` 在三种情况下调用 —— processor 首次写侧车、编辑器首次创建对某资产的引用、用户显式「固定身份」（可选）。
2. **`kairos_editor_app::project::identity`**
   - 扫描时顺带只读侧车，收集 `guid → path`；索引缓存 `Library/asset_index.ron`（**可删可重建，权威源永远是侧车**，与 §2.7.2 同一套纪律）；
   - 改名/移动后侧车随行（M3 的成簇规则），索引按扫描增量更新；guid 冲突（两份侧车同 guid）→ 后到者按 path 处理并 warn。
3. **引用格式（`kairos_graphics` 材质）**
   - 纹理槽从 `path: String` 升级为 `AssetRef { guid: Option<AssetGuid>, path: String }`；
   - 解析顺序：guid 命中 → 索引路径；guid 缺失/失效 → path；都失败 → 记 `missing` 并显示，不 panic；
   - 旧 `.mat` 无需迁移：只有 path 的旧数据按 path 读，保存时补 guid。
4. **UI**：Inspector 的引用行区分「已解析 / 回退到路径 / 缺失」三态；缺失只标红，不阻塞打开。

**验收**
- 改名 `Suzanne.png` → 材质引用仍解析到新路径（集成测试用 tempdir + 真 `.mat`）；
- 删掉 `.meta` → 仍按 path 解析；
- 材质里 path 写错但 guid 对 → 仍能解析；guid 与 path 都错 → Inspector 标 `missing` 且不崩；
- 删掉 `Library/asset_index.ron` → 重建后结果一致。

**测试**：侧车 guid 往返；旧侧车（无 guid）可读；guid 冲突；改名后引用存活；侧车丢失回退；索引重建幂等。

**风险**：与 processor 的并发写（用「只 patch 自己字段」+ 原子替换收敛）；给非资产文件建侧车的噪音（只对 `FileType::referenceable` 且「被引用时」才建）。

### M8 — 冻结旧实现 + 文档（0.5–1 天）

**D8 定稿：旧 crate 整体退役时再删，本计划一行旧代码都不动。** 所以 M8 只做两件事：让人不再往旧实现加东西，以及把决定写下来。

**交付**
- 新 ADR：`0015-project-tree-and-panel.md`（路径身份、扫描协议、忽略规则、**布局文件格式与恢复规则（含 host 侧持久化触发的理由）**、类型注册表、删除语义）；
- 新 ADR：`0016-asset-guid-and-references.md`（M7 的身份与引用；与 ADR 0002 的 meta 格式如何兼容）；
- 新 ADR：`0019-external-tools-and-open-targets.md`（External Tools 设置文件格式、打开目标解析优先级、嗅探协议、系统选择器的平台能力矩阵）；
- 交叉链接 ADR 0017（M0.5 的成品根布局）到 ADR 0004/0005 与 `rewrite-plan.md`；
- `rewrite-plan.md` 的模块映射表加一行：旧 project 子树已由 `core::project` + `app::project` + `panels::project` 取代，随 `kairos_editor` 退役删除；
- `docs/plan/editor-ui-architecture/README.md` 的面板清单把 Project 标为已迁移；`kairos_editor/CONTEXT.md` 加 §2.13 词表；
- `docs/research` 里引用旧项目窗口的地方标注「已被 docs/plan/project-panel 取代」。

**验收**：三份 ADR 合入；仓库里只剩文档层面的「旧实现在哪、为什么不删」说明，没有新的旧代码改动；`cargo check --workspace --all-targets` 通过（本阶段不改代码，跑它是为了确认前几个里程碑没留编译债）。

---

## 4. 测试策略

沿用仓库现状：没有 `egui_kittest`，组件测试用裸 `egui::Context` + 合成指针输入（参照 `kairos_editor_component/tests/panel_host.rs`）。

| 层 | 测什么 | 怎么测 | 成本 |
|---|---|---|---|
| core | 树组装、可见行、reducer 规则、名字校验、忽略规则匹配、查询解析、view 序列化 | 纯函数 + fixture `Vec<ScanEntry>` | 毫秒级 |
| app | 扫描器（含忽略与系统目录预算）、watcher 去抖、fs 操作、持久化、`.kairosignore` 读取与 seed | `tempfile` + 真实文件系统 | 十毫秒级 |
| asset/graphics | guid 侧车往返、改名后引用仍解析、缺失回退（M7）；成品根新路径下 processor/loader 仍工作（M0.5） | tempdir + 真 `.mat`/`.meta` | 十毫秒级 |
| widgets | 行绘制、展开三角命中、选中、键盘、改名提交/取消 | `Context::run` + 合成事件 | 毫秒级 |
| component | 面板装配：点击 → action、双击 → 导航、右键 → 菜单项；External Tools 的检测列表/打开方式表 | `Context::run` + 断言 `UiOut` | 毫秒级 |
| external tools | 打开目标解析（覆盖/嗅探/默认/未知/缺失路径）、设置文件 round-trip 与坏文件、嗅探候选、启动 argv | core 纯函数表 + `tempfile` + 临时可执行文件写 marker | 毫秒–十毫秒级 |

**断言风格**：不测像素。组件层断言「这次点击产生了 `Action::Project(Select { .. })`」，因为这正是 `UiOut` 存在的意义（`frame.rs:194`）。

**必须补的三类回归测试**（对应旧实现的缺陷表）：静默失败的目录、目录改名后子孙路径、同名判重用全名。

---

## 5. 性能预算

| 指标 | 目标 | 怎么保证 |
|---|---|---|
| 扫描 1 万节点 | < 50ms（release） | 后台线程，一次 `read_dir` + `entry.file_type()`，不额外 `metadata`；被忽略目录直接不进遍历 |
| 扫描 10 万节点 | < 500ms；> 20 万截断 | `ScanPolicy::max_nodes` + 状态栏告警 |
| `Library` 子树 | 独立预算（默认 2 万节点），超出不再深入 | 它是产物目录，不吃主扫描预算；截断只在展开时提示 |
| 面板单帧（全展开 10 万节点） | < 8ms | 只画可见行；可见行缓存按（generation, expanded, filter）失效；过滤 O(n) 单趟 |
| 搜索输入延迟 | 单帧内完成，无去抖 | 同上；行 id 建树时算好 |
| 打开方式解析 | 单次 O(类型数 + 检测数) | 纯函数；只在打开/面板绘制时算 |
| IDE 嗅探（启动 / Detect） | 不占 UI 帧；单次 < 1s（8 个候选的 `--version` + 固定路径 stat） | 工作线程 + 每候选超时 + 结果只存内存（不落盘） |
| 内存（10 万节点） | < 20MB | `Node` + 路径字符串；相对路径不重复存绝对前缀 |

**怎么量**：5 万 / 20 万节点的耗时用 `#[ignore]` 的一次性脚本（或 `examples/`）在 tempdir 里建好 fixture 后只计扫描时间——建 fixture 比扫它慢一个数量级，**不要放进日常 `cargo test-crate`**。M1 收尾量一次，把实测数字补到本表下面。

**M1 实测**（dev profile，`opt-level = 1`，Windows NTFS，tempdir fixture）：**50,251 节点 42 ms**。对应命令：
`cargo test -p kairos_editor_app --bins -- --ignored fifty_thousand --nocapture`（建 fixture 本身约 18 s，所以不进日常测试）。

**留的扩展点**：`ScanPolicy` 已有 `max_depth`/`follow_dir_symlinks`；若实测网络盘不可接受，把 `Scan` 换成「首屏只扫一层 + 展开时补扫」，接口不变（`ScanRequested { reason: Lazy(dir) }` 已预留 `reason`）。

---

## 6. 风险与缓解

| 风险 | 触发迹象 | 缓解 |
|---|---|---|
| TreeView 抽象撑不住改名/拖拽/多选 | 面板里开始「绕开 widget 自己画行」 | M2 定稿 N3 当时**没留**空通道（与本节旧文矛盾，M3 定稿补充 3 已修正）；M3 一次性补齐契约（T1/T2/T3），撑不住就改 widget API，不在面板里绕过 |
| 扫描拖慢启动 | 网络盘/巨量目录下首帧 > 1s | 后台扫 + 截断 + `elapsed_ms` 日志；M1 就用 5 万节点 fixture 验 |
| Windows 文件语义踩坑 | 仅大小写改名失败、保留名被系统拒 | 全部收敛到 `name.rs`/`fs.rs`；每条规则一条测试 |
| watcher 自激（自己写自己） | 每次刷新都触发下一次刷新 | watcher 按路径前缀**丢掉 `Library/` 下的事件**（缓存、日志、视图文件都在那里）+ generation + 300ms 去抖 |
| 布局恢复把面板搞乱/搞丢 | 重启后面板不见了、位置错乱 | 未知 kind / 坏文件 / 版本不认识整份退回 `default_dock`；合法空布局尊重；文件里没有的面板追加到当前 leaf；启动快照作基线（开机不回写） |
| core 长回 IO/锁 | review 时看到 `std::fs`/`Mutex`/`Cell`/`Receiver` | 写进 review 检查单：出现即打回 |
| 两套实现长期分叉 | 旧 crate 里还在修 project 的 bug | 旧实现冻结；M7 删除。期间**不在 `kairos_editor/src/project*` 改任何东西** |
| 非 UTF-8 文件名 | 显示成 `?`，操作报错 | 明确限制：显示用 lossy，操作前校验「可表示」，不可表示则给明确错误（不静默） |
| 范围蔓延到资产导入 | 讨论「自动生成 .meta」「缩略图缓存」 | 属 M5 类型任务且依赖 processor；本计划只做「显示与打开」 |
| 嗅探在启动时闪控制台/拖慢启动 | Windows 弹黑框、启动卡顿 | 工作线程 + `CREATE_NO_WINDOW` + 候选超时；结果不落盘、面板可「Detect」重测 |
| 打开方式设置静默丢失/静默回退 | 重启后回默认、配置路径不存在却“成功”打开 | 原子写 + 写失败进面板错误；缺失路径黄字且打开报错，不静默回退 |
| 打开命令拼接引号注入 | 路径/参数含空格、引号 | argv 直传 + `$FILE` 按 argv 元素替换，绝不拼 shell 字符串 |
| 项目内容注入可执行路径 | 打开项目就触发外部进程 | 设置只从 `Library/`（不提交、不进资产源）读；打开必须由用户手势触发 |

---

## 7. 决策记录（已拍板）

| # | 决定 | 一句话理由 | 落在哪 |
|---|---|---|---|
| **D1** | **左右两栏，右栏先空** | 布局/分割条/导航状态一次到位，网格只填内容不返工 | `ProjectView::tree_split`（运行期）+ M4 布局文件持久化、M2 面板骨架 |
| **D2** | **单根；`Library` 是系统目录；忽略走 `.kairosignore`；搜索分 `In Assets`/`In Library`** | 系统目录、忽略、隐藏是三件事，混在一起会既慢又说不清；搜索范围照 Unity 分档最直观 | `ScanPolicy::{system_dirs,ignore}` + `ProjectTree::ordered_children` + `.kairosignore` + `SearchScope`（M6） |
| **D3** | **持久化只存编辑器布局，不持久化项目树** | `asset_registry.toml` 是身份表不是缓存，一条 `read_dir` 也省不掉（§2.7.1）；布局是用户可感知的收益 | `Library/editor_layout.toml`（M4，D13）；项目树永不落盘；索引缓存按需（§2.7.2） |
| **D4** | **删编辑器注册表；资产身份搬进 `.meta`** | 引用是路径、注册表只服务 UI；改名安全需要资产侧 ID（§2.11） | M1 起不再产出 `Library/asset_registry.toml`；M3 埋成簇地基；身份本体 = M7 |
| **D5** | **回收站 + 默认确认（Unity 行为）** | 误删不可逆；确认弹层是 UI 状态不是宿主对话框 | `FileType::delete_confirm`（默认 true）、M3 |
| **D6** | **v1 单选，M3 多选 + 树内移动** | 跨面板拖要资产句柄，属 M5 之后 | M3 |
| **D7** | **搜索含 `t:` 等过滤语法** | 名字子串不够用；类型过滤是资产工作流的日常动作 | §2.9 语法、`core::project::search`、M6 |
| **D8** | **旧代码等到整体退役再删** | 冻结 + 文档足够防止分叉，删半个 crate 反而有编译风险 | M8 只改文档 |
| **D9** | **保持 cwd，但守三条纪律** | 面板侧预留便宜，贵的是引擎的 AssetSource 重配 | §2.12、M0 的 `ProjectState.root` |
| **D10** | **忽略规则一开始就用 `.kairosignore`** | 项目级规则该跟项目走、能进 git；编辑器级配置会分叉 | M1 读 + seed；`app::project::ignore` |
| **D11** | **资产身份与引用维护并入（M7）** | 用户明确要求；而且 M5 本来就要读 `kairos_asset` | M7，5–8 人日，跨 `kairos_asset`/`kairos_graphics` |
| **D12** | **成品根搬进 `Library/`（M0.5）** | 现在搬一次，之后所有代码都少一条「跳过成品根」的特例 | M0.5，0.5–1 人日，跨 `kairos_asset`/`kairos_engine`/`kairos_graphics` |
| **D13** | **持久化范围 = 编辑器布局**（dock 布局 + tab 顺序/焦点 + 浮动窗口 + Project 分栏），面板会话态不落盘 | 用户拍板（2026-11-25）：会话态细节低收益、按架构归属属 `ViewStates`（按需）；原 M4 方案删除 | M4；`ViewRestored` / `PersistView` / `Library/project_view.toml` 不再做 |

### 7.1 早期三个待定项的落点

早稿里的 D5a（文件夹是否确认）、D10（忽略规则放哪）、D11（引用维护是否并入）已全部落定，分别落在 **M3**（默认确认）、**M1**（`.kairosignore`）、**M7**（身份与引用）。这里不再重复，避免与本表分叉。

### 7.2 2026-11-25 新需求（用户已拍板）

| # | 决定 | 一句话理由 | 落在哪 |
|---|---|---|---|
| **D14** | **External Tools 面板**：`Edit ▸ External Tools`；按文件族记录/设置打开应用；嗅探结果是默认、用户设置优先、未知类型默认系统「打开方式」；设置存 `Library/external_tools.toml`、Browse 用 `native-dialog`、系统选择器先做 Windows | 当前打开方式由 OS 关联 + 硬编码嗅探决定，不可见不可改；用户明确要求一个 Panel | §2.14、M5a/M5b；**P1–P3 已拍板（§2.14.9）** |
| **D15** | **Rust 脚本图标**：`.rs` 的类型图标从 Unity C# 页（`#`）改为 Rust 页（8.3° 斜体 `R` + Unity 绿，dark `#276A2A` / light `#2E7D32`），保留页面外形 | 引擎主语言是 Rust，C# 图标语义错误；用户明确要求换 R，并两次反馈：红色不协调 → 改 Unity 绿，R 过大 → 缩小并斜体化 | §2.15；**已交付** |

> D14 的细节（设置位置、Browse 方式、系统选择器平台范围）已拍板：`Library/external_tools.toml`、`native-dialog`、先做 Windows（§2.14.9）。D15 已交付，并按 2026-11-25 反馈把橙色改为 Unity 绿（§2.15）。

---

## 8. 与原 5 步的映射

| 你的步骤 | 本计划 | 变化 |
|---|---|---|
| 1 重构 ProjectTree | **M0**（接缝）+ **M1**（树 v2） | 拆成「骨架」与「树」两步；树改为 core 纯模型 + app 扫描；顺带删 GUID |
| （新增） | **M0.5** | 成品根搬进 `Library/`，换来 `ScanPolicy` 少一个特例（D12） |
| 2 面板 hierarchy + 导航 | **M2** | 增加 TreeView widget 这个前置；按 D1 一次做成左右两栏（右栏先占位） |
| 3 持久化 | **M4** | 存编辑器布局（dock 树/焦点/浮动窗口/Project 分栏）；面板会话态不落盘（D13）；皮肤已有；GUID 注册表删除 |
| 4 引入类型资源 | **M5**（seam + 每类一个任务） | 增加 `FileTypeRegistry` 这个前置 |
| （新增 2026-11-25）External Tools 面板 | **M5b** | 打开方式从隐式变可配置：`Edit ▸ External Tools`；嗅探默认 + 用户覆盖 + 系统选择器兜底；设置存 `Library/`（P1 已拍板） |
| （新增 2026-11-25）Rust 脚本图标 | **M3 后续修正 / §2.15** | `script.png` 从 C# `#` 换成 Rust `R`；不改代码、不新增类型 |
| 5 搜索 | **M6** | 名字/路径 + `t:` 类型过滤语法；内容搜索单独立项 |
| （你的后端清单里的 CRUD） | **M3** | 你列了它但 5 步没排；它必须早于类型 |
| （你强调的引用维护） | **M7** | 资产身份进 `.meta` + 材质引用 `AssetRef` + 派生索引（D11 并入） |
| — | **M8** | 冻结旧实现（D8：不删）+ 三份 ADR + 文档 |

---

## 9. 不做（out of scope）

- **内容/全文搜索**（需要索引与读文件，独立 ADR）；
- **面板会话态持久化**（展开集合、当前目录、选中、滚动位置、搜索词）：M4 只存布局，这些不落盘（D13；方向属 `ViewStates`，见 panel-viewport-design §2.6）；
- **打开的 World/Scene 文档恢复**：等文档系统落地后作为独立「会话」需求（布局文件已带 `version`，届时新增 section 或新文件）；
- **右栏的网格 / 缩略图本体**（D1：两栏骨架先做好，右栏只放面包屑 + 占位；网格依赖类型任务与预览管线，作为 M7 之后的可选 M9）；
- **从树拖到视口 / 拖出到系统**（需要资产句柄与场景实体，属 Inspector/视口迁移）；
- **资产导入流水线本身**（processor、`.meta` 生成、成品目录）—— 本计划只读取和显示它们的状态；
- **成品根的其它布局变更**（改名、按类型分目录、多成品根）：M0.5 只把它搬进 `Library/` 并保持 `imported_assets/Default` 结构不变，更细的布局调整不在本计划；
- **引用维护的「重链 / 批量修复」**（把坏引用指回正确资产、跨工程迁移）：M7 只做到「解析 + 标缺失」，重链 UI 不在本计划；
- **工程清单 / 多工程切换 / 打开工程对话框**（D9 选 a）；
- **撤销/重做**：删除走回收站 + 确认覆盖 v1；
- **文件变更的增量 diff**（watcher 一律整树重扫，靠 generation 保证正确）；
- **Unity External Tools 的其它区块**：版本控制 diff/merge 工具、生成工程文件（`.csproj` / IDE 集成）、参数模板/行号跳转；本需求只做「按类型的打开应用」；
- **系统文件关联的读写**：面板不写 Windows 注册表、不改 OS 默认应用；`System default` 只调用 OS 关联，`SystemPicker` 只调用系统「打开方式」；
- **用户级（跨工程）设置目录**：v1 按项目存 `Library/`（P1）；升级为用户级是后续独立需求；
- **按文件内容/语言嗅探打开方式**：v1 只按扩展名家族；多扩展名家族（audio wav/ogg）共享一行；
- **内置编辑器/预览**：`OpenDefault::Internal` 的落地归 M5 的每类任务与 Inspector/视口迁移，不在本需求；
- **旧 `kairos_editor` 其他面板的迁移**。

---

## 10. 开工顺序（M0 的第一刀）

1. **先在 core 立 slice**：`ProjectState { name, root, tree, view, scan, error }` + 空的 `ProjectAction/ProjectEffect` + `reduce/project.rs` 注册进 `reduce.rs`；删 `EditorState::project_name`，修 3 个 core 测试与 inspector demo。
2. **再把回写通道打通**：`EditorApp.inbox` + `begin_frame`（每帧一次、egui pass 之外）+ `EffectExecutor::apply -> Vec<Action>`；写两条测试（保序、下一帧回写）。
3. **最后挂空服务**：`ProjectService` 空壳（`execute` 同步返回空结果的 `Scanned`，`poll` 空），`ProjectState.root` 取 cwd，`name` 取目录名。此时编辑器行为与现在**完全一致**，但下一刀（M1）已经有地方落。

每一步都按 `docs/agents/testing.md` 只测改动的 crate；M0 结束跑一次 `cargo check --workspace --all-targets`。
