# Avian Physics 0.7.0 架构与调度地图

> **来源与范围**
> - 权威源码：`/Users/baiaoxiang/KairosEngine/.scratch/avian-research/avian-0.7.0`（只读检出）
> - `git log -1`：commit `965e85bf590f53fbf8a29f7ebd0326b5dbc985d1`，tag `v0.7.0`，subject `Release v0.7.0 (#1005)`
> - 本文所有 `path/to/file.rs:LINE` 都相对于该检出根目录。
> - 只做静态源码阅读，未运行 `cargo build`/`cargo test`。
> - 标注约定：普通陈述 = 源码事实；以 「推论」 开头的句子 = 我基于源码的推断，未被源码直接声明。
> - **重要提醒**：本版本 Avian 的插件命名与旧版本/common blog 中的印象不同。源码里 **不存在** `PhysicsSetupPlugin`、`PhysicsTimePlugin`、`BroadPhasePlugin`、`IntegratorPlugin` 之外的 `SolverPlugin` 顶层挂载方式等。全部 `*Plugin` 的真实名字见第 3 节，均以源码为准。
>   验证：`grep -rn "PhysicsSetupPlugin\|PhysicsTimePlugin\|BroadPhasePlugin" src/` 只命中 `BroadPhaseCorePlugin` / `BvhBroadPhasePlugin`（`src/collision/broad_phase/mod.rs:156-157`、`src/collision/broad_phase/bvh_broad_phase.rs:31`），无 `PhysicsSetupPlugin` / `PhysicsTimePlugin`。

---

## 1. Workspace 与 crate 布局

### 1.1 顶层目录

```
Cargo.toml          # workspace 定义
src/                # 唯一的引擎源码树（128 个 .rs 文件，52307 行）
crates/
  avian2d/          # 只含 Cargo.toml + examples/ + assets/
  avian3d/          # 只含 Cargo.toml + examples/ + assets/ + benches/
  avian_derive/     # proc-macro 助手 crate
  examples_common_2d/
  examples_common_3d/
benches/            # 被 workspace exclude
assets/ migration-guides/ publish.sh README.md LICENSE-*
```

事实：

- `Cargo.toml:1-4`：
  ```toml
  [workspace]
  members = ["crates/avian2d", "crates/avian3d"]
  exclude = ["benches"]
  resolver = "2"
  ```
  → **`avian_derive`、`examples_common_2d/_3d` 不是 workspace member**，它们只作为 `path` 依赖被 `avian2d`/`avian3d` 引用（`crates/avian3d/Cargo.toml:81`、`crates/avian3d/Cargo.toml:105`）。
- `crates/avian_derive`：`src/lib.rs` 共 140 行，唯一内容来源；`crates/avian_derive/Cargo.toml` 19 行。它是 `#[derive(...)]` 宏的宿主，通过 `src/lib.rs:572` 的 `pub use avian_derive::*;` 从 `prelude` 再导出。
- `crates/examples_common_3d/src/lib.rs` 85 行、`crates/examples_common_2d/src/lib.rs` 85 行 —— 仅示例公共代码，不参与引擎逻辑。

### 1.2 「一份源码，两个 crate」是怎么成立的

关键在 `[lib] path`：

- `crates/avian3d/Cargo.toml:74-78`
  ```toml
  [lib]
  name = "avian3d"
  path = "../../src/lib.rs"
  required-features = ["3d"]
  bench = false
  ```
- `crates/avian2d/Cargo.toml:69-73`：同样 `path = "../../src/lib.rs"`，但 `required-features = ["2d"]`。

所以：

1. 两个 crate 各自是一个**独立的 `cargo` package**，但**共享同一个 crate root 文件** `src/lib.rs`，连同它 `mod` 出来的整棵 `src/` 树。
2. 编译哪个 crate，就决定了激活哪一组 feature。`avian2d` 默认激活 `2d`+`f32`+`parry-f32`（`crates/avian2d/Cargo.toml:15-25`），`avian3d` 默认激活 `3d`+`f32`+`parry-f32`（`crates/avian3d/Cargo.toml:15-25`）。
3. `src/` 内部全部用 `#[cfg(feature = "2d")]` / `#[cfg(feature = "3d")]` 做维度分叉，用 `#[cfg(feature = "f32")]` / `#[cfg(feature = "f64")]` 做精度分叉。规模：`src/` 中 `feature = "3d"` 出现 743 次、`feature = "2d"` 623 次、`feature = "f32"` 63 次、`feature = "f64"` 13 次（`grep -rn ... src/ | wc -l`）。
4. 互斥性由 `src/lib.rs` 顶部的 `compile_error!` 强制：
   - `src/lib.rs:464-465`：必须开 `f32` 或 `f64` 之一。
   - `src/lib.rs:467-468`：`f32` 与 `f64` 不能同时开。
   - `src/lib.rs:470-471`：必须开 `2d` 或 `3d` 之一。
   - `src/lib.rs:473-474`：`2d` 与 `3d` 不能同时开。
   - `src/lib.rs:476-492`：`default-collider` 在 `f32` 下要求 `parry-f32`，在 `f64` 下要求 `parry-f64`。
5. Parry 的别名注入：`src/lib.rs:496-506` 用 `pub extern crate parry3d as parry;`（以及 `parry3d_f64` / `parry2d` / `parry2d_f64`）把不同精度的 parry 统一成 `avian::parry`。引擎内部一律 `use parry::...`。
   - 因此“切精度”= 换一个 parry crate，而**不是**泛型化。这就是 `parry-f32`/`parry-f64` 必须与 `f32`/`f64` 分开的原因，Cargo.toml 里有注释说明：`crates/avian3d/Cargo.toml:42-43`「We unfortunately can't reuse the f32 and f64 features for this, because Parry uses separate crates for f32 and f64.」

维度/精度抽象层：`src/math/mod.rs`。

- `src/math/mod.rs:19-20`：`#[cfg(feature = "2d")] pub const DIM: usize = 2;` / `src/math/mod.rs:22-23`：3D 时 `DIM = 3`。
- `Vector` / `Scalar` / `Quaternion` 等别名在 `src/math/single.rs`（f32）与 `src/math/double.rs`（f64）中分维度定义，例如 `src/math/single.rs:6` `pub type Scalar = f32;`、`src/math/single.rs:18-21` `Vector = Vec2`（2D）/ `Vec3`（3D）、`src/math/single.rs:48` `Quaternion = Quat`；f64 版在 `src/math/double.rs:6`、`src/math/double.rs:18-21`。
- `AdjustPrecision` trait：`src/math/mod.rs:104` `fn adjust_precision(&self) -> Self::Adjusted;`，实现分散在 `src/math/single.rs:52-143` 与 `src/math/double.rs`，用于 `f32 ⇄ f64` 与 Bevy 的 `Transform`（只有 f32）之间转换。

「推论」对 Kairos 的直接含义：如果 Kairos 不做 2D/3D 同源，可以省掉 743+623 处 `cfg` 分叉，但代价是失去“3D 引擎顺带得到 2D”这一点。若只做 3D，则只需保留 `feature = "3d"` 侧分支，`src/math/mod.rs` 的 `DIM`/`Vector` 别名层仍然值得照搬，因为它把 `glam` 类型收敛到单一别名。

### 1.3 feature 矩阵（3D 相关）

以 `crates/avian3d/Cargo.toml` 为准，逐个列出任务要求核对的那批 feature：

| feature | 定义位置 | 展开/作用 |
| --- | --- | --- |
| `3d` | `crates/avian3d/Cargo.toml:26` | 空 feature，仅作 `cfg` 开关 |
| `f32` | `crates/avian3d/Cargo.toml:27` | 空 feature，选 `src/math/single.rs` |
| `f64` | `crates/avian3d/Cargo.toml:28` | 空 feature，选 `src/math/double.rs` |
| `parry-f32` | `crates/avian3d/Cargo.toml:44` | `["f32", "dep:parry3d", "default-collider"]` |
| `parry-f64` | `crates/avian3d/Cargo.toml:45` | `["f64", "dep:parry3d-f64", "default-collider"]` |
| `default-collider` | `crates/avian3d/Cargo.toml:41` | 空 feature；被两个 parry feature 自动带上 |
| `xpbd_joints` | `crates/avian3d/Cargo.toml:48` | 空 feature，门控 `src/dynamics/solver/xpbd`（`src/dynamics/solver/mod.rs:15-16`）与 `XpbdSolverPlugin`（`src/dynamics/solver/mod.rs:79-80`） |
| `parallel` | `crates/avian3d/Cargo.toml:32` | `["bevy/multi_threaded", "parry3d?/parallel", "parry3d-f64?/parallel"]` |
| `debug-plugin` | `crates/avian3d/Cargo.toml:30` | `["bevy/bevy_gizmos", "bevy/bevy_render"]`，门控 `src/debug_render`（`src/lib.rs:515-516`） |
| `collider-from-mesh` | `crates/avian3d/Cargo.toml:50` | `["bevy/bevy_mesh", "bevy/bevy_mikktspace", "3d"]`（**avian2d 无此 feature**） |
| `bevy_scene` | `crates/avian3d/Cargo.toml:51` | `["bevy/bevy_world_serialization"]`，用于 `ColliderConstructorHierarchy` 等场景加载（`src/collision/collider/backend.rs:14-17`、`src/collision/collider/backend.rs:331-333`） |
| `bevy_picking` | `crates/avian3d/Cargo.toml:52` | `["bevy/bevy_picking"]`，门控 `src/picking`（`src/lib.rs:522-523`） |
| `enhanced-determinism` | `crates/avian3d/Cargo.toml:33-39` | `["dep:libm", "bevy_math/libm", "bevy_heavy/libm", "parry3d?/enhanced-determinism", "parry3d-f64?/enhanced-determinism"]` |
| `simd` | `crates/avian3d/Cargo.toml:31` | `["parry3d?/simd-stable", "parry3d-f64?/simd-stable"]`。**注意：`src/` 里没有任何 `#[cfg(feature = "simd")]`（grep 计数 0）**，它纯粹转发给 parry |
| `serialize` | `crates/avian3d/Cargo.toml:53-63` | serde + bevy serialize + glam_matrix_extras/bevy_heavy/bevy_transform_interpolation/parry/smallvec/bitflags 的 serde。在 `src/` 中出现 300 次 |
| `validate` | `crates/avian3d/Cargo.toml:72` | 空 feature；`src/` 中出现 11 次，用于额外的正确性检查 |
| `bevy_diagnostic` | `crates/avian3d/Cargo.toml:66` | 空 feature；门控 `PhysicsDiagnosticsPlugin`（`src/diagnostics/mod.rs:104-105`） |
| `diagnostic_ui` | `crates/avian3d/Cargo.toml:69` | `["bevy_diagnostic", "bevy/bevy_ui"]`；门控 `PhysicsDiagnosticsUiPlugin`（`src/lib.rs:538-539`、`src/diagnostics/ui.rs:22`） |

`avian3d` 的 `default`（`crates/avian3d/Cargo.toml:15-25`）：
`["3d", "f32", "parry-f32", "debug-plugin", "xpbd_joints", "parallel", "collider-from-mesh", "bevy_scene", "bevy_picking"]`

「推论」**3D 仿真的最小 feature 集合**：

- 必需：`3d` + `f32`（或 `f64`）。缺任一会在 `src/lib.rs:464-474` 编译期报错。
- 若要有碰撞体与碰撞检测：再加 `parry-f32`（它会自动带上 `default-collider`，`crates/avian3d/Cargo.toml:44`）。`default-collider` 单独开而在 `f32` 下不开 `parry-f32` 会触发 `src/lib.rs:476-483` 的 `compile_error!`。
- 其余全部可选，且都在 `PhysicsPlugins::build` 里带 `#[cfg]` 条件（见第 3 节）。源码给出的“最小可用”示例是 `src/lib.rs:42`：
  ```toml
  avian3d = { version = "0.7", default-features = false, features = ["3d", "f64", "parry-f64", "xpbd_joints"] }
  ```
  即官方认为 **`3d` + 精度 + 对应 parry + `xpbd_joints`** 就是一个合理的精简集合。
- 不带 `default-collider` 时，`src/character_controller` 整个模块不编译（`src/lib.rs:508-512`），`NarrowPhasePlugin`/`ColliderBackendPlugin`/`ColliderTreePlugin`/`BvhBroadPhasePlugin` 的“检测”部分也不注册（第 3 节），此时只剩 integrator + solver 的动力学积分，没有任何接触。

---

## 2. `src/` 目录地图

总规模：128 个 `.rs` 文件、52307 行。

模块声明集中在 `src/lib.rs:512-530`：

- `src/lib.rs:508-512`：`character_controller`（门控 `default-collider && (parry-f32|parry-f64)`）
- `src/lib.rs:513`：`collider_tree`
- `src/lib.rs:514`：`collision`
- `src/lib.rs:515-516`：`debug_render`（门控 `debug-plugin`）
- `src/lib.rs:517`：`diagnostics`
- `src/lib.rs:518`：`dynamics`
- `src/lib.rs:519`：`interpolation`
- `src/lib.rs:520`：`math`
- `src/lib.rs:521`：`physics_transform`
- `src/lib.rs:522-523`：`picking`（门控 `bevy_picking`）
- `src/lib.rs:524`：`schedule`
- `src/lib.rs:525`：`spatial_query`
- `src/lib.rs:527`：`data_structures`
- `src/lib.rs:529-530`：`pub(crate) mod ancestor_marker;`（源码自带 TODO：「Where should this go?」）
- `src/lib.rs:575`：`mod utils;`
- `src/lib.rs:577-578`：`#[cfg(test)] mod tests;`

### 2.1 表格

「行数」= 该模块（或文件）下所有 `.rs` 的 `wc -l` 之和。

