# Avian 0.7.0 源码研究 03：刚体与动力学（Rigid Body & Dynamics）

- **权威源**：`/Users/baiaoxiang/KairosEngine/.scratch/avian-research/avian-0.7.0`，tag `v0.7.0`，commit `965e85bf590f53fbf8a29f7ebd0326b5dbc985d1`。
- **路径约定**：下列所有 `path/to/file.rs:LINE` 均相对于该 checkout 根目录。`crates/avian3d/Cargo.toml` 设置 `[lib] path = "../../src/lib.rs"`，因此全部引擎代码位于仓库根的 `src/`，由 `avian2d` / `avian3d` 通过 `#[cfg(feature = "2d")]` / `#[cfg(feature = "3d")]` 共享。
- **阅读方式**：纯静态阅读，未执行 `cargo build` / `cargo test`。所有行号来自仓库中的实际文本。
- **约定**：**源码事实**直接陈述并附引用；**推论**以「推论」显式标记。

---

## 0. 先修正若干 API 记忆偏差（对本项目最重要的一节）

任务描述中列出的一些类型名在 v0.7.0 中**并不存在**。读者如果照旧版 Avian（或 Rapier）记忆去实现，会找错 API。逐条核对结果：

| 被提到的名字 | v0.7.0 实际情况 | 依据 |
| --- | --- | --- |
| `RigidBody::KinematicVelocityBased` | **不存在**。`RigidBody` 只有 `Dynamic` / `Static` / `Kinematic` 三个变体 | `src/dynamics/rigid_body/mod.rs:284-304` |
| `RigidBodyMarkers` | **不存在**（无任何匹配） | `grep -rn "RigidBodyMarkers" src/` 无输出 |
| `PreviousPosition` / `PreviousRotation` | **不存在**。位置增量存在 `SolverBody.delta_position` / `delta_rotation`；插值改用基于 `Transform` 的 easing + 私有 `PreviousLinearVelocity` / `PreviousAngularVelocity` | `src/dynamics/solver/solver_body/mod.rs:79-86`；`src/interpolation.rs:304,308` |
| `AccumulatedTranslation` | **不存在**。被 `SolverBody.delta_position` + `PreSolveDeltaPosition` 取代 | `src/dynamics/solver/solver_body/mod.rs:79`；`src/physics_transform/transform.rs:125` |
| `AdditionalMassProperties` | **不存在**。覆盖机制是直接插入 `Mass` / `AngularInertia` / `CenterOfMass` | `src/dynamics/rigid_body/mass_properties/components/mod.rs:160,326,536,915` |
| `PrincipalAngularInertia` 组件 | **不存在**。3D 下是 `AngularInertia { principal: Vec3, local_frame: Quat }` | `src/dynamics/rigid_body/mass_properties/components/mod.rs:536-543` |
| `LocalCenterOfMass` 组件 | **不存在**。局部质心就是 `CenterOfMass`，全局的是 `ComputedCenterOfMass` | `components/mod.rs:915`；`components/computed.rs:776` |
| `ExternalForce` / `ExternalImpulse` / `ExternalTorque` / `ExternalAngularImpulse` 组件 | **不存在**。全部被 `Forces` 这个 `QueryData` 取代 | `grep` 无输出；`src/dynamics/rigid_body/forces/query_data.rs:105-120` |
| `SleepingThreshold` | 已废弃别名，真实类型是 `SleepThreshold`（`SleepingThreshold` 仅为 `pub type` 别名） | `src/dynamics/rigid_body/sleeping.rs:84,99-101` |
| `PhysicsMaterial` 组件 | **不存在**。材质由独立的 `Friction` / `Restitution` 组件 + `DefaultFriction` / `DefaultRestitution` 资源表达 | `grep -rn "PhysicsMaterial" src/` 无输出；`src/dynamics/rigid_body/physics_material.rs:48,59,137,305` |
| `SolverIterations` / `contact_iterations` 资源 | **不存在**。唯一的迭代计数是 `SolverConfig::restitution_iterations` | `src/dynamics/solver/plugin.rs:216-289` |
| `IslandManager` | **不存在**。真实名字是 `PhysicsIslands` 资源 + `PhysicsIsland` 组件 + `BodyIslandNode` | `src/dynamics/solver/islands/mod.rs:209,414,1313` |
| `Ccd` / `CcdEnabled` / `CcdHooks` / `CcdSet` / `CcdSystems` | **全部不存在**。开关组件叫 `SweptCcd`，集合叫 `SweptCcdSystems`，且 CCD 无任何 hooks 扩展点 | `src/dynamics/ccd/mod.rs:270,389`；全仓 grep 其余名字零命中 |
| `CoefficientCombineRule` | **不存在**。真实枚举名是 `CoefficientCombine` | `src/dynamics/rigid_body/physics_material.rs:13` |
| `src/dynamics/solver/joints/` 目录 | **不存在**。关节代码在 `src/dynamics/joints/`；solver 下只有 `joint_graph/` 与 `xpbd/` | `ls src/dynamics/solver/`（无 `joints`）；`src/dynamics/joints/mod.rs:246-272` |
| `ContactConstraintBuilder` / `ContactConfig` / `BaumgarteStabilization` | **均不存在**。Baumgarte 只以注释形式出现在 `normal_part.rs:137`，软度由 `SolverConfig` + `ContactSoftnessCoefficients` 表达 | `grep` 零命中；`src/dynamics/solver/plugin.rs:216-324` |

> **推论**：本文件的"最小可运行刚体"路线（第 10 节）刻意只使用 v0.7.0 真实存在的类型，读者可直接照抄。

---

## 1. `RigidBody` 组件与其衍生组件

### 1.1 枚举本体

`src/dynamics/rigid_body/mod.rs:284-304` 定义：

- `RigidBody::Dynamic`（默认，`#[default]` 在 287 行）——受力和碰撞影响的主体。
- `RigidBody::Static`（295 行）——不受力/速度/碰撞影响，等效无限质量与无限转动惯量；只能手动改位置（289-294 行注释）。
- `RigidBody::Kinematic`（303 行）——不受外力与碰撞影响，但会影响与之碰撞的动态体；**可以有速度**，且引擎不改写它的组件值（297-302 行注释）。

三个谓词（`src/dynamics/rigid_body/mod.rs:306-320`）：

| 方法 | 行 | 语义 |
| --- | --- | --- |
| `is_dynamic()` | 308-310 | `*self == Self::Dynamic` |
| `is_static()` | 313-315 | `*self == Self::Static` |
| `is_kinematic()` | 318-320 | `*self == Self::Kinematic` |

组件元数据：`#[component(immutable, on_add = RigidBody::on_add)]`（`mod.rs:283`），`on_add` 只做一件事——初始化物理变换：`init_physics_transform(&mut world, &ctx)`（`mod.rs:322-325`，实现见 `src/physics_transform/transform.rs:1116-1274`）。`RigidBody` 是 **immutable**，改类型是插入新值而非 `&mut` 改写。

还有一个 crate 内部过滤器：`pub(crate) type RigidBodyActiveFilter = (Without<RigidBodyDisabled>, Without<Sleeping>)`（`mod.rs:329`）。几乎所有求解路径都用它排除"禁用/睡眠"的刚体。

### 1.2 标记组件与状态组件一览

| 类型 | 定义位置 | 一行职责 |
| --- | --- | --- |
| `RigidBodyDisabled` | `src/dynamics/rigid_body/mod.rs:380` | 标记刚体被临时禁用：不参与速度、力、接触响应与关节；**不**关闭碰撞检测与空间查询（331-339 行文档） |
| `Sleeping` | `src/dynamics/rigid_body/sleeping.rs:61` | 标记刚体正在睡眠、不参与模拟直到被唤醒 |
| `SleepingDisabled` | `src/dynamics/rigid_body/sleeping.rs:70` | 禁止该刚体进入睡眠 |
| `SleepThreshold { linear, angular }` | `src/dynamics/rigid_body/sleeping.rs:84-97` | 允许睡眠的速度上限；默认各 `0.15`（103-110 行）；`linear` 会被 `PhysicsLengthUnit` 隐式缩放（87 行） |
| `SleepTimer(pub f32)` | `src/dynamics/rigid_body/sleeping.rs:124` | 已静止累计时长；超过 `TimeToSleep` 才允许睡眠 |
| `TimeToSleep(pub f32)`（资源） | `src/dynamics/rigid_body/sleeping.rs:143` | 允许睡眠所需的静止时长，默认 `0.5` 秒（149-153 行） |
| `Dominance(pub i8)` | `src/dynamics/rigid_body/mod.rs:662` | 动态体之间的支配权；数值高者表现为无限质量；范围 `-127..=127`，默认 `0`；静态/运动学体恒高于动态体（631-639 行） |
| `LockedAxes(u8)` | `src/dynamics/rigid_body/locked_axes.rs:32` | 6 bit 锁轴位掩码（高 3 位平移、低 3 位旋转） |
| `RigidBodyColliders` | `src/collision/collider/collider_hierarchy/mod.rs:212` | `RelationshipTarget`，记录挂在该刚体上的碰撞体实体列表 |
| `MaxLinearSpeed(pub Scalar)` | `src/dynamics/rigid_body/mod.rs:441` | 线性速度上限，默认 `INFINITY`（443-447 行） |
| `MaxAngularSpeed(pub Scalar)` | `src/dynamics/rigid_body/mod.rs:471` | 角速度上限，默认 `INFINITY`（473-477 行） |

`LockedAxes` 提供了锁/解锁与判定方法：`lock_translation_x/y/z`（`locked_axes.rs:66,73,81`）、`lock_rotation_x/y/z`（88,96,104）、`is_translation_x_locked()`（172）、`is_rotation_locked()`（2D 219 / 3D 225），以及三个内部应用的辅助函数 `apply_to_vec`（230）、`apply_to_angular_inertia`（2D 246 / 3D 260）、`apply_to_angular_velocity`（2D 286 / 3D 295）。常量 `TRANSLATION_LOCKED = 0b111_000`、`ROTATION_LOCKED = 0b000_111`、`ALL_LOCKED = 0b111_111` 在 `locked_axes.rs:36-40`。

> **注意**：`LockedAxes` 在 2D 只有 1 个旋转位（`lock_rotation()`，`locked_axes.rs:110-115`），`apply_to_angular_inertia` 在 2D 把 `inverse_mut()` 直接置 0（251-254 行）；在 3D 则按行清零对称矩阵的对应行列（266-280 行）。这是"锁轴 = 质量属性层面的无限惯量"的实现，见第 4 节。

---

## 2. 运动状态组件（位置、旋转、速度、阻尼、重力缩放）

### 2.1 声明位置与底层类型

**关键事实**：`Position` / `Rotation` **不在** `src/dynamics/` 下，而在 `src/physics_transform/transform.rs`，并在 `src/physics_transform/mod.rs:6` 重新导出。

| 组件 | 行 | 定义 | 语义 / 单位 |
| --- | --- | --- | --- |
| `Position(pub Vector)` | `src/physics_transform/transform.rs:48` | newtype over `Vector` | 刚体/碰撞体的 **全局**位置；与 `Transform` 由 `PhysicsTransformPlugin` 双向同步（16-27 行） |
| `Position::PLACEHOLDER` | `transform.rs:54` | `Vector::MAX` | 占位值，表示"尚未初始化" |
| `Rotation`（2D） | `transform.rs:175-184` | `{ cos: Scalar, sin: Scalar }` | 单位复数表示；角被包裹在 `(-pi, pi]`（146 行） |
| `Rotation`（3D） | `transform.rs:745` | `pub struct Rotation(pub Quaternion)`，带 `Deref/DerefMut` | 单位四元数 |
| `Rotation::PLACEHOLDER` | `transform.rs:198-201`（2D）/ `752-757`（3D） | 全 `Scalar::MAX` | 占位 |
| `PreSolveDeltaPosition(pub Vector)` | `transform.rs:125` | newtype | XPBD 位置求解前的累计平移（120 行注释） |
| `PreSolveDeltaRotation(pub Rotation)` | `transform.rs:132` | newtype | XPBD 位置求解前的累计旋转 |
| `LinearVelocity(pub Vector)` | `src/dynamics/rigid_body/mod.rs:412` | newtype | 线速度，通常 **m/s**（382 行） |
| `LinearVelocity::ZERO` | `mod.rs:416` | 常量 | 零线速度 |
| `AngularVelocity`（2D） | `mod.rs:510` | `pub struct AngularVelocity(pub Scalar)` | **弧度/秒**，正值逆时针（479-480 行） |
| `AngularVelocity`（3D） | `mod.rs:543` | `pub struct AngularVelocity(pub Vector)` | 旋转轴 × 角速度(rad/s)（512-513 行） |
| `GravityScale(pub Scalar)` | `mod.rs:575` | newtype | 全局重力的逐体倍率；默认 `1.0`（577-581 行）；`0.0` 关闭、负值反向（554-557 行） |
| `LinearDamping(pub Scalar)` | `mod.rs:605` | newtype，`Default = 0.0` | 线性阻尼系数，模拟空气阻力 |
| `AngularDamping(pub Scalar)` | `mod.rs:629` | newtype，`Default = 0.0` | 角阻尼系数 |

底层数学类型来自 `bevy_math`（`crates/avian3d/Cargo.toml:86`：`bevy_math = { version = "0.19.0", features = ["approx"] }`），Avian 在 `src/math/single.rs` 与 `src/math/double.rs` 做别名：

- `pub type Scalar = f32`（`src/math/single.rs:6`）/ `f64`（`src/math/double.rs:6`）。
- `pub type Vector = Vec2`（`src/math/single.rs:18`，2D）/ `Vec3`（`src/math/single.rs:21`，3D）。
- `pub type Quaternion = Quat`（`src/math/single.rs:48`）/ `DQuat`（`src/math/double.rs:48`）。
- `pub type SymmetricMatrix = SymmetricMat2/Mat3`（`src/math/single.rs:39,42`）。
- `pub(crate) type AngularVector = Scalar`（2D，`src/math/mod.rs:63`）/ `Vector`（3D，`src/math/mod.rs:67`）。

### 2.2 "上一帧位置"在哪里

v0.7.0 **不再**用 `PreviousPosition` / `PreviousRotation` 组件保存上一帧位姿。取而代之：

- 求解器内部用 `SolverBody.delta_position` / `delta_rotation` 表示"相对本帧起点的增量"，这样可以避免远离原点时的舍入误差（`src/dynamics/solver/solver_body/mod.rs:74-86`、模块文档 30-53 行解释"Option 1 vs Option 2"的取舍）。
- 帧间插值改由 `PhysicsInterpolationPlugin` 负责，它是基于 `Transform` 的 easing，并额外注册私有组件 `PreviousLinearVelocity`（`src/interpolation.rs:304`）和 `PreviousAngularVelocity`（`src/interpolation.rs:308`），注册点 `src/interpolation.rs:275-276`。

> **推论**：读者若要实现"位置积分 + 写回"，最省事的做法是照 Avian 用 **delta** 而不是"上一帧位置"：求解器只需要"本帧起点 + 增量"，静态体作为零增量的 dummy 参与（见第 7 节 `SolverBody::DUMMY`）。

---

## 3. 刚体生成时的自动装配（required components 与 observer）

### 3.1 `RigidBody` 的 `#[require(...)]`

`src/dynamics/rigid_body/mod.rs:267-282` 声明生成任意 `RigidBody` 时自动插入：

```
Position::PLACEHOLDER, Rotation::PLACEHOLDER,
LinearVelocity, AngularVelocity,
ComputedMass, ComputedAngularInertia, ComputedCenterOfMass,
AccumulatedLocalAcceleration,
PreSolveDeltaPosition, PreSolveDeltaRotation,
```

即 **速度、质量属性、局部加速度累加器、XPBD 预求解增量** 都在这一刻装配。源码里的 TODO 明确说"只有 dynamic/kinematic 需要速度，只有 dynamic 需要质量和惯量"（268-269 行），说明这是保守的全量装配。

`Position::PLACEHOLDER` / `Rotation::PLACEHOLDER` 会被 `on_add` 钩子 (`mod.rs:283, 322-325`) 触发的 `init_physics_transform` 解析为真实值：若二者都是占位，则从 `Transform`（含父级链）计算全局位姿回填（`src/physics_transform/transform.rs:1229-1274`）；若用户显式写了 `Position`/`Rotation`，则在 `PhysicsTransformConfig::position_to_transform` 为真时反向写回 `Transform`（`transform.rs:1163-1220`）。

### 3.2 其他插件追加的组件

