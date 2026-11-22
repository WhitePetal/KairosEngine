---
Status: accepted
---

# bake 是编辑器功能：排除规则 = 项目浏览器的忽略规则，引擎只提供仪器

> 一句话：`.kairosignore` 是「项目拥有哪些目录」的唯一答案；bake 是编辑器侧的
> 命令，引擎侧不读项目配置。

## 背景

两件事被混在了一起。

**一、bake 的输入口径。** ADR 0004 把默认资产源根定在进程 cwd，于是 bake 会把
`res/` 之外一切能被 loader/processor 认领的东西都加工：`docs/` 的研究截图、
`Preferences/` 的图标、甚至 `target/doc` 的 rustdoc 图（实测一次 bake 额外产出 230 个
未提交成品，约 330 MB，外加一整套 `target/` 镜像）。其中 `target/` 恰恰是项目浏览器
按 `.kairosignore` 隐藏的目录——同一个项目，面板说「不属于我」，bake 说「我烘它」。

**二、bake 住在哪个 crate。** bake 的 bin 一直在 `kairos_engine`。但烘焙是**创作期**
工具，不是引擎运行时能力：引擎跑起来从不 bake。让引擎来读 `.kairosignore`，等于让
它依赖编辑器的项目策略语义；而 `kairos_editor_core` 又不能反向依赖引擎——这正是为什么
「引擎读忽略规则」只能靠往依赖图里塞一个新 crate 来绕开，而那是错的方向。

## 决策

1. **`.kairosignore` 是排除规则的唯一真相**，面板和 bake 共用同一份实现
   （`kairos_editor_core::project::IgnoreRules`），不新增 crate、不复制解析器。
2. **bake 归属编辑器**：`cargo run --bin bake_assets` 现在是
   `kairos_editor_app/src/bin/bake_assets.rs`。它读项目根的 `.kairosignore`，把被忽略
   的目录（每棵子树取最顶层那个）作为 `AssetSourceBuilder::unprocessed_excludes` 传给
   默认 source，再调用引擎的两件仪器跑完处理。命令名不变。
3. **引擎只提供仪器**：`kairos_engine::asset_pipeline` 导出
   `bake_world(builders)` 与 `run_bake(world)`（原本就存在，只是私有），删掉
   `bake_assets()` 入口与自己的 bin；它不知道 `.kairosignore`、也不知道项目根在哪。
4. **`kairos_asset` 的排除项变成列表**：一个 source 的
   `unprocessed_exclude: Option<PathBuf>` 改为 `unprocessed_excludes: Vec<PathBuf>`，
   因为同一个 source 现在要同时排除成品根（ADR 0017）与项目忽略的若干目录。这是
   「一个 source 可以排除多个子树」的通用化，不含任何项目概念。

## 考虑过的替代

- **把忽略规则下沉成新的共享 crate，让引擎直接读**：会多一个 workspace 成员，且把
  「项目策略」放进引擎的依赖面；方向错。
- **bake 留在引擎，加命令行参数传排除清单**：`cargo run --bin bake_assets` 不再是
  完整命令，漏传参数会悄悄少烘一批资产。
- **bake 只烘 `res/`（资产根白名单）**：把「资产是什么」写死进工具；「资产根」是将来
  工程清单/类型注册表该回答的问题，不是今天的 bake 该发明的概念。
- **bake 留在引擎但只当哑工具，忽略规则在别处生效**：那份规则最终还是要落到源上，
  绕不开。

## 影响

- 新 bin `kairos_editor_app/src/bin/bake_assets.rs`；`kairos_engine/src/bin/bake_assets.rs`
  删除，引擎的 `env_logger` 依赖随之删除（它只服务那个 bin）。
- 引擎 `asset_pipeline::{bake_world, run_bake}` 变成 `pub`；调用方与测试需要在两步之间
  注入事务日志（`FileTransactionLogFactory`），所以两步必须分开。
- 运行日志：bake 用 `kairos_log`，但**不写** `Library/Logs/KairosEngine.log`（bake 不是
  编辑器会话，不该截断编辑器正在写的日志）。
- 缺 `.kairosignore` 时用与面板完全相同的内置兜底（`.*`、`target`）；坏规则记 warn 并跳过。

## 实测（本仓库）

- `bake: 3 directories ignored by …\.kairosignore: .cargo, .zed, target`；
- `target/` 不再产生成品镜像；
- 已提交的 10 个成品**逐字节不变**（`git diff -- Library` 为空）；
- 仍为可见目录产出 230 个未提交成品（`Preferences/Textures` 96 个 160 MB、
  `docs/research` 74 个 91 MB、`prototypes/editor-ui` 20 个 79 MB）。**这些多出来的产物
  不做额外丢弃处理**：bake 就烘可见树，产物是否提交由人决定。

## 关联

- [ADR 0004](./0004-asset-default-source-root.md) — 源根 = 进程 cwd（本 ADR 给这个根加上
  项目级排除规则）
- [ADR 0017](./0017-processed-root-under-library.md) — 成品根在 `Library/` 下，它的排除与
  项目忽略目录共用同一个列表
- [docs/plan/project-panel](../../plan/project-panel/README.md) — D10（`.kairosignore`）