| 模块 / 文件 | 职责（一句话） | `Plugin` 所在文件 | 行数 |
| --- | --- | --- | --- |
| `src/lib.rs` | crate root：doc、feature 校验、`prelude`、`PhysicsPlugins` 插件组 | 本文件（`PhysicsPlugins:681`、`PhysicsPluginsWithHooks:794`） | 852 |
| `src/schedule/` | 物理调度与时间资源：`PhysicsSchedule`、`SubstepSchedule`、`PhysicsSystems`、`PhysicsStepSystems`、`Time<Physics>`、`Time<Substeps>`、`LastPhysicsTick` | `src/schedule/mod.rs:37`（`PhysicsSchedulePlugin`） | 608（`mod.rs` 316 + `time.rs` 292） |
| `src/physics_transform/` | 权威位姿 `Position`/`Rotation` 与 Bevy `Transform`/`GlobalTransform` 双向同步 | `src/physics_transform/mod.rs:48`（`PhysicsTransformPlugin`） | 1998 |
| `src/dynamics/` | 刚体、力、质量属性、积分器、约束求解器、关节、CCD、islands（最大模块） | 多个：`dynamics/solver/mod.rs:47`（`SolverPlugins`）、`dynamics/solver/schedule.rs:15`（`SolverSchedulePlugin`）、`dynamics/solver/solver_body/plugin.rs:38`（`SolverBodyPlugin`）、`dynamics/solver/plugin.rs:68`（`SolverPlugin`）、`dynamics/integrator/mod.rs:24`（`IntegratorPlugin`）、`dynamics/ccd/mod.rs:248`（`CcdPlugin`）、`dynamics/solver/islands/mod.rs:69`（`IslandPlugin`）、`dynamics/solver/islands/sleeping.rs:42`（`IslandSleepingPlugin`）、`dynamics/joints/mod.rs:243`（`JointPlugin`）、`dynamics/solver/joint_graph/plugin.rs:30`（`JointGraphPlugin<T>`）、`dynamics/rigid_body/mass_properties/mod.rs:256`（`MassPropertyPlugin`）、`dynamics/rigid_body/forces/plugin.rs:20`（`ForcePlugin`）、`dynamics/solver/xpbd/plugin.rs:19`（`XpbdSolverPlugin`） | 21764 |
| `src/collision/` | 宽相 / 窄相、`Collider`、`CollisionLayers`、碰撞事件、`ContactGraph`、`CollisionHooks` | 多个：`collision/collider/backend.rs:68`（`ColliderBackendPlugin<C>`）、`collision/collider/collider_hierarchy/plugin.rs:10`（`ColliderHierarchyPlugin`）、`collision/collider/collider_transform/plugin.rs:18`（`ColliderTransformPlugin`）、`collision/collider/cache.rs:8`（`ColliderCachePlugin`）、`collision/broad_phase/mod.rs:174`（`BroadPhaseCorePlugin`）、`collision/broad_phase/bvh_broad_phase.rs:31`（`BvhBroadPhasePlugin<H>`）、`collision/narrow_phase/mod.rs:59`（`NarrowPhasePlugin<C,H>`） | 11482 |
| `src/collider_tree/` | 用 BVH（`obvhs`）加速宽相与空间查询；按 dynamic/kinematic/static/standalone 分四棵树 | `collider_tree/mod.rs:49`（`ColliderTreePlugin<C>`）、`collider_tree/update.rs:39`（`ColliderTreeUpdatePlugin<C>`）、`collider_tree/optimization.rs:15`（`ColliderTreeOptimizationPlugin`） | 2944 |
| `src/spatial_query/` | `SpatialQuery` 系统参数、`RayCaster`、`ShapeCaster`、`SpatialQueryFilter` | `src/spatial_query/mod.rs:178`（`SpatialQueryPlugin`） | 2884 |
| `src/math/` | 维度/精度抽象：`Scalar`、`Vector`、`Quaternion`、`AdjustPrecision`、`DIM` | 无 | 960 |
| `src/data_structures/` | 引擎内部专用容器：`graph`、`stable_graph`、`stable_vec`、`sparse_secondary_map`、`bit_vec`、`id_pool`、`pair_key` | 无（`src/data_structures/mod.rs:1-11` 只列模块） | 3013 |
| `src/debug_render/` | gizmo 调试绘制（AABB、BVH、collider、contact、joint、raycast、island）；门控 `debug-plugin` | `src/debug_render/mod.rs:90`（`PhysicsDebugPlugin`） | 1621 |
| `src/diagnostics/` | 计时/计数诊断，写入 `DiagnosticsStore`；`ui.rs` 提供调试面板 | `src/diagnostics/mod.rs:105`（`PhysicsDiagnosticsPlugin`）、`src/diagnostics/ui.rs:22`（`PhysicsDiagnosticsUiPlugin`） | 1082 |
| `src/character_controller/` | `MoveAndSlide` 系统参数与速度投影工具（**不是**完整角色控制器，`src/lib.rs:371-375`） | 无 Plugin（`src/character_controller/mod.rs:1-12` 只有模块与 prelude） | 1610 |
| `src/picking/` | `bevy_picking` 后端；门控 `bevy_picking` | `src/picking/mod.rs:61`（`PhysicsPickingPlugin`） | 260 |
| `src/interpolation.rs` | 基于外部 crate `bevy_transform_interpolation` 的 `Transform` 插值/外推封装 | 本文件 `:183`（`PhysicsInterpolationPlugin`） | 382 |
| `src/ancestor_marker.rs` | `AncestorMarker<C>` 与 `AncestorMarkerPlugin<C>`：给「含有 C 的实体的祖先」打标记，用于剪枝 transform 传播 | 本文件 `:12`（`AncestorMarkerPlugin<C>`） | 393 |
| `src/utils.rs` | `par_for_each`（按 `parallel` feature 在串行/`ComputeTaskPool` 并行之间切换）、`Instant` 再导出 | 无 | 87 |
| `src/tests/` | `#[cfg(test)]` 单元测试与 2D 确定性测试 | 无 | 367 |

### 2.2 二级子模块一览（供检索用）

- `src/collision/`：`broad_phase/`（`mod.rs`, `bvh_broad_phase.rs`）、`collider/`（`mod.rs`, `backend.rs`, `cache.rs`, `constructor.rs`, `layers.rs`, `trimesh_builder.rs`, `collider_hierarchy/`, `collider_transform/`, `parry/`）、`collision_events.rs`、`contact_types/`、`hooks.rs`、`narrow_phase/`、`diagnostics.rs`。
- `src/dynamics/`：`ccd/`、`integrator/`、`joints/`（`fixed.rs`, `distance.rs`, `prismatic.rs`, `revolute.rs`, `spherical.rs`, `motor.rs`, `tests.rs`）、`rigid_body/`（`mod.rs`, `sleeping.rs`, `locked_axes.rs`, `physics_material.rs`, `world_query.rs`, `forces/`, `mass_properties/`）、`solver/`（`plugin.rs`, `schedule.rs`, `constraint_graph.rs`, `contact/`, `islands/`, `joint_graph/`, `softness_parameters/`, `solver_body/`, `xpbd/`）。
- `src/schedule/`：`mod.rs`（调度与集合）、`time.rs`（`Physics`、`Substeps`、`PhysicsTime` trait、`TimePrecisionAdjusted`）。
- `src/collider_tree/`：`mod.rs`, `tree.rs`, `update.rs`, `optimization.rs`, `obvhs_ext.rs`, `proxy_key.rs`, `traverse.rs`, `diagnostics.rs`。
- `src/physics_transform/`：`mod.rs`（Plugin + 同步系统）、`transform.rs`（`Position`/`Rotation` 定义 + `init_physics_transform`）、`helper.rs`（`PhysicsTransformHelper`）、`tests.rs`。

---

## 3. `PhysicsPlugins` 插件组与全部插件

### 3.1 类型与构造

- `src/lib.rs:681-684`：
  ```rust
  pub struct PhysicsPlugins {
      schedule: Interned<dyn ScheduleLabel>,
      length_unit: Scalar,
  }
  ```
- `PhysicsPlugins::new(schedule)`：`src/lib.rs:690-695`，默认 `length_unit = 1.0`。
- `PhysicsPlugins::default()`：`src/lib.rs:751-755` → `Self::new(FixedPostUpdate)`。**默认调度是 `FixedPostUpdate`。**
- `with_length_unit(unit: Scalar)`：`src/lib.rs:745-748`。
- `with_collision_hooks::<H>()`：`src/lib.rs:701-709`，返回 `PhysicsPluginsWithHooks<H>`（`src/lib.rs:794-797`）；后者在 `src/lib.rs:834-851` 里先 `self.plugins.build()`，再 `disable::<BvhBroadPhasePlugin>()` + `add(BvhBroadPhasePlugin::<H>::default())`，以及 `disable::<NarrowPhasePlugin<Collider>>()` + `add(NarrowPhasePlugin::<Collider, H>::default())`。
- 自定义调度示例在 doc 中：`src/lib.rs:674-679`（`PhysicsPlugins::new(PostUpdate)`）。

### 3.2 `PhysicsPlugins::build` 的精确插入顺序

`src/lib.rs:757-789`。**按代码书写顺序**（`cfg` 未满足时该项不存在）：

| # | 行号 | 插件 | 条件 |
| --- | --- | --- | --- |
| 1 | `src/lib.rs:760` | `PhysicsSchedulePlugin::new(self.schedule)` | 无条件 |
| 2 | `src/lib.rs:761` | `MassPropertyPlugin::new(self.schedule)` | 无条件 |
| 3 | `src/lib.rs:762` | `ForcePlugin` | 无条件 |
| 4 | `src/lib.rs:763` | `ColliderHierarchyPlugin` | 无条件 |
| 5 | `src/lib.rs:764` | `ColliderTransformPlugin::new(self.schedule)` | 无条件 |
| 6 | `src/lib.rs:766-767` | `ColliderCachePlugin` | `collider-from-mesh` + `default-collider` |
| 7 | `src/lib.rs:774` | `ColliderBackendPlugin::<Collider>::new(self.schedule)` | `default-collider` + (`parry-f32`\|`parry-f64`)（`src/lib.rs:769-772`） |
| 8 | `src/lib.rs:775` | `ColliderTreePlugin::<Collider>::default()` | 同上 |
| 9 | `src/lib.rs:776` | `NarrowPhasePlugin::<Collider>::default()` | 同上 |
| 10 | `src/lib.rs:779` | `add_group(SolverPlugins::new_with_length_unit(self.length_unit))` | 无条件 |
| 11 | `src/lib.rs:782` | `BroadPhaseCorePlugin` | 无条件 |
| 12 | `src/lib.rs:783` | `BvhBroadPhasePlugin::<()>::default()` | 无条件 |
| 13 | `src/lib.rs:784` | `JointPlugin` | 无条件 |
| 14 | `src/lib.rs:785` | `SpatialQueryPlugin::new(self.schedule)` | 无条件 |
| 15 | `src/lib.rs:786` | `PhysicsTransformPlugin::new(self.schedule)` | 无条件 |
| 16 | `src/lib.rs:787` | `PhysicsInterpolationPlugin::default()` | 无条件 |

**顺序有硬约束的两处**（源码里是 `expect(...)` 而非宽容处理）：

- `src/dynamics/solver/schedule.rs:27-29`：`SolverSchedulePlugin` 做 `app.get_schedule_mut(PhysicsSchedule).expect("add PhysicsSchedule first")`。→ `PhysicsSchedulePlugin`（#1）必须先于 `SolverPlugins`（#10）内部所有走这条路径的插件。
- `src/dynamics/solver/plugin.rs:121-124`：`SolverPlugin` 做 `app.get_schedule_mut(SubstepSchedule).expect("add SubstepSchedule first")`。→ `SolverSchedulePlugin` 必须先于 `SolverPlugin`；这一点由 `SolverPlugins` 内部顺序保证（见 3.3）。

「推论」其余插件的相对顺序对系统执行顺序基本无影响，因为 Avian 用显式 `SystemSet::chain()` 定义顺序，而不是依赖插件注册顺序。顺序真正影响的是：(a) 上面两个 `expect` 的成立；(b) `register_required_components` / `register_component_hooks` 的先后（先注册者胜出，后注册会被 `register_*` 覆盖或 panic；Avian 大量使用 `try_register_*` 来回避，例如 `src/lib.rs:764` 之后的 `ColliderBackendPlugin` 用 `try_register_required_components_with`，见 `src/collision/collider/backend.rs:97-104`）。

### 3.3 `SolverPlugins` 内部顺序

`src/dynamics/solver/mod.rs:61-83`：

| # | 行号 | 插件 | 条件 |
| --- | --- | --- | --- |
| 1 | `src/dynamics/solver/mod.rs:64` | `SolverBodyPlugin` | 无条件 |
| 2 | `src/dynamics/solver/mod.rs:65` | `SolverSchedulePlugin` | 无条件 |
| 3 | `src/dynamics/solver/mod.rs:66` | `IntegratorPlugin::default()` | 无条件（默认调度 = `SubstepSchedule`，`src/dynamics/integrator/mod.rs:39-43`） |
| 4 | `src/dynamics/solver/mod.rs:67` | `SolverPlugin::new_with_length_unit(self.length_unit)` | 无条件 |
| 5 | `src/dynamics/solver/mod.rs:68` | `CcdPlugin` | 无条件 |
| 6 | `src/dynamics/solver/mod.rs:69` | `IslandPlugin` | 无条件 |
| 7 | `src/dynamics/solver/mod.rs:70` | `IslandSleepingPlugin` | 无条件 |
| 8 | `src/dynamics/solver/mod.rs:71` | `JointGraphPlugin::<FixedJoint>::default()` | 无条件 |
| 9 | `src/dynamics/solver/mod.rs:72` | `JointGraphPlugin::<RevoluteJoint>::default()` | 无条件 |
| 10 | `src/dynamics/solver/mod.rs:73` | `JointGraphPlugin::<PrismaticJoint>::default()` | 无条件 |
| 11 | `src/dynamics/solver/mod.rs:74` | `JointGraphPlugin::<DistanceJoint>::default()` | 无条件 |
| 12 | `src/dynamics/solver/mod.rs:77` | `JointGraphPlugin::<SphericalJoint>::default()` | `3d` |
| 13 | `src/dynamics/solver/mod.rs:80` | `XpbdSolverPlugin` | `xpbd_joints` |