| 组件 | 由谁插入 | 依据 |
| --- | --- | --- |
| `Transform` | `PhysicsTransformPlugin` 注册 `Position -> Transform`、`Rotation -> Transform` | `src/physics_transform/mod.rs:81-82` |
| `SolverBody` + `SolverBodyInertia` | `SolverBodyPlugin` 的 observer：`on_insert_rigid_body`（只在非 static 时插入） | `src/dynamics/solver/solver_body/plugin.rs:132-152`；文档列出 4 种创建/4 种移除时机 `plugin.rs:25-37` |
| `VelocityIntegrationData` | `IntegratorPlugin`：`register_required_components::<SolverBody, VelocityIntegrationData>()` | `src/dynamics/integrator/mod.rs:48` |
| `BodyIslandNode` | `IslandPlugin`：`register_required_components::<SolverBody, BodyIslandNode>()` | `src/dynamics/solver/islands/mod.rs:76` |
| `SleepThreshold` + `SleepTimer` | `IslandSleepingPlugin`：`register_required_components::<SolverBody, ...>()` | `src/dynamics/solver/islands/sleeping.rs:50-51` |
| `RecomputeMassProperties` | `MassPropertyPlugin`：`register_required_components::<RigidBody, RecomputeMassProperties>()` | `src/dynamics/rigid_body/mass_properties/mod.rs:281` |
| `ColliderTransform` | `ColliderOf` 的 `#[require(ColliderTransform)]` | `src/collision/collider/collider_hierarchy/mod.rs:49` |

**装配顺序上的一个重要事实**：`SolverBody` 是"派生组件"而不是 `RigidBody` 的 required component，它由 observer 插入，因此**静态体没有 `SolverBody`**（`plugin.rs:141-151`：static → remove，非 static → insert）。而 `IntegratorPlugin` 的 `VelocityIntegrationData` 又挂在 `SolverBody` 上，所以静态体既没有 `SolverBody` 也没有 `VelocityIntegrationData`。这解释了为什么积分查询到处都带 `With<SolverBody>`（例如 `src/dynamics/integrator/mod.rs:317`）。

### 3.3 `RigidBodyColliders` 与层级发现

`RigidBodyColliders` 不是手工维护的列表，而是 Bevy 0.19 关系（relationship）系统的 target：

- `ColliderOf { body: Entity }`（`src/collision/collider/collider_hierarchy/mod.rs:53`）是 source 组件，`#[require(ColliderTransform)]`（49 行）。
- `impl Relationship for ColliderOf { type RelationshipTarget = RigidBodyColliders; const ALLOW_SELF_REFERENTIAL: bool = true; }`（`mod.rs:69-72`）。`ALLOW_SELF_REFERENTIAL` 让"刚体实体自己就是碰撞体"这种最常见的情形成立（67-68 行注释）。
- `RigidBodyColliders` 用 `#[relationship_target(relationship = ColliderOf, linked_spawn)]`（`mod.rs:208`）声明，`linked_spawn` 意味着碰撞体随刚体一同生成/销毁。
- `ColliderOf::on_insert`（`mod.rs:82-148`）做两件事：计算碰撞体相对刚体的局部变换写入 `ColliderTransform`（103-108 行）；把碰撞体实体 push 进 `RigidBodyColliders`，若 target 不存在则通过 `commands` 插入（132-139 行）。若刚体不存在则 `warn!`（141-147 行）。

"自动把碰撞体挂到最近的刚体上"由 `ColliderHierarchyPlugin` 的 observer 完成（`src/collision/collider/collider_hierarchy/plugin.rs:13-60`，包括 `on_collider_body_changed` 66 行与 `on_body_removed` 119 行）。

**对质量计算的影响**：`MassPropertyHelper::total_mass_properties` 用 `children.iter_descendants(entity)` 遍历**全部后代**（不只是直接碰撞体），把每个后代自身的 `Mass` / `AngularInertia` / `CenterOfMass` / `ColliderMassProperties` 累加起来（`src/dynamics/rigid_body/mass_properties/system_param.rs:140-149`）。所以"刚体 + 子实体带碰撞体"会自动合并质量。

> **推论**：Kairos 若暂时不做 ECS 关系系统，可以退化为"刚体拥有一个 `Vec<Entity>` 碰撞体列表 + 在插入/删除碰撞体时打脏标记"，与 Avian 的 `RigidBodyColliders` 语义等价。真正必须在层级里保留的是**碰撞体相对刚体的局部变换**（`ColliderTransform`），质量合并要用它做平行轴定理（见 4.5 节）。

---

## 4. 质量属性（mass properties）全解析

### 4.1 组件清单（真实存在的）

`src/dynamics/rigid_body/mass_properties/` 的目录结构：`mod.rs`（933 行）、`system_param.rs`（221 行）、`components/mod.rs`（1133 行）、`components/computed.rs`（1028 行）、`components/collider.rs`（94 行）。**没有** `mass_properties_components!` 之类的宏（`grep` 无输出），全部为手写。

**用户可写的（输入）组件：**

| 组件 | 行 | 职责 |
| --- | --- | --- |
| `Mass(pub f32)` | `components/mod.rs:160` | 局部质量；`Mass::ZERO`（`components/mod.rs:164`） |
| `AngularInertia(pub f32)` | `components/mod.rs:326`（`#[cfg(feature = "2d")]`） | 2D 局部转动惯量，`#[doc(alias = "MomentOfInertia")]`（325 行） |
| `AngularInertia { principal: Vec3, local_frame: Quat }` | `components/mod.rs:536-543`（3D） | 主惯量 + 局部惯性系朝向 |
| `CenterOfMass(pub VectorF32)` | `components/mod.rs:915` | 局部质心；`CenterOfMass::new(x, y[, z])`（2D 923 / 3D 931） |
| `ColliderDensity(pub f32)` | `components/collider.rs:32` | 碰撞体密度，默认 `1.0`（34-38 行）；`ColliderDensity::ZERO`（42） |

**自动计算的（输出）组件：**

| 组件 | 行 | 内部表示 |
| --- | --- | --- |
| `ComputedMass` | `components/computed.rs:48`，字段 `inverse: Scalar`（约 52-55 行） | **存逆质量**，减少除法并避免除零（25-34 行文档）；`inverse()` 是 no-op、`value()` 含一次除法 |
| `ComputedAngularInertia`（2D） | `components/computed.rs:222` | 标量逆惯量；`shifted` / `shifted_inverse`（330,336）做平行轴 |
| `ComputedAngularInertia`（3D） | `components/computed.rs:428`，字段 `inverse: SymmetricMatrix`（441-443） | 存逆张量；`tensor()`（617）、`inverse_tensor()`（626）、`shifted_tensor`（672）、`shifted_inverse_tensor`（687） |
| `ComputedCenterOfMass(pub Vector)` | `components/computed.rs:776` | 全局/本地合并后的质心 |
| `ColliderMassProperties(MassProperties)` | `components/collider.rs:80` | **只读**；由形状 + 密度算出；手动插入无效（88-89 行注释） |

**控制与触发用的标记组件：**

| 组件 | 行 | 职责 |
| --- | --- | --- |
| `NoAutoMass` | `components/mod.rs:981` | 不让后代/碰撞体贡献到 `ComputedMass`；带 `#[require(RecomputeMassProperties)]`（979）与 `on_remove = on_remove_no_auto_mass_property`（980） |
| `NoAutoAngularInertia` | `components/mod.rs:996` | 同上，作用于 `ComputedAngularInertia`（994-995） |
| `NoAutoCenterOfMass` | `components/mod.rs:1011` | 同上，作用于 `ComputedCenterOfMass`（1009-1010） |
| `RecomputeMassProperties` | `components/mod.rs:1030` | `#[component(storage = "SparseSet")]` 的脏标记，重算后自动移除（1022-1029 行文档） |
| `MassPropertiesBundle { mass, angular_inertia, center_of_mass }` | `components/mod.rs:1038-1042` | 一次插入三个输入组件的 bundle；`from_shape(shape, density)`（1081-1083） |

`NoAuto*` 的 `on_remove` 钩子是 `on_remove_no_auto_mass_property`（`components/mod.rs:1016-1020`），职责是"重新打开自动计算时补一次重算"。

### 4.2 计算与合并语义

`MassPropertyHelper` 是 `SystemParam`，字段见 `src/dynamics/rigid_body/mass_properties/system_param.rs:14-53`（两类查询：局部质量属性输入 + `Computed*` 输出）。

- `update_mass_properties(entity)`（`system_param.rs:60-134`）：先 `total_mass_properties`，再按 `NoAuto*` 三个开关分别决定"用计算值"还是"用局部显式值"：
  - `no_auto_mass` 且有 `Mass` → `mass_props.set_mass(mass.0, !no_auto_inertia)`，即**只有没显式给 `AngularInertia` 时才按新质量缩放惯量**（82-93 行）。
  - `no_auto_inertia` 且有 `AngularInertia`：2D 直接覆盖标量（97-101），3D 用 `principal` + `local_frame` 重建 `ComputedAngularInertia::new_with_local_frame`（102-110 行）。
  - `no_auto_com` 且有 `CenterOfMass` → 直接覆盖（126-130 行）。
- `total_mass_properties(entity)`（`system_param.rs:140-149`）：`once(local(entity)).chain(descendants.map(local)).flatten().sum()`。**合并是通过 `MassProperties` 的 `Sum` 实现的**，也就是 bevy_heavy 的 `MassProperties3d/2d` 累加语义（自动处理质量相加、质心加权、平行轴定理）。
- `local_mass_properties(entity)`（`system_param.rs:167-220`）的优先级链：
  1. 起点 = `ColliderMassProperties`，但**传感器（Sensor）被过滤掉**（172-174 行，`With<ColliderMassProperties> + With<ColliderTransform> + Without<Sensor>` 的查询过滤器在 30-34 行）。
  2. 若有 `Mass` → `set_mass(mass.0, angular_inertia.is_none())`（177-182 行，含 TODO：尚未考虑 `NoAutoMass`）。
  3. 若有 `AngularInertia` → 覆盖主惯量与局部惯性系（185-195 行）。
  4. 若有 `CenterOfMass` → 覆盖质心（198-200 行）。
  5. 若同时有碰撞体质量与 `ColliderTransform` → `mass_props.transform_by(Isometry3d::new(translation, rotation))`（2D 205-208 / 3D 212-215），即把该碰撞体的质量属性从其局部坐标系搬到刚体坐标系。

**密度与体积**：`ColliderMassProperties` 的计算路径是 `collider.mass_properties(density.0)`（`src/collision/collider/backend.rs:507`），最终委托给 bevy_heavy 的 `ComputeMassProperties3d`/`ComputeMassProperties2d`（Avian 在 `mass_properties/mod.rs:215-222` 做别名，`pub use bevy_heavy` 在 213 行）。`crates/avian3d/Cargo.toml:88` 声明 `bevy_heavy = { version = "0.5" }`（dev-dependencies 中另有 `bevy_heavy = { version = "0.5", features = ["approx"] }`，`Cargo.toml:107`）。`MassPropertiesExt::to_bundle`（`mass_properties/mod.rs:225-247`）把 `MassProperties` 转成 `MassPropertiesBundle`；3D 分支用 `principal_angular_inertia` + `local_inertial_frame`（236-239 行）。

源码单测给出了可直接当作规格的行为（`mass_properties/mod.rs:451-932`）：

- 只有 `RigidBody::Dynamic`、无碰撞体 → 质量/惯量/质心全为默认（零）`mass_properties/mod.rs:483-498`。
- 有碰撞体、无显式 `Mass` → 质量等于 `collider.mass_properties(1.0).mass`（500-522 行）。
- 有 `Mass(5.0)`、无 `AngularInertia` → 惯量 = `5.0 * unit_angular_inertia()`（524-549 行）。
- `RigidBody + Collider + Mass(5.0)` 且有子碰撞体 → 总质量 `5.0 + child_mass`，总惯量 `5.0*unit_inertia + child_angular_inertia`（579-606 行）。
- 子实体带 `Mass(10.0)` 时子质量直接相加（608-636 行）。
- `NoAutoMass` → 只用本体 `Mass`；惯量按"总惯量 × 本体质量 / 总质量"缩放（662-691 行、863-932 行）。
- 移动子碰撞体后质心重新计算为 `[0, 1]`（`Vector::new(0.0, 1.0)`，808-861 行）。
- 增删子碰撞体会重算（693-760 行）。
- 修改 `Mass` 会重算（762-806 行）。

**"零质量 = 无限质量"**：文档明确 `Static`/`Kinematic` 有无限质量与无限惯量，`Dynamic` 的零质量也被当作无限质量特例（`mass_properties/mod.rs:12-14`，以及 `src/dynamics/rigid_body/mod.rs:134-136`）。`ComputedMass` 存逆质量，所以无限质量就是 `inverse = 0.0`（`components/computed.rs:59-61` 的 `INFINITY` 常量）。**没有质量也没有惯量的 `Dynamic` 刚体会被 `warn_invalid_mass` 警告**（`mass_properties/mod.rs:420-449`，警告文本在 443-446 行）。

### 4.3 何时重算（脏追踪与调度顺序）—— 最容易写错的地方

系统集合定义在 `src/dynamics/rigid_body/mass_properties/mod.rs:333-341`：

```
UpdateColliderMassProperties -> QueueRecomputation -> UpdateComputedMassProperties  (chain)
```

配置语句在 `mod.rs:299-309`，三重关键约束：

1. `.in_set(PhysicsSystems::Prepare)`（307 行）——属于 `PhysicsSchedule` 的 prepare 阶段。
2. `.after(PhysicsTransformSystems::TransformToPosition)`（308 行）——**必须在 `Transform -> Position/Rotation` 同步之后**，否则用旧位姿算质心。
3. 三个集合 `chain()`（306 行）——先算碰撞体自身质量属性，再决定谁脏，最后重算。

**Stage 1 — `UpdateColliderMassProperties`**：`update_collider_mass_properties::<C>`（`src/collision/collider/backend.rs:498-509`）在 `Or<(Changed<C>, Changed<ColliderDensity>)>` 且 `Without<Sensor>` 时重算 `ColliderMassProperties`。注册点在 `src/collision/collider/backend.rs:228-238`（与 `update_collider_scale` 组成 chain，前者在 `PhysicsSystems::Prepare` 且 `.after(TransformToPosition)`）。

此外还有若干 observer 覆盖"瞬时"场景：

- 碰撞体组件 `on_add` 钩子：初始化时按 `ColliderDensity`（缺省 `1.0`）算一次；若带 `Sensor` 则直接置 `MassProperties::ZERO`（`backend.rs:142-159`）。
- 碰撞体 `on_remove` 钩子：给所属刚体插 `RecomputeMassProperties`（`backend.rs:164-187`）。
- `On<Add, Sensor>`：给刚体插 `RecomputeMassProperties`（`backend.rs:190-208`）。
- `On<Remove, Sensor>`：立刻重算该碰撞体的质量属性（`backend.rs:211-226`）。

**Stage 2 — `QueueRecomputation`**：两个系统：

- `queue_mass_recomputation_on_mass_change`（`mass_properties/mod.rs:362-376`）：对 `WithComputedMassProperty + Without<ColliderOf> + MassPropertyChanged` 的实体插 `RecomputeMassProperties`。`MassPropertyChanged = Or<(Changed<Mass>, Changed<AngularInertia>, Changed<CenterOfMass>)>`（`mod.rs:351-355`），`WithComputedMassProperty = Or<(With<ComputedMass>, With<ComputedAngularInertia>, With<ComputedCenterOfMass>)>`（`mod.rs:344-348`）。`Without<ColliderOf>` 是为了把碰撞体的变化交给下一个系统，避免重复。
- `queue_mass_recomputation_on_collider_mass_change`（`mass_properties/mod.rs:380-396`）：查询 `&ColliderOf`，条件 `Or<(Changed<ColliderMassProperties>, Changed<ColliderTransform>, MassPropertyChanged)>`，把脏标记插到 **body** 上。这里 `Changed<ColliderTransform>` 就是"碰撞体相对刚体移动/旋转了"→ 需要重算质心与平行轴项。

**Stage 3 — `UpdateComputedMassProperties`**：`update_mass_properties`（`mass_properties/mod.rs:398-408`）遍历带 `RecomputeMassProperties` 的实体，调用 `mass_helper.update_mass_properties(entity)` 并**移除脏标记**（406 行）；随后 `warn_invalid_mass`（420-449 行）在 `ComputedMass`/`ComputedAngularInertia` `Changed` 且值非有限时告警。

**额外的立即路径**（observer，绕过调度）：`MassPropertyPlugin::build` 注册两个 observer：

