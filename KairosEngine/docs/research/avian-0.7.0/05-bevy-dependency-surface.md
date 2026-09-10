# Avian 0.7.0 的 Bevy 依赖面：把它的架构搬到 Kairos ECS/Math 栈上的可行性与摩擦（#05）

> 上游依据：Avian 只读 checkout `/Users/baiaoxiang/KairosEngine/.scratch/avian-research/avian-0.7.0`，tag `v0.7.0`，commit `965e85bf590f53fbf8a29f7ebd0326b5dbc985d1`（`git log -1` 确认）。
> 注意：`crates/avian3d/Cargo.toml:74-76` 里 `[lib] name = "avian3d"` / `path = "../../src/lib.rs"`，所以 **2D/3D 共用仓库根的 `src/`**，`#[cfg(feature = "2d")]` / `#[cfg(feature = "3d")]` 二选一编译。本文所有 `src/...` 路径都相对该 repo 根。
> 本地依据：Kairos workspace `/Users/baiaoxiang/KairosEngine/KairosEngine`（`kairos_ecs` / `kairos_math` / `kairos_transform` / `kairos_time` / `kairos_physics`）。`kairos_*` 路径相对 workspace 根。
> 第三方 crate 依据：本地注册表 `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`（本文只读到了 `parry3d-0.28.0`、`glamx-0.3.0`、`rapier3d-0.33.0`；**`bevy_heavy`、`bevy_transform_interpolation`、`obvhs`、`glam_matrix_extras`、`parry3d 0.27` 本地均未 vendored**，见 §8）。凡涉及这些 crate 的依赖形状，依据分两类并明确标注：**(i)** 两份 `Cargo.lock` 的解析结果（`path:LINE`）；**(ii)** docs.rs 上的 `Cargo.toml` 原文（给出 URL）。
> 方法：**全部静态阅读**（`grep` / `find` / `wc` / `sort -u`）。**未运行 `cargo build` / `cargo test` / `cargo add`，未联网拉取依赖**。所有"能编译 / 不能编译"的判断都是类型层面的静态推理，不是运行验证。

---

## 0. 结论速览

- **必须先纠正一个前提**：Avian 0.7.0 **没有任何 `bevy_*` 子 crate 依赖**。`crates/avian3d/Cargo.toml:82-85` 只依赖 umbrella crate `bevy = { version = "0.19.0", default-features = false, features = ["std", "bevy_log"] }`，外加一个直接依赖 `bevy_math = { version = "0.19.0", features = ["approx"] }`（`:86`）。全 `src/` 里 `bevy_ecs` / `bevy_transform` / `bevy_reflect` 这些 token **出现次数为 0**；代码一律写 `bevy::ecs::…`、`bevy::transform::…`、`bevy::reflect::…`。**任何"Avian 依赖 bevy_ecs"的移植计划都必须先改成"Avian 依赖 `bevy` 这个 umbrella crate 的 re-export 面"。**
- **Avian 的体积**：`src/` 共 **128 个 `.rs` 文件、52 307 行**。其中文档注释行 13 856、空行 5 133、专门的测试文件（`src/**/tests.rs` + `src/tests/*.rs`，5 个文件）2 141 行；**扣掉这三类的"实体代码"约 31 000 行**（「估算」，见 §1 口径）。
- **最反直觉的一条事实**：Avian 的**算法核心几乎是 bevy-free 的**。全 `src/` 只有 **205 行非注释代码**出现 `bevy` 字样（分布在 114 个文件），其中：
  - `src/dynamics/solver/`（含 `islands` / `contact` / `joint_graph` / `xpbd` / `solver_body`）**8 623 行里只有 30 行**；
  - `src/collider_tree/` **2 944 行里只有 7 行**；
  - `src/collision/narrow_phase/` **1 505 行里只有 2 行**；`src/collision/broad_phase/` **511 行里只有 2 行**；
  - `src/dynamics/integrator/` **630 行里只有 5 行**；`src/dynamics/ccd/` **770 行里只有 2 行**；
  - **12 个文件完全不含 `bevy`**（3 567 行），包括 `src/data_structures/graph.rs`（1 114 行）、`src/dynamics/solver/xpbd/{angular,positional}_constraint.rs`、`src/character_controller/velocity_project.rs`（468 行）。
  结论：**要改的不是物理，是胶水层和数学类型。**
- **最大的好消息**：`kairos_ecs` 是 **`bevy_ecs` 0.19 的完整 fork**（238 个 `.rs` / 113 063 行，含测试），不是"风格相似的小 ECS"。§3 逐项核对后，Avian 用到的 `QueryData` / `SystemParam` / `QueryFilter` / `SystemState` / `RelationshipTarget` / required components / observer / `Message` / `EntityEvent` / `ParallelCommands` / `par_iter_mut` / `SQuery`·`SRes`·`Read`·`Write` lifetimeless 别名 / `Single` / `Populated` / `Disabled` / `SystemChangeTick` / `CheckChangeTicks` **全部存在且同名**（逐条 `kairos_ecs/src/...:LINE` 见 §3、§6.1）。`kairos_tasks` 同样是 `bevy_tasks` 的 fork，`ComputeTaskPool` / `ParallelSlice` 齐备。
- **三个真正的硬阻塞**（不是胶水，是缺模块）：
  1. **泛型时钟 `Time<T: Clock>`**。Avian 要 `Time<Physics>`（`src/schedule/mod.rs:61`）和 `Time<Substeps>`（`src/dynamics/solver/schedule.rs:20`、`:199-205`），并在 `run_physics_schedule` 里来回换 generic `Time`（`src/schedule/mod.rs:236-283`）。Kairos 的 `Time`（`kairos_time/src/lib.rs:36`）和 `FixedTime`（`:178`）是**两个具体结构体**，没有 `Clock` trait、没有 `Time<T>`、没有 `advance_by` / `relative_speed` / `as_generic`。
  2. **Transform 传播系统不存在**。Avian 直接调用 bevy_transform 的三个系统 `mark_dirty_trees` / `propagate_parent_transforms` / `sync_simple_transforms`（`src/physics_transform/mod.rs:26` 导入、`:98-100` 注册），并用 `TransformHelper` + `ComputeGlobalTransformError`（`src/physics_transform/helper.rs:9`、`:33`）。`kairos_transform/src/lib.rs:17-26` 明写传播系统 **"has not landed yet"**，且 `GlobalTransform` 被标注"不要 spawn、不要读"（`kairos_transform/src/global_transform.rs:13-22`）。
  3. **`glam_matrix_extras` 被钉死在 glam 0.32**。`glam_matrix_extras 0.3.0`（**最新版**）的 `[dependencies.glam] version = "0.32"`（docs.rs 源码页），Avian lock 解析为 `glam 0.32.1`（`Cargo.lock:3094-3105`，glam 引用于 `:3101`）。Kairos 全栈在 `glam 0.33.1`（`Cargo.lock:1768-1772`，`kairos_math` 引用于 `:2570`）。而 `SymmetricMat2/3`、`SymmetricDMat2/3` 在 Avian 核心里被用了 **73 处非注释代码**（`src/dynamics/rigid_body/mass_properties/components/*.rs`），是 angular inertia 的存储类型 → **必须 fork 或重写 `glam_matrix_extras`**。
- **第二个 glam 层面的坏消息（但有解）**：`obvhs 0.3.1`（Avian 锁定的版本）要求 `glam = ">=0.30.10, <0.32"`（docs.rs 0.3.1 源码页），所以 Avian lock 里 **`obvhs` 独自落在 `glam 0.31.1`**（`Cargo.lock:4418-4429`，`"glam 0.31.1"` 在 `:4424`），而其余全栈是 0.32.1。这就是 `src/collider_tree/obvhs_ext.rs:1` 里 `use bevy_math::Vec3A;` 与 `obvhs::aabb::Aabb` 两套 glam 类型并存的原因（该文件 `:20-22` 用 `Vec3A`，`:31` 收 `Aabb`）。**`obvhs` 最新版 0.3.3 的 glam 约束是 `">=0.30.10, <0.34"`（docs.rs 源码页）→ 0.3.3 直接支持 glam 0.33**。所以这一条是"升级 obvhs"而不是"fork obvhs"。
- **两个 crate 的 ECS 耦合度差别巨大**（这是本节最有操作价值的区分）：
  - `bevy_heavy 0.5.0` 的依赖是 `approx, bevy_math, bevy_reflect, glam_matrix_extras, serde`（`Cargo.lock:1011-1019`）——**没有 `bevy_ecs`**。它是**纯数学 crate**，只是绑在 bevy_math/glam 0.32 上。
  - `bevy_transform_interpolation 0.5.0` 的依赖是 `bevy, serde`（`Cargo.lock:1756-1761`）——依赖**umbrella `bevy`**，即 **`bevy_ecs` 强耦合**，只能重写。
- **`Reflect` 是可以整层丢掉的**：`#[derive(Reflect)]` 出现 171 次、`#[reflect(...)]` 属性 161 次，但非注释代码里 `register_type` 只有 **3 次**（`src/schedule/mod.rs:61`、`src/dynamics/solver/schedule.rs:20`、`src/ancestor_marker.rs:23`），且**没有任何核心类型把 `Reflect` 用作 trait bound**（`grep` 全库只命中 `derive` 行与 `#[reflect]` 行）。`kairos_ecs` 侧 `src/reflect.rs` 是 **1 字节空文件**，`kairos_reflect` 是**非默认 feature**（`kairos_ecs/Cargo.toml:11,13`），且 `kairos_ecs_macros` **不导出 Reflect derive**。→ 删除 reflect 的成本远低于实现它。
- **代价仍是"上百处机械改写 + 十来个真实重写"**：`use bevy…` 语句 146 条（含测试）／非测试文件 106 个；`App`/`Plugin`/`PluginGroup` 层 Kairos **完全没有**（全 workspace `grep 'pub struct App'`、`'pub trait Plugin'`、`'PluginGroup'` 命中数为 0，只有 4 处注释提到）；`Deref`/`DerefMut` derive（bevy_derive 提供）在 Avian 里 46 处带 `Deref` 的 derive 行，而 `kairos_ecs_macros` 的 17 个 `#[proc_macro_derive]` 里**没有** `Deref`/`DerefMut`/`From`。
- **许可证兼容**：Avian 是 `MIT OR Apache-2.0`（`crates/avian3d/Cargo.toml:9`；`LICENSE-MIT:1-3` 署名 `Copyright (c) 2022 Jondolf`、`LICENSE-APACHE:1-4`），Kairos workspace 也是 `MIT OR Apache-2.0`（`Cargo.toml:24`）→ **兼容**。双许可下只需择一遵守；选 MIT 需保留版权与许可声明，选 Apache-2.0 需保留 `NOTICE`/变更声明。注意 Kairos 仓库根**没有 LICENSE 文件**（`ls` 无 `LICENSE*`），这是需要补的家务活。

---

## 1. 测量口径与统计方法

本文所有计数都可复现，命令写在这里，避免"数字来源不明"：

| 指标 | 命令（在 Avian repo 根执行） | 口径说明 |
|---|---|---|
| 文件数 / 总行数 | `find src -name '*.rs' \| wc -l`；`find src -name '*.rs' -exec cat {} + \| wc -l` | 含测试文件 |
| 文档注释行 | `grep -rcE '^\s*(///\|//!\|#\[cfg_attr\(.*doc)' src --include='*.rs' \| awk -F: '{s+=$2} END{print s}'` | 只算整行以 `///` / `//!` 开头的行 |
| 空行 | `grep -rcE '^\s*$' src --include='*.rs' \| awk -F: '{s+=$2} END{print s}'` | |
| 出现某 token 的**非注释代码行** | `grep -rn '<TOKEN>' src --include='*.rs' \| grep -vE ': *//' \| wc -l` | 统计的是**行数**，不是 token 数；`grep -vE ': *//'` 剔除"行首内容为 `//`"的注释行（含 `///`），因此**行内注释与文档示例里的 `#[cfg_attr(doc = ...)]` 行会被算作代码**，是上界 |
| 某子系统的体量 | `find src/<DIR> -name '*.rs' -exec cat {} + \| wc -l` | 递归含子目录 |
| 某子系统的 bevy 密度 | `grep -rnE '\bbevy' src/<DIR> --include='*.rs' \| grep -vE ': *//' \| wc -l` | 同上口径 |
| `bevy::<mod>` 路径出现次数 | `grep -rEoh 'bevy::[a-z_0-9]+' src --include='*.rs' \| sort \| uniq -c \| sort -rn` | **含注释与文档**，且**无法穿透 `bevy::prelude::*` glob**，只反映"显式写出来的路径" |
| `use bevy…;` 语句数 | 用 `awk` 把跨行 `use` 折叠到 `;` 后统计（见下表 §2.2 脚注） | 含测试 |
| derive 计数 | `grep -rEoh '#\[derive\([^]]*\)\]' src --include='*.rs' \| sed 's/#\[derive(//; s/)\]//' \| tr ',' '\n' \| sed 's/^ *//; s/ *$//' \| sort \| uniq -c \| sort -rn` | 含测试与文档内示例 |

**「实体代码 ≈ 31 000 行」的推导**：52 307 − 13 856（文档行）− 5 133（空行）− 2 141（独立测试文件）= 31 177。这**没有**扣掉文件内 `#[cfg(test)] mod tests { … }` 块（`grep -rl '#\[cfg(test)\]' src` 命中 17 个文件），所以 **31 177 偏高**，标注「估算」。

---

## 2. Avian 的 Bevy 依赖全清单

### 2.1 先纠正前提：Avian 只用 umbrella `bevy` crate

`crates/avian3d/Cargo.toml:82-85`：

```toml
bevy = { version = "0.19.0", default-features = false, features = [
    "std",
    "bevy_log",
] }
```

`grep -rEoh 'bevy_[a-z_0-9]+' src --include='*.rs' | sort | uniq -c` 的全部结果（**含注释/文档**）只有这些：`bevy_scene` 22、`bevy_diagnostic` 19、`bevy_math` 16、`bevy_picking` 9、`bevy_heavy` 9、`bevy_rapier` 5（文档里对比 rapier 的叙述）、`bevy_transform_interpolation` 4、`bevy_transform` 2、`bevy_ahoy` 2 / `bevy_tnua` 1（文档里提外部 crate）。

**`bevy_ecs` 出现 0 次。**包路径一律写成 `bevy::ecs::…`。这是移植计划里第一个必须先纠正的认知：Avian 与 Bevy 的耦合面是 **umbrella crate 的 re-export 面**，`bevy_internal` 决定哪些子 crate 可见，`avian3d` 的 feature 通过 `bevy/bevy_mesh`、`bevy/bevy_gizmos`、`bevy/bevy_picking`、`bevy/bevy_world_serialization`、`bevy/bevy_ui`、`bevy/multi_threaded` 这种**转发 feature** 开关它们（`crates/avian3d/Cargo.toml:30,32,50,51,52,69`）。

解析后的版本（Avian `Cargo.lock`）：`bevy 0.19.0`（`:461-464`）、`bevy_ecs 0.19.0`（`:801-804`）、`bevy_app 0.19.0`（`:559-562`）、`bevy_math 0.19.0`（`:1237-1250`）、`bevy_transform 0.19.0`（`:1739-1742`）、`bevy_reflect 0.19.0`（`:1428-1431`）、`bevy_time 0.19.0`（`:1724-1727`）、`bevy_tasks 0.19.0`（`:1676-1679`）、`bevy_log 0.19.0`（`:1173-1176`）、`bevy_diagnostic 0.19.0`（`:783-786`）、`bevy_asset 0.19.0`（`:581-584`）、`bevy_scene 0.19.0`（`:1537-1540`）、`bevy_picking 0.19.0`（`:1351-1354`）。

### 2.2 `bevy` 子模块清单表（A = 核心仿真必需 / B = 可选或工具链）

计数口径：`bevy::<mod>` 的**字面出现次数（含注释与文档，不含 glob 展开）**，命令见 §1。**`bevy::prelude::*` 出现 210 次、63 个文件**，它把 ECS/math/transform/time/log 的类型一次性引进来，所以"prelude 之外的路径次数"只能反映**显式限定**的部分，真实依赖面必须结合 §3 的标识符计数看。

