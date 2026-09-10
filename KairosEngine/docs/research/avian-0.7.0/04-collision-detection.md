# Avian 0.7.0 碰撞检测流水线参考(从 `Collider` 到 solved contacts)

- 研究目标: 为 Kairos(`kairos_physics` 目前包 `rapier3d 0.33`)**按 Avian 风格自研物理**的学习路线提供碰撞检测阶段的权威参考。本文是该路线的**后期较难里程碑**,回答「collider 已经存在」如何变成「求解器可以消费的成对接触」。
- 权威源码: 只读检出 `/Users/baiaoxiang/KairosEngine/.scratch/avian-research/avian-0.7.0`,tag `v0.7.0`,commit `965e85bf590f53fbf8a29f7ebd0326b5dbc985d1`。
- 路径约定: 本文所有 `path/to/file.rs:LINE` 均**相对于上述检出根目录**。`crates/avian3d/Cargo.toml:15` 一类即检出根下的 `crates/avian3d/Cargo.toml`。
- 方法: 纯静态阅读 + `grep`/`rg`/`wc`。**未运行 `cargo build`/`cargo test`**。所有结构断言均附文件+行号;凡非直接读出的判断标「**推论**」。
- 上游依赖版本(读自源码声明,非猜测): `obvhs = { version = "0.3" }`(`crates/avian3d/Cargo.toml:94`)、`parry3d = { version = "0.27", optional = true }`(`crates/avian3d/Cargo.toml:92`)、`bevy_math = { version = "0.19.0", features = ["approx"] }`(`crates/avian3d/Cargo.toml:86`)。

---

## 0. 先读这一节: 任务书中的若干 API 名在 v0.7.0 **不存在**

本任务书正文按较早版本 Avian 的命名习惯列出了若干标识符。逐条核对后,以下名称在 v0.7.0 源码中**不存在**;下表给出实际存在的替代物。后续章节一律使用**实际**名称。

| 任务书假设的名字 | v0.7.0 实际情况 | 证据 |
|---|---|---|
| resource `BroadCollisionPairs` | **不存在**。broad phase 不再产出中间 pair 列表资源,而是**直接写入 `ContactGraph`** | 全库 `grep -rn "BroadCollisionPairs" src/` 无命中;`src/collision/broad_phase/bvh_broad_phase.rs:173-204` 是唯一落地点 |
| `BroadPhaseConfig` / `trait BroadPhase` | **不存在**。自定义 broad phase 的方式是「禁用插件 + 换插件」(禁用 `BvhBroadPhasePlugin`,加自己的 Plugin) | `src/collision/broad_phase/mod.rs:22-42`、`src/collision/broad_phase/mod.rs:131-154`;示例 `crates/avian3d/examples/custom_broad_phase.rs:10-16` |
| `ContactMatch` 类型 | **不存在**。暖启动匹配是方法 `ContactManifold::match_contacts` + 类型 `PackedFeatureId` | `src/collision/contact_types/mod.rs:426-472`、`src/collision/contact_types/feature_id.rs:11` |
| `CollisionStarted` / `CollisionEnded` | **不存在**。实际是 `CollisionStart` / `CollisionEnd`,且同时是 `Message` 和 `EntityEvent` | `src/collision/collision_events.rs:169-189`、`src/collision/collision_events.rs:266-286` |
| `CollisionEventReader` | **不存在**。用 Bevy 原生 `MessageReader<CollisionStart>` 或 observer `On<CollisionStart>` | `src/collision/collision_events.rs:52`、`src/collision/collision_events.rs:90` |
| `Contact` 类型 | **不存在**。单位接触点是 `ContactPoint`;另有精简的 `SingleContact` | `src/collision/contact_types/mod.rs:603-660`、`src/collision/contact_types/mod.rs:731-742` |
| `CollisionHooks::on_collision` | **不存在**。trait 只有 `filter_pairs` 与 `modify_contacts` 两个方法 | `src/collision/hooks.rs:147`、`src/collision/hooks.rs:164`、`src/collision/hooks.rs:187` |
| `QueryFilterFlags` | **不存在**。`SpatialQueryFilter` 只有 `mask` + `excluded_entities` 两个字段 | `src/spatial_query/query_filter.rs:35-40` |
| `SpatialQueryPipeline` / 多份 pipeline 副本 | **不存在**。`SpatialQuery` 只是一个持有 `Query` + `Res<ColliderTrees>` 的 `SystemParam`,没有预构建加速结构副本 | `src/spatial_query/system_param.rs:59-64`;全库 `grep -rn "SpatialQueryPipeline" src/` 无命中 |
| `SpatialQuery::intersections_with_point` | 实际名 `point_intersections` | `src/spatial_query/system_param.rs:964` |
| `SpatialQuery::intersecting_aabb` | 实际名 `aabb_intersections_with_aabb` | `src/spatial_query/system_param.rs:1069` |
| `NarrowPhaseSystems` 的「compute contacts / emit events」分支 | 实际只有 `First` / `Update` / `Last` 三个空壳语义集,事件触发在**另一个** set `CollisionEventSystems` | `src/collision/narrow_phase/mod.rs:261-268`、`src/collision/narrow_phase/mod.rs:197-198` |

仍然存在的名字: `ColliderAabb`(`src/collision/collider/mod.rs:438`)、`RayHits`(`src/spatial_query/ray_caster.rs:342`)、`RayHitData`(`src/spatial_query/ray_caster.rs:395`)、`ShapeHits`(`src/spatial_query/shape_caster.rs:528`)、`SpatialQueryFilter`(`src/spatial_query/query_filter.rs:35`)、`ContactGraph`(`src/collision/contact_types/contact_graph.rs:76`)、`ContactPair`(`src/collision/contact_types/mod.rs:155`)、`ContactManifold`(`src/collision/contact_types/mod.rs:342`)。

> **推论**: v0.7.0 的 broad phase 把「中间 pair 列表」这一层删掉了 —— 它不再先写 `BroadCollisionPairs` 再让 narrow phase 消费,而是在 `CollectCollisions` 系统里**同步**地把 pair 直接 `add_edge_with` 进 `ContactGraph`(`src/collision/broad_phase/bvh_broad_phase.rs:189-203`)。这意味着对自研引擎而言,`ContactGraph` 是唯一跨阶段边界的数据结构,broad/narrow 两阶段的耦合比 rapier 更紧。设计分层时若想保留「broad phase 输出候选列表」这层抽象,是偏离 Avian 的;但 rapier 就是这么做的(见 `physics-rapier-facts-inventory.md` 的管线描述),两条路线都能用。

---

## 1. 流水线总览: 从 `Collider` 到 solved contacts

### 1.1 调度骨架: 阶段之间的**硬性顺序**由 system set 的 `.chain()` 保证

`PhysicsSchedule` 是独立 schedule(`src/schedule/mod.rs:140-141`),它的顶层 step set 被显式链接:

```text
PhysicsStepSystems::First
  → BroadPhase      // src/schedule/mod.rs:199
  → NarrowPhase     // src/schedule/mod.rs:203
  → Solver          // src/schedule/mod.rs:207
  → Sleeping
  → Finalize        // src/schedule/mod.rs:211
  → Last
```
证据: `src/schedule/mod.rs:96-107`(`schedule.configure_sets((...).chain())`)。集合定义见 `src/schedule/mod.rs:191-214`。

外层 `PhysicsSystems` 是 `First / Prepare / StepSimulation / Writeback / Last`(`src/schedule/mod.rs:161-176`),`run_physics_schedule` 挂在 `StepSimulation`(`src/schedule/mod.rs:110-113`)。另注意 `PhysicsSchedule` 被强制单线程 executor(`src/schedule/mod.rs:90`),并行发生在系统**内部**(`par_iter` / `par_for_each`),不是系统之间 —— 这点对本引擎架构选择很关键。

### 1.2 数据流图(含每阶段承载数据的类型与所在文件)

```text
┌─ 阶段 A: 组件就位 ────────────────────────────────────────────────
│ Collider (Component)                       src/collision/collider/parry/mod.rs:366
│   └─ required components:                  src/collision/collider/backend.rs:97-104
│      Position, Rotation, ColliderMarker, ColliderAabb, EnlargedAabb,
│      CollisionLayers, ColliderDensity, ColliderMassProperties
│   └─ ColliderTreePlugin 追加 required:      src/collider_tree/mod.rs:60-63
│      ColliderTreeProxyKey
└───────────────────────────────────────────────────────────────────
                     │
┌─ 阶段 B: AABB 更新 (BroadPhase 内部的第一步) ──────────────────────
│ set: ColliderTreeSystems::UpdateAabbs
│      .in_set(PhysicsStepSystems::BroadPhase)
│      .after(BroadPhaseSystems::First)
│      .before(BroadPhaseSystems::CollectCollisions)
│      src/collider_tree/mod.rs:78-84
│
│ sys: update_moved_collider_aabbs::<C>      src/collider_tree/update.rs:839
│   写: ColliderAabb  (src/collision/collider/mod.rs:438)
│       EnlargedAabb  (src/collision/collider/mod.rs:579)
│       EnlargedProxies 位图 (src/collider_tree/update.rs:574)
│       ColliderTree.bvh 节点 AABB + MovedProxies
│         src/collider_tree/update.rs:984 (update_tree)
│         src/collider_tree/update.rs:512-518 (MovedProxies)
└───────────────────────────────────────────────────────────────────
                     │
┌─ 阶段 C: 空间索引 ────────────────────────────────────────────────
│ Res<ColliderTrees> { dynamic_tree, kinematic_tree, static_tree, standalone_tree }
│      src/collider_tree/mod.rs:120-130
│ 每棵树: ColliderTree { bvh: obvhs::bvh2::Bvh2, proxies: StableVec<ColliderTreeProxy>, … }
│      src/collider_tree/tree.rs:24-36
└───────────────────────────────────────────────────────────────────
                     │
┌─ 阶段 D: Broad phase ────────────────────────────────────────────
│ set: BroadPhaseSystems::CollectCollisions
│      src/collision/broad_phase/mod.rs:201-209
│ sys: collect_collision_pairs::<H>          src/collision/broad_phase/bvh_broad_phase.rs:51
│   读: Res<ColliderTrees>, Res<MovedProxies>, Res<JointGraph>
│   滤: CollisionLayers / 同 body / JointGraph / 重复 pair /
│       CollisionHooks::filter_pairs          bvh_broad_phase.rs:256-295
│   写: ContactGraph.add_edge_with(...)       bvh_broad_phase.rs:189-203
│         → ContactEdge + ContactPair(manifolds 为空)
└───────────────────────────────────────────────────────────────────
                     │
┌─ 阶段 E: Narrow phase ───────────────────────────────────────────
│ set: NarrowPhaseSystems::Update .in_set(PhysicsStepSystems::NarrowPhase)
│      src/collision/narrow_phase/mod.rs:128-137, 144-151
│ sys: update_narrow_phase::<C, H>            src/collision/narrow_phase/mod.rs:274
│   → NarrowPhase::update                     src/collision/narrow_phase/system_param.rs:117
│      ├─ update_contacts::<H>                system_param.rs:440
│      │    par_for_each 遍历 ContactGraph.active_pairs_mut()   system_param.rs:480
│      │    ├─ EnlargedAabb 重叠复检 + layers 复检             system_param.rs:511-516
│      │    ├─ AnyCollider::contact_manifolds_with_context     system_param.rs:706
│      │    │     → Collider 实现 src/collision/collider/parry/mod.rs:419
│      │    │     → Parry: contact_query::contact_manifolds    contact_query.rs:156
│      │    ├─ 余量/推测余量/裁剪接触点                         system_param.rs:718-769
│      │    ├─ hooks.modify_contacts                           system_param.rs:774-781
│      │    └─ ContactManifold::match_contacts (暖启动)         system_param.rs:789-798
│      └─ 串行遍历 ContactStatusBits 位图                       system_param.rs:144-392
│           ├─ CollisionStart / CollisionEnd (Message)          system_param.rs:215/270
│           ├─ CollidingEntities 增删                           system_param.rs:224/279
│           └─ ConstraintGraph::push_manifold / pop_manifold    system_param.rs:243/301
└───────────────────────────────────────────────────────────────────
                     │
┌─ 阶段 F: 求解器约束生成与求解 ────────────────────────────────────
│ Res<ConstraintGraph> 按 color 分组            src/dynamics/solver/constraint_graph.rs:129-132
│ sys: prepare_contact_constraints .in_set(SolverSystems::PrepareContactConstraints)
│      src/dynamics/solver/plugin.rs:111-114, 363
│   → ContactConstraint::generate(...)          src/dynamics/solver/plugin.rs:424
│ 写: Res<ContactConstraints>(Vec<ContactConstraint>)   src/dynamics/solver/plugin.rs:354
│ SubstepSchedule: WarmStart → SolveConstraints → Relax → …    plugin.rs:129/132/136
│ 回写: store_contact_impulses → ContactPoint::warm_start_*_impulse  plugin.rs:119, 720
└───────────────────────────────────────────────────────────────────
                     │
┌─ 阶段 G: 碰撞事件广播 (已求解之后) ───────────────────────────────
│ set: CollisionEventSystems .in_set(PhysicsStepSystems::Finalize)
│      src/collision/narrow_phase/mod.rs:138-141
│ sys: trigger_collision_events (独占 world)    src/collision/narrow_phase/mod.rs:310
│   读 CollisionStart/CollisionEnd 的 Message,按 CollisionEventsEnabled
│   逐个反射成 EntityEvent(角色互换后双发)      src/collision/narrow_phase/mod.rs:321-378
└───────────────────────────────────────────────────────────────────
```

### 1.3 各阶段承载数据的资源/组件一览