- `On<Add, RigidBody>` → `update_mass_properties`（`mass_properties/mod.rs:284-288`）。
- `On<Insert, RigidBodyColliders>` → `update_mass_properties`（`mass_properties/mod.rs:292-296`）。

源码 279-280 行有 TODO 承认这与 `register_required_components::<RigidBody, RecomputeMassProperties>()`（281 行）存在重复工作。

**与求解器的相对顺序**（这是必须在 Kairos 中复刻的硬约束）：

`PhysicsSystems` 的链是 `First -> Prepare -> StepSimulation -> Writeback -> Last`（`src/schedule/mod.rs:74-85`），而 `StepSimulation` 内部运行的 `PhysicsSchedule` 链是 `First -> BroadPhase -> NarrowPhase -> Solver -> Sleeping -> Finalize -> Last`（`src/schedule/mod.rs:96-107`）。所以：

```
PhysicsSystems::Prepare { TransformToPosition ... MassPropertySystems(chain) }   // 位置与质量同步
PhysicsSchedule { BroadPhase -> NarrowPhase -> Solver -> Sleeping -> Finalize }
PhysicsSystems::Writeback { PositionToTransform }                                 // 写回 Transform
```

即 **质量属性在每一帧物理步之前完成更新，且在求解器之前**；位置/旋转的最终写回发生在物理步之后。求解器读取的是 `ComputedMass` / `ComputedAngularInertia`（`src/dynamics/solver/solver_body/plugin.rs:188-190` 的查询），因此"质量必须在 prepare 阶段结束前算好"是硬性依赖。

> **推论**：Kairos 的最小实现应把"质量重算"放在 `physics_step_system` 内部的**最前段**（位置同步之后、积分之前），并保证"碰撞体移动/增删"会打脏标记。若把质量重算放在积分之后，读者会遇到"质量滞后一帧"的隐蔽 bug。

### 4.4 `LockedAxes` 与惯量的交互

`LockedAxes` 不修改 `ComputedAngularInertia` 组件，而是修改**求解器侧的副本**：

- `prepare_solver_bodies` 构造 `SolverBodyInertia` 时传入 `locked_axes`（`src/dynamics/solver/solver_body/plugin.rs:214-224`）。
- 3D 下世界空间逆惯量是 `angular_inertia.rotated(rotation.0).inverse()`（`plugin.rs:220`），即先旋转到世界空间再取逆。
- 之所以有效，是因为 `apply_to_angular_inertia` 把被锁轴的**逆惯量矩阵行列清零**（`src/dynamics/rigid_body/locked_axes.rs:266-280`），零逆惯量 = 无限惯量 = 该轴永不被扭矩改变。

另外，锁定轴还会在多个位置"提前"生效，避免先污染再修正：

- 重力与锁定轴在预计算阶段一起作用于速度增量（`src/dynamics/integrator/mod.rs:301-303`）。
- 局部加速度在子步内应用时也过一遍锁轴（`src/dynamics/rigid_body/forces/plugin.rs:226-230`）。
- 冲量/力的 API 用 `locked_axes().apply_to_vec(Vector::splat(inverse_mass))` 把质量逆也做轴过滤（`src/dynamics/rigid_body/forces/query_data.rs:390-394`）。
- `SolverBody.flags` 把锁轴位图整包带走（`src/dynamics/solver/solver_body/plugin.rs:225`），`SolverBodyFlags::TRANSLATION_X_LOCKED` 等位定义在 `src/dynamics/solver/solver_body/mod.rs:132-157`。

---

## 5. 力与重力

### 5.1 单位约定

`src/dynamics/rigid_body/forces/mod.rs:9-13` 给出权威表：

| 类型 | 公式 | 与速度关系 | 单位 |
| --- | --- | --- | --- |
| Force | `F = m * a` | `Δa = F / m` | N = kg·m/s² |
| Impulse | `J = F * Δt` | `Δv = J / m` | N·s |
| Acceleration | `a = F / m` | `Δv = a * Δt` | m/s² |

### 5.2 常量（持久）累加器组件

| 组件 | 行 | 语义 |
| --- | --- | --- |
| `ConstantForce(pub Vector)` | `forces/mod.rs:260` | 世界空间常量力，跨步持续（225-227 行） |
| `ConstantTorque(pub AngularVector)` | `forces/mod.rs:317` | 世界空间常量扭矩 |
| `ConstantLocalForce(pub Vector)` | `forces/mod.rs:371` | 局部空间常量力 |
| `ConstantLocalTorque(pub AngularVector)` | `forces/mod.rs:424` | 局部空间常量扭矩，`#[cfg(feature = "3d")]` |
| `ConstantLinearAcceleration(pub Vector)` | `forces/mod.rs:478` | 世界空间常量线加速度（忽略质量） |
| `ConstantAngularAcceleration(pub AngularVector)` | `forces/mod.rs:538` | 世界空间常量角加速度 |
| `ConstantLocalLinearAcceleration(pub Vector)` | `forces/mod.rs:598` | 局部空间常量线加速度 |
| `ConstantLocalAngularAcceleration(pub AngularVector)` | `forces/mod.rs:651` | 局部空间常量角加速度，`#[cfg(feature = "3d")]` |
| `AccumulatedLocalAcceleration { linear, angular }` | `forces/mod.rs:667-673` | **内部累加器**（`angular` 仅 3D），由 `RigidBody` 的 required components 自动插入（`rigid_body/mod.rs:278`） |

### 5.3 "常量 vs 外部"两套累加器的分工与清空点

这是本节的**核心机制**，也是任务描述里"external force 每步清空"的 v0.7.0 真实形态：

1. **世界空间累加器**是 `VelocityIntegrationData`（`src/dynamics/integrator/mod.rs:216-233`）：
   - `linear_increment: Vector`（221）、`angular_increment: AngularVector`（226）——**在 `IntegrationSystems::UpdateVelocityIncrements` 之前被当作加速度**，之后才乘 dt 变成速度增量（219-225 行注释、305-308 行代码）。
   - `linear_damping_rhs`（229）、`angular_damping_rhs`（232）——阻尼方程的右端项 `1 / (1 + dt * c)`（228、231 行注释；计算方法 248-256 行）。
   - **清空**：`clear_velocity_increments`（`integrator/mod.rs:316-328`），注册在 `SolverSystems::PostSubstep` 内的 `IntegrationSystems::ClearVelocityIncrements`（`integrator/mod.rs:58-60, 69`）。
2. **局部空间累加器**是 `AccumulatedLocalAcceleration`：
   - **清空**：`clear_accumulated_local_acceleration`（`forces/plugin.rs:243-251`），在 `ForceSystems::Clear`，而 `ForceSystems::Clear` 被放进 `SolverSystems::PostSubstep`（`forces/plugin.rs:31`）。

因此 v0.7.0 **没有** `ExternalForce` 之类的"外部力组件"；"一次性、每步清空"的语义改由 `Forces` QueryData 直接往上面两个累加器里写（见 5.5 节）。

### 5.4 `ForcePlugin` 的调度

`src/dynamics/rigid_body/forces/plugin.rs:22-72`：

- `ForceSystems::ApplyConstantForces` ∈ `IntegrationSystems::UpdateVelocityIncrements`，且 `.before(integrator::pre_process_velocity_increments)`（28-30 行）→ **常量力先于重力与阻尼预计算**。
- `ForceSystems::Clear` ∈ `SolverSystems::PostSubstep`（31 行）。
- `ForceSystems::ApplyLocalAcceleration` ∈ `SubstepSchedule` 的 `IntegrationSystems::Velocity`，`.before(integrator::integrate_velocities)`（34-39 行）→ **局部加速度必须在每个子步内应用**，因为刚体朝向会随子步改变（60-61 行注释）。
- 8 个常量系统按 `chain()` 顺序注册（42-58 行）。

各系统的数学（`forces/plugin.rs`）：

| 系统 | 行 | 数学 |
| --- | --- | --- |
| `apply_constant_forces` | 96-104 | `linear_increment += mass.inverse() * force` |
| `apply_constant_torques` | 107-120 | `angular_increment += inertia.effective_inv_angular_inertia() * torque` |
| `apply_constant_linear_acceleration` | 123-131 | `linear_increment += acceleration`（不过质量） |
| `apply_constant_angular_acceleration` | 134-142 | `angular_increment += acceleration` |
| `apply_constant_local_forces` | 145-157 | `AccumulatedLocalAcceleration.linear += mass.inverse() * force` |
| `apply_constant_local_torques` | 161-173 | `acc.angular += angular_inertia.inverse() * torque`（3D） |
| `apply_constant_local_linear_acceleration` | 176-187 | `acc.linear += acceleration` |
| `apply_constant_local_angular_acceleration` | 191-202 | `acc.angular += acceleration`（3D） |
| `apply_local_acceleration` | 207-241 | 在子步内：`world_a = locked_axes.apply_to_vec(rotation * acc.linear)`，然后 `SolverBody.linear_velocity += world_a * dt_substep`（226-236 行） |

注意 `apply_constant_torques` 用的是 `SolverBodyInertia::effective_inv_angular_inertia()` 而不是 `ComputedAngularInertia`（`plugin.rs:110,118`），因为求解器侧的惯量已经包含锁轴与世界空间旋转（见 7.2 节）。

### 5.5 `Forces` QueryData 与真实方法名

`src/dynamics/rigid_body/forces/query_data.rs:105-120` 定义 `#[derive(QueryData)] #[query_data(mutable)] pub struct Forces`，字段为 `position` / `rotation` / `linear_velocity` / `angular_velocity` / `mass`（`ComputedMass`）/ `angular_inertia`（`ComputedAngularInertia`）/ `center_of_mass`（`ComputedCenterOfMass`）/ `locked_axes`（`Option`）/ `integration`（`VelocityIntegrationData`）/ `accumulated_local_acceleration` / `sleep_timer`（`Option`）/ `is_sleeping`（`Has<Sleeping>`）。用法是 `Query<Forces>`（**不带 `&`/`&mut`**），见模块文档 `forces/mod.rs:74-75`。

方法分两个 trait：

**只读 trait `ReadRigidBodyForces`（`query_data.rs:191-280`）**：`position()`（194）、`rotation()`（200）、`linear_velocity()`（206）、`angular_velocity()`（212）、`accumulated_linear_acceleration()`（223）、`accumulated_angular_acceleration()`（2D 241 / 3D 255）、`velocity_at_point(world_point)`（269）。

**写入 trait `WriteRigidBodyForces`（`query_data.rs:292-…`）**，真实方法名与行号：

| 方法 | 行 | 数学 / 落点 |
| --- | --- | --- |
| `apply_force(force)` | 300 | `integration.linear_increment += inverse_mass * force` |
| `apply_force_at_point(force, world_point)` | 330 | `apply_force` + `apply_torque(cross(world_point - global_com, force))` |
| `apply_local_force(force)` | 344 | 写 `local_acc.linear` |
| `apply_torque(torque)` | 358 | `angular_increment += effective_inverse_angular_inertia * torque` |
| `apply_local_torque(torque)` | 374 | 写 `local_acc.angular`（3D） |
| `apply_linear_impulse(impulse)` | 388 | **立刻**改 `LinearVelocity`：`delta_v = locked_axes(Vector::splat(inv_mass)) * impulse` |
| `apply_linear_impulse_at_point(impulse, world_point)` | 420 | `apply_linear_impulse` + `apply_angular_impulse(cross(r, impulse))` |
| `apply_local_linear_impulse(impulse)` | 432 | 把 impulse 旋到世界空间再按上式施加 |
| `apply_angular_impulse(impulse)` | 450 | **立刻**改 `AngularVelocity` |
| `apply_local_angular_impulse(impulse)` | 466 | 旋到世界空间再施加（3D） |
| `apply_linear_acceleration(acceleration)` | 482 | 写 `linear_increment`，不过质量 |
| `apply_linear_acceleration_at_point(acceleration, world_point)` | 511 | `apply_linear_acceleration` + `apply_angular_acceleration(cross(r, a))` |
| `apply_local_linear_acceleration(acceleration)` | 526 | 写 `local_acc.linear` |
| `apply_angular_acceleration(acceleration)` | 539 | 写 `angular_increment` |
| `apply_local_angular_acceleration(acceleration)` | 554 | 写 `local_acc.angular` |

**唤醒语义**：所有写入方法都形如 `if force != ZERO && self.try_wake_up() { … }`（例如 301、345、359 行）。`ForcesItem::non_waking()`（`query_data.rs:153`）返回 `NonWakingForcesItem`（126 行），用于"施力但不清醒睡眠体"；`NonWakingForcesItem::waking()`（162）反向转换。模块文档在 `forces/mod.rs:100-116` 说明这一点。

**注意 `apply_force_at_point` 的已知坑**：源码 317-327 行说明若在施加前改了 `Transform`，扭矩会用**过期的全局质心**计算，可能产生巨大的错误扭矩；建议改用 `PhysicsTransformHelper` 先同步。`ForcesItem::reborrow()`（132 行）用于避免借用冲突。

### 5.6 `Gravity` 资源与 `GravityScale`

- `pub struct Gravity(pub Vector)`：`src/dynamics/integrator/mod.rs:156`，`#[derive(Reflect, Resource, Debug)]`（152 行）。
- 默认值：`Self(Vector::Y * -9.81)`（`integrator/mod.rs:158-162`）——注意是 `Vector::Y`，2D/3D 都是"向上为 Y"的约定，3D 下即 `(0, -9.81, 0)`。
- `Gravity::ZERO`（166）。
- `GravityScale(pub Scalar)`：`src/dynamics/rigid_body/mod.rs:575`，默认 `1.0`（577-581）。
- **全局重力被初始化为资源**：`app.init_resource::<Gravity>()`（`integrator/mod.rs:50`）。
- **施加位置（唯一）**：`pre_process_velocity_increments` 内的
  `integration.linear_increment += gravity.0 * gravity_scale.map_or(1.0, |scale| scale.0);`（`integrator/mod.rs:298`）。
  随后锁轴过滤（301-303）与乘子步 dt（307-308）。
- 修改 `Gravity` 会唤醒所有岛屿：`wake_all_islands.run_if(resource_changed::<Gravity>)`（`src/dynamics/solver/islands/sleeping.rs:77`）。

> **关键点**：重力是**加速度**，只加到 `linear_increment` 上，**不乘质量**。质量只在"力→加速度"这一步起作用（`mass.inverse() * force`）。这符合 `a = g` 与质量无关的物理事实，也是读者最容易写错的一处（把重力当力乘质量）。

---

## 6. 积分器（`src/dynamics/integrator/mod.rs`）

### 6.1 方案与整体定位

模块文档（`integrator/mod.rs:15-23`）明确：

- 这是 **semi-implicit（symplectic）Euler**，是唯一的积分方案（"Currently, only the semi-implicit (symplectic) Euler integration scheme is supported"）。
- 它的角色是**预测（prediction）**：`The solver corrects these predicted positions to take constraints like contacts and joints into account.`（17-18 行）。

`IntegratorPlugin` 默认跑在 `SubstepSchedule`（`integrator/mod.rs:39-43`），可通过 `new(schedule)` 改变（32-36 行）。

### 6.2 系统集合与调度（含行号）

`IntegrationSystems` 定义在 `integrator/mod.rs:92-111`：

| 集合 | 行 | 所在 schedule / 父集合 |
| --- | --- | --- |
| `UpdateVelocityIncrements` | 98 | `PhysicsSchedule` 的 `SolverSystems::PreSubstep` |
| `Velocity` | 102 | `SubstepSchedule` 的 `IntegrationSystems::Velocity` |
| `Position` | 106 | `SubstepSchedule` 的 `IntegrationSystems::Position` |
| `ClearVelocityIncrements` | 110 | `PhysicsSchedule` 的 `SolverSystems::PostSubstep` |

注册代码（`integrator/mod.rs:52-86`）：

```rust
// 每帧一次（PhysicsSchedule）
(UpdateVelocityIncrements).in_set(SolverSystems::PreSubstep).before(IntegrationSystems::Velocity)
(ClearVelocityIncrements).in_set(SolverSystems::PostSubstep).after(IntegrationSystems::Velocity)
// 每子步一次（SubstepSchedule）
(IntegrationSystems::Velocity, IntegrationSystems::Position).chain()   // 行 73-76
( (integrate_velocities, clamp_velocities).chain().in_set(Velocity),
  integrate_positions.in_set(Position) )                               // 行 78-86
```

`.chain()` 在 `integrator/mod.rs:75` —— **速度积分严格先于位置积分**，这就是 symplectic 顺序的落点。