| bevy 子模块 | 出现次数 | 涉及文件数 | 代表引用（`file:line`） | 分类 | 备注 |
|---|---|---|---|---|---|
| `bevy::prelude` | 210 | 63 | `src/physics_transform/mod.rs:24`、`src/dynamics/rigid_body/world_query.rs:4` | **A**（通道） | glob，隐藏真实依赖；见 §3 |
| `bevy::ecs` | 31 | 13 | `src/schedule/mod.rs:17`、`src/dynamics/rigid_body/world_query.rs:5`、`src/data_structures/sparse_secondary_map.rs:18` | **A** | 非 prelude 的 ECS 深路径（`QueryData`、`SystemParam`、`ScheduleLabel`、`HookContext`、`EntityGeneration`…） |
| `bevy::diagnostic` | 11 | 4 | `src/diagnostics/ui.rs:11`、`src/diagnostics/path_macro.rs:33`、`src/spatial_query/diagnostics.rs:6` | **A（泄漏）/B** | 见 §2.4：`DiagnosticPath` 出现在 `PhysicsDiagnostics` trait 签名里，**不在 default feature 里也被引用** |
| `bevy::reflect` | 10 | 6 | `src/data_structures/bit_vec.rs:8,10`、`src/dynamics/solver/contact/tangent_part.rs:2,4`、`src/dynamics/solver/constraint_graph.rs:22` | **B** | 仅 derive/属性，无 bound；见 §3.4 |
| `bevy::transform` | 7 | 2 | `src/schedule/mod.rs:23`（`TransformSystems`）、`src/physics_transform/helper.rs:11-14`（`GlobalTransform`/`TransformHelper`） | **A** | ⚠ 硬阻塞之一 |
| `bevy::scene` | 6 | 5 | `src/dynamics/integrator/mod.rs:555`、`src/tests/mod.rs:28,198`、`src/tests/determinism_2d.rs:44` | **B**（且**只在测试里**） | 全是 `ScenePlugin` 加进测试 App；生产代码 0 处 |
| `bevy::mesh` | 4 | 3 | `src/collision/collider/parry/mod.rs:16`、`src/collision/collider/trimesh_builder.rs:342-343`、`src/tests/mod.rs:26,200` | **B**（`collider-from-mesh`，但是 **default-on**） | 见 §2.4 |
| `bevy::world_serialization` | 3 | 2 | `src/collision/collider/backend.rs:15`、`src/collision/collider/constructor.rs:548,762` | **B**（`bevy_scene`，default-on） | `WorldAssetRoot`/`WorldInstance as SceneInstance`/`WorldInstanceSpawner`/`WorldInstanceReady` |
| `bevy::platform` | 3 | 3 | `src/data_structures/sparse_secondary_map.rs:13`（`RandomState`）、`src/utils.rs:3`（`Instant`）、`src/dynamics/solver/solver_body/plugin.rs:274` | **A** | 注意 `src/data_structures/sparse_secondary_map.rs` 是核心容器 |
| `bevy::picking` | 3 | 2 | `src/picking/mod.rs:21-24`、`src/lib.rs:72,620` | **B**（`bevy_picking`，default-on） | `PickingSystems`、`backend::{HitData, PointerHits, ray::RayMap}` |
| `bevy::asset` | 3 | 2 | `src/tests/mod.rs:24,196`（`AssetPlugin`）、`src/collision/collider/trimesh_builder.rs:342`（`RenderAssetUsages`） | **B** | 生产代码只在 mesh→trimesh 路径 |
| `bevy::tasks` | 2 | 1 | `src/collider_tree/optimization.rs:11`（`AsyncComputeTaskPool, Task, block_on`） | **A**（优化路径） | 另有 `src/utils.rs:65,74` 与 `src/collision/broad_phase/bvh_broad_phase.rs:16,72` 走 `ComputeTaskPool`/`ParallelSlice` — 但**这两处不是 `bevy::tasks::` 前缀写法**，所以本表只统计到 2 次 |
| `bevy::math` | 2 | 1 | `src/schedule/time.rs:87,104` | **B**（文档示例） | 真实数学面走 `bevy_math` 直接依赖，见 §4 |
| `bevy::color` | 1 | 1 | `src/debug_render/configuration.rs:2`（`palettes::css::*`） | **B** | 仅 debug 渲染 |
| `bevy::gltf` | 1 | 1 | `src/collision/collider/constructor.rs:753`（`GltfMeshName`） | **B**（测试） | |
| `bevy::app` | 0（字面） | — | `src/lib.rs:581` 是 `app::PluginGroupBuilder` | **A** | 因为写的是 `app::PluginGroupBuilder` 而不是 `bevy::app::…`，字面统计漏掉；**实际存在** |
| `bevy::log` | 0（字面） | — | `src/collision/collider/parry/mod.rs:17`：`use bevy::{log, prelude::*};` | **A** | `bevy_log` 是 `bevy` 依赖里唯一显式打开的 feature（`crates/avian3d/Cargo.toml:84`） |
| `bevy::camera` | 0 | — | `src/collider_tree/update.rs` 附近的条件导入里出现 `camera::visibility::VisibilitySystems` | **B** | 见 §8 |
| `bevy::ui` | 0（字面） | — | `diagnostic_ui` feature 打开 `bevy/bevy_ui`（`crates/avian3d/Cargo.toml:69`），`src/diagnostics/ui.rs` 是使用者 | **B** | 非默认 feature |
| `bevy::winit` | 0（字面） | — | 仅测试 App（`use bevy::{diagnostic::DiagnosticsPlugin, winit::WinitPlugin};`） | **B** | |
| `bevy::time` | 0（字面） | — | `src/schedule/time.rs` 的 `Time`/`TimeUpdateStrategy` 走 prelude 与 `time::TimeUpdateStrategy` | **A** | 见 §3.2 / §7 |

**Class A（核心仿真必需）汇总**：`bevy::ecs`（全部深路径）、`bevy::transform`（`TransformSystems`、`GlobalTransform`、`TransformHelper`、`ComputeGlobalTransformError`）、`bevy::time`（`Time` 族、`TimeUpdateStrategy`）、`bevy::tasks`（`ComputeTaskPool`/`ParallelSlice`/`AsyncComputeTaskPool`）、`bevy::platform`（`Instant`、`RandomState`）、`bevy::log`（宏）、`bevy::app`（`App`/`Plugin`/`PluginGroup`/`PluginGroupBuilder`），以及**泄漏进 trait 签名的 `bevy::diagnostic::DiagnosticPath`**。

**Class B（工具/可选）汇总**：`bevy::reflect`（可删）、`bevy::scene` + `bevy::world_serialization`（仅测试与场景构造器）、`bevy::mesh` + `bevy::asset` + `bevy::gltf`（mesh→collider 与测试）、`bevy::picking`、`bevy::color` + `bevy::gizmos` + `bevy::render`（debug 渲染）、`bevy::ui`（诊断 UI）。

### 2.3 非 Bevy 依赖清单与版本

来源：`crates/avian3d/Cargo.toml:81-102`（声明）+ Avian `Cargo.lock`（解析结果）。

| crate | 声明（`crates/avian3d/Cargo.toml:LINE`） | 锁定版本（Avian `Cargo.lock:LINE`） | 分类 | 与 Kairos 的关系 |
|---|---|---|---|---|
| `avian_derive` | `:81` `path = "../avian_derive", version = "0.2"` | `0.2.2`（`:445-447`） | **A**（`PhysicsLayer` derive） | 纯 proc-macro，无 ECS 依赖，可直接 vendor；见 §6.1 注 |
| `bevy_math` | `:86` `0.19.0`, features `["approx"]` | `0.19.0`（`:1237-1250`） | **A（数学）** | ⚠ 依赖 `glam 0.32.1`（`:1246`）；另依赖 `bevy_reflect`（`:1244`）。**不含 `bevy_ecs`** |
| `glam_matrix_extras` | `:87` `0.3`, features `["bevy_reflect"]` | `0.3.0`（`:3094-3105`） | **A（数学）** | ⚠⚠ `glam = "0.32"` + `bevy_reflect = "0.19"`（docs.rs）。**必须 fork** |
| `bevy_heavy` | `:88` `0.5` | `0.5.0`（`:1011-1019`） | **A（质量属性）** | 依赖 `approx, bevy_math, bevy_reflect, glam_matrix_extras, serde` → **纯数学，无 ECS** |
| `bevy_transform_interpolation` | `:89` `0.5` | `0.5.0`（`:1756-1761`） | **A（插值）但可延后** | 依赖 `bevy, serde` → **ECS 强耦合**，必须重写 |
| `obvhs` | `:94` `0.3` | `0.3.1`（`:4418-4429`） | **A（BVH 宽相 + collider tree）** | `glam 0.31.1`（`:4424`）；**升到 0.3.3 即支持 glam 0.33** |
| `parry3d` / `parry3d-f64` | `:92` / `:93` `0.27`，optional | `0.27.0`（`:4629-4658`）/ `0.27.0`（`:4661-…`） | **A（碰撞检测）** | Kairos 用 `parry3d 0.28.0`（Kairos `Cargo.lock:3634-3636`）→ **版本错位**，见 §4.5 |
| `approx` | `:91` `0.5` | `0.5.x` | A（`AbsDiffEq` 等） | 无耦合 |
| `libm` | `:90` `0.2`, optional | — | A 仅 `enhanced-determinism` | 无耦合 |
| `smallvec` | `:98` `1.15` | — | A | kairos_ecs 已依赖 |
| `itertools` | `:99` `0.15` | — | A（`Either` 等） | 无耦合 |
| `bitflags` | `:100` `2.5.0` | — | A（`CollisionLayers`） | kairos_ecs 已依赖 `2.13.1` |
| `derive_more` | `:96` `2` | — | A（`From` 等 derive，`src/dynamics/rigid_body/mass_properties/components/mod.rs:12` 用 `derive_more::From`） | kairos_ecs 已依赖 `2.1.1` |
| `thiserror` | `:97` `2` | — | A（`src/physics_transform/helper.rs:15`） | 无耦合 |
| `thread_local` | `:101` `1.1` | — | A（`src/collider_tree/update.rs:27`、`src/collision/narrow_phase/system_param.rs:26`） | kairos_ecs 已依赖 |
| `disqualified` | `:102` `1.0` | — | B（只有 1 处：`src/dynamics/solver/joint_graph/plugin.rs:212` 用 `ShortName`） | kairos_ecs 已依赖 |
| `serde` | `:95` `1`, optional | — | B（`serialize` feature） | 无耦合 |

### 2.4 feature gate 与"默认 feature 的真实代价"

`#[cfg(feature = "…")]` 出现次数（`grep -rEoh '#\[cfg\(feature = "[a-z0-9_-]+"\)\]' src | sort | uniq -c`）：

| feature | 次数 | feature | 次数 |
|---|---|---|---|
| `3d` | 443 | `f64` | 8 |
| `2d` | 361 | `debug-plugin` | 7 |
| `collider-from-mesh` | 37 | `serialize` | 6 |
| `f32` | 25 | `xpbd_joints` | 3 |
| `bevy_scene` | 16 | `diagnostic_ui` | 3 |
| `bevy_diagnostic` | 14 | `bevy_picking` | 3 |
| `validate` | 11 | `enhanced-determinism` | 2 |
| `parallel` | 11 | | |
| `default-collider` | 10 | | |

**`avian3d` 的默认 feature 集合**（`crates/avian3d/Cargo.toml:15-25`）：

```
3d, f32, parry-f32, debug-plugin, xpbd_joints, parallel,
collider-from-mesh, bevy_scene, bevy_picking
```

**关键结论**：默认配置就把 **`bevy_gizmos` + `bevy_render`（debug-plugin，`:30`）、`bevy_mesh` + `bevy_mikktspace`（collider-from-mesh，`:50`）、`bevy_world_serialization`（bevy_scene，`:51`）、`bevy_picking`（`:52`）** 全部拉进来。对 Kairos 而言这四类**全部不存在对应物**（Kairos 的渲染栈在 `kairos_graphics`，且没有 bevy 的 gizmo/asset/mesh/picking 抽象）。

**但 `default-features = false` 可以干净地砍掉它们**，代价也明确：
- 关掉 `debug-plugin` → `src/debug_render/`（3 文件 1 621 行）不编译；
- 关掉 `collider-from-mesh` → `src/collision/collider/{trimesh_builder.rs, cache.rs, constructor.rs}` 里的 mesh 路径不编译；
- 关掉 `bevy_picking` → `src/picking/`（1 文件 260 行）不编译；
- 关掉 `bevy_scene` → `src/collision/collider/backend.rs` 的场景等待逻辑不编译。

**唯一不能靠 feature 关掉的 Class B 泄漏是 `bevy::diagnostic::DiagnosticPath`**：`src/diagnostics/mod.rs:137-145` 的 `PhysicsDiagnostics` trait 签名直接写 `Vec<(&'static DiagnosticPath, Duration)>`（`DiagnosticPath` 来自 `bevy::diagnostic`，见模块头 `use bevy::{diagnostic::DiagnosticPath, prelude::{ReflectResource, Resource}, reflect::Reflect}`），而 `impl_diagnostic_paths!` 宏（`src/diagnostics/path_macro.rs:33`）把常量定义成 `&'static bevy::diagnostic::DiagnosticPath`。核心侧的 7 个 diagnostics 资源（`SolverDiagnostics`、`CollisionDiagnostics`、`ColliderTreeDiagnostics`、`SpatialQueryDiagnostics`、`PhysicsEntityDiagnostics`、`PhysicsTotalDiagnostics`、`PhysicsPickingDiagnostics`）都带这些常量（7 次 `impl_diagnostic_paths!` 调用，共 28 个路径常量）。**替换成本极低**（`&'static str` 即可），但它是"核心路径上必须动的一处 Bevy 类型"。

---

## 3. `bevy_ecs` API 使用清单（含 Kairos 侧现状）

标识符计数口径见 §1（**非注释代码行数**）。**重要**：这些数字是**行数**，会把 `use`、类型声明、函数签名分别计一次，因此用于比较"量级"而非"调用点精确数"。**"核心 / 仅工具" 一列是本节的结论**。

### 3.1 应用层：`App` / `Plugin` / `PluginGroup`

| API | Avian 用量（非注释行） | 代表引用 | 核心还是工具 | Kairos 现状 |
|---|---|---|---|---|
| `App` | 76 | `src/dynamics/integrator/mod.rs:44`、`src/physics_transform/mod.rs:70` | **核心**（每个 plugin 的 `build(&self, app: &mut App)`） | ❌ **不存在**（全 workspace 0 命中） |
| `Plugin` trait | 35 | `src/dynamics/integrator/mod.rs:43`、`src/lib.rs:` `impl Plugin` 共 **28 处** | **核心** | ❌ 不存在 |
| `PluginGroup` / `PluginGroupBuilder` | 7 | `src/lib.rs:757-786`（`impl PluginGroup for PhysicsPlugins`）、`src/lib.rs:830-838` | **核心**（`PhysicsPlugins` 是唯一装配入口） | ❌ 不存在 |
| `app.*` 调用点 | `add_systems` 59、`world_mut` 137、`world` 145、`update` 67、`add_observer` 52、`insert_resource` 19、`configure_sets` 18、`finish` 28、`add_plugins` 21、`edit_schedule` 2、`register_diagnostic` 1、`init_schedule` 1（命令：`grep -rnE 'app\.[a-z_]+\(' src \| grep -vE ': *//' \| grep -oE 'app\.[a-z_]+\(' \| sort \| uniq -c`） | `src/lib.rs:759-786` | **核心** | ⚠ `World`/`Schedules` 有等价物（见 §6.1），但**没有 `app` 这一层** |
| `app.register_required_components` / `try_register_required_components` | `src/dynamics/integrator/mod.rs:47`、`src/interpolation.rs:274-284`（`try_` 版本） | — | **核心** | ✅ `World::register_required_components`（`kairos_ecs/src/world.rs:493`）、`try_register_required_components`（`:599`）都在；**缺的是 `App` 上的转发方法** |

`PhysicsPlugins::build`（`src/lib.rs:757-786`）一次性 `.add(...)` 了 17 个 plugin / plugin group：`PhysicsSchedulePlugin`、`MassPropertyPlugin`、`ForcePlugin`、`ColliderHierarchyPlugin`、`ColliderTransformPlugin`、`ColliderCachePlugin`、`ColliderBackendPlugin<Collider>`、`ColliderTreePlugin<Collider>`、`NarrowPhasePlugin<Collider>`、`SolverPlugins`、`BroadPhaseCorePlugin`、`BvhBroadPhasePlugin<()>`、`JointPlugin`、`SpatialQueryPlugin`、`PhysicsTransformPlugin`、`PhysicsInterpolationPlugin`。

### 3.2 调度层

| API | Avian 用量 | 代表引用 | 核心还是工具 | Kairos 现状 |
|---|---|---|---|---|
| `Schedule` / `Schedules` | — | `src/schedule/mod.rs:96-118`（`app.edit_schedule(PhysicsSchedule, …)`） | **核心** | ✅ `Schedules`（prelude `kairos_ecs/src/lib.rs:68`）、`Schedule::new/run/set_executor/set_build_settings`（`kairos_ecs/src/schedule/schedule.rs:447,603,587,565`）、`Schedules::add_systems`（`:261`、`:473`）、`configure_sets`（`:288`、`:539`） |
| `ScheduleLabel` trait + derive | 33 行 / **3 个 derive 点** | `src/schedule/mod.rs:140-141`（`PhysicsSchedule`）、`src/dynamics/solver/schedule.rs:76-77`（`SubstepSchedule`）、`src/tests/mod.rs:187`（测试） | **核心** | ✅ 同名 trait + `#[derive(ScheduleLabel)]`（`kairos_ecs/macros/src/lib.rs:502`）；`Interned<T>`（`kairos_ecs/src/intern.rs:49`）、`InternedScheduleLabel` 别名 |
| `Interned<dyn ScheduleLabel>` | `src/schedule/mod.rs:34`、`src/dynamics/integrator/mod.rs:24`、`src/lib.rs:581` | — | **核心** | ✅ `Interned` 存在；Kairos 侧惯用别名 `InternedScheduleLabel`（`kairos_engine/src/kairos_editor/schedule.rs:113,190`） |
| `.intern()` | `src/schedule/mod.rs:41`、`src/dynamics/integrator/mod.rs:31` | — | **核心** | ✅ `ScheduleLabel::intern()` |
| `IntoScheduleConfigs` / `in_set` / `.chain()` / `.before()` / `.after()` / `.ambiguous_with_all()` | `IntoScheduleConfigs` 2、`Or` 55、`configure_sets` 18 | `src/schedule/mod.rs:71-84`、`src/dynamics/integrator/mod.rs:52-76`、`src/diagnostics/mod.rs:205`（`ambiguous_with_all`） | **核心** | ✅ 均在 prelude（`kairos_ecs/src/lib.rs:67` 导出 `IntoScheduleConfigs, IntoSystemSet, SystemSet, SystemCondition, common_conditions::*`） |
| `run_if` | **0**（`grep -rn 'run_if('` 只命中文档示例） | — | — | ✅ `SystemCondition`/`common_conditions::*` 存在 |
| `SystemSet` derive | 17 个 enum/struct | `src/schedule/mod.rs:162`（`PhysicsSystems`）、`:192`（`PhysicsStepSystems`）、`src/dynamics/solver/schedule.rs:93,134` | **核心** | ✅ `#[derive(SystemSet)]`（`kairos_ecs/macros/src/lib.rs:516`） |
| `SingleThreadedExecutor` | `src/schedule/mod.rs:98`、`src/dynamics/solver/schedule.rs:55` | — | **核心**（物理调度强制单线程 executor） | ✅ `kairos_ecs/src/schedule/executor/single_threaded.rs:22` |
| `ScheduleBuildSettings` / `LogLevel` | `src/schedule/mod.rs:19,100-103`、`src/dynamics/solver/schedule.rs:10,56-59` | — | **核心**（歧义检测设为 `Error`） | ✅ `Schedule::set_build_settings`（`kairos_ecs/src/schedule/schedule.rs:565`）、`Schedules::configure_schedules`（`:222`）、`ScheduleBuildSettings`/`LogLevel` |
| `World::try_schedule_scope` + `Schedule::run(world)` | `src/schedule/mod.rs:236`、`src/dynamics/solver/schedule.rs:202` | — | **核心**（物理步 / 子步循环） | ✅ `kairos_ecs/src/world.rs:3856`；`kairos_engine` 的固定步驱动同样用它（`kairos_engine/src/kairos_editor/schedule.rs:242`） |
| `World::run_system_once` | 1 处（`src/dynamics/solver/islands/mod.rs:1393`，在**测试**里） | — | 仅测试 | ✅ `RunSystemOnce` 在 kairos_ecs 中出现 10 次 |
| `check_change_ticks` / `CheckChangeTicks` | **0** | — | — | ✅ `kairos_ecs/src/change_detection/tick.rs:111`（Avian 不用） |