| 阶段 | 承载者 | 种类 | 定义位置 |
|---|---|---|---|
| 形状与刚体归属 | `Collider` / `ColliderOf` | Component | `src/collision/collider/parry/mod.rs:366`、`src/collision/collider/collider_hierarchy/mod.rs:53` |
| 紧 AABB | `ColliderAabb` | Component | `src/collision/collider/mod.rs:438` |
| 扩张 AABB(BVH 用,抗抖动) | `EnlargedAabb` | Component | `src/collision/collider/mod.rs:579` |
| 层级过滤 | `CollisionLayers` / `LayerMask` | Component | `src/collision/collider/layers.rs:362`、`layers.rs:86` |
| 传感器 | `Sensor` | Component | `src/collision/collider/mod.rs:429` |
| 空间索引 | `ColliderTrees` | Resource | `src/collider_tree/mod.rs:121` |
| 代理句柄 | `ColliderTreeProxyKey` | Component | `src/collider_tree/proxy_key.rs:15` |
| 本步移动代理 | `MovedProxies` | Resource | `src/collider_tree/update.rs:513` |
| 所有 pair 与 manifold | `ContactGraph` | Resource | `src/collision/contact_types/contact_graph.rs:76` |
| 求解分组 | `ConstraintGraph` | Resource | `src/dynamics/solver/constraint_graph.rs:129` |
| 最终约束数组 | `ContactConstraints` | Resource | `src/dynamics/solver/plugin.rs:354` |
| 事件消息 | `CollisionStart` / `CollisionEnd` | Message + EntityEvent | `src/collision/collision_events.rs:169/266` |
| 用户可读碰撞集合 | `Collisions` | SystemParam | `src/collision/contact_types/system_param.rs:53` |

插件注册顺序(`PhysicsPlugins` plugin group)对理解依赖很有帮助: `ColliderBackendPlugin::<Collider>` → `ColliderTreePlugin::<Collider>` → `NarrowPhasePlugin::<Collider>` → `SolverPlugins` → `BroadPhaseCorePlugin` → `BvhBroadPhasePlugin::<()>`(`src/lib.rs:773-783`)。注意 **broad phase 插件排在 narrow phase 与 solver 之后注册**(`src/lib.rs:782-783`),因为 broad phase 依赖 `ContactGraph`/`JointGraph` 已被 `BroadPhaseCorePlugin` 初始化(`src/collision/broad_phase/mod.rs:178-179`)。

---

## 2. Broad phase

### 2.1 两个插件,不是一个

`src/collision/broad_phase/mod.rs` 只有 209 行,且**不含算法**。它定义:

- `BroadPhaseCorePlugin`(`src/collision/broad_phase/mod.rs:174`):`build()` 里 `init_resource::<ContactGraph>()` + `init_resource::<JointGraph>()`(`mod.rs:178-179`),并配置 set 链 `First → CollectCollisions → Last` 且整体 `.in_set(PhysicsStepSystems::BroadPhase)`(`mod.rs:181-190`);`finish()` 注册 `CollisionDiagnostics`(`mod.rs:193-196`)。
- `BroadPhaseSystems { First, CollectCollisions, Last }`(`src/collision/broad_phase/mod.rs:200-209`)。注释明确 `CollectCollisions` 的语义是「找 AABB 重叠对**并为其在 `Collisions` 中创建 contact pairs**」(`mod.rs:204-206`)。

真正的算法在 `BvhBroadPhasePlugin`(`src/collision/broad_phase/bvh_broad_phase.rs:31`),它只做一件事:把 `collect_collision_pairs::<H>` 加进 `BroadPhaseSystems::CollectCollisions`(`bvh_broad_phase.rs:44-48`)。泛型参数 `H: CollisionHooks` 使 broad phase 可以调用用户的 `filter_pairs`。

### 2.2 算法到底是哪一种: **BVH 的 AABB 查询横扫,不是 sweep-and-prune**

`collect_collision_pairs` 的算法契约(逐条附证):

1. 只对**本步移动过的代理**发起查询:`moved_proxies.proxies().par_splat_map(ComputeTaskPool::get(), None, |_chunk_index, proxies| {…})`(`bvh_broad_phase.rs:71-74`)。
2. 对每个移动代理,依次查询 **4 棵树**:dynamic(`bvh_broad_phase.rs:91-105`)、kinematic(`108-122`)、static(`127-141`,仅在「非 static 代理 或 该代理是 sensor」时才查,`125`)、standalone(`145-159`)。
3. 单次查询的实现是 `tree.bvh.aabb_traverse(proxy_aabb1, |bvh, node_index| {…})`(`bvh_broad_phase.rs:225`),即用查询 AABB 在 BVH 里做一次遍历,取出该叶子节点覆盖的 primitive 区间 `node.first_index..node.first_index + node.prim_count`(`bvh_broad_phase.rs:227-231`),再把 primitive 索引映射回 `ProxyId`(`bvh_broad_phase.rs:231-232`)。

> **结论(源码事实)**: 默认 broad phase = **对「移动代理 × 4 棵 BVH」的 AABB 重叠查询**,每帧只查询移动过的代理(O(移动数 · log N)),**没有** sweep-and-prune、没有排序列表、没有 `BroadCollisionPairs` 中间资源。
> **推论**: 这实际上把「空间划分」的职责完全下沉给了 `ColliderTree`;broad phase 只是 tree 的一层薄封装 + 过滤。学习路线上,先做 `ColliderTree` 再做 broad phase 是唯一自然的顺序 —— 没有 tree 就没有 broad phase。

### 2.3 配对过滤的全部条件(顺序即源码顺序)

`query_tree`(`bvh_broad_phase.rs:209-302`)对每个候选 `proxy2` 依次判断:

| # | 条件 | 行号 |
|---|---|---|
| 1 | 跳过自己(`proxy_key1 == proxy_key2`) | `225-237` |
| 2 | 避免双份 pair:两边都移动时只让「较大」的一侧处理(static/standalone 树的 sensor 例外) | `241-254` |
| 3 | `CollisionLayers::interacts_with` | `256-259` |
| 4 | 同一个 rigid body 上的两个 collider 不碰撞(`proxy1.body == proxy2.body`) | `261-264` |
| 5 | `PairKey` 已存在于 `ContactGraph.pair_set` | `269-273` |
| 6 | `JointGraph` 中该 body 对之间存在 `collision_disabled` 的边 | `275-283` |
| 7 | 用户 hook `hooks.filter_pairs(entity1, entity2, commands)`(仅当 `ColliderTreeProxyFlags::CUSTOM_FILTER` 置位) | `285-295` |

只有第 7 步用到 `CollisionHooks`;第 5 步的 `ContactGraph::contains_key`(`src/collision/contact_types/contact_graph.rs:301-303`)是去重的关键。

### 2.4 落进 `ContactGraph` 时写入的初始 flag

`broad_collision_pairs` 收集完(串行 drain,`bvh_broad_phase.rs:167-170`)后,对每对做:

- `ContactEdge::new(proxy1.collider, proxy2.collider)` 并填 `body1`/`body2`(`bvh_broad_phase.rs:177-179`);
- `ContactEdgeFlags::CONTACT_EVENTS` ← 两个代理 flag 的并集是否含 `CONTACT_EVENTS`(`bvh_broad_phase.rs:181-187`;
  `mod.rs:181` 的 `proxy1.flags.union(proxy2.flags)`);
- `add_edge_with(contact_edge, |pair| { … })`(`bvh_broad_phase.rs:189`),在回调里设置:
  - `ContactPairFlags::MODIFY_CONTACTS` ← flag 并集含 `MODIFY_CONTACTS`(`bvh_broad_phase.rs:193-196`);
  - `ContactPairFlags::GENERATE_CONSTRAINTS` ← **既不**是 `BODY_DISABLED` **也不**是 `SENSOR`(`bvh_broad_phase.rs:198-202`)。

`ColliderTreeProxyFlags` 的定义与来源: `src/collider_tree/tree.rs:57-72`(SENSOR / BODY_DISABLED / CUSTOM_FILTER / MODIFY_CONTACTS / CONTACT_EVENTS),由 `ColliderTreeProxyFlags::new(is_sensor, is_body_disabled, events_enabled, active_hooks)` 构造(`tree.rs:74-101`)。诊断计时 `diagnostics.broad_phase += start.elapsed()`(`bvh_broad_phase.rs:206`)。

### 2.5 自定义 broad phase 的**唯一**官方姿势

文档给出的模式(`src/collision/broad_phase/mod.rs:44-154`)与示例(`crates/avian3d/examples/custom_broad_phase.rs`)一致:

```rust
app.add_plugins(
    PhysicsPlugins::default()
        .build()
        .disable::<BvhBroadPhasePlugin>()   // crates/avian3d/examples/custom_broad_phase.rs:13
        .add(BruteForceBroadPhasePlugin),   // :14
);
```

自定义插件只需把自己的系统放进 `BroadPhaseSystems::CollectCollisions`(`crates/avian3d/examples/custom_broad_phase.rs:69-73`),并在系统里自行完成**全部**配对过滤。文档明确列出了替换实现必须自己承担的责任(`src/collision/broad_phase/mod.rs:33-41`): `CollisionLayers`、`CollisionHooks`、`JointCollisionDisabled`、跳过 parent rigid body 自身的 collider、跳过非 dynamic × 非 dynamic。

暴力示例的过滤只有 4 条: 两边都能取到 `RigidBody`(`custom_broad_phase.rs:91-98`)、至少一方 dynamic(`:100-103`)、`aabb1.intersects(aabb2)`(`:105-107`)、joint 未禁用碰撞(`:110-114`);**它没有做 layers 检查** —— 这正是「用自己的实现就要自己负责」的实证。

注意 `BroadPhaseCorePlugin` **不能**被替换,它提供 `ContactGraph`/`JointGraph` 初始化与 set 骨架(`src/collision/broad_phase/mod.rs:164-174` 的 doc 说明它是 other broad phase plugins 的基础)。

---

## 3. Collider tree

### 3.1 BVH 库与数据结构

- BVH 实现来自外部 crate **`obvhs` 0.3**(`crates/avian3d/Cargo.toml:94`);`ColliderTree.bvh` 的类型就是 `obvhs::bvh2::Bvh2`(`src/collider_tree/tree.rs:5-10`、`tree.rs:26`)。
- `ColliderTree` 字段(`src/collider_tree/tree.rs:24-36`):
  - `pub bvh: Bvh2`
  - `pub proxies: StableVec<ColliderTreeProxy>`(稳定索引数组,`ProxyId` 即其下标,`tree.rs:165-166`)
  - `pub moved_proxies: Vec<ProxyId>`(供 tree 优化使用,`tree.rs:29-33`)
  - `pub workspace: ColliderTreeWorkspace`(复用分配,`tree.rs:35`)
- `ColliderTreeWorkspace` 字段直接暴露算法的三个组成部分(`src/collider_tree/tree.rs:128-138`): `PlocBuilder`(PLOC = Parallel, Locally Ordered Clustering 构建器)、`ReinsertionOptimizer`(按 SAH 成本重插入)、`HeapStack<SiblingInsertionCandidate>`(插入候选栈)、`temp_flags`(局部重建的标记)。
- `ColliderTreeProxy`(`src/collider_tree/tree.rs:40-49`)持 `collider: Entity`、`body: Option<Entity>`、`layers: CollisionLayers`、`flags: ColliderTreeProxyFlags`。
- 四棵树按刚体类型分离,见 `ColliderTrees`(`src/collider_tree/mod.rs:121-130`): `dynamic_tree` / `kinematic_tree` / `static_tree` / `standalone_tree`;`tree_for_type` / `tree_for_type_mut` 做映射(`mod.rs:135-153`)。模块文档说明了更新策略差异: **dynamic 与 kinematic 树每个 physics step 重建/更新,static 树只在静态 collider 增删改时增量更新**(`src/collider_tree/mod.rs:8-11`)。

### 3.2 代理键: `ProxyId` + `ColliderTreeType` 打包进一个 `u32`

- `ColliderTreeProxyKey(u32)`(`src/collider_tree/proxy_key.rs:15`)是**组件**(`#[derive(Component)]`,`proxy_key.rs:14`),由 `ColliderTreePlugin` 注册为 `C` 的 required component(`src/collider_tree/mod.rs:60-63`),初值 `ColliderTreeProxyKey::PLACEHOLDER = ColliderTreeProxyKey(u32::MAX)`(`proxy_key.rs:19`)。
- 编码: 低 2 位存 `ColliderTreeType`,高 30 位存 `ProxyId` —— `ColliderTreeProxyKey((id.id() << 2) | (tree_type as u32))`(`proxy_key.rs:23-26`);解码 `self.0 >> 2`(`proxy_key.rs:30-32`)、`self.0 & 0b11`(`proxy_key.rs:36-45`)。
- `ProxyId(u32)` 只用低 30 位,`debug_assert!(id < (1 << 30))`(`proxy_key.rs:107-120`)。
- `ColliderTreeType` 是 `Dynamic = 0 / Kinematic = 1 / Static = 2 / Standalone = 3`(`proxy_key.rs:165-174`),`from_body(Option<RigidBody>)` 做映射(`proxy_key.rs:189-196`),`ALL: [ColliderTreeType; 4]`(`proxy_key.rs:178-183`)。

> **推论**: 把 tree 类型塞进 key 的低位是为了「一个 `u32` 同时回答『在哪个树』和『树里第几个』」,从而 `ColliderTrees::get_proxy(key)` 无需额外参数(`src/collider_tree/mod.rs:181-185`),也便于在 `MovedProxies` 这样的 `Vec` + `HashSet` 结构里做单一键(`update.rs:513-518`)。自研引擎若只有「静态 + 动态」两棵树,这个 2-bit 技巧价值不大,但「key 自带树归属」的设计值得抄。

### 3.3 更新如何批量化与增量

`src/collider_tree/update.rs`(1024 行)是这一节的主体。关键机制:

1. **AABB 有两层**: 紧的 `ColliderAabb` 与带余量的 `EnlargedAabb`。余量常量 `const AABB_MARGIN: Scalar = 0.05`(`update.rs:34`),注释说明其目的是「允许代理移动一小段而不触发树更新」(`update.rs:29-33`)。`EnlargedAabb::update(&aabb, margin)` 若新 AABB 仍在扩张 AABB 内则返回 `false`(不更新,`src/collision/collider/mod.rs:592-602`)。
2. **批量位图 + 线程局部**: 移动的代理先用**线程局部 `BitVec`** 记录(`EnlargedProxiesBitVec { global, thread_local: ThreadLocal<RefCell<BitVec>> }`,`update.rs:620-623`),并行遍历结束后按类型 `combine_thread_local()` 做按位或合并(`update.rs:638-643`),再串行处理(`update.rs:806-829`、`update.rs:936-978`)。位图按 `ProxyId` 索引,四棵树各一份(`EnlargedProxies` 的 `dynamic/kinematic/static/standalone_proxies`,`update.rs:574-584`)。注释解释了为何不按 entity 索引而按 proxy 索引(避免极稀疏位图,`update.rs:575-579`)。
3. **两个 AABB 更新系统,时机不同**:
   - `update_moved_collider_aabbs::<C>`(`update.rs:839`)跑在 `ColliderTreeSystems::UpdateAabbs`(`update.rs:56-63`),用途是处理**用户手动移动**的 collider;用 change detection 剪枝:`if !pos.last_changed().is_newer_than(last_tick.0, this_run) && !rot… && !collider…` 则直接 return(`update.rs:893-898`)。
   - `update_solver_body_aabbs::<C>`(`update.rs:653`)跑在 `PhysicsStepSystems::Finalize` 之后、`Last` 之前(`update.rs:66-72`),处理刚被积分过的 `SolverBody`;它会计算**扫掠 AABB**(`swept_aabb_with_context`,考虑角速度造成的偏置点速度,`update.rs:744-778`)。
