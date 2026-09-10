# Avian 0.7.0 Collider 子系统参考

> **权威来源**：只读 checkout `/Users/baiaoxiang/KairosEngine/.scratch/avian-research/avian-0.7.0`
> tag `v0.7.0`，commit `965e85bf590f53fbf8a29f7ebd0326b5dbc985d1`。
> **本文件中所有结构性命断的引用路径都相对于该 checkout 根目录。**
>
> **路径布局警告**：`crates/avian3d/Cargo.toml:66-70` 设置 `[lib] path = "../../src/lib.rs"`。
> 也就是说 `avian2d` / `avian3d` 两个 crate **没有自己的源码目录**，全部引擎代码位于仓库根 `src/`，
> 通过 `#[cfg(feature = "2d")]` / `#[cfg(feature = "3d")]` 区分维度。
> 本文件凡涉及维度差异处均显式标注。
>
> **本文档定位**：读者（Kairos）的 `kairos_physics` 目前只包装 `rapier3d 0.33`，只支持静态 box "plane"、
> 动态 sphere + ball collider、重力与简单碰撞响应。本文是**碰撞体子系统（第一个里程碑）**的施工参考，
> 目标是读完 + 对照被引用的源码后，能独立实现「静态形状存在于 world 中，并正确注册进空间索引」的子系统。
>
> 标注约定：默认叙述为 **源码事实**；带「推论」前缀的段落是**未在源码中直接明写的推断**。

---

## 0. 一页速览（TL;DR）

v0.7.0 的碰撞体架构与旧版 Avian（0.1–0.5）差别很大，**不要凭记忆写代码**：

1. **`Collider` 结构体只有 3 个字段**，全部与「形状」有关：
   `shape` / `scaled_shape` / `scale`（`src/collision/collider/parry/mod.rs:366-376`）。
2. **密度、摩擦、弹性、传感器、碰撞层、碰撞余量、禁用标记全部被拆成了独立组件**：
   `ColliderDensity`、`Friction`、`Restitution`、`Sensor`、`CollisionLayers`、
   `CollisionMargin`、`ColliderDisabled`。它们**不是** `Collider` 的字段，也**并非全部**是 required component。
3. **`Collider` 是一个具体的 Parry 后端实现**，不是枚举、不是泛型。后端可替换性由
   `AnyCollider` / `SimpleCollider` / `ScalableCollider` 三个 trait + `ColliderBackendPlugin<C>` 提供
   （`src/collision/collider/mod.rs:128-342`、`src/collision/collider/backend.rs:68-248`）。
4. **空间索引是 `obvhs` 的 BVH**，封装为 `ColliderTree`，按刚体类型分成 **4 棵树**
   （dynamic / kinematic / static / standalone）（`src/collider_tree/mod.rs:121-130`）。
5. **树本身就是 broad phase 的加速结构**；broad phase 是另一个插件 `BvhBroadPhasePlugin`，
   它查询这些树来生成 `ContactEdge`（`src/collision/broad_phase/bvh_broad_phase.rs:20-31,51-207`）。
   两者**共存**，0.7.0 的默认 broad phase 就是 BVH broad phase（`src/lib.rs:783`）。
6. **碰撞层过滤发生在 broad phase 的树遍历里**，具体一行是
   `src/collision/broad_phase/bvh_broad_phase.rs:257`。
7. `Group` / `InteractionGroups` 在 0.7.0 **已不存在**（全仓 grep 无命中），统一为 `CollisionLayers`。

---

## 1. `Collider` 组件：真实定义与全部字段

### 1.1 结构体定义

```rust
// src/collision/collider/parry/mod.rs:356-376
#[derive(Clone, Component, Debug)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[require(
    ColliderMarker,
    ColliderAabb,
    CollisionLayers,
    EnlargedAabb,
    ColliderDensity,
    ColliderMassProperties
)]
pub struct Collider {
    /// The raw unscaled collider shape.
    shape: SharedShape,          // :368
    /// The scaled version of the collider shape.
    scaled_shape: SharedShape,   // :373
    /// The global scale used for the collider shape.
    scale: Vector,               // :375
}
```

**字段逐条说明**：

| 字段 | 行号 | 类型 | 含义与物理效果 |
|---|---|---|---|
| `shape` | `src/collision/collider/parry/mod.rs:368` | `parry::shape::SharedShape` | **未缩放的原始形状**。所有几何查询最终都走 `shape_scaled()`，此字段只在 `set_scale(ONE)` 或 `set_shape` 时被直接克隆使用。 |
| `scaled_shape` | `src/collision/collider/parry/mod.rs:373` | `parry::shape::SharedShape` | **已应用全局 scale 的形状**。注释明确：scale 为 `Vector::ONE` 时它等价于 `shape`。所有碰撞/AABB/质量计算都用它（`:546`、`:411-412`、`:448`）。 |
| `scale` | `src/collision/collider/parry/mod.rs:375` | `Vector` | 全局缩放因子，来源是实体的 `GlobalTransform` scale（`src/collision/collider/backend.rs:124-140`）或子碰撞体的 `ColliderTransform::scale`（`src/collision/collider/collider_transform/mod.rs:26`）。非均匀缩放无法精确表达时会被近似为凸包/凸多面体（`src/collision/collider/parry/mod.rs:567-592`）。 |

**关键结论**：`Collider` **没有** `density`、`mass`、`friction`、`restitution`、`linear_damping`、
`sensor`、`collision_groups`、`collision_layers`、`contact_skin`、`additional_mass`、`offset` 这些字段。
读者若照抄旧版 Avian 的 `Collider { shape, density, friction, ... }` 会得到完全错误的架构。

> **推论**：这次拆分是 ECS 化的进一步推进——把「几何」与「材质/过滤/语义」解耦，
> 使得 Sparse Set 查询（如只查 `Friction`）不必拖着整个 `SharedShape` 走。

### 1.2 字段的访问与修改 API

| 方法 | 行号 | 说明 |
|---|---|---|
| `shape()` | `src/collision/collider/parry/mod.rs:536` | 返回未缩放 `&SharedShape` |
| `shape_mut()` | `src/collision/collider/parry/mod.rs:541` | 可变引用（**不会**同步 `scaled_shape`，需自行 `set_shape`） |
| `shape_scaled()` | `src/collision/collider/parry/mod.rs:546` | 返回应用 scale 后的形状——**管线里全部用这个** |
| `set_shape()` | `src/collision/collider/parry/mod.rs:551` | 设置原始形状并用当前 scale 重新生成 `scaled_shape`（细分次数硬编码为 10，`:554-555` 有 TODO） |
| `scale()` | `src/collision/collider/parry/mod.rs:563` | 读取全局 scale |
| `set_scale()` | `src/collision/collider/parry/mod.rs:574-592` | 设置 scale；`ONE` 走捷径（`:579-584`），失败时 `log::error!`（`:590`） |

`AnyCollider` 实现：`src/collision/collider/parry/mod.rs:401-441`，其中 `type Context = ()`
（即 Parry 后端不需要额外 world 上下文），`aabb_with_context` 在 `:404-417`，
`contact_manifolds_with_context` 在 `:419-440`。

`ScalableCollider` 实现：`src/collision/collider/parry/mod.rs:524-532`（纯转发给固有方法）。

### 1.3 默认值

```rust
// src/collision/collider/parry/mod.rs:388-399
impl Default for Collider {
    fn default() -> Self {
        #[cfg(feature = "2d")] { Self::rectangle(0.5, 0.5) }
        #[cfg(feature = "3d")] { Self::cuboid(0.5, 0.5, 0.5) }
    }
}
```

即 **3D 默认是 1×1×1 的立方体**（`cuboid` 参数是全长，内部乘 0.5 得半长，见 `:747-749`），
2D 默认是 1×1 的矩形（`:741-743`）。

### 1.4 被拆出去的组件及其默认值

这些组件**全部不在** `Collider` 的 `#[require(...)]` 列表中，除了 `ColliderDensity` 与 `CollisionLayers`。

| 组件 | 定义位置 | 默认值 | 物理效果 |
|---|---|---|---|
| `ColliderDensity(pub f32)` | `src/dynamics/rigid_body/mass_properties/components/collider.rs:32` | `1.0`（`:34-38`） | 与形状体积/惯量共同决定该 collider 对刚体质量的贡献。`ColliderDensity::ZERO` 在 `:42`。 |
| `ColliderMassProperties(MassProperties)` | `src/dynamics/rigid_body/mass_properties/components/collider.rs:80` | `MassProperties::ZERO`（`ZERO` 常量在 `:84`） | **只读**、自动计算（`:45-50` 文档明写）。手动插入无效。 |
| `Friction { dynamic_coefficient, static_coefficient, combine_rule }` | `src/dynamics/rigid_body/physics_material.rs:137-151` | 动/静系数均 `0.5`，规则 `CoefficientCombine::Average`（`:152-160`） | 接触约束求解时使用的库仑摩擦参数。 |
| `Restitution { coefficient, combine_rule }` | `src/dynamics/rigid_body/physics_material.rs:305-319` | `coefficient: 0.0`、`Average`（`:320-327`） | 弹性/回弹系数，0 = 完全非弹性。 |
| `Sensor` | `src/collision/collider/mod.rs:429` | `Default`（`:425`） | 触发器：只发事件/登记相交，不产生约束；**不贡献质量**（`:402`、`backend.rs:149-153`）。 |
| `CollisionLayers { memberships, filters }` | `src/collision/collider/layers.rs:362-369` | `CollisionLayers::DEFAULT` = `{ memberships: 1, filters: ALL }`（`:373-376`、`Default` 在 `:430-434`） | 决定谁能和谁交互。注意 `#[component(immutable)]`（`:358`）。 |
| `CollisionMargin(pub Scalar)` | `src/collision/collider/mod.rs:669` | `Default`（`Scalar` 零值） | 形状外的额外厚度/"skin"，改善稳定性；文档给出三条理由（`:614-626`）。别名 `ContactSkin`（`:668`）。 |
| `ColliderDisabled` | `src/collision/collider/mod.rs:394` | 单元结构 | 临时禁用：不参与碰撞检测与空间查询，**但仍贡献质量**（`:350-351`）。**只作用于自身 entity，不传递给 children**（`:353`）。 |
| `CollidingEntities(pub EntityHashSet)` | `src/collision/collider/mod.rs:704` | 空集合 | 手动添加后才生效（`:672` 明写 "Must be added manually"），用于读「谁在撞我」。 |

**摩擦/弹性的回退链**（源码事实，非猜测）：
`narrow_phase/system_param.rs:610-629` 显示顺序为
**collider 自己的 `Friction` → 刚体的 `Friction` → `DefaultFriction` 资源**；
`Restitution` 同理回退到 `DefaultRestitution`。
两个资源由 `NarrowPhasePlugin` 初始化（`src/collision/narrow_phase/mod.rs:114-115`），
类型定义在 `src/dynamics/rigid_body/physics_material.rs:48` 与 `:59`。

---

## 2. Required components / 自动伴随组件

`Collider` 上身的组件有**三个来源**，必须全部检查：

### 2.1 来源 A：结构体上的 `#[require(...)]`

`src/collision/collider/parry/mod.rs:358-365`：

| 组件 | 定义 | 一句话职责 |
|---|---|---|
| `ColliderMarker` | `src/collision/collider/backend.rs:256` | 后端无关的「这是碰撞体」标记，用于跨后端过滤 entity（`:251-253` 文档明写）。由 `on_remove` hook 自动移除（`:166-173`）。 |
| `ColliderAabb` | `src/collision/collider/mod.rs:438-443` | world space 的紧密 AABB。 |
| `CollisionLayers` | `src/collision/collider/layers.rs:362` | 碰撞层成员/过滤。 |
| `EnlargedAabb` | `src/collision/collider/mod.rs:579` | AABB + margin，**真正放进 BVH 的框**（`:568-574`）。 |
| `ColliderDensity` | `src/dynamics/rigid_body/mass_properties/components/collider.rs:32` | 密度。 |
| `ColliderMassProperties` | `src/dynamics/rigid_body/mass_properties/components/collider.rs:80` | 由形状+密度算出的该碰撞体质量属性。 |

