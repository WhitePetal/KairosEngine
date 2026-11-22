# ProjectPanel 迁移重构计划

> **状态**：已拍板（D1–D12 全部落定，见 [§7](#7-决策记录已拍板)）。**M0 / M0.5 / M1 已落地**（M1 分 M1a core + M1b app 两刀）。M1 开工前对方案做了一次复核，补上了 5 处会现场发明的设计空白、3 处自相矛盾，结论已并入 §2.2 / §2.3 / §2.5 与 §3 的 M1 一节。每个里程碑的实测与已知行为记在对应小节末尾。下一步是 M2（面板 v1）。
> **范围**：`kairos_editor_core`（项目领域）/ `kairos_editor_app`（扫描、监听、文件操作、持久化）/ `kairos_editor_component`（面板）/ `kairos_editor_widgets`（树控件）；旧 `kairos_editor` 只冻结，最后退役
> **配套**：[ADR 0008](../../adr/0008-editor-ui-architecture.md)、[ADR 0009](../../adr/0009-editor-panel-host-and-viewport.md)、[rewrite-plan](../editor-ui-architecture/rewrite-plan.md)（Phase 0.5 写明「只迁 ProjectPanel 一页」）、[panel-viewport-design](../editor-ui-architecture/panel-viewport-design.md) §6.2 P1、[widget-layer](../editor-ui-architecture/widget-layer.md) §2.2/§5
> **可抄的形状**：`prototypes/editor-ui/crates/editor-components/src/project_panel.rs`（派生函数 + 行渲染 + action 映射）、`prototypes/editor-ui/crates/editor-core/src/reduce/project.rs`（data-in / effects-out）
> **旧实现**：`kairos_editor/src/{project_path_tree.rs, asset_registry.rs, project_watcher.rs, ui/project_window/*}`

---

## 0. 结论

### 0.1 你给的骨架成立

5 个阶段的**依赖顺序是对的**：先树、再面板、再持久化、再类型、最后搜索。我不会推倒重来，而是把它落成 9 个可独立验收的里程碑（M0–M8），每一步都写明**代码落在哪个 crate**、**验收怎么量**。

### 0.2 要改的 5 处

1. **第 1 步不是「重构 ProjectTree」，而是「把树拆成三层」**：core 里的纯模型 + app 里的扫描/监听 + 删除 GUID 身份。只重构数据结构，会把 `std::fs` 调用和原生弹窗一起留在原地（现状：`project_path_tree.rs:395` 建目录时直接弹错误框，`:406-410` 五种类型是 `todo!()`，右键建个纹理就 panic）。
2. **第 2 步之前缺一整层接缝**：新 app 目前**没有**「host 异步结果写回 state」的通道（`app.rs:250` 的 `pump` 只吃 UI 产出的 action），也**没有**树控件。不做 M0 + TreeView，面板会重新长出旧那一套 `Arc<Mutex>` / `Cell`（旧 `ProjectWindowModel` 里有 3 个 `Cell`、2 个 `Arc<Mutex>`）。
3. **第 3 步的「持久化」是三件不同的事**，必须分开：(a) 面板视图状态（展开/选中/当前目录）→ 新增 TOML；(b) 面板皮肤 → 已由 `#[panel(skin = ...)]` 解决；(c) GUID 注册表 → **删的是"编辑器自己发号"的那份**（唯一消费者是 Inspector 显示一行文本：`ui/inspector_window.rs:170`）；**资产身份本身要保留**，只是应该搬进 `.meta`，见 §2.11 与 D11。
4. **你把「文件/文件夹的增删改查」列进了后端组成，但 5 步里没有任何一步在排它**。必须补 M3，而且它要早于类型任务——类型任务会新增创建入口、拖拽移动、删除确认，全靠这套协议。
5. **「引入各种类型的资源」需要一个前置 seam**：`FileTypeRegistry`（扩展名 → 类型、图标、后缀标签、打开方式、是否伴生文件）。没有它，每个类型任务都会去改树和面板，第 4 步就会从 5 个任务变成 5 次重构。

### 0.3 要补的 4 块

- **文件操作**（建/改名/移动/删除 + 系统回收站 + 失败可见），见 M3；
- **外部变更**（watcher，已有实现要搬家，不是新写），见 M1；
- **多选与拖拽**（右键菜单之外的真实使用路径），见 M3；
- **性能上限与截断**（后台扫描、节点上限、深度上限、状态栏告警），见 M1/§5。

### 0.4 工期

| 阶段 | 估算（单人） |
|---|---|
| M0.5 资产布局迁移（D12） | **0.5–1 人日**（跨 `kairos_asset`/`kairos_engine`/`kairos_graphics`） |
| M0 + M1–M6（面板本体，不含类型任务） | **13.5–21 人日 ≈ 3–4.5 周** |
| M7 资产身份与引用维护 | **5–8 人日**（跨 `kairos_asset`/`kairos_graphics`） |
| M8 收尾（冻结 + 文档） | **0.5–1 人日** |
| **合计（不含类型任务）** | **19.5–31 人日 ≈ 4–6 周** |
| M5 的 seam（已含在 M0–M6 里） | 2–3 人日 |
| M5 的每个类型任务（另计） | 0.5–2 人日 / 类型 |

前提：保留现有 dock、字体/样式资源与资产加载，不重写引擎侧。

### 0.5 已拍板的决定（摘要，细节与理由见 §7）

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
| 选中恢复 | 重扫后按 GUID 找回 | 按路径找回，失效则回退最近祖先（M1/M4） |
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

---

## 2. 目标设计

### 2.1 归属表

| 关注点 | 归属 | 理由 |
|---|---|---|
| 树的形状、排序、可见行、选择/展开/过滤规则 | `core::project` | 纯函数，可单测，可回放 |
| 扫描策略（跳过谁、上限、深度） | `core::project::ScanPolicy`（数据） | 策略是数据，不是 `if`；测试可注入 |
| 遍历文件系统、监听、读/写文件、开进程、读编辑器设置 | `app::project` | core 不许 IO |
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
// core/src/project/view.rs —— 「用户意图」，可持久化；本帧才有效的放面板里
pub struct ProjectView {
    pub expanded: BTreeSet<RelPath>,
    pub active: Option<RelPath>,        // 面包屑 / 内容区当前目录
    pub selection: BTreeSet<RelPath>,   // M2 只放 0/1 个；M3 多选
    pub sort: SortKey,                  // v1 只有 Name
    pub show_hidden: bool,              // 点文件与 .meta 伴生
    /// 左右两栏的分割比例（D1）：右栏从第一天就存在，只是先空着。
    pub tree_split: f32,                // 0.15..=0.6，默认 0.28
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
    ViewRestored { view: ProjectView },    // M4

    // 变更（请求 → effect → 完成）
    CreateRequested { parent: RelPath, name: String, kind: CreateKind },
    RenameRequested { path: RelPath, name: String },
    MoveRequested { paths: Vec<RelPath>, target: RelPath },
    DeleteRequested { paths: Vec<RelPath> },
    FsCompleted { op: FsOpId, result: Result<FsOutcome, String> },

    // 打开
    OpenRequested { path: RelPath },
    RevealRequested { path: RelPath },
}

pub enum ProjectEffect {
    Scan { root: PathBuf, policy: ScanPolicy, generation: u64 },
    PersistView { view: ProjectView, root: PathBuf },   // app 侧去抖 + 原子写
    Fs(FsOp),                                            // create/rename/move/trash
    Open { root: PathBuf, path: RelPath, with: OpenWith },
    Reveal { root: PathBuf, path: RelPath },
}

pub struct FsOp { pub id: FsOpId, pub root: PathBuf, pub kind: FsOpKind }
pub enum FsOpKind {
    CreateFolder { parent: RelPath, name: String },
    CreateFile { parent: RelPath, name: String, template: FileTemplate },
    Rename { from: RelPath, to: RelPath },
    Move { from: Vec<RelPath>, to: RelPath },
    Trash { paths: Vec<RelPath> },
}
```

**reducer 里的关键规则**（都要有测试）：

- `Scanned` 的 generation 不匹配 → 整条丢弃（旧结果绝不覆盖新树）；
- 重扫后 `expanded`/`selection` 里不存在的路径被剪掉；`active` 失效则回退到最近的仍存在的祖先；
- `RenameRequested` 校验通过后**不改树**，只发 effect；完成 → `ScanRequested { focus: Some(新路径) }`。乐观改树会与 watcher 打架，而扫描很便宜；
- `Select { mode }` 三种：`Replace` / `Toggle`（Ctrl）/ `Range`（Shift，用可见行序算区间）；
- 所有 `*Requested` 在入口做**同一套名字校验**（`project/name.rs`），失败返回 `error`，不发 effect。

**名字校验**（纯函数，一张表）：空名、`<>:"/\|?*`、控制字符、Windows 保留名（`CON`/`PRN`/`AUX`/`NUL`/`COM1..9`/`LPT1..9`）、结尾点或空格、超长（组件 255 字节）、同级大小写不敏感重名、改名为自身、根节点不可改不可删。每条一个测试。

### 2.6 面板 UI（M2）

D1 定稿：**左右两栏从第一天就存在**，右栏先只有面包屑 + 占位，网格/缩略图在 M9 补内容，布局本身不返工。

```
┌ Project ────────────────────────────────────────────────────────────┐
│ [新建文件夹] [新建文件▾] [删除] [折叠全部] [刷新]      [隐藏文件] │ 工具栏 20px
├──────────────────────────────┬──────────────────────────────────────┤
│ ▾ res                        │ res / models /                       │ 面包屑 20px
│   ▾ models                   │                                      │
│     ▣ Suzanne.png      png   │          （内容区占位）               │ 分割条可拖
│     ▣ readme.md        md    │      网格视图在 M9 填充              │
│   ▸ shaders                  │                                      │
│ ▸ scripts                    │                                      │
│ ──────────────────────────── │                                      │ ← 分隔线
│ ▸ Library                    │                                      │ 固定最下、默认折叠
├──────────────────────────────┴──────────────────────────────────────┤
│ 12 items · res/models/Suzanne.png · (truncated)                     │ 状态栏 18px
└─────────────────────────────────────────────────────────────────────┘
```

- 分割条用现成的 `widgets::splitter`（两边宽度进 `ProjectView::tree_split`，M4 持久化）；
- **`Library` 固定排最后**（D2）：模型里它就是顶层的一个普通目录，只是 `ordered_children` 把它排到最后、面板在它前面画一条 1px 分隔线；默认折叠，展开状态同样进 `expanded`；不受忽略规则影响，但有独立节点预算；
- 搜索**不进入 `Library`**：里面是引擎产物，搜出来只会是噪音（要查导入产物请直接展开它）；
- 右栏 v1 是「面包屑 + 空状态」：面包屑可点任意一级跳转，空状态写明下一阶段做网格 —— 它已经能验证 `active` 这套导航状态是对的；
- 状态栏跨两栏通栏：左半边计数、右半边选中路径、截断告警优先（宁可挤掉路径）。

**widget 契约**（`widgets/src/tree.rs`）：

```rust
pub struct TreeRow<'a> {
    pub depth: u16,
    pub label: &'a str,
    pub suffix: Option<&'a str>,
    pub icon: Option<&'a str>,      // Preferences/Textures 下的相对路径
    pub expander: Option<Expander>, // None = 文件
    pub selected: bool,
    pub drop_target: bool,
}

pub struct TreeOutput<Id> {
    pub clicked: Option<Id>,
    pub double_clicked: Option<Id>,
    pub expander_toggled: Option<Id>,
    pub context_menu: Option<Id>,
    pub rename_committed: Option<(Id, String)>,
    pub rename_cancelled: Option<Id>,
    pub drag_started: Option<Id>,
    pub dropped_on: Option<(Id, Id)>,
    pub hovered: Option<Id>,
}

impl<'a, Id: Copy + Eq + Hash> TreeView<'a, Id> {
    /// rows 只给 id（前序）；行内容按需回调，只对可见行调用。
    pub fn new(rows: &'a [Id], row: impl Fn(Id) -> TreeRow<'a> + 'a) -> Self;
    pub fn renaming(self, id: Id, buffer: &'a mut String) -> Self; // 行内编辑器
    pub fn show(self, ui: &mut egui::Ui) -> TreeOutput<Id>;
}
```

三处必须和原型不同（否则到了 2 万行会痛）：

1. **只对可见行回调**：`ScrollArea::show_rows` 给可见区间，widget 只对区间内行调用 `row(id)`。原型把所有行物化成 `Vec<TreeRow>`，全展开时每帧 MB 级分配。
2. **行 id 来自 `Node.row_id`**（建树时算一次），不在绘制期做哈希。
3. **不做 `PanelChrome`**：标题栏属于 dock（`Panel` trait 只管 body，见 `panel.rs:77`），面板 body 从工具栏开始 —— 和 Console 一致。

**面板自己持有的视图状态**（关掉标签页就丢，这是对的）：搜索框文本、重命名 buffer、删除确认、拖拽中的路径、滚动定位。**core 持有**：`expanded`/`active`/`selection`/`sort`/`show_hidden`。判据：**关掉面板再打开还该在的 → core；只影响这一帧的 → 面板**。这条规则有硬理由：`PanelHost::sync` 的 `retain` 会直接 drop 关闭的面板实例（`panel/host.rs:52`），面板里存不住任何东西。

**绘制**：`TreeView` 只画行；重命名编辑器、右键菜单（`menu::submenu` + `menu_item_enabled`）、删除确认（`Popup`）都由面板画 —— 面板有 `&mut Ui` 和 `&mut UiOut`，widget 没有 `Action` 的概念。

### 2.7 持久化（M4）

持久化分两类，各有各的所有者：**视图缓存**（本文档新增，一个 TOML）与**面板皮肤**（已有机制，`Preferences/Styles/ProjectStyle.ron`）。前者可删可重建：

```toml
# Library/project_view.toml —— 可删、可重建的缓存
version = 1
root = "D:/Projects/MyGame"      # 不匹配就整份忽略
sort = "name"
show_hidden = false
tree_split = 0.28
expanded = ["res", "res/models"]
active = "res/models"
selection = ["res/models/Suzanne.png"]
```

- **写**：`Effect::Project(PersistView)` → app 侧去抖（最后一次变更后 500ms）→ 临时文件 + `rename` 原子替换；
- **读**：boot 时读一次，入 `Action::ViewRestored`；**必须等第一次 `Scanned` 之后再合并**（否则对着空树剪枝会全丢）—— 这是最容易写错的一处，写两条测试；
- **坏文件**：解析失败 / 版本不认识 / root 不匹配 → 记一条 warn，按默认视图启动，绝不阻塞编辑器；
- **不存**：树本身（重扫即可）、滚动位置、搜索文本、GUID、忽略规则（那是编辑器设置，见 §2.2）。

#### 2.7.1 对齐：`asset_registry.toml` 是什么，不是什么

这是 D3 里最需要说清的一点。旧注册表（`GUID ↔ 路径`）**不是用来加速重建树的**，它一条 `read_dir` 也省不掉：

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
// core/src/project/file_type.rs —— 纯数据，注册在启动时传入
pub struct FileType {
    pub id: &'static str,             // "texture" | "mesh" | ...
    pub label: &'static str,          // 行尾后缀，如 "png" / "mat"
    pub extensions: &'static [&'static str],
    pub icon: &'static str,           // Preferences/Textures/model_icon.png
    pub companion: bool,              // .meta/.uid 之类：默认隐藏
    pub referenceable: bool,          // 可被别的资产引用 → M7 会给它发 guid
    pub delete_confirm: bool,         // D5：默认 true（Unity 行为），按类型可关
    pub open: OpenWith,               // ExternalEditor | SystemDefault | Inspector(..) | Preview(..)
}