4. **增量 vs 全量重建的阈值**: `update_moved_collider_aabbs` 统计 `moved_ratio = moved_count / tree.proxies.len()`,当 `moved_ratio < 0.1` 时用 `tree.resize_proxy_aabb`(只从被改动的叶子向上 refit),否则用 `tree.set_proxy_aabb` + 一次 `tree.refit_all()`(`update.rs:944-977`,含注释「TODO: Tune the threshold ratio」)。对应 `ColliderTree::resize_proxy_aabb`(`tree.rs:283-286`)、`refit_all`(`tree.rs:299-301`)。
   `update_solver_body_aabbs` 则**无条件**对 dynamic/kinematic 树 `refit_all()`(`update.rs:828`),注释指出小规模移动时只 refit 受影响叶子会更快(「TODO」,`update.rs:826-827`)。
5. **全量/局部重建与优化的入口**: `ColliderTree::rebuild_full()` 走 `ploc_builder.full_rebuild(&mut bvh, PlocSearchDistance::Minimum, SortPrecision::U64, 0)`(`tree.rs:305-312`);`rebuild_partial(&[u32])` 先 `compute_rebuild_path_flags` 再 `ploc_builder.partial_rebuild`(`tree.rs:316-329`);`optimize(batch_size_ratio)` / `optimize_candidates` 走 `ReinsertionOptimizer::run` / `run_with_candidates`(`tree.rs:336-352`)。
6. **异步优化与调度位置**: `ColliderTreeOptimizationPlugin`(`src/collider_tree/optimization.rs:18-28`)加两个系统:`optimize_trees` 在 `ColliderTreeSystems::BeginOptimize`(= `BroadPhaseSystems::Last`,`src/collider_tree/mod.rs:85-88`),`block_on_optimize_trees` 在 `EndOptimize`(= `SolverSystems::Finalize`,`src/collider_tree/mod.rs:89-93`)。二者之间用 `AsyncComputeTaskPool` 跑异步任务(`optimization.rs:10-12`、`optimization.rs:22-26`)。配置资源 `ColliderTreeOptimization` 有 `optimization_mode`、`optimize_in_place`、`use_async_tasks`(`optimization.rs:34-83`);文档特别指出 `optimize_in_place = true` 会让树在模拟步期间**不可用于空间查询**(例如 collision hooks 里的查询),`optimization.rs:41-51`。
7. **增删与状态同步全靠 observer**: `ColliderTreeUpdatePlugin::build` 用 11 个 observer 覆盖 11 种情形(源码里逐条编号,`update.rs:109-121` 的注释列表),对应实现散落在 `update.rs:123-399`。helper 是 `add_to_tree_on`(`update.rs:404`)与 `remove_from_tree_on`(`update.rs:476`),二者都维护 `MovedProxies`(`update.rs:472`、`update.rs:495`)并把 `ColliderTreeProxyKey` 写成 `PLACEHOLDER` 表示失效(`update.rs:498`)。

### 3.4 配对查询如何发起

`src/collider_tree/traverse.rs` 给 `ColliderTree` 添加了一组遍历方法(`impl ColliderTree`,`traverse.rs:14`):

| 方法 | 用途 | 行号 |
|---|---|---|
| `ray_traverse_closest` | 光线最近的唯一命中(返回 `(ProxyId, Scalar)`) | `traverse.rs:24-46` |
| `ray_traverse_all` | 光线全部命中,回调返回 `false` 可提前终止 | `traverse.rs:59-72` |
| `sweep_traverse_closest` / `sweep_traverse_all` | AABB 沿方向扫掠 | `traverse.rs:85-114` / `127-154` |
| `squared_distance_traverse_closest` | 点最近投影 | `traverse.rs:164-190` |
| `point_traverse` | 点包含遍历 | `traverse.rs:200-220` |
| **`aabb_traverse`** | **AABB 重叠遍历 —— broad phase 用的就是它** | `traverse.rs:230-245` |

额外的 obvhs 适配在 `src/collider_tree/obvhs_ext.rs`(`Bvh2Ext` trait,`obvhs_ext.rs:74`;`obvhs_ray` 辅助函数,`obvhs_ext.rs:526`)。

---

## 4. Narrow phase

### 4.1 插件与系统

`NarrowPhasePlugin<C: AnyCollider, H: CollisionHooks = ()>`(`src/collision/narrow_phase/mod.rs:59-69`)带两个泛型: collider 后端类型与 hooks 类型。`build()` 干这些事(`mod.rs:106-185`):

1. 初始化资源: `NarrowPhaseConfig`、`ContactGraph`、`ConstraintGraph`、`JointGraph`、`ContactStatusBits`、`DefaultFriction`、`DefaultRestitution`(`mod.rs:109-115`);`parallel` feature 下再加 `ThreadLocalContactStatusBits`(`mod.rs:117-118`)。
2. 注册两条 message: `app.add_message::<CollisionStart>()` / `<CollisionEnd>()`(`mod.rs:120-121`)。
3. 若 `generate_constraints` 为真则 `init_resource::<ContactConstraints>()`(`mod.rs:123-125`)。该开关由 `NarrowPhasePlugin::new(schedule, generate_constraints)` 给出,默认 `Self::new(PhysicsSchedule, true)`(`mod.rs:80-93`)。
4. 配置 set: `NarrowPhaseSystems::{First, Update, Last}.chain().in_set(PhysicsStepSystems::NarrowPhase)`(`mod.rs:128-137`),以及 `CollisionEventSystems.in_set(PhysicsStepSystems::Finalize)`(`mod.rs:138-141`)。
5. 加入核心系统 `update_narrow_phase::<C, H>` 到 `NarrowPhaseSystems::Update`,并显式 `.ambiguous_with_all()` —— 注释说明这是为了支持同时存在多个 collision backend(`mod.rs:144-151`)。
6. `NarrowPhaseInitialized` 资源(`mod.rs:99-100`)保证下面的 observer 只注册一次(即使有多个 collider 类型): `remove_collider_on::<Add, (Disabled, ColliderDisabled)>`、`remove_collider_on::<Remove, ColliderMarker>`、`on_add_sensor`、`on_remove_sensor`、`on_body_remove_rigid_body_disabled`、`on_disable_body`、`remove_body_on::<Insert, RigidBody>`、`remove_body_on::<Remove, RigidBody>`(`mod.rs:153-182`),以及 `trigger_collision_events`(`mod.rs:174-181`)。

**系统执行顺序**(回答「更新 AABB? 计算接触? 发事件?」):

- AABB 更新**不在** narrow phase,而在 broad phase 的 `ColliderTreeSystems::UpdateAabbs`(见 §3.3)。
- 计算接触 = `update_narrow_phase`(`NarrowPhaseSystems::Update`,`mod.rs:144-151`)。
- 事件**写入**发生在 `update_narrow_phase` 内部(`mod.rs:276-277` 的两个 `MessageWriter`,由 `NarrowPhase::update` 在 `system_param.rs:164` 与 `270` 写入)。
- 事件**触发(observer)**发生在 `CollisionEventSystems` ∈ `PhysicsStepSystems::Finalize`(`mod.rs:174-181`),即 solver 之后(`mod.rs:193-198` 的 doc 明确: 运行于 solver 之后、接触冲量已计算并应用)。这是「Message 立即写、EntityEvent 延后触发」的两段式设计。

### 4.2 `NarrowPhase` system param

`pub struct NarrowPhase<'w, 's, C: AnyCollider>`(`src/collision/narrow_phase/system_param.rs:70-92`),字段:

| 字段 | 类型 | 行号 |
|---|---|---|
| `collider_query` | `Query<ColliderQuery<C>, Without<ColliderDisabled>>` | `73` |
| `colliding_entities_query` | `Query<&mut CollidingEntities>` | `74` |
| `body_query` | `Query<RigidBodyQuery, Without<RigidBodyDisabled>>` | `75` |
| `body_islands` | `Query<&mut BodyIslandNode, Or<(With<Disabled>, Without<Disabled>)>>` | `76-77` |
| `pub contact_graph` | `ResMut<ContactGraph>` | `78` |
| `pub joint_graph` | `ResMut<JointGraph>` | `79` |
| `pub constraint_graph` | `ResMut<ConstraintGraph>` | `80` |
| `pub islands` | `Option<ResMut<PhysicsIslands>>` | `81` |
| `contact_status_bits` | `ResMut<ContactStatusBits>` | `82` |
| `thread_local_contact_status_bits` | `ResMut<ThreadLocalContactStatusBits>`(仅 `parallel`) | `83-84` |
| `pub config` | `Res<NarrowPhaseConfig>` | `85` |
| `default_friction` / `default_restitution` | `Res<DefaultFriction>` / `Res<DefaultRestitution>` | `86-87` |
| `length_unit` | `Res<PhysicsLengthUnit>` | `88` |
| `default_speculative_margin` / `contact_tolerance` | `Local<'s, Scalar>`(按长度单位缩放后缓存) | `90-91` |

查询形状的 `QueryData` 定义: `ColliderQuery`(`system_param.rs:28-43`,含 `shape`、`enlarged_aabb`、`position`、`rotation`、`transform`、`layers`、`friction`、`restitution`、`collision_margin`、`speculative_margin`、`is_sensor`)与 `RigidBodyQuery`(`system_param.rs:45-59`,含 `rb`、`center_of_mass`、`linear_velocity`、`angular_velocity` 等)。

对外公开的 API 只有一个方法 `pub fn update<H: CollisionHooks>(&mut self, collision_started_writer, collision_ended_writer, delta_secs, hooks, context, commands)`(`system_param.rs:117-127`)。其余 `update_contacts` / `add_colliding_entities` / `remove_colliding_entities` 都是私有(`system_param.rs:406`、`420`、`440`)。

`ContactStatusBits(pub BitVec)`(`system_param.rs:96-97`)与 `ThreadLocalContactStatusBits(pub ThreadLocal<RefCell<BitVec>>)`(`system_param.rs:105-107`)用于记录**本步状态发生变化的 pair**(新增/移除),从而让主循环只串行处理变化的那些。

### 4.3 Parry 如何被调用: 后端无关的间接层

三层结构(自下而上):

1. **Parry 层**: `src/collision/collider/parry/contact_query.rs:156-257` 的 `pub fn contact_manifolds(...)`,内部调用
   `parry::query::DefaultQueryDispatcher.contact_manifolds(&isometry12, shape1, shape2, prediction_distance, &mut new_manifolds, &mut None)`(`contact_query.rs:177-184`),把 Parry 的 `ContactManifold<(), ()>` 转成 Avian 的 `ContactManifold`(`contact_query.rs:227-256`),并把 `contact.fid1`/`fid2` 转成 `PackedFeatureId` 写进接触点(`contact_query.rs:249-250`)。若 dispatcher 返回 `Err`(自定义形状不支持),退化为 support-map 接触 `parry::query::contact::contact_support_map_support_map`(`contact_query.rs:189-225`)。
   单点查询是另一个函数 `pub fn contact(...) -> Result<Option<SingleContact>, UnsupportedShape>`(`contact_query.rs:64-109`),内部 `parry::query::contact(...)`(`contact_query.rs:78-84`),并且**只做求值不做 manifold 组装**(注意 `contact_query.rs:111-113` 的 TODO:「Add a persistent version of this that tries to reuse previous contact manifolds」)。
   同模块还导出 `closest_points`(`contact_query.rs:343`)、`distance`(`contact_query.rs:421`)、`intersection_test`(`contact_query.rs:489`)、`time_of_impact`(`contact_query.rs:570`),全部薄封装 Parry。
2. **后端无关层**: trait `AnyCollider`(`src/collision/collider/mod.rs:128-259`)声明
   - `type Context: for<'w,'s> ReadOnlySystemParam<…>`(`mod.rs:204`)
   - `fn aabb_with_context(&self, position, rotation, context: AabbContext<Self::Context>) -> ColliderAabb`(`mod.rs:214-219`)
   - `fn swept_aabb_with_context(...)`(有默认实现,合并起止 AABB,`mod.rs:230-240`)
   - `fn contact_manifolds_with_context(&self, other, position1, rotation1, position2, rotation2, prediction_distance, manifolds: &mut Vec<ContactManifold>, context: ContactManifoldContext<Self::Context>)`(`mod.rs:248-258`)
   另有便利 trait `SimpleCollider: AnyCollider<Context = ()>`(`mod.rs:263-320`),把 context 换成 `AabbContext::fake()` / `ContactManifoldContext::fake()`(`mod.rs:81-88`、`mod.rs:116-124`),并自动为所有 `Context = ()` 的类型实现(`mod.rs:322`)。
3. **默认 `Collider` 实现**: `impl AnyCollider for Collider { type Context = (); … }`(`src/collision/collider/parry/mod.rs:401-441`),其中 `aabb_with_context` 调 `self.shape_scaled().compute_aabb(&make_pose(position, rotation))`(`parry/mod.rs:410-416`),`contact_manifolds_with_context` 直接转调 `contact_query::contact_manifolds(...)`(`parry/mod.rs:430-439`)。

narrow phase 的调用点只有一处: `collider1.shape.contact_manifolds_with_context(collider2.shape, collider1.position.0, *collider1.rotation, collider2.position.0, *collider2.rotation, max_contact_distance, &mut contacts.manifolds, context)`(`system_param.rs:706-715`),其中 `context` 是 `ContactManifoldContext::new(collider1.entity, collider2.entity, collider_context)`(`system_param.rs:701-705`)。

