---
Status: accepted
---

# kairos_asset 热重载：watcher 后端、开关与偏离清单

## 背景与决策

`kairos_asset` 照 `bevy_asset` 0.19.1 重写后（map #184），`AssetSourceEvent` /
`AssetWatcher` / `watching_for_changes` 等类型已在，但没有任何监听后端。本决策为热重载定后端与接入：
**忠实移植 0.19.1 的 `io/file/file_watcher.rs`**——`FileWatcher`、泛型 over `FilesystemEventHandler`
的 `new_asset_event_debouncer`、`get_default_watcher`、`platform_default` 装默认 watcher；后端用
`notify-debouncer-full 0.7.0`（optional、`default-features = false`），平台分派照上游（Linux inotify /
macOS FSEvents 默认 / Windows `ReadDirectoryChangesW` / BSD kqueue / 其余 `PollWatcher`）。运行时开关经
`AssetOptions.watch_for_changes_override: Option<bool>`（生效值 = override 或 `cfg!(feature = "watch")`）。
feature 面照搬 `watch` / `file_watcher` / `embedded_watcher` 三档，`default = ["file_watcher"]`。
**并入嵌入资产**：照搬 `io/memory.rs`、`EmbeddedAssetRegistry`、`embedded://` 源与 `EmbeddedWatcher`，
以及 `embedded_path!` / `embedded_asset!` / `load_embedded_asset!` / `load_internal_asset!` /
`load_internal_binary_asset!` 宏。

## 刻意偏离（相对 bevy_asset 0.19.1）

1. **无 `App` / `Plugin`（ADR 0003）**：`embedded_asset!` 等宏收 `&mut World`；`GetAssetServer` 只实现
   `World` / `AssetServer`。
2. **`get_base_path()` 取本地 cwd 语义（ADR 0004）**，env override 用 `KAIROS_ASSET_ROOT`；不采纳上游
   `CARGO_MANIFEST_DIR` / exe-parent 两级。
3. **去掉 `multi_threaded` feature**：`IoTaskPool` 恒多线程（ADR 0001），该 feature 无对应门控。
4. **不写 wasm32 门控**：当前不考虑 web 平台；Android 照上游明确「不支持 watch」。
5. **主 server 忽略 `RenamedAsset` / `RenamedMeta`**（照上游 `_ => {}`）：纯 rename 不触发主 server 重载，
   rename 的处理器在 `AssetProcessor` 侧。已知风险——只产出纯 `Rename(Both)` 的编辑器原子保存可能不重载，
   实现票补一条真实编辑器的运行时验证。
6. **embedded meta 不热重载**（照上游）：改 `.meta` 只 warn，不重读。
7. **去抖 300ms 硬编码**、不加更上层节流、不暴露配置（照上游）。
8. **编辑器项目树另起一条 watcher**：`AssetSourceEvent` 不升为公开广播面；已加载资产面走既有
   `AssetEvent`。这是本地决策（上游未把编辑器刷新纳入 `bevy_asset`）。（落地：`kairos_editor`
   的 `project_watcher::ProjectTreeWatcher` 监听 `ProjectPathGraph::scan_root`，
   `ui::Context::handle` 每帧排空一次并让项目树重扫；重扫只在注册了新 GUID 时写回注册表，
   否则注册表自身的写入会把刚叫醒它的 watcher 再叫一次。因此 `kairos_editor` 也直接依赖
   `notify-debouncer-full`。）

   > **2026-09 修订**：宿主拆分后归属变化——`ProjectTreeWatcher` 与 `notify-debouncer-full` 直接依赖均由 `kairos_engine` 移到 `kairos_editor`；决策本身未变。
9. **`get_asset_path` 在根拼写不匹配时回退到 canonical 根**：macOS FSEvents 报告的事件路径会解析
   符号链接——根写作 `/tmp` 时报 `/private/tmp/...`，而登录会话给的 `TMPDIR=/var/folders/...`
   会被报成 `/private/var/...`。上游只按原样根做 `strip_prefix`（失败即 panic），于是这类环境下
   所有事件都匹配不上、热重载失效。本地在 `strip_prefix` 失败后再用 `canonicalize(root)` 试一次；
   事件语义不变，只是让符号链接根也能映射成功。

## 考虑过的替代

- **本地自研 watcher 层**（只依赖 `notify`、自建去抖与 rename 配对）：更贴本地，但去抖与跨平台 rename
  配对最易写错，且会与上游对照持续分叉。
- **仅 `watch` 一档 feature**：省一层，但丢掉「有 watch 无后端」的 `watch_warning` 诊断语义。
- **`embedded_watcher` 出界**：因 `EmbeddedWatcher` 依赖注册表与 handler 重建、本地嵌入源为空，出界等于
  该后端无从设计；故纳入并补齐运行时闭环，宏/构建期工具一并随迁。

## 影响

- 新依赖 `notify-debouncer-full 0.7.0`（随 `notify 8.2.0` / `notify-types 2.0.0` / `file-id 0.2.3` /
  `walkdir` / `log`），仅非 web 目标。
- `AssetInfos` 在 `watching_for_changes` 下多维护 `loader_dependents`、labeled / folder 寻址，属额外存储
  成本；`StrongHandle` 增存 `meta_transform`、`path` 与 `asset_server_managed`，因此失去 `Debug` derive。
- 实现切片多出「嵌入资产（memory 后端 + 宏 + 嵌入源 + embedded watcher）」一块，落地顺序归 map #214 的
  #223。
- folder 重载的端到端验证依赖 #215 的 `load_folder` 实现票先落地。

## 关联

- [ADR 0001](./0001-asset-async-runtime-kairos-tasks.md) — 异步运行时与去 tokio
- [ADR 0003](./0003-asset-registration-and-stage-ownership.md) — 无 App、stage 由调用方注入
- [ADR 0004](./0004-asset-default-source-root.md) — 默认 source root = 进程 cwd