**`SubstepSchedule` 内部的完整链**（`src/dynamics/solver/schedule.rs:59-69`）：

```
IntegrationSystems::Velocity            → 积分速度（含阻尼、增量、陀螺力矩）
SubstepSolverSystems::WarmStart         → 热启动
SubstepSolverSystems::SolveConstraints  → 带 bias 求解
IntegrationSystems::Position            → 积分位置
SubstepSolverSystems::Relax             → 无 bias 求解（松弛）
SubstepSolverSystems::Damping           → 关节阻尼
```

`SubstepSolverSystems` 的文档注释（`schedule.rs:123-149`）逐条解释了为什么这样排（例如 131-132 行："Solve constraints without bias to relax velocities"）。

`SolverSystems` 的完整链在 `src/dynamics/solver/schedule.rs:32-46`：

```
PrepareSolverBodies → PrepareJoints → PrepareContactConstraints
→ PreSubstep → Substep → PostSubstep → Restitution → Finalize → StoreContactImpulses
```

### 6.3 每子步 / 每帧各做什么

**每帧一次（`SolverSystems::PreSubstep`）**

1. `update_contact_softness`（`src/dynamics/solver/plugin.rs:108, 326-350`）——在 NarrowPhase 之前刷新接触软度系数。
2. `apply_constant_*`（8 个，见 5.4）——把常量力/扭矩/加速度写进增量。
3. `pre_process_velocity_increments`（`integrator/mod.rs:260-313`）：
   - 跳过非动态体（280-283 行）；
   - 缓存阻尼右端项 `1/(1+dt*c)`（289-292 行）；
   - `linear_increment += gravity * gravity_scale`（298 行）；
   - 锁轴过滤线/角增量（301-303 行）；
   - **乘子步 dt**（307-308 行）。注意 dt 来自 `Res<Time<Substeps>>`（270 行），而 `Time<Substeps>` 在 `PhysicsSchedule` 运行前已被推进一次（`src/schedule/mod.rs:250-255`），所以这一步拿到的是**子步 dt**。

**每子步一次**

4. `apply_local_acceleration`（`forces/plugin.rs:207-241`）——局部加速度，用当子步的朝向（222 行 `let rotation = body.delta_rotation * *rotation;`）。
5. `integrate_velocities`（`integrator/mod.rs:343-391`）：
   - 跳过 `body.solver_body.flags.is_kinematic()`（357-360 行）；
   - **先阻尼**：`linear_velocity *= linear_damping_rhs`，`angular_velocity *= angular_damping_rhs`（363-364 行）；
   - **再加增量**：`+= linear_increment` / `+= angular_increment`（367-368 行）；
   - 3D 可选陀螺力矩（372-387 行）。
6. `clamp_velocities`（`integrator/mod.rs:467-500`）——按 `MaxLinearSpeed` / `MaxAngularSpeed` 截断（2D 角速度用 `copysign` 保号，488 行）。
7. WarmStart / SolveConstraints（求解器，见第 7 节）。
8. `integrate_positions`（`integrator/mod.rs:503-535`）：
   - `*delta_position += *linear_velocity * delta_secs;`（521 行）
   - 2D：`*delta_rotation = Rotation::radians(*angular_velocity * delta_secs) * *delta_rotation;`（525 行）
   - 3D：`delta_rotation.0 = Quaternion::from_scaled_axis(*angular_velocity * delta_secs) * delta_rotation.0;`（529-530 行）
   - `delta_secs = time.delta_seconds_adjusted()`（510 行），即子步 dt。
   - 3D 下紧接着 `update_solver_body_angular_inertia`（`src/dynamics/solver/solver_body/plugin.rs:117-123, 295-303`）刷新世界空间逆惯量。
9. Relax / Damping。
10. `solve_restitution`（`SolverSystems::Restitution`，`solver/plugin.rs:116, 630`）。
11. `writeback_solver_bodies`（`SolverSystems::Finalize`，`solver_body/plugin.rs:112, 263-292`）。
12. `store_contact_impulses`（`SolverSystems::StoreContactImpulses`，`solver/plugin.rs:119, 722`）。

**每帧结束后**（`PhysicsSystems::Writeback`）：`position_to_transform`（`src/physics_transform/mod.rs:114-123`）把 `Position`/`Rotation` 写进 `Transform`；`Position`/`Rotation` 的过滤器是 `Or<(Changed<Position>, Changed<Rotation>)>`（`src/physics_transform/mod.rs:254-257` 的类型别名 `PosToTransformFilter`）。

### 6.4 阻尼数学

`VelocityIntegrationData::update_linear_damping_rhs(damping_coefficient, delta_secs)`：`1.0 / (1.0 + delta_secs * damping_coefficient)`（`integrator/mod.rs:248-250`），角速度同理（254-256 行）。应用方式是**乘法**（`integrator/mod.rs:363-364`），即隐式阻尼 `v' = v / (1 + c·dt)`，而不是显式 `v' = v·(1 - c·dt)`。系数来自 `LinearDamping` / `AngularDamping`，缺省 `0.0`（289-290 行），默认即无阻尼。

> **推论**：隐式形式在 `c·dt > 1` 时仍稳定（不会把速度变成负数），这是可抄的工程细节。

### 6.5 子步（`SubstepCount` 与 SubstepSchedule）

- `pub struct SubstepCount(pub u32)`（`src/dynamics/solver/schedule.rs:185`），**默认 6**（187-191 行）。
- `run_substep_schedule`（`src/dynamics/solver/schedule.rs:194-213`）：
  ```rust
  let delta = world.resource::<Time<Physics>>().delta();
  let SubstepCount(substeps) = *world.resource::<SubstepCount>();
  let sub_delta = delta.div_f64(substeps as f64);
  world.resource_mut::<Time<Substeps>>().advance_by(sub_delta);
  for i in 0..substeps {
      *world.resource_mut::<Time>() = world.resource::<Time<Substeps>>().as_generic();
      schedule.run(world);
  }
  *world.resource_mut::<Time>() = world.resource::<Time<Physics>>().as_generic();
  ```
- 该 exclusive system 注册在 `SolverSystems::Substep`（`schedule.rs:49`）。
- `SubstepSchedule` 用 `SingleThreadedExecutor` 且 `ambiguity_detection: LogLevel::Error`（`schedule.rs:52-58`）；`PhysicsSchedule` 同样单线程（`src/schedule/mod.rs:88-94`）。

**重要事实**：循环体内**不再推进** `Time<Substeps>`，所以**同一个物理步的所有子步使用同一个 `delta`（= `dt_physics / substeps`）**，子步之间变化的只有刚体状态（位置、旋转、速度）。`Time<Substeps>` 的 `elapsed` 会因两处 `advance_by`（`src/schedule/mod.rs:254` 与 `solver/schedule.rs:200`）一帧累计 2×sub_delta，但 `delta` 每次都被**覆写**为同一个 sub_delta。

> **推论**：读者实现子步时，只需 `dt_sub = dt / n`，然后循环 n 次执行"应用加速度→积分速度→求解→积分位置→写回"；不必（也不应该）把 `Time` 的 elapsed 拆成 n 段。

### 6.6 `GravityScale`、睡眠处理、`PhysicsLengthUnit`

- `GravityScale` 只在 `integrator/mod.rs:298` 出现一次，作为全局重力的乘子；`0.0` 关闭、负值反向（`src/dynamics/rigid_body/mod.rs:556-557`）。
- **睡眠处理不在积分器里**：积分查询用 `RigidBodyActiveFilter`（`integrator/mod.rs:346`），即 `Without<RigidBodyDisabled> + Without<Sleeping>`（`src/dynamics/rigid_body/mod.rs:329`）。而睡眠体**根本没有 `SolverBody`**（`src/dynamics/solver/solver_body/plugin.rs:94-99` 的 observer 在加入 `Sleeping` 时移除 `SolverBody`），所以 `integrate_velocities` / `integrate_positions`（都查 `&mut SolverBody`）天然跳过睡眠体。这是"睡眠体不被模拟"的实现方式——**移除组件**而不是打标志位。
- `PhysicsLengthUnit(pub Scalar)`（`src/dynamics/solver/plugin.rs:201`），默认 `1.0`（203-207 行），由 `SolverPlugin::new_with_length_unit` 注入（95-101 行），也被 `PhysicsSchedulePlugin` 提前 `init_resource`（`src/schedule/mod.rs:69`）。积分器**不使用**长度单位；使用它的是睡眠阈值（`islands/sleeping.rs:196-217`）、求解器的最大分离速度与恢复阈值（`plugin.rs:535,542,634,640`）、以及 `transform_to_position` 的容差（`src/physics_transform/mod.rs:203`）。
- `delta_seconds_adjusted()` 只是 `f32`/`f64` 精度适配（`src/schedule/time.rs:273-292`），不涉及单位缩放。

### 6.7 积分正确性的现成测试

`integrator/mod.rs:561-629`（`fn semi_implicit_euler`）：`SubstepCount(1)`、`Time::from_hz(10.0)`、跑 100 步后断言

- `position ≈ Vector::NEG_Y * 490.5`（epsilon 10.0）；
- `linear_velocity ≈ Vector::NEG_Y * 98.1`；
- `rotation` 从 `AngularVelocity(2.0)` 累积到 20 rad（2D 精确、3D epsilon 0.01）。

解析解核对：`v = g·t = 9.81 × 10 = 98.1`，`y = -½g t² = -490.5` —— 这正是"半隐式欧拉一帧一步 0.1 s"的预期结果。**推论**：这个测试可以直接移植成 Kairos 的验收用例（`dt=0.1s`、100 步、`SubstepCount=1`）。

### 6.8 可选的自定义积分钩子

- `CustomVelocityIntegration`（`integrator/mod.rs:182`）：标记后跳过重力、阻尼、常量力；接触/关节冲量与 `Forces` 的冲量仍然生效（169-177 行文档）。查询过滤在 `integrate_velocities`（346 行）与 `apply_local_acceleration`（`forces/plugin.rs:210`）。
- `CustomPositionIntegration`（`integrator/mod.rs:195`）：标记后不按速度更新位置（`integrate_positions` 的 `Without<...>`，504 行）。

---

## 7. 求解器结构（`src/dynamics/solver/`）

### 7.1 插件与调度骨架

`SolverPlugins`（`src/dynamics/solver/mod.rs:44-84`）按序加入：`SolverBodyPlugin`、`SolverSchedulePlugin`、`IntegratorPlugin`、`SolverPlugin`、`CcdPlugin`、`IslandPlugin`、`IslandSleepingPlugin`、四个（3D 五个）`JointGraphPlugin<T>`，以及 `#[cfg(feature = "xpbd_joints")]` 的 `XpbdSolverPlugin`（`mod.rs:57-83`）。插件职责表在 `mod.rs:27-40`。

`SolverPlugin::build`（`src/dynamics/solver/plugin.rs:88-157`）注册：

| 系统 | 行 | 集合 |
| --- | --- | --- |
| `update_contact_softness` | 108 | `.before(PhysicsStepSystems::NarrowPhase)` |
| `prepare_contact_constraints` | 111-113 | `SolverSystems::PrepareContactConstraints` |
| `solve_restitution` | 116 | `SolverSystems::Restitution` |
| `store_contact_impulses` | 119 | `SolverSystems::StoreContactImpulses` |
| `warm_start` | 129 | `SubstepSolverSystems::WarmStart` |
| `solve_contacts::<true>` | 132 | `SubstepSolverSystems::SolveConstraints`（带 bias） |
| `solve_contacts::<false>` | 136 | `SubstepSolverSystems::Relax`（无 bias） |
| `joint_damping::<T>` × 4/5 | 139-150 | `SubstepSolverSystems::Damping`（`chain()`） |

在 `SubstepSchedule` 中 `warm_start` / `solve_contacts` 每子步执行一次（因为 `SubstepSchedule` 被子步循环调用 N 次），而 `prepare_contact_constraints` 每帧一次。

两个易踩的阅读陷阱：

- **`SolverPlugin` 没有类型参数、也不定义任何 `SystemSet`**（`src/dynamics/solver/plugin.rs:68-70` 只有 `length_unit: Scalar` 字段）。所有顺序枚举都在 `schedule.rs`。
- **`SolverPlugin` 的文档（`plugin.rs:49-64`）以"插件组"视角叙述**，把 solver body 准备、关节准备、writeback、restitution、冲量存储都写成"求解器步骤"，但实现上这些系统分属 `SolverBodyPlugin`（`src/dynamics/solver/solver_body/plugin.rs:102-113`）、`XpbdSolverPlugin`（`src/dynamics/solver/xpbd/plugin.rs:44-56, 113-125`）与 `SolverPlugin`（`plugin.rs:111-119`）。读代码时不要指望在 `plugin.rs` 里找到全部。

关节侧的调度点（若启用关节）：

- `JointPlugin` 只配置 `JointSystems::PrepareLocalFrames`，位置为 `.after(SolverSystems::PrepareSolverBodies).before(SolverSystems::PrepareJoints)`（`src/dynamics/joints/mod.rs:258-262`；枚举定义 `:268-272`），且注释明确"does *not* include the actual joint constraint solver"（`:242-243`）。
- 真正的关节求解由 XPBD 承担（见 7.6），它把 `prepare_xpbd_joint::<T>` 链进 `SolverSystems::PrepareJoints`（`src/dynamics/solver/xpbd/plugin.rs:44-56`）。**若关闭 `xpbd_joints`，`PrepareJoints` 集合为空**，关节不被求解，只剩 `joint_damping` 的速度阻尼（`src/dynamics/solver/plugin.rs:139-150` → 函数 `:759-805`）。
- 关节不参与接触图着色：`GraphColor` 只有一个 `contact_constraints` 字段，源码留了 `// TODO: Joints`（`src/dynamics/solver/constraint_graph.rs:79`）。

### 7.2 `SolverBody` / `SolverBodyInertia`：为什么要有中间表示

`src/dynamics/solver/solver_body/mod.rs` 模块文档第 1-8 行："Efficient rigid body definitions used by the performance-critical solver. This helps improve memory locality and makes random access faster for the constraint solver."

**布局澄清**：`SolverBody` 与 `SolverBodyInertia` 都是挂在**每个实体**上的普通 `Component`（即 AoS，每实体一份），**不是 SoA/批量数组**。模块注释只说"改善内存局部性"，并把 2D 布局描述为"为将来 SIMD scatter/gather 优化预留"（`mod.rs:51-53`），布局借鉴 Box2D v3 的 `b2BodyState`（`mod.rs:21`）。不要按"Avian 用 SoA 求解"去理解。

`SolverBody`（`mod.rs:59-91`）字段：

| 字段 | 行 | 说明 |
| --- | --- | --- |
| `linear_velocity: Vector` | 63 | 积分与求解过程中唯一的线速度真值 |
| `angular_velocity: Scalar`(2D) / `Vector`(3D) | 68 / 73 | 同上，角速度 |
| `delta_position: Vector` | 79 | **本帧位移增量**，故意用增量避免远离原点时的舍入误差（76-77 行） |
| `delta_rotation: Rotation` | 86 | 旋转增量；静态体也可用零增量参与（82-84 行） |
| `flags: SolverBodyFlags` | 90 | u32 位图：锁轴 6 位 + `IS_KINEMATIC`（`1 << 6`）+ `GYROSCOPIC_MOTION`（`1 << 7`），见 `mod.rs:132-157` |

尺寸注释：2D `f32` 下 32 字节、3D 56 字节（`mod.rs:49`）。布局灵感来自 Box2D v3 的 `b2BodyState`（`mod.rs:21`）。

`SolverBody::DUMMY`（`mod.rs:95-104`）是**静态体的替身**：零速度、零增量、单位旋转、空 flags。为什么需要替身——文档 30-46 行解释：求解器**无法访问静态体与睡眠体的位姿**，于是采用"Option 1：全部用增量表示"，把静态体当作"零增量的 dummy"。

`SolverBodyInertia`（`mod.rs:218-261`）：2D 直接存 `effective_inv_mass: Vector`（224）+ `effective_inv_angular_inertia: SymmetricTensor`（237）；3D 存 `inv_mass: Scalar`（230）+ 世界空间逆惯量 `effective_inv_angular_inertia: SymmetricTensor`（243）；另有 `dominance: i16`（254，静态/运动学体恒为 `128`，见 247-248 行注释与 `DUMMY` 的 `i8::MAX as i16 + 1`，`mod.rs:274`）与 `flags: InertiaFlags`（260）。`SolverBodyInertia::DUMMY` 在 `mod.rs:265-276`。文件 176-206 行有一段关键设计注释：为什么不把惯量塞进每个约束（2D 16 字节、3D 44 字节的独立结构更省）。