`JointPlugin` 又自己挂子插件：`src/dynamics/joints/mod.rs:247-254` → `fixed::plugin`、`distance::plugin`、`prismatic::plugin`、`revolute::plugin`、`spherical::plugin`（`3d`）。这些 `plugin` 是**普通 `fn(app: &mut App)`**，不是 `Plugin` 类型（例：`src/dynamics/joints/fixed.rs:247`），各自把 `update_local_frames` 放进 `JointSystems::PrepareLocalFrames`（`src/dynamics/joints/fixed.rs:248-251`）。

### 3.4 每个插件的系统注册明细

格式：`系统 → 目标 schedule / set`。

#### PhysicsSchedulePlugin（`src/schedule/mod.rs:37`，build 在 `:58-126`）

- 资源与类型：`register_type::<Time<Physics>>()`（`:61`）、`init_resource::<Time<Physics>>()`（`:63`）、`insert_resource(Time::new_with(Substeps))`（`:64`）、`init_resource::<SubstepCount>()`（`:65`）、`init_resource::<LastPhysicsTick>()`（`:66`）、`init_resource::<PhysicsLengthUnit>()`（`:69`）。
- 在宿主 schedule（默认 `FixedPostUpdate`）上配置 `PhysicsSystems` 五段链 + `.before(TransformSystems::Propagate)`（`:74-85`）。
- `edit_schedule(PhysicsSchedule, ...)`：设置 `SingleThreadedExecutor`、`ambiguity_detection: LogLevel::Error`，并配置 `PhysicsStepSystems` 七段链（`:88-108`）。
- `run_physics_schedule.in_set(PhysicsSystems::StepSimulation)` 加到宿主 schedule（`:110-113`）。
- `update_last_physics_tick.after(PhysicsStepSystems::Last)` 加到 `PhysicsSchedule`（`:115-118`）。
- `#[cfg(debug_assertions)] assert_components_finite.in_set(PhysicsSystems::First)`（`:120-124`）。

#### MassPropertyPlugin（`src/dynamics/rigid_body/mass_properties/mod.rs:256`，build `:277-329`）

- `register_required_components::<RigidBody, RecomputeMassProperties>()`（`:281`）。
- observer `On<Add, RigidBody>` → `mass_helper.update_mass_properties(...)`（`:284-288`）。
- observer `On<Insert, RigidBodyColliders>` → 同上（`:292-296`）。
- sets：`MassPropertySystems::{UpdateColliderMassProperties, QueueRecomputation, UpdateComputedMassProperties}` 链式，`.in_set(PhysicsSystems::Prepare).after(PhysicsTransformSystems::TransformToPosition)`（`:298-309`）。
- 系统：`queue_mass_recomputation_on_mass_change`、`queue_mass_recomputation_on_collider_mass_change` → `QueueRecomputation`（`:312-319`）；`update_mass_properties`、`warn_invalid_mass` → `UpdateComputedMassProperties`（`:322-327`）。

#### ForcePlugin（`src/dynamics/rigid_body/forces/plugin.rs:20`，build `:22-73`）

- `ForceSystems::ApplyConstantForces` → `PhysicsSchedule`，`in_set(IntegrationSystems::UpdateVelocityIncrements).before(integrator::pre_process_velocity_increments)`（`:28-30`）。
- `ForceSystems::Clear` → `PhysicsSchedule`，`in_set(SolverSystems::PostSubstep)`（`:31`）。
- `ForceSystems::ApplyLocalAcceleration` → `SubstepSchedule`，`in_set(IntegrationSystems::Velocity).before(integrator::integrate_velocities)`（`:36-38`）。
- 系统：8 个常量力/力矩/加速度系统链式 → `ApplyConstantForces`（`:42-58`，其中 `apply_constant_local_torques`、`apply_constant_local_angular_acceleration` 仅 3D）。
- `apply_local_acceleration` → `SubstepSchedule` / `ApplyLocalAcceleration`（`:62-65`）。
- `clear_accumulated_local_acceleration` → `PhysicsSchedule` / `ForceSystems::Clear`（`:68-71`）。

#### ColliderHierarchyPlugin（`src/collision/collider/collider_hierarchy/plugin.rs:10`）

- 无系统，只有 4 个 observer：`On<Add, (RigidBody, ColliderMarker)>` 插入 `ColliderOf`（`:15`）；`On<Remove, (RigidBody, ColliderMarker)>` 移除 `ColliderOf`（`:43-44`）；`on_collider_body_changed`（`:57`，实现 `:65` 起）；`on_body_removed`（`:60`，实现 `:119` 起）。

#### ColliderTransformPlugin（`src/collision/collider/collider_transform/plugin.rs:18`）

- 加子插件 `AncestorMarkerPlugin::<ColliderMarker>::default()`（`:46`）。
- `propagate_collider_transforms.in_set(PhysicsTransformSystems::Propagate)` → 宿主 schedule（默认 `FixedPostUpdate`，`:50-53`）。
- `update_child_collider_position` → `PhysicsSchedule` / `PhysicsStepSystems::First`（`:55-62`）。
- 定义 `ColliderTransform` 的传播算法（`:108` 起），是 `bevy_transform::propagate_transforms` 的克隆（注释 `:106-107`）。

#### ColliderCachePlugin（`src/collision/collider/cache.rs:8`）

- `init_resource::<ColliderCache>()`（`:12`）。
- `clear_unused_colliders` → **`PreUpdate`**（`:13`）。

#### ColliderBackendPlugin\<C\>（`src/collision/collider/backend.rs:68`，build `:94-249`）

- required components（用 `try_register_*`，允许被覆盖）：`C → Position::PLACEHOLDER`、`Rotation::PLACEHOLDER`（`:97-98`）、`ColliderMarker`、`ColliderAabb`、`EnlargedAabb`、`CollisionLayers`、`ColliderDensity`、`ColliderMassProperties`（`:99-104`）。
- 资源：`PhysicsTransformConfig`、`NarrowPhaseConfig`、`PhysicsLengthUnit`（`:107-109`）。
- 组件 hooks on `C`：
  - `on_add`：若实体没有 `RigidBody` 则 `init_physics_transform`（`:114-122`）。
  - `on_insert`：用 `GlobalTransform::scale()` 调 `set_scale`，并按 `ColliderDensity` 计算 `ColliderMassProperties`（`:123-160`）。
  - `on_remove`：移除 `ColliderMarker`，给 `ColliderOf.body` 插入 `RecomputeMassProperties`（`:164-187`）。
- observer：`On<Add, Sensor>`（`:190-208`）、`On<Remove, Sensor>`（`:211-226`）—— 更新质量属性。
- 系统：`update_collider_scale::<C>.in_set(PhysicsSystems::Prepare).after(PhysicsTransformSystems::TransformToPosition)` 与 `update_collider_mass_properties::<C>.in_set(MassPropertySystems::UpdateColliderMassProperties)`，**链式**（`:228-238`）。
- `#[cfg(feature = "default-collider")]`：`init_collider_constructors`、`init_collider_constructor_hierarchies` → **`Update`**（`:240-247`）。

#### ColliderTreePlugin\<C\>（`src/collider_tree/mod.rs:49`）

- required component：`C → ColliderTreeProxyKey`（`:60-63`，`try_register_required_components_with`）。
- 加子插件 `ColliderTreeUpdatePlugin::<C>::default()`（`:66`）与 `ColliderTreeOptimizationPlugin`（`:69-71`，只在 `!app.is_plugin_added::<ColliderTreeOptimizationPlugin>()` 时）。
- 资源：`ColliderTrees`、`MovedProxies`（`:74-75`）。
- sets（全在 `PhysicsSchedule`）：
  - `ColliderTreeSystems::UpdateAabbs` ∈ `PhysicsStepSystems::BroadPhase`，`.after(BroadPhaseSystems::First).before(BroadPhaseSystems::CollectCollisions)`（`:78-84`）。
  - `ColliderTreeSystems::BeginOptimize` ∈ `BroadPhaseSystems::Last`（`:85-88`）。
  - `ColliderTreeSystems::EndOptimize` ∈ `SolverSystems::Finalize`（`:89-93`）。
- `finish` 里 `register_physics_diagnostics::<ColliderTreeDiagnostics>()`（`:95-99`）。

子插件：

- `ColliderTreeUpdatePlugin<C>`（`src/collider_tree/update.rs:39`，build `:47`）：先 `init_resource::<MovedProxies>()`、`EnlargedProxies`、`LastDynamicKinematicAabbUpdate`（`:50-52`）；`update_moved_collider_aabbs::<C>` → `ColliderTreeSystems::UpdateAabbs`（`:56-63`）；`(clear_moved_proxies, update_solver_body_aabbs::<C>).chain()` → `PhysicsSchedule`，`.after(PhysicsStepSystems::Finalize).before(PhysicsStepSystems::Last)`（`:66-72`）；observer `On<Add, C>` 初始化 `ColliderAabb`/`EnlargedAabb`（`:75` 起）。
- `ColliderTreeOptimizationPlugin`（`src/collider_tree/optimization.rs:15`）：`optimize_trees` → `ColliderTreeSystems::BeginOptimize`；`block_on_optimize_trees` → `ColliderTreeSystems::EndOptimize`（`:22-28`，后者在 wasm/unknown 平台上不注册）。

#### BroadPhaseCorePlugin（`src/collision/broad_phase/mod.rs:174`）

- 资源：`ContactGraph`、`JointGraph`（`:178-179`）。
- sets：`BroadPhaseSystems::{First, CollectCollisions, Last}` 链式，`.in_set(PhysicsStepSystems::BroadPhase)`（`:181-190`）。
- `finish`：`register_physics_diagnostics::<CollisionDiagnostics>()`（`:193-196`）。
- **不实现任何算法**（`:167-168`）。

#### BvhBroadPhasePlugin\<H\>（`src/collision/broad_phase/bvh_broad_phase.rs:31`，build `:39-49`）

- `collect_collision_pairs::<H>.in_set(BroadPhaseSystems::CollectCollisions)` → `PhysicsSchedule`（`:44-47`）。使用 `ColliderTrees` + `MovedProxies` + `JointGraph` + `CollisionHooks`（签名见 `:51-59`）。

#### NarrowPhasePlugin\<C, H\>（`src/collision/narrow_phase/mod.rs:59`，build `:106-185`）

- 资源：`NarrowPhaseConfig`、`ContactGraph`、`ConstraintGraph`、`JointGraph`、`ContactStatusBits`、`DefaultFriction`、`DefaultRestitution`（`:109-115`）；`parallel` 下加 `ThreadLocalContactStatusBits`（`:117-118`）；`generate_constraints == true` 时 `init_resource::<ContactConstraints>()`（`:123-126`，默认 `true`，见 `:89-91`）。
- message：`add_message::<CollisionStart>()` / `<CollisionEnd>()`（`:120-121`）。
- sets：`NarrowPhaseSystems::{First, Update, Last}` 链式 `.in_set(PhysicsStepSystems::NarrowPhase)`（`:128-137`）；`CollisionEventSystems.in_set(PhysicsStepSystems::Finalize)`（`:138-141`）。
- 系统：`update_narrow_phase::<C, H>.in_set(NarrowPhaseSystems::Update).ambiguous_with_all()`（`:144-151`）。
- `!already_initialized` 时的一次性部分（`:153-182`，判定在 `:107`）：observer `remove_collider_on`、`on_add_sensor`/`on_remove_sensor`、`on_body_remove_rigid_body_disabled`/`on_disable_body`、`remove_body_on`，以及 `trigger_collision_events.in_set(CollisionEventSystems)`（`:174`）。
- `app.init_resource::<NarrowPhaseInitialized>()`（`:184`）；`NarrowPhaseInitialized` 定义在 `:100`。
- `fn finish` 在 `:187`：`register_physics_diagnostics::<CollisionDiagnostics>()`（`:189`）。
- `CollisionEventSystems` 定义在 `:198`；`NarrowPhaseConfig` 定义在 `:205`，`impl Default` 在 `:249`；`NarrowPhaseSystems` 枚举在 `:261`；废弃别名 `NarrowPhaseSet` 在 `:270-272`。

#### SolverBodyPlugin（`src/dynamics/solver/solver_body/plugin.rs:38`）

- `app.add_observer(on_insert_rigid_body)`（`:43`；实现 `:132-152`）：dynamic/kinematic → 插入 `(SolverBody::default(), SolverBodyInertia::default())`，static → 移除。
- 另外三个 observer 处理「从 disabled/sleeping 恢复」时补回 solver body（`:47-85`），以及 `On<Remove, RigidBody>` / `On<Add, (Disabled, RigidBodyDisabled, Sleeping)>` 时移除（`:87-100`）。
- `prepare_solver_bodies.chain().in_set(SolverSystems::PrepareSolverBodies)` → `PhysicsSchedule`（`:102-107`）。
- `writeback_solver_bodies.in_set(SolverSystems::Finalize)` → `PhysicsSchedule`（`:110-113`）。
- `#[cfg(feature = "3d")] update_solver_body_angular_inertia.in_set(IntegrationSystems::Position).after(integrate_positions)` → `SubstepSchedule`（`:117-123`）。
- `fn finish`（`:126`）：`register_physics_diagnostics::<SolverDiagnostics>()`（`:128`）。

#### SolverSchedulePlugin（`src/dynamics/solver/schedule.rs:15`，build `:17-72`）