### 2.2 来源 B：`ColliderBackendPlugin::build` 里的 `try_register_required_components*`

`src/collision/collider/backend.rs:96-104`：

```rust
let _ = app.try_register_required_components_with::<C, Position>(|| Position::PLACEHOLDER);   // :97
let _ = app.try_register_required_components_with::<C, Rotation>(|| Rotation::PLACEHOLDER);   // :98
let _ = app.try_register_required_components::<C, ColliderMarker>();                          // :99
let _ = app.try_register_required_components::<C, ColliderAabb>();                            // :100
let _ = app.try_register_required_components::<C, EnlargedAabb>();                            // :101
let _ = app.try_register_required_components::<C, CollisionLayers>();                          // :102
let _ = app.try_register_required_components::<C, ColliderDensity>();                          // :103
let _ = app.try_register_required_components::<C, ColliderMassProperties>();                   // :104
```

`Position` / `Rotation` 定义在 `src/physics_transform/transform.rs:48` 与 `:745`（3D 为
`Rotation(pub Quaternion)`，2D 为标量角度版 `:175`），二者的 `PLACEHOLDER` 常量分别在 `:54` 与 `:752`。
占位值语义由 `init_physics_transform` 消费：`src/physics_transform/transform.rs:1116-1151`
会把占位 `Position` 变成 `Vector::ZERO`、占位 `Rotation` 变成 `Rotation::IDENTITY`，
并沿 `ChildOf` 向上累乘 `Transform` 求出初始位姿。

> **意图**：`:97-104` 与结构体 `#[require]` 内容有重叠。原因见 `backend.rs:33-39` 的文档——
> 该插件是**泛型后端**插件，对任意 `C: ScalableCollider` 都要保证这套伴随组件存在，
> 而 `#[require]` 只对具体类型有效。**推论**：这是为了支持自定义碰撞体类型而保留的双保险。

### 2.3 来源 C：`ColliderTreePlugin`

`src/collider_tree/mod.rs:59-63`：

```rust
let _ = app.try_register_required_components_with::<C, ColliderTreeProxyKey>(|| {
    // Use a default proxy key. This will be overwritten when the proxy is actually created.
    ColliderTreeProxyKey::PLACEHOLDER
});
```

`ColliderTreeProxyKey` 定义在 `src/collider_tree/proxy_key.rs:15`，
`PLACEHOLDER` 在 `:19`（`u32::MAX`）。它把一个 `ProxyId`（30 bit）与
`ColliderTreeType`（低 2 bit）打包进一个 `u32`（`:22-45`）。
**这是碰撞体在空间索引中的句柄**，也是生命周期观察者用来定位 proxy 的关键（见 §5.3）。

### 2.4 与刚体的关系：`ColliderOf` / `RigidBodyColliders` / `ColliderTransform`

```rust
// src/collision/collider/collider_hierarchy/mod.rs:47-56
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[component(immutable, on_insert = <ColliderOf as Relationship>::on_insert,
                          on_discard = <ColliderOf as Relationship>::on_discard)]
#[require(ColliderTransform)]
pub struct ColliderOf {
    pub body: Entity,
}
```

| 类型 | 位置 | 角色 |
|---|---|---|
| `ColliderOf` | `src/collision/collider/collider_hierarchy/mod.rs:53` | **Relationship**：把 collider 挂到某个 `RigidBody` entity 上。未手动指定时自动指向最近的 rigid body 祖先（`:20-21`）。 |
| `RigidBodyColliders(Vec<Entity>)` | `src/collision/collider/collider_hierarchy/mod.rs:212` | **RelationshipTarget**：刚体侧的反向索引（`#[relationship_target(relationship = ColliderOf, linked_spawn)]`，`:208`）。文档明确警告不要手改（`:203-204`）。 |
| `ColliderTransform` | `src/collision/collider/collider_transform/mod.rs:20-27` | collider 相对**刚体**的局部变换（`translation`/`rotation`/`scale`），自动更新，不应手改（`:13-15`）。 |

`ColliderOf` 的 `Relationship` 实现有两个非默认之处：
1. `ALLOW_SELF_REFERENTIAL = true`（`:72`）——因为 Bevy 不允许 relationship 指向自身，
   这里手写实现来绕过限制（`:67-68` 注释）。
2. `on_insert`（`:82-148`）在插入时会**立即用 `GlobalTransform` 之差初始化 `ColliderTransform`**
   （`:104-108`），并把 collider push 进 `RigidBodyColliders`（`:132-139`）。

自动挂载逻辑在 `src/collision/collider/collider_hierarchy/plugin.rs`：
观察 `On<Add, (RigidBody, ColliderMarker)>` 找最近 rigid body 祖先（`:15-40`）；
`On<Remove, (RigidBody, ColliderMarker)>` 时移除（`:43-54`）；
祖先变化时重算（`on_collider_body_changed`，`:66-116`）；
刚体被移除时清理 `ColliderOf` + `ColliderTransform`（`on_body_removed`，`:119-137`）。

### 2.5 `Collider` 的组件 hooks

`src/collision/collider/backend.rs:111-187`：

| hook | 行号 | 行为 |
|---|---|---|
| `on_add` | `:115-122` | 若 entity 上**没有** `RigidBody`，调用 `init_physics_transform` 初始化位姿（`:119-121`）。注释坦承对 rigid body 的特判是个 hack（`:118`）。 |
| `on_insert` | `:123-160` | (1) 从 `GlobalTransform::scale()` 取 scale 并 `set_scale(scale.adjust_precision(), 10)`（`:124-140`）；(2) 读 `ColliderDensity`；(3) 若带 `Sensor` 则质量属性为 `MassProperties::ZERO`，否则 `collider.mass_properties(density.0)`（`:149-153`）；(4) 写回 `ColliderMassProperties`（`:155-159`）。 |
| `on_remove` | `:164-187` | 移除 `ColliderMarker`（`:170-173`），若存在 `ColliderOf` 则给刚体插入 `RecomputeMassProperties`（`:178-186`）。 |

另外注册了两个 observer 处理 `Sensor` 的增删对质量的影响：
`On<Add, Sensor>` 在 `:190-208`，`On<Remove, Sensor>` 在 `:211-226`。

**注意一处注释中的已知局限**：`backend.rs:167-169` 指出「同一 entity 上若有多种不同类型的 collider，
移除其一也会移除 `ColliderMarker`」——被作者判定为极窄的边界情况。

---

## 3. 形状表示、后端抽象与构造器

### 3.1 设计模式：backend-agnostic collider + 可替换后端

这是本文最值得学习的一点。**Avian 的 `Collider` 不是 enum，而是一个具体的 Parry 实现**；
「可替换后端」是通过 **trait + 泛型插件** 实现的，而不是通过运行时枚举分发。

抽象层次（`src/collision/collider/mod.rs`）：

| trait | 行号 | 约束 | 职责 |
|---|---|---|---|
| `IntoCollider<C>` | `:51-54` | — | 从任意类型构造碰撞体（`fn collider(&self) -> C`）。`Collider` 侧有 blanket impl：`parry/mod.rs:22-26`。 |
| `AnyCollider` | `:128-259` | `Component<Mutability = Mutable> + ComputeMassProperties` | 核心抽象。要求 `type Context: ReadOnlySystemParam`、`aabb_with_context`、`swept_aabb_with_context`（有默认实现 `:230-240`）、`contact_manifolds_with_context`。 |
| `SimpleCollider` | `:263-320` | `AnyCollider<Context = ()>` | 给不需要 world 上下文的后端提供免传 context 的包装方法（`aabb`/`swept_aabb`/`contact_manifolds`）。blanket impl 在 `:322`。 |
| `ScalableCollider` | `:325-342` | `AnyCollider` | `scale()` / `set_scale(scale, detail)` / `scale_by()`。 |

`Context` 的设计意图在文档示例里写得很清楚（`:136-203`）：可以让碰撞体在计算 AABB 或接触流形时
读取任意**只读** `SystemParam`（例如查询自定义组件、取 `Time`）。
`AabbContext` / `ContactManifoldContext` 是承载它的包装类型（`:58-88`、`:92-124`），
对 `Context = ()` 提供 `fake()` 构造（`:81-88`、`:116-124`）。

后端插件：

```rust
// src/collision/collider/backend.rs:68-71
pub struct ColliderBackendPlugin<C: ScalableCollider> {
    schedule: Interned<dyn ScheduleLabel>,
    _phantom: PhantomData<C>,
}
```

默认 schedule 是 `FixedPostUpdate`（`:85-92`）。文档 `:33-67` 给出完整替换后端的方式：

```rust
ColliderBackendPlugin::<MyCollider>::default(),
NarrowPhasePlugin::<MyCollider>::default(),   // 还需要窄相位插件
```

**明确限制**：`backend.rs:67` 的文档注明 **"Spatial queries are not supported for custom colliders yet."**
（自定义后端尚不支持空间查询）。这是读者做架构决策时必须知道的一条硬约束。

> **推论**：这条限制的原因很可能是 `SpatialQuery` 直接持有 `Query<&ColliderAabb>` 并假定
> 唯一后端（`src/spatial_query/system_param.rs:62`），而不是像 broad phase 那样按 trait 泛型化。

`Collider` 内部 3 字段（`shape` / `scaled_shape` / `scale`）正是这个模式的体现：
**后端无关的组件集合（AABB、层、密度、质量属性、树句柄）由插件统一装配，
后端只负责「形状」这一件事**，通过 `AnyCollider` 暴露 `aabb_with_context` 与 `contact_manifolds_with_context`。

### 3.2 `ColliderShape` 枚举？

**在 0.7.0 中不存在 `ColliderShape` 枚举。** 全仓搜索无该类型。
形状由 `SharedShape`（Parry 的共享形状指针）直接表示，`Collider` 的 `shape` 字段即 `SharedShape`
（`src/collision/collider/parry/mod.rs:368`）。

2D 专属的形状扩展类型只有两个，定义在 `src/collision/collider/parry/primitives2d.rs`：
`EllipseColliderShape` 与 `RegularPolygonColliderShape`，导出在 `src/collision/collider/parry/mod.rs:10-11`。

> **推论**：读者若想保留 Rapier 形状而自建 `ColliderShape` 枚举，需要自行设计 dispatch；
> Avian 的取舍是「直接复用 Parry 的 `SharedShape`（内部是 `enum` + `Arc`）」，代价是
> `Collider` 无法 `Reflect`——这一点在 `parry/mod.rs:355` 有明确注释：
> *"`Collider` is currently not `Reflect`. If you need to reflect it, you can use `ColliderConstructor` as a workaround."*

### 3.3 构造器完整清单（含行号与维度门控）

全部位于 `impl Collider`（`src/collision/collider/parry/mod.rs:534` 起）。

**组合与基础形状**

| 构造器 | 行号 | 维度门控 | 备注 |
|---|---|---|---|
| `compound(shapes: Vec<(impl Into<Position>, impl Into<Rotation>, impl Into<Collider>)>)` | `:698-715` | 通用 | 内部调用 `SharedShape::compound`；文档建议动态刚体优先用 compound 而非 trimesh/polyline（`:693-694`）。 |
| `circle(radius)` | `:719` | 2D | |
| `sphere(radius)` | `:725` | 3D | |
| `ellipse(half_width, half_height)` | `:731` | 2D | |
| `rectangle(x_length, y_length)` | `:741` | 2D | **参数是全长**，内部 `* 0.5`（`:742`） |
| `cuboid(x_length, y_length, z_length)` | `:747` | 3D | **参数是全长**，内部 `* 0.5`（`:748`） |
| `round_rectangle(x, y, border_radius)` | `:753` | 2D | |
| `round_cuboid(x, y, z, border_radius)` | `:759` | 3D | |
| `cylinder(radius, height)` | `:777` | 3D | 半径在 XZ 平面，高沿 Y（`:774-775`） |
| `cone(radius, height)` | `:784` | 3D | |
| `capsule(radius, length)` | `:790` | 通用 | `length` **不含两个半球**（`:789`） |
| `capsule_endpoints(radius, a, b)` | `:800` | 通用 | |
| `half_space(outward_normal)` | `:806` | 通用 | 无限大半空间 |
| `segment(a, b)` | `:811` | 通用 | |
| `triangle(a, b, c)` | `:823` / `:847` | 2D / 3D | |
| `triangle_unchecked(a, b, c)` | `:841` | 2D | |
| `regular_polygon(circumradius, sides)` | `:853` | 2D | |

