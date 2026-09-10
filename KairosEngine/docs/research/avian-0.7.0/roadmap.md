# Avian 0.7.0 学习与物理模块重建路线图

> 目标：让你**按顺序精读 Avian 源码**，并**同步把 `kairos_physics` 一步步重建**成 Avian 风格的物理模块。
> 顺序遵循你的要求：**碰撞体 → 刚体 → 刚体物理驱动 → 碰撞检测 → 更复杂内容**。

## 本目录文件

| 文件 | 内容 |
|---|---|
| `roadmap.md`（本文件） | 总路线图：基准、架构骨架、里程碑、读码清单、验收标准 |
| `01-architecture-and-schedules.md` | 插件与调度架构、固定步骨架、`physics_transform` |
| `02-colliders.md` | `Collider` 子系统：形状、必需组件、碰撞层、collider tree |
| `03-rigid-bodies-and-dynamics.md` | `RigidBody`、质量属性、力与重力、积分器、求解器、CCD |
| `04-collision-detection.md` | Broad phase、narrow phase、contact types、事件、hooks、空间查询 |
| `05-bevy-dependency-surface.md` | Avian 对 Bevy 的依赖面清点 + 移植到 `kairos_ecs`/`kairos_math` 的可行性与成本 |

---

## 0. 基准（先固定住版本，否则后面全是错）

- **Avian 源码**：`v0.7.0`，commit `965e85bf590f53fbf8a29f7ebd0326b5dbc985d1`。
  本地只读检出：`/Users/baiaoxiang/KairosEngine/.scratch/avian-research/avian-0.7.0`。
  下面所有 `src/...` 引用都相对该检出根目录。
- **Avian 的依赖版本**：`bevy 0.19.0`、`parry3d 0.27`、`obvhs 0.3`、`bevy_heavy 0.5`、`bevy_transform_interpolation 0.5`
  （`crates/avian3d/Cargo.toml`）。注意与 Kairos 现状（`rapier3d 0.33` / `parry3d 0.28`）**不是同一代**。
- **Kairos 现状**：`kairos_physics` 全crate 仅 860 行（`lib.rs` 387 / `tests.rs` 429 / `collider.rs` 27 / `rigid_body.rs` 17），
  基于 `rapier3d 0.33`，对外只暴露：
  - `physics::install(&mut world, schedule::FixedUpdate)`（`kairos_engine/src/kairos_editor.rs:45`）
  - `PhysicsEngine::{insert_movable_sphere, insert_immovable_box}`（`kairos_physics/src/lib.rs:111-163`）
  - `RigidBody` / `Collider`：**只持有一个 rapier handle**，句柄 crate-private（`rigid_body.rs:17` / `collider.rs:27`）
  - `physics_step_system`：push（写 LocalTransform→rapier）→ step → pull（rapier→LocalTransform）（`lib.rs:278-303`）
  - `on_discard` hook 回收 rapier 对象（`lib.rs:233-258`）
- **唯一实际用例**：demo 的一块平面 + 一个球（`kairos_engine/src/kairos_game.rs:251-257`）。
- **Kairos 已有的调度骨架与 Avian 同构**：Kairos 有 `RunFixedMainLoop` 驱动器 + `FixedUpdate` 内容调度
  （`kairos_engine/src/kairos_editor/schedule/test.rs:17-18, 139-154`），
  这与 Avian 的「固定步调度里跑 `PhysicsSchedule`」模型可以直接一一对应，**不需要改造调度系统**。

> ⚠️ **首要纪律**：只以本检出为准。网上教程、LLM 记忆、Avian 0.1–0.6 的资料在 0.7.0 上会**直接编译不过**（见 §1.1）。

---

## 1. 读码前必须知道的 13 条 0.7.0 反直觉事实

这一节专门用来防止「带着旧认知读新代码」。

### 1.1 `RigidBody` 的静态变体叫 `Static`，不叫 `Fixed`
`src/dynamics/rigid_body/mod.rs:284-304` 定义 `RigidBody { Dynamic, Static, Kinematic }`，
并提供 `is_dynamic()` / `is_static()` / `is_kinematic()`（同文件 `:308-320`）。
`README.md:108-118` 的官方示例也用 `RigidBody::Static`。
全仓 grep 确认没有任何 `RigidBody::Fixed` / `is_fixed` 残留（仅有无关的 `Time::<Fixed>` 与 `FixedJoint`）。

### 1.2 `Collider` 组件只有形状字段，物理材质全在**兄弟组件**上
```rust
#[derive(Clone, Component, Debug)]
#[require(ColliderMarker, ColliderAabb, CollisionLayers,
          EnlargedAabb, ColliderDensity, ColliderMassProperties)]
pub struct Collider { shape: SharedShape, scaled_shape: SharedShape, scale: Vector }
```
（`src/collision/collider/parry/mod.rs:356-376`）。
即：**密度、摩擦、恢复系数、质量属性都不是 `Collider` 的字段**，而是自动伴随插入的组件。
这与 rapier「一个 `Collider` 带全部参数」的对象式设计**根本不同**，是 Avian 最核心的 ECS 化决策。

### 1.3 默认的 `Collider` 类型本体就住在 `parry/` 下
`pub struct Collider` 定义在 `src/collision/collider/parry/mod.rs:366`。
Avian 用「后端无关的 `Collider` 契约 + Parry 实现」的方式组织（`ColliderBackendPlugin<C>`、`src/collision/collider/backend.rs:68`），
所以你可以有自定义 collider 类型（`crates/avian3d/examples/custom_collider.rs` 是官方示例）。

### 1.4 积分发生在**子步循环内部**，不是独立阶段
`IntegratorPlugin` 的默认调度是 `SubstepSchedule`，不是 `PhysicsSchedule`
（`src/dynamics/integrator/mod.rs:39-43`）。
所以「重力/速度/位置」这一块属于求解器子步循环，而不是顶层的一个 `Integrate` 集合。

### 1.5 重力先被折成**速度增量**，每个物理步算一次、每个子步复用
`IntegrationSystems::UpdateVelocityIncrements` 在 `PhysicsSchedule` 的 `SolverSystems::PreSubstep` 上运行，
`ClearVelocityIncrements` 在 `PostSubstep` 上清理
（`src/dynamics/integrator/mod.rs:52-62, 92-111`）。
子步里只做 `integrate_velocities` + `clamp_velocities`（`Velocity` 集合）与 `integrate_positions`（`Position` 集合）（同文件 `:73-86`）。

### 1.6 `Sleeping` 排在 `Solver` **之后**
`PhysicsStepSystems` 的顺序是 `First → BroadPhase → NarrowPhase → Solver → Sleeping → Finalize → Last`
（`src/schedule/mod.rs:96-107, 191-214`）。旧版 Avian 的睡眠阶段位置不同，别照抄旧顺序。

### 1.7 默认 broad phase 是 **BVH**；它是**两层**结构，不是一层
**不是** sweep-and-prune。两层要分清：

- **加速结构层**：`ColliderTrees` —— 按 dynamic / kinematic / static / standalone 分成 **4 棵** `obvhs` BVH
  （`src/collider_tree/mod.rs:121-130`）；
- **算法层**：`BvhBroadPhasePlugin` 去查询这些树（`src/collision/broad_phase/bvh_broad_phase.rs:51-56`），
  它是 0.7.0 默认（`src/lib.rs:783`，由 `BroadPhaseCorePlugin` + `BvhBroadPhasePlugin` 组成，`:782-783`）。

### 1.8 broad phase **直接写入 `ContactGraph`**，没有中间的 pair 列表资源
`BroadCollisionPairs` 这个资源在 0.7.0 **不存在**（全库 grep 零命中）。
落地点是 `src/collision/broad_phase/bvh_broad_phase.rs:173-204`，函数直接拿 `ResMut<ContactGraph>`（同文件 `:56`）。

### 1.9 碰撞事件叫 `CollisionStart` / `CollisionEnd`
不是 `CollisionStarted` / `CollisionEnded`（`src/collision/collision_events.rs:171, 268`）。
它们同时是 `Message` 与 `EntityEvent`，所以既能用 `MessageReader<CollisionStart>` 也能用观察者 `On<CollisionStart>`
（同文件 `:52, :90`）。

### 1.10 `CollisionHooks` 只有**两个**方法
`filter_pairs` 与 `modify_contacts`（`src/collision/hooks.rs:164, 187`），**没有** `on_collision`。

### 1.11 子步数默认是 **6**
`SubstepCount` 默认 `Self(6)`（`src/dynamics/solver/schedule.rs:185-190`）。
一个物理步 = 6 次子步积分 + 6 次约束求解。

### 1.12 两个物理调度都强制**单线程**执行器 + 歧义检测升级为 Error
`PhysicsSchedule` 与 `SubstepSchedule` 都 `.set_executor(SingleThreadedExecutor::new())`
且 `ambiguity_detection: LogLevel::Error`
（`src/schedule/mod.rs:88-94`；`src/dynamics/solver/schedule.rs:59-70`）。
含义：Avian 用「集合顺序 + 单线程」保证确定性，而不是靠自动并行。

### 1.13 碰撞层是 `CollisionLayers` + `LayerMask`，**没有** rapier 的 `Group`/`InteractionGroups`
`src/collision/collider/layers.rs:9`（`PhysicsLayer`）、`:86`（`LayerMask`）、`:362`（`CollisionLayers`）。
全库 `InteractionGroups` 零命中。若你按 rapier 的组/掩码模型设计接口，会设计和 Avian 不一样的东西。

---

## 2. 固定步骨架（已逐条核对源码，这是整个路线的脊柱）

四层嵌套，从外到内：

