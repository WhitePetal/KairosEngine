---
Status: accepted
---

# 资产管线的排除规则 = 项目浏览器的忽略规则（`.kairosignore` 唯一真相）

> 本 ADR 让「项目拥有哪些目录」只有一个答案：面板看不见的目录，bake 也不进。

## 背景与决策

ADR 0004 把默认资产源根定在进程 cwd，bake 因此会把 `res/` 之外的一切也当成源：
只要某个文件有 loader 或 processor 认领，它就产出成品。实测（M0.5 验收时）一次
`cargo run --bin bake_assets` 除了提交过的 10 个成品，还写下了 230 个未提交的产物
（`docs/` 的研究截图、`Preferences/` 的图标、`prototypes/`），并额外镜像了 `target/`
里的 rustdoc 图——它们都不是项目想发布的东西。

更深的问题不是体积，而是**两个「项目内容」的定义**：面板按 `.kairosignore` 决定
哪些目录不进快照，bake 却按「有没有 loader」遍历整棵 cwd 树。两者迟早会分叉
（用户在 `.kairosignore` 里写 `res/generated`，面板隐藏它，bake 照样处理它）。

**决策**：`.kairosignore` 是项目排除规则的**唯一真相**，两个消费者共用同一份实现：

- 新叶子 crate `kairos_ignore`：gitignore 形状的解析与匹配（纯函数、零依赖）；
- 项目浏览器（`kairos_editor_core::project`）用它决定哪些目录不进 `ProjectTree`；
- bake（`kairos_engine::asset_pipeline`）用它决定哪些目录不做为资产源进入 processor。

**为什么是叶子 crate**：引擎在依赖图里位于编辑器之下，不能反向依赖
`kairos_editor_core`；而 `kairos_editor_core` 又不能依赖 `kairos_asset`（ADR 0008：
状态树不许拖进异步资产栈）。两边各写一份解析器正是本 ADR 要消灭的分叉，所以规则
下沉到一个两边都能用的零依赖 crate。

bake 侧的接法：`bake_assets()` 以 cwd 为项目根，读根上的 `.kairosignore`（缺失时用与
面板完全相同的内置兜底 `.*`、`target`），把被忽略的目录（每棵子树取最顶层那个）作为
`AssetSourceBuilder::unprocessed_excludes` 传给默认 source。不支持的规则行照旧记一条
warn 并跳过，不静默改变语义。

## 考虑过的替代

- **在引擎里再写一份解析器**：省一个 crate，但两份实现会分叉，而这正是要修的问题。
- **编辑器把解析结果写成一个生成文件给 bake 读**：多一个产物、多一次同步，bake 在
  文件不存在时又要发明第三种行为。
- **bake 改成接受命令行路径参数**：`cargo run --bin bake_assets` 就不再是完整命令，
  漏传参数会悄悄少烘一批资产。
- **bake 只烘 `res/`**：把「资产根」写死进工具，等于把 ADR 0004 的 cwd 源根再分叉一次；
  排除规则应该来自项目，而不是来自工具里的字面量。

## 影响

- 新 workspace 成员 `kairos_ignore`（见 `kairos_ignore/src/lib.rs` 的模块文档）。
- `kairos_editor_core` 依赖它，`project::IgnoreRules` 变为再导出（面板/扫描器/监听器
  的行为不变，测试随类型搬家到新 crate）。
- `kairos_asset`：`AssetSource` 的 `unprocessed_exclude: Option<PathBuf>` 变成
  `unprocessed_excludes: Vec<PathBuf>`——一个 source 现在可以排除多个子树（成品根
  一个 + 项目忽略的若干个）；`is_excluded_from_unprocessed` 改为任一命中即排除。
- `kairos_engine`：`bake_assets()` 不再用 `AssetSourceBuilders::default()`，而是显式构造
  默认 source 并把忽略目录加进去；日志多一行「N directories ignored」。
- **只作用于 bake**：运行时宿主保持现状。它跑布局①并按路径读 `Library/` 下的成品，
  把成品根排除掉会让它读不到自己的资产；将来若要让它也跳过忽略目录，把同一份列表
  传给它的 source 即可，接口已经就绪。
- **仍然会烘「可见但不是游戏资产」的目录**（`docs/`、`Preferences/`、`prototypes/`）：
  按本 ADR 的规则它们没有被忽略，所以它们属于「项目可见内容」。如果仓库不想产出这些
  成品，正确的做法是在 `.kairosignore` 里表达（代价是面板也看不见它们），而不是给 bake
  加一条面板不知道的特例。

## 实测

本仓库（项目根 = workspace 根）一次 `cargo run --bin bake_assets`：

- 日志：`bake: 3 directories ignored by …\.kairosignore: .cargo, .zed, target`；
- `target/` 下的 rustdoc 图不再产出成品；`.git` 在仓库根（项目根之外），本就不在扫描范围；
- 已提交的 10 个成品**逐字节不变**；
- 仍产出 230 个未提交成品（约 330 MB），其中 `Preferences/Textures` 96 个 160 MB、
  `docs/research` 74 个 91 MB、`prototypes/editor-ui` 20 个 79 MB。它们是「可见的项目内容」，
  按本 ADR 的口径属于要烘的范围；是否要缩小这个口径（例如把编辑器 UI 资源从资产源里排除）
  是后续决定，见「影响」最后一条。

## 关联

- [ADR 0004](./0004-asset-default-source-root.md) — 默认源根 = cwd（本 ADR 给这个根加上
  项目级排除规则）
- [ADR 0017](./0017-processed-root-under-library.md) — 成品根在 `Library/` 下；它的排除
  现在与项目忽略目录共用同一个列表
- [docs/plan/project-panel](../../plan/project-panel/README.md) — D10（`.kairosignore`）与
  「面板看不见的就不烘」的验收口径