**网格类形状**

| 构造器 | 行号 | 门控 |
|---|---|---|
| `polyline(vertices, indices: Option<Vec<[u32;2]>>)` | `:858` | 通用 |
| `trimesh(vertices, indices: Vec<[u32;3]>)` | `:874` | 通用（panic 版） |
| `try_trimesh(...)` | `:891` | 通用（Result 版） |
| `trimesh_with_config(..., flags: TrimeshFlags)` | `:911` | 通用 |
| `try_trimesh_with_config(...)` | `:933` | 通用 |
| `convex_decomposition(vertices, indices)` | `:945` (2D `[u32;2]`) / `:952` (3D `[u32;3]`) | 通用 |
| `convex_decomposition_with_config(..., params: VhacdParameters)` | `:960` / `:973` | 通用 |
| `convex_hull(points) -> Option<Self>` | `:985` (2D) / `:992` (3D) | 通用（返回 Option） |
| `convex_polyline(points) -> Option<Self>` | `:1000` | 2D |
| `heightfield(heights, scale)` | `:1114` (2D `Vec<Scalar>`) / `:1128` (3D `Vec<Vec<Scalar>>`) | 通用 |

**体素类形状**

| 构造器 | 行号 | 门控 |
|---|---|---|
| `voxels(voxel_size, grid_coordinates: &[IVector])` | `:1007` | 通用 |
| `voxels_from_points(voxel_size, points)` | `:1025` | 通用 |
| `voxelized_polyline(...)` | `:1031` | 2D |
| `voxelized_trimesh(...)` | `:1042` | 3D |
| `voxelized_trimesh_from_mesh(...)` | `:1055` | `collider-from-mesh` |
| `voxelized_convex_decomposition(...)` | `:1073` | 通用 |
| `voxelized_convex_decomposition_with_config(...)` | `:1092` | 通用 |

**从 Bevy `Mesh` 生成（均需 `collider-from-mesh`）**

| 构造器 | 行号 |
|---|---|
| `trimesh_from_mesh(mesh: &Mesh) -> Option<Self>` | `:1174` |
| `trimesh_from_mesh_with_config(mesh, flags)` | `:1218` |
| `convex_hull_from_mesh(mesh) -> Option<Self>` | `:1243` |
| `convex_decomposition_from_mesh(mesh)` | `:1265` |
| `convex_decomposition_from_mesh_with_config(mesh, params)` | `:1293` |
| `try_from_constructor(constructor, mesh: Option<&Mesh>) -> Option<Self>` | `:1321` |

**注意**：**没有 `Collider::from_mesh`**（全仓 grep `fn from_mesh` 无命中）。
读者若在旧文档里看到这个名字，那是旧 API。

### 3.4 缩放机制：`ScaledCollider` / `scale_shape`

缩放不是「在查询时乘一个矩阵」，而是**预先生成一个新的缩放后形状**：

```rust
// src/collision/collider/parry/mod.rs:1528-1532
fn scale_shape(
    shape: &SharedShape,
    scale: Vector,
    num_subdivisions: u32,
) -> Result<SharedShape, UnsupportedShape>
```

它对 `TypedShape` 逐分支处理（`:1534` 起），例如 cuboid/round_cuboid 可精确缩放（`:1535-1539`），
capsule 需要细分（`:1540`）。无法精确表达时用 `num_subdivisions` 近似凸包，
细分次数在调用点被硬编码为 `10`，并有 TODO 承认不该硬编码
（`src/collision/collider/parry/mod.rs:554-555`、`src/collision/collider/backend.rs:480-481,489-490`）。

缩放的两个驱动源：
1. **根实体**：`Transform::scale` → collider scale，由 `update_collider_scale` 处理（`backend.rs:474-485`），
   受 `PhysicsTransformConfig::transform_to_collider_scale` 开关控制（默认 `true`，`src/physics_transform/mod.rs:153,162`）。
   该 system 调度位置：`backend.rs:228-238`，设在 `PhysicsSystems::Prepare` 且
   **在 `PhysicsTransformSystems::TransformToPosition` 之后**（`:232-233`）。
2. **子碰撞体**：走 `ColliderTransform::scale`（`backend.rs:487-493`）。

`ColliderTransform` 自身由 `ColliderTransformPlugin` 维护
（`src/collision/collider/collider_transform/plugin.rs:41-63`）：
`propagate_collider_transforms` 在 `PhysicsTransformSystems::Propagate`（`:50-53`），
`update_child_collider_position` 在 `PhysicsStepSystems::First`（`:61-62`）。
传播算法是 `bevy_transform::propagate_transforms` 的克隆（`:106`、`:171`），
并用 `AncestorMarker<ColliderMarker>` 剪枝，跳过不含碰撞体的子树（`:43-46`、`:100`）。

`update_child_collider_position`（`:67-96`）的公式值得抄：
`position.0 = rb_pos.0 + rb_rot * collider_transform.translation`（`:84`），
3D 旋转为 `(rb_rot.0 * collider_transform.rotation.0).normalize()`（`:91-93`）。

### 3.5 `ColliderConstructor` 与 `ColliderConstructorHierarchy`

这两个类型的存在理由是：**`Collider` 不是 `Reflect` 的**（`parry/mod.rs:355` 明写），
所以需要一个可序列化/可反射的「形状配方」在运行时生成实际 `Collider`。

| 类型 | 定义 | 作用 |
|---|---|---|
| `ColliderConstructor` | `src/collision/collider/constructor.rs:316` | 形状配方的 enum（`:316` 起，`#[non_exhaustive]`，`:314`）。每个 variant 对应一个 `Collider::*` 构造器；从 Bevy 原始形状（`Circle`/`Sphere`/`Cuboid`…）到 `Collider` 的映射也在这里。 |
| `ColliderConstructorHierarchy` | `src/collision/collider/constructor.rs:118-136` | 给 entity 的**所有后代**按名字配置生成碰撞体。字段：`default_constructor`（`:121`）、`default_layers`（`:125`）、`default_density`（`:129`）、`config: HashMap<String, Option<ColliderConstructorHierarchyConfig>>`（`:135`）。 |
| `ColliderConstructorHierarchyConfig` | `src/collision/collider/constructor.rs:250-263` | 单个名字的配置：`constructor` / `layers` / `density` 三个 `Option`。 |
| `ColliderConstructorReady` | `src/collision/collider/constructor.rs:144` | `EntityEvent`：单个构造成功时触发。 |
| `ColliderConstructorHierarchyReady` | `src/collision/collider/constructor.rs:156` | 批量构造完成时触发（文档注明「不代表真的有 collider 生成」，`:151-153`）。 |

**运行位置**：`src/collision/collider/backend.rs:240-247`

```rust
#[cfg(feature = "default-collider")]
app.add_systems(Update, (init_collider_constructors, init_collider_constructor_hierarchies));
```

注意是 **`Update` schedule**，不是物理 schedule——因为它们要等资产（mesh）加载。

- `init_collider_constructors`：`backend.rs:267-320`。
  若 entity 已有 `Collider` 则警告并跳过（`:281-288`）；需要 mesh 但 mesh 未加载时 `continue`
  等下一帧（`:294-297`）；**需要 mesh 却没有 `Mesh3d` 时 panic**（`:291-293`）。
- `init_collider_constructor_hierarchies`：`backend.rs:326-450`。
  遍历 `children.iter_descendants`（`:356`）；按 `Name` 查 `config`（`:370-385`）；
  scene 未 ready 时等待（`:341-354`）；最后插入 collider + layers + density（`:424-433`）
  并移除自身组件（`:442-444`）。

**feature 门控**（`src/collision/collider/mod.rs:42-48`）：
`ColliderConstructor*` 全部在 `#[cfg(feature = "default-collider")]` 下。
`collider-from-mesh` 进一步门控「从 mesh 生成」的能力（`crates/avian3d/Cargo.toml:49`：
`collider-from-mesh = ["bevy/bevy_mesh", "bevy/bevy_mikktspace", "3d"]` —— **该 feature 隐含 3d**）。
`bevy_scene` 门控 scene 层级生成（`Cargo.toml:50`，代码见 `backend.rs:14-17`、`:331-354`）。

`ColliderConstructor::requires_mesh()`（`constructor.rs:495-505`）列出需要 mesh 的 variant：
`TrimeshFromMesh`、`TrimeshFromMeshWithConfig`、`ConvexDecompositionFromMesh`、
`ConvexDecompositionFromMeshWithConfig`、`ConvexHullFromMesh`、`VoxelizedTrimeshFromMesh`。

还有一层 mesh→Collider 的缓存：`ColliderCachePlugin`，位于 `src/collision/collider/cache.rs`，
门控在 `src/collision/collider/mod.rs:18-21`，在 `backend.rs:271,298-301,330,411-414` 使用。

### 3.6 `trimesh_builder.rs`

门控：`src/collision/collider/mod.rs:24-25` ——
`#[cfg(all(feature = "3d", any(feature = "parry-f32", feature = "parry-f64")))]`，
即**仅 3D 且启用 Parry 时存在**。

| 类型/方法 | 行号 |
|---|---|
| `TrimeshBuilder` 结构体（含 `shape`/`position`/`rotation`/`fail_on_compound_error`/`fallback_subdivisions`/`sphere_subdivisions`/`capsule_subdivision`/`cylinder_subdivisions`/`cone_subdivisions`） | `src/collision/collider/trimesh_builder.rs:52-79` |
| `Trimesh` | `:81` |
| `TrimeshBuilderError` | `:105` |
| `TrimeshBuilder::new` | `:113` |
| `translated` / `rotated` | `:129` / `:135` |
| `fallback_subdivisions` | `:142` |
| `sphere_subdivisions` / `capsule_subdivisions` / `cylinder_subdivisions` / `cone_subdivisions` | `:151` / `:171` / `:193` / `:203` |
| `fail_on_compound_error` | `:214` |
| `build() -> Result<Trimesh, TrimeshBuilderError>` | `:228` |
| `Collider::trimesh_builder()` | `:334` |

它的用途是**反向**的：把 `Collider` 转成三角形网格（用于调试渲染/导出），
内部按 `TypedShape` 分支调用 Parry 的 `to_trimesh()`（`:232-236`）。
`fallback_subdivisions` 默认 16（`:63-64` 注释），注意字段是 `NonZeroU32`。

### 3.7 层级碰撞体（`collider_hierarchy/`）

目录内容：`mod.rs`（231 行）+ `plugin.rs`（137 行）。

- `mod.rs` 定义 `ColliderOf` / `RigidBodyColliders` 并手写 `Relationship`（见 §2.4）。
- `plugin.rs` 定义 `ColliderHierarchyPlugin`（`:10`），只做 `ColliderOf` 的自动装配与维护，
  **不涉及几何**。

设计要点（源码事实）：`ColliderOf::on_insert` 会**先把 `ColliderTransform` 从
`GlobalTransform` 差值初始化**（`mod.rs:104-108`），再建立 relationship 反向索引（`:126-139`）。
这意味着用户可以在 spawn 时用 `Transform` 摆放子碰撞体，位姿会被正确换算到刚体局部空间。

> **推论**：Avian 因此有「刚性体自身的 collider」与「子 entity 上的 collider」两条路径，
> 后者更灵活（每个 collider 可独立设置 friction/layers/events），
> `parry/mod.rs:283-327` 的文档专门讨论了这两种「多个碰撞体」的写法。

---

## 4. 碰撞层与过滤

### 4.1 三个类型

```rust
// src/collision/collider/layers.rs:9-14
pub trait PhysicsLayer: Sized + Default {
    fn to_bits(&self) -> u32;
    fn all_bits() -> u32;
}
```