- `register_type::<Time<Substeps>>()`（`:20`）；`insert_resource(Time::new_with(Substeps))`、`init_resource::<SubstepCount>()`（`:23-24`）。
- 在 `PhysicsSchedule` 上配置 `SolverSystems` 九段链 `.in_set(PhysicsStepSystems::Solver)`（`:32-46`）。
- `run_substep_schedule.in_set(SolverSystems::Substep)` → `PhysicsSchedule`（`:49`）。
- `edit_schedule(SubstepSchedule, ...)`：`SingleThreadedExecutor`、`ambiguity_detection: Error`，配置六段链 `IntegrationSystems::Velocity → SubstepSolverSystems::WarmStart → SolveConstraints → IntegrationSystems::Position → Relax → Damping`（`:52-70`）。

#### IntegratorPlugin（`src/dynamics/integrator/mod.rs:24`，build `:45-88`）

- `register_required_components::<SolverBody, VelocityIntegrationData>()`（`:48`）。
- `init_resource::<Gravity>()`（`:50`）。
- `PhysicsSchedule` sets：`IntegrationSystems::UpdateVelocityIncrements.in_set(SolverSystems::PreSubstep).before(IntegrationSystems::Velocity)`、`IntegrationSystems::ClearVelocityIncrements.in_set(SolverSystems::PostSubstep).after(IntegrationSystems::Velocity)`（`:52-62`）。
- `PhysicsSchedule` 系统：`pre_process_velocity_increments` → `UpdateVelocityIncrements`；`clear_velocity_increments` → `ClearVelocityIncrements`（`:64-71`）。
- 在 `self.schedule`（默认 `SubstepSchedule`）配置 `(IntegrationSystems::Velocity, IntegrationSystems::Position).chain()`（`:73-76`），并加 `(integrate_velocities, clamp_velocities).chain().in_set(Velocity)` 与 `integrate_positions.in_set(Position)`（`:78-86`）。

#### SolverPlugin（`src/dynamics/solver/plugin.rs:68`，build `:88-151`）

- 资源：`SolverConfig`、`ContactSoftnessCoefficients`、`ContactConstraints`、`ConstraintGraph`（`:90-93`）。
- `PhysicsLengthUnit` 覆盖逻辑：仅当不存在或为默认 `1.0` 时，才写入插件组传入的 `length_unit`（`:95-101`）。
- `PhysicsSchedule` 系统：`update_contact_softness.before(PhysicsStepSystems::NarrowPhase)`（`:108`）；`prepare_contact_constraints` → `SolverSystems::PrepareContactConstraints`（`:111-113`）；`solve_restitution` → `SolverSystems::Restitution`（`:116`）；`store_contact_impulses` → `SolverSystems::StoreContactImpulses`（`:119`）。
- `SubstepSchedule` 系统：`warm_start` → `SubstepSolverSystems::WarmStart`（`:129`）；`solve_contacts::<true>` → `SolveConstraints`（`:132`）；`solve_contacts::<false>` → `Relax`（`:136`）；`(joint_damping::<FixedJoint>, <RevoluteJoint>, [3d] <SphericalJoint>, <PrismaticJoint>, <DistanceJoint>).chain()` → `Damping`（`:139-150`）。
- `fn finish` 在 `:153`：`register_physics_diagnostics::<SolverDiagnostics>()`（`:155`）。

#### CcdPlugin（`src/dynamics/ccd/mod.rs:248`）

- `physics.configure_sets(SweptCcdSystems.after(SolverSystems::PostSubstep).before(SolverSystems::Restitution))`（`:257-262`）。
- `#[cfg(any(parry-f32, parry-f64))] physics.add_systems(solve_swept_ccd.in_set(SweptCcdSystems))`（`:264`）。

#### IslandPlugin（`src/dynamics/solver/islands/mod.rs:69`，build `:71-154`）

- `init_resource::<PhysicsIslands>()`（`:73`）；`register_required_components::<SolverBody, BodyIslandNode>()`（`:76`）。
- 一批 observer 管 `BodyIslandNode` 的增删（`:80-146`）。
- `split_island.in_set(SolverSystems::Finalize)` → `PhysicsSchedule`（`:150-153`）。

#### IslandSleepingPlugin（`src/dynamics/solver/islands/sleeping.rs:42`，build `:44-85`）

- 资源：`AwakeIslandBitVec`、`TimeToSleep`（`:46-47`）；三个缓存的 `SystemState` 资源（`:54-60`）。
- `register_required_components::<SolverBody, SleepThreshold>()`、`<SolverBody, SleepTimer>()`（`:50-51`）。
- 组件 hooks：`Sleeping` 的 `.on_add(sleep_on_add_sleeping).on_remove(wake_on_remove_sleeping)`（`:63-66`）。
- observer：`wake_on_replace_rigid_body`（`:68`）、`wake_on_enable_rigid_body`（`:69`）。
- 系统（链式）：`update_sleeping_states`、`wake_islands_with_sleeping_disabled`、`wake_on_changed`、`wake_all_islands.run_if(resource_changed::<Gravity>)`（`:77`）、`sleep_islands` → `PhysicsSchedule`，`.run_if(resource_exists::<PhysicsIslands>)`（`:81`）、`.in_set(PhysicsStepSystems::Sleeping)`（`:82`）；整块从 `:71` 开始。

#### JointPlugin（`src/dynamics/joints/mod.rs:243`）

- 子插件 `fixed::plugin`、`distance::plugin`、`prismatic::plugin`、`revolute::plugin`、`[3d] spherical::plugin`（`:247-254`）。
- `JointSystems::PrepareLocalFrames.after(SolverSystems::PrepareSolverBodies).before(SolverSystems::PrepareJoints)` → `PhysicsSchedule`（`:256-261`）。
- 每个子 `plugin` 加 `update_local_frames.in_set(JointSystems::PrepareLocalFrames)`（例：`src/dynamics/joints/fixed.rs:248-251`），查询过滤为 `Changed<FixedJoint>`（`src/dynamics/joints/fixed.rs:254`）。

#### JointGraphPlugin\<T\>（`src/dynamics/solver/joint_graph/plugin.rs:30`，build `:60-116`）

- `init_resource::<JointGraph>()`、`init_resource::<JointGraphPluginInitialized>()`（`:65-66`）；`JointGraphPluginInitialized` 定义在 `:57`。
- `register_required_components::<T, JointComponentId>()`（`:69`）。
- 组件 hooks `on_add(on_add_joint).on_remove(on_remove_joint)`（`:72-75`）。
- 6 处 observer 维护图（`:78`、`:83`、`:87`、`:90`、`:94`、`:108`；其中 `:85` 的 `if !already_initialized` 只做一次）。
- `on_change_joint_entities::<T>.in_set(PhysicsStepSystems::First).ambiguous_with(PhysicsStepSystems::First)`（`:110-115`）。

#### XpbdSolverPlugin（`src/dynamics/solver/xpbd/plugin.rs:19`，build `:21-`）

- 为 5 种关节注册 `*JointSolverData` required components（`:23-28`）。
- `SubstepSchedule` sets：`(XpbdSolverSystems::SolveConstraints, SolveUserConstraints, VelocityProjection).chain().after(SubstepSolverSystems::Relax).before(SubstepSolverSystems::Damping)`（`:31-41`）。
- `prepare_xpbd_joint::<T>`（5 种，链式）→ `SolverSystems::PrepareJoints`（`:44-56`）。
- motor 的 warm start 等（`:62-`）。

#### SpatialQueryPlugin（`src/spatial_query/mod.rs:178`，build `:199-225`）

- `configure_sets(self.schedule, SpatialQuerySystems.after(TransformSystems::Propagate))`（`:201-204`）—— 注意用的是 **宿主 schedule（默认 `FixedPostUpdate`）**，不是 `PhysicsSchedule`。
- 系统（链式）→ `SpatialQuerySystems`：`update_ray_caster_positions`、`(update_shape_caster_positions, raycast, shapecast).chain()`（`:206-218`，后者在 `default-collider && parry-*` 下）。
- `finish`：`register_physics_diagnostics::<SpatialQueryDiagnostics>()`（`:221-224`）。

#### PhysicsTransformPlugin（`src/physics_transform/mod.rs:48`，build `:69-125`）

详见第 5 节。

#### PhysicsInterpolationPlugin（`src/interpolation.rs:183`，build `:266-300`）

- 加 3 个外部插件：`TransformInterpolationPlugin::default()`、`TransformExtrapolationPlugin::<LinVelSource, AngVelSource>::default()`、`TransformHermiteEasingPlugin::<LinVelSource, AngVelSource>::default()`（`:268-272`）。
- `register_required_components::<TranslationHermiteEasing, PreviousLinearVelocity>()`、`<RotationHermiteEasing, PreviousAngularVelocity>()`（`:275-276`）。
- 若构造时开了 `*_all`，用 `try_register_required_components::<RigidBody, TranslationInterpolation/RotationInterpolation/TranslationExtrapolation/RotationExtrapolation>()`（`:279-292`）。
- `update_previous_velocity.in_set(PhysicsStepSystems::First)` → `PhysicsSchedule`（`:295-298`）。
- 构造器：`interpolate_all()`（`:195`）、`extrapolate_all()`（`:234`）等，字段定义 `:184-187`。

#### PhysicsDiagnosticsPlugin（`src/diagnostics/mod.rs:105`，门控 `bevy_diagnostic`）

- `PhysicsSchedule` sets：`PhysicsDiagnosticsSystems::Reset.before(PhysicsStepSystems::First)`、`WriteDiagnostics.after(PhysicsStepSystems::Last)`（`:110-117`）。
- 注意：真正的 `reset`/`write_diagnostics` 系统是由 `AppDiagnosticsExt::register_physics_diagnostics::<T>()`（`src/diagnostics/mod.rs:166-223`）在各插件 `finish()` 里注册的；`register_physics_diagnostics` 还会在 `PhysicsDiagnosticsPlugin` 不存在时只注册 reset（`:175-191`）。

#### PhysicsDiagnosticsUiPlugin（`src/diagnostics/ui.rs:22`）

- `init_resource::<PhysicsDiagnosticsUiSettings>()`（`:26`）；`setup_diagnostics_ui` → `Startup`；一组 UI 更新系统链式 → `Update`，整体 `.run_if(diagnostics_are_enabled)`（`:28-42`）。

#### PhysicsDebugPlugin（`src/debug_render/mod.rs:90`，门控 `debug-plugin`）

- `init_gizmo_group::<PhysicsGizmos>()`（`:94`）。
- 一批 `debug_render_*` 系统 → **`PostUpdate`**，`.after(TransformSystems::Propagate)`，`.run_if(gizmo group enabled)`（`:107-136`）。
- `change_mesh_visibility.before(VisibilitySystems::CalculateBounds)` → `PostUpdate`（`:137-139`）。

#### PhysicsPickingPlugin（`src/picking/mod.rs:61`，门控 `bevy_picking`）

- `init_resource::<PhysicsPickingSettings>()`；`update_hits.in_set(PickingSystems::Backend)` → **`PreUpdate`**（`:65-66`）。
- `finish`：`register_physics_diagnostics::<PhysicsPickingDiagnostics>()`（`:69-72`）。

---

## 4. 固定步长调度结构（核心）

### 4.1 调度标签类型

| 类型 | 定义位置 | 说明 |
| --- | --- | --- |
| `PhysicsSchedule` | `src/schedule/mod.rs:140-141` | `#[derive(Debug, Hash, PartialEq, Eq, Clone, ScheduleLabel)] pub struct PhysicsSchedule;` —— **不是** enum，而是一个 unit struct 的 `ScheduleLabel` |
| `SubstepSchedule` | `src/dynamics/solver/schedule.rs:76-77` | `ScheduleLabel`，跑子步循环 |
| `Physics` | `src/schedule/time.rs:123-126` | **不是 schedule label**，而是 `Time<Physics>` 的时钟 mark 类型（`paused` + `relative_speed` 字段） |
| `Substeps` | `src/schedule/time.rs:271` | `Time<Substeps>` 的时钟 mark 类型 |
| `PhysicsSystems` | `src/schedule/mod.rs:161-176` | 五段高层 set |
| `PhysicsStepSystems` | `src/schedule/mod.rs:191-214` | 七段步内 set |

废弃别名（仍导出）：`PhysicsSet`（`src/schedule/mod.rs:178-180`）、`PhysicsStepSet`（`:216-218`）、`SolverSet`（`src/dynamics/solver/schedule.rs:119-121`）、`SubstepSolverSet`（`:151-153`）、`IntegrationSet`（`src/dynamics/integrator/mod.rs:113-115`）、`NarrowPhaseSet`（`src/collision/narrow_phase/mod.rs:270-272`）、`PhysicsTransformSet`（`src/physics_transform/mod.rs:178-180`）、`SweptCcdSet`（`src/dynamics/ccd/mod.rs:266-268`）。

`PhysicsSchedule` 的创建与配置在 `PhysicsSchedulePlugin`（`src/schedule/mod.rs:88-108`）：`edit_schedule(PhysicsSchedule, ...)` 同时设置 `SingleThreadedExecutor`（`:90`）和 `ambiguity_detection: LogLevel::Error`（`:91-94`）。`SubstepSchedule` 同理在 `SolverSchedulePlugin`（`src/dynamics/solver/schedule.rs:52-70`）。

「推论」`ambiguity_detection: LogLevel::Error` 意味着：任何未显式排序、但在同一 set 内访问冲突的系统，在**调度被构建时**就会以 `Error` 级别记录/触发歧义告警。这是 Avian 强制自己把顺序写清楚的手段，Kairos 若照搬需注意自己的系统会有大量告警。

### 4.2 `PhysicsStepSystems` 声明顺序与 chain

声明顺序（`src/schedule/mod.rs:191-214`）：

