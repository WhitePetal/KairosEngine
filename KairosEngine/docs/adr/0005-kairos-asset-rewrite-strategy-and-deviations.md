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
   preregister_asset_loader, init_asset_loader, register_asset_source,
   register_asset_processor, set_default_asset_processor}`，对位 `AssetApp`；与上游一样，
   各方法返回 `&mut Self` 以便链式调用。stage label 的家在依赖上游，详见 ADR 0003。
2. **stage 由调用方注入**：`install(world, AssetOptions)` 把
   `PreUpdate`/`PostUpdate`/`Startup` 收进 `AssetStages` World 资源，`MainScheduleOrder` 不动。
   这是相对 bevy（`AssetPlugin` 直挂）的唯一结构性偏离，详见 ADR 0003。
3. **异步运行时是 `kairos_tasks`，不是 tokio**：加载任务跑 `IoTaskPool`，IO 走
   futures-io + async-fs，loader 返回 `ConditionalSendFuture`。详见 ADR 0001。
4. **meta 统一为 `.meta` 边车（RON）**：退役每类型 TOML wrapper，
   `AssetMetaCheck` 默认 `Always`。详见 ADR 0002。
5. **默认 source root = 进程 cwd**，不是 bevy 的 `"assets"`；`UnapprovedPathMode`
   默认 `Forbid`（可由 `AssetOptions::unapproved_path_mode` 覆盖，对位上游
   `AssetPlugin::unapproved_path_mode`）。详见 ADR 0004。
6. **`Asset` 不含 `TypePath`**：加载器名以 `std::any::type_name` 为准。
7. **句柄非 `Copy`**：`Handle<A>` 内部的 `Arc` 就是引用计数，最后一个强句柄
   析构才发 `DropEvent`；命名可先于存在（弱句柄只带 id）。
8. **`add_async` 与 `wait_for_asset*`**：随后续 effort #214 的 A4/A6 片落地（#235）；
   `DirectAssetAccessExt`（A7）与默认 loader `.meta` 写出（A8）同批落地。
   （未类型化加载与 `Handle` guard 已随 A2 片落地；整目录加载
   `load_folder` 与 `LoadedFolder` 已随 A3 片落地；loader 门控与异步 loader 查询（A5，#234）已落地，
   其缺 loader 的错误面（`AssetLoadError` 的 `MissingAssetLoaderFor{Extension,TypeName,TypeId}Error`）
   由结构体变体改元组变体，属破坏性变更，下游同步见
   [docs/research/asset-load-error-shape-change.md](../research/asset-load-error-shape-change.md)；
   `AssetProcessor` 本体已随加工主轴 S5（#231）落地，
   写前日志（WAL）与启动恢复已随 S6（#232）落地，gated 成品 reader 与布局②接线已随 S7（#233）落地，
   现有手动加工管线的收编归 S8/S9。产物的生成入口在宿主侧：`kairos_engine::asset_pipeline`
   按布局②跑一遍 processor（`cargo run --bin bake_assets`），运行时宿主本身仍走布局①，
   按路径读 `imported_assets/Default` 下的成品。）
9. **`load_untyped_async` 也执行 `UnapprovedPathMode` 门**：上游 `LoadBuilder` 的该
   方法不查未批准路径；kairos 让它与其余加载入口一致，`Forbid` 一律拒绝
   （`Deny` 仍可被 `override_unapproved` 覆盖）。
10. **被忽略 / 无扩展名 / 源已消失的资产在 gated reader 面前标为 `NonExistent`**：
    上游 `finish_processing` 对这些结果不落状态，`ProcessorGatedReader` 的
    `wait_until_processed` 会永久等待；kairos 把它们标为 `NonExistent`，让成品读取
    立即得到 `NotFound`（#233 要求「无成品时按三态给出明确结果」）。
11. **`wait_for_asset*` 在依赖失败时也唤醒**：上游只在某个资产的
    `LoadedWithDependencies` 或该资产自身加载失败时唤醒它的等待任务；等待**依赖方**的
    任务在依赖已失败后不会被唤醒。kairos 在失败沿依赖树向上传播时（`propagate_failed_state`）
    以及资产带着已失败的依赖树落定时唤醒，`WaitForAssetError::DependencyFailed`
    因此可被观测。
12. **默认 source 排除成品子树**：ADR 0004 把默认 source root 定在进程 cwd，
    成品 root `imported_assets/Default` 因此落在未处理 root 之内；上游 bevy 的
    `assets/` 与 `imported_assets/` 互不嵌套，无此问题。kairos 让
    `AssetSourceBuilder::platform_default`
    在成品 root 嵌套于未处理 root 时，自动把成品路径的顶层目录（`imported_assets`，
    连带旁边的 WAL `log`）记为 source 的 `unprocessed_exclude`；processor 的
    初扫与 `AssetSourceEvent` 处理都跳过它，成品不会被当源二次加工（#239）。

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
  `kairos_asset::install(world, AssetOptions::new(PreUpdate, PostUpdate, Startup))` → 各 crate 在
  自己的 `install` 里 `init_asset` / `register_asset_loader`。
- 编辑器的资产时序收口到 `PreUpdate`，四处手摇 `handle()` 消失；渲染缓存改吃
  `AssetEvent`。
- 后续资产类型只需在自己的 crate 里 `install`，不必改 `kairos_asset`。

## 关联

- [ADR 0001](./0001-asset-async-runtime-kairos-tasks.md) — 异步运行时与去 tokio
- [ADR 0002](./0002-asset-meta-sidecar-ron.md) — `.meta` 边车
- [ADR 0003](./0003-asset-registration-and-stage-ownership.md) — 注册与 stage 归属
- [ADR 0004](./0004-asset-default-source-root.md) — 默认 source root