```
固定步内容调度（默认 FixedPostUpdate，src/schedule/mod.rs:52-56）
│  PhysicsSystems 链（src/schedule/mod.rs:74-85）
│  … .before(TransformSystems::Propagate)      ← 物理先算完，再传播 GlobalTransform
│
├─ 1. PhysicsSystems::First          （默认空；debug 下跑 assert_components_finite，:120-124）
├─ 2. PhysicsSystems::Prepare        （physics transform / 质量属性等准备）
├─ 3. PhysicsSystems::StepSimulation → run_physics_schedule（:110-113）→ 运行 PhysicsSchedule
│     │
│     │  PhysicsStepSystems 链（src/schedule/mod.rs:96-107）
│     ├─ First
│     ├─ BroadPhase        AABB 重叠 → ContactGraph 里建 contact pair
│     ├─ NarrowPhase       更新 ContactGraph 里的接触、处理状态变化
│     ├─ Solver            ↓ 见下
│     ├─ Sleeping          控制何时把 body 标记为 Sleeping
│     ├─ Finalize
│     └─ Last              （之后 update_last_physics_tick，:115-118）
│
└─ 4. PhysicsSystems::Writeback      （把 Position/Rotation 写回 Transform）
   5. PhysicsSystems::Last           （默认空）
```

`PhysicsStepSystems::Solver` 内部（`src/dynamics/solver/schedule.rs:32-57`）：

```
SolverSystems 链
├─ PrepareSolverBodies        把刚体拷进求解器专用的 SolverBody（:34）
├─ PrepareJoints
├─ PrepareContactConstraints
├─ PreSubstep                 ← IntegrationSystems::UpdateVelocityIncrements 在这里（integrator/mod.rs:55-57）
├─ Substep                    ← run_substep_schedule，跑 SubstepCount 次（schedule.rs:44, 194）
│     SubstepSchedule 链（schedule.rs:59-70）
│     ├─ IntegrationSystems::Velocity        integrate_velocities → clamp_velocities
│     ├─ SubstepSolverSystems::WarmStart     用上一步冲量预热
│     ├─ SubstepSolverSystems::SolveConstraints   带 bias 求解
│     ├─ IntegrationSystems::Position        integrate_positions
│     ├─ SubstepSolverSystems::Relax         不带 bias 再解一次，抑制过冲
│     └─ SubstepSolverSystems::Damping
├─ PostSubstep                ← IntegrationSystems::ClearVelocityIncrements 在这里（integrator/mod.rs:58-60）
├─ Restitution
├─ Finalize                   把 SolverBody 数据写回刚体
└─ StoreContactImpulses       存接触冲量，供下一步 warm start
```

时间资源：`Time<Physics>`（物理步）、`Time<Substeps>`（子步）、`SubstepCount`、`LastPhysicsTick`、`PhysicsLengthUnit`
（`src/schedule/mod.rs:61-69`；`src/dynamics/solver/schedule.rs:18-25`）。
`run_substep_schedule` 会把通用 `Time` 临时切成子步时钟，子步循环结束后再切回 `Time<Physics>`（`schedule.rs:194-213`）。

**这张图必须背下来**。后面每个里程碑都是在往这张图的某个格子里填东西。

### 2.1 插件组清单（`src/lib.rs:757-789`，按实际添加顺序）

```
PhysicsSchedulePlugin → MassPropertyPlugin → ForcePlugin
→ ColliderHierarchyPlugin → ColliderTransformPlugin
→ [ColliderCachePlugin]                       (cfg: collider-from-mesh + default-collider)
→ ColliderBackendPlugin::<Collider> → ColliderTreePlugin::<Collider> → NarrowPhasePlugin::<Collider>
→ SolverPlugins                               (见下)
→ BroadPhaseCorePlugin → BvhBroadPhasePlugin::<()>
→ JointPlugin → SpatialQueryPlugin → PhysicsTransformPlugin → PhysicsInterpolationPlugin
```

`SolverPlugins` **实际 `build` 顺序**（`src/dynamics/solver/mod.rs:61-83`，注意与同文件 `:33-44` 的文档表格顺序不同）：
`SolverBodyPlugin` → `SolverSchedulePlugin` → `IntegratorPlugin` → `SolverPlugin` → `CcdPlugin` →
`IslandPlugin` → `IslandSleepingPlugin` → `JointGraphPlugin::<FixedJoint/RevoluteJoint/PrismaticJoint/DistanceJoint>`
→ [`JointGraphPlugin::<SphericalJoint>`，3D] → [`XpbdSolverPlugin`，feature `xpbd_joints`]。

一个有用的推论：**碰撞钩子是通过插件泛型注入的** —— `PhysicsPluginsWithHooks<H>` 会 disable 默认的
`BvhBroadPhasePlugin` / `NarrowPhasePlugin<Collider>` 并换成带 `H` 的版本（`src/lib.rs:834-851`）。
所以 broad phase 也参与 `CollisionHooks::filter_pairs`。

### 2.2 最小 3D feature 集

`crates/avian3d/Cargo.toml` 的 `default` 含 `3d, f32, parry-f32, debug-plugin, xpbd_joints, parallel, collider-from-mesh, bevy_scene, bevy_picking`。
只做「碰撞体 + 刚体 + 重力 + 碰撞」需要：`3d`、`f32`、`parry-f32`（=`default-collider`）。
`debug-plugin` 建议保留（调试渲染是最便宜的验证手段）。`collider-from-mesh`、`bevy_scene`、`bevy_picking`、
`xpbd_joints`、`parallel`、`diagnostic_ui` 在第一到第四阶段都可以不要。

---

## 3. 规模盘点（决定每个里程碑的投入）

Avian `src/` 共 **52,307 行**；按下面的口径（**排除所有 `tests.rs` 与 `tests/` 目录**）为 **50,166 行**。
每个文件只归入一个里程碑，故各列可相加：

| 里程碑 | 对应 Avian 源码 | 行数 | 占比 |
|---|---|---|---|
| M1 碰撞体 | `src/collision/collider/`（6,915）+ `src/collider_tree/`（2,944） | **9,859** | 19.7% |
| M2 刚体 | `src/dynamics/rigid_body/` 去掉 `forces/`（5,156，含 `mass_properties` 3,409）+ `src/dynamics/mod.rs`（130） | **5,286** | 10.5% |
| M3 物理驱动 | `src/dynamics/integrator/`（630）+ `src/physics_transform/`（1,731）+ `src/dynamics/rigid_body/forces/`（1,726） | **4,087** | 8.1% |
| M4 碰撞检测 | `broad_phase`（511）+ `narrow_phase`（1,505）+ `contact_types`（1,860）+ `collision_events.rs`（297）+ `hooks.rs`（236）+ `collision/mod.rs`（117）+ `collision/diagnostics.rs`（41） | **4,567** | 9.1% |
| M5 求解器 | `src/dynamics/solver/` 去掉 `xpbd/` | **6,062** | 12.1% |
| M6 进阶 | `spatial_query` 2,884 / `joints` 3,222 / `xpbd` 2,561 / `character_controller` 1,610 / `debug_render` 1,621 / `diagnostics` 1,082 / `ccd` 770 / `picking` 260 | **14,010** | 27.9% |
| 支撑 | `math` 960 + `data_structures` 3,013 + `lib.rs` 852 + `schedule` 608 + `ancestor_marker` 393 + `interpolation` 382 + `utils` 87 | **6,295** | 12.5% |
| | | **50,166** | 100% |

**M1–M5 核心路径 ≈ 29,861 行**。注释密度很高（`integrator/mod.rs` 630 行里 185 行是注释/文档，约 29%），
所以实际需要理解的「代码量」明显低于行数给人的印象。

> 数字用法的建议：**不要把行数当工期**，只当「这一阶段的阅读量和风险量级」。
> 真正的成本在 §5 的移植决策上，而不是抄多少行。

---

## 4. 里程碑路线

每个里程碑统一结构：**学习目标 → 精读清单（带行数）→ 必答问题 → 实现任务 → 验收 → 坑 → 可推迟**。

「必答问题」是这个路线的核心机制：**先读，再合上源码用自己的话回答；答不出来就说明没读懂，不要往下走。**

---

### M0 · 准备与基准（半天）

**学习目标**：把基准固定死，建立「只信检出源码」的习惯。

**要做的事**
1. 保留只读检出，并确认 commit：
   ```bash
   cd /Users/baiaoxiang/KairosEngine/.scratch/avian-research/avian-0.7.0
   git log -1 --format='%H'
   # 965e85bf590f53fbf8a29f7ebd0326b5dbc985d1
   ```
2. 通读 `README.md`（218 行）与 `src/lib.rs:1-260`（getting started + table of contents）——这是唯一的「官方导览」。
3. 读 `migration-guides/0.6-to-main.md`：知道 0.7 相对 0.6 的破坏性变更（Bevy 0.19、`SpatialQueryPlugin` 调度可配置等）。
4. **决定 §5 的移植策略**，并特别注意 §5.6 里那条「**M1 的第一个 collider 提交是全路线不可逆点**」
   ——它比「选 A/B/C」更实际：真正决定后续成本的是**你的组件/set 词汇是否与 Avian 一致**。
5. 在 `kairos_physics` 里建好新模块骨架（`collider/`、`rigid_body/`、`integration/`、`collision/`、`solver/`），
   **与现有 rapier 版并存**，靠 feature flag 或独立 crate 切换。
6. **做一遍「架构通读」**：按 `01-architecture-and-schedules.md` §7 的 **20 条有序精读清单**（带 `file:line`
   的精确区间）从外到内读一遍。那 20 条不是里程碑，而是「一次把骨架看懂」的路线；
   它和下面 M1–M5 的关系是：**先纵览骨架，再按里程碑横向深入**。

**验收**：能一张纸画出 §2 的四层骨架，并说清每一步「输入什么组件、输出什么组件」。

---

### M1 · 碰撞体（第一阶段，最大块）

**学习目标**：理解 Avian 如何用 ECS 表达「形状」，以及形状如何进入空间索引。