后端上下文可携带任意只读 `SystemParam`(`AnyCollider::Context` 的 doc 示例用 `(SQuery<&'static VoxelData>, SRes<Time>)`,`src/collision/collider/mod.rs:170-202`),`ColliderBackendPlugin<C>` 允许为非 `Collider` 类型接入(`src/collision/collider/backend.rs:33-66`),但明确注明 **自定义 collider 暂不支持空间查询**(`backend.rs:67`)。

### 4.4 接触更新的策略: **每帧全量重算 + 事后特征匹配暖启动**

这是最容易误解的一点。源码事实:

- `contacts.manifolds` 被**完全重算**: 调用前先 `let old_manifolds = contacts.manifolds.clone();`(`system_param.rs:696`),然后 `contact_manifolds_with_context` 在内部 `manifolds.clear()`(`src/collision/collider/parry/contact_query.rs:186-187`)再重新填充。
- **没有**把上一帧的 manifold 交给 Parry 复用。两处 TODO 明说: 「TODO: It'd be good to persist the manifolds and let Parry match contacts. This isn't currently done because it requires using Parry's contact manifold type.」(`system_param.rs:698-699`),以及 `contact_query.rs:174` 的「TODO: Reuse manifolds from previous frame to improve performance」。
- 暖启动靠**事后匹配**: 若 `contacts.manifolds.len() <= 4 && self.config.match_contacts`,对每个新 manifold 调 `manifold.match_contacts(&previous_manifold.points, distance_threshold)`,`distance_threshold = 0.1 * self.length_unit.0`(`system_param.rs:789-798`)。
- `match_contacts` 的匹配规则(`src/collision/contact_types/mod.rs:426-472`): 先按 feature id 匹配,且**双向**比较(因为实体顺序按 `aabb.min.x` 排序可能变化,`mod.rs:439-443`);feature id 为 `PackedFeatureId::UNKNOWN` 时退化为按 `anchor1`/`anchor2` 距离阈值匹配(双向,`mod.rs:450-469`)。命中后拷贝 `warm_start_normal_impulse` 与 `warm_start_tangent_impulse`(`mod.rs:444-446`)。
- 开关是 `NarrowPhaseConfig::match_contacts`,默认 `true`(`src/collision/narrow_phase/mod.rs:238-246`、`mod.rs:249-257`)。
- 特征 id 类型 `PackedFeatureId(pub u32)`(`src/collision/contact_types/feature_id.rs:11`),`UNKNOWN = 0`(`feature_id.rs:17`),把 vertex/edge/face 种类编码进高 2 位(`feature_id.rs:19-27`)。
- 每个接触点的 feature id 存在 `ContactPoint::feature_id1` / `feature_id2`(`src/collision/contact_types/mod.rs:656-659`)。

### 4.5 一次 `update_contacts` 的完整步骤(逐行)

`update_contacts::<H>`(`system_param.rs:440-833`):

1. 位图容量按 `contact_graph.edges.raw_edges().len()` 设定并清空(`452-463`);注释解释为何按**容量**而非活跃 pair 数: pair index 不稳定(`449-451`)。
2. `crate::utils::par_for_each(self.contact_graph.active_pairs_mut(), 64, |_i, contacts| {…})`(`480`),chunk 大小 64。`parallel` 下状态位写线程局部位图(`488-500`),否则写全局(`483-484`)。
3. 取两个 collider(`503-508`);`EnlargedAabb` 重叠复检 + `CollisionLayers` 复检(`511-516`)。任一失败 → 置 `DISJOINT_AABB` 并标记状态变化(`518-519`)。
4. 取两侧 body bundle(`523-528`),解包出 `is_static`、`collider_offset`、世界质心、线/角速度、body 级 friction/collision_margin/speculative_margin(`532-579`),并把这些静态性写进 flags `STATIC1`/`STATIC2`(`583-584`,注释说明是为了后面处理状态变化时不必再查 body)。
5. 判定是否生成约束: `is_disabled = body1_bundle.is_none() || body2_bundle.is_none() || collider1.is_sensor || collider2.is_sensor`(`587-590`);由 disabled 变为 enabled 时置 `STARTED_GENERATING_CONSTRAINTS` 并标记状态变化(`592-598`);随后 `GENERATE_CONSTRAINTS` 置为 `!is_disabled`(`600-602`)。
6. 组合摩擦/弹性: collider 自身优先,回退到 body,再回退到 `DefaultFriction`/`DefaultRestitution`,并用 `.combine(...)`(`606-629`)。
7. 余量: `collision_margin_sum = margin1 + margin2`(`636-644`);推测余量按 collider → body → `default_speculative_margin` 逐级回退(`652-661`)。
8. **有效推测余量**: 按各自 speculative margin 用 `lin_vel.clamp_length_max(margin * inv_delta_secs)` 限制速度(`674-679`),再取 `effective_speculative_margin = delta_secs * relative_linear_velocity.length()`(`683-684`)。
9. **最大接触距离**: `max_contact_distance = effective_speculative_margin.max(contact_tolerance) + collision_margin_sum`(`689-691`)—— 这个值就是传给 Parry 的 `prediction_distance`。
10. 调用 `contact_manifolds_with_context`(`706-715`),然后 `manifolds.retain_mut(...)`(`718-769`):
    - 写入 `friction` / `restitution`,
    - `#[cfg(feature="2d")] tangent_speed = 0.0` / `#[cfg(feature="3d")] tangent_velocity = Vector::ZERO`(`722-729`),
    - `retain_points_mut` 里把 `anchor1/anchor2` 平移到相对质心(`736-737`)、把 `collision_margin_sum` 加到 `penetration`(`740`)、计算 `normal_speed = relative_velocity.dot(normal)`(2D 用 `perp()`,3D 用 `cross()`,`743-752`),
    - 保留判据: `-point.penetration < effective_speculative_margin || (normal_speed * delta_secs - point.penetration) < effective_speculative_margin`(`754-759`),
    - 3D 下点数 > 4 时 `manifold.prune_points()`(`763-766`),
    - 空 manifold 丢弃(`768`)。
11. `touching = !contacts.manifolds.is_empty()`(`772`);若 `MODIFY_CONTACTS` 置位则调 `hooks.modify_contacts(contacts, &mut commands)`,返回 `false` 则清空 manifolds(`774-781`)。
12. 写 `TOUCHING` flag(`783`),然后 `match_contacts` 暖启动匹配(`789-798`)。
13. `manifold_count_change = manifolds.len() as i32 - old_manifolds.len() as i32`(`802-803`),用于给持久接触增删约束。
14. 状态迁移判定(`807-819`): `touching && !was_touching` → `STARTED_TOUCHING` + 标记;`!touching && was_touching` → `STOPPED_TOUCHING` + 标记;仅 manifold 数变化 → 标记。
15. `parallel` 下把线程局部位图 OR 进全局(`823-832`)。

### 4.6 串行状态处理: 为什么必须串行

`NarrowPhase::update` 在 `update_contacts` 之后**串行**遍历 `ContactStatusBits` 的置位(`system_param.rs:140-392`)。源码注释解释: 「iterating over set bits serially to maintain determinism」(`system_param.rs:140-143`),位遍历用 count-trailing-zeros 技巧(`144-147`)。

分支树:

| 分支 | 条件 | 动作 |
|---|---|---|
| 移除 | `contact_pair.aabbs_disjoint()`(`158`) | 若原为 `TOUCHING ∧ CONTACT_EVENTS` 写 `CollisionEnd`(`160-170`);从 `CollidingEntities` 移除(`173-177`);按 `constraint_handles` 数量 `pop_manifold`(`190-197`);从 island 解绑(`200-207`);`contact_graph.remove_edge_by_id`(`211`) |
| 开始接触 | `contact_pair.collision_started()`(`212`) | 若 `contact_edge.events_enabled()` 写 `CollisionStart`(`214-221`);加入 `CollidingEntities`(`224-228`);`debug_assert!(!manifolds.is_empty())`(`230-233`);置 `TOUCHING`、清 `STARTED_TOUCHING`(`235-238`);若 `generates_constraints()` 则对每个 manifold `push_manifold` 并 add island(`240-263`) |
| 结束接触 | `flags.contains(STOPPED_TOUCHING)`(`264-267`) | 写 `CollisionEnd`(`269-276`);移出 `CollidingEntities`(`279-283`);`debug_assert!(manifolds.is_empty())`(`285-288`);清 `TOUCHING`、清 `STOPPED_TOUCHING`(`290-293`);`pop_manifold` + remove island(`296-324`) |
| 开始生成约束 | `is_touching() && STARTED_GENERATING_CONSTRAINTS`(`325-329`) | 清 flag(`331-333`);`push_manifold` + add island(`336-356`) |
| manifold 变多 | `is_touching() && generates_constraints() && manifold_count_change > 0`(`357-360`) | `push_manifold` × N(`363-367`) |
| manifold 变少 | 同类且 `manifold_count_change < 0`(`368-371`) | `pop_manifold` × N(`374-386`) |

遍历中收集到的 sleeping island 会 `sort_unstable()` + `dedup()` 后通过 `commands.queue(WakeIslands(...))` 唤醒(`system_param.rs:394-402`)。

`Collisions` 是给用户的只读视图: `pub struct Collisions<'w> { contact_graph: ResMut<'w, ContactGraph> }`(`src/collision/contact_types/system_param.rs:52-56`),提供 `get` / `contains` / `contains_key` / `iter`(仅 touching,active + sleeping)/ `collisions_with` / `entities_colliding_with`(`system_param.rs:64-123`),`graph()` 可拿到完整 `ContactGraph`(`system_param.rs:64-66`)。

---

## 5. Contact types

### 5.1 `ContactGraph`: 三层存储

`pub struct ContactGraph`(`src/collision/contact_types/contact_graph.rs:76-99`)字段:

| 字段 | 类型 | 作用 | 行号 |
|---|---|---|---|
| `edges` | `ContactGraphInternal` = `StableUnGraph<Entity, ContactEdge>`(`contact_graph.rs:107`) | 无向图,节点=实体,边=`ContactEdge`,索引**稳定** | `81` |
| `active_pairs` | `Vec<ContactPair>`(crate-private) | 醒着的 dynamic/kinematic body 的 pair | `84` |
| `sleeping_pairs` | `Vec<ContactPair>`(crate-private) | 睡眠 dynamic body 的 pair | `87` |
| `pair_set` | `HashSet<PairKey>`(crate-private) | `PairKey` = 排序后的两个 entity index 打包成 `u64`,做 O(1) 去重 | `95` |
| `entity_to_node` | `SparseSecondaryEntityMap<NodeIndex>`(私有) | entity → 图节点索引 | `98` |

关键点: **`ContactEdge` 在 `edges` 里,`ContactPair` 在 `active_pairs`/`sleeping_pairs` 里,二者通过 `ContactEdge::pair_index` 关联**(`src/collision/contact_types/mod.rs:70-71`)。`ContactEdge` 持有 `ContactId`(稳定 `u32`,`mod.rs:24`),`ContactId::PLACEHOLDER = u32::MAX`(`mod.rs:30`)。

**为什么要拆成 edge + pair**? 源码给出的理由是「冷热分离」: `ContactEdgeFlags` 的定义处注释说,这些 flags 单独存放是为了在**只想知道哪些对在接触**(例如查询 touching contacts)时**不必去取 `ContactPair`**(`src/collision/contact_types/mod.rs:125-126`)。`ContactEdge` 只含 `id / collider1 / collider2 / body1 / body2 / pair_index / constraint_handles / island / flags`(`mod.rs:54-86`),很轻;`ContactPair` 含 `manifolds: Vec<ContactManifold>`(`mod.rs:169`)与 `manifold_count_change`(`mod.rs:175`),较重。

**为什么 graph 与 pair 分开**?

> **推论**(基于 `active_pairs`/`sleeping_pairs` 的物理含义与 `wake_entity_with`/`sleep_entity_with` 的实现): 图结构负责「拓扑查询」(某实体的邻居、某对是否存在、按实体迭代),`Vec` 负责「顺序扫描与并行处理」。narrow phase 要对**全部活跃 pair** 做 `par_for_each`(`system_param.rs:480`),直接扫 `&mut [ContactPair]` 才能拿到连续内存并被 `par_for_each` 切块;solver 则需要按 `ContactId` 随机访问(`src/dynamics/solver/plugin.rs:391-398`)。两者对访问模式的要求不同,所以维护了「同一份数据的两套索引」。`wake_entity_with` / `sleep_entity_with`(`contact_graph.rs:710-768` / `773-831`)专门负责在睡眠状态切换时用 `swap_remove` 在两个 `Vec` 之间搬移 pair 并**修正 `pair_index`**(`contact_graph.rs:757-766`、`819-829`)—— 这些修正代码的存在本身就是「冗余索引的代价」的证据。

`ContactGraph` 的公开查询 API: `get_edge`(`contact_graph.rs:124`)、`get_edge_by_id`(`142`)、`get`(`180`)、`get_by_id`(`191`)、`get_mut`(`202`)、`get_mut_by_id`(`224`)、`get_pair_by_edge`(`244`)、`get_manifold`(`269`)/`get_manifold_mut`(`277`)、`contains`(`290`)/`contains_key`(`301`)、`active_pairs`(`307`)/`sleeping_pairs`(`313`)/`*_mut`(`319`/`325`)、`iter_active`(`336`)/`iter_active_touching`(`345`)/`iter_active_mut`(`357`)/`iter_active_touching_mut`(`366`)、`iter_sleeping*`(`378`/`387`/`399`/`408`)、`contact_edges_with`(`415`)/`_mut`(`426`)、`contact_pairs_with`(`446`)、`entities_colliding_with`(`456`)、`add_edge`(`477`)/`add_edge_with`(`496`)/`add_edge_and_key_with`(`526`)、`remove_edge`(`581`)/`remove_edge_by_id`(`604`)、`remove_collider_with`(`646`)、`wake_entity_with`(`710`)、`sleep_entity_with`(`773`)、`clear`(`846`)。

注意模块文档的两条警告: 用户直接增删改 contact pair **不会**触发碰撞事件、不会唤醒实体、不做其他清理(`contact_graph.rs:69-74`);过滤/修改应使用 `CollisionHooks`(`contact_graph.rs:74`)。`clear()` 还**不会**清 `ConstraintGraph`(`contact_graph.rs:840-844`)。