- `PhysicsLayer`（`layers.rs:9`）：可用 `#[derive(PhysicsLayer)]` 派生的 enum trait，
  `&L` 也有 blanket impl（`:16-27`）。第 0 bit 保留给默认层（`:32`、`:47`）。
- `LayerMask(pub u32)`（`layers.rs:86`）：位掩码。常量 `ALL = 0xffff_ffff`（`:117`）、
  `NONE = 0`（`:119`）、`DEFAULT = 1`（`:121`）。`has_all` 实为「有交集」而非「全包含」
  （`:180-183`，注意文档自承 `has_all` 与 `&` 等价，`:173-177`）。
- `CollisionLayers { memberships, filters }`（`layers.rs:362-369`）：
  **`#[component(immutable)]`（`:358`）——这意味着不能 `&mut CollisionLayers` 就地修改，
  必须重新 insert**（这正是 §5.3 Case 9 观察者监听 `On<Insert, CollisionLayers>` 的原因）。

### 4.2 过滤判定规则

文档规则（`layers.rs:248-251`）：A 与 B 可交互 **当且仅当**
- A 的 memberships 与 B 的 filters 有交集，**且**
- B 的 memberships 与 A 的 filters 有交集。

实现：

```rust
// src/collision/collider/layers.rs:424-427
pub fn interacts_with(self, other: Self) -> bool {
    (self.memberships & other.filters) != LayerMask::NONE
        && (other.memberships & self.filters) != LayerMask::NONE
}
```

默认「属于第 0 层、可与任何层交互」（`layers.rs:373-376`、`:430-434`）。

**`Group` / `InteractionGroups` 已删除**：全仓 grep 这两个标识符无任何命中。
0.7.0 只有 `CollisionLayers`。

### 4.3 掩码检查发生在哪里（管线位置）

**唯一一处碰撞层检查在 broad phase 的 BVH 遍历内**：

```rust
// src/collision/broad_phase/bvh_broad_phase.rs:256-259
// Check if the layers interact.
if !proxy1.layers.interacts_with(proxy2.layers) {
    continue;
}
```

其中 `proxy1.layers` / `proxy2.layers` 是 **mirror 到 `ColliderTreeProxy` 里的副本**
（`src/collider_tree/tree.rs:45-46`），不是每次去查 entity 的组件。
副本的同步由观察者完成：`On<Insert, CollisionLayers>` → 更新 `proxy.layers`
（`src/collider_tree/update.rs:322-343`）。

同一函数内还包含其余 5 类过滤，按执行顺序：

| 检查 | 行号 |
|---|---|
| 跳过自身 | `bvh_broad_phase.rs:234-237` |
| 避免移动 proxy 的重复对（含 sensor 特例） | `:241-254` |
| **碰撞层 `interacts_with`** | `:256-259` |
| 同一刚体的 collider 之间不碰撞（`proxy1.body == proxy2.body`） | `:261-264` |
| 已有 pair 去重（`PairKey` + `contact_graph.contains_key`） | `:269-273` |
| 被 joint 禁用碰撞 | `:275-283` |
| 用户自定义 `filter_pairs` hook（`CUSTOM_FILTER` flag） | `:285-295` |

而**静态-静态对直接不查询 static tree**（除非 proxy1 是 sensor）：

```rust
// src/collision/broad_phase/bvh_broad_phase.rs:124-142
// Skip static-static body collisions unless sensors or standalone colliders are involved.
if proxy_type1 != ColliderTreeType::Static || proxy1.is_sensor() {
    query_tree(&trees.static_tree, ...);
}
```

**推论**：这对 Kairos 的静态 box "plane" 场景很关键——若玩家只放静态碰撞体，
broad phase 根本不会生成任何对。必须有至少一个 dynamic/kinematic 物体，或把静态体设为 sensor。

### 4.4 `Sensor` 语义与管线差异

`Sensor` 是 unit struct（`src/collision/collider/mod.rs:429`），`#[doc(alias = "Trigger")]`（`:424`）。
**不是** `Collider` 的 required component，必须手动插入。

它在管线中的 5 处差异：

| 差异 | 位置 | 说明 |
|---|---|---|
| 不贡献质量 | `src/collision/collider/backend.rs:149-153` | `on_insert` 里若带 `Sensor` 则质量属性为 `ZERO` |
| 质量属性变化时通知刚体重算 | `src/collision/collider/backend.rs:190-226` | `On<Add, Sensor>` 给刚体插入 `RecomputeMassProperties`；`On<Remove, Sensor>` 重算 collider 质量 |
| proxy flag 置位 | `src/collider_tree/tree.rs:62`（`SENSOR = 1 << 0`）；写入在 `src/collider_tree/update.rs:283-300`，清除在 `:303-320` | 供 broad phase 查询 |
| 不生成约束 | `src/collision/broad_phase/bvh_broad_phase.rs:198-202` | `GENERATE_CONSTRAINTS = !BODY_DISABLED && !SENSOR` |
| 触碰中的接触从 `ConstraintGraph` 移除 | `src/collision/narrow_phase/mod.rs:620-645`（`on_add_sensor`）、`:649+`（`on_remove_sensor`）；注册在 `:158-161` | 收到 `Sensor` 时把已有接触从约束图与 island 中摘掉 |

另外 sensor 还影响两处 broad phase 逻辑：
- 静态 tree 查询条件放宽（`bvh_broad_phase.rs:125`）。
- 去重时「无论如何都处理该对」（`:246-253`），注释解释是为了避免「静态 sensor 撞静态体被漏掉」。

文档层面的语义：sensor 「发送碰撞事件与相交登记，但允许其他物体穿过」（`src/collision/collider/mod.rs:398-400`），
且「不贡献刚体质量」（`:402`）。

---

## 5. 空间索引：`collider_tree/`

### 5.1 依赖与总体结构

BVH 来自 **`obvhs` crate**：`crates/avian3d/Cargo.toml:94` → `obvhs = { version = "0.3" }`。

模块文档（`src/collider_tree/mod.rs:1-22`）明确了三件事：
1. 为所有碰撞体维护 `ColliderTree`，是 BVH（`:4-6`）。
2. **按刚体类型分成多棵树**，以便「高效查询碰撞体的特定子集」并「按刚体类型优化树更新」（`:8-9`）。
3. dynamic/kinematic 树「每个物理步重建」，static 树「在增删改时增量更新」（`:10-11`）。
   —— 注意：实际实现是 **refit + 重新插入/部分重建/全量重建的混合策略**，
   见 §5.5 的 `update_solver_body_aabbs` 与优化插件；模块文档是高层概括。

资源：

```rust
// src/collider_tree/mod.rs:120-130
#[derive(Resource, Default, Clone)]
pub struct ColliderTrees {
    pub dynamic_tree: ColliderTree,
    pub kinematic_tree: ColliderTree,
    pub static_tree: ColliderTree,
    pub standalone_tree: ColliderTree,   // 没有关联刚体的独立碰撞体
}
```

辅助方法：`tree_for_type`（`:135-142`）、`tree_for_type_mut`（`:146-153`）、
`iter_trees`（`:157-165`）、`get_proxy` / `get_proxy_mut`（`:181-193`）。

`ColliderTree`：

```rust
// src/collider_tree/tree.rs:23-36
#[derive(Clone, Default)]
pub struct ColliderTree {
    pub bvh: Bvh2,                              // :26  obvhs 的 BVH
    pub proxies: StableVec<ColliderTreeProxy>,  // :28  稳定索引的 proxy 数组
    pub moved_proxies: Vec<ProxyId>,            // :33  自上次更新以来移动过的 proxy
    pub workspace: ColliderTreeWorkspace,       // :35  复用的临时分配
}
```

`ColliderTreeProxy`（`tree.rs:40-49`）：`collider: Entity`（`:42`）、`body: Option<Entity>`（`:44`）、
`layers: CollisionLayers`（`:46`）、`flags: ColliderTreeProxyFlags`（`:48`）。
即**每个 proxy 自带过滤信息与语义 flag 的副本**，避免遍历时回查 ECS。

`ColliderTreeProxyFlags`（`tree.rs:57-71`，`u32` bitflags）：

| flag | 位 | 含义 |
|---|---|---|
| `SENSOR` | `1 << 0`（`:62`） | 是传感器 |
| `BODY_DISABLED` | `1 << 1`（`:64`） | 所属刚体被 `RigidBodyDisabled` |
| `CUSTOM_FILTER` | `1 << 2`（`:66`） | 启用了 `FILTER_PAIRS` 自定义过滤 hook |
| `MODIFY_CONTACTS` | `1 << 3`（`:68`） | 启用了 `MODIFY_CONTACTS` hook |
| `CONTACT_EVENTS` | `1 << 4`（`:70`） | 启用了接触事件 |

构造函数 `new(is_sensor, is_body_disabled, events_enabled, active_hooks)` 在 `tree.rs:77-100`。

`ColliderTreeWorkspace`（`tree.rs:129-138`）：`ploc_builder`（`:131`）、
`reinsertion_optimizer`（`:133`）、`insertion_stack`（`:135`，容量 2000，`:156`）、`temp_flags`（`:137`）。

### 5.2 关键类型：`ProxyId`、`ColliderTreeProxyKey`、`ColliderTreeType`

| 类型 | 位置 | 说明 |
|---|---|---|
| `ProxyId(u32)` | `src/collider_tree/proxy_key.rs:103` | proxy 的稳定索引，**只能用低 30 bit**（`:118` debug_assert）。`PLACEHOLDER` 在 `:107`。 |
| `ColliderTreeProxyKey(u32)` | `src/collider_tree/proxy_key.rs:15` | `ProxyId`（30 bit）+ `ColliderTreeType`（低 2 bit）打包（`:22-26`）。`id()` 在 `:30`，`tree_type()` 在 `:36`，`body()` 在 `:51`。`PLACEHOLDER = u32::MAX`（`:19`）。**这是 entity 上的组件**（`#[derive(Component)]`，`:14`）。 |
| `ColliderTreeType` | `src/collider_tree/proxy_key.rs:165-174` | `Dynamic = 0` / `Kinematic = 1` / `Static = 2` / `Standalone = 3`。`ALL` 数组在 `:178-183`。`from_body(Option<RigidBody>)` 在 `:189-196`。 |

「低 2 bit 存 tree type」这个编码技巧值得注意：它让 `ColliderTreeProxyKey` 保持 `u32`，
同时一次比较即可判定「是否同一个 proxy」（`tree.rs`/`bvh_broad_phase.rs` 大量使用 `proxy_key1 == proxy_key2`）。

### 5.3 碰撞体在树中的完整生命周期

`src/collider_tree/update.rs:109-121` 用注释列出了全部 11 种情况。这是本子系统最核心的一张表：

| # | 情况 | 注册位置 | 行为 |
|---|---|---|---|
| 1 | 插入 `C` 或 `ColliderOf`（且未禁用） | `update.rs:124` | `add_to_tree_on::<Insert, (C, ColliderOf), Without<ColliderDisabled>>` |
| 2 | 移除 `C` | `update.rs:131` | `remove_from_tree_on::<Remove, C, Allow<Disabled>>`（允许已禁用，处理「despawn 一个禁用碰撞体」的边界，`:127-130`） |
| 3 | 移除 `ColliderOf`（碰撞体仍在） | `update.rs:134-191` | 从旧树移除，**迁到 standalone tree**（`:183-189`） |
| 4 | 重新启用（移除 `Disabled`/`ColliderDisabled`） | `update.rs:195-198` | `add_to_tree_on::<Discard, Disabled, ...>` 与 `::<Discard, ColliderDisabled, ()>`。用 `Discard` 保证**先于** Case 2 运行（`:194`） |
| 5 | 禁用（新增 `Disabled`/`ColliderDisabled`） | `update.rs:201-204` | `remove_from_tree_on::<Add, Disabled, ...>` / `::<Add, ColliderDisabled, ()>` |
| 6 | 替换 `RigidBody` | `update.rs:207-280` | 按新刚体类型重算 tree type，从旧树挪到新树（`:242-277`）。注意 `:244-247` 若类型不变则 `break` |
| 7 | 新增 `Sensor` | `update.rs:283-300` | 设置 `SENSOR` flag |
| 8 | 移除 `Sensor` | `update.rs:303-320` | 清除 `SENSOR` flag |
| 9 | 插入 `CollisionLayers` | `update.rs:323-343` | 覆盖 `proxy.layers`（因为 `CollisionLayers` 是 immutable 组件，改动即重新 insert） |
| 10 | 插入 `ActiveCollisionHooks` | `update.rs:346-370` | 设置/清除 `CUSTOM_FILTER` flag |
| 11 | `RigidBodyDisabled` 变化 | `update.rs:373-399` | 设置/清除 `BODY_DISABLED` flag |

