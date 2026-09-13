---
Status: accepted
---

# 默认资产 source root = 进程工作目录（偏离 bevy 的 "assets"）

## 背景与决策

kairos 现有资产路径全是**进程 cwd 相对**字面量（`res/models/Suzanne.mesh`、`Preferences/SublimeSyntax/rust_syntax.toml`），旧栈没有 source-root 抽象（`tokio::fs::read(path)` 直接解析）。照 bevy 引入 `AssetSource` 后，默认 source root 取 **进程工作目录（`""`）**，于是 `AssetPath::from("res/...")` 解析到与今天相同的文件。

## 考虑过的替代

- **照 bevy 默认 `"assets"`**：所有现有字面量都要加/改前缀，`Preferences/...` 也要挪，迁移期路径改写面大。
- **默认 root 取 `res/`**：`Preferences/...` 落到 root 之外，需第二 source。

## 影响

- #185 迁移期**零路径改写**；`AssetPath` 的 source 字段仍保留，将来可加多 source。
- `UnapprovedPathMode` 默认 `Forbid`（拒绝 `..` 逃逸出 source root）；现有路径全在 cwd 之下，不受影响。若将来需读 cwd 之外的用户工程目录，改 `Allow`。
- `install(world, AssetOptions)` 收下 tracking/event/startup stage 与 `mode`/`meta_check`/`use_asset_processor`/`watch_for_changes_override`；默认 `AssetMode::Unprocessed`（cwd root，无 processed root）、`AssetMetaCheck::Always`、`use_asset_processor` 关；`install` 会在 sources 冻结前经 `EmbeddedAssetRegistry` 注册 `embedded` source，`get_base_path()` 取进程 cwd（`KAIROS_ASSET_ROOT` 可覆盖）。`AssetMode::Processed` 时默认 source 加成品 root `imported_assets/Default`（可用 `AssetOptions::with_processed_file_path` 改）。因成品 root 嵌套在 cwd root 内，默认 source 会排除成品路径的顶层目录（`imported_assets`，含 WAL `log`），processor 不再把成品当源二次加工（ADR 0005 偏离 12，见 #239）。
- #239 迁移后，图形资产的加载字面量从源侧 wrapper（`res/models/Suzanne.mesh`）改为成品路径（`imported_assets/Default/res/models/Suzanne.glb`）；宿主机仍跑 `AssetMode::Unprocessed`，直接以成品 `.meta` 的 `AssetAction::Load` 读成品，成品的唯一写者仍是 processor。