pub struct FileTypeRegistry { types: Vec<FileType>, by_ext: HashMap<String, usize> }
impl FileTypeRegistry {
    pub fn lookup(&self, name: &str) -> Option<&FileType>; // 大小写不敏感，最长扩展名优先
    pub fn is_companion(&self, name: &str) -> bool;
}
```

- 默认表 = 现存的 8 种（png/glb/mat/audio/wgsl/rs/md/toml/ttf）+ `meta` 伴生 + fallback；
- 图标复用 `Preferences/Textures/` 已有文件（`folder_icon.png` / `folder_fill_icon.png` / `model_icon.png` ...），**不需要新美术**；
- 加一个类型的代价 = 表里一行 + 一个打开处理器 + 测试。**验收方式**：用测试替身注册一个假类型，断言树、行、打开分发都不需要改 —— 如果改了三处，说明 seam 没做对；
- 引擎侧的 loader 注册表（`AssetLoader::extensions`）是未来的第二真相源，v1 不接（core 不能依赖 `kairos_asset`，要接就经 app 注入）。

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
- 面板缓存 `Vec<NodeId>`，失效条件 = `(tree.generation, expanded, query, sort, show_hidden)`；
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
  project/file_type.rs     FileTypeRegistry（M5）
  project/name.rs          名字校验（M3）
  reduce/project.rs        reducer（新增 slice，注册到 reduce.rs）
  tests/project_*.rs

kairos_editor_app/src/
  project/mod.rs           ProjectService（poll / execute）
  project/scanner.rs       后台扫描线程 + generation
  project/watcher.rs       从 kairos_editor/src/project_watcher.rs 搬来（notify-debouncer-full）
  project/fs.rs            建/改名/移动/回收站，错误转字符串
  project/persist.rs       project_view.toml 读写 + 去抖 + 原子写
  project/ignore.rs        .kairosignore 解析（缺文件 seed 默认） + 规则匹配入口
  project/identity.rs      guid ↔ path 索引（读侧车、可重建缓存，M7）
  project/open.rs          外部编辑器 / 系统默认（从 ui/ide_detection.rs 搬来）

kairos_editor_component/src/
  panels/project.rs        装配：工具栏 + [树 | 面包屑+占位] + 状态栏 + 弹层
  panels/project/style.rs  ProjectStyle（RON，dark/light，含 tree_split 默认值）
  panels/project/rows.rs   core 状态 → TreeRow 的纯映射 + 行缓存
  panels/project/content.rs 右栏占位（M9 换成网格，接口先定）

kairos_editor_widgets/src/tree.rs   TreeView / TreeRow / TreeOutput
```