**填充（`prepare_solver_bodies`，`src/dynamics/solver/solver_body/plugin.rs:181-259`）**：

- 从 `LinearVelocity` / `AngularVelocity` 拷贝到 solver body（209-210 行）；
- `delta_position = Vector::ZERO`、`delta_rotation = Rotation::IDENTITY`（211-212 行）——**每帧清零**，这是"没有 PreviousPosition 组件"的原因；
- 构造 `SolverBodyInertia::new(mass.inverse(), angular_inertia.rotated(rotation.0).inverse() /* 3D */, locked_axes, dominance, rb.is_dynamic())`（215-224 行）；
- `flags = SolverBodyFlags(locked_axes.to_bits())` 并置 `IS_KINEMATIC`（225-228 行）；
- 3D 下用 `!locked_axes.is_rotation_locked() && !angular_inertia.inverse().is_isotropic(1e-6)` 决定 `GYROSCOPIC_MOTION`（249-255 行），即在 249 行有 `let epsilon = 1e-6;`。

**回写（`writeback_solver_bodies`，`plugin.rs:263-292`）**：

```rust
let old_world_com = *rot * com.0;
*rot = (solver_body.delta_rotation * *rot).fast_renormalize();
let new_world_com = *rot * com.0;
pos.0 += solver_body.delta_position + old_world_com - new_world_com;
lin_vel.0 = solver_body.linear_velocity;
ang_vel.0 = solver_body.angular_velocity;
```

（`plugin.rs:280-287`）—— 关键细节：旋转是围绕**质心**进行的，所以要补偿 `old_world_com - new_world_com`。旋转用 `fast_renormalize()`（`src/physics_transform/transform.rs:394-400` / 3D 811-817）做一阶归一化，防止误差累积。

> **推论**：Kairos 可以把"body 状态"直接存在刚体组件里（省掉中间层），但必须复刻两条语义：(a) 每帧把位移增量清零；(b) 旋转围绕质心做补偿。少了 (b)，带偏移质心的物体在旋转时会漂移。

### 7.3 接触约束

`ContactConstraint`（`src/dynamics/solver/contact/mod.rs:64-106`）字段：`body1`（`:66`）/`body2`（`:68`）、`relative_dominance: i16`（`:73`）、`friction: Scalar`（`:75`）、`restitution: Scalar`（`:77`）、`tangent_speed`（2D `:84`）/`tangent_velocity`（3D `:91`）、`normal: Vector`（`:93`）、`tangent1`（3D `:96`）、`points: Vec<ContactConstraintPoint>`（`:99`，带 `TODO: Use a SmallVec`）、`contact_id`（`:103`）、`manifold_index`（`:105`）。一个约束对应一个 manifold。

`ContactConstraintPoint`（`contact/mod.rs:32-55`）包含 `normal_part: ContactNormalPart`、`tangent_part: Option<ContactTangentPart>`、`anchor1`/`anchor2`、`normal_speed`、`initial_separation`（构造块在 `contact/mod.rs:166-201`）。

生成路径：`ContactConstraint::generate(...)`（`contact/mod.rs:110-220`，`pub(super)`）由 `prepare_contact_constraints` 调用（`src/dynamics/solver/plugin.rs:424-441`）。要点：

- 用 `SolverBodyInertia::DUMMY` 兜底静态体（`contact/mod.rs:125-126`）；
- `relative_dominance = inertia1.dominance() - inertia2.dominance()`（126 行）；若为正，body1 的逆质量与逆惯量置零（129-148 行）；
- 软度按"是否跨 dominance"选择 `softness.non_dynamic` 或 `softness.dynamic`（`contact/mod.rs:150-154`）；
- 每个点生成 `ContactNormalPart::generate(...)`，并按 `warm_start_enabled` 传入上帧冲量（法向 `contact/mod.rs:181`、切向 `:192`）；
- **摩擦部分只在 `manifold.friction > 0.0` 时生成**（`contact/mod.rs:185-193`）。

每子步的求解函数：`ContactNormalPart::solve_impulse`（`src/dynamics/solver/contact/normal_part.rs:116`）、`ContactTangentPart::solve_impulse`（`src/dynamics/solver/contact/tangent_part.rs:155`）；约束级 `ContactConstraint::warm_start`（`contact/mod.rs:223`）、`solve`（267）、`apply_restitution`（358）。切向方向由 `tangent_directions()`（411）/`compute_tangent_directions`（427）计算。

**实际的接触数学**（这是最小实现最需要抄的部分）：

- 每个点的法向求解在 `ContactNormalPart::solve_impulse`（`src/dynamics/solver/contact/normal_part.rs:116-166`），三个分支：
  - `separation > 0`（尚未接触，推测接触）→ `-m * (v_n + separation / dt)`（`:129-131`）；
  - `USE_BIAS = true` → soft-constraint 形式（`:147-151`）：
    ```rust
    let bias = (self.softness.bias * separation).max(-max_overlap_solve_speed);
    let scaled_mass = self.softness.mass_scale * self.effective_mass;
    let scaled_impulse = self.softness.impulse_scale * self.impulse;
    -scaled_mass * (normal_speed + bias) - scaled_impulse
    ```
    其中 `bias` 就是 Baumgarte 型位置偏置，且被 `max_overlap_solve_speed` 夹紧（防止深穿透被"炸开"）；
  - `USE_BIAS = false`（relax）→ 纯 `-self.effective_mass * normal_speed`（`:152-156`）。
  - 累积冲量最后 `max(0.0)` 夹紧（`:159-161`），并同步累加 `total_impulse`（`:162`，跨子步与 restitution 共用）。
  - 有效质量的推导注释在 `:52-95`（含旋转项 `(r × n)ᵀ I⁻¹ (r × n)`），实现 `:97-111`。
- 摩擦求解在 `ContactTangentPart::solve_impulse`（`src/dynamics/solver/contact/tangent_part.rs:155-244`），Coulomb 锥上限 `let impulse_limit = friction * normal_impulse;`（`:187`，推导注释 `:165-185`），2D 逐分量 `clamp(-impulse_limit, impulse_limit)`（`:199`），3D `clamp_length_max(impulse_limit)`（`:237`）。
- 恢复在 `ContactConstraint::apply_restitution`（`src/dynamics/solver/contact/mod.rs:358-407`）：若 `point.normal_speed > -threshold || point.normal_part.total_impulse == 0.0` 则跳过（`:372-376`），冲量为 `-effective_mass * (normal_speed + self.restitution * point.normal_speed)`（`:387-388`），再夹紧到 ≥ 0（`:390-393`）。跳过条件里的 `total_impulse == 0.0` 是**为兼容推测接触**而设（源码注释）。
- 约束里的分离度是用**增量**重算的：`delta_translation = body2.delta_position - body1.delta_position`，每点 `separation = (delta_rotation * anchor)·normal + initial_separation`（`contact/mod.rs:282-291`）；相对速度用 `SolverBody::velocity_at_point`（`:298`，实现 `src/dynamics/solver/solver_body/mod.rs:107-116`）。这再次说明"delta 表示"是整个求解器的统一约定。

**并行与图着色**：`solve_contacts::<USE_BIAS>`（`src/dynamics/solver/plugin.rs:531-579`）先把 `constraint_graph.colors[COLOR_OVERFLOW_INDEX]` 的"溢出约束"串行求解（544-555 行，注释说明它们优先级更低），再对前 `COLOR_OVERFLOW_INDEX` 个颜色并行求解（557-572 行，`crate::utils::par_for_each`，批大小 64）。`ConstraintGraph` 初始化在 `solver/plugin.rs:93`。颜色常量：`GRAPH_COLOR_COUNT = 24`（`src/dynamics/solver/constraint_graph.rs:39`）、`COLOR_OVERFLOW_INDEX = 23`（`:43`）、`DYNAMIC_COLOR_COUNT = 20`（`:48`，即"两个非静态体之间的约束"只进前 20 色，把含静态体的约束留到后面的颜色以获得更高优先级，从而减少穿隧——该理由写在 `:46-47` 的注释里）。着色本身在 `push_manifold`（`:163-236`）/ `pop_manifold`（`:245-296`）。

**没有 SolverIterations**：`solve_contacts_internal`（`plugin.rs:581-619`）对每个约束**只调用一次** `constraint.solve(...)`，没有内层迭代循环。源码注释说明设计意图（`plugin.rs:520-521`）："This solve is done `iterations` times. With a substepped solver, `iterations` should typically be `1`, as substeps will handle the iteration." 即 **v0.7.0 用子步替代迭代**：默认 6 个子步，每个子步解一次带 bias、一次不带 bias，共 12 次求解。

**配置资源**：`SolverConfig`（`src/dynamics/solver/plugin.rs:216-289`），默认值在 291-302 行：

| 字段 | 行 | 默认 | 语义 |
| --- | --- | --- | --- |
| `contact_damping_ratio` | 227 | `10.0` | 接触稳定阻尼比；越小越"软/弹" |
| `contact_frequency_factor` | 240 | `1.5` | 接触频率倍率 |
| `max_overlap_solve_speed` | 250 | `4.0` | 分离重叠的最大速度，被 `PhysicsLengthUnit` 缩放（247 行） |
| `warm_start_coefficient` | 262 | `1.0` | 热启动冲量系数，`[0,1]` |
| `restitution_threshold` | 275 | `1.0` | 触发恢复的最小法向速度，被长度单位缩放（272 行） |
| `restitution_iterations` | 288 | `1` | 恢复的迭代次数；仅当接触点 > 1 时有意义（713 行 `let iterations = if point_count > 1 { iterations } else { 1 };`） |

`ContactSoftnessCoefficients`（`plugin.rs:310-315`）是自动维护的资源（306-307 行明确"不打算手工修改"），默认值用 `SoftnessParameters::new(10.0, 30.0)` / `new(10.0, 60.0)` 生成（317-324 行）；`update_contact_softness`（326-350）按 Nyquist 限制频率上限 `max_hz = 1/(dt*2)`（338 行）、`hz = contact_frequency_factor * max_hz.min(0.25 / h)`（339 行），并对"静态/运动学接触"用 `2.0 * hz` 使其更硬，以避免穿透环境（344-348 行注释）。`SoftnessParameters` / `SoftnessCoefficients` 定义在 `src/dynamics/solver/softness_parameters/mod.rs:18, 87`，核心方法是 `new(damping_ratio, frequency_hz)`（36）与 `compute_coefficients(delta_secs)`（64）；该目录另有 `README.md` 说明推导。

**热启动链**：`warm_start`（`plugin.rs:453-482`）遍历约束图，每子步把上一子步的冲量按 `warm_start_coefficient` 重新施加到两个 body 的速度上（`contact/mod.rs:223-264`）；`store_contact_impulses`（`plugin.rs:722-755`）在物理步末把 `ContactConstraints` 的冲量写回 `ContactGraph` 的 `ContactPoint` 字段（`plugin.rs:741-750`；目标字段 `src/collision/contact_types/mod.rs` 中的 `warm_start_normal_impulse` / `warm_start_tangent_impulse`），供下一帧使用（`SolverSystems::StoreContactImpulses` 的文档 `src/dynamics/solver/schedule.rs:112-116`）。注意 `StoreContactImpulses` 排在 `Finalize` **之后**（`schedule.rs:42`）。

**没有 `writeback_contacts` 系统**：body 层的回写只有 `writeback_solver_bodies`（`src/dynamics/solver/solver_body/plugin.rs:263-292`），接触层的"回写"就是把冲量写回 `ContactGraph`。

热启动冲量**不存在 `SolverBody` 里**，而是存在接触点（`ContactConstraintPoint.normal_part.impulse` / `total_impulse` 与 `tangent_part.impulse`，`src/dynamics/solver/contact/normal_part.rs:16-34`、`tangent_part.rs:17-31`）。

### 7.4 岛屿与睡眠（`islands/`）

`src/dynamics/solver/islands/mod.rs`（1410 行）与 `sleeping.rs`（626 行）。

**关键结构性问题：岛屿图不是"每帧重建"，而是增量维护的。** `IslandPlugin`（`mod.rs:71-156`）**只注册了一个系统**：`split_island.in_set(SolverSystems::Finalize)`（`mod.rs:150-153`）。其余全是 observer（为启用中的动态/运动学体插入 `BodyIslandNode`，`mod.rs:80-93`、`94-111`；移除 `BodyIslandNode` 的三个 observer，`mod.rs:118-148`）与 `register_required_components::<SolverBody, BodyIslandNode>()`（`mod.rs:76`）。

**算法**：源码模块文档列出三种候选方案并写明"Avian uses persistent islands"（`islands/mod.rs:39`），实现借鉴 Box2D（`:42-43`）。组合方式是 **持久化岛屿 + union-find 式合并 + 延迟 DFS 分裂**：每步最多分裂一个岛，选"有约束被移除且 sleep timer 最大"的岛作为候选（`:6-7`、`PhysicsIslands::split_candidate` 字段 `:420-426`）。

数据结构（**用链表把 body/contact/joint 串成三条链**，而非每帧重建集合）：

- `PhysicsIsland` 字段：`id`、`head_body`/`tail_body`/`body_count`、`head_contact`/`tail_contact`/`contact_count`、`head_joint`/`tail_joint`/`joint_count`、`sleep_timer: f32`、`is_sleeping: bool`、`constraints_removed: u32`（`islands/mod.rs:209-228`）。
- 链表节点 `IslandNode<Id> { island_id, prev, next, is_visited }`（`islands/mod.rs:1266-1277`）。
- `PhysicsIslands` 资源字段：`islands: StableVec<PhysicsIsland>`、`split_candidate: Option<IslandId>`、`split_candidate_sleep_timer: f32`（`islands/mod.rs:414-425`）。CRUD/迭代 API 在 `:431-501`。

真正的图更新发生在**窄相位**：

- `PhysicsIslands::add_contact(...)`（`islands/mod.rs:509`）与 `remove_contact(...)`（`mod.rs:590`）由 `src/collision/narrow_phase/system_param.rs:249, 343`（add）与 `201, 311`（remove）调用，另有 `src/collision/narrow_phase/mod.rs:458`。
- `add_joint(...)`（`mod.rs:665`）/ `remove_joint(...)`（`mod.rs:745`）由 `src/dynamics/solver/joint_graph/plugin.rs:147, 342`（add）与 `179, 318`（remove）调用。
- `merge_islands(...)`（`islands/mod.rs:810-986`）由 `add_contact`（526 行）与 `add_joint`（675 行）内部调用。合并策略：保留 **body 数量更多**的岛（必要时交换，`:839-845`），把被吞并岛中所有 body/contact/joint 的 `island_id` 重映射（`:850-872`），拼接三条链表（`:877-958`），累加 `constraints_removed`（`:961`），若小岛在睡则大岛保持睡且取 `sleep_timer` 最大值（`:963-968`）。
- `split_island(...)`（`mod.rs:991-1262`）只在 `split_candidate` 被设置时执行（`split_island` 系统，`mod.rs:157-176`），注释明确"Splitting is only done when bodies want to sleep."（`mod.rs:168`）。它跳过已睡的岛（`:1001-1004`）与 `constraints_removed == 0` 的岛（`:1006-1009`），用显式栈做 DFS 重建多个岛（`:1074-1261`），只沿"仍未移除的约束边"遍历（接触 `:1128-1136`、关节 `:1196-1246`）。
- `remove_contact`（`mod.rs:590-657`）与 `remove_joint`（`mod.rs:745-801`）都会把 `constraints_removed` 加一（`:648`、`:788`）——这正是"分裂候选"的触发条件。

类型：`IslandId(pub u32)`（`mod.rs:181`）、`PhysicsIsland`（`mod.rs:209`）、`PhysicsIslands` 资源（`mod.rs:414`，在 `mod.rs:73` 被 `init_resource`）、`IslandNode<Id>`（`mod.rs:1268`）、`BodyIslandNode(IslandNode<Entity>)`（`mod.rs:1313`，其 `on_add`/`on_remove` 钩子在 1323/1338 行）。调试校验函数 `validate`（290）/`validate_bodies`（302）/`validate_contacts`（339）/`validate_joints`（377）。

**睡眠**：`IslandSleepingPlugin`（`sleeping.rs:44-84`）：