**精读清单**（按此顺序）
| 顺序 | 文件 | 行数 | 读什么 |
|---|---|---|---|
| 1 | `src/collision/collider/mod.rs` | 715 | 模块导览、禁用/传感标记、各 marker 组件 |
| 2 | `src/collision/collider/parry/mod.rs` | 1,830 | **`Collider` 本体**（`:356-376`）+ 全部构造函数 |
| 3 | `src/collision/collider/backend.rs` | 619 | `ColliderBackendPlugin` / `ColliderMarker` / 后端无关抽象 |
| 4 | `src/collision/collider/layers.rs` | 472 | `CollisionLayers` / `PhysicsLayer` / `LayerMask` |
| 5 | `src/collision/collider/collider_transform/` | 347 | `ColliderTransform` + 缩放如何进入形状 |
| 6 | `src/collider_tree/update.rs` | 1,024 | 碰撞体如何进入/更新/离开 BVH |
| 7 | `src/collider_tree/tree.rs` + `obvhs_ext.rs` | 890 | `ColliderTree` 本体与 `obvhs` 封装 |
| 8 | `src/collision/collider/constructor.rs` | 901 | `ColliderConstructor` / mesh/scene 派生（可后读） |

**必答问题**
1. `Collider` 上到底还有哪些字段？物理材质为什么不在上面？（→ §1.2）
2. 生成一个裸 `Collider` 后，实体上会多出哪 6 个组件？各自作用是什么？
3. 一个实体的碰撞体世界位姿，是从哪里推导出来的？（对比 rapier 的 standalone collider 语义）
4. `ColliderAabb` 何时被重算？旋转变了会重算吗？缩放呢？
5. collider tree 的 `ProxyKey` 是什么？为什么更新要「批量/增量」而不是每帧重建？
6. `CollisionLayers` 的掩码测试发生在流水线的哪一步？

**实现任务**（`kairos_physics`）
- `Collider` 组件：只放形状。**注意 Avian 0.7.0 里 `Collider` 是一个具体类型（Parry 支撑），
  不是 `ColliderShape` 枚举**；可替换性来自 trait 抽象（`AnyCollider` / `SimpleCollider` / `ScalableCollider`，
  `src/collision/collider/mod.rs:128-342`）与泛型 `ColliderBackendPlugin<C>`（`src/collision/collider/backend.rs:68-248`）。
  你第一版可以只做一个具体类型，把 trait 抽象留到需要第二种后端时再抽；
- 用 `#[require]`（若 `kairos_ecs` 支持）或 setup system 自动挂上：`ColliderAabb`、`CollisionLayers`、`ColliderDensity`、`ColliderMassProperties`、`ColliderMarker`；
- 构造函数集：`sphere` / `cuboid` / `capsule` / `cylinder` / `cone` / `half_space`（先做这 6 个）；
- AABB 计算 + dirty 追踪；
- 空间索引：**先用简单网格或暴力配对**，把 BVH 留到 M4（见「可推迟」）。

**验收**
- 在测试里 spawn 一个静态 cuboid，断言：组件齐全、AABB 正确、位姿正确；
- 旋转该实体后 AABB 会更新；
- `kairos_physics` 现有 `insert_immovable_box` 的语义（世界空间尺寸、setup 时写一次位姿）能被新 API 表达。

**坑**
- 别把「世界空间尺寸」和「本地半长」混起来：Avian 用 `ColliderTransform` 的 scale 进入形状，Kairos 现在是在调用侧把 scale 折进 half_extents（`kairos_game.rs:243-248` 有注释说明）。重建时要明确选一种并写进文档。
- `ColliderAabb` 的更新时机如果排错，会在 M4 表现为「穿模」而不是报错。
- **BVH 里存的是 `EnlargedAabb`，不是 `ColliderAabb`**（`src/collider_tree/update.rs:466, 1007`）；
  `EnlargedAabb::update` 的返回值就是「是否需要更新树」的信号（`src/collision/collider/mod.rs:592-602`）。
  若 M1 就上 BVH，先照抄 `ColliderTreeProxyKey` 的位打包（`ProxyId` 30 bit + `ColliderTreeType` 2 bit 塞进 `u32`，
  `src/collider_tree/proxy_key.rs:22-26`）——这样从 1 棵树扩到 4 棵树不需要改组件布局。

**可推迟**：trimesh、convex decomposition、`ColliderConstructorHierarchy`、mesh 派生、传感器细节、BVH（先用暴力配对）。

> 更细的碰撞体事实（152 处带行号引用、必需组件的三个来源、层掩码的唯一调用点、11 个 observer 的生命周期）见 `02-colliders.md`。

> ⚠️ **M1 是全路线杠杆最高的一个里程碑**（见 §5.6）：一旦你定下自己的 `Collider` / `ColliderAabb` /
> `ColliderTrees` proxy 布局 / `Position`·`Rotation` 词汇，后面 6,000+ 行 `collision/collider` 与
> 2,000 行 `physics_transform` 就不再能直接 vendor。**所以在 M1 就坚持用 Avian 的组件名与 system set 名。**
> 具体做法：命名照抄，`obvhs` 用 **0.3.3**（对齐 glam 0.33，见 §5.3），
> 物理位姿按 Avian 用独立的 `Position`/`Rotation` 组件而不是复用 `LocalTransform`。

---

### M2 · 刚体（第二阶段）

**学习目标**：理解刚体是什么、质量属性从哪来、以及 ECS 里「body 与它的 collider 如何建立关系」。

**精读清单**
| 顺序 | 文件 | 行数 | 读什么 |
|---|---|---|---|
| 1 | `src/dynamics/rigid_body/mod.rs` | 662 | `RigidBody` 枚举（`:284-320`）、marker 组件、`on_add` 钩子（`:322-325`） |
| 2 | `src/dynamics/rigid_body/mass_properties/mod.rs` | 933 | 质量属性组件的组织与插件 |
| 3 | `src/dynamics/rigid_body/mass_properties/components/mod.rs` | 1,133 | 每个质量组件的定义 |
| 4 | `src/dynamics/rigid_body/mass_properties/components/computed.rs` | 1,028 | 计算型质量属性 |
| 5 | `src/dynamics/rigid_body/mass_properties/components/collider.rs` | 94 | collider 侧的质量输入 |
| 6 | `src/dynamics/rigid_body/mass_properties/system_param.rs` | 221 | 质量属性系统参数 |
| 7 | `src/dynamics/rigid_body/world_query.rs` | 173 | `RigidBodyQuery` 之类的查询抽象 |
| 8 | `src/dynamics/rigid_body/locked_axes.rs` | 307 | 轴向锁定（可后读） |
| 9 | `src/physics_transform/mod.rs` | 349 | `Position` / `Rotation` 与 Transform 的关系 |

**必答问题**
1. `RigidBody` 三个变体各自的物理语义？（谁被力影响、谁有速度、谁影响别人）
2. 质量属性有哪几个组件？哪些是用户写的、哪些是引擎算的？（区分 `Mass` vs `ComputedMass` 这类命名）
3. 质量什么时候重算？如果在 `Added`/`Changed` 上重算，为什么不会每帧重算？
4. 一个刚体有多个 collider 时，质量和惯量怎么合并？密度从哪读？
5. `RigidBody` 与它的 `Collider` 之间是父子关系还是标记关系？引擎怎么找到「这个 body 的所有 collider」？
6. `Position`/`Rotation` 为什么不是直接复用 `Transform`？（→ 与 `physics_transform` 的分工）

**实现任务**
- `RigidBody` 枚举组件（`Dynamic` / `Static` / `Kinematic`）+ 谓词方法；
- 运动状态组件：`Position`、`Rotation`（或直接复用 `LocalTransform`，需明确决策）、`LinearVelocity`、`AngularVelocity`；
- 质量属性：`ColliderDensity`、`ColliderMassProperties`、`ComputedMass`、`ComputedCenterOfMass`、`ComputedAngularInertia`（这些名字已核对存在）；
  注意覆盖机制：**没有** `AdditionalMassProperties`（0.7.0 已删除），要覆盖就直接插入 `Mass` / `AngularInertia` / `CenterOfMass`
  （`src/dynamics/rigid_body/mass_properties/components/mod.rs:160, 326, 536, 915`）；
- 从形状算体积与惯量的代码（`parry3d` 有现成能力可调用）；
- 质量重算系统，按 Avian 的 `Changed` 过滤策略挂到固定步的 `Prepare` 位置。

**验收**
- 一个 `Dynamic` 球 + 一个 `Static` 平面：断言球的 `ComputedMass`、球心、惯量数值正确（可用解析值手算对照）；
- 多个 collider 合并质量的数值测试；
- 改密度后质量会重算；不改则不会重算（用 change detection 计数验证）。

**坑**
- **质量必须在求解器之前就绪，这是硬约束**。Avian 的确切位置（可直接照抄）：
  质量重算跑在 `PhysicsSystems::Prepare` 里，且**必须 `after(PhysicsTransformSystems::TransformToPosition)`**
  ——否则质心是用上一帧位姿算的（`src/dynamics/rigid_body/mass_properties/mod.rs:299-309, 333-341`）。
  完整链路是：
  ```
  Prepare { TransformToPosition → 质量三连(UpdateColliderMassProperties → QueueRecomputation → UpdateComputedMassProperties) }
  PhysicsSchedule { BroadPhase → NarrowPhase → Solver → Sleeping → Finalize }
  Writeback { PositionToTransform }
  ```
  求解器读的是 `ComputedMass` / `ComputedAngularInertia`（`src/dynamics/solver/solver_body/plugin.rs:188-190`）。
  **放在积分之后 → 「质量滞后一帧」的隐蔽 bug**；放在 `TransformToPosition` 之前 → 质心错位。
- 别在每帧无条件重算质量：Avian 用 dirty 标记（`RecomputeMassProperties`）+ change detection
  （`Changed<ColliderMassProperties>` / `Changed<ColliderTransform>` 等）过滤，
  且用 `Without<ColliderOf>` 把「碰撞体的变化」与「刚体自身的变化」分给两个系统，避免重复重算
  （`mass_properties/mod.rs:362-396`）。这是性能与睡眠正确性的前提。