**调度层小结**：Avian 的调度词汇与 `kairos_ecs` 的调度词汇**逐项同名同位**。`App` 上的 `add_systems`/`configure_sets`/`edit_schedule`/`init_resource`/`insert_resource` 在 Kairos 侧对应到 `Schedules::add_systems`（`kairos_ecs/src/schedule/schedule.rs:261,473`）、`Schedules::configure_sets`（`:288,539`）、`Schedules::get_mut`/`entry`（`:171,176`）、`World::init_resource`/`insert_resource`（`kairos_ecs/src/world.rs:2048,2061`）。**形状差异只有一处**：Kairos 没有 `App`，`kairos_physics::install(world: &mut World, fixed_update_stage: impl ScheduleLabel)`（`kairos_physics/src/lib.rs:317`）就是 Kairos 的"plugin"写法。

### 3.3 查询与系统参数层

| API | Avian 用量 | 代表引用 | 核心还是工具 | Kairos 现状 |
|---|---|---|---|---|
| `Query` | 244 | `src/dynamics/integrator/mod.rs:341`、`src/schedule/mod.rs:293-295` | **核心** | ✅ `Query<'w,'s,D,F>`（prelude `kairos_ecs/src/lib.rs:71-73`） |
| `QueryData` derive | **9 个结构体** | `src/dynamics/rigid_body/world_query.rs:10`（`RigidBodyQuery`）、`src/dynamics/integrator/mod.rs:330`（`VelocityIntegrationQuery`）、`src/dynamics/ccd/mod.rs:501`、`src/dynamics/solver/plugin.rs:356`、`src/dynamics/rigid_body/forces/query_data.rs:105`、`src/interpolation.rs:310,340`、`src/collision/narrow_phase/system_param.rs:28,45` | **核心** | ✅ trait（`kairos_ecs/src/query/fetch.rs:379`）+ `#[derive(QueryData)]`（`kairos_ecs/macros/src/lib.rs:488`）+ `#[query_data(mutable)]` 属性（`kairos_ecs/macros/src/query_data.rs:26,135`） |
| `#[derive(SystemParam)]` | **8 个**（非测试） | `src/spatial_query/system_param.rs:59`（`SpatialQuery`）、`src/dynamics/rigid_body/mass_properties/system_param.rs:13`（`MassPropertyHelper`）、`src/physics_transform/helper.rs:30`、`src/character_controller/move_and_slide.rs:66`、`src/collision/narrow_phase/system_param.rs:70`、`src/collision/narrow_phase/mod.rs:301`、`src/collision/contact_types/system_param.rs:52` | **核心** | ✅ trait（`kairos_ecs/src/system/system_param.rs:248`）+ derive（`kairos_ecs/macros/src/lib.rs:240`） |
| `SystemParamItem` / `StaticSystemParam` | 4 / 3 | `src/collider_tree/update.rs:21,87,687`（`StaticSystemParam<C::Context>`） | **核心** | ✅ 同名，`kairos_ecs/src/system/system_param.rs` 里 `StaticSystemParam` 有文档与实现 |
| `ReadOnlySystemParam` | `src/collision/hooks.rs:6,147`、`src/collision/collider/mod.rs:8` | — | **核心**（`CollisionHooks: ReadOnlySystemParam + Send + Sync`） | ✅ 存在（kairos_ecs 中出现 37 次） |
| `SystemState` | 9 | `src/dynamics/solver/islands/sleeping.rs:54-57`（`SystemState::new(app.world_mut())`） | **核心**（缓存系统状态） | ✅ 存在（34 次） |
| `ParamSet` | `src/dynamics/integrator/mod.rs:468`、`src/dynamics/solver/islands/sleeping.rs:567` | — | **核心** | ✅ prelude 导出（`kairos_ecs/src/lib.rs:72`） |
| `Local<T>` | 100 | `src/dynamics/solver/islands/sleeping.rs:247-248`、`src/schedule/mod.rs:235` | **核心** | ✅ prelude |
| `Res` / `ResMut` | 99 / 115 | `src/dynamics/integrator/mod.rs:344-345`、`src/schedule/mod.rs:236` | **核心** | ✅ prelude |
| `Single` / `Populated` | 1 / 0 | `src/diagnostics/ui.rs:493`（唯一一处，且是 UI 工具） | **仅工具** | ✅ `Single`（`kairos_ecs/src/system/query.rs:2876`）、`Populated`（`:2918`）—— **Avian 核心不用** |
| lifetimeless `Read` / `Write` / `SQuery` / `SRes` / `SResMut` | 2 / 2 | `src/physics_transform/helper.rs:6`（`lifetimeless::Write`）、`src/dynamics/rigid_body/forces/query_data.rs:2`、`src/collision/collider/backend.rs` | **核心** | ✅ **全部存在**：`Read`（`kairos_ecs/src/system/system_param.rs:2175`）、`Write`（`:2177`）、`SQuery`（`:2173`）、`SRes`（`:2179`）、`SResMut`（`:2181`）、`SCommands`（`:2183`） |
| `QueryFilter`：`With`/`Without`/`Or`/`Has`/`Changed`/`Ref` | 91 / 87 / 55 / 32 / 38 / 27 | `src/dynamics/integrator/mod.rs:342`（`Without<CustomVelocityIntegration>`）、`src/dynamics/rigid_body/world_query.rs:7`（`Has`, `Ref`） | **核心** | ✅ 全在 prelude（`kairos_ecs/src/lib.rs:63`）；`#[derive(QueryFilter)]` 亦在（`kairos_ecs/macros/src/lib.rs:494`） |
| `Ref<C>` / `Mut<C>` | 27 / 7 | `src/dynamics/rigid_body/world_query.rs:6`（`Ref<'static, RigidBody>`） | **核心** | ✅ |
| `Commands` / `EntityCommands` | 45 / 4 | `src/collision/collider/backend.rs:192`、`src/collision/hooks.rs:164` | **核心** | ✅ `Commands::spawn/entity/get_entity/queue`（`kairos_ecs/src/system/commands.rs:424,465,516,667`）、`insert/remove/try_remove/retain`（`:1455,1746,1847,2070`） |
| `Commands::queue` | `src/dynamics/solver/joint_graph/plugin.rs:158,188,358` | — | **核心** | ✅ `:667` |
| `ParallelCommands` | 4 | `src/collision/narrow_phase/mod.rs:281`、`src/collision/broad_phase/bvh_broad_phase.rs:55` | **核心**（宽相/窄相并行） | ✅ prelude `#[doc(hidden)]` 导出（`kairos_ecs/src/lib.rs:86`） |
| `par_iter_mut()` / `par_iter()` | **10 处** | `src/dynamics/integrator/mod.rs:278,322,356,512`、`src/dynamics/solver/solver_body/plugin.rs:196,276,299`、`src/collider_tree/update.rs:713,890`、`src/collision/collider/collider_transform/plugin.rs:123` | **核心** | ✅ `Query::par_iter`（`kairos_ecs/src/system/query.rs:1294`）、`par_iter_mut`（`:1329`）；query 状态版在 `kairos_ecs/src/query/state.rs:1447,1499` |
| `bevy::tasks`（`ComputeTaskPool` / `ParallelSlice` / `AsyncComputeTaskPool`） | `src/utils.rs:65,74`、`src/collision/broad_phase/bvh_broad_phase.rs:16,72`、`src/collider_tree/optimization.rs:11,177,266,289,298` | — | **核心**（BVH 构建并行） | ✅ `kairos_tasks` 是 `bevy_tasks` fork：`ComputeTaskPool`、`ParallelSlice`、`ParallelSliceMut`、`AsyncComputeTaskPool`、`TaskPool`（`kairos_tasks/src/lib.rs:9-16`） |

### 3.4 组件模型层

| API | Avian 用量 | 代表引用 | 核心还是工具 | Kairos 现状 |
|---|---|---|---|---|
| `#[derive(Component)]` | 126 处（95 个类型） | `src/physics_transform/transform.rs`（`Position`）、`src/dynamics/rigid_body/mod.rs:408` | **核心** | ✅ `kairos_ecs/macros/src/lib.rs:830`（`attributes(component, require, relationship, relationship_target, entities)`） |
| `#[require(...)]` 声明式必需组件 | 见 `require` 关键词 59 行 | `kairos_ecs/macros/src/lib.rs:756` 的文档示例；Avian 侧多为运行时 `app.register_required_components` | **核心** | ✅ derive 支持 `require`；运行时 API 在 `kairos_ecs/src/world.rs:493`、`:544`、`:599` |
| `#[derive(Resource)]` | 43 处（30 个类型） | `src/schedule/mod.rs:200`（`LastPhysicsTick`）、`src/dynamics/integrator/mod.rs:152`（`Gravity`） | **核心** | ✅ `kairos_ecs/macros/src/lib.rs:591` |
| `ComponentId` | 3 | `src/dynamics/solver/joint_graph/plugin.rs:21`、`src/lib.rs` | **核心** | ✅（kairos_ecs 中 611 次） |
| `DeferredWorld` + `HookContext`（component hooks） | 29 / `src/collision/collider/backend.rs:11`、`src/dynamics/solver/islands/mod.rs:49`、`src/dynamics/rigid_body/mass_properties/components/mod.rs:5-6` | — | **核心**（collider/body 的初始化钩子） | ✅ `DeferredWorld`（prelude 之外，`kairos_ecs/src/world/deferred_world.rs`）、`HookContext`（`kairos_ecs/src/lifecycle.rs`）；`kairos_physics` 已在用（`kairos_physics/src/lib.rs:30,180`） |
| `on_add` / `on_remove` / `on_discard` 钩子 | `src/collision/hooks.rs` 附近；`src/collision/collider/collider_hierarchy/mod.rs:70-172` | — | **核心** | ✅ `register_component_hooks`（`kairos_ecs/src/world.rs:424`）；`kairos_physics` 已注册 `on_discard`（`kairos_physics/src/lib.rs:322-327`） |
| `Relationship` / `RelationshipTarget` / `RelationshipHookMode` / `RelationshipSourceCollection` | `Relationship` 3、`RelationshipTarget` 6 | `src/collision/collider/collider_hierarchy/mod.rs:70`（`type RelationshipTarget = RigidBodyColliders`）、`:165,172`；`src/collision/collider/collider_hierarchy/mod.rs:46,206`（文档链接） | **核心**（`ColliderOf`/`RigidBodyColliders` 父子关系） | ✅ `RelationshipTarget` 在 prelude（`kairos_ecs/src/lib.rs:64`）、`Relationship` trait 在 `kairos_ecs/src/relationship.rs`（81 处）；`ChildOf`/`Children` 在 prelude `:56` |
| `ChildOf` | 54 | `src/diagnostics/ui.rs:12,168` | **仅工具**（UI 层级） | ✅ `:56` |
| `entity_disabling::Disabled` | 5 | `src/dynamics/solver/islands/mod.rs:49`、`src/dynamics/solver/joint_graph/plugin.rs:21` | **核心**（查询里排除禁用实体） | ✅ `kairos_ecs/src/entity_disabling.rs:133` |
| `EntityMapper` / `MapEntities` / `ReflectMapEntities` | 8 / 8 / 5 | `src/spatial_query/ray_caster.rs:4,382-407`、`src/dynamics/joints/distance.rs:8` | **核心**（joint/ray 里存 `Entity`，需要重映射） | ✅ `#[proc_macro_derive(MapEntities, attributes(entities))]`（`kairos_ecs/macros/src/lib.rs:214`）、`EntityMapper` 在 prelude `:53`。⚠ **`ReflectMapEntities` 不存在**（属 reflect 层） |
| `Deref` / `DerefMut` derive | 46 处带 `Deref` 的 derive 行 | `src/dynamics/solver/plugin.rs:199,353`、`src/dynamics/rigid_body/mod.rs:408,436`、`src/dynamics/ccd/mod.rs:305`、`src/spatial_query/ray_caster.rs:338` | **核心**（`Mass(pub f32)`、`LinearVelocity(pub Vector)` 等 newtype 直接当数值用） | ⚠ **`kairos_ecs_macros` 没有 `Deref`/`DerefMut`/`From` derive**（17 个 `#[proc_macro_derive]` 全清单：`Bundle, MapEntities, SystemParam, QueryData, QueryFilter, ScheduleLabel, SystemSet, Event, EntityEvent, Message, Resource, SettingsGroup, Component, FromWorld, FromTemplate, VariantDefaults`）。**但 `kairos_ecs` 已依赖 `derive_more` 且开了 `deref`/`deref_mut` feature**（`kairos_ecs/Cargo.toml:34-40`） → 用 `derive_more::{Deref, DerefMut, From}` 即可，但 46 处 import 路径要改 |
| `Bundle` derive | 1 | `src/dynamics/rigid_body/mass_properties/components/mod.rs:1036`（`MassPropertiesBundle`） | **核心** | ✅ `kairos_ecs/macros/src/lib.rs:58` |
| `Reflect` derive / `#[reflect(...)]` | 171 / 161 | `src/dynamics/integrator/mod.rs:152,178,191`、`src/dynamics/joints/mod.rs:523`、`src/dynamics/solver/contact/mod.rs:17` | **B（可删）** | ❌ **无 Reflect derive**；`kairos_ecs/src/reflect.rs` 是 **1 字节空文件**；`kairos_reflect` 非默认 feature（`kairos_ecs/Cargo.toml:11,13`）。**可删的理由见下** |
| `register_type` | **3（非注释）** | `src/schedule/mod.rs:61`、`src/dynamics/solver/schedule.rs:20`、`src/ancestor_marker.rs:23` | **B** | ❌ 无 `AppTypeRegistry` 可用（被 feature 关掉） |
| `ReflectResource` | 5 | `src/diagnostics/mod.rs:8`、`src/spatial_query/diagnostics.rs:6` | **B** | ❌ 同上 |

**`Reflect` 为什么可以整层删掉（证据）**：
1. 全库 `grep -rn 'Reflect' src | grep -vE ': *//'` 的 201 行里，**没有任何一行出现在泛型 bound 位置** —— 抽出的行全是 `#[derive(...)]`、`#[reflect(...)]`、以及 `use bevy::reflect::…`（例如 `src/data_structures/bit_vec.rs:8,10`、`src/dynamics/solver/softness_parameters/mod.rs:9,11`）。也就是说，**没有任何核心算法要求 `T: Reflect`**。
2. 3 处 `register_type` 都是可选注册（`Time<Physics>`、`Time<Substeps>`、`AncestorMarker<C>`），删掉不影响仿真。
3. Reflect 只在两条外围路径上真正"必需"：`bevy_scene` 序列化（`src/collision/collider/backend.rs:15`）与 `ReflectMapEntities`（joints 的 `Entity` 字段重映射，`src/dynamics/joints/{distance,fixed,revolute,prismatic,spherical}.rs:8`）。Kairos 前者本来就没有（无 scene 系统），后者可以改手写 `MapEntities`。

### 3.5 事件 / 观察者 / Message

| API | Avian 用量 | 代表引用 | 核心还是工具 | Kairos 现状 |
|---|---|---|---|---|
| `Observer` / `add_observer` | 52 个 `add_observer` | `src/dynamics/rigid_body/mass_properties/mod.rs:289-296`、`src/dynamics/solver/joint_graph/plugin.rs:78-87`、`src/collider_tree/update.rs:404`（`add_to_tree_on<E: EntityEvent, …>`） | **核心**（质量属性重算、joint graph 增删、collider tree 增删） | ✅ `Observer` 在 prelude（`kairos_ecs/src/lib.rs:62`）；`World::add_observer`（`kairos_ecs/src/observer.rs:67`）、`Commands::add_observer`（`kairos_ecs/src/system/commands.rs:1219`）；Avian 用 `app.add_observer` |
| `On<E>` / `On<Add, C>` / `On<Insert, C>` | `On` 44、`Trigger` 1 | `src/dynamics/rigid_body/mass_properties/mod.rs:290,295`、`src/dynamics/solver/joint_graph/plugin.rs:78-87` | **核心** | ✅ `On` 在 prelude `:62`；`Add`/`Insert`/`Remove`/`Despawn`/`Discard` 在 `:57` |
| `EntityEvent` derive | 4 | `src/collision/collision_events.rs:169,266`、`src/dynamics/solver/joint_graph/plugin.rs:121` | **核心**（碰撞事件同时是 EntityEvent 和 Message） | ✅ `kairos_ecs/macros/src/lib.rs:560`（`attributes(entity_event, event_target)`） |
| `Message` derive / `MessageReader` / `MessageWriter` / `Messages` | `Message` 2、`MessageWriter` 12、`MessageReader` 3 | `src/collision/collision_events.rs:169,266`（同时 `#[derive(EntityEvent, Message, …)]`）、`src/collision/narrow_phase/mod.rs:304-305`、`src/collision/narrow_phase/system_param.rs:119-120` | **核心**（`CollisionStart` / `CollisionEnd`） | ✅ `Message`/`MessageReader`/`MessageWriter`/`Messages`/`MessageMutator` 在 prelude（`kairos_ecs/src/lib.rs:58-60`）；derive 在 `kairos_ecs/macros/src/lib.rs:566` |
| `Event` / `EventReader` / `EventWriter` | **0（非注释）** | 只有文档链接 `src/collision/mod.rs:52` | — | ✅ 有 `Event`（prelude `:55`），但 Avian 不用旧式 `EventReader`/`EventWriter` —— **这消除了一个移植风险点**：Kairos 已实现 0.19 的 `Message` 拆分 |