新依赖（只进 app）：`notify-debouncer-full`、`toml`、`serde`、`trash`、`open` —— 版本与 `kairos_editor/Cargo.toml:39-49` 现有条目一致。M7 不引入新依赖：`kairos_asset` 已经依赖 `uuid`（`kairos_asset/Cargo.toml`）。

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
2. **配置按根键化**：`Library/project_view.toml` 里已经带 `root` 守卫；未来的「最近打开」列表也按根键化。
3. **切换工程 = 一条 action**：`ProjectOpened { root }` → 清空 `ProjectState`、重置 generation、`ScanRequested`、重读视图缓存。面板只认 `frame.state.project.root`，一行都不用改。

**真正贵的地方不在面板，在引擎**：`kairos_asset` 的 source root 是进程级的（ADR 0004：cwd + `KAIROS_ASSET_ROOT` 覆盖），换工程要重配 asset server / 处理器 / watcher。那是引擎侧的一整套改动，所以「打开工程」不进本计划 —— 但上面三条保证它将来只需要动 app + 引擎，不碰面板。

### 2.13 词表（写进 `kairos_editor/CONTEXT.md`）

**Project**：编辑器的资产工作区，一个根目录 + 它的文件树 + 用户意图（展开、选中、当前目录）。**System dir**：正常扫描但有三条特殊待遇的目录（当前只有 `Library`）——根级排最后、带分隔线、独立预算、不受 ignore 影响。**ProjectTree**：某一时刻文件系统结构的快照，身份是相对路径。**ScanPolicy**：扫描规则（忽略 + 系统目录预算 + 节点/深度上限）。**ProjectView**：树之上可持久化的用户意图。**FileType**：一个扩展名族的显示与打开方式，不含任何文件操作。**Cluster**：一个资产的主文件 + 它的伴生侧车（`.meta` 等），移动/删除以簇为单位。**AssetId / Guid**：资产的持久身份（写在 `.meta` 里，见 M7），与树的路径身份正交。

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
> - **顺带发现（不是 M0.5 引入的）**：`bake_assets` 的源根是进程 cwd（ADR 0004），所以它会把 `docs/` 的研究截图、`Preferences/` 的图标、甚至 `target/doc` 的 rustdoc 图一并加工——本次产出 **230 个、约 346 MB** 未被仓库提交的成品；仓库历史上只提交了 `res/` 那 10 个。要修的话是给 bake 一个排除清单（`target`/`docs`/`Preferences`/`prototypes`）或把源根收到 `res/`，属独立小任务，不在 M1 范围。

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