1. `First`（`:194`）
2. `BroadPhase`（`:199`）
3. `NarrowPhase`（`:203`）
4. `Solver`（`:207`）
5. `Sleeping`（`:209`）
6. `Finalize`（`:211`）
7. `Last`（`:213`）

排序配置（`src/schedule/mod.rs:96-107`）：

```rust
schedule.configure_sets(
    (
        PhysicsStepSystems::First,
        PhysicsStepSystems::BroadPhase,
        PhysicsStepSystems::NarrowPhase,
        PhysicsStepSystems::Solver,
        PhysicsStepSystems::Sleeping,
        PhysicsStepSystems::Finalize,
        PhysicsStepSystems::Last,
    )
        .chain(),
);
```

**`Sleeping` 在 `Solver` 之后**（不是之前）。

### 4.3 外层：`PhysicsSystems`（宿主 schedule 内）

声明顺序（`src/schedule/mod.rs:161-176`）：`First`（`:164`）→ `Prepare`（`:167`）→ `StepSimulation`（`:170`）→ `Writeback`（`:173`）→ `Last`（`:175`）。

排序配置（`src/schedule/mod.rs:74-85`）：

```rust
app.configure_sets(
    schedule,  // 默认 FixedPostUpdate
    (
        PhysicsSystems::First,
        PhysicsSystems::Prepare,
        PhysicsSystems::StepSimulation,
        PhysicsSystems::Writeback,
        PhysicsSystems::Last,
    )
        .chain()
        .before(TransformSystems::Propagate),
);
```

关键点：整条链**在 Bevy 的 `TransformSystems::Propagate` 之前**（`:84`），这样 `Position → Transform` 的写回发生在 Bevy 把 `Transform` 传播成 `GlobalTransform` 之前。

### 4.4 每段的实际内容（按执行先后汇总）

#### 宿主 schedule（默认 `FixedPostUpdate`）

1. **`PhysicsSystems::First`**
   - `assert_components_finite`（仅 `debug_assertions`）—— `src/schedule/mod.rs:120-124`，实现 `:290-316`，检查 `Position`/`LinearVelocity`/`AngularVelocity` 的 NaN/Inf。
2. **`PhysicsSystems::Prepare`**
   - `PhysicsTransformSystems::Propagate` → `PhysicsTransformSystems::TransformToPosition`（链式，`src/physics_transform/mod.rs:86-94`）
     - `Propagate` 内：`mark_dirty_trees` → `propagate_parent_transforms` → `sync_simple_transforms`，`.run_if(config.propagate_before_physics)`（`src/physics_transform/mod.rs:95-105`）；另外 `propagate_collider_transforms` 也挂在这个 set（`src/collision/collider/collider_transform/plugin.rs:50-53`）
     - `TransformToPosition`：`transform_to_position`，`.run_if(config.transform_to_position)`（`src/physics_transform/mod.rs:106-111`）
   - `PhysicsSystems::Prepare` 内还有：`MassPropertySystems` 三段链，`.after(PhysicsTransformSystems::TransformToPosition)`（`src/dynamics/rigid_body/mass_properties/mod.rs:298-309`）；`update_collider_scale::<C>` 同样 `.after(PhysicsTransformSystems::TransformToPosition)`（`src/collision/collider/backend.rs:231-233`）。
3. **`PhysicsSystems::StepSimulation`**
   - 只有一个系统：`run_physics_schedule`（`src/schedule/mod.rs:110-113`，实现 `:235-279`）。它内部 `try_schedule_scope(PhysicsSchedule, ...)` 跑整个物理步。
4. **`PhysicsSystems::Writeback`**
   - `PhysicsTransformSystems::PositionToTransform` → `position_to_transform`，`.run_if(config.position_to_transform)`（`src/physics_transform/mod.rs:114-123`）。
5. **`PhysicsSystems::Last`** —— 引擎默认为空（doc `src/schedule/mod.rs:174-175`）。

#### `PhysicsSchedule` 内部（第 4.5 节给出完整树）

#### 宿主 schedule 上但不在 `PhysicsSystems` 链内的

- `SpatialQuerySystems.after(TransformSystems::Propagate)`（`src/spatial_query/mod.rs:201-204`）。
- `update_last_physics_tick` 在 `PhysicsSchedule` 内（`src/schedule/mod.rs:115-118`）。
- `ColliderCachePlugin::clear_unused_colliders` → `PreUpdate`（`src/collision/collider/cache.rs:13`）。
- `init_collider_constructors` / `init_collider_constructor_hierarchies` → `Update`（`src/collision/collider/backend.rs:240-247`）。
- 调试/诊断/拾取见 3.4。

### 4.5 固定步长系统顺序（读者应期待的完整顺序）

```
宿主 schedule（默认 FixedPostUpdate）
│
├─ PhysicsSystems::First
│    └─ [debug] assert_components_finite
│
├─ PhysicsSystems::Prepare
│    ├─ PhysicsTransformSystems::Propagate
│    │    ├─ mark_dirty_trees → propagate_parent_transforms → sync_simple_transforms
│    │    └─ propagate_collider_transforms
│    ├─ PhysicsTransformSystems::TransformToPosition → transform_to_position
│    ├─ MassPropertySystems::UpdateColliderMassProperties
│    │    └─ update_collider_mass_properties::<C>
│    ├─ update_collider_scale::<C>
│    └─ MassPropertySystems::QueueRecomputation → UpdateComputedMassProperties
│
├─ PhysicsSystems::StepSimulation
│    └─ run_physics_schedule  ─────────────►  PhysicsSchedule 开始
│                                              │
│       ┌──────────────────────────────────────┴───────────────────────────────┐
│       │ PhysicsSchedule 内部                                                       │
│       │  PhysicsStepSystems::First                                                │
│       │    ├─ update_child_collider_position        (collider_transform/plugin)   │
│       │    ├─ update_previous_velocity              (interpolation)               │
│       │    └─ on_change_joint_entities::<T>         (joint_graph)                 │
│       │                                                                           │
│       │  PhysicsStepSystems::BroadPhase                                           │
│       │    ├─ BroadPhaseSystems::First                                            │
│       │    ├─ ColliderTreeSystems::UpdateAabbs   (after First, before Collect)     │
│       │    ├─ BroadPhaseSystems::CollectCollisions                                │
│       │    │    └─ collect_collision_pairs::<H>                                   │
│       │    └─ BroadPhaseSystems::Last                                             │
│       │         └─ ColliderTreeSystems::BeginOptimize → optimize_trees            │
│       │                                                                           │
│       │  PhysicsStepSystems::NarrowPhase                                          │
│       │    ├─ (before NarrowPhase) update_contact_softness                        │
│       │    ├─ NarrowPhaseSystems::First                                           │
│       │    ├─ NarrowPhaseSystems::Update → update_narrow_phase::<C, H>            │
│       │    └─ NarrowPhaseSystems::Last                                            │
│       │                                                                           │
│       │  PhysicsStepSystems::Solver                                               │
│       │    SolverSystems::PrepareSolverBodies  → prepare_solver_bodies            │
│       │    JointSystems::PrepareLocalFrames    → update_local_frames::<Joint>     │
│       │    SolverSystems::PrepareJoints        → prepare_xpbd_joint::<Joint>      │
│       │    SolverSystems::PrepareContactConstraints → prepare_contact_constraints │
│       │    SolverSystems::PreSubstep                                              │
│       │      ├─ ForceSystems::ApplyConstantForces                                 │
│       │      │    └─ 8 个 apply_constant_* 链式                                   │
│       │      └─ IntegrationSystems::UpdateVelocityIncrements                      │
│       │           └─ pre_process_velocity_increments                              │
│       │    SolverSystems::Substep → run_substep_schedule                          │
│       │      └─ for i in 0..SubstepCount { SubstepSchedule }  ← 见 4.6            │
│       │    SolverSystems::PostSubstep                                             │
│       │      ├─ (after Velocity) IntegrationSystems::ClearVelocityIncrements      │
│       │      │    └─ clear_velocity_increments                                    │
│       │      └─ ForceSystems::Clear → clear_accumulated_local_acceleration        │
│       │    SweptCcdSystems → solve_swept_ccd   (after PostSubstep, before Rest.)  │
│       │    SolverSystems::Restitution → solve_restitution                         │
│       │    SolverSystems::Finalize                                                │
│       │      ├─ writeback_solver_bodies                                           │
│       │      ├─ split_island                                                      │
│       │      └─ ColliderTreeSystems::EndOptimize → block_on_optimize_trees        │
│       │    SolverSystems::StoreContactImpulses → store_contact_impulses           │
│       │                                                                           │
│       │  PhysicsStepSystems::Sleeping                                             │
│       │    ├─ update_sleeping_states                                              │
│       │    ├─ wake_islands_with_sleeping_disabled                                 │
│       │    ├─ wake_on_changed                                                     │
│       │    ├─ wake_all_islands              (.run_if(resource_changed::<Gravity>))│
│       │    └─ sleep_islands                                                       │
│       │      （整体 .run_if(resource_exists::<PhysicsIslands>)）                   │
│       │                                                                           │
│       │  PhysicsStepSystems::Finalize                                             │
│       │    └─ CollisionEventSystems → trigger_collision_events                    │
│       │                                                                           │
│       │  PhysicsStepSystems::Last                                                 │
│       │    └─ (after Finalize, before Last) clear_moved_proxies →                 │
│       │                                      update_solver_body_aabbs::<C>        │
│       │    └─ (after Last) update_last_physics_tick                               │
│       └───────────────────────────────────────────────────────────────────────────┘
│
└─ PhysicsSystems::Writeback
     └─ PhysicsTransformSystems::PositionToTransform → position_to_transform

（然后 Bevy 的 TransformSystems::Propagate 才运行 —— 见 src/schedule/mod.rs:84）
```

引用来源逐条：
- `PhysicsStepSystems` 链：`src/schedule/mod.rs:96-107`。
- `PhysicsStepSystems::First` 内容：`src/collision/collider/collider_transform/plugin.rs:61-62`、`src/interpolation.rs:295-298`、`src/dynamics/solver/joint_graph/plugin.rs:110-115`。
- `BroadPhaseSystems` 链：`src/collision/broad_phase/mod.rs:181-190`；`ColliderTreeSystems::UpdateAabbs` 位置：`src/collider_tree/mod.rs:78-84`；`BeginOptimize`：`src/collider_tree/mod.rs:85-88` + `src/collider_tree/optimization.rs:22-28`。
- `NarrowPhaseSystems` 链：`src/collision/narrow_phase/mod.rs:128-137`；`update_contact_softness.before(PhysicsStepSystems::NarrowPhase)`：`src/dynamics/solver/plugin.rs:108`。
- `SolverSystems` 链：`src/dynamics/solver/schedule.rs:32-46`。
- `JointSystems::PrepareLocalFrames`：`src/dynamics/joints/mod.rs:256-261`。
- `IntegrationSystems::UpdateVelocityIncrements` / `ClearVelocityIncrements`：`src/dynamics/integrator/mod.rs:52-62`；`ForceSystems::ApplyConstantForces` 在其内 `.before(pre_process_velocity_increments)`：`src/dynamics/rigid_body/forces/plugin.rs:28-30`；`ForceSystems::Clear` ∈ `PostSubstep`：`src/dynamics/rigid_body/forces/plugin.rs:31`。
- `SweptCcdSystems`：`src/dynamics/ccd/mod.rs:257-262`。
- `writeback_solver_bodies` ∈ `Finalize`：`src/dynamics/solver/solver_body/plugin.rs:110-113`；`split_island` ∈ `Finalize`：`src/dynamics/solver/islands/mod.rs:150-153`；`EndOptimize` ∈ `Finalize`：`src/collider_tree/mod.rs:89-93`。
- `Sleeping` 集合体：`src/dynamics/solver/islands/sleeping.rs:71-82`。
- `CollisionEventSystems` ∈ `Finalize`：`src/collision/narrow_phase/mod.rs:138-141`。
- `clear_moved_proxies`/`update_solver_body_aabbs`：`src/collider_tree/update.rs:66-72`。
- `update_last_physics_tick`：`src/schedule/mod.rs:115-118`。

### 4.6 `SubstepCount` 与子步循环

- 资源定义：`src/dynamics/solver/schedule.rs:185-191`，`pub struct SubstepCount(pub u32)`，`Default` 为 **`Self(6)`**（`:187-191`；doc 也写「The default substep count is currently 6」，`:159`）。
- 初始化发生在两处：`src/schedule/mod.rs:65` 和 `src/dynamics/solver/schedule.rs:24`（都是 `init_resource`，后到的不覆盖）。
- 循环系统：`run_substep_schedule`，注册于 `SolverSystems::Substep`（`src/dynamics/solver/schedule.rs:49`），实现在 `:193-213`：
  ```rust
  fn run_substep_schedule(world: &mut World) {
      let delta = world.resource::<Time<Physics>>().delta();
      let SubstepCount(substeps) = *world.resource::<SubstepCount>();
      let sub_delta = delta.div_f64(substeps as f64);

      let mut sub_delta_time = world.resource_mut::<Time<Substeps>>();
      sub_delta_time.advance_by(sub_delta);

      let _ = world.try_schedule_scope(SubstepSchedule, |world, schedule| {
          for i in 0..substeps {
              trace!("running SubstepSchedule: {i}");
              *world.resource_mut::<Time>() = world.resource::<Time<Substeps>>().as_generic();
              schedule.run(world);
          }
      });

      *world.resource_mut::<Time>() = world.resource::<Time<Physics>>().as_generic();
  }
  ```
  要点：**子步循环是一个独占世界的 exclusive system**（`fn(world: &mut World)`），它把泛型 `Time` 切成 `Time<Substeps>` 后**重复运行 `SubstepSchedule` `substeps` 次**，循环体内**不**再次 `advance`，所以每个子步看到相同的 `delta`。结束后把泛型 `Time` 还原为 `Time<Physics>`。