### 3.6 变更检测

| API | Avian 用量 | 代表引用 | 核心还是工具 | Kairos 现状 |
|---|---|---|---|---|
| `Ref::is_changed` / `last_changed` / `Tick::is_newer_than` | `src/schedule/mod.rs:225-232`（`is_changed_after_tick`）、`src/collider_tree/update.rs:893-895` | — | **核心**（只在"上一个物理 tick 之后是否变过"这个判定里用） | ✅ `Ref`/`DetectChanges` 在 prelude `:50`；`Tick`/`is_newer_than`/`last_changed` 在 kairos_ecs（469 / 21 / 15 次） |
| `SystemChangeTick` | 9 | `src/schedule/mod.rs:20,248`、`src/dynamics/solver/islands/sleeping.rs:596`、`src/collider_tree/update.rs:21` | **核心**（`LastPhysicsTick` 的写入） | ✅ `kairos_ecs/src/system/system_param.rs:1638` |
| `changed_by()`（`bevy/track_location`） | 1（`src/schedule/mod.rs:303`，在 `#[cfg(debug_assertions)]` 的 NaN 检查系统里） | — | 仅调试 | ✅ `kairos_ecs` 有 `track_location` feature（`kairos_ecs/Cargo.toml:8`）与 `debug::MaybeLocation` |
| `CheckChangeTicks` / `check_change_ticks` | **0** | — | — | ✅ 存在（`kairos_ecs/src/change_detection/tick.rs:111`） |

### 3.7 其他

| API | Avian 用量 | 代表引用 | 核心还是工具 | Kairos 现状 |
|---|---|---|---|---|
| `World` 直接访问 | 16 | `src/schedule/mod.rs:235`（exclusive system `fn(world: &mut World, …)`）、`src/dynamics/solver/schedule.rs:193` | **核心** | ✅ `World`（prelude `:78`）；`try_schedule_scope`（`kairos_ecs/src/world.rs:3856`）、`run_schedule`（`:3953`）、`resource_mut`/`resource_scope`（`:2333,2878`） |
| `DeferredWorld` | 29 | `src/dynamics/rigid_body/mass_properties/components/mod.rs:5-6`、`src/collision/collider/backend.rs` | **核心**（hooks） | ✅ |
| `EntityGeneration` | `src/data_structures/sparse_secondary_map.rs:18` | — | **核心** | ✅ `EntityGeneration`（kairos_ecs 41 次） |
| `bevy::platform::hash::RandomState` | `src/data_structures/sparse_secondary_map.rs:13` | — | **核心** | ⚠ Kairos 侧对应 `kairos_collections::hash`（`kairos_ecs/src/schedule/schedule.rs:22` 用 `kairos_collections::hash::FixedHasher`）—— 名称不同，改写点小 |
| `bevy::platform::time::Instant` | `src/utils.rs:3`（re-export 给全库当 `crate::utils::Instant`） | — | **核心**（诊断计时） | ✅ `std::time::Instant` 即可 |
| `bevy::log` 宏（`trace!/debug!/info!/warn!/error!`） | `trace!` 2、`warn!` 6、`info!` 1 | `src/schedule/mod.rs:265`、`src/dynamics/solver/island` 附近 | **核心**（少量） | ✅ Kairos 各 crate 直接用 `log`（`kairos_physics/Cargo.toml:19`）、kairos_ecs 用 `log`+`tracing` |
| `Template` / `FromTemplate` / `SpawnRelated` / `RelatedSpawnerCommands` | `RelatedSpawnerCommands` 1（`src/diagnostics/ui.rs:12,168,345`） | — | **仅工具** | ✅ Kairos 有 `Template`/`FromTemplate`/`template`（prelude `:75`）、`SpawnRelated`/`WithRelated`（`:73`）—— **Avian 核心不用** |

### 3.8 `bevy_ecs` 使用面汇总表（core vs tooling）

| 层 | 核心仿真路径用到 | **仅**工具/测试用到 |
|---|---|---|
| 应用层 | `App`, `Plugin`, `PluginGroup`, `PluginGroupBuilder`, `App::add_systems/configure_sets/edit_schedule/init_resource/insert_resource/add_observer/world/world_mut/register_required_components/finish` | `App::register_diagnostic`, `init_gizmo_group`, `add_mesh` |
| 调度 | `Schedule`, `Schedules`, `ScheduleLabel`, `Interned<dyn ScheduleLabel>`, `SystemSet`, `configure_sets`+`in_set`+`chain`+`before`/`after`, `SingleThreadedExecutor`, `ScheduleBuildSettings`/`LogLevel`, `World::try_schedule_scope`, `ambiguous_with_all` | `run_system_once`（仅测试）、`ScheduleStepping` |
| 查询 | `Query`, `QueryData` derive, `With`/`Without`/`Or`/`Has`/`Changed`/`Ref`/`Mut`, lifetimeless `Read`/`Write`, `Has`, `Disabled`, `par_iter`/`par_iter_mut`, `ParallelCommands` | `Single`（UI 一处）, `Populated`（未用） |
| 系统参数 | `Res`, `ResMut`, `Local`, `Commands`, `EntityCommands`, `ParamSet`, `SystemParam`/`ReadOnlySystemParam`/`StaticSystemParam`/`SystemParamItem`, `SystemState`, `SystemChangeTick` | — |
| 组件模型 | `Component`, `Resource`, `Bundle`, required components, relationship + `RelationshipTarget`, component hooks, `DeferredWorld`+`HookContext`, `ComponentId`, `EntityMapper`+`MapEntities`, `entity_disabling::Disabled`, `Deref`/`DerefMut`/`From` derive | `Reflect`（171 derive / 161 属性 / 3 次 register_type）、`ReflectResource`、`ReflectMapEntities` |
| 事件 | `EntityEvent`, `Message`, `MessageReader`, `MessageWriter`, `Observer`, `On<Add\|Insert\|Remove>`, `add_observer` | `Single`+`Children` 的 UI 树 |
| 其他 | `bevy::tasks`（`ComputeTaskPool`/`ParallelSlice`/`AsyncComputeTaskPool`）、`bevy::platform::{Instant, RandomState}`、`bevy::log`、`bevy::diagnostic::DiagnosticPath`（泄漏）、`bevy::transform::{TransformSystems, GlobalTransform, TransformHelper}` | `bevy::scene`, `bevy::mesh`, `bevy::asset`, `bevy::gltf`, `bevy::picking`, `bevy::color`, `bevy::gizmos`, `bevy::ui`, `bevy::camera` |

---

## 4. 数学类型清单与 Kairos 差距

### 4.1 Avian 真正用到的 `bevy_math` 面

`bevy_math` 的出现次数（**含注释**）：`bevy_math::bounding::Aabb` 4、`bevy_math::Vec` 3、`bevy_math::IVec` 2、`bevy_math::primitives::` 1、`bevy_math::FloatPow` 1、`bevy_math::DVec` 1。显式 `use bevy_math::…` 共 10 处（`src/math/mod.rs:19`、`src/math/single.rs:2`、`src/math/double.rs:2`、`src/collider_tree/obvhs_ext.rs:1`、`src/debug_render/mod.rs:10,230,534`、`src/debug_render/gizmos.rs:198`、`src/collision/collider/parry/primitives3d.rs:1`、`src/collision/collider/parry/primitives2d.rs:5`、`src/dynamics/rigid_body/forces/tests.rs:6`、`src/collision/collider/trimesh_builder.rs:373`）。

标识符计数（**非注释代码行**）：

| 标识符 | 行数 | 是核心还是工具 | 说明 |
|---|---|---|---|
| `Scalar` | 651 | **核心** | Avian 自己的别名（= `f32` 或 `f64`），见 §4.2 |
| `Quaternion` | 70 | **核心** | = `Quat`（3D）/ `crate::physics_transform::Rotation`（2D），`src/math/mod.rs:87-91` |
| `Vec3` | 68 | **核心** | 3D 物理向量；也用于 `bevy_math` 的 f32 转换 |
| `Vec3A` | 24 | **核心** | 只在 `src/collider_tree/obvhs_ext.rs:1,20,22,31,35,159…`（obvhs 加速结构）与 debug_render |
| `Transform` | 94 | **核心** | bevy_transform，不是 bevy_math |
| `GlobalTransform` | 34 | **核心** | 同上 |
| `Time` | 44 | **核心** | bevy_time |
| `Vec4` / `Vec2` / `DVec3` / `DVec2` / `DQuat` | 5 / 24 / 21 / 8 / 9 | **核心**（f64 精度路径）/ 部分测试 | `src/math/double.rs`、`src/collision/collider/trimesh_builder.rs:373` |
| `Isometry2d` / `Isometry3d` | 11 / 3 | **核心**（joints 的 local/global frame） | `src/dynamics/joints/fixed.rs:74,81,150,161`、`src/dynamics/joints/mod.rs:799-877` |
| `Mat3` / `Mat2` | 11 / 5 | **核心** | `SymmetricMat3::from_mat3_unchecked` 等 |
| `Rot2` | 10 | 2D 专用（核心 2D / 工具 3D） | `src/dynamics/joints/mod.rs:852,872`、`src/math/mod.rs:582` |
| `Aabb3d` | 8 | **仅工具** | 全在 `src/debug_render/`（`mod.rs:10,230,534`、`gizmos.rs:198`） |
| `BoundingSphere` | 8 | **核心（来自 parry，不是 bevy_math）** | `src/collision/collider/parry/primitives2d.rs:75-86,325-336` 用的是 `parry::bounding_volume::BoundingSphere` |
| `FloatPow` | 1 | 测试 | `src/dynamics/rigid_body/forces/tests.rs:6` |
| `Dir2` / `Dir3` | 1 / 2 | **核心（但被别名遮蔽）** | `src/math/mod.rs:55,59` 定义 `pub type Dir = Dir2 / Dir3`；**全库没有任何地方写 `crate::math::Dir`**，实际由 `Ray`/shape 参数经 prelude 使用 |
| `Ray2d` / `Ray3d` | 1 / 1 | **核心** | `src/math/mod.rs:45-49` 定义 `pub(crate) type Ray = Ray3d` |
| `VectorSpace` / `Normed` / `FloatExt` / `Real` | **0 / 0 / 0 / 0** | — | **任务描述里列为候选，实测全库未使用** |
| `BoundingCircle` / `Aabb2d` | 0 / 0 | — | 2D 不用 bevy_math 的这两个 |

**对任务描述的一处修正**：`Aabb3d`/`BoundingSphere` **不是核心仿真依赖**；`BoundingSphere` 那一串命中是 **parry 的** 类型。核心仿真用 Avian 自己的 `ColliderAabb` + `obvhs::aabb::Aabb`，不是 `bevy_math::bounding::Aabb3d`。

### 4.2 `src/math/mod.rs` 定义了什么

`src/math/mod.rs` 668 行，是 Avian 的**维度/精度抽象层**，不是数学库。它定义：

| 项 | 位置 | 内容 |
|---|---|---|
| `DIM` | `:23`（2d）/ `:26`（3d） | `pub const DIM: usize = 2 / 3` |
| `VectorF32` | `:32`（2d `Vec2`）/ `:36`（3d `Vec3`） | f32 向量别名 |
| `IVector` | `:40`（`IVec2`）/ `:44`（`IVec3`） | 整数向量（broad phase proxy key） |
| `Ray` | `:45` / `:49` | `Ray2d` / `Ray3d` |
| `Dir` | `:55` / `:59` | `Dir2` / `Dir3` |
| `AngularVector` | `:63` / `:67` | 2D = `Scalar`，3D = `Vector` |
| `SymmetricTensor` | `:74` / `:79` | 2D = `Scalar`，3D = `SymmetricMatrix` |
| `Rot` | `:83` / `:87` | 2D = `crate::physics_transform::Rotation`，3D = `Quaternion` |
| `Isometry` | `:91` / `:95` | `Isometry2d` / `Isometry3d` |
| `trait AdjustPrecision` | `:100-106` | `type Adjusted; fn adjust_precision(&self) -> Self::Adjusted;` |
| `trait AsF32` | `:108-113` | `type F32; fn f32(&self) -> Self::F32;` |
| `trait RecipOrZero` | `:242-246` | `fn recip_or_zero(self) -> Self`（零除保护） |
| `trait MatExt` | `:307-313` | `Mat2/Mat3/SymmetricMat2/3` 的扩展：`from_diagonal`、`inverse`、`is_invertible` 等（9 个 impl，`Mat2:323`、`DMat2:353`、`SymmetricMat2:383`、`SymmetricDMat2:410`、`Mat3:437`、`DMat3:476`、`SymmetricMat3:515`、`SymmetricDMat3:547`） |
| `cross(a,b)` | `:245-250` 附近 | 2D `perp_dot` / 3D `cross` 的统一封装 |
| `skew_symmetric_mat3(v)` | `:630` | |
| `orthonormal_basis_from_vec(axis) -> Rot` | `:639` | |
| `orthonormal_basis(axes) -> Rot` | `:657` | |

`src/math/single.rs`（f32 版）与 `src/math/double.rs`（f64 版）定义具体的 `Scalar`（`single.rs:5`）、`Vector`、`Vector2/3`、`Matrix`/`Matrix2/3`、`SymmetricMatrix`/`SymmetricMatrix2/3`、`Quaternion`，以及 `AdjustPrecision` 的全部 impl（`single.rs:43-…`）。**`SymmetricMatrix = SymmetricMat3`（`single.rs:40`，来自 `glam_matrix_extras`）就是 §0 提到的硬绑定。**

### 4.3 `kairos_math` 实际导出

`kairos_math` = 13 个 `.rs` / 2 516 行（含 `vec.rs` 1 770、`quaternions.rs` 200、`affine/test.rs` 121、`lib.rs` 116、`matrix.rs` 76、`affine.rs` 74）。`kairos_math/src/lib.rs:26-32` 一次性 `pub use` 了 `aabb::*`、`affine_impl::affine`、`consts::*`、`matrix::*`、`quaternions::*`、`trigonometric::*`、`vec::*`。

| 类型 / trait | 位置 | 内部表示 | 关键 API |
|---|---|---|---|
| `float2` | `kairos_math/src/vec.rs:132` | `glam::Vec2`（字段 `pub(crate)`） | `new/from_array/to_array/x/y`、`ZERO/ONE`、全套算术 `impl` |
| `float3` | `kairos_math/src/vec.rs:521` | **`glam::Vec3A`** | `new/from_array/from_array_4/to_array/x/y/z/append`、`ZERO/ONE/RIGHT/LEFT/UP/DOWN/FORWARD/BACK` |
| `float4` | `kairos_math/src/vec.rs:953` | `glam::Vec4` | |
| `quaternion` | `kairos_math/src/quaternions.rs:16` | **`float4`**（即 `glam::Vec4`，非 `glam::Quat`） | `new/identity/from_euler/to_euler/normalized/normalize/from_look/to_float4x4`（`:22,27,32,50,70,76,81,149`） |
| `float4x4` | `kairos_math/src/matrix.rs:13` | `glam::Mat4` | `IDENTITY/new/trs/to_array/c0..c3`（`:18,21,27,38,43-59`） |
| `affine` | `kairos_math/src/affine.rs:17` | `glam::Affine3A` | `IDENTITY/trs/to_float4x4/inverse/transform_point/translation/to_scale_rotation_translation`（`:20,22,33,38,48,53,58`） |
| `AABB` | `kairos_math/src/aabb.rs:4` | `{ max: float3, min: float3 }` | **只有** `contains_point`（`:11`） |
| `trait Vector` | `kairos_math/src/vec.rs:12-73` | — | `dot/cross/len_sq/len/normalize/normalized/distance/distance_sq` |
| `Min` / `Max` / `Sqrt` / `Sin` / `Cos` / `Tan` / `Lerp` / `LerpFactor` | `kairos_math/src/lib.rs:34-49` + `trigonometric.rs` | — | 标量与向量的统一三角/插值 |
| 自由函数 | `kairos_math/src/lib.rs:52-78` | — | `float2/float3/float4/sin/sqrt/min/max/lerp` |
| `dot/cross/length/length_sq/normalize/normalized`（自由函数） | `kairos_math/src/vec.rs:78,86,94,102,110,118` | — | |
| 依赖 | `kairos_math/Cargo.toml:17` | `glam = { version = "0.33", features = ["bytemuck"] }`；`bytemuck` 必需（`:16`）；`serde`/`rkyv` 可选 | |

### 4.4 映射与缺口表