- `init_resource::<AwakeIslandBitVec>()`、`init_resource::<TimeToSleep>()`（46-47 行）；
- `register_required_components::<SolverBody, SleepThreshold>()` 与 `::<SolverBody, SleepTimer>()`（50-51 行）；
- `Sleeping` 的组件钩子：`on_add(sleep_on_add_sleeping)` / `on_remove(wake_on_remove_sleeping)`（63-66 行）；
- 两个 observer：`wake_on_replace_rigid_body`（68）、`wake_on_enable_rigid_body`（69）；
- 系统链（`sleeping.rs:71-83`，全部在 `PhysicsStepSystems::Sleeping`，且 `run_if(resource_exists::<PhysicsIslands>)`）：
  ```
  update_sleeping_states → wake_islands_with_sleeping_disabled → wake_on_changed
  → wake_all_islands (run_if resource_changed::<Gravity>) → sleep_islands
  ```

`update_sleeping_states`（`sleeping.rs:184-241`）用 `Res<PhysicsLengthUnit>`（196 行）与 `length_unit_squared = length_unit.0 * length_unit.0`（200 行）把阈值缩放到世界单位，比较 `lin_vel_squared < length_unit_squared * lin_threshold_squared`（217 行），查询过滤是 `(Without<Sleeping>, Without<SleepingDisabled>)`（194 行）；低于阈值则 `sleep_timer += delta_secs`，否则清零（`:221-225`）；未达 `TimeToSleep` 就把岛屿标为 awake（`:227-229`）；达阈值且该岛 `constraints_removed > 0` 时竞选 `split_candidate`（`:230-238`）。注意它读的是 **`SolverBody` 的速度**而不是组件速度（solver body 才是求解后的真值）。

其余系统：`wake_islands_with_sleeping_disabled`（`sleeping.rs:164-182`）覆盖 `SleepingDisabled`/`Disabled`/`RigidBodyDisabled` 并清零 sleep timer；`sleep_islands`（`sleeping.rs:243-280`）只在未被标 awake 且 `constraints_removed == 0` 时才排队睡（`:260-263`），通过 `commands.queue` 应用 `SleepIslands` / `WakeIslands`（`:266-276`）；`wake_on_changed`（`sleeping.rs:566-614`）用 `LastPhysicsTick` + `is_changed_after_tick` 排除引擎自身的写回（`:600-609`），并对 `ConstantForce*` / `ConstantTorque*` / `GravityScale` 的变化无条件唤醒（类型别名 `:543-562`，判定 `:611-613`）；`wake_all_islands`（`sleeping.rs:617-626`）在 `Gravity` 变更时唤醒全部睡岛。

手动控制用**命令**（不是资源）：`SleepBody(pub Entity)`（`sleeping.rs:294-345`，先在有待移除约束时强制 `split_island` `:314-324`）、`SleepIslands(pub Vec<IslandId>)`（`:362-434`，置 `is_sleeping = true` `:380`，用 `constraint_graph.pop_manifold` 逐个摘除约束 `:390-422`，批量插入 `Sleeping` `:431`）、`WakeBody(pub Entity)`（`:451-471`）、`WakeIslands(pub Vec<IslandId>)`（`:474-541`，置 `is_sleeping = false` `:492`，用 `push_manifold` 重新入图 `:503-526`，清零 sleep timer `:530`，逐个移除 `Sleeping` `:536-538`）。

**睡眠在调度上的位置**：`PhysicsStepSystems::Sleeping` 排在 `Solver` **之后**（`src/schedule/mod.rs:96-107`）——即"先按当前睡眠状态求解，再决定谁该睡"。唤醒后的体在**下一帧**才有 `SolverBody`（observer 是 `On<Remove, Sleeping>`，`src/dynamics/solver/solver_body/plugin.rs:74-84`）。

唤醒清单（`src/dynamics/rigid_body/sleeping.rs:18-27` 的权威列表）：苏醒体撞到睡眠体、与睡眠体建立关节、从睡眠体移除关节或接触、修改睡眠体的 `Transform`/`LinearVelocity`/`AngularVelocity`、改变 `RigidBody` 类型、修改常量力组件、通过 `Forces` 施加力（未用 `non_waking`）、修改 `Gravity` 资源或 `GravityScale`。手动休眠/唤醒可用 `SleepBody` / `WakeBody` 命令（`sleeping.rs:29-31`，类型在 `src/dynamics/solver/islands/`）。

**施力唤醒的实现点**：`Forces` 的所有写入方法都调用内部 `try_wake_up()`（`src/dynamics/rigid_body/forces/query_data.rs:687-693`，它清零 `SleepTimer` 并唤醒所属岛），调用点如 `:297-301`、`:345`、`:359`、`:389`；`forces.non_waking()` 走的 `NonWakingForcesItem` 把这一步替换为恒真（`query_data.rs:696-729`）。

### 7.5 关节图（`joint_graph/`）

**目录更正**：`src/dynamics/solver/joints/` 不存在；关节本体在 `src/dynamics/joints/`，而 solver 侧与关节相关的只有 `joint_graph/` 与 `xpbd/`。

`JointGraph` 是资源：`graph: StableUnGraph<Entity, JointGraphEdge>`、`entity_to_body`、`entity_to_joint`（`src/dynamics/solver/joint_graph/mod.rs:26-30`）；`JointId(pub u32)` 与 `PLACEHOLDER = u32::MAX`（`:37-42`）；边类型 `JointGraphEdge { id, entity, body1, body2, collision_disabled, island: IslandNode<JointId> }`（`:67-93`）——**注意 `island` 字段**：岛归属就挂在图的边上，`JointGraph` 自己不做从零建岛（`islands/mod.rs:220-222`、`:1196-1246`）。

`solver/joint_graph/plugin.rs` 负责为每种关节类型维护该图并与岛屿图保持同步（`JointGraphPlugin<T>` 定义 `:30-36`，build `:60-116`：`init_resource::<JointGraph>()` `:65`、`register_required_components::<T, JointComponentId>()` `:69`、组件 hooks `:72-75`、`on_change_joint_entities::<T>` 入 `PhysicsStepSystems::First` `:110-115`）：

- 加关节：`JointGraphEdge::new`（142 行）→ `joint_graph.add_joint`（143 行）→ `islands.add_joint(...)`（147-152 行）→ 若该岛在睡则发 `WakeIslands`（154-159 行）。
- 移除关节：先 `islands.remove_joint(...)`（179-184 行）并唤醒（186-189 行），再 `joint_graph.remove_joint(entity)`（193 行）。
- 关节实体变更：`on_change_joint_entities`（`:299-360`）检测 `body1`/`body2` 变化（`:315`），移除旧边（`:317-329`）、重新 `add_joint`（`:332-348`）、最后排序去重并唤醒（`:353-359`）。
- `JointCollisionDisabled` 关闭碰撞：`on_disable_joint_collision`（`:247-296`）会对 collider 较少的一方遍历接触并 `pop_manifold`（`:289`）。

它也是少数直接查询 `RigidBodyColliders` 的地方（`plugin.rs:16, 249`）。实例化点：`SolverPlugins` 为 `FixedJoint` / `RevoluteJoint` / `PrismaticJoint` / `DistanceJoint`（以及 3D 的 `SphericalJoint`）各加一个 `JointGraphPlugin<T>`（`src/dynamics/solver/mod.rs:74-80`）。

### 7.6 XPBD 关节（`solver/xpbd/`，feature gate）

- 目录：`xpbd/mod.rs`(441)、`xpbd/plugin.rs`(328)、`xpbd/angular_constraint.rs`(296)、`xpbd/positional_constraint.rs`(99)、`xpbd/joints/{distance,fixed,prismatic,revolute,spherical,mod}.rs`、`xpbd/joints/shared/{point_constraint,fixed_angle_constraint,mod}.rs`。
- **feature gate**：`xpbd_joints = []` 声明在 `crates/avian2d/Cargo.toml:47` 与 `crates/avian3d/Cargo.toml:48`，并被两个 crate 的 default feature 列表包含（`crates/avian2d/Cargo.toml:20`、`crates/avian3d/Cargo.toml:20`）；示例的 `required-features` 见 `crates/avian3d/Cargo.toml:157`。代码侧 `src/dynamics/solver/mod.rs:15-16` 用 `#[cfg(feature = "xpbd_joints")] pub mod xpbd;`，插件在 `mod.rs:82-83` 条件添加，并在 `src/dynamics/mod.rs:73-74` 条件导出 `XpbdSolverPlugin`。
- **作用范围**：把关节当作**位置级约束**求解（XPBD），与基于冲量的接触求解器并存。三个 trait：`XpbdConstraintSolverData`（`mod.rs:300-318`）、`XpbdConstraint`（`mod.rs:321-377`，`prepare` `:326-330`、`solve` `:350-356`、`warm_start_motors` 默认空实现 `:367-376`）、辅助 trait `PositionConstraint`（`positional_constraint.rs:7-98`）与 `AngularConstraint`（`angular_constraint.rs:6-296`）。
- **compliance 数学**（源码文档即此）：`Δλ = -C / (Σ wᵢ|∇Cᵢ|² + α/h²)`（`mod.rs:213-222`）；广义逆质量 `wᵢ = 1/mᵢ + (rᵢ × ∇Cᵢ)ᵀ Iᵢ⁻¹ (rᵢ × ∇Cᵢ)`（`:251-266`）；角修正 `Δqᵢ = 0.5 [Iᵢ⁻¹(rᵢ × (Δλ ∇Cᵢ)), 0] qᵢ`（`:275-279`）。实现 `compute_lagrange_update_with_gradients`（`:389-412`，`tilde_compliance = compliance / dt²` `:409`，返回 `(-c - tilde*λ)/(w_sum + tilde)` `:411`）。示例调用 `DistanceJoint::solve` 用 `compute_lagrange_update(0.0, distance, &w, self.compliance, dt)`（`joints/distance.rs:93`）。
- **调度**：`XpbdSolverSystems::SolveConstraints → SolveUserConstraints → VelocityProjection` 三者 `chain().after(SubstepSolverSystems::Relax).before(SubstepSolverSystems::Damping)`（`xpbd/plugin.rs:31-41`）——即**接在接触 relax 之后、关节阻尼之前**。`warm_start_xpbd_motors` 入 `SubstepSolverSystems::WarmStart`（`:62-71`，复用 `solver_config.warm_start_coefficient` `:253`）；`writeback_joint_forces` 入 `SolverSystems::Finalize`（`:113-125`，把冲量写进 `JointForces` 组件，`:324-326`）。
- 它使用 `PreSolveDeltaPosition` / `PreSolveDeltaRotation` 作为"求解前快照"，并用 `project_linear_velocity` / `project_angular_velocity` 把位置修正投影回速度（`xpbd/plugin.rs:74-102, 259-307`）。这正是这两个组件被列进 `RigidBody` required components 的原因（`src/dynamics/rigid_body/mod.rs:279-281` 的 TODO 注释：等关节不再用 XPBD 就可以删掉它们）。
- 由于默认开启，`XpbdSolverPlugin` 在默认构建中会被加入（`solver/mod.rs:82-83`）。**推论**：Kairos 第一版完全可以不实现 XPBD，但要知道 Avian 默认是"冲量接触求解 + XPBD 关节"双求解器并存。

---

## 8. 连续碰撞检测（CCD，`src/dynamics/ccd/mod.rs`）

### 8.1 问题与两种方案

模块文档（`ccd/mod.rs:5-25`）说明 **tunneling**：离散步进下，未被检测到碰撞的快速小物体可以穿过薄几何体。v0.7.0 提供两种手段：

1. **Speculative collision（推测接触）**——**默认开启**（`ccd/mod.rs:24`）。用速度扩张 AABB，并用 **speculative margin** 决定"最大多远还生成推测接触"（`ccd/mod.rs:29-35`）。
2. **Swept CCD（扫掠）**——**需要显式加 `SweptCcd` 组件**（`ccd/mod.rs:24`），把物体从上一位置扫到当前位置做 TOI 查询（`ccd/mod.rs:129-131`）。两者可同时使用（`ccd/mod.rs:25`）。

Speculative collision 的已知代价（`ccd/mod.rs:84-123`）：**ghost collision**（接触面被求解器当作无限平面，表现为撞到"看不见的墙"，86-90 行），可通过更小的 `SpeculativeMargin` 或更高的物理帧率缓解（111-112 行）；也可能漏检（114-116 行）；还会吸收能量（118-119 行）。Swept CCD 更贵但没有这些问题，且**目前只支持内置 `Collider`**（`ccd/mod.rs:127`）。

### 8.2 组件与配置

| 类型 | 行 | 说明 |
| --- | --- | --- |
| `CcdPlugin` | `ccd/mod.rs:248` | `PhysicsSchedule` 中的 CCD 插件 |
| `SweptCcdSystems` | `ccd/mod.rs:270` | 唯一的集合；被配置为 `.after(SolverSystems::PostSubstep).before(SolverSystems::Restitution)`（257-261 行） |
| `solve_swept_ccd` | `ccd/mod.rs:523` | 唯一注册的系统（264 行），仅在 `any(feature = "parry-f32", "parry-f64")` 下注册（263 行） |
| `SpeculativeMargin(pub Scalar)` | `ccd/mod.rs:308` | 逐体推测边距上限；`ZERO`（312）禁用推测碰撞，`MAX`（315）无界。默认无界，全局默认由 `NarrowPhaseConfig` 给出（280-283 行） |
| `SweptCcd { mode, include_dynamic, linear_threshold, angular_threshold }` | `ccd/mod.rs:389-419` | `mode`（397，默认 `NonLinear`）、`include_dynamic`（402，默认 `true`）、`linear_threshold`（410，默认 `0.0`）、`angular_threshold`（418，默认 `0.0`） |
| `SweepMode { Linear, NonLinear }` | `ccd/mod.rs:479-498` | `NonLinear` 是 `#[default]`（496 行）；同时考虑平移与旋转 |

`SweptCcd` 的便捷常量：`SweptCcd::LINEAR`（432）、`SweptCcd::NON_LINEAR`（437）、`new_with_mode`（441）、`with_velocity_threshold`（456）、`include_dynamic`（465）。注意 `SweptCcd::default()` 等于 `NON_LINEAR`（421-425 行）。

**没有 `Ccd` 或 `CcdEnabled` 组件**——扫掠式 CCD 的开关就叫 `SweptCcd`。

### 8.3 算法（`solve_swept_ccd`，`ccd/mod.rs:523-…`）

查询 `SweptCcdBodyQuery`（`ccd/mod.rs:501-512`）取 `SolverBody`（`Option`）、`RigidBody`、`Position`、`Rotation`、`SweptCcd`（`Option`）、`Collider`、`ComputedCenterOfMass`。

流程：

1. 遍历所有带 `SweptCcd` 的实体（539 行），取其 `solver_body1`、当前位置/旋转、`SweptCcd` 配置（541-557 行）。**注意**：这里的 `pos`/`rot` 被当作 `prev_pos1`/`prev_rot1` 使用（543-544 行的字段绑定名），因为该系统运行在 `PostSubstep` 之后、`writeback_solver_bodies` 之前——此时组件的 `Position`/`Rotation` 仍是本帧起点，而增量在 `SolverBody` 里。
2. `min_toi` 初值为 `delta_secs`（564 行）。
3. 用 `contact_graph.entities_colliding_with(entity)` 取 AABB 相交的碰撞体（567 行），逐个获取对方 body（568-575 行）；若 `!include_dynamic && body2.rb.is_dynamic()` 则跳过（576-578 行）。
4. 阈值过滤：相对线速度与相对角速度都低于阈值则跳过（590-602 行）。
5. 构造 `parry::query::NonlinearRigidMotion::new(iso, com, lin_vel, ang_vel)`（608-609 行），并按双方 `SweepMode` 决定实际模式（**只要任一方是 NonLinear 就用 NonLinear**，611-617 行）。
6. `compute_ccd_toi(sweep_mode, &motion1, collider1, &motion2, collider2, min_toi, narrow_phase_config.default_speculative_margin)`（619-627 行）更新 `min_toi` 与 `min_toi_entity`（628-629 行）。
7. 把双方从上一姿态**推进到第一个 TOI**：`min_toi *= 1.0001`（646-647 行的"略微过冲以免卡住"），然后直接**赋值**（不是累加）`solver_body1.delta_position = min_toi * lin_vel1`（649 行）与 `delta_rotation`（652-660 行），body2 同理（662-672 行）。**2D 的 `delta_rotation` 是整体替换，3D 是左乘到已有值之上**（`delta_rotation.0 = delta_rot * delta_rotation.0`，659 行）。

`compute_ccd_toi`（`ccd/mod.rs:679-770`）的契约写在 679-680 行："If the TOI is larger than `min_toi` or the shapes never touch, `None` is returned."