**插入实现**（`add_to_tree_on`，`update.rs:404-473`）：

1. 查询 `(Option<&ColliderOf>, &EnlargedAabb, &mut ColliderTreeProxyKey, Option<&CollisionLayers>, Has<Sensor>, Has<CollisionEventsEnabled>, Option<&ActiveCollisionHooks>)`（`:407-418`）。
2. 决定 tree type：有 `ColliderOf` 且刚体存在 → `ColliderTreeType::from_body(Some(*rb))`；否则 `Standalone`（`:437-442`）。
3. 构造 proxy（`:444-454`），`layers` 缺失时用 `CollisionLayers::default()`，`body` 为 `collider_of.map(|c| c.body)`。
4. **若旧 key 不是 `PLACEHOLDER`，先从旧树移除**（`:456-462`）。
5. `tree.add_proxy(Aabb::from(enlarged_aabb.get()), proxy)`（`:465-466`）——
   **注意放进 BVH 的是 `EnlargedAabb`，不是 `ColliderAabb`**。
6. 写回 `ColliderTreeProxyKey::new(proxy_id, tree_type)`（`:469`）。
7. **标记为 moved**（`:472`），使 broad phase 本步会查询它。

**移除实现**（`remove_from_tree_on`，`update.rs:476-499`）：
`PLACEHOLDER` 直接返回（`:488-490`），否则 `tree.remove_proxy`、从 `MovedProxies` 移除、
并把 key 重置为 `PLACEHOLDER`（`:492-498`）。

**`ColliderTree::add_proxy` / `remove_proxy`**（`src/collider_tree/tree.rs`）：

```rust
// tree.rs:165-176
pub fn add_proxy(&mut self, aabb: Aabb, proxy: ColliderTreeProxy) -> ProxyId {
    let id = self.proxies.push(proxy) as u32;
    self.bvh.insert_primitive(aabb, id, &mut self.workspace.insertion_stack);
    self.moved_proxies.push(ProxyId::new(id));
    ProxyId::new(id)
}
```

```rust
// tree.rs:182-199
pub fn remove_proxy(&mut self, proxy_id: ProxyId) -> Option<ColliderTreeProxy> {
    if let Some(proxy) = self.proxies.try_remove(proxy_id.index()) {
        self.bvh.remove_primitive(proxy_id.id());
        // ... 从 moved_proxies 中线性查找并 swap_remove
        Some(proxy)
    } else { None }
}
```

`proxies` 是 `StableVec`（`tree.rs:28`），这是 `ProxyId` 能保持稳定的关键。
`remove_proxy` 里的 `moved_proxies` 清理是 O(n) 线性查找（`tree.rs:188-193`）。

**`MovedProxies` 资源**（`update.rs:512-567`）：`Vec` + `HashSet` 双结构，
`Vec` 保插入顺序（供 broad phase 并行分块，`:524-527`），`HashSet` 供 O(1) `contains`（`:531-533`）。
`remove` 用 `swap_remove`，会打乱顺序（`:548-551` 文档明说）。
每个物理步结束后清空：`clear_moved_proxies`（`:1021-1024`）。

### 5.4 `ColliderTreePlugin` 与 schedule 接线

```rust
// src/collider_tree/mod.rs:57-100（节选）
impl<C: AnyCollider> Plugin for ColliderTreePlugin<C> {
    fn build(&self, app: &mut App) {
        let _ = app.try_register_required_components_with::<C, ColliderTreeProxyKey>(|| { ... });  // :60-63
        app.add_plugins(ColliderTreeUpdatePlugin::<C>::default());                                 // :66
        if !app.is_plugin_added::<ColliderTreeOptimizationPlugin>() {                              // :69
            app.add_plugins(ColliderTreeOptimizationPlugin);
        }
        app.init_resource::<ColliderTrees>().init_resource::<MovedProxies>();                      // :74-75

        app.configure_sets(PhysicsSchedule,
            ColliderTreeSystems::UpdateAabbs
                .in_set(PhysicsStepSystems::BroadPhase)          // :81
                .after(BroadPhaseSystems::First)                 // :82
                .before(BroadPhaseSystems::CollectCollisions),   // :83
        );
        app.configure_sets(PhysicsSchedule,
            ColliderTreeSystems::BeginOptimize.in_set(BroadPhaseSystems::Last));  // :87
        app.configure_sets(PhysicsSchedule,
            ColliderTreeSystems::EndOptimize.in_set(SolverSystems::Finalize));    // :92
    }
    fn finish(&self, app: &mut App) {
        app.register_physics_diagnostics::<ColliderTreeDiagnostics>();             // :98
    }
}
```

**这个 order 极其重要**：AABB 更新被放在 broad phase **内部**，
在 `BroadPhaseSystems::First` 之后、`CollectCollisions` 之前——
即「先更新 AABB → 再收集碰撞对」。

`ColliderTreeSystems` 三个集合定义在 `src/collider_tree/mod.rs:104-115`：
`UpdateAabbs`（`:106`）、`BeginOptimize`（`:110`，以 async task 与模拟步并发，`:109`）、
`EndOptimize`（`:113`，在模拟步末尾完成优化，`:114`）。
`mod.rs:91` 有 TODO 承认 `EndOptimize` 的位置「需要在空间查询与碰撞事件之前」，
说明作者自己也不完全满意当前接线。

`ColliderTreeUpdatePlugin::build` 的 system 注册（`src/collider_tree/update.rs:47-400`）：

| system / observer | 行号 | 集合 |
|---|---|---|
| `update_moved_collider_aabbs::<C>` | `:56-63` | `ColliderTreeSystems::UpdateAabbs`，且 `.ambiguous_with_all()`（`:62`，注释解释是为了允许同时存在多个碰撞后端，`:60-61`） |
| `(clear_moved_proxies, update_solver_body_aabbs::<C>).chain()` | `:66-72` | `after(PhysicsStepSystems::Finalize).before(PhysicsStepSystems::Last)` |
| `On<Add, C>` AABB 初始化 observer | `:75-107` | — |
| 11 个生命周期 observer | `:124-399` | — |

资源初始化在 `:50-52`：`MovedProxies`、`EnlargedProxies`、`LastDynamicKinematicAabbUpdate`。

### 5.5 AABB 更新：三条路径

**路径 1 — 插入时初始化**（observer，`update.rs:75-107`）：

```rust
// update.rs:88-104
let contact_tolerance = length_unit.0 * narrow_phase_config.contact_tolerance;
let margin = length_unit.0 * AABB_MARGIN;
...
let growth = Vector::splat(contact_tolerance + collision_margin);
*aabb = collider.aabb_with_context(pos.0, *rot, context).grow(growth);
enlarged_aabb.update(&aabb, margin);
```

其中 `AABB_MARGIN = 0.05`（`update.rs:34`），注释说明它按 `PhysicsLengthUnit` 隐式缩放（`:32`），
且 `update.rs:33` 有 TODO 说「这应该可配置」。
`collision_margin` 来自可选的 `CollisionMargin`（`:94`，缺失算 0）。

**路径 2 — 手动移动后更新**（`update_moved_collider_aabbs`，`update.rs:839-981`）：

这是**每个物理步**都跑的 system。它先做变更检测快速跳过：

```rust
// update.rs:892-898
if !pos.last_changed().is_newer_than(last_tick.0, this_run)
    && !rot.last_changed().is_newer_than(last_tick.0, this_run)
    && !collider.last_changed().is_newer_than(last_tick.0, this_run)
{
    return;
}
```

`last_tick` 是 `LastPhysicsTick` 资源（`:863`）。之后对**全部 4 棵树**执行（`:937`，`ColliderTreeType::ALL`）：

- 用**位向量**（`EnlargedProxiesBitVec`，每棵树一个）标记 moved proxy（`:912-931`；
  并行 `par_iter_mut` 内用 thread-local 位向量，最后 `combine_thread_local`，`:942`）。
- 计算 moved 比例（`:944-949`），**阈值 0.1**：小于 0.1 走 `resize_proxy_aabb`（只从该叶向上 refit），
  否则 `set_proxy_aabb` + 一次 `refit_all()`（`:951-977`）。`:953` 有 TODO 承认阈值需要调。

**路径 3 — 速度扩张的 AABB**（`update_solver_body_aabbs`，`update.rs:653-836`）：

只对带 `SolverBody` 的 entity（醒着的 dynamic/kinematic 刚体）跑（`:663`，`:648-650` 注释解释
为什么要用 `SolverBody` 而不是专门 marker）。
它对每个 collider 计算**扫掠 AABB**（speculative margin 扩张）：

```rust
// update.rs:751-777（节选）
let offset = pos.0 - rb_pos.0 - center_of_mass.0;
#[cfg(feature = "3d")]
let vel = lin_vel.0 + ang_vel.cross(offset);
let movement = (vel * delta_secs).clamp_length_max(speculative_margin.max(contact_tolerance));
// ... 计算 end_pos / end_rot
*aabb = collider.swept_aabb_with_context(pos.0, *rot, end_pos, end_rot, context).grow(growth);
```

之后只对 dynamic/kinematic 两棵树做 `set_proxy_aabb` + **无条件 `refit_all()`**（`:807-829`，
注释 `:825-827` 承认「moved proxy 少时只向上 refit 更快」）。
`:651-652` 的 TODO 明确说「这种速度扩张 AABB 的做法相当低效，
可以改成 Box2D 风格的带 fast body 的 CCD」。若 collider 有 `SweptCcd`，
speculative margin 直接取 `Scalar::MAX`（`:731-735`）。

**结论**：`ColliderAabb` 是**紧密框**（tight AABB），`EnlargedAabb` 是「加上 margin 的框」，
**BVH 里存的是 `EnlargedAabb`**。这个设计让小幅移动不触发树更新
（`src/collision/collider/mod.rs:568-574` 的文档就是这么解释的）。

### 5.6 树与 broad phase 的关系

**它们是两层，不是二选一。** 源码事实：

- `ColliderTrees` 属于 `collider_tree` 模块，定位是「加速结构」，文档明确说它
  「是低层结构，不建议直接使用」（`src/collider_tree/mod.rs:17-19`），
  并建议用 `SpatialQuery` 或 `BvhBroadPhasePlugin`（`:19-22`）。
- `BvhBroadPhasePlugin` 是**算法**，它把 `ColliderTrees` 作为输入：

```rust
// src/collision/broad_phase/bvh_broad_phase.rs:51-58
fn collect_collision_pairs<H: CollisionHooks>(
    trees: ResMut<ColliderTrees>,
    moved_proxies: Res<MovedProxies>,
    ...
)
```

- 它注册在 `BroadPhaseSystems::CollectCollisions`（`bvh_broad_phase.rs:44-48`）。
- 核心遍历：对每个 moved proxy，依次查询 dynamic / kinematic /（条件性）static / standalone 树
  （`:91-159`），每次调用 `query_tree`（`:210-302`），后者用
  `tree.bvh.aabb_traverse(proxy_aabb1, |bvh, node_index| { ... })`（`:225`）。
- 产出 `ContactEdge` 并塞进 `ContactGraph`（`:173-204`）。