| Avian 需要 | Avian 来源（`file:line`） | Kairos 对应物 | 判定 | 证据 |
|---|---|---|---|---|
| `Scalar` / `Real` | `src/math/single.rs:5` / `src/math/double.rs:5` | `f32`（无别名） | 🟢 无成本 | Kairos 全栈 f32；`kairos_math::float3` 内部就是 f32 |
| `Vector`（维度相关） | `src/math/single.rs:17,19` | `float3` | 🟡 **形状不匹配** | Avian 的 `Vector` 是 `Vec3`（`glam::Vec3`，12 字节、非 SIMD）；Kairos 的 `float3` 内部是 `glam::Vec3A`（16 字节、SIMD，`kairos_math/src/vec.rs:521`）→ 内存布局与 `bytemuck::Pod` 性质都不同（`float3` 只实现 `Zeroable`，`vec.rs:825`；而 `float4x4` 实现了 `Pod`，`matrix.rs:74`） |
| `Vec3` / `Vec2` / `Vec4`（裸 glam） | 68 / 24 / 5 行 | 无（只有 newtype） | 🔴 **需要拆包/或保留裸 glam** | 若选"保留 `bevy_math`"路线则不需要动；若选"纯 Kairos 数学"路线，得给 `float3` 加 `pub fn inner()/from_inner()`（现在字段是 `pub(crate)`，外部 crate 拿不到） |
| `Vec3A` | `src/collider_tree/obvhs_ext.rs:1,20,22` | `float3` 恰好就是 `Vec3A` 包装 | 🟢 **天然匹配**（唯一一处 `Vec3A` 显式用法正好对上） | `kairos_math/src/vec.rs:521`：`pub struct float3(pub(crate) Vec3A)` |
| `DVec3` / `DVec2` / `DQuat` / `DMat2` / `DMat3` | `src/math/double.rs`、`src/collision/collider/trimesh_builder.rs:373` | **无** | 🟡 只在 `f64` feature 下需要；Kairos 若不做 f64 精度可直接砍 | `src/math/double.rs:2` `use bevy_math::*;` |
| `Quat` / `Quaternion` | 70 行 | `quaternion`（`glam::Vec4` 包装） | 🟡 **不是同一类型**：`bevy_math::Quat = glam::Quat`，Kairos 的 `quaternion` 存的是 `Vec4`。乘法/共轭语义需要核对 | `kairos_math/src/quaternions.rs:16` vs `src/math/single.rs:43` |
| `Mat2` / `Mat3` / `Matrix` | 5 / 11 行 | **无**（只有 `float4x4`） | 🔴 **缺** | Avian 需要 `Mat3` 做角惯量张量；`SymmetricMat3::from_mat3_unchecked` 收 `Mat3`（`src/dynamics/rigid_body/mass_properties/components/computed.rs:520,549`） |
| `SymmetricMat2/3`（`glam_matrix_extras`） | 73 行 | **无** | 🔴 **缺（最硬的缺口）** | `src/dynamics/rigid_body/mass_properties/components/mod.rs:12`；`computed.rs:431-711` 共 20+ 处 |
| `Dir2` / `Dir3` | `src/math/mod.rs:55,59` | **无归一化向量类型** | 🔴 **缺**（`kairos_math` 的 `normalize` 返回同样的 `float3`，无类型级不变式） | `kairos_math/src/vec.rs:110,118` |
| `Isometry2d` / `Isometry3d` | `src/math/mod.rs:91,95`；`src/dynamics/joints/mod.rs:799-877` | **无 pose 类型**（`LocalTransform` 是 ECS 组件，不是数学 pose） | 🟡 joints 需要时补；`parry3d::math::Pose` 可直接借用（Kairos 已在用，`kairos_physics/src/lib.rs:32,352`） | `kairos_physics/src/lib.rs:352-357`：`Pose::from_parts(Vector3, Rotation)` |
| `Aabb2d` / `Aabb3d` / `BoundingCircle` / `BoundingSphere` | `Aabb3d` 仅 `src/debug_render/`；`BoundingSphere` 来自 parry | `AABB { min, max: float3 }`（只有 `contains_point`） | 🟡 **形状不同**：Kairos 的 `AABB` 是 min/max 结构体，Avian 的 `bevy_math::Aabb3d` 是 `{min: Vec3A, max: Vec3A}`（debug 用），核心用 `obvhs::aabb::Aabb`（glam 0.31）与 Avian 自己的 `ColliderAabb` | `kairos_math/src/aabb.rs:4-17` |
| `Ray2d` / `Ray3d` | `src/math/mod.rs:45,49` | **无** | 🟡 可用 `parry3d::query::Ray` 替代（`src/collider_tree/obvhs_ext.rs:526-536` 已在做 `obvhs_ray(&Ray, max_distance)` 转换） | |
| primitives（`Cuboid`/`Sphere`/`Capsule3d`/`Cone`/`Cylinder`/`InfinitePlane3d`/`Line3d`/`Plane3d`/`Polyline3d`/`Segment3d`） | `src/collision/collider/parry/primitives3d.rs:1-4` | **无** | 🔴 **缺**（这些是 `Collider` 形状的构造源，但**可以用 `parry3d::shape` 替代**） | 同一文件 `:5` `use parry::shape::SharedShape;` —— Avian 是"用 bevy primitives 描述 + 转 SharedShape"，直接改成"直接构造 SharedShape"是可行的 |
| `VectorSpace` / `Normed` / `FloatExt` | **0 处使用** | — | 🟢 不需要 | |
| `FloatPow` | 1 处（测试） | **无** | 🟢 测试专用 | |
| `AdjustPrecision` / `AsF32` | `src/math/mod.rs:100,108`；`src/physics_transform/helper.rs:65-72` | **无** | 🟡 若不支持 f64，可整对删除；若支持，需自己实现（纯 `as` 转换） | `src/math/single.rs:43-…` |

**缺口速览**：`kairos_math` 缺的是 **`Mat2`/`Mat3`（非对称）、`SymmetricMat2/3`、`Dir2/Dir3`、`Ray2d/Ray3d`、`Isometry2d/3d`、bevy 的 shape primitives、`DVec*` 族**，以及**"裸 glam 类型"与"Kairos newtype"之间的桥**。而 `float3`（`Vec3A`）与 `float4x4`（`Mat4`）、`affine`（`Affine3A`）**布局对得上**，`quaternion`（`Vec4`）对不上。

### 4.5 glam 版本矩阵 —— 最硬的一个事实

| | Avian 0.7.0 | Kairos |
|---|---|---|
| 主 glam 版本 | **`0.32.1`** | **`0.33.1`** |
| 证据 | `Cargo.lock:3080-3088` | `Cargo.lock:1768-1772` |
| 使用者（逐条从 lock 反查） | `avian2d`（`:402`）、`bevy_math`（`:1246`）、`bevy_mesh`（`:1276`）、`bevy_reflect`（`:1443`）、`bevy_render`（`:1505`）、`glam_matrix_extras`（`:3101`）、`glamx 0.2.0`（`:3113`）、`hexasphere`（`:3333`） | `glamx 0.3.0`（`:1787`）、`kairos_engine`（`:2479`）、`kairos_math`（`:2570`）、`kira`（`:2651`） |
| 第二版本 | `glam 0.31.1`（`:3071-3076`），**唯一使用者是 `obvhs 0.3.1`**（`:4424`） | `glam 0.30.10 / 0.31.1 / 0.32.1`（`:2974-2976`），**唯一使用者是 `nalgebra`** |
| parry | `parry3d 0.27.0`（`:4629-4636`），经 `glamx 0.2.0` → glam 0.32.1 | `parry3d 0.28.0`（`Cargo.lock:3634-3636`），经 `glamx 0.3.0` → glam 0.33.1 |
| `obvhs` | `0.3.1`（`:4418-4429`），`glam = ">=0.30.10, <0.32"`（docs.rs）→ glam 0.31.1 | Kairos 未依赖 |
| `glam_matrix_extras` | `0.3.0`（`:3094-3105`），`glam = "0.32"` + `bevy_reflect = "0.19"`（docs.rs） | Kairos 未依赖 |
| `bevy_math` | `0.19.0`（`:1237-1250`，glam 引用于 `:1246`） | Kairos 未依赖 |
| `bevy_heavy` | `0.5.0`（`:1011-1019`） | Kairos 未依赖 |
| `rapier` | 未使用（只出现在文档对比里） | `rapier3d 0.33.0`（`Cargo.lock:4204-4206`） |

**三条硬结论**：

1. **Avian 与 Kairos 的数学类型在类型系统层面是两套**。`bevy_math::Vec3` 是 `glam 0.32.1::Vec3`，Kairos 的 `float3` 包的是 `glam 0.33.1::Vec3A`；`glam::Vec3` 与 `glam::Vec3A` 在 0.33 里是**不同类型**，0.32 与 0.33 之间更是**不同 crate 实例**（Cargo 会把 `glam 0.32.1` 与 `glam 0.33.1` 当作两个 crate 同时链接，就像 Kairos 现在同时链了 4 个 glam 一样）。任何"直接 `move` Avian 的数学代码"都是不可能的；必须做一次**类型替换**（把 `Vector`/`Vec3` 换成 `float3`）或一次**版本对齐**（把 Kairos 降到 glam 0.32 / 把 Avian 升到 glam 0.33 —— 后者不可能，因为 `bevy_math 0.19` 钉在 0.32）。

   Kairos 侧的 `Cargo.lock` 已经证明"多 glam 并存"是可行的（`:2974-2977` 同 4 个版本），所以**在同一个 crate 图里同时存在 glam 0.32 与 0.33 是合法的**，只是跨版本要做显式转换（`kairos_physics/src/lib.rs:360-376` 已经在做这种形状的手写转换：`to_rapier_vec3 / to_rapier_rotation / to_float3 / quat_from_rapier`，且注释明说"explicit engine ↔ rapier conversions … orphan rules forbid them there, #139"）。

2. **`obvhs` 有干净的升级路径**：Avian 锁的是 0.3.1（`glam <0.32`），而 **0.3.3 的约束是 `glam >= 0.30.10, <0.34`**，直接支持 Kairos 的 glam 0.33.1。所以 `src/collider_tree/`（2 944 行、bevy 密度 0.24%）可以把 `obvhs::aabb::Aabb` 与 glam 0.33 的 `Vec3`/`Vec3A` 统一，不用再混两套 glam。

3. **`glam_matrix_extras` 没有升级路径**：0.3.0 是 crates.io 上的**最新版**，钉死 `glam = "0.32"`。要上 glam 0.33 只能 fork。好消息是它很小（`src/lib.rs` 32 行 + `ops.rs` / `mat_ext.rs` / `rectangular.rs` / `symmetric.rs` / `eigen.rs` 五个模块，见 docs.rs 源码页），且是同一作者（Jondolf）在同一许可（`MIT OR Apache-2.0`）下发布的 —— **fork 成本「估算」在百行量级**（基于它导出面的规模：`SymmetricMat2/3`、`SymmetricDMat2/3`、`SquareMatExt`、`MatConversionError`、`eigen` 模块）。

---

## 5. `bevy_heavy` / `bevy_transform_interpolation`：是什么，替换要多少

### 5.1 `bevy_heavy`：**纯数学 crate，非 ECS 耦合**

**依赖证据**（Avian `Cargo.lock:1011-1019`）：

```
name = "bevy_heavy"
version = "0.5.0"
dependencies = ["approx", "bevy_math", "bevy_reflect", "glam_matrix_extras", "serde"]
```

**没有 `bevy_ecs`。** 它依赖的是 `bevy_math`（glam 0.32.1）、`bevy_reflect`、`glam_matrix_extras`。所以它的耦合性质是**"绑在 bevy 数学类型 + reflect 上"**，不是"绑在 ECS 上"。这一点很重要，因为修法完全不同。

> 本地未 vendored（`~/.cargo/registry/src/.../` 里没有 `bevy_heavy-*`，见 §8），因此**我无法逐行读它的源码**。下面的"用它什么"完全来自 Avian 的使用点。

**Avian 用了 `bevy_heavy` 的什么**（`grep -rEoh`，含注释）：

| item | 次数 | 使用点 |
|---|---|---|
| `MassProperties`（2D/3D 的别名） | 23 | `src/dynamics/rigid_body/mass_properties/mod.rs:216-223`：`#[cfg(feature = "2d")] pub(crate) use bevy_heavy::{ComputeMassProperties2d as ComputeMassProperties, MassProperties2d as MassProperties};` / `#[cfg(feature = "3d")] …3d…` |
| `AngularInertiaTensor` | 18 | `src/dynamics/rigid_body/mass_properties/components/mod.rs:9`、`:1096`（测试）、`src/dynamics/mod.rs:94` |
| `ComputeMassProperties`（2D/3D） | 12 + 4 + 4 | `src/dynamics/rigid_body/mass_properties/mod.rs:216-223`、`src/dynamics/mod.rs:94` |
| `MassProperties2d` / `MassProperties3d` | 2 / 2 | 同上 |
| `AngularInertiaTensorError` | 1 | `src/dynamics/mod.rs:94` |
| `MassPropertiesExt`（Avian 自己的扩展 trait） | `src/dynamics/rigid_body/mass_properties/mod.rs:225-249` | `fn to_bundle(&self) -> MassPropertiesBundle`，把 `mass`/`principal_angular_inertia`/`local_inertial_frame` 映射到 `Mass`/`AngularInertia`/`CenterOfMass` |

**Avian 没有** `MassPropertiesBundle` 来自 `bevy_heavy` —— `MassPropertiesBundle` 是 Avian 自己的（`src/dynamics/rigid_body/mass_properties/components/mod.rs:1038`，`#[derive(Bundle, …)]` 在 `:1036`）。`GlobalAngularInertia` **不在 Avian 0.7.0 里**（全库无此标识符）；对应概念是 Avian 自己的 `ComputedAngularInertia`（`src/dynamics/rigid_body/mass_properties/components/computed.rs:222` 与 `:428`）。

**替换成本**：`bevy_heavy` 的作用是"从 shape 算 mass / inertia tensor"。它读 `bevy_math` 的 primitives（`Collider::shape_scaled()` → `ComputeMassProperties3d::mass_properties(shape, density)`）。在 Kairos 里，这条路径可以：
- 直接用 **`parry3d 0.28` 的 `parry3d::mass_properties::MassProperties`**（Kairos 已在用 parry），或者
- 自己写（`Cuboid`/`Sphere`/`Capsule` 的惯量解析式是教科书公式）。
**估算**：`bevy_heavy` 的公开面很小（4 个类型 + 2 个 trait 方法），且它是 `MIT OR Apache-2.0`（crates.io 元数据），**fork 到 glam 0.33 的工作量在几百行量级** —— 但因为它内部依赖 `glam_matrix_extras` 的 `SymmetricMat3`，**必须先解决 §4.5 的第 3 条**。

### 5.2 `bevy_transform_interpolation`：**ECS 强耦合，只能重写**

**依赖证据**（Avian `Cargo.lock:1756-1761`）：

```
name = "bevy_transform_interpolation"
version = "0.5.0"
dependencies = ["bevy", "serde"]
```

依赖 **umbrella `bevy`** → 至少拉进 `bevy_app` / `bevy_ecs` / `bevy_math` / `bevy_reflect` / `bevy_transform` / `bevy_time`。**这就是 ECS 强耦合的直接证据。**

Avian 的使用面（只有 **2 个文件**）：

| 使用点 | 内容 |
|---|---|
| `src/interpolation.rs:6` | `use bevy_transform_interpolation::{VelocitySource, prelude::*};` |
| `src/interpolation.rs:9-19` | `pub use` 了 **16 个** item：`TransformEasingSet`、`TransformEasingSystems`，以及 prelude 里的 `NoRotationEasing`、`NoScaleEasing`、`NoTransformEasing`、`NoTranslationEasing`、`RotationExtrapolation`、`RotationHermiteEasing`、`RotationInterpolation`、`ScaleInterpolation`、`TransformExtrapolation`、`TransformHermiteEasing`、`TransformInterpolation`、`TranslationExtrapolation`、`TranslationHermiteEasing`、`TranslationInterpolation` |
| `src/interpolation.rs:266-289` | `PhysicsInterpolationPlugin::build`：`app.add_plugins((TransformInterpolationPlugin::default(), TransformExtrapolationPlugin::<LinVelSource, AngVelSource>::default(), TransformHermiteEasingPlugin::<LinVelSource, AngVelSource>::default()))`，并按开关做 `try_register_required_components::<RigidBody, TranslationInterpolation\|RotationInterpolation\|TranslationExtrapolation\|RotationExtrapolation>()` |
| `src/interpolation.rs:301-308` | 两个 Avian 私有组件 `PreviousLinearVelocity(Vector)`、`PreviousAngularVelocity(AngularVelocity)`，都是 `#[derive(Component, Default, Deref, DerefMut)]` |
| `src/interpolation.rs:310-382` | 两个 `#[derive(QueryData)]` 空结构体 `LinVelSource` / `AngVelSource`，实现 `VelocitySource`（`type Previous; type Current; fn previous(&Self::Previous)->Vec3; fn current(&Self::Current)->Vec3`） |
| `src/lib.rs:786` | `PhysicsPlugins::build` 默认挂上 `PhysicsInterpolationPlugin::default()` |
| `src/interpolation.rs:177-181` | 文档明确"Avian uses `bevy_transform_interpolation`" |

**替换成本**：必须**在 `kairos_ecs` 上重写**，因为它的功能就是"读 `Transform` 历史 + 写 `Transform`"，本质是 ECS 系统 + 组件集合。要重写的最小集是：
- 组件：`TranslationInterpolation`/`RotationInterpolation`/`ScaleInterpolation`/`TranslationExtrapolation`/`RotationExtrapolation` + Hermite 变体 + `No*Easing` opt-out 标记（约 12 个 marker/newtype 组件）；
- 系统：每帧平滑系统（在 `PostUpdate`/`Last` 阶段做 lerp/slerp），加 `TransformEasingSystems` 集合；
- trait：`VelocitySource`（Avian 侧已给出两个 impl，说明 trait 形状很简单：两个关联类型 + 两个静态方法）。
**「估算」**：这是 12 个组件 + 3～5 个系统 + 1 个 trait 的规模，量级在 **几百行**；但它是**从零写**，没有可抄的源码（本地未 vendored），只能照 Avian 的调用面反推行为。
**关键判断**：**它是完全可延后的**。Avian 把它做成一个独立 plugin（`src/lib.rs:786`），插值只影响**视觉平滑**，不影响物理正确性。Kairos 的增量学习路径（colliders → rigid bodies → integration → collision detection）**根本不需要它**。