- `SweepMode::Linear` 分支（`:691-732`）：`cast_shapes(&motion1.start, motion1.linvel, collider1.shape_scaled(), &motion2.start, motion2.linvel, collider2.shape_scaled(), ShapeCastOptions { max_time_of_impact: min_toi, stop_at_penetration: false, ..default() })`（`:692-705`）——只用 `.start` 姿态 + 线速度，**不含角运动**；接受条件 `toi > 0.0 && toi < min_toi`（`:707-710`）。
- `SweepMode::NonLinear` 分支（`:733-767`）：`cast_shapes_nonlinear(motion1, collider1.shape_scaled(), motion2, collider2.shape_scaled(), 0.0, min_toi, false)`（`:736-744`）——传入完整 `NonlinearRigidMotion`（含角速度），因此**含旋转的 TOI**。
- 两个分支在 `toi == 0.0` 时都退化：把 `parry::shape::Ball::new(prediction_distance)` 当作**第二个** shape 再 cast 一次（`:711-731`、`:748-766`）。
- **`min_toi` 的单位是秒**（初值 `delta_secs`，`ccd/mod.rs:564`；位移按 `min_toi * lin_vel` 计算，`649`），不是 `[0, 1]` 比例时间。

**关键语义：CCD 在 TOI 处不创建任何接触或约束。** 它只是把 body 的位移增量改小（"移回时间点"），让下一帧的正常碰撞检测去处理这次碰撞——文档 `ccd/mod.rs:129-132` 原文如此。源码明确记录代价："time loss" / "time stealing"、二级接触不被考虑（`:517-520`），以及尚无 substepped TOI solver（`:193-208`）。

> **推论（需要注意的实现副作用）**：因为 `delta_position` 是被**赋值**而非累加，被 CCD 命中的两个 body 在子步期间由位置约束/关节产生的位移修正会被一并丢弃。

**调度位置的意义**：`SweptCcdSystems` 位于 `SolverSystems::PostSubstep` 之后、`Restitution` 之前（`ccd/mod.rs:257-261`）。也就是说 CCD **不参与本帧的约束求解**，只是把位移增量"截断"到 TOI（并在 647 行略微过冲），从而避免穿透；真正的接触响应要等下一帧的窄相位/求解器。这也解释了为什么它必须早于 `Finalize`（`writeback_solver_bodies`）。

### 8.4 推测接触（默认开启的那一半）与 `SpeculativeMargin` 的实际消费点

推测接触**不是**由 CCD 插件实现的，而是分散在 collider tree 与窄相位里：

- **AABB 按速度扩张**：`src/collider_tree/update.rs:738` 计算 `growth = Vector::splat(contact_tolerance + collision_margin)`；`:756-757` 计算 `movement = (vel * delta_secs).clamp_length_max(speculative_margin.max(contact_tolerance))`；`:775-777` 用 `swept_aabb_with_context(...).grow(growth)`。若 `speculative_margin <= 0.0` 则退化为普通 AABB（`:740-743`）。
- **有效 margin 与接触距离**：`src/collision/narrow_phase/system_param.rs:665-685` 用 `delta_secs * |lin_vel2 - lin_vel1|` 并把速度按 margin 截断；`:687-691` 得到 `max_contact_distance = effective_speculative_margin.max(contact_tolerance) + collision_margin_sum`；流形生成在 `:700-715`（这与 `ContactNormalPart::solve_impulse` 的 `separation > 0` 分支 `normal_part.rs:129-131` 配套：正分离度的接触就是推测接触）。
- `SpeculativeMargin` 是**组件**（`ccd/mod.rs:305-308`），消费点在 `src/collider_tree/update.rs:731-735`（带 `SweptCcd` 的实体会被强制设为 `Scalar::MAX`）与 `src/collision/narrow_phase/system_param.rs:652-661`（collider 优先、否则 body）。全局默认来自 `NarrowPhaseConfig::default_speculative_margin`，默认 `Scalar::MAX`（`src/collision/narrow_phase/mod.rs:222, 252`）；`contact_tolerance` 默认 `0.005`（`:236, 253`）。`DefaultFriction` / `DefaultRestitution` 也由 `NarrowPhasePlugin` 在此处 `init_resource`（`src/collision/narrow_phase/mod.rs:114-115`）。
- **一个单位不一致的观察（源码事实）**：`solve_swept_ccd` 本身**不使用** `SpeculativeMargin`，而是把 `narrow_phase_config.default_speculative_margin` 直接当球半径传入 `compute_ccd_toi`（`ccd/mod.rs:626`，形参名 `prediction_distance` `:689`）；该配置在窄相位与 collider tree 中都会乘 `PhysicsLengthUnit`（`src/collision/narrow_phase/system_param.rs:130-131`、`src/collider_tree/update.rs:707`），而 CCD 传入的是**未缩放**的原始值。
- `CollisionHooks` / `ActiveCollisionHooks` 是唯一的钩子抽象（`src/collision/hooks.rs:147, 227`），只在 broad/narrow phase 使用（`src/collision/narrow_phase/system_param.rs:117` 的 `update::<H>`），**CCD 完全不参与钩子**。

> **推论**：Kairos 若只做"重力 + 静态地面"这一阶段，可以完全不碰 CCD；只要在固定时间步下速度不过大（`v·dt < 物体厚度`）就不会穿透。

---

## 9. 材质与接触响应（`src/dynamics/rigid_body/physics_material.rs`）

### 9.1 组合规则 `CoefficientCombine`

枚举定义在 `physics_material.rs:13-24`，**五个变体**（含判别值，用于 `Ord` 优先级）：

| 变体 | 行 | 判别值 | 语义 |
| --- | --- | --- | --- |
| `Average` | 16 | `1` | `(a + b) / 2.0` |
| `GeometricMean` | 18 | `2` | `sqrt(a * b)` |
| `Min` | 20 | `3` | `min(a, b)` |
| `Multiply` | 22 | `4` | `a * b` |
| `Max` | 24 | `5` | `max(a, b)` |

**冲突优先级**：文件 6-7 行规定 `Max > Multiply > Min > GeometricMean > Average`；实现上因为枚举有显式判别值，`combine` 里直接取 `self.combine_rule.max(other.combine_rule)`（例如 `physics_material.rs:206`、`373`），所以优先级就是判别值大小顺序。`CoefficientCombine::mix(a, b)` 在 28-38 行。

### 9.2 `Friction` / `Restitution` 组件与默认资源

| 类型 | 行 | 字段 / 默认 |
| --- | --- | --- |
| `Friction` | `physics_material.rs:137-150` | `dynamic_coefficient`（默认 `0.5`）、`static_coefficient`（默认 `0.5`）、`combine_rule`（默认 `Average`）；`Default` 在 152-160 行 |
| `Restitution` | `physics_material.rs:305-318` | `coefficient`（默认 `0.0`）、`combine_rule`（默认 `Average`）；`Default` 在 320-328 行；`#[doc(alias = "Bounciness")]` / `"Elasticity"`（303-304 行） |
| `DefaultFriction(pub Friction)`（资源） | `physics_material.rs:48` | 默认动态/静态 `0.5` + `Average`（43 行） |
| `DefaultRestitution(pub Restitution)`（资源） | `physics_material.rs:59` | 默认 `0.0` + `Average`（54 行） |

方法：`Friction::new(c)`（172）、`with_combine_rule`（181）、`with_dynamic_coefficient`（189）、`with_static_coefficient`（197）、`combine`（205-216）；常量 `Friction::ZERO`（约 164-168 行）。`Restitution::new`（356）、`with_combine_rule`（364）、`combine`（372-381）；常量 `ZERO` / `PERFECTLY_INELASTIC` / `PERFECTLY_ELASTIC`（约 331-350 行）。两者都实现了 `From<Scalar>`（`Friction` 217，`Restitution` 383）。

**没有 `PhysicsMaterial` 组件**：解析链是"碰撞体自己的 `Friction`/`Restitution` → 其刚体的 → 全局默认资源"（`physics_material.rs:71-73` 与 232-236 行的文档描述）。

### 9.3 从材质组件到接触约束的完整解析链

1. **窄相位组合**：`src/collision/narrow_phase/system_param.rs:602-627`。

   ```rust
   let friction = collider1.friction.or(rb_friction1).copied()
       .unwrap_or(self.default_friction.0)
       .combine(collider2.friction.or(rb_friction2).copied()
           .unwrap_or(self.default_friction.0))
       .dynamic_coefficient;                       // 行 604-619
   let restitution = collider1.restitution.copied()
       .unwrap_or(self.default_restitution.0)
       .combine(collider2.restitution.copied()
           .unwrap_or(self.default_restitution.0))
       .coefficient;                               // 行 620-627
   ```

   注意三个细节：(a) `Friction` 是"碰撞体优先、其次刚体"，而 `Restitution` 只查碰撞体再回退到资源；(b) 只取 `dynamic_coefficient`，静态摩擦系数不进入约束；(c) 组合规则本身也随 `combine` 一起求出（`Friction::combine` 返回带新 `combine_rule` 的值，206-215 行）。
2. **写入流形**：`manifold.friction = friction;`（`src/collision/narrow_phase/system_param.rs:720`），restitution 相邻写入。
3. **进入约束**：`ContactConstraint::generate` 从 manifold 取 `friction: manifold.friction` / `restitution: manifold.restitution`（`src/dynamics/solver/contact/mod.rs:207-208`）；`manifold.friction > 0.0` 才生成 `ContactTangentPart`（`contact/mod.rs:185-193`）。
4. **求解**：
   - 摩擦：`friction` 作为参数传入切向求解（`contact/mod.rs:336-345`，`self.friction` 在 `:343`），冲量上限是 `let impulse_limit = friction * normal_impulse;`（`src/dynamics/solver/contact/tangent_part.rs:187`，推导注释 `:165-185`），2D 逐分量 `clamp(-impulse_limit, impulse_limit)`（`:199`）、3D `clamp_length_max(impulse_limit)`（`:237`）。
   - 恢复：由独立系统 `solve_restitution`（`src/dynamics/solver/plugin.rs:621-673`）在 `SolverSystems::Restitution` 阶段应用，`solve_restitution_internal` 在 `restitution == 0.0` 时直接返回（`:675-683`）；阈值 `let threshold = solver_config.restitution_threshold * length_unit.0;`（`:640`）。核心冲量在 `ContactConstraint::apply_restitution`（`src/dynamics/solver/contact/mod.rs:358-407`）：`-effective_mass * (normal_speed + self.restitution * point.normal_speed)`（`:387-388`），命中条件与夹紧见 `:372-376`、`:390-393`。
   - **摩擦/恢复系数本身既不缩放也不夹紧**：全仓没有对 `dynamic_coefficient` / `static_coefficient` / `coefficient` 的 `clamp` 调用；被 `PhysicsLengthUnit` 缩放的只有阈值类量（`restitution_threshold` `plugin.rs:640`、`max_overlap_solve_speed` `:542`、`default_speculative_margin`、`contact_tolerance`、睡眠阈值 `sleeping.rs:200`）。
   - `DefaultFriction` / `DefaultRestitution` 两个资源由 `NarrowPhasePlugin` 初始化：`src/collision/narrow_phase/mod.rs:114-115`。

**两处源码与文档不一致（可直接引用的事实）**：

- `Restitution` **没有 body 级回退**。`Friction` 的解析是 `collider.friction.or(rb_friction).unwrap_or(default)`（`src/collision/narrow_phase/system_param.rs:606-618`），而 `Restitution` 是 `collider.restitution.unwrap_or(default_restitution)`（`:619-629`），没有 `.or(rb_restitution)`；与此同时 `Friction` 文档 `:71-73` 与 `Restitution` 文档 `:233-235` 都声称存在 body 级回退。`RigidBodyQuery::restitution` 字段被声明（`src/dynamics/rigid_body/world_query.rs:23`）但从未被读取。
- `Friction::static_coefficient` 在接触约束路径中**未被使用**：窄相位只取 `.dynamic_coefficient`（`:618`），`Friction::combine` 虽然同时组合了动/静摩擦（`physics_material.rs:210-211`），但结果里的静态系数没有下游消费者。

**已知精度问题**（源码自述）：`src/dynamics/solver/plugin.rs:621-624` 指出"restitution 与 TGS Soft + 推测接触一起用时可能不够精确"，这是"便宜的 CCD 比完美恢复更重要"的取舍。

---

## 10. 里程碑：最小可运行刚体 + 重力驱动

目标：动态体在重力下下落，落在静态碰撞体上静止，并且每个固定步把结果写回 `Transform`。以下给出**有序构建清单**与**必须在场的不变量**。

### 10.1 最小组件集（Kairos 侧）

| 角色 | Avian 对应 | Avian 定义位置 |
| --- | --- | --- |
| 刚体类型 | `RigidBody::{Dynamic, Static}` | `src/dynamics/rigid_body/mod.rs:284-304` |
| 全局位姿 | `Position` / `Rotation` | `src/physics_transform/transform.rs:48, 745` |
| 速度 | `LinearVelocity` / `AngularVelocity` | `src/dynamics/rigid_body/mod.rs:412, 543` |
| 质量 | `Mass`（输入）+ `ComputedMass`（输出） | `components/mod.rs:160`；`components/computed.rs:48` |
| 转动惯量 | `AngularInertia` + `ComputedAngularInertia` | `components/mod.rs:536`；`components/computed.rs:428` |
| 质心 | `CenterOfMass` + `ComputedCenterOfMass` | `components/mod.rs:915`；`components/computed.rs:776` |
| 重力 | `Gravity` 资源（默认 `Vector::Y * -9.81`） | `src/dynamics/integrator/mod.rs:156, 158-162` |
| 每帧中段累加器 | `VelocityIntegrationData` | `src/dynamics/integrator/mod.rs:216-233` |
| 增量 | `SolverBody.delta_position` / `delta_rotation`（或等价的独立组件） | `src/dynamics/solver/solver_body/mod.rs:79, 86` |
| 阻尼（可选） | `LinearDamping` / `AngularDamping` | `src/dynamics/rigid_body/mod.rs:605, 629` |
| 材质 | `Friction`（仅 `dynamic_coefficient`）/ `Restitution` | `physics_material.rs:137, 305` |

Avian 把它们全部塞进 `RigidBody` 的 `#[require(...)]`（`src/dynamics/rigid_body/mod.rs:267-282`），读者可以直接照这个清单做 required components。

### 10.2 步骤顺序（按依赖排，非按重要性排）

1. **同步位姿**：把用户改的 `Transform` 同步到 `Position`/`Rotation`（Avian：`PhysicsTransformSystems::TransformToPosition`，`src/physics_transform/mod.rs:106-111`），并在 `PhysicsSystems::Prepare` 且 **早于** 质量重算（`mass_properties/mod.rs:308` 的 `.after(TransformToPosition)`）。
2. **重算质量属性**：碰撞体密度 → `ColliderMassProperties` → 合并后代 → `ComputedMass` / `ComputedAngularInertia` / `ComputedCenterOfMass`（Avian 三段 chain：`mass_properties/mod.rs:299-309`）。**没有 `ComputedMass` 的逆质量，后面的求解器全部失效**（`src/dynamics/solver/solver_body/plugin.rs:188-190, 215-224`）。
3. **准备求解体**：拷贝速度到求解器局部状态，清零增量，计算世界空间逆惯量（`solver_body/plugin.rs:181-259`，增量清零在 211-212 行）。
4. **预计算每帧速度增量**：常量力/扭矩/加速度 → 重力 = `gravity * GravityScale` → 锁轴过滤 → `*= dt_substep`（`integrator/mod.rs:260-313` 与 `forces/plugin.rs:96-202`）。
5. **粒子步循环 `n = SubstepCount`**（默认 6，`solver/schedule.rs:185-191`；循环 `solver/schedule.rs:194-213`），每个子步内严格按 `SubstepSchedule` 的链（`solver/schedule.rs:59-69`）：
   a. 阻尼 + 速度增量 → `integrate_velocities`（`integrator/mod.rs:343-391`）；
   b. 速度截断（`integrator/mod.rs:467-500`，可选）；
   c. **先积分速度、后积分位置**（`integrator/mod.rs:73-76` 的 `.chain()`；位置积分在 `integrator/mod.rs:503-535`）；
   d. 位置积分只累加到 **增量**，不直接改 `Position`（`integrator/mod.rs:521, 525, 529-530`）。