### M2 — 面板 v1：左右两栏 + 树 + 导航（4–6 天）

**目标**：Project 面板变成**左右两栏**的真实面板（D1）：左树能展开/导航/选中/打开，右栏是面包屑 + 空白占位；树底部固定排最后的 `Library` 与分隔线（D2）；**只读**。

**交付**
- widgets：`tree.rs`（§2.6 契约）+ headless 测试；
- component：工具栏（通栏）/ `[树 | 面包屑 + 占位]`（`widgets::splitter`）/ 状态栏（通栏）；`ProjectStyle`（`Preferences/Styles/ProjectStyle.ron`，从 `ProjectWindowStyle.toml` 1:1 映射：行高 20、图标 18、选中 `#3c4b64`、后缀 `#787878`，另加 `tree_split` 默认值）；`#[panel(PROJECT = "project", skin = ProjectStyle)]`；
- core：`Expand/Collapse/Navigate/Select/SetSort/SetShowHidden/SetTreeSplit` 的 reducer；`visible_rows` 缓存；系统目录 `Library` 默认折叠；
- app：`Open` effect 的两个实现（外部编辑器 / 系统默认），`ide_detection` 从旧 crate 搬来；
- 键盘：↑↓ 移动、←→ 折叠/展开、Home/End、Enter 打开、F2 改名（M3 接上）。