---

## 6. Kairos 侧现实检查

> **路径约定提醒**（与 `01`–`04` 不同，容易误读）：本系列的 `01`–`04` 用裸 `src/...` 指 **Avian 检出**。
> 本章讲的是 Kairos 侧，故**本章各表格中的裸 `src/...` 一律指 `kairos_ecs/src/...`**
> （例如 `src/world.rs:493` = `kairos_ecs/src/world.rs:493`）；`macros/src/...` 指 `kairos_ecs/macros/src/...`。
> 需要指 Avian 时本章一律写全（如 `src/dynamics/integrator/mod.rs`）。

### 6.1 `kairos_ecs`：几乎完整的 `bevy_ecs` 0.19 fork

**体量**：`find kairos_ecs -name '*.rs' | wc -l` = **238 个文件**，`xargs wc -l` 合计 **113 063 行**（`src/` 下 108 954 行，含测试）。模块目录与 bevy_ecs 一一对应：`archetype.rs`、`bundle/`、`change_detection/`、`component/required/`、`entity_disabling/`、`hierarchy.rs`、`intern.rs`、`lifecycle.rs`、`message/`、`observer/`、`parallel_queue.rs`、`query/`、`reflect.rs`、`relationship.rs`、`resource.rs`、`schedule/{condition,executor,graph,node,pass,set,stepping}`、`spawn/`、`storage/`、`system/{commands,function_system,observer_system,system_param}`、`template/`、`world/{command_queue,deferred_world,entity_access,filtered_resource,spawn_batch,unsafe_world_cell}`。

**逐项核对结果**（每条都给了 `kairos_ecs/src/...:LINE`）：

| Avian 需要 | Kairos 状态 | 证据 |
|---|---|---|
| `Component` derive | ✅ **存在** | `kairos_ecs/macros/src/lib.rs:830` `#[proc_macro_derive(Component, attributes(component, require, relationship, relationship_target, entities))]`；trait 路径 `kairos_ecs::component::Component` |
| `Resource` derive | ✅ | `macros/src/lib.rs:591` |
| `Bundle` derive | ✅ | `macros/src/lib.rs:58` |
| `ScheduleLabel` trait + derive | ✅ | trait `src/schedule/schedule.rs:41`；derive `macros/src/lib.rs:502`；别名 `InternedScheduleLabel`（`src/schedule/schedule.rs:30`）；`Interned<T>`（`src/intern.rs:49`） |
| `SystemSet` + derive | ✅ | 17 个 `#[proc_macro_derive]` 之一，`macros/src/lib.rs:516` |
| `SystemParam` derive / `ReadOnlySystemParam` / `StaticSystemParam` | ✅ | trait `src/system/system_param.rs:248`（`pub unsafe trait SystemParam`）；derive `macros/src/lib.rs:240`；`ReadOnlySystemParam` 37 次出现；lifetimeless 别名 `:2172-2183` |
| `QueryData` derive（含 `#[query_data(mutable)]`） | ✅ | trait `src/query/fetch.rs:379`（`pub unsafe trait QueryData: WorldQuery`）；derive `macros/src/lib.rs:488`；属性解析 `macros/src/query_data.rs:26,127-155` |
| `QueryFilter` derive | ✅ | trait `src/query/filter.rs:113`；derive `macros/src/lib.rs:494` |
| `Query` / `Single` / `Populated` | ✅ | `Query` prelude `src/lib.rs:71-73`；`Single` `src/system/query.rs:2876`；`Populated` `src/system/query.rs:2918`；`Query::single`/`single_mut` `:2121,2150` |
| `par_iter` / `par_iter_mut` | ✅ | `src/system/query.rs:1294,1329`；`src/query/state.rs:1447,1499`；实现文件 `src/query/par_iter.rs` |
| `ParallelCommands` | ✅ | prelude `src/lib.rs:86`；`src/system/commands/parallel_scope.rs` |
| `Add` / `Insert` / `Remove` / `Despawn` / `Discard`（lifecycle）+ `On<E>` | ✅ | prelude `src/lib.rs:57`（lifecycle）、`:62`（`observer::{Observer, ObserverSystemExt, On}`） |
| 观察者 `add_observer` | ✅ | `World::add_observer` `src/observer.rs:67`；`Commands::add_observer` `src/system/commands.rs:1219` |
| 组件钩子（`on_add`/`on_remove`/`on_discard`）+ `DeferredWorld` + `HookContext` | ✅ | `World::register_component_hooks` `src/world.rs:424`；`register_component_hooks_by_id` `:438`；`DeferredWorld` 在 `src/world/deferred_world.rs`（`query` `:495`、`resource_mut` `:511`）；Kairos 自己的 `kairos_physics` 已在用（`kairos_physics/src/lib.rs:317-327`） |
| required components（运行时 + `#[require]`） | ✅ | `World::register_required_components` `src/world.rs:493`、`register_required_components_with` `:544`、`try_register_required_components` `:599`、`try_register_required_components_with` `:548` |
| relationship：`Relationship` / `RelationshipTarget` / `ChildOf` / `Children` | ✅ | `RelationshipTarget` prelude `src/lib.rs:64`；`ChildOf`/`Children`/`ChildSpawner`/`ChildSpawnerCommands` prelude `:56`；trait `src/relationship.rs`（`Relationship` 81 次、`RelationshipTarget` 80 次） |
| `entity_disabling::Disabled` | ✅ | `src/entity_disabling.rs:133` |
| `EntityMapper` / `MapEntities` derive | ✅ | prelude `src/lib.rs:53`；derive `macros/src/lib.rs:214` |
| `Message` / `MessageReader` / `MessageWriter` / `Messages` + derive | ✅ | prelude `src/lib.rs:58-60`；derive `macros/src/lib.rs:566`；模块 `src/message/{message_reader,message_writer,messages,message_mutator}.rs` |
| `EntityEvent` derive | ✅ | prelude `src/lib.rs:55`；derive `macros/src/lib.rs:560` |
| `SystemChangeTick` | ✅ | `src/system/system_param.rs:1638`，`this_run()` `:1646` |
| `CheckChangeTicks` | ✅ | `src/change_detection/tick.rs:111` |
| `SystemState` | ✅ | 34 次出现 |
| `RunSystemOnce` + `world.run_system_once` | ✅ | 10 次出现 |
| `Deref` / `DerefMut` derive | ❌ **不存在于 `kairos_ecs_macros`** | 17 个 `#[proc_macro_derive]` 全清单（`macros/src/lib.rs:58,214,240,488,494,502,516,542,560,566,591,638,830,850,911,917`）里没有它们；但 `kairos_ecs` 已依赖 `derive_more` 且开了 `deref`/`deref_mut`（`kairos_ecs/Cargo.toml:34-40`） |
| `From` derive | ❌ 同上（Avian 用 21 次） | Avian 里多数来自 `bevy_derive` 的 `From` |
| `Reflect` derive / `ReflectResource` / `ReflectMapEntities` / `AppTypeRegistry` | ❌ **不存在（且默认关闭）** | `kairos_ecs/src/reflect.rs` = **1 字节空文件**；`kairos_reflect` feature 定义在 `Cargo.toml:11`，**不在 `default = ["debug", "trace"]`（`Cargo.toml:10`）里**；prelude 里的 reflect 导出被 `#[cfg(feature = "kairos_reflect")]` 门控（`src/lib.rs:88-94`） |
| **`App` / `Plugin` / `PluginGroup` / `PluginGroupBuilder`** | ❌ **完全不存在** | 全 workspace `grep -rn 'pub struct App'` / `'pub trait Plugin'` / `'PluginGroup'`（排除 `target/`）命中 **0**；只有 4 处注释提到 bevy 的 `MainSchedulePlugin` / `TimePlugin`（`kairos_ecs/src/system/tests.rs:730`、`kairos_engine/src/kairos_editor/schedule.rs:5,168`、`kairos_time/src/lib.rs:33`） |
| `Schedule` / `Schedules` / `configure_sets` / `add_systems` / `edit_schedule` | ✅ | `Schedules::add_systems` `src/schedule/schedule.rs:261`（`Schedules` impl）与 `:473`（`Schedule` impl）；`configure_sets` `:288`/`:539`；`configure_schedules` `:222`；`set_executor` `:587`；`set_build_settings` `:565`；`set_apply_final_deferred` `:597`；`run` `:603`；`check_change_ticks` `:699` |
| `World::try_schedule_scope` / `schedule_scope` / `run_schedule` | ✅ | `src/world.rs:3856` / `:3919` / `:3953` |
| `SingleThreadedExecutor` / `MultiThreadedExecutor` / `MainThreadExecutor` | ✅ | `src/schedule/executor/single_threaded.rs:22`；`multi_threaded.rs:101`；`MainThreadExecutor` `multi_threaded.rs:855` |
| `ComputeTaskPool` / `ParallelSlice` / `AsyncComputeTaskPool` | ✅（在 `kairos_tasks`） | `kairos_tasks/src/lib.rs:11,12,14`；`TaskPool`/`ThreadExecutor` `:12,13` |

**`Schedules` 的安装路径（Kairos 的 "plugin" 形态）**：`kairos_engine/src/kairos_editor/schedule.rs:245-288` 的 `pub(crate) fn install(world: &mut World)` 是 bevy `MainSchedulePlugin` 的等价物：它插入 `Time`/`FixedTime`/`MainScheduleOrder` 资源，建 `First`（挂 `time_system`）、`RunFixedMainLoop`（挂 `run_fixed_main_loop`，该函数在 `:222`）、`FixedUpdate`、`Update`、`PostUpdate`、`Extract`、`Last`、`Main` 子调度，并把 `FixedUpdate` 的 0..N 次驱动放在 `run_fixed_main_loop` 里。`kairos_physics::install(world, fixed_update_stage)`（`kairos_physics/src/lib.rs:339-355`）就是在这个骨架上注册物理系统 —— **它接受 `impl ScheduleLabel`，形式与 `avian3d` 的 `PhysicsPlugins::new(schedule: impl ScheduleLabel)`（`src/lib.rs:702`）完全同构**。这说明"把 Avian 的 plugin 改写成 `install(world, schedule)` 函数"在架构上是有现成范式的。

### 6.2 `kairos_transform`：**没有传播系统，这是硬阻塞**

体量：3 个 `.rs` / 376 行。

| 类型 | 位置 | 形状 |
|---|---|---|
| `LocalTransform` | `kairos_transform/src/local_transform.rs:26-38` | `#[derive(Component, Debug, Clone, Copy, PartialEq)]`，字段 `position: float3`、`rotation: quaternion`、`scale: float3`（全 `pub`） |
| `LocalTransform::compute_local_matrix` | `local_transform.rs:88` | 返回**local→parent** 的 `float4x4`（`float4x4::trs(...)`） |
| `LocalTransform::look_at` | `local_transform.rs:69` | |
| `GlobalTransform` | `kairos_transform/src/global_transform.rs:26` | `#[derive(Component, …)] pub struct GlobalTransform(affine);` |
| `GlobalTransform` 访问器 | `global_transform.rs:29,35,42,48` | `translation()` / `rotation()` / `to_float4x4()` / `transform_point()` |

**关键缺失**（原文，`kairos_transform/src/lib.rs:15-31`）：

> 「# Component lifecycle … The propagation system that maintains each entity's [`GlobalTransform`] from its [`LocalTransform`] hierarchy has **not** landed yet (a wayfinder map follow-up). Until it does:
> - do not spawn entities with a [`GlobalTransform`], and do not read it at consumption sites — while every scene is a root scene (`local == world`), read [`LocalTransform`] directly;
> - [`GlobalTransform`] is still a spawnable `Component` with a meaningful [`Default`] (the identity transform) so the API is in place for the propagation system.」

`global_transform.rs:13-22` 重复了同一警告。

**Avian 侧对应需求**（全部是核心路径）：

| Avian 需要 | 证据 | Kairos |
|---|---|---|
| `TransformSystems::{Propagate, TransformPropagate}` 作为排序锚点 | `src/schedule/mod.rs:84`（`.before(TransformSystems::Propagate)`）、`src/spatial_query/mod.rs:203`、`src/debug_render/mod.rs:134` | ❌ 无 |
| 直接调用 bevy_transform 的三个传播系统 | `src/physics_transform/mod.rs:26`（`use bevy::transform::systems::{mark_dirty_trees, propagate_parent_transforms, sync_simple_transforms};`）、`:98-100`（三个系统注册进 `PhysicsTransformSystems::Propagate`） | ❌ 无 |
| `TransformHelper`（bevy_transform 的 `SystemParam`，按需算单个实体的全局变换）+ `ComputeGlobalTransformError`（`MissingTransform` / `NoSuchEntity` / `MalformedHierarchy`） | `src/physics_transform/helper.rs:9`、`:33`、`:45`、`:59-64` | ❌ 无（也没有等价的错误枚举） |
| `GlobalTransform` 的 `From<Transform>` / `Mul` 组合语义 | `src/physics_transform/transform.rs:74-106`（`impl From<GlobalTransform> for Position`）、`:1058-1071`（`impl From<Transform> for Rotation`）、`:1142-1199`（钩子里算 `parent_global_transform * GlobalTransform::from(transform)`） | 🟡 `GlobalTransform` 有 `translation/rotation/to_float4x4/transform_point`，但**没有 `Mul`**，且语义未验证（模块自述"不要读"） |

**结论**：这不是"少一个优化"，而是**少一个子系统**。Avian 的 `PhysicsTransformPlugin`（`src/physics_transform/mod.rs:56-130`）整个建立在"bevy 已经维护好了 `GlobalTransform` 与层级脏标记"这个前提上；`Position`/`Rotation` 与 `Transform` 的双向同步（`transform.rs` 1 275 行）也依赖它。Kairos 要移植 physics transform 层，**必须先有 `kairos_transform` 的传播系统**。

### 6.3 `kairos_time`：**具体时钟，不是泛型时钟**

体量：2 个 `.rs` / 616 行。

| 类型 | 位置 | 形状 |
|---|---|---|
| `Time`（具体结构体） | `kairos_time/src/lib.rs:36` | 字段：`start_time`、`pre_time`、`total_time`、`delta_time`、`total_frame`、`time_scale`、`paused`、`max_delta`（`:37-50`） |
| `Time::update` / `update_with_raw_delta` | `:69` / `:82` | 每帧由 `First` 阶段的 `time_system` 调一次 |
| `Time::delta_time` / `delta_time_secs` | `:113` / `:118` | 返回 `Duration` / `f32` |
| `FixedTime`（具体结构体） | `kairos_time/src/lib.rs:178` | 字段：`timestep`、`overstep`、`delta`、`elapsed`（`:180-189`） |
| `FixedTime::accumulate` / `expend` | `:272` / `:286` | 语义镜像 `Time<Fixed>`：`checked_sub` 预付一步，余数跨帧保留 |
| `FixedTime::timestep` | `:240` | 返回 `Duration` |
| 默认固定步长 | `:154-160` | 64 Hz（15 625 µs），与 bevy 一致 |
| 默认 `max_delta` | `:27` | 250 ms，与 bevy `Time<Virtual>` 一致 |

**Kairos 缺什么**（对照 §3.2 与 `src/schedule/time.rs`）：

| Avian 需要 | 证据 | Kairos |
|---|---|---|
| 泛型 `Time<T: Clock>` | `src/schedule/time.rs:222` `impl PhysicsTime for Time<Physics>`；`src/schedule/mod.rs:61` `app.register_type::<Time<Physics>>()`、`:62` `init_resource::<Time<Physics>>()`、`:63` `Time::new_with(Substeps)` | ❌ `Time` 是无参数具体类型（`kairos_time/src/lib.rs:36`） |
| 自定义 clock 标签 `Physics`（`Resource + Default + Reflect + 含 paused/relative_speed`） | `src/schedule/time.rs:123-136` | ❌ 无 |
| 自定义 clock 标签 `Substeps` | `src/schedule/time.rs:271` | ❌ 无 |
| `Time::<Physics>::advance_by(Duration)` | `src/schedule/mod.rs:245,255`、`src/dynamics/solver/schedule.rs:200` | ❌ `FixedTime` 有 `accumulate`/`expend`，但语义不同（那是累加器，不是时钟推进） |
| `Time::as_generic()` / `*world.resource_mut::<Time>() = …`（把 generic `Time` 临时换成 Physics/Substeps 时钟，跑完再换回） | `src/schedule/mod.rs:250,253,261,278-280`；`src/dynamics/solver/schedule.rs:205` | ❌ 无 generic `Time` 就无从谈起 |
| `Time::delta_secs_f64()` / `delta_secs()` / `mul_f64` / `relative_speed_f64()` | `src/schedule/mod.rs:238-243`、`src/dynamics/integrator/mod.rs:352`（`time.delta_secs_f64() as Scalar`） | 🟡 `Time::delta_time_secs() -> f32`（`:118`）存在；**没有 f64 版本**、没有 `relative_speed` |
| `Time::is_paused()` | `src/schedule/mod.rs:238,258` | 🟡 `Time` 有 `paused` 字段与 `pause()`/`resume()`（`:150,154`），但**没有 `is_paused()` 访问器** |
| `PhysicsTime` trait（`with_relative_speed` / `relative_speed` / `set_relative_speed` / `pause` / `unpause` / `is_paused`，`f32` 与 `f64` 双版本） | `src/schedule/time.rs:138-220` | ❌ 无 |
| `Time<Substeps>` 的 `SubstepCount` 除法 | `src/schedule/mod.rs:246-249` | 🟡 对应物是 `kairos_physics` 直接读 `FixedTime::timestep()`（`kairos_physics/src/lib.rs:292`） |