- `SubstepSchedule` 内部顺序（`src/dynamics/solver/schedule.rs:59-69`）：
  `IntegrationSystems::Velocity` → `SubstepSolverSystems::WarmStart` → `SubstepSolverSystems::SolveConstraints` → `IntegrationSystems::Position` → `SubstepSolverSystems::Relax` → `SubstepSolverSystems::Damping`。
  内容：
  - `Velocity`：`(integrate_velocities, clamp_velocities).chain()`（`src/dynamics/integrator/mod.rs:81-83`）；`ForceSystems::ApplyLocalAcceleration` 在其中 `.before(integrate_velocities)`（`src/dynamics/rigid_body/forces/plugin.rs:36-38`）。
  - `WarmStart`：`warm_start`（`src/dynamics/solver/plugin.rs:129`）；XPBD motor warm start（`src/dynamics/solver/xpbd/plugin.rs:62-`）。
  - `SolveConstraints`：`solve_contacts::<true>`（`src/dynamics/solver/plugin.rs:132`）；XPBD `SolveConstraints`（`src/dynamics/solver/xpbd/plugin.rs:31-41`）。
  - `Position`：`integrate_positions`（`src/dynamics/integrator/mod.rs:84`）；3D 下 `update_solver_body_angular_inertia.after(integrate_positions)`（`src/dynamics/solver/solver_body/plugin.rs:117-123`）。
  - `Relax`：`solve_contacts::<false>`（`src/dynamics/solver/plugin.rs:136`）。
  - `Damping`：5 个 `joint_damping::<T>` 链式（`src/dynamics/solver/plugin.rs:139-150`）。
  - XPBD 额外段：`XpbdSolverSystems::{SolveConstraints, SolveUserConstraints, VelocityProjection}` 链式，`.after(Relax).before(Damping)`（`src/dynamics/solver/xpbd/plugin.rs:31-41`）。

**「推论」`Time<Substeps>` 的 `elapsed` 每物理步推进 2 次 `sub_delta`**：`src/schedule/mod.rs:250-254` 在 `run_physics_schedule` 里先 `advance_by(sub_delta)` 一次（注释解释说「Advance the substep clock already so that systems running before the substepping loop have the right delta」），随后 `run_substep_schedule`（`src/dynamics/solver/schedule.rs:199-200`）又 `advance_by(sub_delta)` 一次。两次的 `delta` 值相同，因此子步内的 `delta` 语义正确；但 `Time<Substeps>` 的累计 `elapsed` 会是真实物理时间的 2 倍。若 Kairos 要用 `Time<Substeps>` 的 `elapsed` 做任何计时，必须注意这一点。

### 4.7 时间推进与 `dt` 如何到达 integrator

涉及的资源：

- `Time<Physics>`：`init_resource` 于 `src/schedule/mod.rs:63`；`Time<Physics>` 的类型参数 `Physics` 定义在 `src/schedule/time.rs:123-135`（`paused: bool`、`relative_speed: f64`，默认 `false`/`1.0`）。
- `Time<Substeps>`：`insert_resource(Time::new_with(Substeps))` 于 `src/schedule/mod.rs:64`（并在 `src/dynamics/solver/schedule.rs:23` 再插一次）；`Substeps` 定义 `src/schedule/time.rs:270-271`。
- `LastPhysicsTick`：`src/schedule/mod.rs:220-223`，由 `update_last_physics_tick`（`:281-286`）在每次物理步结束后更新为 `SystemChangeTick::this_run()`。

`run_physics_schedule` 的时间推进逻辑（`src/schedule/mod.rs:235-279`，核心 `:236-276`）：

```rust
let is_paused = world.resource::<Time<Physics>>().is_paused();          // :237
let old_clock = world.resource::<Time>().as_generic();                  // :238
let physics_clock = world.resource_mut::<Time<Physics>>();              // :239

// Get the scaled timestep delta time based on the timestep mode.
let timestep = old_clock.delta().mul_f64(physics_clock.relative_speed_f64());  // :242-244

if !is_paused {
    world.resource_mut::<Time<Physics>>().advance_by(timestep);          // :248
    let SubstepCount(substeps) = *world.resource::<SubstepCount>();      // :252
    let sub_delta = timestep.div_f64(substeps as f64);                   // :253
    world.resource_mut::<Time<Substeps>>().advance_by(sub_delta);        // :254
}

*world.resource_mut::<Time>() = world.resource::<Time<Physics>>().as_generic();  // :258

if !world.resource::<Time>().delta().is_zero() {                         // :261
    trace!("running PhysicsSchedule");
    schedule.run(world);                                                 // :263
}

if is_paused {
    world.resource_mut::<Time<Physics>>().advance_by(Duration::ZERO);    // :268-272
}

*world.resource_mut::<Time>() = old_clock;                               // :275
```

链条：

1. `old_clock` 是**泛型 `Time`**，它在宿主 schedule 里的含义由 Bevy 决定：在 `FixedPostUpdate` 下是 `Time<Fixed>`，在 `Update` 下是 `Time<Virtual>`。`src/schedule/time.rs:8-9` 的 doc 明确说了这一点：「In `FixedPostUpdate` and other fixed schedules, this uses `Time<Fixed>`, while in schedules such as `Update`, a variable timestep following `Time<Virtual>` is used.」
2. `timestep = Time.delta() * relative_speed`。
3. 未暂停时：`Time<Physics>.advance_by(timestep)`，同时 `Time<Substeps>.advance_by(timestep / substeps)`（预推进，理由见上）。
4. **把泛型 `Time` 替换为 `Time<Physics>`**（`:258`）—— 因此 `PhysicsSchedule` 内所有 `Res<Time>` 读到的是物理时钟。
5. 若 `Time<Physics>.delta()` 为零则跳过整个 `PhysicsSchedule`（`:261-264`）。
6. 结束后把泛型 `Time` 还原（`:275`）。

`dt` 到达 integrator 的具体路径：

- **子步内的速度积分**：`integrate_velocities` 用 `#[cfg(feature = "3d")] time: Res<Time>` + `time.delta_secs_f64() as Scalar`（`src/dynamics/integrator/mod.rs:349`、`:353-354`；2D 分支不需要 dt，因为它只做 damping + increment 加法）。此时泛型 `Time` == `Time<Substeps>`，故 `dt = sub_delta`。
- **子步内的位置积分**：`integrate_positions` 用 `time: Res<Time>` + `time.delta_seconds_adjusted()`（`src/dynamics/integrator/mod.rs:503-510`）。`delta_seconds_adjusted` 是 Avian 的 `TimePrecisionAdjusted` trait（`src/schedule/time.rs:273-292`），在 `f32` 下返回 `delta_secs()`，在 `f64` 下返回 `delta_secs_f64()`。
- **每物理步一次的速度增量预处理**：`pre_process_velocity_increments` 用 `time: Res<Time<Substeps>>` + `time.delta_secs_f64()`（`src/dynamics/integrator/mod.rs:270`、`:275`），把重力/外力当加速度累积后 `*= delta_secs` 变成增量（`:307-308`）。它跑在 `run_substep_schedule` 之前，所以依赖 `:254` 的那次预推进。
- **窄相**：`update_narrow_phase` 用 `time: Res<Time>` + `time.delta_seconds_adjusted()`（`src/collision/narrow_phase/mod.rs:274`、`:291`）。此时泛型 `Time` == `Time<Physics>`（`SubstepSchedule` 已把泛型 `Time` 还原为 `Time<Physics>`，见 `src/dynamics/solver/schedule.rs:212`），故 `dt = physics delta`。

`Time<Physics>` 的暂停/变速 API 由 `PhysicsTime` trait 提供（`src/schedule/time.rs:138-220`，实现 `:222-263`）：`pause`（`:252`）、`unpause`（`:256`）、`is_paused`（`:260`）、`set_relative_speed`（`:242`）/`set_relative_speed_f64`（`:246`，带 `assert!(ratio.is_finite())` 与 `assert!(ratio >= 0.0)`）、`with_relative_speed`（`:223`）。

`TimePrecisionAdjusted`：`src/schedule/time.rs:273-292`，为 `Time` 提供 `delta_seconds_adjusted()`。它在 `src/lib.rs:570` 以 `pub(crate)` 方式从 prelude 暴露。

### 4.8 `run_if` 与暂停条件

- 物理调度内**没有**对 `Time<Physics>::is_paused()` 的 `run_if`。暂停是通过「不推进时钟 → `delta` 为零 → `:261` 的检查跳过 `schedule.run(world)`」实现的。
- 「推论」这意味着**暂停那一帧仍会执行一次物理步**：`is_paused` 分支只在 `schedule.run` **之后**才 `advance_by(Duration::ZERO)`（`src/schedule/mod.rs:268-272`），而 `:261` 的 `is_zero()` 检查发生在此之前，所以暂停生效的第一帧 `Time<Physics>.delta()` 仍是上一帧的非零值，`PhysicsSchedule` 会照跑一次；从第二帧起 `delta == 0`，才真正停止。
- `PhysicsStepSystems::Sleeping` 整组：`.run_if(resource_exists::<PhysicsIslands>)`（`src/dynamics/solver/islands/sleeping.rs:75`），其中 `wake_all_islands` 单独 `.run_if(resource_changed::<Gravity>)`（`:73`）。
- `PhysicsTransformSystems::Propagate` 的系统：`.run_if(|config| config.propagate_before_physics)`（`src/physics_transform/mod.rs:104`）。
- `transform_to_position`：`.run_if(|config| config.transform_to_position)`（`:110`）。
- `position_to_transform`：`.run_if(|config| config.position_to_transform)`（`:122`）。
- `PhysicsDebugPlugin` 的绘制系统：`.run_if(|store: Res<GizmoConfigStore>| store.config::<PhysicsGizmos>().0.enabled)`（`src/debug_render/mod.rs:135`）。
- `PhysicsDiagnosticsUiPlugin` 的 UI 更新：`.run_if(diagnostics_are_enabled)`（`src/diagnostics/ui.rs:39`）。
- `debug_render_islands`：`.run_if(resource_exists::<PhysicsIslands>)`（`src/debug_render/mod.rs:132`）。
- 其它事实：`PhysicsStepSystems::First` 里 `on_change_joint_entities::<T>` 用 `.ambiguous_with(PhysicsStepSystems::First)` 显式声明歧义（`src/dynamics/solver/joint_graph/plugin.rs:122`）。

「推论」Kairos 若需要「物理暂停但 UI 仍响应」，Avian 的做法（暂停 = delta 归零 + 手动 `run_schedule(PhysicsSchedule)` 步进）比「用 `run_if` 关掉一堆系统」更简单，因为不需要维护每个系统的条件。手动步进的官方写法在 `src/schedule/time.rs:65-73`：

```rust
fn run_physics(world: &mut World) {
    for _ in 0..10 {
        world.resource_mut::<Time<Physics>>().advance_by(Duration::from_secs_f64(1.0 / 120.0));
        world.run_schedule(PhysicsSchedule);
    }
}
```

---

## 5. `src/physics_transform`：权威位姿与 `Transform` 的双向同步

### 5.1 组件

| 组件 | 定义 | 说明 |
| --- | --- | --- |
| `Position(pub Vector)` | `src/physics_transform/transform.rs:44-48`（`#[derive(Reflect, Clone, Copy, Component, Debug, Default, Deref, DerefMut, PartialEq, From)]`） | 全局位置。单位与精度由 `Vector`/`Scalar` 决定（2D `Vec2`/3D `Vec3`；f32 或 f64） |
| `Position::PLACEHOLDER` | `src/physics_transform/transform.rs:54` | `Self(Vector::MAX)`，用于标记「尚未初始化」 |
| `Rotation` (3D) | `src/physics_transform/transform.rs:741-745` | `pub struct Rotation(pub Quaternion);`，`#[cfg(feature = "3d")]` |
| `Rotation` (2D) | `src/physics_transform/transform.rs:170-182` | 结构体，字段 `cos`/`sin`（单位复数的实部/虚部），`#[cfg(feature = "2d")]` |
| `Rotation::PLACEHOLDER` (3D) | `src/physics_transform/transform.rs:752-757` | 四个分量全 `Scalar::MAX` |
| `Rotation::PLACEHOLDER` (2D) | `src/physics_transform/transform.rs:198-201` | `cos`/`sin` 均为 `Scalar::MAX` |
| `Rotation::IDENTITY` | 3D：`src/physics_transform/transform.rs:759-760`；2D：`src/physics_transform/transform.rs:203-207` | 无旋转 |
| `PreSolveDeltaPosition(pub Vector)` | `src/physics_transform/transform.rs:125` | XPBD 位置求解前累积的平移 |
| `PreSolveDeltaRotation(pub Rotation)` | `src/physics_transform/transform.rs:132` | 同上，旋转 |
| `ApplyPosToTransform` | `src/physics_transform/mod.rs:244-245` | 标记组件：让 `position_to_transform` 也作用于非 `RigidBody` 实体 |

`RotationValue` 是内部类型别名：2D 下 = `Scalar`（`src/physics_transform/transform.rs:135-137`），3D 下 = `Quaternion`（`:138-141`），由 `src/physics_transform/mod.rs:8` 以 `pub(crate)` 导出。

### 5.2 为什么不用 `Transform`（源码给出的理由）

`src/lib.rs:401-422` 的 FAQ 直接列出 7 条：

1. Position/rotation 从物理视角应是全局的（`:409`）。
2. `Transform` 的 scale 与 shear 会给物理带来问题与舍入误差（`:410`）。
3. `Transform` 层级本身会带来麻烦（`:411`）。
4. 没有 `f64` 版的 `Transform`（`:412`）。
5. 没有 2D 版的 `Transform`（`:413`）。
6. 位置与旋转分开时，理论上可以有更多系统并行（`:414`）。
7. 只有刚体才需要旋转（`:415`）。