6. **检测与求解**：接触生成与求解（Avian 把接触检测放在 `PhysicsSchedule` 的 `BroadPhase`/`NarrowPhase`，求解在 `Solver`，见 `src/schedule/mod.rs:96-107`）。**Kairos 的最小版本可以退化为"位置积分后立即做一次穿透检测 + 位置/速度修正"**（见 10.4 的说明）。
7. **写回**：`pos += delta_position`，`rot = delta_rotation * rot`（**围绕质心**，`solver_body/plugin.rs:280-283`），并把速度写回组件（286-287 行）。
8. **清空累加器**：`VelocityIntegrationData` 增量清零（`integrator/mod.rs:316-328`）与 `AccumulatedLocalAcceleration` 清零（`forces/plugin.rs:243-251`），都在 `SolverSystems::PostSubstep`（`integrator/mod.rs:58-60`；`forces/plugin.rs:31`）。
9. **写回 `Transform`**：`position_to_transform`（`src/physics_transform/mod.rs:118-123`），过滤条件是 `Or<(Changed<Position>, Changed<Rotation>)>`。

### 10.3 必须遵守的顺序约束（写错任意一条都会出隐蔽 bug）

| 约束 | Avian 的落点 |
| --- | --- |
| `Transform -> Position` 必须在质量重算之前 | `mass_properties/mod.rs:308` 的 `.after(PhysicsTransformSystems::TransformToPosition)` |
| 质量必须在求解器之前算好 | 质量在 `PhysicsSystems::Prepare`（`mass_properties/mod.rs:307`），求解在 `PhysicsSchedule`（`src/schedule/mod.rs:101`） |
| 常量力必须在重力/阻尼预计算之前 | `forces/plugin.rs:29-30` 的 `.before(integrator::pre_process_velocity_increments)` |
| 重力是加速度，不乘质量 | `integrator/mod.rs:298` 与 `forces/plugin.rs:102` 的对比（后者才 `mass.inverse() * force`） |
| 速度增量是在**子步 dt** 上折算 | `integrator/mod.rs:270, 275, 307-308`；`Time<Substeps>` 在 `src/schedule/mod.rs:254` 提前推进 |
| 速度积分必须严格早于位置积分（symplectic 顺序） | `integrator/mod.rs:73-76` 的 `.chain()`；`solver/schedule.rs:59-69` |
| 位置积分产出的必须是**增量**，写回时一次性应用并做质心补偿 | `integrator/mod.rs:521-531`；`solver_body/plugin.rs:280-283` |
| 增量累加器必须在物理步末尾清空 | `integrator/mod.rs:58-60, 69, 316-328`；`forces/plugin.rs:31, 243-251` |
| 局部加速度必须在子步内应用（朝向会变） | `forces/plugin.rs:34-39, 60-61, 207-241` |
| 睡眠/禁用体必须被排除在积分与求解之外 | `RigidBodyActiveFilter`（`rigid_body/mod.rs:329`）+ 移除 `SolverBody`（`solver_body/plugin.rs:94-99`） |
| 阻尼必须与速度增量在同一处、同一次乘法里应用（先阻尼后加增量） | `integrator/mod.rs:363-368`（顺序在代码里是显式的） |
| `Transform` 写回必须在物理步之后 | `src/physics_transform/mod.rs:114-123`（`PhysicsSystems::Writeback`） |
| 锁轴要在"施加加速度/冲量"时过滤，而不是事后清零速度 | `integrator/mod.rs:301-303`；`forces/plugin.rs:226-230`；`query_data.rs:390-394` |

### 10.4 可以推迟的部分，以及推迟后各自会坏成什么样

| 可推迟项 | Avian 位置 | 推迟后的后果 |
| --- | --- | --- |
| **子步（`SubstepCount`）** | `src/dynamics/solver/schedule.rs:185-213` | 单步 `dt` 变大：堆叠体抖动/穿透更明显。**推论**：先做单步 + 多次约束迭代也能跑，但要把"每帧一次预计算、每子步一次积分"的结构留好，否则后期改成子步要重构 |
| **岛屿（`PhysicsIslands`）** | `src/dynamics/solver/islands/mod.rs:414` | 无岛屿 = 只能逐体睡眠，且无法做"整岛一起睡/醒"。对"一个球落在一块地上"完全无影响 |
| **睡眠（`Sleeping` / `SleepThreshold` / `TimeToSleep`）** | `src/dynamics/rigid_body/sleeping.rs:61, 84, 143`；`islands/sleeping.rs:71-83` | 静止物体永远在算，速度会有微小抖动（求解器误差），但不会崩。注意 Avian 的睡眠是**移除 `SolverBody`**（`solver_body/plugin.rs:94-99`），若 Kairos 用标志位实现，记得在所有积分/求解查询里过滤 |
| **CCD** | `src/dynamics/ccd/mod.rs:248, 308, 389` | 高速物体会穿透薄墙。若速度上界满足 `v·dt < 厚度`，可无限期推迟 |
| **关节与关节图** | `src/dynamics/solver/joint_graph/plugin.rs`；`solver/mod.rs:74-80` | 无关节，只是没有约束链。`JointGraphPlugin` 也参与岛屿合并（`joint_graph/plugin.rs:147`），所以推迟关节同时也简化了岛屿 |
| **XPBD（`xpbd_joints`）** | `crates/avian3d/Cargo.toml:48`；`src/dynamics/solver/mod.rs:15-16, 82-83` | 不做关节就不需要。`PreSolveDeltaPosition`/`PreSolveDeltaRotation` 这两个组件（`rigid_body/mod.rs:280-281`）也可以一起省掉 |
| **马达与关节阻尼** | `SubstepSolverSystems::Damping`（`solver/schedule.rs:132, 147-148`）；`joint_damping`（`solver/plugin.rs:759`） | 只有关节马达会受影响 |
| **陀螺力矩（gyroscopic torque）** | `src/dynamics/integrator/mod.rs:372-387, 403-460`；启用判定 `solver_body/plugin.rs:232-256` | 只有"非各向同性惯量张量"的物体会失去进动/章动行为（如细长棒在空中旋转的抖动）。球形/立方体等各向同性体本来就被判定为不启用（`plugin.rs:236-244` 的注释给了充分理由）。对"球落在地上"零影响 |
| **热启动（warm starting）** | `SubstepSolverSystems::WarmStart`（`solver/schedule.rs:135-140`）；`warm_start`（`solver/plugin.rs:453-515`）；`store_contact_impulses`（722） | 收敛变慢：堆叠体会更明显地塌陷/抖动、接触更"弹"。单物体落地基本无感。启用热启动就要一并实现 `StoreContactImpulses`，否则每帧都是零初始冲量 |
| **Relax 阶段（无 bias 二次求解）** | `SubstepSolverSystems::Relax`（`solver/schedule.rs:144-146`）；`solve_contacts::<false>`（`solver/plugin.rs:136`） | 源码说明若无它，热启动会导致 overshooting（`schedule.rs:137-139`）。若同时推迟热启动，则 relax 也可以一起省 |
| **推测接触 / `SpeculativeMargin`** | `src/dynamics/ccd/mod.rs:308` | 快速物体更容易穿薄几何 |
| **TGS Soft / 软度参数** | `softness_parameters/mod.rs`；`ContactSoftnessCoefficients`（`solver/plugin.rs:310`） | 需要自定一套接触刚度/阻尼（Baumgarte + 阻尼），否则接触会发抖或过软 |
| **退化/支配（`Dominance`）** | `src/dynamics/rigid_body/mod.rs:662` | 只有"想让某动态体无视碰撞被推开"的场景需要（如玩家角色），可后置 |
| **`LockedAxes`** | `src/dynamics/rigid_body/locked_axes.rs:32` | 2D 游戏里常用（锁定 Z 旋转）。若推迟，注意后面加时要同步改"逆惯量矩阵"与"加速度过滤"两处 |
| **`MaxLinearSpeed` / `MaxAngularSpeed`** | `src/dynamics/rigid_body/mod.rs:441, 471` | 只是安全阀，可后置；但缺了它，一次错误的力会让物体瞬移 |
| **`PhysicsLengthUnit`** | `src/dynamics/solver/plugin.rs:201` | 若 Kairos 用米为单位可以完全忽略；一旦做"像素单位 2D"，睡眠阈值、分离速度上限、恢复阈值都需缩放（`plugin.rs:247, 272`；`sleeping.rs:87`） |
| **`CustomVelocityIntegration` / `CustomPositionIntegration`** | `src/dynamics/integrator/mod.rs:182, 195` | 只影响自定义运动学控制（如 `MoveAndSlide`） |

### 10.5 一个可直接照抄的最小实现骨架

> **推论**（把上述源码事实浓缩成读者可实现的形态）：

```
每固定步 dt：
  1. 同步：Transform -> Position/Rotation（若用户改了 Transform）
  2. 重算质量：每个动态体的 ComputedMass / ComputedAngularInertia / ComputedCenterOfMass
     （含子碰撞体、含 ColliderTransform 的平行轴平移）
  3. 预计算（每帧一次）：
       for body in dynamic:
           acc = Sum(constant forces/mass) + gravity * GravityScale   // 加速度，不乘质量
           acc = locked_axes(acc)
           body.velocity_increment = acc * dt_substep
           body.damping_rhs = 1 / (1 + dt_substep * damping)
  4. for i in 0..substep_count:          // Avian 的 SubstepSchedule 链，见 solver/schedule.rs:59-69
       for body in active:                // 排除 disabled / sleeping
           body.vel = body.vel * damping_rhs + velocity_increment   // integrator/mod.rs:363-368
           clamp_speed(body.vel)                                      // :467-500
       warm_start_contacts()              // 有热启动时才有
       solve_contacts(use_bias = true)    // normal_part.rs:147-151
       for body in active:
           body.delta_pos += body.vel * dt_substep                    // integrator/mod.rs:521
           body.delta_rot  = integrate(dt_substep * body.vel_ang) * body.delta_rot  // :525, :529-530
       solve_contacts(use_bias = false)   // relax，反 overshoot
  5. 写回（SolverSystems::Finalize -> writeback_solver_bodies）：
       for body in active:
           rot_old = rot
           rot  = normalize(delta_rot * rot)
           pos += delta_pos + (rot_old * com - rot * com)   // 质心补偿，solver_body/plugin.rs:280-283
           LinearVelocity = body.vel ; AngularVelocity = body.vel_ang
           delta_pos = 0 ; delta_rot = identity
       clear velocity_increment / accumulated_local_acceleration   // :316-328 / forces/plugin.rs:243-251
  6. Position/Rotation -> Transform（仅在变化时，physics_transform/mod.rs:118-123）
```

如果第一步只做单步（`substep_count = 1`），流程等价于"积分速度 → 检测求解 → 积分位置"，但**务必保留"速度积分在位置积分之前"这个顺序**（`integrator/mod.rs:73-76` 的 `.chain()`）。

接触求解的最小替代方案：**推论**——在 `substep_count = 1` 的早期阶段，可以先实现"离散检测 + 位置修正 + 速度反射"（把穿透沿法线推出，并把法向相对速度按 `Restitution` 反向、按 `Friction` 削减切向速度），这等价于"一次迭代的 Baumgarte 稳定化"，足以让球停在地上；等到要支持堆叠时再升级为"顺序冲量 + 热启动 + 多子步"。Avian 的接触数学参考实现位于 `src/dynamics/solver/contact/normal_part.rs:116` 与 `tangent_part.rs:155`（每个约束点的冲量求解），约束装配在 `src/dynamics/solver/contact/mod.rs:110-220`。

---

## 11. 本文件未验证 / 不确定

1. **未执行构建或测试**：所有结论来自静态阅读，未运行 `cargo build` / `cargo test`（任务要求）。源码中的单测（如 `integrator/mod.rs:561-629`、`mass_properties/mod.rs:451-932`、`forces/tests.rs`、`physics_material.rs:392-452`）被当作"官方行为规格"引用，但**未实际运行验证**。所有 `#[cfg]` 分支的展开也没有经过编译期验证。
2. **`Time<Substeps>` 被推进两次的语义**（最重要的一条不确定）：`src/schedule/mod.rs:250-255` 与 `src/dynamics/solver/schedule.rs:199-200` 都以同一个 `sub_delta = dt / substeps` 调用了 `advance_by(sub_delta)`。本文件依据 Bevy `Time::advance_by` 的"覆写 `delta`、累加 `elapsed`"语义，推断"两次推进后 `delta` 仍为 `sub_delta`，因此同一物理步的所有子步共用同一 dt（只有 `elapsed` 会双倍累加）"。**Bevy 侧源码不在本 checkout 内，未核对**。若该假设有误，"子步 dt"与"每帧 dt"的相对关系需要重新确认；这直接影响 `pre_process_velocity_increments`（`integrator/mod.rs:307-308`）与 `integrate_positions`（`:510`）使用的 dt。
3. **陀螺力矩的启用判定边界**：判定式为 `!locked_axes.is_rotation_locked() && !angular_inertia.inverse().is_isotropic(epsilon)`（`src/dynamics/solver/solver_body/plugin.rs:250-251`），`epsilon = 1e-6`。未核对 `ComputedAngularInertia::is_isotropic` 的实现与数值边界，因此"哪些具体形状会被判定为各向同性"仅按源码注释（`plugin.rs:236-244`）转述。
4. **`Friction::static_coefficient` 的消费点未确证**：窄相位只取 `.dynamic_coefficient` 进入流形（`src/collision/narrow_phase/system_param.rs:618`），我在 `contact/` 目录内也未见读取静态系数的代码，但**"全仓无消费者"这一结论基于 grep，而非逐行通读全部求解路径**。相应地，`Friction` 文档声称的"静止时用静态摩擦、滑动后用动态摩擦"在 v0.7.0 的实现中并未体现为两套系数。
5. **岛屿合并路径的穷尽性**：`add_contact` / `remove_contact` / `add_joint` / `remove_joint` 的调用点是通过 `grep` 列出的（窄相位与 `joint_graph`），因此"岛屿图只在这两处被增量修改"属于基于 grep 的判断；`IslandPlugin` 本身确实只注册 `split_island` 一个系统（`islands/mod.rs:150-153`，已逐行阅读 `impl Plugin` 全段）。
6. **CCD 读取的 `Position`/`Rotation` 是"本帧起点"**：这是**推论**——依据是 `SweptCcdSystems` 的调度位置（`ccd/mod.rs:257-261`，在位置写回 `SolverSystems::Finalize` 之前）与字段绑定名 `pos: &prev_pos1`（`:543-544`）。源码注释未显式声明这一点。
7. **两处源码注释与实现不一致（已确证为事实，但意图不明）**：
   - `solve_contacts` 的文档注释仍写"This solve is done `iterations` times"（`src/dynamics/solver/plugin.rs:517-521`），但函数体内**没有**迭代循环（`:531-579`），每个约束每子步只求解一次。
   - `SolverBodyInertia` 的字节数注释自相矛盾：`src/dynamics/solver/solver_body/mod.rs:213` 写 3D 为 32 bytes，而 `:196-206` 的注释与字段声明（`:229-260`）合计为 44 bytes。
   - CCD 的 `toi == 0.0` 回退注释写"around the centroid of the first body"（`src/dynamics/ccd/mod.rs:711-712`），但代码把 `Ball` 作为**第二个** shape 放在 `motion2` 上（`:713-725`、`:751-759`）。
8. **parry 版本差异**：`cast_shapes_nonlinear` 的位置参数名（`start_time` / `end_time` / `stop_at_penetration`）是参照本机存在的 `parry3d-0.28.0` 源码得出的，而 Avian 0.7.0 依赖 parry3d **0.27**（`crates/avian3d/Cargo.toml:92`）。因此这些形参名属**推论**；调用点的实参个数与语义（`0.0, min_toi, false`）来自 Avian 源码本身，是事实。
9. **材质解析中的一个未解之谜**：`Restitution` 缺少 body 级回退（`src/collision/narrow_phase/system_param.rs:619-629`）与文档声明（`physics_material.rs:233-235`）矛盾，我无法确认这是遗漏还是刻意的性能取舍；`grep` 也未发现任何把 body 的 `Restitution` 复制/继承到 collider 实体的 observer。
10. **未覆盖的相邻主题**：宽相位 `src/collision/broad_phase/`、窄相位内部实现（流形生成与裁剪、`contact_types`、`CollisionHooks` 的全部语义）、`character_controller`、`debug_render`、`picking`、`src/data_structures/`（`StableVec` / `BitVec`）、`crate::utils::par_for_each` 的批大小语义、`benches/`、`src/tests/`、`crates/*/examples/`。`bevy_heavy` 内部的质量属性解析公式（圆/球/胶囊/凸包的体积与惯量）不在本 checkout 的 Avian 源码中，本文件未涉及。
11. **行号漂移风险**：本文件的行号严格对应 commit `965e85bf590f53fbf8a29f7ebd0326b5dbc985d1`（tag `v0.7.0`）。若读者使用其他 tag/分支，行号会偏移，但类型名与调度关系大体稳定。