**结论**：这是**第二个真实子系统缺口**。Avian 的整个 `schedule` 模块（`src/schedule/mod.rs` 306 行 + `time.rs` 292 行）和 `solver/schedule.rs` 都建立在这套泛型时钟上；而"物理时钟可暂停、可变速、可在 `PhysicsSchedule` 内被临时替换为 generic `Time`"是 Avian 的**公开 API 承诺**（`src/schedule/time.rs:9-83` 的文档示例）。Kairos 要么补一个 `Time<T: Clock>` 泛型时钟，要么把 Avian 的这套逻辑改写成"具体时钟 + 手动传入 dt"（这正是 `kairos_physics` 现在的做法：`physics.step(fixed_time.timestep().as_secs_f32())`，`kairos_physics/src/lib.rs:292`）。

### 6.4 `kairos_physics`：现在做什么（860 行）

体量：4 个 `.rs` / 860 行 —— `lib.rs` 387、`tests.rs` 429、`collider.rs` 27、`rigid_body.rs` 17。

| 项 | 位置 | 内容 |
|---|---|---|
| `PhysicsEngine`（`#[derive(Resource)]`） | `kairos_physics/src/lib.rs:41-56` | 私有字段持 rapier 全套：`rigid_body_set`、`collider_set`、`gravity: float3`、`integration_parameters`、`island_manager`、`broad_phase: DefaultBroadPhase`、`narrow_phase`、`impulse_joint_set`、`multibody_joint_set`、`ccd_solver`、`physics_hooks: ()`、`event_handler: ()`、`physics_pipeline` |
| 构造器 | `:59-79`（`new`）；`:93`（`insert_movable_sphere`）；`:133`（`insert_immovable_box`） | 三种硬编码形状：动态球（带 `CoefficientCombineRule::Max` 的 restitution）、不可动盒 |
| `step(dt)` | `:169-190` | 只改 `integration_parameters.dt` 然后 `physics_pipeline.step(...)` |
| `on_discard_rigid_body` / `on_discard_collider` | `:200` / `:224` | `DeferredWorld` + `HookContext` 钩子，删除 rapier 对象 |
| `physics_step_system` | `:252-292` | **唯一系统**：push（`LocalTransform` → rapier `set_position`，`wake=true`）→ `step` → pull（rapier → `LocalTransform.position`/`rotation`，**不写 `scale`**） |
| `install(world, fixed_update_stage)` | `:317-330` | 插资源 + 注册钩子 + `schedules.get_mut(stage).add_systems(physics_step_system)` |
| `RigidBody` | `kairos_physics/src/rigid_body.rs:16-17` | `#[derive(Component, Debug, PartialEq)] pub struct RigidBody { pub(crate) handle: RigidBodyHandle }` —— **只有 1 个字段** |
| `Collider` | `kairos_physics/src/collider.rs:25-27` | `#[derive(Component, Debug, PartialEq)] pub struct Collider { pub(crate) handle: ColliderHandle }` —— **只有 1 个字段** |
| `ColliderMaterial` | `collider.rs:7-9` | `{ restitution: f32 }` |
| 依赖 | `kairos_physics/Cargo.toml:20` | `rapier3d = { version = "0.33.0", features = ["simd-stable", "parallel", "serde-serialize"] }` |

**对照 Avian 的对应面**：Avian 有 **95 个 `Component` 类型**（`#[derive(Component)]` 所在的 struct 数）、**30 个 `Resource` 类型**、**17 个 `SystemSet`**、**28 个 `impl Plugin`**、**2 个 `ScheduleLabel`**；`Collider` 是一个 **715 行**的 `mod.rs` + **2 983 行**的 `parry/` 子系统（`parry/mod.rs` 1 830 + `contact_query.rs` 613 + `primitives2d/3d`），`RigidBody` 在 `dynamics/rigid_body/mod.rs`（662 行）。Kairos 现在的 `RigidBody`/`Collider` 是**两个 handle 包装**，二者之间是**全部重写**，不是增量。

---

## 7. 结论：三条路线的成本

### 7.1 路线 (a)：Vendor-and-adapt（把 Avian `src/` 拷进 Kairos crate，改 ECS/数学/transform）

**要动多少（实测，非估算）**：

| 指标 | 数值 | 出处 |
|---|---|---|
| 待 vendor 的文件 | **128** 个 `.rs` | §1 |
| 待 vendor 的行数 | **52 307** | §1 |
| 扣掉文档注释 / 空行 / 独立测试文件后的实体代码 | **≈ 31 177**（「估算」，`#[cfg(test)]` 内联测试未扣） | §1 |
| 含 `bevy` 的**非注释代码行** | **205**（分布 **114** 个文件） | §1 |
| `use bevy…;` 语句 | **146**（含测试；非测试文件 106 个） | §1 |
| 需要改动 import 的文件 | **111**（含测试） | §1 |
| 完全不含 bevy 的文件（可直接原样落地） | **12**（3 567 行） | §1 |
| `#[derive(Component)]` 类型数 | **95** | §1 |
| `#[derive(Resource)]` 类型数 | **30** | §1 |
| `#[derive(SystemSet)]` 数 | **17** | §1 |
| `impl Plugin` 数 | **28**（+ 2 `impl PluginGroup`） | §1 |
| `#[derive(Reflect)]` / `#[reflect(...)]` 数 | **171 / 161** | §1 |
| `Deref`/`DerefMut` derive 行数 | **46** | §1 |
| 非注释 `unsafe` 行数 | **57** | §7.4 |

**"这活到底多大"的判断**：把 Avian 的**算法层**落地是便宜的 —— 8 623 行的 solver 只碰 30 行 bevy、2 944 行的 `collider_tree` 只碰 7 行、1 505 行的 narrow phase 只碰 2 行。真正的工作量集中在：

- **146 条 `use bevy…` 的机械改写**（`bevy::ecs::X` → `kairos_ecs::X`、`bevy::transform::X` → `kairos_transform::X`、`bevy_math::Y` → 决策点，见下）；
- **`App`/`Plugin`/`PluginGroup` 层从无到有**：28 个 plugin 要么写一个 `App` shim（`World` + `Schedules` 已经够支撑 `add_systems`/`configure_sets`/`init_resource`/`add_observer`/`register_required_components`/`edit_schedule`/`get_schedule_mut`/`is_plugin_added`/`register_type`），要么把每个 plugin 改写成 `fn install(world: &mut World, schedule: impl ScheduleLabel)`（Kairos 已有这个范式，`kairos_physics/src/lib.rs:317`）。**「估算」：shim 约 300～600 行，能吸收 ~150 处 `app.*` 调用点中的绝大多数**；
- **10 个硬阻塞**（见 7.5 清单）。

**最小可行落地顺序与规模「估算」**（单工程师，含阅读、改写、调试；不含联调渲染）：

| 阶段 | 覆盖内容 | 规模「估算」 |
|---|---|---|
| 0. `App`/`Plugin` shim + `Deref`/`From` derive | 让 28 个 plugin 能编译 | 1～2 周 |
| 1. 数学层替身（`float3`↔`Vector`、`Mat3`、`SymmetricMat3`、`Dir`、`Ray`、shape primitives） | §4.4 的缺口表 | 2～3 周（含 fork `glam_matrix_extras`） |
| 2. `Time<T: Clock>` 泛型时钟 | `src/schedule/` 306 + 292 行 | 1～2 周 |
| 3. `kairos_transform` 传播系统 + `TransformHelper` | §6.2 | 1～2 周（**若已有层级，可能是 2～4 周**） |
| 4. Collider（`collision/collider/` 6 915 行，含 parry 桥）+ `ColliderAabb` + `ColliderTrees` | bevy 密度 0.5% | 2～3 周 |
| 5. RigidBody + mass properties + forces | 含 `bevy_heavy` 替身 | 2～3 周 |
| 6. integrator + broad phase + narrow phase + solver | bevy 密度 <0.4% | 3～5 周 |
| 7. joints / CCD / character controller / debug render | 可延后 | 另计 |

即 **到"colliders → rigid bodies → integration → collision detection 可用"约 10～18 周「估算」**（基于上面的分项相加，未做并行假设、未包含调试性能问题的时间）。

**5～10 个最难的具名阻塞**：

1. **`glam_matrix_extras` 钉死 glam 0.32**（`Cargo.lock:3101`；docs.rs `glam = "0.32"`）+ 73 处核心使用（`src/dynamics/rigid_body/mass_properties/components/computed.rs:431,468,491,520,549,565,…`、`mod.rs:12`）→ **必须 fork**（最新版就是 0.3.0，无升级路径）。
2. **`Time<C: Clock>` 泛型时钟缺失**（`src/schedule/time.rs:123,271`、`src/schedule/mod.rs:61-63,236-283`、`src/dynamics/solver/schedule.rs:20,199-205`）vs Kairos 的具体 `Time`（`kairos_time/src/lib.rs:36`）/`FixedTime`（`:178`）。
3. **Transform 传播系统缺失**（Avian `src/physics_transform/mod.rs:26,98-100`、`helper.rs:9,33`）vs `kairos_transform/src/lib.rs:17-26` 明写 "has not landed yet"。
4. **`bevy_transform_interpolation` 依赖 umbrella `bevy`**（`Cargo.lock:1756-1761`）→ 必须**从零重写**（组件 12 个 + 系统 3～5 个 + `VelocitySource` trait），且本地未 vendored、无源码可抄。
5. **`obvhs` 的 glam 版本错位**（Avian 锁 0.3.1，`glam <0.32`，`Cargo.lock:4424`）→ 必须升到 0.3.3；`src/collider_tree/` 2 944 行受影响（`obvhs_ext.rs:1,20-22,31`）。
6. **`Reflect` 层**（171 derive + 161 属性 + `ReflectMapEntities` ×5 in joints）vs `kairos_ecs/src/reflect.rs` 空文件 + 无 derive → 必须**删除或实现**。
7. **`App`/`Plugin`/`PluginGroup` 不存在**（28 `impl Plugin`、2 `impl PluginGroup`、~150 处 `app.*` 调用）。
8. **`bevy_math` shape primitives**（`src/collision/collider/parry/primitives3d.rs:1-4` 10 个 primitive）—— 但可绕过：直接构造 `parry::shape::SharedShape`（同文件 `:5` 已经在用）。
9. **`Deref`/`DerefMut`/`From` derive 缺失**（46 处 derive 行）—— 小但全局（`src/dynamics/solver/plugin.rs:199`、`src/dynamics/rigid_body/mod.rs:408`）。
10. **parry `0.27` → `0.28` 的 API 漂移**（Avian `Cargo.lock:4629-4636` vs Kairos `Cargo.lock:3634-3636`）—— 影响 `collision/collider/` 6 915 行里所有 `use parry::…` 调用点；`collision/collider/parry/contact_query.rs:19` 用了 `PersistentQueryDispatcher` 这类内部 trait。

### 7.2 路线 (b)：依赖替换 / 抽象层

**先问：能不能写一个 `bevy` 兼容 shim crate，让 Avian 原样编译？**

**不能。** 硬证据：

- Avian 的类型来自**具体 crate**：`#[derive(Component)]` 展开成 `impl bevy_ecs::component::Component`，`Query<'w,'s,D,F>` 是 `bevy_ecs::system::Query`，`SystemParam` 的 derive 也是 `bevy_ecs_macros` 的。Rust 没有"crate 级类型别名 + derive 重定向"机制 —— `pub use kairos_ecs as bevy_ecs;` 会让 `#[derive(Component)]` 依然找不到 `bevy_ecs_macros::Component`，而 `impl Trait for T` 的 `T` 会变成另一个 trait。**要给 Avian 换个 ECS，只能是源码级改名，不能是 crate 级别名。**

- 但**源码级改名在这个具体组合下是机械的**，这与一般情况不同，因为 `kairos_ecs` 是 `bevy_ecs` 0.19 的 fork、**同名同类**：`QueryData`、`SystemParam`、`QueryFilter`、`ReadOnlySystemParam`、`StaticSystemParam`、`SystemState`、`SystemChangeTick`、`Single`、`Populated`、`RelationshipTarget`、`Disabled`、`CheckChangeTicks`、lifetimeless `Read`/`Write`/`SQuery`/`SRes`/`SResMut` —— §6.1 逐条给了 `kairos_ecs/src/...:LINE`。

所以路线 (b) 的真实形态是：**保留 Avian 的架构与源码，把 `bevy_ecs` 面做 1:1 改名（机械），把 Kairos 缺的东西补齐（非机械）**。补齐清单就是 §7.1 的 10 条阻塞。其中**真正非机械的只有 4 条**：
- `Time<T: Clock)`（要设计时钟抽象）；
- Transform 传播（要写子系统）；
- `App`/`Plugin`（要设计应用层，或改写成 `install` 函数）；
- `bevy_heavy` / `bevy_transform_interpolation` / `glam_matrix_extras`（要写或 fork 数学层）。