### 5.2 `ContactPair` 与 flag

`pub struct ContactPair`(`src/collision/contact_types/mod.rs:155-178`): `contact_id: ContactId`、`collider1/collider2: Entity`、`body1/body2: Option<Entity>`、`manifolds: Vec<ContactManifold>`、`manifold_count_change: i16`(crate-private,`mod.rs:175`)、`flags: ContactPairFlags`。

`ContactPairFlags(u16)` 完整位表(`src/collision/contact_types/mod.rs:185-209`):

| flag | 位 | 含义 | 行号 |
|---|---|---|---|
| `TOUCHING` | 1<<0 | 形状在接触(含 sensor) | `190` |
| `DISJOINT_AABB` | 1<<1 | AABB 已不重叠(待移除) | `192` |
| `STARTED_TOUCHING` | 1<<2 | 本帧刚开始接触 | `194` |
| `STOPPED_TOUCHING` | 1<<3 | 本帧刚停止接触 | `196` |
| `GENERATE_CONSTRAINTS` | 1<<4 | 应生成接触约束 | `198` |
| `STARTED_GENERATING_CONSTRAINTS` | 1<<5 | 刚开始生成约束(sensor→普通 collider、collider 挂到 body 上) | `199-201` |
| `STATIC1` / `STATIC2` | 1<<6 / 1<<7 | 第一/第二个刚体是 static(缓存) | `203`/`205` |
| `MODIFY_CONTACTS` | 1<<8 | 需要跑 `modify_contacts` hook | `207` |

配套方法: `is_touching`(`mod.rs:281`)、`aabbs_disjoint`(`287`)、`collision_started`(`293`)、`collision_ended`(`299`)、`generates_constraints`(`307`)、`find_deepest_contact`(`318`)、以及冲量聚合 `total_normal_impulse`(`231`)/`total_normal_impulse_magnitude`(`243`)/`max_normal_impulse`(`255`)/`max_normal_impulse_magnitude`(`273`)。

`ContactEdgeFlags(u8)` 只有 3 位(`src/collision/contact_types/mod.rs:134-143`): `TOUCHING`(1<<0)、`SLEEPING`(1<<1)、`CONTACT_EVENTS`(1<<2);配套 `ContactEdge::is_touching`(`mod.rs:108`)、`is_sleeping`(`114`)、`events_enabled`(`120`)。

### 5.3 `ContactManifold` 与 `ContactPoint`

`ContactManifold`(`src/collision/contact_types/mod.rs:342-378`):
- `points`: **2D 下是 `arrayvec::ArrayVec<ContactPoint, 2>`**(`mod.rs:346-347`),3D 下是 `Vec<ContactPoint>`(`mod.rs:351-353`,带 TODO「Store a maximum of 4 points in 3D in an ArrayVec」);
- `normal: Vector`(世界空间单位法线,从第一个形状指向第二个,同一 manifold 内共享,`mod.rs:354-357`);
- `friction` / `restitution`(`mod.rs:358-361`);
- `tangent_speed`(2D,`mod.rs:367-368`)/ `tangent_velocity`(3D,`mod.rs:376-377`),用于传送带类效果。

方法: `new`(`mod.rs:386`)、`total_normal_impulse`(`404`)、`max_normal_impulse`(`412`)、`match_contacts`(`426`)、`prune_points`(仅 3D,`mod.rs:478`,注释说明算法取自 Jolt 的 `PruneContactPoints`,`mod.rs:479-480`;启发式是 `distance_to_com * penetration`,先取距质心最远的点、再取距其最远的点、再取线段两侧最远点组成凸四边形,`mod.rs:487-565`)、`find_deepest_contact`(`575`)、`retain_points_mut`(crate-private,`585`)。

`ContactPoint`(`src/collision/contact_types/mod.rs:603-660`): `anchor1`/`anchor2`(世界空间接触点,相对于各体质心,`mod.rs:604-607`)、`point`(世界空间接触点,两表面最近点的中点,`mod.rs:608-614`)、`penetration`(可为负 = 推测接触,`mod.rs:615-620`)、`normal_impulse`(`621-626`)、`normal_speed`(沿法线的相对速度,负值表示接近,`627-634`)、`warm_start_normal_impulse`(`635-639`)、`warm_start_tangent_impulse`(2D `Scalar` / 3D `Vector2`,`mod.rs:644-653`)、`feature_id1`/`feature_id2`(`654-659`)。方法: `new`(`669`)、`with_feature_ids`(`689`)、`flip`(`698`)、`flipped`(`709`)。

另有精简的 `SingleContact`(`src/collision/contact_types/mod.rs:731-742`): `local_point1/local_point2/local_normal1/local_normal2/penetration`,是 `contact()` 单点查询的返回类型,不含冲量与 feature id。

### 5.4 `ConstraintGraph`: solver 消费的那一层

`ConstraintGraph`(`src/dynamics/solver/constraint_graph.rs:129-132`)只含 `pub colors: Vec<GraphColor>`(`constraint_graph.rs:131`);`GraphColor` 含 `body_set`、`manifold_handles: Vec<ContactManifoldHandle>`、`contact_constraints: Vec<ContactConstraint>`(`constraint_graph.rs:76`、`149-155`)。文档说明: 每个颜色是一组可并行求解、无数据竞争的 body 与约束;每个 body 在一个颜色里最多出现一次;最后一个颜色 `COLOR_OVERFLOW_INDEX` 用于找不到颜色的约束,串行求解(`constraint_graph.rs:118-126`)。**每个 `ContactManifold` 被当作独立的一条边**,因为每个 manifold 有自己的 `ContactConstraint`(`constraint_graph.rs:120-124`)。

`ContactManifoldHandle { contact_id: ContactId, manifold_index: usize }`(`constraint_graph.rs:89-91`;`manifold_index` 注释说明它「可能随 contact pair 更新而变化」,`constraint_graph.rs:88`)。`ContactConstraintHandle { color_index: u8, local_index: usize }`(`constraint_graph.rs:108-113`)。`push_manifold(&mut self, contact_edge: &mut ContactEdge, contact_pair: &ContactPair)`(`constraint_graph.rs:163`)/ `pop_manifold(...)`(`constraint_graph.rs:245`)是 narrow phase 唯一的写入口。

solver 侧的消费: `prepare_contact_constraints`(`src/dynamics/solver/plugin.rs:363-448`)遍历 `constraint_graph.colors`,按 `handle.contact_id` 从 `contact_graph.get_by_id(...)` 取 pair(`plugin.rs:391-396`),再取 `contact_pair.manifolds[manifold_index]`(`plugin.rs:397-398`),跳过 `!contact_pair.generates_constraints()`(`plugin.rs:400-402`)与「两侧都非 dynamic」(`plugin.rs:419-422`),最后 `ContactConstraint::generate(body1_entity, body2_entity, body1, body2, contact_pair.contact_id, manifold, manifold_index, narrow_phase_config.match_contacts, &contact_softness)`(`plugin.rs:424-434`),非空则压入 `color.contact_constraints`(`plugin.rs:436-438`)。结果落在 `ContactConstraints(pub Vec<ContactConstraint>)`(`plugin.rs:352-354`)。求解循环顺序(PrepareSolverBodies → PrepareJoints → PrepareContactConstraints → 子步 WarmStart/SolveConstraints/Relax → Restitution → Finalize → StoreContactImpulses)见 `src/dynamics/solver/plugin.rs:53-64`;子步系统注册见 `plugin.rs:129`/`132`/`136`;`store_contact_impulses` 把冲量写回 `ContactGraph` 的 `contact.warm_start_normal_impulse` / `warm_start_tangent_impulse`(`plugin.rs:720-745`)。

---

## 6. Collision events

`src/collision/collision_events.rs`(297 行)只定义 3 个类型,没有专属 reader 类型。

### 6.1 类型

- `CollisionStart`(`collision_events.rs:169-189`): `#[derive(EntityEvent, Message, Clone, Copy, Debug, PartialEq)]`(第 169 行),字段 `collider1`(带 `#[event_target]`,第 175-176 行)、`collider2`(178)、`body1: Option<Entity>`(182)、`body2: Option<Entity>`(186)。
- `CollisionEnd`(`collision_events.rs:266-286`): 同构,`#[event_target] collider1`(272-273)、`collider2`(275)、`body1`(279)、`body2`(283)。
- `CollisionEventsEnabled`(`collision_events.rs:292-297`): 空 marker component。

旧名保留为 `#[deprecated]` 别名: `pub type OnCollisionStart = CollisionStart`(`collision_events.rs:191-193`),`pub type OnCollisionEnd = CollisionEnd`(`collision_events.rs:288-290`)。

### 6.2 两条发送路径

1. **Message**(`NarrowPhase::update` 内,**串行状态循环**里写):
   - `CollisionStart`: `src/collision/narrow_phase/system_param.rs:215-220`(条件 `contact_edge.events_enabled()`,`214`);
   - `CollisionEnd`: `system_param.rs:270-275`(条件同,`269`)与 `system_param.rs:164-169`(AABB 分离但原为 `TOUCHING ∧ CONTACT_EVENTS`,`160-163`)。
   - 另外实体被移除/禁用时由 `remove_collider` 补发 `CollisionEnd`(`src/collision/narrow_phase/mod.rs:424-435`)。
   - 写入方是 `update_narrow_phase` 持有的两个 `MessageWriter`(`src/collision/narrow_phase/mod.rs:276-277`)。
2. **EntityEvent**: 由 `trigger_collision_events`(`src/collision/narrow_phase/mod.rs:310-379`)在 `CollisionEventSystems` ∈ `PhysicsStepSystems::Finalize`(`mod.rs:138-141`、`174-181`)执行。它用独占 `&mut World`(注释: 避免为每个事件排命令,`mod.rs:311`),读上一步的 message(`mod.rs:321`、`347`),查询两个实体是否 `Has<CollisionEventsEnabled>`(`mod.rs:303`),然后**对每个启用的实体各发一次**(角色互换,`mod.rs:328-343`、`354-369`),最后 `world.trigger(event)` 逐个反射(`mod.rs:372-378`)。

顺序结论: message 在 narrow phase 期间就写好(**solver 之前**),而 EntityEvent 在 **Finalize(= solver 之后)** 触发。`CollisionStart` 的 doc 明确: 「triggered after the physics step in the `CollisionEventSystems` system set. At this point, the solver has already run and contact impulses have been updated.」(`collision_events.rs:163-168`)。

> **推论**: message 早于 solver、EntityEvent 晚于 solver,这意味着 **observer 里能读到本帧已求解的冲量**(通过 `Collisions`/`ContactGraph`),而读 message 时冲量还是上一帧的暖启动值。若引擎沿用此设计,需要在文档里强调这一差异,否则用户会在 message 里读到「看起来落后一帧」的冲量。

### 6.3 用户如何消费(取自源码文档的真实 API)

必须先在**至少一个**实体上加 `CollisionEventsEnabled`(`collision_events.rs:13`、`30`),否则既不写 message 也不触发 event(`collision_events.rs:45`、`65`)。

方式一: Message + `MessageReader<CollisionStart>`(`collision_events.rs:52-56`):

```rust
fn print_started_collisions(mut collision_reader: MessageReader<CollisionStart>) {
    for event in collision_reader.read() {
        println!("{} and {} started colliding", event.collider1, event.collider2);
    }
}
```

方式二: observer `On<CollisionStart>`,事件目标是 `collider1`(`collision_events.rs:78-99`、`124-145`):

```rust
commands.spawn((
    PressurePlate,
    Collider::cuboid(1.0, 0.1, 1.0),
    Sensor,
    CollisionEventsEnabled,   // collision_events.rs:85
))
.observe(on_player_stepped_on_plate);   // :87

fn on_player_stepped_on_plate(event: On<CollisionStart>, player_query: Query<&Player>) {
    let pressure_plate = event.collider1;   // 事件目标
    let other_entity = event.collider2;     // :94
    if player_query.contains(other_entity) { /* ... */ }
}
```

`CollisionEnd` 的注意事项(源码 doc,`collision_events.rs:259-261`): 如果其中一个 collider 被移除、或两者 AABB 不再重叠,`ContactPair` 也已被删除,因此**在 `Collisions` 里拿不到接触数据**。这解释了为何 `CollisionEnd` 的语义只能是「通知」而非「可查+通知」。

---

## 7. Collision hooks

### 7.1 trait 与两个方法的精确签名

`pub trait CollisionHooks: ReadOnlySystemParam + Send + Sync`(`src/collision/hooks.rs:147`,`#[expect(unused_variables)]`)。**只有两个方法**:

```rust
fn filter_pairs(&self, collider1: Entity, collider2: Entity, commands: &mut Commands) -> bool {
    true  // hooks.rs:164-166
}

fn modify_contacts(&self, contacts: &mut ContactPair, commands: &mut Commands) -> bool {
    true  // hooks.rs:187-189
}
```

默认实现都返回 `true`。为 `()` 提供了空实现 `impl CollisionHooks for () {}`(`src/collision/hooks.rs:193`),这就是 `BvhBroadPhasePlugin::<()>` / `NarrowPhasePlugin::<Collider, ()>` 能用默认 hooks 的原因(`src/collision/broad_phase/bvh_broad_phase.rs:31`、`src/collision/narrow_phase/mod.rs:59`)。

### 7.2 调用点(文件:行,精确)

| 方法 | 调用点 | 前置条件 |
|---|---|---|
| `filter_pairs` | `src/collision/broad_phase/bvh_broad_phase.rs:291` | 外层 `bvh_broad_phase.rs:286-290` 先检查 `proxy1.flags.union(proxy2.flags).contains(ColliderTreeProxyFlags::CUSTOM_FILTER)`;返回 `false` 则 `continue`(`292-294`) |
| `modify_contacts` | `src/collision/narrow_phase/system_param.rs:776` | 外层 `system_param.rs:774` 先检查 `touching && contacts.flags.contains(ContactPairFlags::MODIFY_CONTACTS)`;返回 `false` 则 `contacts.manifolds.clear()`(`778-780`) |

`CUSTOM_FILTER` / `MODIFY_CONTACTS` 这两个 proxy flag 由 `ColliderTreeProxyFlags::new(...)` 从 `ActiveCollisionHooks` 推导(`src/collider_tree/tree.rs:90-95`),而 `ActiveCollisionHooks` 变化时由 observer 更新 proxy flag(`src/collider_tree/update.rs:346-370`,Case 10)。

