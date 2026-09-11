---
Status: accepted
---

# kairos_asset 照 bevy_asset 0.19.1 重写：策略与偏离清单

## 背景与决策

旧的自研资产栈（`AssetsServer` / `AssetsSystem` / `AssetsHandler` / `AssetHandle` /
`AssetIndex` / 每类型 `Assets` + tokio channel）在异步模型、注册方式、事件面、
依赖与热重载上都与 bevy 劈叉，且把通用机制与具体资产类型搅在同一个 crate 里。
本 effort（map #184）把 `kairos_asset` **整体照 `bevy_asset` 0.19.1 重写**，作为
引擎唯一的资产核心。

**落地策略**：新旧并存、增量迁移、末尾收口，而非一步换血。

- P0/P1：新核心先进驻 `kairos_asset::next`（临时命名空间，零消费），逐层补齐
  身份与句柄、存储与事件、IO/路径/meta、加载、注册与调度（#200–#206）。
- P2：按资产类型分片迁移，每片把类型自己的 loader/注册搬到类型所属 crate
  （#207–#212）。新核心此时已被引擎消费，旧栈仍在旁路存活。
- P3（本票）：删旧栈、把 `next` 提升为 crate 根、清理过渡再导出与 `handle()`
  时序、落下本 ADR。

**删旧栈**：`AssetsServer` / `AssetsSystem` / `AssetsHandler` / `AssetHandle` /
旧 `AssetIndex` / `RecyledAssetIndex` / 旧 `AssetLoader` / `LoadedEvent` /
`DropEvent`（旧义） / `DependencyLoadRequest*` / `kairos_asset::consts` 全部删除；
`consts` 里只有 `AssetsServer` 用的 channel-buffer 常量随之作废，各类型的容量
常量留在类型所属 crate 就近定义。

**提升 `next`**：`kairos_asset::next::…` → `kairos_asset::…`，临时命名空间不保留。
引擎经 `pub use kairos_asset as asset` 以 `crate::asset::…` 取用；`asset_loader`
过渡 facade（`assets` / `asset` / `consts` 三个再导出模块）与 `kairos_graphics::assets`
的通用机制再导出整模块删除，不留过渡别名。

**去 tokio**（ADR 0001）与旧栈同生共死：删完旧栈后 `cargo tree -p kairos_asset`
不再含 tokio。

**收口 `handle()`**：旧栈靠手摇 `AssetsServer::handle()` 推进加载与依赖回填，
编辑器在 `update` 后、`handle_ui` 后、帧末、`on_exit` 四处调用它。新核心的加载
结果、句柄回收与运行时插入全部由 `PreUpdate` 的 `handle_internal_asset_events`
与每类型 `track_assets` 驱动，`PostUpdate` 只把队列刷成 `Messages`。故四处
`handle()` 全部删除，资产插入只发生在 `PreUpdate`。

**`modify_count` 退场**：渲染缓存曾靠每帧比对 `version`/`modify_count` 数字失效。
新核心改事件驱动（`AssetEvent::{Modified,Removed}` 汇总进 `GraphicsAssetEvents`），
`modify_count` 无代码残留。

**`AssetKind` 与 inspector**：编辑器资产分类 `AssetKind` 不动；inspector 取资产
改走 `&mut World` / `&World`（`AssetServer` 与 `Assets<A>` 都是 World 资源），
不再被外部逐个传 `AssetsServer` 引用。

## 偏离清单（相对 bevy_asset 0.19.1）

1. **没有 `App` / `Plugin`**：注册面做成 `World` 的扩展 trait
   `AssetWorldExt::{init_asset, init_asset_with_capacity, register_asset_loader,
   init_asset_loader}`，对位 `AssetApp`。stage label 的家在依赖上游，详见 ADR 0003。
2. **stage 由调用方注入**：`install(world, tracking_stage, event_stage)` 把
   `PreUpdate`/`PostUpdate` 收进 `AssetStages` World 资源，`MainScheduleOrder` 不动。
   这是相对 bevy（`AssetPlugin` 直挂）的唯一结构性偏离，详见 ADR 0003。
3. **异步运行时是 `kairos_tasks`，不是 tokio**：加载任务跑 `IoTaskPool`，IO 走
   futures-io + async-fs，loader 返回 `ConditionalSendFuture`。详见 ADR 0001。
4. **meta 统一为 `.meta` 边车（RON）**：退役每类型 TOML wrapper，
   `AssetMetaCheck` 默认 `Always`。详见 ADR 0002。
5. **默认 source root = 进程 cwd**，不是 bevy 的 `"assets"`；`UnapprovedPathMode`
   默认 `Forbid`。详见 ADR 0004。
6. **`Asset` 不含 `TypePath`**：加载器名以 `std::any::type_name` 为准。
7. **句柄非 `Copy`**：`Handle<A>` 内部的 `Arc` 就是引用计数，最后一个强句柄
   析构才发 `DropEvent`；命名可先于存在（弱句柄只带 id）。
8. **明确延后**的部分：未类型化/整目录加载、`add_async` 与 `Handle` guard、
   `wait_for_asset*`、`AssetProcessor`。迁移期保留现有手动加工管线（ADR 0002）。

## 考虑过的替代

- **大爆炸换血**：一次删旧栈、一次接新核心。收敛快，但任一资产类型迁移出问题
  都会拖住整条链路，且无法逐片验证行为不变。
- **新旧并存但不收口**：把 `next` 与过渡再导出长期保留。省一次收口，但两套
  API 并存会让 import 面持续分叉，`handle()` 时序也退不掉。
- **保留 tokio**：见 ADR 0001，与 futures-io `AssetReader`、`ConditionalSendFuture`
  劈叉。

## 影响

- `kairos_asset` 成为唯一资产核心，且不依赖 tokio；`AssetKind`、`kairos_asset/CONTEXT.md`
  的领域词表均不变。
- 引擎引导顺序固定为 `schedule::install` →
  `kairos_asset::install(world, PreUpdate, PostUpdate)` → 各 crate 在自己的
  `install` 里 `init_asset` / `register_asset_loader`。
- 编辑器的资产时序收口到 `PreUpdate`，四处手摇 `handle()` 消失；渲染缓存改吃
  `AssetEvent`。
- 后续资产类型只需在自己的 crate 里 `install`，不必改 `kairos_asset`。

## 关联

- [ADR 0001](./0001-asset-async-runtime-kairos-tasks.md) — 异步运行时与去 tokio
- [ADR 0002](./0002-asset-meta-sidecar-ron.md) — `.meta` 边车
- [ADR 0003](./0003-asset-registration-and-stage-ownership.md) — 注册与 stage 归属
- [ADR 0004](./0004-asset-default-source-root.md) — 默认 source root