**验收（手动 smoke）**：展开 `res/models` → 看到 `Suzanne.png`（后缀灰色）→ 单击选中、状态栏显示路径 → 双击目录后右栏面包屑前进且左树同步展开到该目录 → 拖动分割条后左右比例正确、右栏不溢出 → 树底部有分隔线 + `Library`，展开后看到 `Logs/` 等内容 → 过滤时 `Library` 不参与 → 双击 `.md` 用编辑器打开 → 点 Window ▸ Project 关闭再打开，展开状态还在 → 20k 节点全展开时拖动滚动条不卡（用 Profiler 面板看 UI 线程每帧 < 8ms）。

**风险**：`TreeView` 的 API 被 M3 的改名/拖拽撑破。缓解：M2 就把 `renaming`/`drag_started`/`dropped_on` 三个通道留出来（可以先只返回 `None`），不要让面板绕过 widget 自己画行。

### M3 — 变更：建 / 改名 / 移动 / 删除（3–5 天）

**目标**：面板可以完整编辑文件系统，每个失败都看得见。

**交付**
- core：`name.rs` 校验表（§2.5）、`Create/Rename/Move/Delete` 的 reducer、多选（Ctrl/Shift）、`FsOpId` 与完成分支；
- app：`fs.rs` 执行 create/write/rename/move，删除走 `trash`（失败则报错，绝不静默永久删除）；每个操作完成 → `FsCompleted` → `ScanRequested { focus }`；
- 删除确认（D5）：**文件和文件夹默认都弹确认**（Unity 行为），`FileType::delete_confirm` 只用于个别类型关掉；确认弹层是普通 UI 状态，不是原生对话框；
- component：右键菜单（新建文件夹、新建文件▾、重命名、复制路径、在资源管理器中显示、删除）、行内重命名（Enter 提交 / Esc 取消 / 失焦提交）、拖拽移动（`ui.dnd_drag_source` + `dnd_release_payload`，egui 0.35 已有）、删除确认弹层、失败提示条；
- 目标目录高亮、`Shift+点击` 区间选择；
- **成簇规则（§2.11 的 G2/G3，D4 的地基）**：改名/移动/删除一个文件时，它的伴生侧车（`foo.png.meta`、将来的 `foo.png.uid`）跟着一起走；判据来自 `FileType::companion`/后缀规则，不写死 `meta`。目录操作天然整簇（内部文件一起搬/一起进回收站）。