### 7.3 激活方式与限制

`ActiveCollisionHooks(u8)`(`src/collision/hooks.rs:227`)是 immutable component(`#[component(immutable)]`,`hooks.rs:225`),位定义 `FILTER_PAIRS = 0b0000_0001`(`hooks.rs:232`)、`MODIFY_CONTACTS = 0b0000_0010`(`hooks.rs:234`)。**默认不调用任何 hook**,只有至少一方带对应 flag 才调用(`hooks.rs:110-114`、`197`)。

注册方式(两种等价,`hooks.rs:105-106`):
- `PhysicsPlugins::default().with_collision_hooks::<MyHooks>()`(`hooks.rs:99`),内部实现是 `.disable::<BvhBroadPhasePlugin>().add(BvhBroadPhasePlugin::<H>::default())` 并同理替换 narrow phase(`src/lib.rs:834-841`);
- 或手动加 `BvhBroadPhasePlugin::<H>::default()` 与 `NarrowPhasePlugin::<C, H>::default()`。

明示的 caveat(`hooks.rs:137-145`):
- 每个 broad phase / narrow phase 只能有一套 hooks(`hooks.rs:139`);
- hook 的 system param 只允许**只读** ECS 访问,写操作要用传入的 `Commands`(`hooks.rs:140`);
- `parallel` 下 **命令执行顺序未定义**(`hooks.rs:141`);
- `filter_pairs` 与 `modify_contacts` 内部**不允许访问 `ContactGraph`**,否则 panic(`hooks.rs:142-145`)。

方法级注意事项(`hooks.rs:151-189`): `filter_pairs` 在 broad phase 调用、此时 `ContactPair` 尚未计算(`hooks.rs:151`);`modify_contacts` 在 narrow phase 调用、`ContactPair` 已计算但约束尚未生成(`hooks.rs:171-172`);`contacts` 里的冲量是**上一个 physics tick** 的(`hooks.rs:181`)。

> **推论**: `filter_pairs` 想读 `ContactGraph` 会 panic 的原因,是 broad phase 系统本身已持有 `ResMut<ContactGraph>`(`src/collision/broad_phase/bvh_broad_phase.rs:56`)且正在其中写边;同理 narrow phase 持有 `ResMut`(`src/collision/narrow_phase/system_param.rs:78`)。这是借用检查在运行期的表现,不是设计洁癖。自研引擎若用「先收集候选、后统一写入」的两段式,就不必有这条限制 —— 这是 Avian 为性能换来的用户侧约束。

---

## 8. Spatial queries

### 8.1 插件与模块门控

`src/spatial_query/mod.rs` 的模块声明带有 feature 门控(`mod.rs:153-158`): `query_filter` 与 `ray_caster` 无条件编译,`shape_caster` 与 `system_param` 需要 `parry-f32` 或 `parry-f64`(`mod.rs:155-158`),重导出同条件(`mod.rs:165-168`)。`spatial_query` 模块本身在 `lib.rs` 里**无条件编译**(`src/lib.rs:525`),`PhysicsPlugins` 也无条件加它(`src/lib.rs:785`),**不存在** `spatial-query` feature。但 feature 表把 `default-collider` 标为「Required for spatial queries」(`src/lib.rs:53`)。综合证据: `Collider` 类型本身就需要 `default-collider` + `parry-*`(`src/collision/collider/mod.rs:31-40`),而 `SpatialQuery` 直接查询 `&'static Collider`(`src/spatial_query/system_param.rs:61`)且整个类型被 `parry-*` 门控(`src/spatial_query/mod.rs:157-158`)—— 因此 **实际使用 `SpatialQuery` 需要 `parry-f32` 或 `parry-f64`(二者都会隐含开启 `default-collider`,`crates/avian3d/Cargo.toml:44-45`)**。一个可观察的后果: 无 parry 时 `RayCaster`/`RayHits`/`RayHitData` 仍能编译(`src/spatial_query/mod.rs:154`、`:164` 无 cfg),但没人会去填充 `RayHits` —— 负责 cast 的 `RayCaster::cast` 是 `#[cfg(all(feature = "default-collider", any(feature = "parry-f32", feature = "parry-f64")))]`(`ray_caster.rs:241-244`)。

`SpatialQueryPlugin { schedule }`(`mod.rs:178-180`),默认 `FixedPostUpdate`(`mod.rs:193-197`)。`build()`(`mod.rs:199-219`):
- 配置 `SpatialQuerySystems.after(TransformSystems::Propagate)`(`mod.rs:201-204`);
- 加系统链 `(update_ray_caster_positions, (update_shape_caster_positions, raycast, shapecast).chain()).chain().in_set(SpatialQuerySystems)`(`mod.rs:206-218`),其中 shape 相关三项带 `#[cfg(all(feature = "default-collider", any(feature="parry-f32", feature="parry-f64")))]`(`mod.rs:210-213`)。
- `finish()` 注册 `SpatialQueryDiagnostics`(`mod.rs:221-224`)。

系统实体: `update_ray_caster_positions`(`mod.rs:242-…`)、`raycast`(`mod.rs:397-415`)、`shapecast`(`mod.rs:417-432`)。`raycast` 遍历 `Query<(Entity, &mut RayCaster, &mut RayHits)>`,对 `enabled` 的调用 `ray_caster.cast(entity, &mut hits, &spatial_query)`,否则清空 `hits`(`mod.rs:404-411`)。

### 8.2 `SpatialQuery` 的并行设计: **没有预构建 pipeline,直接查 `Query` + `ColliderTrees`**

```rust
#[derive(SystemParam)]
pub struct SpatialQuery<'w, 's> {
    colliders: Query<'w, 's, (&'static Position, &'static Rotation, &'static Collider)>,  // :61
    aabbs: Query<'w, 's, &'static ColliderAabb>,                                          // :62
    collider_trees: Res<'w, ColliderTrees>,                                               // :63
}
```
证据: `src/spatial_query/system_param.rs:59-64`。

**不存在** `SpatialQueryPipeline` / `SpatialQueryPipelines` / 任何 pipeline 副本计数或索引类型(`grep -rn "SpatialQueryPipeline\|SpatialQueryPipelines\|PipelineIndex" src/` 无命中)。这不是疏漏,而是**有意的删除**,上游迁移指南把它写成了独立小节:「### Removed `SpatialQueryPipeline` — The `SpatialQueryPipeline` has been removed. Instead, use the `SpatialQuery` system parameter, or for lower level control, use the `ColliderTrees` resource directly.」(`migration-guides/0.5-to-0.6.md:32-35`);同一文档还提到改动的 PR 是「changed spatial queries to reuse the `ColliderTrees` used by the broad phase」(`migration-guides/0.5-to-0.6.md:29-30`)。加速结构直接复用 broad phase 的 `ColliderTrees`(`src/collider_tree/mod.rs:121-130`),**每次查询都遍历全部四棵树**(`self.collider_trees.iter_trees()`,`src/collider_tree/mod.rs:157-165`;调用点 `src/spatial_query/system_param.rs:189`、`365`、`538`、`752`、`902`、`1022`、`1119`、`1251`),而不是按树类型选树 —— 虽然 `tree_for_type` 存在(`src/collider_tree/mod.rs:135-153`),空间查询从不使用它。

> **推论**: 因为它是 `SystemParam`,Bevy 的调度器天然允许**每个系统各持一份**实例(读 `Res<ColliderTrees>` 与只读 `Query` 不冲突),并行度由系统级并行提供,不再需要早期版本那种「手工准备 N 份 pipeline 副本」的方案。代价有两条: ① `ColliderTreeOptimization::optimize_in_place = true` 时树在模拟步内不可用(`src/collider_tree/optimization.rs:41-51`),而默认是 `false`(`optimization.rs:59-63`)以保住这个可用性;② `SpatialQuery` 查询 `&Position`/`&Rotation`,因此**任何可变查询这两个组件的系统会与它产生调度冲突**,上游迁移指南明确建议把写操作推迟到之后(`migration-guides/0.5-to-0.6.md:37-38`)。另外旧 API `SpatialQuery::update_pipeline()` 也已删除,取而代之的是公开系统 `update_moved_collider_aabbs`(`migration-guides/0.5-to-0.6.md:40-42`;该系统定义在 `src/collider_tree/update.rs:839`)。

### 8.3 光线投射: 组件式 vs 直接 API

**组件式**:
- `pub struct RayCaster`(`src/spatial_query/ray_caster.rs:73`),`#[component(on_add = on_add_ray_caster)]`、`#[require(RayHits)]`(`ray_caster.rs:71-72`)。字段: `enabled: bool`(`ray_caster.rs:76`)、`origin: Vector`(局部,`80`)、私有 `global_origin`(`83`)、`direction: Dir`(局部,`88`)、私有 `global_direction`(`91`)、`max_hits: u32`(`98`,doc 警告超出上限会**漏掉部分命中**,`ray_caster.rs:92-97`)、`max_distance: Scalar`(`104`)、`solid: bool`(`111`,决定起点在形状内时是否返回距离 0)、`ignore_self: bool`(默认 `true`,`ray_caster.rs:114-115`)、`query_filter: SpatialQueryFilter`(`117`)。
- 构造/配置: `RayCaster::new(origin, direction)`(`ray_caster.rs:145`)、`from_ray(ray)`(`154`)、`with_origin`(`163`)、`with_direction`(`169`)、`with_solidness`(`179`)、`with_ignore_self`(`187`)、`with_max_distance`(`193`)、`with_max_hits`(`199`)、`with_query_filter`(`206`)、`enable`(`212`)、`disable`(`217`)、`global_origin`(`222`)、`global_direction`(`227`)、`get_point`(`285`)、`get_global_point`(`291`)。
- `pub struct RayHits(pub Vec<RayHitData>)`(`ray_caster.rs:342`),`iter_sorted()` 返回按距离排序的迭代器(`ray_caster.rs:348`)。
- `pub struct RayHitData { entity: Entity, distance: Scalar, normal: Vector }`(`ray_caster.rs:395-408`),实现 `MapEntities`(`ray_caster.rs:410-415`)。

**直接 API**(在系统里 `spatial_query: SpatialQuery` 直接调)。`SpatialQuery` 上共有 **17 个公开方法**,全部在 `impl SpatialQuery<'_, '_>`(`src/spatial_query/system_param.rs:66`)下;`Vector`/`Scalar`/`Dir` 是 crate 别名,`RotationValue` 在 2D 下是 `Scalar`、3D 下是 `Quaternion`(`src/lib.rs:540-541`):

| 方法 | 精确签名 | 行号 |
|---|---|---|
| `cast_ray` | `(&self, origin: Vector, direction: Dir, max_distance: Scalar, solid: bool, filter: &SpatialQueryFilter) -> Option<RayHitData>` | `system_param.rs:111-120`(内部转调 `cast_ray_predicate(..., &\|_\| true)`,见 `:119`) |
| `cast_ray_predicate` | `(&self, origin, direction, mut max_distance, solid, filter, predicate: &dyn Fn(Entity) -> bool) -> Option<RayHitData>` | `system_param.rs:176-225` |
| `ray_hits` | `(&self, origin, direction, max_distance, max_hits: u32, solid, filter) -> Vec<RayHitData>` | `system_param.rs:277-298` |
| `ray_hits_callback` | `(&self, origin, direction, max_distance, solid, filter, callback: impl FnMut(RayHitData) -> bool)` | `system_param.rs:354-395` |
| `cast_shape` | `(&self, shape: &Collider, origin, shape_rotation: RotationValue, direction, config: &ShapeCastConfig, filter) -> Option<ShapeHitData>` | `system_param.rs:446-464` |
| `cast_shape_predicate` | 同上 + `predicate: &dyn Fn(Entity) -> bool` | `system_param.rs:523-595` |
| `shape_hits` | `(&self, shape: &Collider, origin, shape_rotation, direction, max_hits: u32, config, filter) -> Vec<ShapeHitData>` | `system_param.rs:651-681` |
| `shape_hits_callback` | `(&self, shape: &Collider, origin, shape_rotation, direction, config, filter, callback: impl FnMut(ShapeHitData) -> bool)` | `system_param.rs:740-803` |
| `project_point` | `(&self, point: Vector, solid: bool, filter) -> Option<PointProjection>` | `system_param.rs:840-847` |
| `project_point_predicate` | `(&self, point, solid, filter, predicate: &dyn Fn(Entity) -> bool) -> Option<PointProjection>` | `system_param.rs:892-931` |
| `point_intersections` | `(&self, point: Vector, filter) -> Vec<Entity>` | `system_param.rs:964-973` |
| `point_intersections_callback` | `(&self, point, filter, callback: impl FnMut(Entity) -> bool)` | `system_param.rs:1016-1041` |
| `aabb_intersections_with_aabb` | `(&self, aabb: ColliderAabb) -> Vec<Entity>` —— **注意: 没有 filter 参数** | `system_param.rs:1069-1078` |
| `aabb_intersections_with_aabb_callback` | `(&self, aabb: ColliderAabb, callback: impl FnMut(Entity) -> bool)`,内部额外用紧的 `ColliderAabb` 复检一次 | `system_param.rs:1114-1134`(复检在 `:1122-1127`) |
| `shape_intersections` | `(&self, shape: &Collider, shape_position, shape_rotation, filter) -> Vec<Entity>` | `system_param.rs:1173-1194` |
| `shape_intersections_callback` | `(&self, shape, shape_position, shape_rotation, filter, callback)` | `system_param.rs:1241-1278` |

`PointProjection { entity: Entity, point: Vector, is_inside: bool }` 定义在 `system_param.rs:1286-1293`。**不存在**的方法: `intersections_with_point`、`intersections_with_shape`、`intersections_with_aabb`、`cast_ray_callback`、`cast_shape_callback`、`update_pipeline`(由上述穷举的 `pub fn` 清单确证)。

三个易踩的坑(均为源码事实):