- `LockedAxes` **不改** `ComputedAngularInertia`，而是清零求解器侧逆惯量矩阵的对应行列
  （`src/dynamics/rigid_body/locked_axes.rs:266-280` → 无限惯量）。别试图用改惯量的方式实现锁定。

**可推迟**：`LockedAxes`、质量覆盖（直接插 `Mass`/`AngularInertia`/`CenterOfMass`，**没有** `AdditionalMassProperties`）、dominance、多 collider 的复杂合并（先支持 1 个）。

---

### M3 · 刚体物理驱动（第三阶段：重力、力、积分）

> ⚠️ **前置依赖（Kairos 侧缺口，先确认）**：Avian 的 `transform_to_position` 读的是 **`GlobalTransform`**，
> 并依赖 bevy_transform 的传播系统（`mark_dirty_trees` / `propagate_parent_transforms` /
> `sync_simple_transforms`，`src/physics_transform/mod.rs:26, 98-100`）。
> 而 `kairos_transform/src/lib.rs:17-26` 明写传播系统 **"has not landed yet"**，
> `GlobalTransform` 被标注「不要 spawn、不要读」（`kairos_transform/src/global_transform.rs:13-22`）。
> **对策**：M3 先在**单层实体**（无父子层级）上跑通 push/pull，用 `LocalTransform` 直接对 `Position`；
> 把传播系统当作独立任务补上，不要把它塞进物理里程碑里。

**学习目标**：把 §2 骨架里 `SubstepSchedule` 那一格填满。这是「球会掉下来」的关键阶段。

**精读清单**
| 顺序 | 文件 | 行数 | 读什么 |
|---|---|---|---|
| 1 | `src/dynamics/integrator/mod.rs` | 630 | **全读**。插件、`IntegrationSystems`（`:92-111`）、系统注册（`:52-86`） |
| 2 | `src/dynamics/rigid_body/forces/plugin.rs` | 251 | 力相关系统的调度位置 |
| 3 | `src/dynamics/rigid_body/forces/mod.rs` | 673 | 力/冲量组件的定义与语义 |
| 4 | `src/dynamics/rigid_body/forces/query_data.rs` | 802 | `Forces` 查询接口（应用力的 API 形态） |
| 5 | `src/physics_transform/transform.rs` | 1,275 | 物理位姿 ↔ `Transform` 的双向同步 |
| 6 | `src/dynamics/rigid_body/physics_material.rs` | 452 | 摩擦/恢复系数与合并规则 |
| 7 | `src/dynamics/rigid_body/sleeping.rs` | 153 | 睡眠阈值（先了解，M5 再实现） |
| 8 | `src/schedule/time.rs` | — | `Time<Physics>` / `Time<Substeps>` 如何推进 |

**必答问题**
1. 一个物理步里，重力被计算几次？子步里被应用几次？为什么这样设计？（→ §1.5）
2. `integrate_velocities` 与 `integrate_positions` 谁先谁后？为什么必须是这个顺序？（半隐式欧拉）
3. `UpdateVelocityIncrements` / `ClearVelocityIncrements` 为什么必须跨越整个子步循环？
4. 阻尼在哪里应用？是乘还是减？
   （答案是**隐式形式**，`src/dynamics/integrator/mod.rs:227-232`：`rhs = 1 / (1 + dt * c)`，
   然后速度乘这个 rhs。不是 `v -= c*v*dt`。大 dt 下这个形式更稳定。）
5. 力的两套累加器如何分工？谁每步被清零、在哪里清零？
   （**注意**：0.7.0 里没有 `ExternalForce`/`ExternalImpulse`/`ExternalTorque` 这些组件，
   它们是 `Forces` 这个 `QueryData`（`src/dynamics/rigid_body/forces/query_data.rs:105-120`）；
   持久的是 `ConstantForce`/`ConstantTorque` 一类组件。两个累加器都在 `SolverSystems::PostSubstep` 清空。）
6. 写回方向是单向还是双向？用户直接把 `Transform` 移动了会发生什么？

**实现任务**
- `Gravity` 资源（默认 `(0, -9.81, 0)`）+ 每 body 的 `GravityScale`；
  注意 Avian 把重力当**加速度**直接加进速度增量，**不乘质量**（`src/dynamics/integrator/mod.rs:298`：
  `linear_increment += gravity.0 * gravity_scale`，源码注释明写 “treated as accelerations at this point”）；
  而**力**才走 `mass.inverse() * force`（`src/dynamics/rigid_body/forces/plugin.rs:102`）。这两条别搞混；
- 速度增量机制（Avian 的 `VelocityIntegrationData` 思路）：每物理步算一次重力增量，每子步施加；
- `SubstepCount` 资源（**先用 1**，M5 再调到 6）；
- `integrate_velocities` / `clamp_velocities` / `integrate_positions` 三个系统，注册进子步调度；
- `LinearDamping` / `AngularDamping`；
- `physics_transform`：把物理位姿写回 `LocalTransform`（pull），以及把用户对 `LocalTransform` 的改动推回物理（push，带回环保护）。

**验收**
- **自由落体数值测试**：无碰撞时，`t` 秒后位置与 `½gt²` 的误差在容差内（这是最能证明积分正确的测试）。
  **Avian 自己就有一个可以直接移植的用例**（`src/dynamics/integrator/mod.rs:561-629` 的 `semi_implicit_euler`，
  已逐行核对）：`SubstepCount(1)` + `dt = 0.1s` 跑 **100 步**（共 10 秒）后断言
  ```
  linear_velocity ≈ -Y * 98.1    (epsilon 1e-4)     // 精确：-9.81 × 10
  position        ≈ -Y * 490.5   (epsilon 10.0)     // ½gt²；epsilon 大是因为半隐式欧拉在大 dt 下累积误差
  ```
  注意位置那条容差高达 10——**这本身就是「半隐式欧拉在大 dt 下位置会偏」的证据**。
  建议先照抄这个用例，再把你自己的 `SubstepCount=6` 结果与它对比（应更接近解析解）。
- 衰减测试：有阻尼时收敛到终端速度；
- push/pull 测试：直接改 `LocalTransform` 能推动刚体；物理跑完后 `LocalTransform` 会更新；
- 把 `SubstepCount` 从 1 调到 6，同一场景结果应更稳定（不是完全不同）。

**坑**
- **push 方向无回环保护会导致抖动**：物理 pull 写了 `LocalTransform`，下一帧 push 又把它当「用户改动」写回物理。
  **这是本阶段最容易踩的坑**。Avian 的解法可直接照抄（`src/physics_transform/mod.rs:196-235`，逐条已核对）：
  - 用一个资源记住上次物理步的 tick：`LastPhysicsTick`（`src/schedule/mod.rs:221-223`）；
  - 判定辅助函数 `is_changed_after_tick(comp, last_physics_tick, this_run)`
    = `is_changed() && last_changed.is_newer_than(tick, this_run)`（`src/schedule/mod.rs:225-232`）；
  - push 只在「**不是刚添加** 且 **自上次物理步之后被改过**」时才生效：
    `!position.is_added() && is_changed_after_tick(...)`；
  - 再加**容差**抑制浮点噪声：平移容差 `length_unit * 1e-5`、旋转容差 `0.1°`
    （距离/角度小于容差就当没改）。
  没有这三层（tick + changed-after + 容差），pull 出来的值会在下一帧被当成用户输入推回去，表现为持续抖动。
- 子步时钟：Avian 会把通用 `Time` 临时切成子步（`solver/schedule.rs:194-213`）。若你的系统从 `FixedTime` 直接读 dt，在子步里就会拿到错的 dt。

**可推迟**：陀螺力矩、`clamp_velocities` 的具体上限策略、睡眠、`PhysicsLengthUnit`。

> M2 + M3 的完整细节（质量属性的脏追踪三阶段、力/重力的真实类型名、积分器每子步做什么、
> 以及一张「任务书里这些名字在 0.7.0 不存在」的对照表）见 `03-rigid-bodies-and-dynamics.md`。

---

### M4 · 碰撞检测（第四阶段）

**学习目标**：从「有碰撞体」走到「知道谁和谁接触、接触在哪、法线朝哪」。注意这一阶段产出的是**接触信息**，还不是碰撞响应（响应在 M5）。

**精读清单**
| 顺序 | 文件 | 行数 | 读什么 |
|---|---|---|---|
| 1 | `src/collision/mod.rs` | 117 | 模块导览 |
| 2 | `src/collision/broad_phase/mod.rs` | 209 | `BroadPhaseCorePlugin`（**注意：无 `BroadCollisionPairs`，见 §1.8**） |
| 3 | `src/collision/broad_phase/bvh_broad_phase.rs` | 302 | 默认 BVH broad phase，**直写 `ContactGraph`** |
| 4 | `src/collider_tree/traverse.rs` + `proxy_key.rs` | — | 配对查询如何遍历 BVH |
| 5 | `src/collision/narrow_phase/mod.rs` | 671 | `NarrowPhasePlugin` 与系统顺序 |
| 6 | `src/collision/narrow_phase/system_param.rs` | 834 | `NarrowPhase` 系统参数与对外 API |
| 7 | `src/collision/collider/parry/contact_query.rs` | 613 | 真正调用 Parry 生成接触的地方 |
| 8 | `src/collision/contact_types/mod.rs` | 807 | `ContactPair` / `ContactManifold` / `ContactPoint` |
| 9 | `src/collision/contact_types/contact_graph.rs` | 853 | `ContactGraph`（求解器消费的数据结构） |
| 10 | `src/collision/collision_events.rs` | 297 | `CollisionStart` / `CollisionEnd`（**不是** `…Started`/`…Ended`） |
| 11 | `src/collision/hooks.rs` | 236 | `CollisionHooks` **两个**扩展点（无 `on_collision`） |