0.7.0 的默认 broad phase **就是 BVH broad phase**：`src/lib.rs:783`
`.add(BvhBroadPhasePlugin::<()>::default())`。
`BroadPhaseCorePlugin` 只负责资源/集合/诊断，不含算法（`src/collision/broad_phase/mod.rs:164-197`）。
模块文档 `:22-42` 明确提供了替换为自定义 broad phase（如 SAP）的方法，
并列出**自定义实现必须自己处理的过滤项**：`CollisionLayers`、`CollisionHooks`、
`JointCollisionDisabled`、跳过父刚体、跳过非 dynamic vs 非 dynamic——这与 §4.3 的实际检查清单一致。

树的其他消费方：`SpatialQuery`（`src/collider_tree/mod.rs:19-22` 提及）与
`traverse.rs` 提供的各种遍历（`ray_traverse_closest` `:24`、`ray_traverse_all` `:59`、
`sweep_traverse_closest` `:85`、`sweep_traverse_all` `:127`、
`squared_distance_traverse_closest` `:164`、`point_traverse` `:200`、`aabb_traverse` `:230`）。

### 5.7 树的优化（`optimization.rs`）

`ColliderTreeOptimizationPlugin`（`src/collider_tree/optimization.rs:15`）注册两个 system（`:22-30`）：
`optimize_trees` 在 `ColliderTreeSystems::BeginOptimize`（`:25`），
`block_on_optimize_trees` 在 `EndOptimize`（`:27`，仅非 wasm，`:26`）。

配置资源 `ColliderTreeOptimization`（`:36`）三个字段：`optimization_mode`（`:40`）、
`optimize_in_place`（`:57`）、`use_async_tasks`（`:63`）。
其 `Default` 实现在 `:67-77`：`optimize_in_place: false`（`:70`）、
`use_async_tasks` 在非 wasm 平台为 `true`（`:73-74`）。
`optimize_in_place = false` 时会 clone 树的局部，使空间查询在优化期间仍可用旧树（文档 `:44-51`）。

`TreeOptimizationMode`（`:81`）四个变体：`Reinsert`（`:86`）、`PartialRebuild`（`:94`）、
`FullRebuild`（`:101`）、`Adaptive`（默认，`:112`）。
`Adaptive` 的两个阈值默认 `reinsert_threshold: 0.15`、`partial_rebuild_threshold: 0.45`（`:129-131`），
`resolve(moved_ratio)` 在 `:141-158` 做三档映射（`< reinsert_threshold` → Reinsert，
`< partial_rebuild_threshold` → PartialRebuild，否则 FullRebuild，`:147-153`）。

`ColliderTree` 上的对应方法（`tree.rs`）：`rebuild_full`（`:305-312`，用 PLOC）、
`rebuild_partial`（`:316-329`，先 `compute_rebuild_path_flags`）、
`optimize`（`:336-340`，SAH reinsertion）、`optimize_candidates`（`:346-352`）。

诊断：`ColliderTreeDiagnostics { optimize, update }`（`src/collider_tree/diagnostics.rs:12-18`），
计时路径 `avian/collider_tree/optimize` 与 `avian/collider_tree/update`（`:24-27`）。

### 5.8 `obvhs_ext.rs` 的作用

该文件（537 行）给 `obvhs` 补了它没有的遍历能力：

| 项 | 行号 |
|---|---|
| `Sweep`（`aabb` + `velocity` + `inv_velocity` + `tmin`/`tmax`） | `src/collider_tree/obvhs_ext.rs:16-27` |
| `SweepHit` | `:57` |
| `trait Bvh2Ext`（`sweep_traverse`、`sweep_traverse_miss`、`sweep_traverse_anyhit`、`sweep_traverse_dynamic`、`squared_distance_traverse`、`squared_distance_traverse_dynamic`） | `:75`，方法在 `:86` / `:102` / `:118` / `:134` / `:159` / `:181` |
| `trait ObvhsAabbExt`（`distance_to_point_squared`、`intersect_sweep`） | `:467`，方法在 `:469` / `:475` |
| `pub(crate) fn obvhs_ray` | `:526` |

导出：`src/collider_tree/mod.rs:33-34`（`Bvh2Ext` 公开，`obvhs_ray` 为 `pub(crate)`）。

---

## 6. `ColliderAabb` 计算与维护

### 6.1 类型与 API

```rust
// src/collision/collider/mod.rs:434-443
#[derive(Reflect, Clone, Copy, Component, Debug, PartialEq)]
pub struct ColliderAabb {
    pub min: Vector,
    pub max: Vector,
}
```

| 项 | 行号 |
|---|---|
| `Default` = `INVALID` | `src/collision/collider/mod.rs:445-449` |
| `INVALID`（`min = +INF`, `max = -INF`） | `:453-456` |
| `new(center, half_size)` | `:459-464` |
| `from_min_max(min, max)` | `:467-469` |
| `from_shape(&SharedShape)`（用 `compute_local_aabb`） | `:476-482`（门控 `default-collider` + parry） |
| `center()` | `:486-488` |
| `size()` | `:492-494` |
| `merged(other)` | `:498-503` |
| `grow(amount)` / `shrink(amount)` | `:507-514` / `:518-525` |
| `intersects(&other)` | `:530-534`（2D）/ `:539-544`（3D） |
| `contains(&other)` | `:548-550` |

3D 的 `intersects` 多一个 z 轴判断（`:542-543`）。`grow`/`shrink` 带 `debug_assert!`（`:512`、`:523`）。

### 6.2 与本项目相关的外部 AABB 类型

- **`obvhs::aabb::Aabb`**：树的原生类型。转换实现：

```rust
// src/collision/collider/mod.rs:553-566
impl From<ColliderAabb> for obvhs::aabb::Aabb {
    fn from(value: ColliderAabb) -> Self {
        Self {
            #[cfg(feature = "2d")]
            min: value.min.f32().extend(-0.5).to_array().into(),
            #[cfg(feature = "2d")]
            max: value.max.f32().extend(0.5).to_array().into(),
            #[cfg(feature = "3d")]
            min: value.min.f32().to_array().into(),
            #[cfg(feature = "3d")]
            max: value.max.f32().to_array().into(),
        }
    }
}
```

2D 时把 AABB 沿 z 轴撑成 `[-0.5, 0.5]`，使 2D 能用 3D BVH。这是复用 `obvhs`（只支持 3D）的标准技巧。

- **`bevy_math::bounding::Aabb3d`**：`ColliderAabb` 与它**没有** `From`/`Into` 实现。
  只有 debug render 在绘制时手工构造 `Aabb3d`：`src/debug_render/mod.rs:230-245`
  （`use bevy_math::bounding::Aabb3d;` 在 `:230`，构造在 `:243-245`）。同样出现在
  `src/debug_render/mod.rs:10`、`:534`、`src/debug_render/gizmos.rs:198`。

> **推论**：`ColliderAabb` 是 Avian 自己的类型（避免与 `bevy_math` 的 bounding volume 语义耦合），
> 只在需要展示给 Bevy 渲染/可视化时才转换。读者若想在 Kairos 里直接用 `bevy_math` 的
> `Aabb3d`，需要自行承担「2D 需要 z 膨胀」「与 obvhs 互转」这些细节。

### 6.3 脏标记与更新时机（汇总）

| 触发场景 | system/observer | 位置 |
|---|---|---|
| `Collider` 刚被添加 | `On<Add, C>` observer | `src/collider_tree/update.rs:75-107` |
| 手动移动（改 `Position`/`Rotation`/collider） | `update_moved_collider_aabbs` | `src/collider_tree/update.rs:839-981`，变更检测在 `:892-898` |
| 动态/运动学刚体每个物理步 | `update_solver_body_aabbs` | `src/collider_tree/update.rs:653-836` |

**Rust 类型层面的说明**：`ColliderAabb` 与 `EnlargedAabb` 都是 `Component`，
但**没有 `#[component(immutable)]`**，所以可以被 `&mut` 就地写。
`EnlargedAabb` 的 `Default` 是 derive 的（`src/collision/collider/mod.rs:575`），
即 `ColliderAabb::default()` = `INVALID`。

`EnlargedAabb` 的更新逻辑本身就是一个「脏检查」：

```rust
// src/collision/collider/mod.rs:592-602
pub fn update(&mut self, aabb: &ColliderAabb, margin: Scalar) -> bool {
    if self.contains(aabb) {
        return false;            // 还在旧框内 → 不动树
    }
    let margin = Vector::splat(margin);
    self.0.min = aabb.min - margin;
    self.0.max = aabb.max + margin;
    true                         // 返回 true → 标记 proxy 为 moved
}
```

返回值就是「是否需要更新树」的信号，被 §5.5 的三条路径直接使用
（`update.rs:104`、`:780`、`:910`）。

---

## 7. 与质量相关的 collider 数据（交接给刚体文档）

本节只写「碰撞体侧提供了什么」，够用即可，详细处理留给刚体文档。

**数据流**：

1. 形状 + 密度 → 单 collider 的 `MassProperties`。
   计算入口是 `ComputeMassProperties` trait，它是 **`bevy_heavy` 的 2D/3D 版本按维度重命名后的 re-export**：

```rust
// src/dynamics/rigid_body/mass_properties/mod.rs:216-221
#[cfg(feature = "2d")]
pub(crate) use bevy_heavy::{ComputeMassProperties2d as ComputeMassProperties, MassProperties2d as MassProperties};
#[cfg(feature = "3d")]
pub(crate) use bevy_heavy::{ComputeMassProperties3d as ComputeMassProperties, MassProperties3d as MassProperties};
```

   依赖声明在 `crates/avian3d/Cargo.toml:88` → `bevy_heavy = { version = "0.5" }`。

2. `Collider` 的实现（**注意：这里全部委托给 Parry**）：
   - 2D：`src/collision/collider/parry/mod.rs:446-480`（`mass` `:447`、`unit_angular_inertia` `:452`、
     `angular_inertia` `:456`、`center_of_mass` `:461`、`mass_properties` `:466`）
   - 3D：`src/collision/collider/parry/mod.rs:483-522`（`mass` `:484`、`unit_principal_angular_inertia` `:489`、
     `principal_angular_inertia` `:493`、`local_inertial_frame` `:498`、`center_of_mass` `:503`、
     `mass_properties` `:508`）
   - 实现体一律是 `self.shape_scaled().mass_properties(density)` 后拆字段
     （例如 `:467-478`）。**用 `shape_scaled()` 而非 `shape`**，即缩放会影响质量。
   - `:443-444` 有一条重要 TODO：**`bevy_heavy` 支持对 Bevy 原始形状高效地分别计算各项质量属性，
     但 Parry 的形状不支持，所以每个方法都得把全部质量属性算一遍**。读者若关心性能可注意这点。

3. 每 collider 缓存到 `ColliderMassProperties`。
   system：`update_collider_mass_properties`（`src/collision/collider/backend.rs:498-509`），
   查询过滤 `Or<(Changed<C>, Changed<ColliderDensity>)>, Without<Sensor>`（`:501`），
   即**改形状或改密度才重算，且 sensor 不算**。
   调度到 `MassPropertySystems::UpdateColliderMassProperties`（`backend.rs:234-235`）。
   该集合定义在 `src/dynamics/rigid_body/mass_properties/mod.rs:335`。

4. 刚体侧聚合：`MassPropertySystems` 另有 `QueueRecomputation`（`:337`）
   与 `UpdateComputedMassProperties`（`:340`），
   配合 `RecomputeMassProperties` 标记组件（`backend.rs:186`、`:204` 插入）。
   查询过滤类型别名 `WithComputedMassProperty` 在 `:344-348`，`MassPropertyChanged` 在 `:351-355`。

5. 相关联的组件：`Mass(pub f32)`（`src/dynamics/rigid_body/mass_properties/components/mod.rs:160`，
   `Mass::from_shape<T: ComputeMassProperties>` 在 `:177-179`）、
   `ComputedMass`（`src/dynamics/rigid_body/mass_properties/components/computed.rs:48`）、
   `MassPropertiesBundle`（`components/mod.rs:1038-1042`）。

**`ColliderDisabled` 的质量语义**：源码明确「被禁用的碰撞体**仍然**贡献刚体质量；
要阻止这一点就把 `Mass` 设为零」（`src/collision/collider/mod.rs:350-351`）。