**验收**：建文件夹 → 自动进入重命名 → 输入含 `/` 被拒并提示 → 改名目录后子孙仍可选中 → 改名 `Suzanne.png` 后 `Suzanne.png.meta` 同步改名 → Ctrl+点击多选 → 拖到另一个目录 → 删除进回收站 → 回收站里能找回来（主文件与侧车都在）→ 只读目录上删除失败有提示而不是静默。

**测试**：校验表逐条；fs 操作在 tempdir（含目录带子孙、仅大小写改名、目标已存在、只读父目录）；`trash` 在 CI 上不可用时 `cfg` 跳过并注释原因。

**风险**：Windows 语义（仅大小写改名要两段式、保留名、尾点空格、路径占用）。全部收敛到 `name.rs` + `fs.rs` 两处，别散落在面板里。

### M4 — 持久化（1.5–2.5 天）

**目标**：关掉编辑器再打开，展开/当前目录/选中都还在；删掉缓存文件也只是回到默认。

**交付**
- app：`persist.rs`（读、去抖写、原子替换、版本号、root 守卫；字段含 `tree_split`）；
- core：`ViewRestored` + 「首次 Scanned 后再合并」的规则；
- 面板皮肤已由 RON 持久化（`#[panel(skin)]`），本次不动。

**验收**：展开三层（含 `Library` 根）+ 拖动分割条 → 重启 → 展开与比例都一样；把 `Library/project_view.toml` 改成乱码 → 正常启动 + 一条 warn；把 root 改成别的项目 → 忽略；改名过的路径在文件里 → 加载时被剪掉。

**测试**：round-trip；坏文件；root 不匹配；悬空路径剪枝 + active 回退最近祖先；原子写不产生半文件。

### M5 — 类型 seam + 每种类型（2–3 天 + 每类 0.5–2 天）

**目标**：加一个类型不再触碰树和面板。

**交付**
- core：`FileTypeRegistry` + 默认表 + 伴生规则；`OpenWith` 分发；
- component：行图标/后缀来自注册表；未注册类型显示通用图标（不隐藏）；「只看资产」开关（用注册表过滤，D2）；
- app：打开分发（外部编辑器 / 系统默认），未知类型回退系统默认；
- **之后每类一个任务**：纹理（缩略图 + `.meta` 是否有 / 是否已导入）、网格（预览）、材质（Inspector）、音频（试听）、着色器/代码（IDE）、文档（Markdown 只读）。凡依赖「文档/预览」的能力，都依赖 Inspector 与视口迁移，不在本计划排期。

**验收**：测试替身注册一个 `foo` 类型 → 树里出现带图标的行 → `Enter` 走它自己的处理器 → 删掉这个类型后行为回到通用文件。整个过程不改 `tree.rs` / `panels/project.rs`。

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
- 新 ADR：`0015-project-tree-and-panel.md`（路径身份、扫描协议、忽略规则、持久化格式、类型注册表、删除语义）；
- 新 ADR：`0016-asset-guid-and-references.md`（M7 的身份与引用；与 ADR 0002 的 meta 格式如何兼容）；
- 交叉链接 ADR 0017（M0.5 的成品根布局）到 ADR 0004/0005 与 `rewrite-plan.md`；
- `rewrite-plan.md` 的模块映射表加一行：旧 project 子树已由 `core::project` + `app::project` + `panels::project` 取代，随 `kairos_editor` 退役删除；
- `docs/plan/editor-ui-architecture/README.md` 的面板清单把 Project 标为已迁移；`kairos_editor/CONTEXT.md` 加 §2.13 词表；
- `docs/research` 里引用旧项目窗口的地方标注「已被 docs/plan/project-panel 取代」。

**验收**：两份 ADR 合入；仓库里只剩文档层面的「旧实现在哪、为什么不删」说明，没有新的旧代码改动；`cargo check --workspace --all-targets` 通过（本阶段不改代码，跑它是为了确认前几个里程碑没留编译债）。

---

## 4. 测试策略

沿用仓库现状：没有 `egui_kittest`，组件测试用裸 `egui::Context` + 合成指针输入（参照 `kairos_editor_component/tests/panel_host.rs`）。

| 层 | 测什么 | 怎么测 | 成本 |
|---|---|---|---|
| core | 树组装、可见行、reducer 规则、名字校验、忽略规则匹配、查询解析、view 序列化 | 纯函数 + fixture `Vec<ScanEntry>` | 毫秒级 |
| app | 扫描器（含忽略与系统目录预算）、watcher 去抖、fs 操作、持久化、`.kairosignore` 读取与 seed | `tempfile` + 真实文件系统 | 十毫秒级 |
| asset/graphics | guid 侧车往返、改名后引用仍解析、缺失回退（M7）；成品根新路径下 processor/loader 仍工作（M0.5） | tempdir + 真 `.mat`/`.meta` | 十毫秒级 |
| widgets | 行绘制、展开三角命中、选中、键盘、改名提交/取消 | `Context::run` + 合成事件 | 毫秒级 |
| component | 面板装配：点击 → action、双击 → 导航、右键 → 菜单项 | `Context::run` + 断言 `UiOut` | 毫秒级 |

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
| 内存（10 万节点） | < 20MB | `Node` + 路径字符串；相对路径不重复存绝对前缀 |