**一个值得认真评估的变体（(b')）**：**把 `bevy_math` 当作纯数学依赖引入 Kairos 的 physics crate**。依据是 `Cargo.lock:1241-1250` —— `bevy_math 0.19.0` 的依赖是 `approx, arrayvec, bevy_reflect, derive_more, glam 0.32.1, itertools, libm, rand, rand_distr, serde, thiserror, variadics_please`，**没有 `bevy_ecs`**。采纳它意味着：
- ✅ Avian 的数学代码（`src/math/`、`SymmetricMat3`、`bevy_heavy`、shape primitives、`Dir`/`Ray`/`Isometry`）**几乎可以原样编译**；
- ✅ `glam_matrix_extras` 0.3.0 与 `bevy_heavy` 0.5.0 也都能直接用（它们本来就只依赖 bevy_math + glam 0.32 + bevy_reflect）；
- ❌ **代价 1**：`bevy_math` 依赖 `bevy_reflect`（`:1244`）→ reflect 层删不掉，得把 `kairos_ecs` 的 `kairos_reflect` 补起来；
- ❌ **代价 2**：**两套 glam 并存**（physics crate 内 0.32.1，引擎其余 0.33.1），每个 ECS 边界（`Position`/`Rotation` ↔ `LocalTransform`）都要转换。Kairos 的 `Cargo.lock` 已证明多 glam 共存可行（`:2974-2977`），但这会让 `parry` 也分叉：Avian 的 parry 是 0.27（glamx 0.2 → glam 0.32），Kairos 的是 0.28（glamx 0.3 → glam 0.33）—— **要么把 physics crate 也降到 parry 0.27，要么在 physics crate 内做 parry 版本转换**。后者实际不可行（parry 类型不能跨版本），前者可行但会让 `kairos_physics` 同时依赖 `parry3d 0.27` 与 `0.28`。
- **结论**：(b') 是一个**真实的、能大幅缩短周期 1～2（数学层与 glam_matrix_extras fork）的选项**，但把成本转嫁到"双 glam + 双 parry + reflect 必须实现"上。**「估算」净效果：省掉约 2～4 周数学工作，增加约 2～3 周 reflect + 边界转换 + 双版本维护**，并且**长期背上一个 Kairos 不想长期持有的 glam 0.32 分支**。除非 Kairos 愿意把主 glam 降到 0.32（那会波及 `kairos_engine` 的 `glam 0.33.1`，`Cargo.lock:2479`），否则不推荐。

### 7.3 路线 (c)：Learn-from-Avian，native 实现

**拿到什么**：
- **一份高质量的算法参照**，而且是**可读、bevy 密度极低**的那种：`src/dynamics/solver/`（8 623 行 / 30 行 bevy）、`src/collider_tree/`（2 944 / 7）、`src/collision/narrow_phase/`（1 505 / 2）、`src/collision/contact_types/`（1 860 / 4）、`src/dynamics/integrator/`（630 / 5）、`src/dynamics/ccd/`（770 / 2）、`src/data_structures/`（3 013 行，**其中 `graph.rs` 1 114 行与 `stable_graph.rs` 778 行完全不含 bevy**）。
- **参考实现的细粒度**：Avian 连注释里的参考来源都给了，例如 gyroscopic torque 的实现直接引 Erin Catto 的 GDC 2015 slides 与 Jolt Physics 的具体函数（`src/dynamics/integrator/mod.rs:396-401`）。
- **`parry` / `obvhs` 继续直接复用**：`parry3d 0.28`（Kairos 现状）可以完整承担 shape / contact / query；`obvhs 0.3.3` 支持 glam 0.33。**这两个库是碰撞检测的实质工作**，不需要重写。

**丢掉什么**：
- **求解器的成熟度**：Avian 的 solver 有 XPBD 子模块（`src/dynamics/solver/xpbd/` 2 561 行，含 `angular_constraint.rs` 296 行、`positional_constraint.rs` 99 行，**这两个文件完全不含 bevy**）、contact graph（`src/collision/contact_types/contact_graph.rs` 853 行）、island 睡眠启发式（`src/dynamics/solver/islands/` 2 036 行）、joint graph（674 行）、softness parameters（100 行）。自己写要重新踩这些坑。
- **确定性/精度资产**：`src/tests/determinism_2d.rs`（154 行）与 `enhanced-determinism` feature（`crates/avian3d/Cargo.toml:33-40`，链到 `libm` / `parry3d/enhanced-determinism` / `bevy_heavy/libm`）。
- **95 个组件 / 30 个资源 / 17 个系统集的既成数据模型**，以及"哪些组件是 required、哪些字段要 Reflect、哪些是 marker"这些经验。

**成本「估算」**：写一个"colliders → rigid bodies → semi-implicit Euler 积分 → broad phase + narrow phase"的最小闭环，**如果只做球/盒 + 简单接触**，大概 1～3 周；**做到 Avian 那样的形状覆盖（`Collider` 的全部枚举）+ 接触流形 + 迭代求解器 + 睡眠**，实际会需要**同路线 (a) 一样长的时间甚至更长**，因为你是在"读着 Avian 重写 Avian"。**（c）的价值不在"更省时间"，而在"不背 glam 0.32 / 不背 52k 行 fork / 数学栈统一"。**

### 7.4 许可证 / `unsafe` / `no_std`

**许可证：兼容。**
- Avian：`crates/avian3d/Cargo.toml:9` `license = "MIT OR Apache-2.0"`；`LICENSE-MIT:1-3`：
  ```
  MIT License
  Copyright (c) 2022 Jondolf
  ```
  `LICENSE-APACHE:1-4` 是标准 Apache-2.0 全文头。
- Kairos：`Cargo.toml:24` `license = "MIT OR Apache-2.0"`（`[workspace.package]` 段，所有成员 `license.workspace = true`）。
- **判定：兼容。** 双许可下接受方**择一**即可。若选 MIT：保留版权与许可声明。若选 Apache-2.0：保留 `NOTICE`（若有）并声明修改。
- **要做的家务**：
  1. Kairos 仓库根**没有 `LICENSE-MIT` / `LICENSE-APACHE` 文件**（`ls` 只看到 `AGENTS.md CONTEXT.md Cargo.lock Cargo.toml Library Preferences deny.toml docs kairos_* prototypes res rust-toolchain.toml target`）→ **vendor 之前必须补上**，否则 `Cargo.toml:24` 的声明是空头支票。
  2. vendor 进来的目录要有独立的 `LICENSE-MIT`/`LICENSE-APACHE` 与来源说明（commit `965e85bf…`）。
  3. Kairos 有 `deny.toml`（`deny.toml:1-3` 是 cargo-deny 的模板）→ 应把新的许可与来源加进 `[licenses]` 白名单检查（该文件是默认模板，`[licenses]` 段未看到实际配置，属待确认项）。

**`unsafe`**：Avian `src/` 里非注释 `unsafe` 行 **57** 处，集中在**不含 bevy 的容器层**：
- `src/data_structures/graph.rs:142,799,808,914,920`（`core::mem::transmute` 借出权重）
- `src/data_structures/sparse_secondary_map.rs:149,204,206,226,228,262,275,289,292,298,312,317`（`get_disjoint_unchecked_mut`、`MaybeUninit::uninit().assume_init()`、`unreachable_unchecked`）
- `src/data_structures/stable_vec.rs:176,177,191`
- 另有 `src/data_structures/bit_vec.rs`、`src/utils.rs` 等

**含义（对移植是好消息）**：这些 `unsafe` 都在**与 ECS 无关的容器**里（`grep` 出的 12 个 bevy-free 文件包含 `graph.rs` 1 114 行、`stable_vec.rs` 417 行、`id_pool.rs` 102 行），所以它们**可以整体搬运**，只需 `bevy::platform::collections::HashMap` → `hashbrown`/`std` 的一处替换（`src/data_structures/sparse_secondary_map.rs:13` `use bevy::platform::hash::RandomState;`）。

**`no_std`**：**Avian 不是 `no_std` crate。** 依据：`src/lib.rs` 里**没有 `#![no_std]`**；只有 `src/lib.rs:494` 的 `extern crate alloc;`；`src/data_structures/sparse_secondary_map.rs:8` 的注释写 "…`no_std` compatible"（只描述该模块的意图）。workspace 的 clippy lint 里配了 `alloc_instead_of_core`、`std_instead_of_alloc`、`std_instead_of_core`（`Cargo.toml:7-9`），说明作者在意，但 **`src/` 里大量使用 `std::` 与 `f32::sqrt` 等 `std`-only 路径**（例如 `src/math/mod.rs:12` `use approx::abs_diff_ne;` 与 `core::f32::consts` 混用、`src/utils.rs:3` 用 `bevy::platform::time::Instant`）。同时 `bevy` 依赖显式打开了 `"std"` feature（`crates/avian3d/Cargo.toml:83`），并启用了 `bevy_math/libm` 等仅在 `enhanced-determinism` 下的路径（`:33-40`）。

**对 Kairos 的含义**：Kairos 全栈是 `std`（各 crate 都用 `std::`，`kairos_time/src/lib.rs:21` `use std::time::{Duration, Instant};`），**`no_std` 不构成障碍**。反过来，若 Kairos 将来要 `no_std`，Avian 的 `src/` 不是现成答案。

### 7.5 推荐，以及不可逆点

**推荐：以 (c) 为骨架、(a) 为加速器 —— 即"按 Avian 的架构与数据模型，逐子系统移植它的算法代码，边移植边把胶水层换成 Kairos 原生"**，理由逐条对应证据：

1. **Kairos 侧的最大资产是 `kairos_ecs` 与 `bevy_ecs` 0.19 同名同类**（§6.1 逐条 `kairos_ecs/src/...:LINE`）。这意味着"按 Avian 写"和"vendor Avian"**不冲突**：你写出来的 `QueryData` 结构体、`SystemParam`、`SystemSet`、`#[require]`、observer 钩子，**最终能直接吃 Avian 的源码**。这是把 (c) 变成 (a) 的低摩擦路径，也是把"学习"变成"可兑现的资产"的关键。
2. **Avian 的算法层 bevy 密度极低**（solver 0.35%、collider_tree 0.24%、narrow phase 0.13%），所以"读着它写"的边际成本很低 —— 你可以**逐文件 transliterate**，把 `Vector`→`float3`、`Vec3`→`float3` 的替换限制在很薄的边界上。
3. **不要现在就付 `glam_matrix_extras` fork 与 `Time<Clock>` 的账**。§7.1 的阶段 1（数学层）与阶段 2（泛型时钟）**在"colliders → rigid bodies"阶段根本用不到**：
   - 角惯量张量（`SymmetricMat3`）只在 mass properties 的 3D 路径上；先落 2D/标量或者先用 `parry3d::mass_properties::MassProperties` 顶着；
   - `Time<Physics>`/`Time<Substeps>` 只在 solver 的子步循环里必需；先照 `kairos_physics` 现在的做法把 dt 直接传进系统（`kairos_physics/src/lib.rs:289-292`）。
4. **不要现在就 vendor 全部 52 307 行**。先落 `src/collision/{collider,collider_tree,broad_phase,narrow_phase,contact_types}` + `src/dynamics/{rigid_body,integrator}`（这些的 bevy 密度合计 <0.5%），`debug_render` / `picking` / `diagnostics/ui` / `character_controller` / `xpbd` / `joints` 全部延后。
5. **(b') —— 引入 `bevy_math` 作为纯数学依赖 —— 不建议**：它省掉数学层与 `glam_matrix_extras` fork，但把成本转成"双 glam（0.32.1 + 0.33.1）+ 双 parry（0.27 + 0.28）+ 必须实现 reflect"，而且要在每个 ECS 边界做 `float3` ↔ `Vec3` 转换。**唯一让它值得的情形**：Kairos 决定把主 glam 从 0.33 降到 0.32（波及 `kairos_engine` 的 `glam 0.33.1`，`Cargo.lock:2479`），那 (b') 会变成最省时的一条路。

**不可逆点（决策在哪儿变贵）**：

> **不可逆点 = 你为 `Collider` + `ColliderAabb` + `Position`/`Rotation` 定下"数据模型词汇"的那一刻 —— 具体是"colliders"阶段的第一次 commit，而不是 integration 或 solver 阶段。**

理由：
- **在此之前转向 (a) 是免费的**：还没有自己的 `Collider` 枚举、没有 `ColliderTree` 的 proxy key 布局、没有 `ColliderAabb` 的字段选择，vendor Avian 只是"拷文件 + 改 import"。
- **在此之后转向 (a) 的代价陡增**，因为这三样东西会被后面每一层引用：
  - `Collider`（Avian 是 715 行枚举 + 2 983 行 parry 桥）决定了 shape 的表示、`Scalar` 精度、`parry` 版本绑定；
  - `ColliderTrees`（`src/collider_tree/` 2 944 行）决定了 broad phase 的 proxy 布局（`obvhs` 的 BVH2 node、`IVector` key、`ColliderTreeSystems` 系统集），而 `obvhs` 的 glam 版本错位（§4.5 第 2 条）就在这一层；
  - `Position`/`Rotation`（`src/physics_transform/transform.rs`）决定了与 `LocalTransform`/`GlobalTransform` 的同步方向，而 `kairos_transform` 的传播系统缺失（§6.2）也正好在这一层结算。
  这三者一旦按 Kairos 自己的术语定型，后面 6 000+ 行的 `collision/collider`、2 000 行的 `physics_transform` 就**不能直接吃 Avian 的源码了**，只能重写 —— 那时 (a) 与 (c) 的成本曲线会交叉。
- **反过来，(c) → (a) 的方向是安全且廉价的**：只要你**始终抄 Avian 的组件名、系统集名、ScheduleLabel 名**（`Position`、`Rotation`、`Collider`、`ColliderAabb`、`ColliderTrees`、`PhysicsSchedule`、`SubstepSchedule`、`PhysicsSystems`、`PhysicsStepSystems`、`SolverSystems`…），那么任何时刻你都可以切到 (a)，因为改名成本已经提前付掉了。

**建议的落地顺序**（与上面的不可逆点对齐）：

1. **先写 `kairos_transform` 的传播系统 + `TransformHelper` 等价物**（§6.2）。这是所有后续步骤的前置，也是唯一一个"Kairos 侧本来就要做、且 Avian 无法提供"的子系统 —— 越早做越好，因为它会暴露 `LocalTransform`/`GlobalTransform` 是否是 Avian 能接受的形态。
2. **立 `kairos_physics_v2` crate，抄 Avian 的组件/系统集/ScheduleLabel 命名**，第一版只做：`Collider`（先 3～4 种形状，直接用 `parry3d 0.28` 的 `SharedShape`）+ `ColliderAabb` + `RigidBody` + `Position`/`Rotation` + `PhysicsSchedule`/`PhysicsStepSystems`。
3. **再抄 `src/collider_tree/` 与 `src/collision/broad_phase/bvh_broad_phase.rs`**，此时先把 `obvhs` 升到 0.3.3（glam 0.33 兼容）。
4. **然后抄 `src/dynamics/integrator/`**（630 行 / 5 行 bevy），用 `par_iter_mut()`（`kairos_ecs/src/system/query.rs:1329`）。
5. **最后抄 `src/collision/narrow_phase/` + `src/dynamics/solver/`**，此时才需要 `Time<T: Clock>` 与 `SymmetricMat3`，也才需要在 (a)/(b')/(c) 之间做最终决策。

---

## 8. 本文件未验证/不确定

以下是我**没有**做到或无法从现有材料确证的部分，逐条说明原因与补证方式：

1. **没有编译验证。** 全部结论是静态阅读的结果，未运行 `cargo build` / `cargo test` / `cargo metadata`。具体风险点：`bevy::ecs::…` → `kairos_ecs::…` 的改名在**类型推导**上是否真的一次通过（例如 `Query<'w,'s,D,F>` 的 `D`/`F` 边界、`StaticSystemParam<C::Context>` 的 `'static` 要求、`SystemState::new(app.world_mut())` 的参数形态），本文只做了名称层面的核对。**列在 §6.1 的每一条"✅"是"同名项存在"的证据，不是"能直接替换"的证明。**
2. **`bevy_heavy`、`bevy_transform_interpolation`、`obvhs`、`glam_matrix_extras`、`parry3d 0.27` 的源码本地均未 vendored。** 检查方式：`ls ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ | grep -iE 'obvhs|bevy_heavy|bevy_transform_interpolation|glam_matrix_extras|parry3d'` 只返回 `glamx-0.3.0`、`parry3d-0.28.0`（另有 `bevy-0.17.3` 系列，与 Avian 的 0.19 无关）。因此：
   - `bevy_heavy` 的**公开 API 形状**（`MassProperties3d` 的字段、`ComputeMassProperties3d::mass_properties` 的签名、`AngularInertiaTensor` 的方法集）我只能从 Avian 的使用点反推，**没有读它的源码**；
   - `bevy_transform_interpolation` 的**组件与系统清单**我只能从 `src/interpolation.rs:9-19` 的 16 个 re-export 反推，**组件字段与系统实现未验证**；
   - 两者的**依赖形状**来自 `Cargo.lock`（`bevy_heavy`：`Cargo.lock:1011-1019`；`bevy_transform_interpolation`：`Cargo.lock:1756-1761`）—— 这是可靠的（Cargo 解析结果），但**feature 层面的细节**（例如 `bevy_transform_interpolation` 具体打开了 `bevy` 的哪些 feature）无从得知。
3. **`obvhs 0.3.1/0.3.3` 与 `glam_matrix_extras 0.3.0` 的 `Cargo.toml` 来自 docs.rs 而非本地源码**：<https://docs.rs/crate/obvhs/0.3.1/source/Cargo.toml>（`glam = ">=0.30.10, <0.32"`）、<https://docs.rs/crate/obvhs/latest/source/Cargo.toml>（0.3.3，`glam = ">=0.30.10, <0.34"`）、<https://docs.rs/crate/glam_matrix_extras/latest/source/Cargo.toml>（0.3.0，`glam = "0.32"`、`bevy_reflect = "0.19"`）。这些是**外部内容**，虽然与本地 `Cargo.lock` 的解析结果（obvhs→glam 0.31.1、glam_matrix_extras→glam 0.32.1）互相印证，但未在本地复现。`glam_matrix_extras` 的源码规模（`src/lib.rs` 32 行 + 5 个模块）来自 docs.rs 目录列表，**未逐行核对**。
4. **`src/math/mod.rs` 的部分函数签名未逐行读全**（668 行中我只读了 `:1-120`、`:120-250`，并用 `grep -n 'pub fn \|pub trait \|...'` 抽了结构）。`MatExt` trait 的完整方法集（`from_diagonal`/`inverse`/`is_invertible` 的确切签名）**未逐条核对**。
5. **`src/collision/collider/` 与 `src/dynamics/` 的具体 API 面未逐个函数读**。§7.1 里"parry 0.27→0.28 API 漂移"这一条，我**没有对比两版 parry 的 API 差异**（本地只有 0.28，且不打算读 6 915 行去枚举调用点）。这条是**基于"主版本不同必然有 API 变化"的推断，不是实测**；严重程度未量化。
6. **`bevy::camera::visibility::VisibilitySystems` 的引用位置未定位。** 我在一次宽泛 grep 中看到 `camera::visibility::VisibilitySystems` 出现在某个 `use bevy::{…}` 块里（与 `src/debug_render/` 或 `src/collider_tree/` 相关），但**没有定位到具体 `file:line`**，因此 §2.2 表里 `bevy::camera` 一行标为"0（字面）"并注明"见 §8"。补证方式：`grep -rn 'VisibilitySystems' src`。
7. **`kairos_ecs` 与 `bevy_ecs` 0.19 的"等价性"没做行为验证。** 我说"同名同类"，依据是名字与签名形态（例如 `QueryData` 的 `#[query_data(mutable)]` 属性在 `kairos_ecs/macros/src/query_data.rs:135` 存在）。**语义差异未查**，例如：
   - `Query::par_iter_mut` 的调度语义（`kairos_ecs/src/system/query.rs:1329`）与 bevy 是否一致（Avian 依赖 `par_iter_mut` 的正确性，10 处核心使用）；
   - `Single` 在 kairos_ecs 是 `pub struct Single<'w,'s,D,F>`（`src/system/query.rs:2876`）——**是 struct 不是 enum**，与 bevy 0.19 的 `Single`（`Result` 语义）形态可能有差别（Avian 只在 `src/diagnostics/ui.rs:493` 用了一处，影响小）；
   - `try_register_required_components` 的返回类型（`kairos_ecs/src/world.rs:599`）与 Avian 在 `src/interpolation.rs:274-284` 的用法（`let _ = app.try_register_required_components::<…>();`）是否匹配。
8. **`kairos_engine` 的 `install` 与 `kairos_time` 是否有隐藏的泛型时钟实现未检查。** 我读了 `kairos_time/src/lib.rs`（342 行全文）与 `kairos_engine/src/kairos_editor/schedule.rs`（全文），但**没有遍历 `kairos_engine` 其余 ~100 个文件**去找"是否有人在别处实现了泛型时钟或 transform 传播"。`find kairos_engine/src -name '*.rs' | xargs grep -ln 'RunFixedMainLoop\|FixedUpdate'` 只命中 `kairos_editor/schedule.rs` 与 `kairos_editor.rs`，但这只覆盖了那一个关键词。
9. **Kairos 的 `deny.toml` 许可白名单未检查。** 我只确认该文件存在且前 80 行是 cargo-deny 的默认模板（`deny.toml:1-3`），**没有读到 `[licenses]` 段**。vendor Avian 前需确认白名单是否覆盖 `MIT OR Apache-2.0`。
10. **Kairos 仓库根是否存在 LICENSE 文本**：`ls` 输出中没有 `LICENSE*`，但可能有其他命名（如 `LICENCE`、`COPYING`）或在 `docs/` 下 —— 我只做了顶层 `ls`。`Cargo.toml:24` 的 `license = "MIT OR Apache-2.0"` 是确定的。
11. **"实体代码 ≈ 31 177 行"是上界。** 它没有扣除 `#[cfg(test)] mod tests { … }` 内联块（`grep -rl '#\[cfg(test)\]' src` 命中 17 个文件，但块内行数未统计），也没有扣除 `#![cfg_attr(feature = "2d", doc = "…")]` 这类多行文档属性（它们在计数口径里被当作"文档行"，但有些跨 1 行以上时可能被重复/漏算）。
12. **各阶段工期（1～2 周、10～18 周）纯粹是「估算」**，依据只是"§7.1 分项行数 × bevy 密度"这一条口径，**没有历史项目数据支撑**，也没有考虑联调、性能调优、与现有 `kairos_physics`（rapier 路线）的切换成本。请当作量级而非计划。