**必答问题**
1. broad phase 的输入和输出分别是什么？默认算法是什么？（→ §1.7/§1.8：输入是 AABB + 4 棵 BVH，输出**直接是 `ContactGraph` 的边**）
2. `ContactPair` 与 `ContactGraph` 为什么要分成两个结构？各自给谁用？
3. 接触流形（manifold）是每帧全量重算还是增量更新？点 ID（feature id）用来干什么？
4. `CollisionStart` / `CollisionEnd` 在哪个阶段发出？用户在哪读？为什么它们同时是 `Message` 和 `EntityEvent`？
5. `CollisionHooks` 两个方法分别在流水线哪一步被调用？`filter_pairs` 与掩码测试什么关系？
6. 传感器（`Sensor`）在 narrow phase 与 solver 里的行为差异？

**实现任务**
- 配对系统：**直接写入接触图**（不要再造一个中间候选列表资源——0.7.0 没有 `BroadCollisionPairs`）。
  先用暴力 AABB 两两配对，**M1 若已做 BVH 则直接接上**；
- narrow phase：对每对 candidate 调用 `parry3d` 的接触查询，生成 `ContactManifold`；
- `ContactPair` + 状态迁移（新增/保持/结束）；
- `ContactGraph`：按 body 分组接触（求解器要按 body 取它的所有接触）；
- 最便宜的用户可见信号：**先做 `CollidingEntities`**（Avian `src/collision/collider/mod.rs:704`），
  它不需要调度复杂度；碰撞事件对「只有一两个接触对」的场景是纯开销；
- 碰撞事件（若要，用 `CollisionStart` / `CollisionEnd`）；
- 把 `CollisionLayers` 过滤接进配对阶段（Avian 里层掩码只有**一个**调用点：
  `bvh_broad_phase.rs:257` 的 `layers.interacts_with()`，且它读的是镜像进 proxy 的层副本，不是 ECS 查询）。

**验收**
- 两个重叠的 collider（**至少一个是 dynamic**，见坑）产生一个 `ContactPair`，法线与穿透深度数值正确；
- 分开后 pair 结束并发出 `CollisionEnd`；
- 掩码设置为互斥的两个 collider 不产生 pair；
- 与 rapier 版对比：同样场景下接触法线方向一致（可用现有 demo 做交叉验证）。

**坑**
- **纯静态世界不产生任何配对**：Avian 的 broad phase **直接跳过 static-vs-static**（除非涉及 sensor，
  `src/collision/broad_phase/bvh_broad_phase.rs:124-142`）。
  你现在 demo 里的「静态平面」正属于这一类——所以 M4 的测试**必须**有一个动态物体，否则会误以为配对逻辑坏了。
- **AABB 必须早于 broad phase 更新**。顺序错了会出现「上一帧位置」的配对，表现为高速物体穿透。
  Avian 把 AABB 更新放在 broad phase **内部**、`BroadPhaseSystems::First` 之后、`CollectCollisions` 之前
  （`src/collider_tree/mod.rs:78-84`）。
- `ContactGraph` 按 body 分组是有原因的：求解器要遍历 `body → 它的所有接触`。若你直接在 pair 列表上求解，M5 会需要推倒重来。
- 别在这一阶段就把「接触」当成「碰撞响应」。Avian 在这里**不动速度**，只产出接触数据。
- 别急着做「增量流形更新」：**Avian 0.7.0 自己就是每帧全量重算**（显式让 Parry `clear()`，
  `src/collision/narrow_phase/system_param.rs:698-699` 还留着两处 TODO），暖启动是事后
  `ContactManifold::match_contacts` 按 feature id / 位置匹配（同文件 `:789-798`）。照全量重算做，你不会比 Avian 差。

**可推迟**：空间查询（raycast/shape cast）、`CollisionHooks` 的**两个**扩展点（先只做掩码）、增量流形更新（Avian 也没做）、warm start 所需的冲量存储、debug render、diagnostics、`ContactGraph` 的三重存储优化（先只用最简形态）。

> M4 是四个里程碑里细节最多的一块：带行号的完整参考见 `04-collision-detection.md`，
> 其中 §0 有一张「任务书假设的 API 名在 0.7.0 不存在」的对照表（`BroadCollisionPairs`、`ContactMatch`、
> `CollisionStarted`、`QueryFilterFlags`、`SpatialQueryPipeline` 等全部要按该表替换）。

---

### M5 · 求解器与岛屿/睡眠（第五阶段：让碰撞真正发生响应）

**学习目标**：把接触变成速度修正，让球落地后弹起/停住。这是「球落地」的最后一块。

**精读清单**
| 顺序 | 文件 | 行数 | 读什么 |
|---|---|---|---|
| 1 | `src/dynamics/solver/schedule.rs` | 213 | **全读**。`SolverSystems`（`:93`）、`SubstepSchedule`（`:59-70`）、`SubstepSolverSystems`（`:134`） |
| 2 | `src/dynamics/solver/plugin.rs` | 806 | `SolverPlugin` 与系统注册 |
| 3 | `src/dynamics/solver/solver_body/mod.rs` | 520 | `SolverBody`：为什么要把刚体拷进求解器专用类型 |
| 4 | `src/dynamics/solver/solver_body/plugin.rs` | 362 | solver body 的构建与写回 |
| 5 | `src/dynamics/solver/contact/mod.rs` | 456 | 接触约束 |
| 6 | `src/dynamics/solver/contact/normal_part.rs` | 167 | 法向冲量 |
| 7 | `src/dynamics/solver/contact/tangent_part.rs` | 245 | 切向（摩擦）冲量 |
| 8 | `src/dynamics/solver/constraint_graph.rs` | 314 | 约束图 |
| 9 | `src/dynamics/solver/islands/mod.rs` | 1,410 | 岛屿构建（真实类型：`PhysicsIslands` 资源 + `PhysicsIsland` 组件 + `BodyIslandNode`，`:209, 414, 1313`；**没有** `IslandManager`） |
| 10 | `src/dynamics/solver/islands/sleeping.rs` | 626 | 睡眠/唤醒（阈值类型是 `SleepThreshold`，`:84`；`SleepingThreshold` 只是废弃别名） |
| 11 | `src/dynamics/solver/softness_parameters/mod.rs` | 100 | 软约束参数 |

**必答问题**
1. 为什么需要 `SolverBody` 而不是直接解刚体？（数据局部性？并行？统一 static/dynamic？）
2. 子步循环里 `WarmStart → SolveConstraints → Position → Relax` 这个顺序各自解决什么问题？
   为什么 `Relax` 要**不带 bias** 再解一次？
3. `StoreContactImpulses` 为什么要在 `SolverSystems` 的**最后**？（下一帧 warm start）
4. 岛屿是什么？为什么要分岛屿？睡眠的判定阈值与唤醒触发条件分别是什么？
5. 恢复系数（restitution）为什么单独放在 `SolverSystems::Restitution`，而不是在接触约束里？
6. 单线程执行器 + 集合链在这里起的确定性作用？

**实现任务**
- `SolverBody` 中间表示 + 准备/写回系统；
- 接触约束：法向冲量（含 bias / Baumgarte 项）+ 切向摩擦冲量；
- 子步循环 `run_substep_schedule`，`SubstepCount` 从 1 提到 6；
- warm start 与接触冲量留存；
- 岛屿构建 + 睡眠/唤醒（可最后做）；
- 恢复系数应用。

**两个必须照抄的实现细节（容易被忽略，但错了就是「抖动/漂移」）**
1. **写回位置必须绕质心补偿**（`src/dynamics/solver/solver_body/plugin.rs:276-284`，已核对）：
   ```rust
   let old_world_com = *rot * com.0;
   *rot = (solver_body.delta_rotation * *rot).fast_renormalize();
   let new_world_com = *rot * com.0;
   pos.0 += solver_body.delta_position + old_world_com - new_world_com;
   ```
   少了 `old_world_com - new_world_com` 这一项，质心不在原点的物体会在旋转时「漂移」。
2. **Avian 的「睡眠」是靠移除 `SolverBody` 组件实现的，不是打标志位**
   （`src/dynamics/solver/solver_body/plugin.rs:93-99`：observer 监听 `On<Add, (Disabled, RigidBodyDisabled, Sleeping)>`
   → `remove_solver_body`）。即「被求解的集合」由组件是否存在决定。
   若你改用标志位，就必须在**所有**积分与求解查询里手动过滤，否则睡着的物体会继续被积分。

**验收**
- **球落地测试**：`restitution = 0.8` 的球落到平面后反弹高度比例接近 0.64（能量按 e² 衰减的近似），并最终静止；
- **静止测试**：球停在平面上不掉下去、不抖动（穿透深度在容差内且速度收敛到 0）；
- **稳定性测试**：一摞 5 个盒子堆叠 5 秒不炸（子步 > 1 时应明显更稳）；
- 睡眠测试：静止后 body 被标记 `Sleeping`，被新物体撞到会唤醒。

**坑**
- **warm start 必须存冲量**，否则收敛慢、堆叠会软。
- **`Position` 积分在约束求解中间**（不是最后）：这是 Avian/XPBD 式顺序的关键，顺序错了堆叠会抖。
- 恢复系数在约束里直接做会和 bias 打架，所以 Avian 把它单独放到 `Restitution` 阶段。

**可推迟**：XPBD joints（feature `xpbd_joints`）、motors、joint graph、约束阻尼、多线程、CCD、`Relax` 迭代调优。

> 求解器与岛屿/睡眠的完整参考（`SolverBody` 中间表示的理由、接触约束的软度参数、
> `PhysicsIslands`/`PhysicsIsland`/`BodyIslandNode` 的真实形态、CCD 与材质）见 `03-rigid-bodies-and-dynamics.md` §7–§9。

---

### M6 · 进阶（按需，优先级从高到低）