**怎么量**：5 万 / 20 万节点的耗时用 `#[ignore]` 的一次性脚本（或 `examples/`）在 tempdir 里建好 fixture 后只计扫描时间——建 fixture 比扫它慢一个数量级，**不要放进日常 `cargo test-crate`**。M1 收尾量一次，把实测数字补到本表下面。

**M1 实测**（dev profile，`opt-level = 1`，Windows NTFS，tempdir fixture）：**50,251 节点 42 ms**。对应命令：
`cargo test -p kairos_editor_app --bins -- --ignored fifty_thousand --nocapture`（建 fixture 本身约 18 s，所以不进日常测试）。

**留的扩展点**：`ScanPolicy` 已有 `max_depth`/`follow_dir_symlinks`；若实测网络盘不可接受，把 `Scan` 换成「首屏只扫一层 + 展开时补扫」，接口不变（`ScanRequested { reason: Lazy(dir) }` 已预留 `reason`）。

---

## 6. 风险与缓解

| 风险 | 触发迹象 | 缓解 |
|---|---|---|
| TreeView 抽象撑不住改名/拖拽/多选 | 面板里开始「绕开 widget 自己画行」 | M2 就留 `renaming`/`drag`/`dropped_on` 通道；撑不住就改 widget API，不在面板里绕过（`rewrite-plan` 同款风险） |
| 扫描拖慢启动 | 网络盘/巨量目录下首帧 > 1s | 后台扫 + 截断 + `elapsed_ms` 日志；M1 就用 5 万节点 fixture 验 |
| Windows 文件语义踩坑 | 仅大小写改名失败、保留名被系统拒 | 全部收敛到 `name.rs`/`fs.rs`；每条规则一条测试 |
| watcher 自激（自己写自己） | 每次刷新都触发下一次刷新 | watcher 按路径前缀**丢掉 `Library/` 下的事件**（缓存、日志、视图文件都在那里）+ generation + 300ms 去抖 |
| 持久化吃掉用户状态 | 重启后展开全没了 | 「首次 Scanned 后合并」+ 坏文件当默认 + 删文件无害 |
| core 长回 IO/锁 | review 时看到 `std::fs`/`Mutex`/`Cell`/`Receiver` | 写进 review 检查单：出现即打回 |
| 两套实现长期分叉 | 旧 crate 里还在修 project 的 bug | 旧实现冻结；M7 删除。期间**不在 `kairos_editor/src/project*` 改任何东西** |
| 非 UTF-8 文件名 | 显示成 `?`，操作报错 | 明确限制：显示用 lossy，操作前校验「可表示」，不可表示则给明确错误（不静默） |
| 范围蔓延到资产导入 | 讨论「自动生成 .meta」「缩略图缓存」 | 属 M5 类型任务且依赖 processor；本计划只做「显示与打开」 |

---

## 7. 决策记录（已拍板）

| # | 决定 | 一句话理由 | 落在哪 |
|---|---|---|---|
| **D1** | **左右两栏，右栏先空** | 布局/分割条/导航状态一次到位，网格只填内容不返工 | `ProjectView::tree_split`、M2 面板骨架 |
| **D2** | **单根；`Library` 是系统目录；忽略走 `.kairosignore`；搜索分 `In Assets`/`In Library`** | 系统目录、忽略、隐藏是三件事，混在一起会既慢又说不清；搜索范围照 Unity 分档最直观 | `ScanPolicy::{system_dirs,ignore}` + `ProjectTree::ordered_children` + `.kairosignore` + `SearchScope`（M6） |
| **D3** | **只持久化视图意图，不持久化树** | `asset_registry.toml` 是身份表不是缓存，一条 `read_dir` 也省不掉（§2.7.1） | `Library/project_view.toml`（M4）；索引缓存按需（§2.7.2） |
| **D4** | **删编辑器注册表；资产身份搬进 `.meta`** | 引用是路径、注册表只服务 UI；改名安全需要资产侧 ID（§2.11） | M1 起不再产出 `Library/asset_registry.toml`；M3 埋成簇地基；身份本体 = M7 |
| **D5** | **回收站 + 默认确认（Unity 行为）** | 误删不可逆；确认弹层是 UI 状态不是宿主对话框 | `FileType::delete_confirm`（默认 true）、M3 |
| **D6** | **v1 单选，M3 多选 + 树内移动** | 跨面板拖要资产句柄，属 M5 之后 | M3 |
| **D7** | **搜索含 `t:` 等过滤语法** | 名字子串不够用；类型过滤是资产工作流的日常动作 | §2.9 语法、`core::project::search`、M6 |
| **D8** | **旧代码等到整体退役再删** | 冻结 + 文档足够防止分叉，删半个 crate 反而有编译风险 | M8 只改文档 |
| **D9** | **保持 cwd，但守三条纪律** | 面板侧预留便宜，贵的是引擎的 AssetSource 重配 | §2.12、M0 的 `ProjectState.root` |
| **D10** | **忽略规则一开始就用 `.kairosignore`** | 项目级规则该跟项目走、能进 git；编辑器级配置会分叉 | M1 读 + seed；`app::project::ignore` |
| **D11** | **资产身份与引用维护并入（M7）** | 用户明确要求；而且 M5 本来就要读 `kairos_asset` | M7，5–8 人日，跨 `kairos_asset`/`kairos_graphics` |
| **D12** | **成品根搬进 `Library/`（M0.5）** | 现在搬一次，之后所有代码都少一条「跳过成品根」的特例 | M0.5，0.5–1 人日，跨 `kairos_asset`/`kairos_engine`/`kairos_graphics` |