- **`aabb_intersections_with_aabb` 不接 filter**(`system_param.rs:1069`),因此它**无法**遵守 layer mask 或排除实体 —— 唯一能过滤的入口是它的 `_callback` 版本(`:1114`),但那个版本也没有 filter 参数,过滤逻辑只能写在闭包里。
- **组件与直接 API 的默认值不同**: `RayCaster` 默认 `max_distance = Scalar::MAX`、`max_hits = u32::MAX`(`src/spatial_query/ray_caster.rs:128-129`);`ShapeCaster` 默认 `max_distance = Scalar::MAX`、`max_hits = 1`(`src/spatial_query/shape_caster.rs:178-179`)。
- **命中顺序无保证**: `ray_hits` / `shape_hits` 的 doc 明确不保证按距离排序(`system_param.rs:229-230`、`:600-601`),要排序请用 `RayHits::iter_sorted`(`ray_caster.rs:348`)/ `ShapeHits::iter_sorted`(`shape_caster.rs:534`)。

**两者区别**(源码 doc 明确,`system_param.rs:22-23`): 组件式适合「每帧都要投射一次」的简单场景,由 `SpatialQueryPlugin` 自动更新,结果直接躺在 `RayHits`/`ShapeHits` 里;直接 API 适合按需、带自定义条件(用 `*_predicate`)、或在任意系统里一次性查询。组件式每个实体每帧一次;直接 API 由调用者控制频率。

### 8.4 形状投射

- `pub struct ShapeCaster`(`src/spatial_query/shape_caster.rs:75`),`#[require(ShapeHits)]`(`shape_caster.rs:74`)。字段: `shape: Collider`(`81`)、`origin: Vector`(`87`)、`shape_rotation`(2D `Scalar` / 3D `Quaternion`,`shape_caster.rs:97`/`104`)、`direction: Dir`(`117`)、`max_hits: u32`(`123`)、`max_distance: Scalar`(`129`)、`target_distance: Scalar`(`138`)、`ignore_self: bool`(`152`)、`query_filter: SpatialQueryFilter`(`155`)。
- 方法: `new`(两个重载,`shape_caster.rs:192`/`208`)、`with_origin`(`224`)、`with_direction`(`230`)、`with_target_distance`(`242`)、`with_compute_contact_on_penetration`(`250`)、`with_ignore_origin_penetration`(`261`)、`with_ignore_self`(`269`)、`with_max_distance`(`275`)、`with_max_hits`(`281`)、`with_query_filter`(`288`)、`enable`(`294`)、`disable`(`299`)、`global_origin`(`304`)、`global_shape_rotation`(`310`/`316`)、`global_direction`(`321`)。
- `pub struct ShapeHits(pub Vec<ShapeHitData>)`(`shape_caster.rs:528`),`iter_sorted()`(`shape_caster.rs:534`);`ShapeHitData`(`shape_caster.rs:581`)。`ShapeCastConfig { max_distance, target_distance, … }`(`shape_caster.rs:412-426`)。

### 8.5 查询过滤器

`pub struct SpatialQueryFilter { pub mask: LayerMask, pub excluded_entities: EntityHashSet }`(`src/spatial_query/query_filter.rs:35-40`)。**没有** `QueryFilterFlags`,也**没有**「排除 sensor」的开关。

- `DEFAULT = { mask: LayerMask::ALL, excluded_entities: EntityHashSet::new() }`(`query_filter.rs:51-54`);
- `from_mask(mask)`(`query_filter.rs:61-66`)、`from_excluded_entities(entities)`(`query_filter.rs:71-76`)、`with_mask`(`83-86`)、`with_excluded_entities`(`89-92`);
- `test(&self, entity: Entity, layers: CollisionLayers) -> bool`(`query_filter.rs:97-101`): `!excluded_entities.contains(&entity) && CollisionLayers::new(LayerMask::ALL, self.mask).interacts_with(CollisionLayers::new(layers.memberships, LayerMask::ALL))`。

`LayerMask(pub u32)`(`src/collision/collider/layers.rs:86`)、`LayerMask::ALL = 0xffff_ffff`(`layers.rs:117`);`CollisionLayers { memberships, filters }`(`layers.rs:362-368`),`interacts_with`(`layers.rs:424`)。

> **推论**: 因为 `test` 只看 layers 与 excluded entities,**spatial query 默认会命中 sensor**(源码中 `src/spatial_query/` 全目录对 "sensor" 零命中,`grep -rn -i sensor src/spatial_query/` 无输出)。这与早期 Avian 的 `QueryFilterFlags::EXCLUDE_SENSORS` 行为不同 —— 迁移或对照时务必注意。若自研引擎要「不命中 trigger」,需要在 `test` 里自行加 `Has<Sensor>` 判断。

**一个重要副作用(源码事实)**: `RayCaster::cast` / `ShapeCaster::cast` 会在**每帧**修改组件自己的 filter —— 若 `ignore_self` 为真则把施法者实体 `insert` 进 `query_filter.excluded_entities`,否则 `remove`(`src/spatial_query/ray_caster.rs:251-255`;`shape_caster.rs:353-357`;调用来自 `raycast` 系统 `src/spatial_query/mod.rs:406`、`shapecast` `mod.rs:425`)。也就是说组件的 `query_filter` 在第一次 cast 之后**不再是用户设的那个值**。若要复用同一 filter 值,需要自己保存一份副本。

`RayCaster` 组件式用法与 filter 的组合示例见 `query_filter.rs:24-28`(`SpatialQueryFilter::from_mask(0b1011).with_excluded_entities([object])`,再 `RayCaster::default().with_query_filter(query_filter)`)。

---

## 9. Debug render 与 diagnostics(简述)

**Debug render**: `src/debug_render/`(3 个文件,共 1621 行)提供 `PhysicsDebugPlugin`(`src/debug_render/mod.rs:92` 的 `impl Plugin`),用 Bevy gizmos 可视化刚体坐标轴与质心、`ColliderAabb`、collider 线框、睡眠体配色、**`ContactPair` 接触点**、joint、`RayCaster`、`ShapeCaster`、simulation island、**collider tree 节点**,以及「只显示 debug 渲染」的可见性开关(完整清单见 `src/debug_render/mod.rs:30-43`)。全局配置走 `GizmoConfigStore` 里的 `PhysicsGizmos`(`debug_render/mod.rs:49`、初始化在 `mod.rs:94-98`),实体级走 `DebugRender` 组件(`debug_render/mod.rs:51`);各渲染系统都有 `run_if(...store.config::<PhysicsGizmos>().0.enabled)` 门控(例如 `mod.rs:135`)。整个模块由 `debug-plugin` feature 门控: `src/lib.rs:515-516`(`#[cfg(feature = "debug-plugin")] pub mod debug_render;`)、prelude 重导出同理(`lib.rs:534-535`);feature 表见 `src/lib.rs:65`。**注意 `debug-plugin` 在 avian3d 里是默认 feature**(`crates/avian3d/Cargo.toml:19`),且它拉入 `bevy/bevy_gizmos` 与 `bevy/bevy_render`(`crates/avian3d/Cargo.toml:30`)。

**Diagnostics**: `src/diagnostics/`(4 个受 feature 门控的文件 + `mod.rs`)提供两套: 默认注册的「插件自带 timer/counter」与需要额外加插件的总量/实体计数。物理插件各自实现 `PhysicsDiagnostics` trait 并在 `finish()` 里 `app.register_physics_diagnostics::<T>()`(`src/collision/narrow_phase/mod.rs:187-190`、`src/collision/broad_phase/mod.rs:193-196`、`src/collider_tree/mod.rs:96-99`、`src/spatial_query/mod.rs:221-224`)。碰撞相关的是 `CollisionDiagnostics { broad_phase: Duration, narrow_phase: Duration, contact_count: u32 }`(`src/collision/diagnostics.rs:10-18`),诊断路径 `avian/collision/broad_phase`、`avian/collision/update_contacts`、`avian/collision/contact_count`(`src/collision/diagnostics.rs:36-40`);collider tree 的是 `ColliderTreeDiagnostics { optimize, update }`(`src/collider_tree/diagnostics.rs:13-18`);spatial query 的是 `SpatialQueryDiagnostics { update_ray_casters, update_shape_casters }`(`src/spatial_query/diagnostics.rs:14-19`)。写进 Bevy 的 `DiagnosticsStore` 需要 `bevy_diagnostic` feature **且**手动加 `PhysicsDiagnosticsPlugin`(`src/diagnostics/mod.rs:9-10`、`mod.rs:104-108`);实时 UI 需要 `diagnostic_ui` feature **且**手动加 `PhysicsDiagnosticsUiPlugin`(`src/diagnostics/mod.rs:12-13`);两者在 `mod.rs:67-78` 都有 `#[cfg]` 门控。这两个 feature **都不是默认**(`src/lib.rs:63-64`)。

---

## 10. 里程碑: 最小可运行碰撞检测

目标: 从「colliders + rigid bodies 已存在」走到 (a) 候选 pair → (b) 真实接触 → (c) solver 可消费的表示 → (d) 用户可见的「这两个碰上了」信号。下面按依赖顺序给出施工清单,并标注该镜像哪个 Avian 文件。

### 10.1 顺序约束(硬性,附证)

**这个顺序不能换**:

1. **AABB 更新必须在 broad phase 之前**: `ColliderTreeSystems::UpdateAabbs.in_set(PhysicsStepSystems::BroadPhase).after(BroadPhaseSystems::First).before(BroadPhaseSystems::CollectCollisions)`(`src/collider_tree/mod.rs:78-84`)。broad phase 读的是树的 AABB,不先更新就永远是上一帧的包围盒。
2. **四个阶段必须在同一 schedule 里串联**: `First → BroadPhase → NarrowPhase → Solver → Sleeping → Finalize → Last`(`src/schedule/mod.rs:96-107`)。且 broad phase 内部 `First → CollectCollisions → Last`(`src/collision/broad_phase/mod.rs:181-190`),narrow phase 内部 `First → Update → Last`(`src/collision/narrow_phase/mod.rs:128-137`)。
3. **broad phase 必须在 narrow phase 之前**: narrow phase 的输入是 `ContactGraph.active_pairs`,而它只由 broad phase 写入(`src/collision/broad_phase/bvh_broad_phase.rs:189-203` → `src/collision/narrow_phase/system_param.rs:480`);narrow phase 模块文档第一段就写明「Before the narrow phase, the broad phase creates a contact pair in the `ContactGraph`」(narrow_phase/mod.rs:5-6)。
4. **narrow phase 必须在 solver 之前**: `ContactConstraint` 由 narrow phase 的 `push_manifold` 写入 `ConstraintGraph`(`src/collision/narrow_phase/system_param.rs:243`、`301`),solver 的 `prepare_contact_constraints` 才去读(`src/dynamics/solver/plugin.rs:363-398`);set 顺序证 `src/schedule/mod.rs:100-101`。
5. **事件触发必须在 solver 之后**(如果要做事件): `CollisionEventSystems.in_set(PhysicsStepSystems::Finalize)`(`src/collision/narrow_phase/mod.rs:138-141`),doc 说明运行于 solver 之后(`src/collision/narrow_phase/mod.rs:193-198`)。
6. **接触要用「扩张 AABB」而不是紧 AABB 做早期剔除**: narrow phase 用的是 `collider1.enlarged_aabb.intersects(collider2.enlarged_aabb)`(`src/collision/narrow_phase/system_param.rs:511`),broad phase 用的也是 `EnlargedAabb`(`src/collider_tree/update.rs:184`、`466`、`1007`)。`EnlargedAabb` 的余量是 `AABB_MARGIN = 0.05`(`update.rs:34`)。
7. **摩擦/弹性/余量要在调用 Parry 之前算好,接触点要在之后变换**: 顺序见 `system_param.rs:606-691`(参数)→ `:706`(Parry)→ `:718-769`(变换与裁剪)。

### 10.2 施工清单

**阶段 0 — 组件与 required components**

| 步骤 | 参考 Avian 文件位置 |
|---|---|
| 定义 `Collider`(或你自己的形状组件)并注册 required components: `Position`、`Rotation`、`ColliderAabb`、`EnlargedAabb`、`CollisionLayers` | `src/collision/collider/backend.rs:97-104` |
| 追加 required component `ColliderTreeProxyKey`(或你的 proxy 句柄) | `src/collider_tree/mod.rs:60-63` |
| `on_add` 时初始化 AABB(含 `contact_tolerance + collision_margin` 的增长) | `src/collider_tree/update.rs:75-107` |
| `ColliderAabb` 用 `min/max` 两向量 + `intersects` | `src/collision/collider/mod.rs:438-443`、`mod.rs:539-544`(3D) |
| `EnlargedAabb` 用 `update(&aabb, margin) -> bool` 做「是否真的越界」判定 | `src/collision/collider/mod.rs:579`、`mod.rs:592-602` |

**阶段 1 — (a) 候选 pairs**

| 步骤 | 参考 |
|---|---|
| 建 BVH(可直接用 `obvhs`),四棵树按 dynamic/kinematic/static/standalone 分离 | `src/collider_tree/mod.rs:121-130`、`src/collider_tree/tree.rs:26` |
| 一个系统批量更新 AABB: 线程局部位图记录移动代理 → OR 合并 → 串行写 BVH | `src/collider_tree/update.rs:620-643`、`update.rs:806-829`、`update.rs:936-978` |
| 维护「本步移动代理」列表 | `src/collider_tree/update.rs:512-518`(`Vec` + `HashSet` 双结构) |
| 配对查询: 遍历移动代理,在目标树里做 `aabb_traverse` | `src/collider_tree/traverse.rs:230-245`,`src/collision/broad_phase/bvh_broad_phase.rs:225` |
| 过滤: layers → 同 body → 已存在 → joint → 用户 hook | `src/collision/broad_phase/bvh_broad_phase.rs:256-295` |
| 落库: 建 `ContactEdge`(填 `body1/body2`)→ `ContactGraph::add_edge_with` 初始化 flags | `src/collision/broad_phase/bvh_broad_phase.rs:177-203`、`src/collision/contact_types/contact_graph.rs:526-570` |
| 去重键: `PairKey`(排序后的 entity index 打包 `u64`)+ `pair_set` | `src/collision/contact_types/contact_graph.rs:95`、`mod.rs:290-303` |
| **最省事的起步版**: 用暴力 O(n²) 替换,Bevy 的 `iter_combinations` + `aabb1.intersects(aabb2)` | `crates/avian3d/examples/custom_broad_phase.rs:76-117` |

**阶段 2 — (b) 真实接触**