| 子项 | Avian 源码 | 行数 | 为什么值得做 |
|---|---|---|---|
| 空间查询 | `src/spatial_query/`（`system_param.rs` 1,293 是关键） | 2,884 | 射线拾取、相机碰撞、地面检测；编辑器最常用 |
| Debug 渲染 | `src/debug_render/` | 1,621 | **性价比最高的调试工具**，建议提前到 M4 就少量引入 |
| Diagnostics | `src/diagnostics/` | 1,082 | 计数与计时，定位性能问题 |
| CCD | `src/dynamics/ccd/mod.rs` | 770 | 高速穿透；先了解 `SweptCcd` 组件（`solver/mod.rs:39`） |
| 关节 | `src/dynamics/joints/` | 3,222 | 约束类玩法；依赖 M5 的约束图 |
| 角色控制器 | `src/character_controller/`（`move_and_slide.rs` 1,130） | 1,610 | 依赖空间查询 + M5 |
| 插值 | `src/interpolation.rs` | 382 | 固定步与渲染帧率解耦的平滑（`PhysicsInterpolationPlugin`） |

---

### 4.1 依赖关系图（不要跳步）

```
M1 碰撞体 ──→ M2 刚体 ──→ M3 物理驱动 ──→ M4 碰撞检测 ──→ M5 求解器 ──→ M6 进阶
   │              │              │                │              │
  空间索引       质量属性       AABB 更新        配对待求解      接触约束
                必须早于      必须早于          必须早于        依赖
                求解器        broad phase       narrow phase    ContactGraph
```

**允许的并行/提前项**：
- `debug_render` 可以在 M4 就引入（看得到 AABB 和接触点，能省掉大量猜测）；
- `spatial_query` 的 raycast 可以在 M4 后独立做（不依赖求解器）；
- 空间索引（BVH）可以从 M1 延后到 M4，先用暴力配对。

**不允许的跳步**：
- 跳过 M2 的质量直接做 M5 → 冲量算不对；
- 跳过 M4 的 `ContactGraph` 直接做 M5 → 求解器输入形态不匹配，要重写；
- 跳过 M3 的 push/pull 回环保护 → 到 M5 会表现为无法解释的抖动。

---

## 5. 移植策略（最关键、也最容易做错的决策）

> 详细依赖面清点见 `05-bevy-dependency-surface.md`。本节给结论与决策方法。

> 本节结论已逐项核对**两侧源码与 `Cargo.lock`**，不是估算。原始清点见 `05-bevy-dependency-surface.md`。

### 5.1 好消息：`kairos_ecs` 是 `bevy_ecs` 0.19 的**完整 fork**，Avian 要的 API 同名同在

先纠正一个前提（来自 `05-bevy-dependency-surface.md` §0）：**Avian 0.7.0 不依赖任何 `bevy_*` 子 crate**。
它只依赖 umbrella crate `bevy 0.19.0`（`crates/avian3d/Cargo.toml:82-85`，`default-features = false`，
features `["std", "bevy_log"]`）加一个直接的 `bevy_math`（`:86`）。代码里写的是 `bevy::ecs::…`、
`bevy::transform::…`；全 `src/` 里 `bevy_ecs` 这个词出现 **0 次**。
所以「把 Avian 移到 `kairos_ecs`」在实现上等于「把 `bevy::ecs::X` 改名成 `kairos_ecs::X`」。

| Avian 依赖的 ECS 能力 | Avian 侧用例 | `kairos_ecs` 现状 | 结论 |
|---|---|---|---|
| 必需组件 `#[require(...)]` | `src/collision/collider/parry/mod.rs:358-365` | `kairos_ecs/src/component/required.rs:123`、`register_required` `:599`；派生宏支持 `#[require]`（`kairos_ecs/macros/src/lib.rs:756, 832`） | ✅ **有** |
| 组件生命周期钩子 | `RigidBody::on_add`（`src/dynamics/rigid_body/mod.rs:322-325`） | `kairos_ecs/src/lifecycle.rs:166`（`ComponentHooks`）、`on_add` `:202`、`on_discard` `:242` | ✅ **有** |
| 关系 / 层级（body 找它的 collider） | `ColliderHierarchyPlugin`（`src/lib.rs:763`） | `kairos_ecs/src/relationship.rs:123`（`Relationship`）、`:285`（`RelationshipTarget`） | ✅ **有** |
| 观察者 | `src/observer/`（如 `add_observer`） | `kairos_ecs/src/observer/`（`runner.rs`、`system_param.rs` 等） | ✅ **有** |
| change detection + tick | `LastPhysicsTick` + `is_changed_after_tick`（`src/schedule/mod.rs:220-229`） | change detection 与 tick 齐备 | ✅ **有** |
| `SystemParam` / `QueryData` 派生 | `Forces`、`NarrowPhase`、`SpatialQuery` 等 | 相关引用 636 / 693 处 | ✅ **有** |
| `Message`/`MessageReader`、`EntityEvent`、`ParallelCommands`、`par_iter_mut`、lifetimeless `Read`/`Write`/`SQuery`/`SRes`、`Single`、`Populated`、`Disabled` | 遍布核心路径 | 逐条核对**全部存在且同名**（见 `05-bevy-dependency-surface.md` §6.1） | ✅ **有** |
| `App` / `Plugin` / `PluginGroup` | 28 个 `impl Plugin` + 2 个 `impl PluginGroup`，~150 处 `app.*` 调用 | 整个 workspace 查无 `pub trait Plugin` | ⚠️ **缺**（但是"层"，不是"能力"） |

**一个关键的量级事实**：`kairos_ecs` 有 **238 个 `.rs` / 113,063 行**，模块目录与 `bevy_ecs 0.19` 一一对应。
这不是「风格相似的小 ECS」，而是 **fork**——所以**源码级改名在这个组合下是机械的**（这跟一般情况不同）。

**唯一的结构缺口**是 `App`/`Plugin`/`PluginGroup`。两条出路等价：
给 `World + Schedules` 套一个薄 `App` shim（「估算」300–600 行，能吸收绝大多数 `app.*` 调用点），
或把每个 plugin 改写成 `install(world, schedule)` 函数（Kairos 已有这个范式：
`kairos_physics/src/lib.rs:317`、`kairos_engine/src/kairos_editor.rs:45`）。

### 5.2 最关键的反直觉事实：**Avian 的算法核心几乎与 Bevy 无关**

这一条改变了成本判断（`05-bevy-dependency-surface.md` §0/§1 实测）：

| 模块 | 总行数 | 其中出现 `bevy` 的**非注释代码行** |
|---|---|---|
| `src/dynamics/solver/`（含 islands/contact/joint_graph/xpbd/solver_body） | 8,623 | **30** |
| `src/collider_tree/` | 2,944 | **7** |
| `src/collision/narrow_phase/` | 1,505 | **2** |
| `src/collision/broad_phase/` | 511 | **2** |
| `src/dynamics/integrator/` | 630 | **5** |
| `src/dynamics/ccd/` | 770 | **2** |
| 全 `src/` 合计 | 52,307 | **205**（分布在 114 个文件） |

另有 **12 个文件完全不含 `bevy`**（3,567 行），包括 `src/data_structures/graph.rs`（1,114 行）。
**结论：要改的不是物理，是胶水层与数学类型。**

### 5.3 坏消息：三个**真正的**硬阻塞（不是胶水，是缺子系统）

| # | 阻塞 | 证据 |
|---|---|---|
| 1 | **泛型时钟 `Time<T: Clock>` 缺失**。Avian 需要 `Time<Physics>`（`src/schedule/mod.rs:61`）、`Time<Substeps>`（`src/dynamics/solver/schedule.rs:20, 199-205`），并在 `run_physics_schedule` 里来回换 generic `Time`（`src/schedule/mod.rs:236-283`）。Kairos 的 `Time`（`kairos_time/src/lib.rs:36`）与 `FixedTime`（`:178`）是**两个具体结构体**，无 `Clock` trait、无 `Time<T>`、无 `advance_by`/`as_generic` | `05-…` §0/§7.1 |
| 2 | **Transform 传播系统在 Kairos 侧不存在**。Avian 直接调用 bevy_transform 的 `mark_dirty_trees`/`propagate_parent_transforms`/`sync_simple_transforms`（`src/physics_transform/mod.rs:26, 98-100`）并用 `TransformHelper`（`src/physics_transform/helper.rs:9, 33`）。而 `kairos_transform/src/lib.rs:17-26` 明写传播系统 **"has not landed yet"**，`GlobalTransform` 被标注「不要 spawn、不要读」（`kairos_transform/src/global_transform.rs:13-22`） | `05-…` §6.2 |
| 3 | **`glam_matrix_extras 0.3.0`（最新版）把 glam 钉死在 0.32**（Avian lock 解析为 `glam 0.32.1`）。而 `SymmetricMat2/3` 在 Avian 核心被用了 **73 处非注释代码**（angular inertia 的存储类型，`src/dynamics/rigid_body/mass_properties/components/*.rs`）→ **必须 fork 或重写**，没有升级路径 | `05-…` §4/§7.1 |

阻塞 2 对你尤其重要：**它是 M3 的前置依赖**。Avian 的 `transform_to_position` 读的是 `GlobalTransform`；
若 Kairos 的传播系统还没落地，M3 的 push/pull 就得先解决这一层（或先在单层实体上跑通）。