### 7.1 早期三个待定项的落点

早稿里的 D5a（文件夹是否确认）、D10（忽略规则放哪）、D11（引用维护是否并入）已全部落定，分别落在 **M3**（默认确认）、**M1**（`.kairosignore`）、**M7**（身份与引用）。这里不再重复，避免与本表分叉。

---

## 8. 与原 5 步的映射

| 你的步骤 | 本计划 | 变化 |
|---|---|---|
| 1 重构 ProjectTree | **M0**（接缝）+ **M1**（树 v2） | 拆成「骨架」与「树」两步；树改为 core 纯模型 + app 扫描；顺带删 GUID |
| （新增） | **M0.5** | 成品根搬进 `Library/`，换来 `ScanPolicy` 少一个特例（D12） |
| 2 面板 hierarchy + 导航 | **M2** | 增加 TreeView widget 这个前置；按 D1 一次做成左右两栏（右栏先占位） |
| 3 持久化 | **M4** | 只存视图状态；皮肤已有；GUID 注册表删除 |
| 4 引入类型资源 | **M5**（seam + 每类一个任务） | 增加 `FileTypeRegistry` 这个前置 |
| 5 搜索 | **M6** | 名字/路径 + `t:` 类型过滤语法；内容搜索单独立项 |
| （你的后端清单里的 CRUD） | **M3** | 你列了它但 5 步没排；它必须早于类型 |
| （你强调的引用维护） | **M7** | 资产身份进 `.meta` + 材质引用 `AssetRef` + 派生索引（D11 并入） |
| — | **M8** | 冻结旧实现（D8：不删）+ 两份 ADR + 文档 |

---

## 9. 不做（out of scope）

- **内容/全文搜索**（需要索引与读文件，独立 ADR）；
- **右栏的网格 / 缩略图本体**（D1：两栏骨架先做好，右栏只放面包屑 + 占位；网格依赖类型任务与预览管线，作为 M7 之后的可选 M9）；
- **从树拖到视口 / 拖出到系统**（需要资产句柄与场景实体，属 Inspector/视口迁移）；
- **资产导入流水线本身**（processor、`.meta` 生成、成品目录）—— 本计划只读取和显示它们的状态；
- **成品根的其它布局变更**（改名、按类型分目录、多成品根）：M0.5 只把它搬进 `Library/` 并保持 `imported_assets/Default` 结构不变，更细的布局调整不在本计划；
- **引用维护的「重链 / 批量修复」**（把坏引用指回正确资产、跨工程迁移）：M7 只做到「解析 + 标缺失」，重链 UI 不在本计划；
- **工程清单 / 多工程切换 / 打开工程对话框**（D9 选 a）；
- **撤销/重做**：删除走回收站 + 确认覆盖 v1；
- **文件变更的增量 diff**（watcher 一律整树重扫，靠 generation 保证正确）；
- **旧 `kairos_editor` 其他面板的迁移**。

---

## 10. 开工顺序（M0 的第一刀）

1. **先在 core 立 slice**：`ProjectState { name, root, tree, view, scan, error }` + 空的 `ProjectAction/ProjectEffect` + `reduce/project.rs` 注册进 `reduce.rs`；删 `EditorState::project_name`，修 3 个 core 测试与 inspector demo。
2. **再把回写通道打通**：`EditorApp.inbox` + `begin_frame`（每帧一次、egui pass 之外）+ `EffectExecutor::apply -> Vec<Action>`；写两条测试（保序、下一帧回写）。
3. **最后挂空服务**：`ProjectService` 空壳（`execute` 同步返回空结果的 `Scanned`，`poll` 空），`ProjectState.root` 取 cwd，`name` 取目录名。此时编辑器行为与现在**完全一致**，但下一刀（M1）已经有地方落。

每一步都按 `docs/agents/testing.md` 只测改动的 crate；M0 结束跑一次 `cargo check --workspace --all-targets`。