补充 doc 说明：外部项目只在**需要在 `PhysicsSystems::StepSimulation` 内管理位置**时才必须用 `Position`/`Rotation`（`src/lib.rs:417-418`）。

### 5.3 `PhysicsTransformPlugin`

`src/physics_transform/mod.rs:48-67`（struct + `new`），`build` 在 `:69-125`：

1. `init_resource::<PhysicsTransformConfig>()`（`:71`）。
2. `init_resource::<StaticTransformOptimizations>()`（`:74`）—— 注释说明是为了在 `TransformPlugin` 缺席时也能工作（`:73`）。
3. **条件注册 required components**（`:76-83`）：
   ```rust
   if app.world().resource::<PhysicsTransformConfig>().position_to_transform {
       app.register_required_components::<Position, Transform>();
       app.register_required_components::<Rotation, Transform>();
   }
   ```
   → 「推论」若用户在插件注册之后才把 `position_to_transform` 置为 `false`，required component 已经注册，无法撤销；这个开关必须在 `add_plugins` 之前/之时决定。
4. 在宿主 schedule（默认 `FixedPostUpdate`）上：
   - `(PhysicsTransformSystems::Propagate, PhysicsTransformSystems::TransformToPosition).chain().in_set(PhysicsSystems::Prepare)`（`:86-94`）。
   - `mark_dirty_trees` → `propagate_parent_transforms` → `sync_simple_transforms` 链式 ∈ `PhysicsTransformSystems::Propagate`，`.run_if(config.propagate_before_physics)`（`:95-105`）。这三个函数来自 `bevy::transform::systems`（`src/physics_transform/mod.rs:26`）—— 即 Avian **自己重跑一遍 Bevy 的 transform 传播**。
   - `transform_to_position` ∈ `PhysicsTransformSystems::TransformToPosition`，`.run_if(config.transform_to_position)`（`:106-111`）。
   - `PhysicsTransformSystems::PositionToTransform.in_set(PhysicsSystems::Writeback)`（`:114-117`），`position_to_transform` ∈ 该 set，`.run_if(config.position_to_transform)`（`:118-123`）。

`PhysicsTransformSystems` 枚举（`src/physics_transform/mod.rs:168-176`）：`Propagate`、`TransformToPosition`、`PositionToTransform`。

### 5.4 写回系统：`position_to_transform`

两个按维度分叉的实现：2D 在 `src/physics_transform/mod.rs:270-310`，3D 在 `:317-349`。

- 查询类型（`:247-263`）：
  - `PosToTransformComponents = (&mut Transform, &Position, &Rotation, Option<&ChildOf>)`
  - `PosToTransformFilter = (Or<(With<RigidBody>, With<ApplyPosToTransform>)>, Or<(Changed<Position>, Changed<Rotation>)>)`
  - `ParentComponents = (&GlobalTransform, Option<&Position>, Option<&Rotation>)`
- **过滤条件**：只处理「有 `RigidBody` 或 `ApplyPosToTransform`」**且**「`Position` 或 `Rotation` 本帧被改动」的实体（`:254-257`）。即写回本身也是变更驱动的。
- 有父实体时，用父的 `Position`/`Rotation`（若无则退回父的 `GlobalTransform`）重建父的 `Transform`，再 `GlobalTransform::from(子 Transform).reparented_to(&parent_global)` 反算局部 `Transform`，只写 `translation` 与 `rotation`，**不写 `scale`**（`:322-347`）。
- 无父时直接 `transform.translation = pos.f32(); transform.rotation = rot.f32();`（`:344-347`）。
- 层级语义：嵌套刚体被视为**扁平结构**，子刚体不跟随父（plugin doc `src/physics_transform/mod.rs:40-47`）。要刚性附着应使用 `FixedJoint`（`:46-47`）。

### 5.5 反向同步：`transform_to_position`（「用户是否移动了 Transform？」）

`src/physics_transform/mod.rs:187-237`。

- 查询：`Query<(&GlobalTransform, &mut Position, &mut Rotation)>`（`:188`）。
- 签名还取 `Res<PhysicsLengthUnit>`、`Res<LastPhysicsTick>`、`SystemChangeTick`（`:189-191`）。
- tick 修正：`let this_run = if last_physics_tick.0.get() == 0 { Tick::new(1) } else { system_tick.this_run() };`（`:196-200`），注释解释首帧两者都是 0，需要人为让 `this_run` 更大。
- 容差：
  - `distance_tolerance = length_unit * 1e-5`（`:203`，注释：约 0.01 mm）。
  - `rotation_tolerance = (0.1 as Scalar).to_radians()`（`:205`，注释：约 0.1°）。
- **变更检测逻辑**（核心）：
  ```rust
  let position_changed = !position.is_added()
      && is_changed_after_tick(Ref::from(position.reborrow()), last_physics_tick.0, this_run);
  if !position_changed && position.abs_diff_ne(&transform_translation, distance_tolerance) {
      position.0 = transform_translation;
  }
  ```
  （`:215-223`；旋转版 `:225-235`。）
  即：**只有当 `Position` 在「上一次物理步结束之后」没有被物理引擎自己改动时**，才用 `GlobalTransform` 覆盖它。这正是「用户是否移动了 transform」的判定机制 —— 与 `LastPhysicsTick` 配合，把「物理步内的写」和「步外的用户写」区分开。
- 辅助函数 `is_changed_after_tick`：`src/schedule/mod.rs:225-232`：
  ```rust
  pub(crate) fn is_changed_after_tick<C: Component>(component_ref: Ref<C>, tick: Tick, this_run: Tick) -> bool {
      let last_changed = component_ref.last_changed();
      component_ref.is_changed() && last_changed.is_newer_than(tick, this_run)
  }
  ```
- `LastPhysicsTick` 的更新：`src/schedule/mod.rs:281-286`，在 `PhysicsSchedule` 内 `.after(PhysicsStepSystems::Last)`。

### 5.6 `init_physics_transform`：实体创建时的位姿初始化

`src/physics_transform/transform.rs:1116` 起。触发点有两处：

- `RigidBody` 的 `on_add` hook：`src/dynamics/rigid_body/mod.rs:283`（`#[component(immutable, on_add = RigidBody::on_add)]`）→ `:322-325` 调用 `init_physics_transform(&mut world, &ctx)`。
- `Collider` 后端 hook：`src/collision/collider/backend.rs:114-122`，**仅当实体没有 `RigidBody` 时**才调用（避免重复，源码自带注释「The special case for rigid bodies is a bit of a hack here.」）。

`init_physics_transform` 的行为：

1. 读 `Position`/`Rotation`，若缺失或等于 `PLACEHOLDER` 则视为 placeholder（`:1120-1125`）；placeholder 分别归零/单位化（`:1127-1132`）。
2. `is_not_placeholder = !is_pos_placeholder || !is_rot_placeholder`（`:1135`）—— 即用户是否手动设过 `Position`/`Rotation`。
3. 沿 `ChildOf` 往上累乘父的 `Transform` 得到 `parent_global_transform`（`:1142-1151`）。
4. 若有自己的 `Transform`，计算 `global_transform = parent_global_transform * GlobalTransform::from(transform)` 并**强制写入 `GlobalTransform`**（`:1153-1159`，注意 `unwrap()`）。
5. 若用户手动设过 `Position`/`Rotation` 且 `config.position_to_transform` 为真，则反过来把 `Transform` 调整成匹配 `Position`/`Rotation` 的局部值（`:1161-1176+`，含父实体分支）。
6. 末尾还会把仍是 placeholder 的 `Position`/`Rotation` 补上（`:1263`、`:1270` 有过滤 `**pos == Position::PLACEHOLDER` / `**rot == Rotation::PLACEHOLDER`）。

### 5.7 与 Kairos 的对应关系（对照要点）

事实层面已经很清楚：Avian 的模型是

- **`Position`/`Rotation` = 物理权威位姿（全局、无父、无 scale）**；
- **`Transform` = 渲染/层级用的局部变换**；
- 物理侧在 `PhysicsSystems::Prepare` 把 `GlobalTransform` 拉回 `Position`/`Rotation`（`transform_to_position`），在 `PhysicsSystems::Writeback` 把 `Position`/`Rotation` 推回 `Transform`（`position_to_transform`）；
- 两个方向都用「tick + 容差 + `Added` 判定」来避免自激循环（`src/physics_transform/mod.rs:196-235`）。

「推论」映射到 Kairos：`kairos_transform` 的 `LocalTransform`/`GlobalTransform` 对应 Bevy 的 `Transform`/`GlobalTransform`；`kairos_physics` 需要引入的对应物是 `Position`/`Rotation`（全局、无 scale、可 f64），并在物理调度里插两段：准备段的 `Global → Position` 拉回、写回段的 `Position → Local` 推回。Avian 依赖 Bevy 的 `ChildOf`/`Children`/`GlobalTransform::compute_transform`/`reparented_to`，Kairos 需要提供等价操作。`LastPhysicsTick` 这种「物理步计数器」也应一并照搬，否则无法区分「谁改的」。

---

## 6. 启动期 / 一次性工作与 required components

### 6.1 `(RigidBody::Dynamic, Collider::sphere(0.5))` 的完整追踪

**A. `RigidBody::Dynamic` 的 required components**

`src/dynamics/rigid_body/mod.rs:267-282`：

```rust
#[require(
    Position::PLACEHOLDER,
    Rotation::PLACEHOLDER,
    LinearVelocity,
    AngularVelocity,
    ComputedMass,
    ComputedAngularInertia,
    ComputedCenterOfMass,
    AccumulatedLocalAcceleration,   // "Required for local forces and acceleration."
    PreSolveDeltaPosition,
    PreSolveDeltaRotation,
)]
#[component(immutable, on_add = RigidBody::on_add)]
pub enum RigidBody { Dynamic, Static, Kinematic }
```

- `RigidBody` 是 `#[component(immutable)]`（`:283`）→ 值不可原地改，改类型会触发替换（进而触发 `On<Insert, RigidBody>` 与 `On<Discard, RigidBody>` 观察者）。
- `on_add = RigidBody::on_add`（`:283`）→ `src/dynamics/rigid_body/mod.rs:322-325` 调 `init_physics_transform`（见 5.6）。
- 注意源码自带 TODO（`:268-269`）：「Only dynamic and kinematic bodies need velocity, and only dynamic bodies need mass and angular inertia.」→ 目前对静态刚体也无条件插入这些组件。

**B. `PhysicsTransformPlugin` 让 `Position`/`Rotation` 反过来 require `Transform`**

`src/physics_transform/mod.rs:81-82`（前提是 `config.position_to_transform == true`，默认 `true`，见 `:156-165`）。

**C. `Collider` 自己的 required components**

`src/collision/collider/parry/mod.rs:356-365`：

```rust
#[derive(Clone, Component, Debug)]
#[require(
    ColliderMarker,
    ColliderAabb,
    CollisionLayers,
    EnlargedAabb,
    ColliderDensity,
    ColliderMassProperties
)]
pub struct Collider { shape, scaled_shape, scale }
```

- `ColliderAabb::default()` = `INVALID`（`src/collision/collider/mod.rs:445-449`，常量为 `min = +INF`、`max = -INF`，`:453-456`）。
- `Collider` **没有** `#[require(Position)]`/`#[require(Rotation)]` —— 这是由插件补的（下一条），且源码注释说明 `Collider` 目前不实现 `Reflect`（`:355`）。

**D. `ColliderBackendPlugin` 补 required components 与 hooks**

`src/collision/collider/backend.rs:97-104`：

```rust
let _ = app.try_register_required_components_with::<C, Position>(|| Position::PLACEHOLDER);
let _ = app.try_register_required_components_with::<C, Rotation>(|| Rotation::PLACEHOLDER);
let _ = app.try_register_required_components::<C, ColliderMarker>();
let _ = app.try_register_required_components::<C, ColliderAabb>();
let _ = app.try_register_required_components::<C, EnlargedAabb>();
let _ = app.try_register_required_components::<C, CollisionLayers>();
let _ = app.try_register_required_components::<C, ColliderDensity>();
let _ = app.try_register_required_components::<C, ColliderMassProperties>();
```

`try_register_*` 而非 `register_*`：允许自定义 collider 类型覆盖。

**E. 各类 hook / observer / 一次性系统的分工**

| 时机 | 机制 | 位置 | 做什么 |
| --- | --- | --- | --- |
| `RigidBody` 插入时（组件 hook，同步） | `on_add = RigidBody::on_add` | `src/dynamics/rigid_body/mod.rs:283`, `:322-325` | `init_physics_transform`：算全局位姿、写 `GlobalTransform`、必要时反向调 `Transform` |
| `Collider` 插入时（组件 hook，同步） | `on_add` | `src/collision/collider/backend.rs:114-122` | 若无 `RigidBody` 则 `init_physics_transform` |
| `Collider` 插入/覆盖时（组件 hook，同步） | `on_insert` | `src/collision/collider/backend.rs:123-160` | 按 `GlobalTransform::scale()` 设 collider scale；按 `ColliderDensity` 算 `ColliderMassProperties`；`Sensor` 时质量为 0 |
| `Collider` 移除时（组件 hook） | `on_remove` | `src/collision/collider/backend.rs:164-187` | 移除 `ColliderMarker`；给 `ColliderOf.body` 插 `RecomputeMassProperties` |
| `RigidBody` 插入时（observer） | `on_insert_rigid_body` | `src/dynamics/solver/solver_body/plugin.rs:43`, `:132-152` | dynamic/kinematic → 插入 `SolverBody` + `SolverBodyInertia`；static → 移除 |
| `RigidBody` 插入时（observer） | `On<Add, RigidBody>` | `src/dynamics/rigid_body/mass_properties/mod.rs:284-288` | `mass_helper.update_mass_properties(entity)` |
| `RigidBodyColliders` 插入时（observer） | `On<Insert, RigidBodyColliders>` | `src/dynamics/rigid_body/mass_properties/mod.rs:292-296` | 同上（collider 挂到 body 时重算质量） |
| `Collider`/`RigidBody` 插入时（observer） | `On<Add, (RigidBody, ColliderMarker)>` | `src/collision/collider/collider_hierarchy/plugin.rs:15` | 找到最近的 `RigidBody` 祖先，插入 `ColliderOf` |
| `Collider` 插入时（observer） | `On<Add, C>` | `src/collider_tree/update.rs:75` | 初始化 `ColliderAabb` / `EnlargedAabb`（考虑 `CollisionMargin`、`contact_tolerance`，`:88-102`） |
| `RigidBody` 替换时（observer） | `on_change_joint_entities` 等 | `src/dynamics/solver/joint_graph/plugin.rs:110-115` | 关节实体变化时更新图 |
| 每帧 `Update` | `init_collider_constructors`、`init_collider_constructor_hierarchies` | `src/collision/collider/backend.rs:240-247` | 把 `ColliderConstructor`/`ColliderConstructorHierarchy` 转成 `Collider`（mesh 未加载时**每帧重试**，`:294-297`） |
| 每帧 `PreUpdate` | `clear_unused_colliders` | `src/collision/collider/cache.rs:13` | 按 `AssetEvent<Mesh>` 清 collider 缓存 |