**其余版本事实（已核对）**：
| 类型/库 | Avian 0.7.0 | Kairos 现状 | 影响 |
|---|---|---|---|
| `glam` | **0.32.1**（`bevy_math 0.19`） | **0.33.1**（`kairos_math`） | ⛔ semver 不兼容（0.x）：两个 glam 的 `Vec3`/`Quat` 是**不同类型** |
| `glamx`（Parry 的 glam 抽象） | 0.2.0（→ glam 0.32.1） | 0.3.0（→ glam 0.33.1） | 同上 |
| `parry3d` | **0.27.0** | **0.28.0** | 各自绑定不同 glam；0.27→0.28 还有 API 漂移（`PersistentQueryDispatcher` 等） |
| `obvhs`（BVH） | 0.3.1（→ **glam 0.31.1**，第三套！） | 未使用 | ✅ **有解**：obvhs 最新 **0.3.3** 允许 `glam >=0.30.10,<0.34`，直接支持 glam 0.33 → 升级而非 fork |
| `bevy_heavy 0.5.0` | 质量属性类型（`MassProperties` 等） | 无对应物 | ✅ 依赖是 `approx, bevy_math, bevy_reflect, glam_matrix_extras, serde`——**不含 `bevy_ecs`**，是**纯数学 crate**，跟着 `bevy_math` 走 |
| `bevy_transform_interpolation 0.5.0` | 插值 | 无 | ⛔ 依赖 umbrella `bevy` → **ECS 强耦合，只能重写**；但它**完全可以延后**（只是视觉平滑） |
| `bevy_reflect` | 171 个 `#[derive(Reflect)]` + 161 个 `#[reflect(...)]` | `kairos_ecs/src/reflect.rs` 是 1 字节空文件 | ✅ **可以删而不是实现**：全 `src/` 只有 **3 处**非注释 `register_type` 调用，且**零个 `Reflect` trait bound** |

**由此得出两条确定的选型结论**：
1. Kairos 的 `kairos_math` 基于 **glam 0.33**，因此应当**继续使用 `parry3d 0.28`**
   （其 `glamx 0.3.0` 正好对齐 glam 0.33），**不要**为对齐 Avian 降到 `parry3d 0.27`。
