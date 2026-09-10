---
Status: accepted
---

# kairos_asset 的异步运行时改用 kairos_tasks，去 tokio 依赖

## 背景与决策

`kairos_asset` 正照 `bevy_asset` 0.19.1 重写（map #184）。bevy 的加载任务跑在 `bevy_tasks` 的 `IoTaskPool` 上，其 IO 抽象 `AssetReader` 基于 futures-io，loader 返回 `ConditionalSendFuture`；而 `kairos_tasks` 基本是 `bevy_tasks` 的移植。因此本 crate 的异步运行时定为 **`kairos_tasks`**：加载任务走 `IoTaskPool`，IO 读取 over futures-io + async-fs，`kairos_asset` **不再依赖 tokio**。旧的自研资产栈、以及引擎其余部分目前走 tokio（`main` 是 `#[tokio::main]`），那是被本次重写替换的旧路径。

## 考虑过的替代

- **保留 tokio（现状）**：`tokio::spawn` 隐式依赖运行中的 tokio runtime 上下文，且与已定的 futures-io `AssetReader`、`ConditionalSendFuture` 劈叉——同一个加载抽象里混两套 async 生态。

## 影响

- `kairos_tasks` 需补 `ConditionalSend` / `ConditionalSendFuture`（本 effort 的前置；`block_on` / `poll_once` / `futures_lite` 再导出等随消费方再补）。
- 加载任务的保活/取消照搬 bevy：`infos.pending_tasks` 保活防取消、handle 掉光即 `return`；**暂不搬** bevy 的 wasm / 无 `multi_threaded` `detach` 特例，因目标设备默认多线程。
- 本决策只约束 `kairos_asset` 的资产加载路径；引擎其余部分是否去 tokio 不在其中。
