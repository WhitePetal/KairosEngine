---
Status: accepted
---

# 成品根搬进 `Library/`：`Library/imported_assets/Default`

> 本 ADR 只改 ADR 0004 / 0005 偏离 12 里**成品根的位置**这一条结论；源根 = 进程 cwd、
> 双树双 `.meta`、`unprocessed_exclude` 机制与其余结论都不变。

## 背景与决策

旧布局把加工产物放在仓库顶层 `imported_assets/Default`，与源树 `res/` 并列。
ProjectPanel 迁移（`docs/plan/project-panel`，D12）要求面板把成品当**系统产物**而不是项目内容：
项目树只有一个系统目录 `Library`，它在根级排最后、带分隔线、有独立节点预算、不受
`.kairosignore` 影响。只要成品根还在顶层，面板的 `ScanPolicy` 就得多一个「跳过成品根」字段，
并额外解释「为什么这个目录不显示」。

**决策**：成品根从 `imported_assets/Default` 搬到 `Library/imported_assets/Default`，一次性搬完；
面板侧从此不需要任何成品特例。

搬迁是纯位置改动，没有行为变化：

- 仓库：`git mv imported_assets Library/imported_assets`（成品提交进仓库，只有 WAL 被 gitignore）；
- 常量/字面量/数据（10 处）：`AssetOptions::DEFAULT_PROCESSED_FILE_PATH`
  （`kairos_asset/src/install.rs`）、processor 的 `LOG_PATH`（`kairos_asset/src/processor/log.rs`）、
  `kairos_engine::kairos_paths::PATH_WHITE_TEXTURE`、`kairos_graphics::render_pipeline::PATH_WHITE_TEXTURE`、
  `kairos_game.rs` 的两处 `load` 字面量、旧编辑器的 `DEFAULT_PREVIEW_MESH`、
  `res/materials/material.mat` 的 `texture_path`、`Preferences/Styles/Inspectors/Material.toml`
  的 `preview_meshes`，以及旧编辑器 `product_directory_is_hidden` 的测试夹具（规则不变，
  夹具跟着成品目录改名）；
- `.gitignore`：`/imported_assets/log` → `/Library/imported_assets/log`（`/Library/Logs`、
  `/Library/Profiler` 两条不变）；
- 文档注释：`bake_assets`、`asset_pipeline`、`kairos_asset::io::source` 等提到旧路径的地方同步。

## 语义确认

- **`processed_subtree_exclude` 现在返回 `Library`**：它取成品路径相对源根的第一段。因此默认
  asset source 的未处理排除变成**整个 `Library` 目录**（连带 `imported_assets/log` 与 `Logs/`）。
  这正是想要的：资产源永不包含 `Library`，asset watcher 也不再盯它。
- **WAL 跟着走**：`<cwd>/Library/imported_assets/log`，仍在 gitignore 内。
- **`KAIROS_ASSET_ROOT` 覆盖不变**：成品根仍是相对 base path 的字面量，换根时两者一起变。
- **bake 仍需重跑一次**：`cargo run --bin bake_assets` 会在新路径下重新生成成品；
  由于成品是提交进仓库的，这次搬迁的 `git mv` 之后紧接着必须 rebake 才能保证内容一致
  （内容不变时 git 只记录改名）。

## 考虑过的替代

- **留在顶层，面板加「跳过成品根」字段**：少一次迁移，但把「成品」这个概念塞进面板的扫描策略，
  还要在 UI 上解释它为什么不在；M0.5 只花半天就能永久删掉这条特例。
- **搬到 `Library/Processed/`（同时改成品子树结构）**：搬迁代价一样，但顺带改布局会把 `.meta`
  与路径镜像一起动，超出「只搬位置」的范围；成品子树结构留给将来的布局决策。
- **成品放进工程外缓存（Unity 的 `Library/` 式不可见缓存）**：需要「打开工程」与重建缓存的概念，
  而当前编辑器还没有工程概念（D9 保持 cwd）；现在做会把未定的工程模型提前钉死。

## 影响

- **`kairos_asset` / `kairos_engine` / `kairos_graphics`**：只改常量与字面量，测试里用 tempdir 自建根
  的用例不受影响（它们显式传成品路径），断言默认成品路径的用例已同步。
- **旧编辑器（`kairos_editor`，已冻结）**：`AssetRegistry` 通过
  `AssetOptions::DEFAULT_PROCESSED_FILE_PATH` 自动跟到新路径；但它的项目树里
  「跳过顶层成品目录」规则不再匹配任何目录，于是 `Library/` 会作为普通文件夹显示。
  这是冻结实现的可接受退化——它随 ProjectPanel 迁移一起退役。只为补 `DEFAULT_PREVIEW_MESH`
  改了一处路径字面量，未动任何逻辑。
- **ProjectPanel**：`ScanPolicy` 不再需要「跳过成品根」字段；`Library` 成为唯一的系统目录。
- **CI / 新检出**：`Library/imported_assets/Default` 随仓库提交，`git status` 不会因为第一次跑
  编辑器而变脏（`/Library/imported_assets/log` 已忽略）。

## 关联

- [ADR 0002](./0002-asset-meta-sidecar-ron.md) — `.meta` 边车（不变）
- [ADR 0004](./0004-asset-default-source-root.md) — 源根 = cwd；成品根位置由本 ADR 取代
- [ADR 0005](./0005-kairos-asset-rewrite-strategy-and-deviations.md) — 偏离 12（成品子树排除），
  排除结果由 `imported_assets` 变成 `Library`
- [ADR 0018](./0018-asset-source-follows-kairosignore.md) — 资产源的排除规则改由 `.kairosignore`
  统一提供；成品根的排除现在与项目忽略目录共用同一个列表
- [docs/plan/project-panel](../../plan/project-panel/README.md) — D12，本 ADR 的服务对象