「推论」综合起来，`commands.spawn((RigidBody::Dynamic, Collider::sphere(0.5)))` 之后，实体上最终会有：

- 来自 `RigidBody` 的 require：`Position(PLACEHOLDER)`、`Rotation(PLACEHOLDER)`、`LinearVelocity`、`AngularVelocity`、`ComputedMass`、`ComputedAngularInertia`、`ComputedCenterOfMass`、`AccumulatedLocalAcceleration`、`PreSolveDeltaPosition`、`PreSolveDeltaRotation`、`RecomputeMassProperties`（`mass_properties/mod.rs:281`）。
- 来自 `Position`/`Rotation` 的 require：`Transform`（`physics_transform/mod.rs:81-82`）。`Transform` 又 require `GlobalTransform`（Bevy 侧）。
- 来自 `Collider` 与 `ColliderBackendPlugin` 的 require：`ColliderMarker`、`ColliderAabb`、`EnlargedAabb`、`CollisionLayers`、`ColliderDensity`、`ColliderMassProperties`、`ColliderTreeProxyKey`（`collider_tree/mod.rs:70-73`）。`Position`/`Rotation` 已由 `RigidBody` 提供，`try_register_required_components_with` 不会重复插入。
- 由 hook 同步产出：被正确计算的 `GlobalTransform`、`Position`/`Rotation` 的实际值（不再是 PLACEHOLDER）、collider 的 `scale`、`ColliderMassProperties` 实际值。
- 由 observer 产出：`ColliderOf { body: self }`（`ALLOW_SELF_REFERENTIAL = true`，`src/collision/collider/collider_hierarchy/mod.rs:72`）、`ColliderTransform`（`ColliderOf` 的 require，`src/collision/collider/collider_hierarchy/mod.rs:49`）、`RigidBodyColliders`（`src/collision/collider/collider_hierarchy/mod.rs:132-140`）、`SolverBody` + `SolverBodyInertia`。
- 由 `SolverBody` 的 require 链继续派生：`VelocityIntegrationData`（`src/dynamics/integrator/mod.rs:48`）、`BodyIslandNode`（`src/dynamics/solver/islands/mod.rs:76`）、`SleepThreshold`（`src/dynamics/solver/islands/sleeping.rs:50`）、`SleepTimer`（`src/dynamics/solver/islands/sleeping.rs:51`）。
- 下一物理步的第一个 `PhysicsSchedule` 里，`prepare_solver_bodies`（`src/dynamics/solver/solver_body/plugin.rs:181-`）才把 `LinearVelocity`/`AngularVelocity`/`ComputedMass`/`ComputedAngularInertia` 拷进 `SolverBody`，`writeback_solver_bodies`（`:263-292`）在 `SolverSystems::Finalize` 再写回。

### 6.2 只跑 `Added`/`Changed` 的部分（务必注意）

| 系统 / hook | 过滤条件 | 位置 |
| --- | --- | --- |
| `position_to_transform` | `Or<(Changed<Position>, Changed<Rotation>)>` + `Or<(With<RigidBody>, With<ApplyPosToTransform>)>` | `src/physics_transform/mod.rs:254-257` |
| `transform_to_position` | 全量遍历，但内部用 `is_added()` + `is_changed_after_tick` + 容差三重判定 | `src/physics_transform/mod.rs:215-235` |
| `queue_mass_recomputation_on_mass_change` | `MassPropertyChanged = Or<(Changed<Mass>, Changed<AngularInertia>, Changed<CenterOfMass>)>` | `src/dynamics/rigid_body/mass_properties/mod.rs:350-355`, 系统注册 `:315` |
| `update_mass_properties` | 查询 `With<RecomputeMassProperties>`（`RecomputeMassProperties` 用完即删） | `src/dynamics/rigid_body/mass_properties/mod.rs:322-327`；`MassPropertySystems` doc `:338-340` |
| `update_local_frames::<Joint>` | `Query<&mut Joint, Changed<Joint>>` | `src/dynamics/joints/fixed.rs:254`（其它关节同构） |
| `propagate_collider_transforms` | `Ref<Transform>` 变更传播 + `AncestorMarker<ColliderMarker>` 剪枝 | `src/collision/collider/collider_transform/plugin.rs:108`、变更判定 `:130` |
| `init_collider_constructors` | 查询 `&ColliderConstructor`（无 `Changed` 过滤 → 每帧扫到移除为止） | `src/collision/collider/backend.rs:272-278`, `:318` |
| `update_previous_velocity` | 未加 `Changed` 过滤（全量） | `src/interpolation.rs:295-298` |

---

## 7. 推荐阅读顺序（架构理解用）

按依赖从外到内、从静态结构到运行时行为排序。每项一行理由。

1. `src/lib.rs:452-530` —— 先看 crate 级别的 feature 校验与模块声明，确定「哪些代码会在 3D 构建里真的存在」。
2. `src/lib.rs:681-789` —— `PhysicsPlugins` 的字段、`new`/`default`、以及 `build()` 的完整插入顺序；这是整张架构图的目录。
3. `crates/avian3d/Cargo.toml:14-78` —— feature 矩阵与 `[lib] path = "../../src/lib.rs"`，理解「一份源码两个 crate」。
4. `src/schedule/mod.rs:26-126` —— `PhysicsSchedulePlugin`：调度标签、资源初始化、`PhysicsSystems` 与 `PhysicsStepSystems` 的 chain；整篇文档最重要的一段。
5. `src/schedule/time.rs:1-292` —— `Physics`/`Substeps` 时钟、`PhysicsTime` trait、`TimePrecisionAdjusted`；`dt` 的来源。
6. `src/schedule/mod.rs:128-316` —— `run_physics_schedule`、`is_changed_after_tick`、`update_last_physics_tick`、`assert_components_finite`；时间推进与 change-detection 基建。
7. `src/physics_transform/mod.rs:29-349` —— `PhysicsTransformPlugin`、`PhysicsTransformConfig` 四个开关、`transform_to_position` / `position_to_transform` 的真实实现。
8. `src/physics_transform/transform.rs:44-60,111-135,170-210,735-760,1116-1290` —— `Position`/`Rotation`/`PreSolveDelta*` 的定义、`PLACEHOLDER` 机制、`init_physics_transform` 全流程。
9. `src/dynamics/rigid_body/mod.rs:255-330` —— `RigidBody` 的 `#[require(...)]` 列表、`immutable`、`on_add` hook；启动期行为的总入口。
10. `src/dynamics/solver/mod.rs:27-84` —— `SolverPlugins` 组内顺序，看 solver 侧的插件全景。
11. `src/dynamics/solver/schedule.rs:13-213` —— `SolverSchedulePlugin`、`SolverSystems`、`SubstepSolverSystems`、`SubstepCount`、`run_substep_schedule`；子步循环的权威定义。
12. `src/dynamics/integrator/mod.rs:15-115,195-260,315-535` —— `IntegratorPlugin`、`IntegrationSystems` 四段、`VelocityIntegrationData`、`pre_process_velocity_increments`、`integrate_velocities`、`integrate_positions`；重力与外力如何变成位移。
13. `src/dynamics/solver/plugin.rs:88-151` —— `SolverPlugin` 的完整系统注册（接触约束准备、warm start、solve、restitution、damping），理解 solver 的输入输出。
14. `src/dynamics/solver/solver_body/plugin.rs:132-303` —— `SolverBody` 的生命周期、`prepare_solver_bodies` 与 `writeback_solver_bodies`；「物理状态如何进出 ECS 组件」。
15. `src/collision/broad_phase/mod.rs:156-209` + `src/collision/broad_phase/bvh_broad_phase.rs:20-59` —— 宽相被拆成 core + 算法两层，以及 `CollectCollisions` 的角色。
16. `src/collider_tree/mod.rs:36-194` —— `ColliderTreePlugin<C>`、四棵树、`ColliderTreeSystems` 三段在调度中的位置（跨 BroadPhase / Solver::Finalize）。
17. `src/collision/narrow_phase/mod.rs:59-198,274-302` —— `NarrowPhasePlugin<C,H>` 的 sets、`NarrowPhaseConfig`、`CollisionEventSystems` 在 `Finalize`。
18. `src/collision/collider/backend.rs:94-249` —— `ColliderBackendPlugin<C>` 的 required components、三个组件 hook、一个 observer 与两个系统；「只 spawn 一个 Collider 会发生什么」的答案。
19. `src/dynamics/rigid_body/mass_properties/mod.rs:250-355` —— `MassPropertyPlugin` 的 `MassPropertySystems` 三段与 `RecomputeMassProperties` 的重算流程。
20. `src/dynamics/solver/islands/sleeping.rs:40-79` + `src/dynamics/solver/islands/mod.rs:66-154` —— sleeping/islands 在 `PhysicsStepSystems::Sleeping` 的五系统链与 `run_if` 条件，补完步内七段中的最后几段。

补充（按需）：`src/dynamics/ccd/mod.rs:248-268`（`SweptCcdSystems` 的插入点）、`src/spatial_query/mod.rs:179-225`（空间查询挂在宿主 schedule 而非 `PhysicsSchedule`）、`src/interpolation.rs:266-300`（插值插在 `PhysicsStepSystems::First`）、`src/lib.rs:794-851`（`PhysicsPluginsWithHooks` 如何用 `disable` 替换宽相/窄相）。

---

## 8. 本文件未验证 / 不确定

以下内容**没有**从源码中确认，不做猜测：

1. **未编译、未运行测试。** 按任务约束跳过了 `cargo build`/`cargo test`，因此所有 feature 组合是否真的能编译通过、`register_required_components` 之间是否存在注册顺序冲突导致的 panic，均未经验证。文中「顺序有硬约束」只依据源码里的 `expect(...)` 断言。
2. **Bevy 0.19 侧的行为未核对。** 文中关于 `Time<Fixed>`/`Time<Virtual>`/泛型 `Time` 在哪个 schedule 取什么时钟的说法，来源于 Avian 自己的 doc（`src/schedule/time.rs:8-9`）与调用点 `src/schedule/mod.rs:238`，**没有**去读 Bevy 0.19 源码验证。`TransformSystems::Propagate`、`PickingSystems::Backend`、`VisibilitySystems::CalculateBounds`、`GizmoConfigStore` 等外部调度标签的实际定义位置未确认。
3. **`RegisterRequiredComponents` 在 Bevy 0.19 中的确切语义未核对。** 特别是 `try_register_required_components_with` 与先注册者优先的规则；文中「顺序影响 required components 注册结果」是 「推论」。
4. **多个 `PhysicsPlugins` 实例共存的行为未验证。** `NarrowPhasePlugin` 里有一个 `NarrowPhaseInitialized` 资源（`src/collision/narrow_phase/mod.rs:100`, `:107`, `:184`）和 `JointGraphPlugin` 里的 `JointGraphPluginInitialized`（`src/dynamics/solver/joint_graph/plugin.rs:57`, `:63-66`）用于「只做一次」，但同一 app 添加两次 `PhysicsPlugins` 会怎样未验证。
5. **`Time<Substeps>.elapsed` 双倍推进**（4.6 末）是我按两处 `advance_by` 直接读出的，未能通过运行时验证其是否真的导致可见问题。同样，**暂停后多跑一步**（4.8）是按 `src/schedule/mod.rs:261-272` 的语句顺序推出的，未实测。
6. **`src/physics_transform/transform.rs:1116` 之后 `init_physics_transform` 的尾部逻辑未逐行读完。** 我确认了 `:1116-1176` 与 `:1263`/`:1270` 的 placeholder 过滤行，但 `:1177-1290` 的分支细节（尤其 2D/3D 父层级分支与 `ApplyPosToTransform` 相关部分）未完整阅读，文中相关描述限于已读部分。
7. **`CollisionHooks` 与 `ColliderConstructor`/`ColliderConstructorHierarchy` 的内部实现未展开**（`src/collision/hooks.rs`、`src/collision/collider/constructor.rs`），只覆盖了它们被注册的时机。
8. **`src/dynamics/solver/xpbd/` 的算法细节未展开**，只记录了 `XpbdSolverPlugin` 的 set 顺序与系统注册。`SubstepSolverSystems` 与 `XpbdSolverSystems` 的相互作用（尤其 `VelocityProjection`）未深读。
9. **`src/character_controller/`、`src/debug_render/`、`src/diagnostics/ui.rs` 的细节未展开**，只记录了插件与其系统所在 schedule。
10. **各模块 `wc -l` 行数为 2025-09-10 检出时刻的静态值**，随源码变动会失效；引用时应以 file:line 为准。