**`Sensor` 的质量语义**：`ColliderMassProperties::ZERO`（`backend.rs:149-153`），
并有专门的回归测试 `sensor_mass_properties`（`backend.rs:518-618`），
断言加/去 `Sensor` 时刚体的 `ComputedMass` 与 `ComputedCenterOfMass` 会相应变化。

---

## 8. 里程碑：最小可运行 collider 子系统

以下是为 Kairos 定制的**有序施工清单**。每一节都给出「Avian 对应实现的位置」，
以便读者逐行对照。

### 8.0 先决定的两件事

**(a) 形状表示。** Avian 的选择是直接复用 Parry 的 `SharedShape`
（`src/collision/collider/parry/mod.rs:368`），代价是 `Collider` 不 `Reflect`（`:355` 明说），
补救手段是 `ColliderConstructor`（`constructor.rs:265-270`）。
Kairos 现已有 `rapier3d 0.33`，最省力的路线是**照抄这个模式**：
自己的 `Collider` 持有 rapier 的 `SharedShape` 等价物 + `scaled_shape` + `scale`，
并把「可反射的形状配方」拆成独立的 constructor 组件。

**(b) 是一棵树还是四棵树。** Avian 用 4 棵（`src/collider_tree/mod.rs:121-130`），
理由是「按刚体类型优化更新策略」与「高效查询子集」（`:8-9`）。
**推论**：Kairos 第一阶段可以先只做 **1 棵树（或 2 棵：static / 非 static）**，
把 `ColliderTreeType` 的枚举先留成占位，接口形状按 4 棵树设计。
`ColliderTreeProxyKey` 的低 2 bit 编码（`proxy_key.rs:22-26`）建议一开始就照抄，
因为后续从 2 棵树扩到 4 棵时不需要改组件布局。

### 8.1 第 1 步：组件集合（最小）

| 组件 | 抄哪里 | 最小职责 |
|---|---|---|
| `Collider` | `src/collision/collider/parry/mod.rs:366-376` | `shape` + `scaled_shape` + `scale` 三字段。先只实现 `sphere` / `cuboid` / `rectangle` 构造器（`:725`、`:747`、`:741`） |
| `ColliderMarker` | `src/collision/collider/backend.rs:256` | 「这是碰撞体」标记，供跨后端查询过滤 |
| `ColliderAabb` | `src/collision/collider/mod.rs:438-443` | `min`/`max`，含 `INVALID`（`:453-456`）、`grow`（`:507`）、`intersects`（`:539` 3D） |
| `EnlargedAabb` | `src/collision/collider/mod.rs:579` | 包装 `ColliderAabb` + `update(aabb, margin) -> bool`（`:592-602`） |
| `CollisionLayers` | `src/collision/collider/layers.rs:362-369` | `memberships` + `filters`；`interacts_with`（`:424-427`）；`DEFAULT`（`:373-376`） |
| `ColliderDensity` | `.../mass_properties/components/collider.rs:32` | 默认 `1.0`（`:34-38`） |
| `ColliderMassProperties` | `.../mass_properties/components/collider.rs:80` | 若暂时不做动力学，可先留空实现，但**建议保留组件**，否则后续加动力学要改 required 列表 |
| `Position` / `Rotation` | `src/physics_transform/transform.rs:48` / `:745` | 物理位姿。带占位值语义（`:54`、`:752`） |
| `ColliderTreeProxyKey` | `src/collider_tree/proxy_key.rs:15` | 树句柄，`PLACEHOLDER` = `u32::MAX`（`:19`） |

**用 Avian 的 required component 机制装配**（两处都要，语义不同）：

```
// 结构体侧（对应 parry/mod.rs:358-365）
#[require(ColliderMarker, ColliderAabb, EnlargedAabb, CollisionLayers, ColliderDensity, ColliderMassProperties)]

// 插件侧（对应 backend.rs:96-104 + collider_tree/mod.rs:59-63）
app.try_register_required_components::<Collider, ...>()
```

**第一阶段明确不要自动插入**：`Sensor`、`CollisionMargin`、`Friction`、`Restitution`、
`ColliderDisabled`、`ColliderOf`、`ColliderTransform`。理由见 §8.5。

### 8.2 第 2 步：AABB 计算（先于树）

镜像 `src/collision/collider/parry/mod.rs:404-417`：

```rust
fn aabb_with_context(&self, position: Vector, rotation: impl Into<Rotation>, _: ...) -> ColliderAabb {
    let aabb = self.shape_scaled().compute_aabb(&make_pose(position, rotation));
    ColliderAabb { min: aabb.mins, max: aabb.maxs }
}
```

要点：
- **一定用 `shape_scaled()`**，否则 `Transform::scale` 不生效。
- `grow` 的量是 `length_unit * (contact_tolerance + collision_margin)`（`update.rs:99`）。
  Avian 默认 `contact_tolerance = 0.005`（`src/collision/narrow_phase/mod.rs:235,253`）、
  `AABB_MARGIN = 0.05`（`src/collider_tree/update.rs:34`）。
  **第一阶段可以两个都取 0**，只要 tree margin 保持为 0 就不会出现「框已扩但树未更新」的不一致。

### 8.3 第 3 步：树

镜像 `src/collider_tree/tree.rs`：

1. `ColliderTree { bvh: Bvh2, proxies: StableVec<ColliderTreeProxy>, moved_proxies: Vec<ProxyId>, workspace: ... }`（`:23-36`）。
   直接依赖 `obvhs = "0.3"`（`crates/avian3d/Cargo.toml:94`）。
   **`proxies` 必须是稳定索引容器**，否则 `ProxyId` 会失效（`:28` 注释也强调）。
2. `ColliderTreeProxy { collider, body, layers, flags }`（`:40-49`）。
   **推论**：`layers` 这份副本是性能关键——它把层过滤从「每对查 ECS」变成「读两个结构体」。
3. `add_proxy`（`:165-176`）：push proxy → `bvh.insert_primitive` → 记入 `moved_proxies`。
   注意 Avian 把 `moved_proxies.push` 也放在这里，保证新插入的 proxy 立即参与本步 broad phase。
4. `remove_proxy`（`:182-199`）：`try_remove` → `bvh.remove_primitive` → 从 `moved_proxies` 清理。
5. AABB 更新三件套：`set_proxy_aabb`（`:267-273`）、
   `resize_proxy_aabb`（`:283-286`，改框并向上 refit）、`refit_all`（`:299-301`）。
6. **可以先不实现**：`reinsert_proxy`（`:290-295`）、`rebuild_full`（`:305-312`）、
   `rebuild_partial`（`:316-329`）、`optimize`（`:336-352`）。
   第一阶段只用 `resize_proxy_aabb` + `refit_all` 就能保证正确性
   （这正是 `update.rs:814-828` 在 dynamic/kinematic 树上的做法）。

**关于「树与 broad phase 是不是同一个东西」的结论**（重要）：
在 Avian 里**不是**。树是加速结构（`collider_tree` 模块），
broad phase 是消费它的算法（`BvhBroadPhasePlugin`，`broad_phase` 模块），
两者通过 `ColliderTrees` 资源解耦（`bvh_broad_phase.rs:52`）。
**推论**：这个分层对 Kairos 有直接价值——第一阶段只做「树 + 查询 API」，
不写 broad phase，也能用射线/AABB 查询验证树是否正确，不必等到能做碰撞对生成。

### 8.4 第 4 步：生命周期接线（核心难点）

这是本里程碑**最容易出错**的部分。Avian 用 11 个 observer 覆盖 `update.rs:109-121` 列出的场景。
第一阶段只需要其中 3 个 + 1 个 AABB 初始化：

| 需要的 | Avian 位置 | 触发的 ECS 事件 |
|---|---|---|
| 插入时算 AABB | `src/collider_tree/update.rs:75-107` | `On<Add, Collider>` |
| 插入树 | `src/collider_tree/update.rs:124`（`add_to_tree_on::<Insert, (C, ColliderOf), Without<ColliderDisabled>>`） | `On<Insert, (Collider, ColliderOf)>` |
| 移除树 | `src/collider_tree/update.rs:131`（`remove_from_tree_on::<Remove, C, Allow<Disabled>>`） | `On<Remove, Collider>` |
| 移动时更新 | `src/collider_tree/update.rs:839-981` | 每步 `par_iter_mut` + 变更检测 |

**关键陷阱 1 — 顺序**：Avian 用 `Discard` / `Allow<Disabled>` 等 Bevy 0.19 的事件时机控制
observer 相对顺序（`update.rs:194` 注释：「用 `Discard` 让它先于 Case 2 运行」）。
若 Kairos 的 Bevy 版本没有这套 API，**推论**：需要用别的方式保证「先移除再插入」
或「幂等化插入逻辑」，不能依赖 observer 注册顺序。

**关键陷阱 2 — `ColliderTreeProxyKey` 的写回时机**：`add_to_tree_on` 里
**先查 `EnlargedAabb`、再决定 tree type、再插入**（`update.rs:424-466`）。
这意味着 AABB 初始化 observer（`On<Add, Collider>`）**必须**早于插入 observer 生效，
否则 `EnlargedAabb` 还是 `INVALID`，插进树的是一个退化框。

**推论**：这正是 `On<Add, Collider>` 与 `On<Insert, (Collider, ColliderOf)>` 被分开的原因——
前者在组件刚加上时跑（算 AABB），后者在 relationship 建立后跑（此时才知道 tree type）。

**关键陷阱 3 — `Aabb::from(enlarged_aabb.get())`**：插入树用的是 **enlarged** AABB
（`update.rs:466`），更新时也用（`:1007`）。用错成 `ColliderAabb` 会导致
「enlarged 框变了但树里还是旧框」的不一致。

### 8.5 第 5 步：schedule 接线

镜像 `src/collider_tree/mod.rs:78-93`。最小版本：

```
PhysicsStepSystems::BroadPhase
  └─ BroadPhaseSystems::First
       └─ ColliderTreeSystems::UpdateAabbs      // 更新 AABB + 树节点
  └─ BroadPhaseSystems::CollectCollisions       // （第二阶段）生成碰撞对
  └─ BroadPhaseSystems::Last                    // （可选）BeginOptimize
SolverSystems::Finalize                         // （可选）EndOptimize
```

Avian 里 `ColliderTreeSystems::UpdateAabbs` 的精确约束是
`.in_set(PhysicsStepSystems::BroadPhase).after(BroadPhaseSystems::First).before(BroadPhaseSystems::CollectCollisions)`
（`src/collider_tree/mod.rs:80-83`）。

另外两个 system 的落点（`src/collider_tree/update.rs:66-72`）：
`(clear_moved_proxies, update_solver_body_aabbs).chain()`
放在 `.after(PhysicsStepSystems::Finalize).before(PhysicsStepSystems::Last)`。
即**本步统计的 moved proxies 在步末清空**，供下一步 broad phase 重新收集。

**推论**：如果 Kairos 第一阶段不做动力学，可以省略
`update_solver_body_aabbs`（它依赖 `SolverBody`/`ComputedCenterOfMass`/`LinearVelocity`/
`AngularVelocity`，`update.rs:654-664`），只保留 `update_moved_collider_aabbs` 即可覆盖
「手动移动静态碰撞体」这一唯一场景。

### 8.6 目标 (a)：一个能被撞到的静态 box collider

最小 entity 组合（对照 `parry/mod.rs:356-376` 的 required 列表）：

```
(
    RigidBody::Static,                    // 或你自己的 static 标记（见下）
    Collider::cuboid(10.0, 0.5, 10.0),    // parry/mod.rs:747，参数是全尺寸
    Transform::from_xyz(0.0, -0.5, 0.0),
)
```

自动获得：`ColliderMarker`、`ColliderAabb`、`EnlargedAabb`、`CollisionLayers`、
`ColliderDensity`、`ColliderMassProperties`（`parry/mod.rs:358-365`）、
`Position`、`Rotation`（`backend.rs:97-98`）、`ColliderTreeProxyKey`（`collider_tree/mod.rs:60-63`）。