2. 存在一个**要把 `bevy_math` 直接引入 physics crate** 的变体（(b')，见 `05-…` §7.2）：
   它能让 Avian 的数学代码几乎原样编译，但代价是**双 glam + 双 parry + 必须实现 Reflect**，
   并长期背上一个 glam 0.32 分支。**除非你愿意把主 glam 降到 0.32（会波及 `kairos_engine`），否则不推荐。**

### 5.4 三条可选路线（成本已按 §5.1–§5.3 的实测修正）

| 路线 | 做法 | 优点 | 代价 |
|---|---|---|---|
| **A. Vendor 后改写** | 拷 Avian `src/`（128 文件 / 52,307 行）进 Kairos，把 `bevy::ecs` / `bevy::transform` / `bevy_math` 用法逐处改名 | **算法层几乎可直接落地**（§5.2：solver 8,623 行只碰 30 行 bevy）；拿到成熟算法与调度结构 | 111 个文件要改 import、146 条 `use bevy…`；`App`/`Plugin` 层 1–2 周；§5.3 的三个硬阻塞；**每个数学边界要转换 glam 0.32↔0.33**；上游升级需重复改写 |
| **B. 依赖替换/抽象层** | 保留 Avian 架构，写 Kairos 原生等价物垫片 | 改动集中在接口层 | **crate 级别名做不到**：`#[derive(Component)]` 展开成具体的 `bevy_ecs::…` trait，Rust 无「crate 别名 + derive 重定向」；所以 B 的真实形态就是 A（源码级改名）。真正非机械的只有 4 项：`Time<T:Clock>`、Transform 传播、`App`/`Plugin`、数学层替身 |
| **C. 学架构、原生实现** | 只借算法与调度骨架，代码自己写；几何直接调 `parry3d 0.28` | 与 Kairos 的 ECS/数学风格一致；无 glam 版本转换层；无上游同步债；**与「边学边建」的诉求完全吻合** | 求解器/岛屿/睡眠的成熟度要自己爬；短期正确性风险高 |

### 5.5 本路线图的建议：**C 打骨架 + 随时可切 A**

**建议以 C（学架构、原生实现）为主线，但把 A 当作随时可用的加速器。**
关键洞察是：**`kairos_ecs` 与 `bevy_ecs 0.19` 同名同类**（§5.1），
所以你现在用**Avian 的确切词汇**写 Kairos 原生代码（同样的组件名、同样的 system set 名、同样的调度结构），
将来想改成 vendor 就还是「改名 + 补缺」，而不是「重写」。

理由按强度排序：

1. **与你的诉求字面吻合**：你要的是「学明白 avian，再一点点构造自己的物理模块」——C 就是这个过程本身。
2. **ECS 支撑不是风险**（§5.1）：`kairos_ecs` 是完整 fork，Avian 最吃基础设施的点（必需组件、钩子、
   关系、观察者、change detection、`SystemParam`/`QueryData`、`par_iter_mut`）全都在且同名。
   **Avian 难的部分（物理与调度设计）值得学；容易的部分（ECS 支撑）你已经有了。**
3. **真正的成本在数学与子系统，不在物理**（§5.2/§5.3）：C 让你只在**自己的边界**上付一次转换成本，
   而 A 要你在**每一处几何调用**上付。
4. **三个硬阻塞都无法靠"选路线"绕开**：泛型 `Time<T>`、Transform 传播、`glam_matrix_extras`——
   无论 A 还是 C 都要面对；C 至少可以**推迟**它们（见下）。
5. `bevy_transform_interpolation` 依赖整个 `bevy`，M6 的插值无论哪条路线都要自写；好消息是它可完全延后。

**三条必须保留的例外（否则 C 的正确性风险会很高）**：
- **几何层直接调 `parry3d 0.28`**：形状定义、AABB、GJK/EPA、接触流形生成全部复用。
  Avian 自己就是这么做的（§1.3），这不是偷懒，而是 Avian 架构本身的组成部分。
- **质量属性只重写类型，不重写算法**：`bevy_heavy` 是**纯数学 crate**（不含 `bevy_ecs`），
  用解析公式/parry 现成能力即可，别自己推导惯量张量。
- **`Reflect` 直接删**：全 `src/` 只有 3 处非注释 `register_type`、零个 `Reflect` trait bound。

**可以安全推迟的**（因此 C 的 M1–M3 能很快跑起来）：
`glam_matrix_extras`/`SymmetricMat3`（M2 才需要 angular inertia 的存储类型）、
泛型 `Time<T>`（M3 先直接传 dt 也行）、Transform 传播（先在单层实体上跑通 push/pull）、
`bevy_transform_interpolation`（纯视觉，可最后做）。

**时间量级「估算」**（`05-…` §7.1，单工程师，含阅读/改写/调试，不含渲染联调）：
到「colliders → rigid bodies → integration → collision detection 可用」约 **10～18 周**。
这是按分项相加的估算，**没有历史数据支撑，不要当承诺**。

**何时该改主意选 A**：如果出现这两种情况之一——
(a) 你发现自己在 M5 的求解器数值调优上耗时远超预期，愿意用「改名 + 类型搬家」换成熟实现；
(b) 你在 M1 就发现自己的组件词汇与 Avian 分叉太远，导致后续无法再利用它的代码。


### 5.6 决策点与不可逆性

| 时点 | 决策 | 不可逆程度 |
|---|---|---|
| M0 | 走 A / B / C；确认 `parry3d` 留在 0.28、glam 留在 0.33 | 高（改动整个 crate 结构） |
| **M1（第一个 collider 提交）** | **你自己的 `Collider` / `ColliderAabb` / `ColliderTrees` proxy 布局 / `Position`·`Rotation` 词汇一旦定下，后面 6,000+ 行 `collision/collider` 与 2,000 行 `physics_transform` 就不再能直接 vendor** | **最高（本路线最重要的决策点）** |
| M1 末 | 空间索引：暴力配对 vs BVH；若用 BVH 是否引 `obvhs`（用 0.3.3 以对齐 glam 0.33） | 中（可替换，但 API 会渗出去） |
| M2 | 物理位姿是复用 `LocalTransform` 还是独立 `Position`/`Rotation` | 中高（决定 push/pull 回环保护的做法） |
| M2 末 | 质量属性是否自实现（`bevy_heavy` 的替代）；是否继续 C | 高 |
| M4 末 | `ContactGraph` 的形态（按 body 分组 vs 平坦列表） | 高（M5 求解器直接依赖它） |
| M5 | 求解器方案：Avian 式（warm start + bias/relax 两段）vs 简化 impulse | 中（数值行为不同，接口可保持） |

> **最重要的那一行值得展开**：真正让 vendor 变贵的不是"写了多少代码"，而是**词汇是否与 Avian 一致**。
> 只要你坚持用 Avian 的组件名（`Collider`/`ColliderAabb`/`ColliderTrees`/`Position`/`Rotation`/
> `ComputedMass`…）与 system set 名（`PhysicsSystems`/`PhysicsStepSystems`/`SolverSystems`…），
> C → A 的切换就永远是「改名 + 补缺」。反之，一旦自己发明了一套词汇，
> 后续所有 vendor 都变成重写。**所以在 M1 就按 Avian 的词汇命名，是这份路线图给你的最高杠杆建议。**

> 关于「物理位姿放哪」那一行：**Avian 的答案是「独立组件」**——
> `Position`（`src/physics_transform/transform.rs:48`）与 `Rotation`（同文件 `:745` 3D 本体），
> 独立于 Bevy 的 `Transform`；两者通过 `position_to_transform`（`src/physics_transform/mod.rs:120-122`，
> 在 `PhysicsSystems::Writeback` 里）与 `transform_to_position`（同文件 `:106-110`）双向同步，
> 两个方向都由 `PhysicsTransformConfig` 开关控制（默认都为 `true`，同文件 `:142-161`）。
> 建议 Kairos 照此办理：物理位姿独立于 `LocalTransform`，而不是直接复用——否则 push/pull 的回环判定会没有落脚点。
> 详见 `01-architecture-and-schedules.md` §5。

---

## 6. 与现有 rapier 版 `kairos_physics` 的对接

现有对外契约只有 5 个入口（§0），重建时可以逐个替换，**不需要一次性切换**：

| 现有 | 新版的对应物 | 替换时机 |
|---|---|---|
| `physics::install(world, FixedUpdate)` | 同样的 `install`，内部改为注册 Avian 式插件/系统链 | M1（骨架可以先空跑） |
| `PhysicsEngine::insert_immovable_box` | `(RigidBody::Static, Collider::cuboid(..))` 的 spawn | M1 |
| `PhysicsEngine::insert_movable_sphere` | `(RigidBody::Dynamic, Collider::sphere(..))` | M2 |
| `PhysicsEngine::step`（内含 rapier 全流水线） | `run_physics_schedule`（§2 的四层骨架） | M3→M5 逐步填充 |
| `RigidBody`/`Collider` 持 rapier handle | 组件自身携带数据（不再有句柄间接层） | M1/M2 |
| `on_discard` hooks 回收 rapier 对象 | **不再需要**（数据即组件，ECS 自动回收） | M2 |

**一个重要简化**：现在的 `on_discard_*` 钩子（`lib.rs:233-258`）与 `remove_attached_colliders=false`
这类设计，全部是「rapier 的集合式所有权 + ECS 实体所有权」不一致导致的。
Avian 式设计把这个不一致**从根上消除**了——组件就是数据，实体销毁即数据销毁。
这是重建最直接的收益，值得在 M2 验收时专门确认一遍。

`kairos_game.rs:251-257` 的 demo 是唯一的调用点，改完即完成迁移验证。

---

## 7. 风险清单

| 风险 | 症状 | 缓解 |
|---|---|---|
| `kairos_ecs` 缺乏 Avian 所需的 ECS 能力 | M2 起被迫做 ECS 基础设施 | 已核对：`kairos_ecs` 是 `bevy_ecs 0.19` 的**完整 fork**，Avian 用到的 API 同名同在（§5.1），风险低；真正缺的只有 `App`/`Plugin` 层，可退化为 `install` 函数 |
| glam 版本不一致（0.32 vs 0.33） | 几何类型无法互通，被迫到处转换 | 这是**选 C 的主要理由**（§5.5）：Kairos 留 glam 0.33 + `parry3d 0.28`，不跟随 Avian 的 glam 0.32 |
| `glam_matrix_extras` 钉死 glam 0.32（73 处核心用法） | M2 的 angular inertia 存储类型无法复用 | **M2 之前就要决定**：fork 它，或自己写 `SymmetricMat3`（§5.3）。不要在 M2 中途才发现 |
| `kairos_time` 无泛型 `Time<T: Clock>` | M3 无法实现 `Time<Physics>`/`Time<Substeps>` 与子步时钟切换 | M3 先直接传 `dt`；把泛型时钟当独立任务（§5.3） |
| `kairos_transform` 传播系统未落地 | M3 的 push/pull 没有 `GlobalTransform` 可读 | 先在单层实体上跑通（§M3 前置）；传播系统单独排期 |
| push/pull 回环抖动 | 无法解释的高频抖动 | M3 就实现 tick/change-detection 保护（§M3 坑，含 Avian 的三层做法） |
| 顺序错配 | 穿透、第一帧异常、堆叠抖动 | 严格照 §2 骨架；每个里程碑用「数值测试」而不是「看起来对」验收 |
| 一步到位做求解器 | 长期跑不起来，失去反馈 | 严格按 M1→M5；`SubstepCount` 先用 1 |
| 自创组件/set 词汇 | 后续无法 vendor Avian，C→A 变重写 | **M1 就按 Avian 的词汇命名**（§5.6），这是杠杆最高的一条 |
| 只读教程不读源码 | 用旧 API（`Fixed`、`BroadCollisionPairs`、`CollisionStarted` 等）反复踩坑 | §1 的 13 条先记住；一切以检出源码为准 |
| 数值不确定 | 测试偶发失败 | 学 Avian：单线程执行器 + 集合链（§1.12），测试用固定 dt |

---

## 8. 参考索引（读码时按这个顺序找）

**架构与调度**
- `src/lib.rs:757-789`（插件组真实顺序）
- `src/schedule/mod.rs:74-85, 88-108, 161-176, 191-214`（四层骨架 + 两个枚举）
- `src/dynamics/solver/schedule.rs:32-70, 93, 134, 185-213`（求解器与子步循环）
- `src/dynamics/integrator/mod.rs:39-111`（积分集合与速度增量）
- `src/dynamics/solver/mod.rs:33-44`（`SolverPlugins` 成员）

**碰撞体**
- `src/collision/collider/parry/mod.rs:356-376`（`Collider` 本体 + `#[require]`）
- `src/collision/collider/backend.rs:68, 256`（后端抽象 / `ColliderMarker`）
- `src/collision/collider/layers.rs`（碰撞层）
- `src/collider_tree/update.rs`（BVH 增量更新）

**刚体与动力学**
- `src/dynamics/rigid_body/mod.rs:284-325`（`RigidBody` 枚举 + `on_add`）
- `src/dynamics/rigid_body/mass_properties/`（质量属性全套）
- `src/dynamics/rigid_body/forces/`（力与冲量）
- `src/dynamics/rigid_body/physics_material.rs`（摩擦/恢复系数）

**碰撞检测**
- `src/collision/broad_phase/bvh_broad_phase.rs`（默认 broad phase）
- `src/collision/narrow_phase/mod.rs` + `system_param.rs`
- `src/collision/contact_types/mod.rs` + `contact_graph.rs`
- `src/collision/collision_events.rs`、`src/collision/hooks.rs`

**示例（对照用法）**
- `crates/avian3d/examples/cubes.rs`（102 行，最小 3D 场景）
- `crates/avian3d/examples/collider_constructors.rs`（65 行）
- `crates/avian3d/examples/trimesh_shapes_3d.rs`、`voxels_3d.rs`、`ccd.rs`、`custom_broad_phase.rs`
- `crates/avian3d/examples/diagnostics.rs`、`debugdump_3d.rs`

**基准事实**
- `crates/avian3d/Cargo.toml`（feature 矩阵、依赖版本、`[lib] path = "../../src/lib.rs"`）
- `README.md:100-120`（`RigidBody::Static` 官方示例）
- `migration-guides/0.6-to-main.md`（0.7 破坏性变更）

---

## 附：本路线图的验证状态

- §1、§2、§3 的事实与行号：**已直接阅读检出源码逐条核对**（`schedule/mod.rs`、`dynamics/solver/schedule.rs`、
  `dynamics/integrator/mod.rs`、`dynamics/solver/mod.rs`、`dynamics/rigid_body/mod.rs`、
  `collision/collider/parry/mod.rs`、`lib.rs`、`crates/avian3d/Cargo.toml`、`README.md`）。
- §3 行数：由 `wc -l` 实测（已排除 `tests.rs` 与 `tests/` 目录），各里程碑互斥且可加总为 50,166。
- §5.1（`kairos_ecs` 是完整 fork）与 §5.3（版本/子系统错配）：**已直接核对两侧源码与 `Cargo.lock`**——
  `kairos_ecs/src/component/required.rs`、`src/lifecycle.rs`、`src/relationship.rs`、`kairos_ecs/macros/src/lib.rs`，
  以及 Avian/Kairos 两侧 `Cargo.lock` 的 `glam`/`glamx`/`parry3d`/`obvhs`/`bevy_heavy`/`bevy_transform_interpolation` 条目。
  **注：初稿曾据一次过窄的 grep 误判 `kairos_ecs` 缺必需组件，已在复核后修正**（见 §5.1）。
- §1 的 13 条、以及 M3/M4/M5 里所有 API 名，**已用 grep 逐条对源码复核**。
  初稿沿用了若干旧版 Avian 的名字，已按下表更正的（每一条都独立验证过）：
  `RigidBody::Fixed`→`Static`、`BroadCollisionPairs`（已删除，broad phase 直写 `ContactGraph`）、
  `CollisionStarted/Ended`→`CollisionStart/End`、`CollisionHooks` 三方法→两方法（无 `on_collision`）、
  `ExternalForce`/`AdditionalMassProperties`/`PhysicsMaterial`/`CoefficientCombineRule`/`IslandManager`
  （均不存在）、`Group`/`InteractionGroups`→`CollisionLayers`/`LayerMask`、
  无 `ColliderShape` 枚举（`Collider` 是 Parry 支撑的具体类型）。
  这些更正是**跨文档交叉核对**的产物——`02`/`03`/`04` 各自独立给出了带行号的反证。
- §5.2/§5.3/§5.4 的实测数字（205 行 bevy 密度、各模块 bevy 行数、10 条阻塞、行数统计）来自
  `05-bevy-dependency-surface.md`，该文档同时对本文初稿的**两个前提做了纠正**：
  (i) Avian 不依赖 `bevy_*` 子 crate，而是只依赖 umbrella `bevy`（`bevy_ecs` token 全 `src/` 零命中）；
  (ii) `kairos_ecs` 不是「风格相似的 ECS」而是 `bevy_ecs 0.19` 的**完整 fork**（238 文件 / 113,063 行）。
  这两条都**降低了**移植成本估计，因此 §5.5 的建议已从「只做 C」调整为「C 打骨架 + 随时可切 A」。
- 第三方 crate 的依赖形状（`bevy_heavy`/`bevy_transform_interpolation`/`obvhs`/`glam_matrix_extras`）来自
  两份 `Cargo.lock` 的解析结果与 docs.rs 的 `Cargo.toml` 原文；
  **这些 crate 本地未 vendored，其 API 细节是从 Avian 的调用点反推的**，见 `05-…` §8。
- 全目录 **1,260 处源码引用已用脚本批量校验**：每一个被引用的文件都存在，且行号落在文件范围内
  （校验过程中修正了 `02-colliders.md` 的一处路径笔误：`src/collision/collider/tree.rs` → `src/collider_tree/tree.rs`）。
- §5 的其余结论（路线取舍）：属**判断**而非事实，量化细节另见 `05-bevy-dependency-surface.md`。
- 未做运行验证：本路线图**没有**编译或运行 Avian（依赖需联网且耗时长），
  所有行为判断来自源码阅读。M1 开始时建议先跑一次官方 `cubes` 示例，把「读到的」和「看到的」对上。
