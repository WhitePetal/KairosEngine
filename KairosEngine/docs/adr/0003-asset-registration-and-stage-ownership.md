---
Status: accepted
---

# 资产注册与调度的 stage 归属：调用方传入 stage label，SystemSet 归 kairos_asset

## 背景与决策

`kairos_asset` 正照 `bevy_asset` 0.19.1 重写（map #184）。bevy 里 `AssetPlugin` 直接往 `PreUpdate`/`PostUpdate` 挂资产系统，因为 `bevy_asset` 依赖 `bevy_app`——stage label 的家。kairos 里 `MainScheduleOrder` 与 `PreUpdate`/`PostUpdate`/`FixedPostUpdate` 住在 **`kairos_engine`**，而 `kairos_asset` **不能依赖 engine**（engine 经 graphics 间接依赖 asset，反向成环）。

故资产 stage 不归 `kairos_asset`，改由调用方传入：`kairos_asset::install(world, tracking_stage, event_stage)`，engine 传自己的 `PreUpdate`/`PostUpdate`，`MainScheduleOrder` **不动**。`AssetTrackingSystems`/`AssetEventSystems` 作为 `SystemSet` 定义在 `kairos_asset`，系统以 `in_set` + `configure_sets` 落进调用方给的 stage——即 bevy 的做法，只是 stage label 由外部注入。

每类型注册对位 `AssetApp`，做成 `World` 的扩展 trait：`init_asset::<A>()` / `register_asset_loader::<L>()`。因 kairos 无 `App`，`install` 把两个 `InternedScheduleLabel` 存进 `AssetStages` World 资源，供此后任意时点的 `init_asset` 读取——这是相对 bevy 的唯一结构性偏离。

## 考虑过的替代

- **engine 往 `MainScheduleOrder` 插资产专属 stage**（`AssetTracking`/`AssetEvent`）：偏离 bevy 落点，多两个只为 asset 存在的 stage，`Extract` 相对位置要重推。
- **`kairos_asset` 依赖 `kairos_engine`**：可直呼 stage label，但与 graphics 形成依赖环。

## 影响

- engine 引导顺序被固定：`schedule::install` → `kairos_asset::install(world, PreUpdate, PostUpdate)` → `physics::install` → `graphics::install`（各 crate 在自身 install 里 `init_asset`）。
- 连带新增 `FixedPostUpdate` schedule：fixed-step 后处理的家，`signal_message_update_system` 落于此（对齐 bevy `TimePlugin`），为后续物理模块留口。
- `install` 与 `AssetStages` 使「先 install、后任意处 `init_asset`」成立，注册点跟资产类型所属 crate 走。
- 分期见 #192：新核心 P0 零消费，P1 才接这条缝。