**必须注意的一点**：要让「其他东西能撞到它」，光有静态碰撞体是不够的——
§4.3 已证明 **broad phase 会跳过 static-static 对**（`bvh_broad_phase.rs:124-142`）。
所以还需要至少一个 dynamic 或 kinematic 碰撞体，或者把静态体标为 `Sensor`。
这在 Kairos 当前「静态 plane + 动态 sphere」的形态下恰好满足。

**放哪个树**：`ColliderTreeType::from_body(Some(RigidBody::Static))` → `static_tree`
（`proxy_key.rs:189-196`，使用点在 `update.rs:437-442`、`:465-466`）。
**若 Kairos 第一阶段还没有 `RigidBody` 组件**：走 `Standalone` 分支
（`update.rs:440-442`），即 `standalone_tree`。这完全可行——
Avian 专门为「没有刚体的独立碰撞体」留了这样一棵树（`collider_tree/mod.rs:128-129`）。

### 8.7 目标 (b)：碰撞体被正确跟踪在空间索引中

验收条件（对照源码行为）：

1. spawn 后 `entity.get::<ColliderTreeProxyKey>()` 不是 `PLACEHOLDER`（`update.rs:469`）。
2. 该 key 的 `tree_type()` 与刚体类型一致（`proxy_key.rs:189-196`）。
3. `ColliderTrees::get_proxy(key)` 返回 `Some`（`collider_tree/mod.rs:181-185`）。
4. 移动实体后（改 `Transform`），同一物理步内树节点的 AABB 被更新，
   通过 `tree.bvh` 的 AABB 查询能命中（更新路径 `update.rs:890-931`，
   变更检测在 `:892-898`）。
5. 小幅移动**不**触发树更新（`EnlargedAabb::update` 返回 `false`，`collider/mod.rs:592-602`）。
6. 移除碰撞体后 `get_proxy` 返回 `None`，且 key 复位为 `PLACEHOLDER`（`update.rs:492-498`）。

### 8.8 明确可以推迟的部分

| 可以推迟 | Avian 位置 | 推迟理由 |
|---|---|---|
| `Sensor` | `src/collision/collider/mod.rs:429` | 只影响「是否产生约束」与质量（`backend.rs:149-153`、`bvh_broad_phase.rs:198-202`）。第一阶段无约束求解器，无意义。 |
| `CollisionMargin` | `src/collision/collider/mod.rs:669` | 只影响 AABB 扩大量（`update.rs:94,99`）与稳定性，取 0 不影响正确性。 |
| `Friction` / `Restitution` | `src/dynamics/rigid_body/physics_material.rs:137` / `:305` | 只在接触约束求解时使用（`narrow_phase/system_param.rs:610-629`）。且它们**不是** required component，不实现也不破坏结构。 |
| `ColliderDisabled` | `src/collision/collider/mod.rs:394` | 增加 Case 4/5 两个 observer（`update.rs:195-204`），第一阶段的收益为零。 |
| `ColliderOf` / `RigidBodyColliders` / `ColliderTransform` | `collider_hierarchy/mod.rs:53` / `:212`、`collider_transform/mod.rs:20` | 整套层级传播机制（`collider_hierarchy/plugin.rs:12-61`、`collider_transform/plugin.rs:41-63`、含 283 行的递归传播 + unsafe）。**第一阶段可以只支持「碰撞体和刚体在同一 entity 上」**，把 `body` 字段留成 `None`→Standalone 树。 |
| `ColliderConstructor` / `ColliderConstructorHierarchy` | `src/collision/collider/constructor.rs:316` / `:118` | 依赖 `Reflect`/序列化/mesh 资产/scene 加载（`backend.rs:267-450`），是工具链功能而非运行时核心。 |
| trimesh / polyline / heightfield / voxel 等网格类形状 | `parry/mod.rs:858,874,1114,1007` | 数量多、依赖 Parry 具体 API；先只做 sphere/cuboid/capsule 即可覆盖 Kairos 当前用例。 |
| `trimesh_builder.rs` | `src/collision/collider/trimesh_builder.rs` | 本来就只有 3D + parry 才编译（`collider/mod.rs:24-25`），且用途是导出而非模拟。 |
| `ColliderCachePlugin` | `src/collision/collider/cache.rs` | 只服务 mesh 生成的去重。 |
| 树优化（`ColliderTreeOptimizationPlugin`） | `src/collider_tree/optimization.rs:15` | 只影响查询性能，不影响正确性；用 `refit_all` 兜底即可（`update.rs:828`）。 |
| `obvhs_ext.rs` 的 sweep/distance 遍历 | `src/collider_tree/obvhs_ext.rs:75,467` | 服务 CCD 与射线/形状投射查询，属后续里程碑。 |
| debug render | `src/debug_render/mod.rs` | 可视化，与内核正确性无关。 |
| 4 棵树拆分 | `src/collider_tree/mod.rs:121-130` | 先 1 棵树；`ColliderTreeType` 枚举提前定型即可（`proxy_key.rs:165-174`）。 |

### 8.9 建议的验证顺序

1. **纯几何**：不经 ECS，直接对 `Collider::cuboid(...)` 调 `aabb_with_context`，
   断言与手算 box 一致。镜像 `parry/mod.rs:404-417`。
2. **单个实体**：spawn 一个碰撞体，断言 required 组件全部就位 + `ColliderTreeProxyKey != PLACEHOLDER`。
3. **树查询**：用 `traverse.rs:230` 的 `aabb_traverse` 思路（或直接 `bvh.aabb_traverse`），
   在实体位置附近查询应命中，远处查询不命中。
4. **移动**：改 `Transform` 后跑一个物理步，重复第 3 步。
5. **移除**：despawn 后断言 proxy 已从树中消失。

---

## 9. 与本项目现状（`kairos_physics` + `rapier3d 0.33`）的差异对照

| 维度 | Kairos 现状 | Avian 0.7.0 |
|---|---|---|
| 碰撞体表示 | 单一 ball collider + 静态 box "plane" | `Collider`（`SharedShape` + `scaled_shape` + `scale`，`parry/mod.rs:366-376`），约 40 个构造器 |
| 参数承载 | 推测在 collider 结构体里 | **拆成独立组件**（`ColliderDensity`/`Friction`/`Restitution`/`Sensor`/`CollisionLayers`/`CollisionMargin`/`ColliderDisabled`） |
| 空间索引 | Rapier 内部维护 | `obvhs` BVH，4 棵树，`ColliderTrees` 资源（`collider_tree/mod.rs:121-130`） |
| Broad phase | Rapier 内部 | `BvhBroadPhasePlugin` 查询 `ColliderTrees`（`bvh_broad_phase.rs:51-56`） |
| 层过滤 | Rapier interaction groups | `CollisionLayers::interacts_with`（`layers.rs:424-427`），调用点在 `bvh_broad_phase.rs:257` |
| 位姿 | Rapier `Isometry` | 独立 `Position`/`Rotation` 组件 + 双向 `Transform` 同步（`physics_transform/`） |
| 质量 | Rapier 自动 | `ColliderDensity` → `ColliderMassProperties` → 刚体 `ComputedMass`（§7） |

**推论**：从 Rapier 迁移到自建物理时，最容易低估的是
**`Position`/`Rotation` 与 `Transform` 的双向同步**（`src/physics_transform/mod.rs` 是一整个模块）
与**层级 collider 的 `ColliderTransform` 传播**（`collider_transform/plugin.rs:108-281`，含 unsafe 递归）。
本里程碑（§8）刻意把这两块推迟，是合理的收敛路径。

`PhysicsPlugins` 默认插件组（`src/lib.rs:757-787`）可作为 Kairos 最终插件划分的对照表：

```
PhysicsSchedulePlugin → MassPropertyPlugin → ForcePlugin
→ ColliderHierarchyPlugin → ColliderTransformPlugin
→ [ColliderCachePlugin]                                  // 条件
→ ColliderBackendPlugin::<Collider> → ColliderTreePlugin::<Collider> → NarrowPhasePlugin::<Collider>
→ SolverPlugins
→ BroadPhaseCorePlugin → BvhBroadPhasePlugin::<()>
→ JointPlugin → SpatialQueryPlugin → PhysicsTransformPlugin → PhysicsInterpolationPlugin
```

注意 `ColliderTreePlugin` 紧跟 `ColliderBackendPlugin`、**早于** broad phase 插件，
这与「树是 broad phase 的输入」一致。

---

## 10. 本文件未验证 / 不确定

1. **`Collider` 是否是 `Reflect`**：`parry/mod.rs:355` 的文档明确说「currently not `Reflect`」，
   且 `#[derive(...)]`（`:356`）里确实没有 `Reflect`。**已确认**，但它是否在别处通过
   `app.register_type` 注册过，我没有检查所有注册点。
2. **`swept_aabb_with_context` 在 AABB 更新中的实际使用范围**：我只确认它被
   `update_solver_body_aabbs` 使用（`update.rs:776`）。它是否也被空间查询使用，
   我没有逐一检查 `src/spatial_query/`。
3. **`obvhs 0.3` 的具体 API 语义**（如 `refit_all` 与 `resize_node` 的确切行为）：
   我读的是 Avian 的调用点（`tree.rs:299-301`、`:283-286`），
   **没有** 阅读 `obvhs` crate 自身的源码。关于「`resize_proxy_aabb` 是否只向上 refit」
   我是从 `tree.rs:275-282` 的文档注释推断的，未在 obvhs 内验证。
4. **模块文档与实现的张力**：`collider_tree/mod.rs:10-11` 说 dynamic/kinematic 树
   「每个物理步重建」，但 `update.rs:814-828` 实际做的是 `set_proxy_aabb` + `refit_all`
   （refit 而非 rebuild）；真正的 rebuild 在优化插件里按 moved ratio 触发
   （`optimization.rs:141-156`）。我按实现描述，但**未找到**「每步无条件全量重建」的代码路径。
5. **`ColliderCachePlugin` 的完整行为**：我只读了它的门控（`collider/mod.rs:18-21`）
   与调用点（`backend.rs:271,298-301,330,411-414`），**未读** `cache.rs` 全文（55 行）。
6. **`Sensor` 与 standalone collider 的交互**：源码只在 `bvh_broad_phase.rs:124-142`
   放宽了静态树查询，我**未验证**「sensor 与 standalone 碰撞体的对是否会在
   没有 dynamic 物体时被生成」——`:246-253` 的注释暗示会，但我没有构造测试确认。
7. **`CollisionLayers` 的 `#[component(immutable)]` 在其他 Bevy 版本下的行为**：
   我只确认了 0.7.0 源码中的属性（`layers.rs:358`）与 observer 的应对方式
   （`update.rs:322-343`）。Kairos 若使用不同 Bevy 版本，该属性的语义需另行确认。
8. **性能断言的量化**：`optimization.rs:53-63` 关于 `optimize_in_place` 的内存/可用性权衡、
   `update.rs:951-954` 关于 0.1 阈值的取舍，都是**源码注释中的定性说法**，
   我没有做任何 benchmark，也没有 `cargo build`（任务要求仅静态阅读）。
9. **`ColliderTransformPlugin` 中 unsafe 递归的正确性**：`collider_transform/plugin.rs:143-162`
   与 `:260-279` 使用 `get_unchecked`，其安全性论证写在注释里（`:135-142`、`:199-225`）。
   我**未验证**这些论证，且 `:283` 有 TODO 自承「传播逻辑很容易出错且改动有风险」。
10. **`Group`/`InteractionGroups` 的移除版本**：我仅确认 0.7.0 源码中不存在；
   具体在哪个版本移除、是否有迁移指南，未查（`migration-guides/` 目录存在但未读）。
11. **`ColliderDisabled` 与 `Disabled`（Bevy 内建）的关系**：
   `update.rs:196,202` 同时处理两者，`bvh_broad_phase.rs` 中的 `Allow<Disabled>`
   等泛型 `QueryFilter` 参数（`update.rs:404,476`）我只读了签名，未深入 Bevy 的
   entity disabling 语义。