| 步骤 | 参考 |
|---|---|
| narrow phase 系统遍历 `ContactGraph.active_pairs_mut()`(起步可串行) | `src/collision/narrow_phase/system_param.rs:480` |
| 重叠复检(扩张 AABB)+ layers 复检;失败则标 `DISJOINT_AABB` | `src/collision/narrow_phase/system_param.rs:511-519` |
| 解出 body 的静态性、质心、线/角速度、friction/restitution/margin | `src/collision/narrow_phase/system_param.rs:523-661` |
| 算「有效推测余量」与 `max_contact_distance` | `src/collision/narrow_phase/system_param.rs:666-691` |
| 调形状库生成 manifold(传 `prediction_distance = max_contact_distance`) | `src/collision/narrow_phase/system_param.rs:706-715`;后端间接层 `src/collision/collider/mod.rs:248-258`;Parry 适配 `src/collision/collider/parry/contact_query.rs:156-257` |
| 接触点后处理: 平移到相对质心、加 collision margin 到 penetration、算 `normal_speed` | `src/collision/narrow_phase/system_param.rs:736-752` |
| 裁剪: 超出推测余量且不接近的丢弃;3D 下 >4 点剪枝 | `src/collision/narrow_phase/system_param.rs:754-766`;`src/collision/contact_types/mod.rs:478-566` |
| 定 `TOUCHING` = manifolds 非空 | `src/collision/narrow_phase/system_param.rs:772-783` |
| 状态迁移置 `STARTED_TOUCHING` / `STOPPED_TOUCHING`,并标记「本步变化」 | `src/collision/narrow_phase/system_param.rs:807-819` |

**阶段 3 — (c) solver 可消费的表示**

| 步骤 | 参考 |
|---|---|
| 为每个 manifold 建一个 handle 放进按颜色分组的约束图 | `src/dynamics/solver/constraint_graph.rs:129-132`、`constraint_graph.rs:163`(`push_manifold`) |
| 只在 `TOUCHING` 或「预计即将接触」时建 handle;不接触时 `pop_manifold` | `src/collision/narrow_phase/system_param.rs:240-263`、`:296-324`、`:363-386` |
| 起步可以先不做 graph coloring,直接 `Vec<ContactConstraint>` 顺序求解 | `src/dynamics/solver/plugin.rs:352-354`(只有 colors 时才需要着色) |
| 生成约束: 从 handle 反查 pair 与 manifold,跳过 sensor / 双侧非 dynamic | `src/dynamics/solver/plugin.rs:389-434` |
| 暖启动数据要**留在接触点上**(`warm_start_normal_impulse` / `warm_start_tangent_impulse`),由 solver 事后写回 | `src/collision/contact_types/mod.rs:635-653`、`src/dynamics/solver/plugin.rs:720-745` |

**阶段 4 — (d) 用户可见的「碰上了」**

最小可用方案有 3 个层次,按实现成本排序:

1. **最省事**: `CollidingEntities(EntityHashSet)` 组件,由 narrow phase 在状态迁移时 `insert`/`remove`(`src/collision/narrow_phase/system_param.rs:406-417`、`420-431`;类型 `src/collision/collider/mod.rs:704`)。用户在任意系统里 `Query<(Entity, &CollidingEntities)>` 即可。用户必须**手动**加该组件(`src/collision/collider/mod.rs:671-673`)。
2. **只读查询**: 提供一个 `Collisions` 风格的 system param,`iter()` 只返回 touching 的 pair(`src/collision/contact_types/system_param.rs:94-100`),或 `collisions_with(entity)`(`system_param.rs:105-109`)。
3. **事件**: 加 `CollisionStart` / `CollisionEnd`(双形态: Message + EntityEvent),并在 `Finalize` 阶段用独占 world 反射(`src/collision/collision_events.rs:169/266`、`src/collision/narrow_phase/mod.rs:310-379`)。

> **推论**: 对「静止盒子 + 弹跳球」这个当前用例,(1) 的 `CollidingEntities` 已经足够,而且它不引入任何调度复杂度。事件是给「多对多、需要知道开始/结束时刻」的场景的,在这个用例里是纯开销。

### 10.3 可以延后什么,延后各会破坏什么

| 可延后项 | 延后后**不会**坏的东西 | 延后会**坏掉**的东西 |
|---|---|---|
| **Spatial queries / raycasting**(`src/spatial_query/`,§8) | 整个碰撞→求解流水线完全不依赖它: `ColliderTrees` 由 `ColliderTreePlugin` 独立维护(`src/collider_tree/mod.rs:57-94`),`SpatialQueryPlugin` 只加自己的三个系统(`src/spatial_query/mod.rs:206-218`) | ① 用 `CollisionHooks` 做高级过滤时**没有**可用的世界查询手段(hooks 文档明确把 spatial query 列为 hook 的常见用途,`src/collision/hooks.rs:26`);② 角色控制器、拾取(picking)、`RayCaster`/`ShapeCaster` 全部不可用;③ `ColliderTreeOptimization::optimize_in_place` 的取舍失去意义(`src/collider_tree/optimization.rs:41-51`) |
| **Collision events**(§6) | broad/narrow/solver 都不读事件 | 无法知道「何时开始/结束接触」。但注意 `CollidingEntities`(`src/collision/collider/mod.rs:704`)与 `Collisions`(`src/collision/contact_types/system_param.rs:53`)是**独立**于事件的,先做这两个不会白费 |
| **`CollisionHooks`**(§7) | 不注册 hook 时 `H = ()`,`impl CollisionHooks for ()` 是空实现(`src/collision/hooks.rs:193`),零额外成本 | ① 单向平台、传送带、非均匀摩擦等必须走 hook 的需求做不了;② `ActiveCollisionHooks` → proxy flag 的整条链路(`src/collider_tree/tree.rs:90-95`、`src/collider_tree/update.rs:346-370`)可以先不做,但以后要补 |
| **Contact graph 优化**(`active_pairs`/`sleeping_pairs` 分离 + `StableUnGraph` + `pair_set`)(§5.1) | 一个 `Vec<ContactPair>` + 线性查找就能跑通小场景 | ① 大场景下 `contains_key` 与「按实体查邻居」退化为 O(n);② 睡眠机制(`wake_entity_with`/`sleep_entity_with`,`src/collision/contact_types/contact_graph.rs:710-831`)没有落脚点;③ `ConstraintGraph` 的 `ContactId` 稳定寻址(`src/dynamics/solver/constraint_graph.rs:89-91`)需要稳定索引,若 `Vec` 用 `swap_remove` 就必须自己维护 `pair_index` 修正(对照 `contact_graph.rs:620-633`) |
| **增量 manifold 更新 / 持久 manifold**(§4.4) | Avian 0.7.0 **自己也没做**: 每帧 `manifolds.clear()` 后全量重算(`src/collision/collider/parry/contact_query.rs:186-187`),暖启动靠事后 `match_contacts`(`src/collision/narrow_phase/system_param.rs:789-798`) | 延后**不影响正确性**,只影响收敛速度: 没有暖启动时堆叠物体抖动更大、接触点更易跳变。若连 `match_contacts` 也延后,`match_contacts` 的开关默认 `true`(`src/collision/narrow_phase/mod.rs:246`),关掉后 `warm_start_*_impulse` 永远为 0 |
| **Debug render**(`debug-plugin`,`src/debug_render/`) | 纯可视化,不影响任何物理行为 | 调试碰撞问题会困难得多 —— 尤其 `debug_render_contacts`(`src/debug_render/mod.rs:316`)与 `debug_render_bvh`(`mod.rs:256`)直接可视化接触点与树的节点,是排查「pair 有了但 contact 没生成」这类问题的第一工具。**建议不要延后太久** |
| **Diagnostics**(`bevy_diagnostic` / `diagnostic_ui`,§9) | 计时与计数是旁路,不注册就不产生成本(`src/diagnostics/mod.rs:9-13`) | 失去 `CollisionDiagnostics.contact_count`(`src/collision/diagnostics.rs:16-17`)这类「一眼看出 pair 数量是否异常」的信号;`broad_phase` / `narrow_phase` 两个 timer(`src/collision/diagnostics.rs:12-14`)是定位性能瓶颈的最直接手段 |
| **`ConstraintGraph` 的 graph coloring** | 顺序求解在功能上完全等价(只是慢) | 无法并行求解;`COLOR_OVERFLOW_INDEX` 的溢出色串行路径(`src/dynamics/solver/constraint_graph.rs:118-126`)也失去意义 |
| **多 collider 后端(`AnyCollider::Context`)** | `src/collision/collider/mod.rs:128-259` 的间接层可以退化为直接调自己的形状库 | 一旦想要体素/自定义形状后端,需要重构成 trait —— 但这层重构本身很浅,先硬编码风险可控 |
| **`parallel` feature 的线程局部位图**(§3.3、§4.5) | 串行版本逻辑完全一样,只是慢(源码里 `#[cfg(not(feature = "parallel"))] let status_change_bits = &mut self.contact_status_bits;` 就是同一套逻辑,`src/collision/narrow_phase/system_param.rs:483-484`) | 大规模场景性能;并且会错过一个重要的架构约束: **状态变更处理必须串行以保证确定性**(`src/collision/narrow_phase/system_param.rs:140-143`) |

---

## 11. 本文件未验证/不确定

以下条目**未经源码确证**,标注以便后续核对(均不影响 §1–§10 的结论):

1. **未运行任何编译或测试**。所有「这个 API 长这样」的结论来自阅读源码,未通过 `cargo check` 验证;特征的组合(例如 `f64` + `parallel`)下的行为未逐一核对。
2. **`obvhs` 0.3 的内部行为未阅读**。`Bvh2` 的 `insert_primitive` / `remove_primitive` / `resize_node` / `reinsert_node` / `refit_all` / `aabb_traverse` / `ray_traverse*` / `squared_distance_traverse` / `point_traverse` 的具体算法(是否 SIMD、是否多线程、复杂度常数)只在 Avian 侧看到调用点(`src/collider_tree/tree.rs:165-352`、`src/collider_tree/traverse.rs`、`src/collider_tree/obvhs_ext.rs`),未查看 obvhs 源码。§2.2 关于复杂度的表述仅基于「BVH 遍历」这一常识性推断。
3. **`PlocBuilder` / `ReinsertionOptimizer` 的具体策略未核对**(`src/collider_tree/tree.rs:130-136` 只是字段声明)。`TreeOptimizationMode` 除 `Reinsert` 外的其余变体在 `src/collider_tree/optimization.rs:75-90` 附近有 doc,但本文未逐字读完该文件的后半部分(303 行中的 90-303 行未读)。
4. **`prune_points` 的几何正确性未验证**。本文只陈述「算法取自 Jolt 的 `PruneContactPoints`」(`src/collision/contact_types/mod.rs:479-480`)以及四个点的挑选顺序(`mod.rs:487-565`),未验证其输出是否真的构成凸多边形。
5. **`ConstraintGraph` 的着色算法未阅读**。`push_manifold` 的色选择逻辑在 `src/dynamics/solver/constraint_graph.rs:175` 之后,本文只读到 `:175`,未确认具体是贪心还是其他策略,也未确认 `GRAPH_COLOR_COUNT` 与 `COLOR_OVERFLOW_INDEX` 的常量值(both 在 `constraint_graph.rs:118-126` 的 doc 中被提及,定义行未读取)。
6. **`PhysicsIslands` 的交互未完整阅读**。`NarrowPhase::update` 里 `islands.add_contact` / `remove_contact` 的语义(`src/collision/narrow_phase/system_param.rs:249`、`311`)与 `WakeIslands` 命令(`system_param.rs:394-402`)的行为未展开;岛屿(sleeping)机制超出了本文范围。
7. **`speculative margin` 与 CCD 的完整语义未阅读**。本文只覆盖 narrow phase 里对它的使用(`system_param.rs:652-691`),CCD 侧的展开在 `src/dynamics/ccd/mod.rs`(由 `src/dynamics/ccd/mod.rs:33`、`:59`、`:281`、`:529` 可看出其存在),未阅读。
8. **spatial query 的部分 doc 示例与实际签名不一致**(源码事实,不是本文的不确定项,但会影响照抄示例的人): `src/spatial_query/system_param.rs:337` 的 `ray_hits_callback` 示例多传了一个 `max_hits` 参数(真实签名 `:354-362` 没有);`:506` 在 `cast_shape_predicate` 的文档里调用的是 `cast_shape` 并多传了 predicate;`:722` 的 `shape_hits_callback` 示例同样多传 `max_hits`(真实签名 `:740-749`);`:878` 把 `SpatialQueryFilter::default()` 按值传给 `project_point_predicate`,而真实签名要 `&SpatialQueryFilter`(`:896`)。这些是 `///` doctest 文本,照抄会编译失败。
9. **`SpatialQuery` 是否在 `ColliderDisabled` 实体上生效未确认**。查询里是 `Query<(&Position, &Rotation, &Collider)>`(`src/spatial_query/system_param.rs:61`),**没有** `Without<ColliderDisabled>` 过滤;但 Avian 用 Bevy 的 entity disabling,`ColliderDisabled` 只是 marker(见 `src/collision/collider/mod.rs:390-394` 的定义与 `src/collision/narrow_phase/system_param.rs:73` 的 `Without<ColliderDisabled>` 用法),实际是否被禁用取决于 `Disabled` 组件的传播语义,本文未核对。
10. **`ShapeHits` 文档自相矛盾**: `src/spatial_query/shape_caster.rs:493` 声称命中「按距离排序」,而同文件 `:500-501` 与 `src/spatial_query/system_param.rs:600-601` 又说顺序不保证。本文以「不保证」为准(有 `iter_sorted` 作为正确做法),但未逐行核对 `shape_hits` 的实现是否恰好有序。
11. **Collision events 在 `parallel` feature 下的确定性未验证**。`update_contacts` 并行写线程局部位图后再 OR(`src/collision/narrow_phase/system_param.rs:823-832`),`block` 顺序应保证遍历顺序一致,但本文未验证 `or` 的实现是否影响后续串行循环的确定性。
12. **任务书要求的部分细节在 v0.7.0 不存在**,已在 §0 列表中逐条给出替代物;若研究路线图的其他文档按任务书原名描述 v0.7.0,需要以本文 §0 为准更正。
13. **`RayCaster::cast` / `ShapeCaster::cast` 修改组件内 `query_filter` 这一副作用**是否在其他地方(例如 `picking`、`character_controller`)被有意依赖,本文未追查。
