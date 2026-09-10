# Wayfinder #151 — physics/rapier 事实清点(生命周期、静态 collider 与步进 dt 语义)

- 关联 issue: [WhitePetal/KairosEngine#151](https://github.com/WhitePetal/KairosEngine/issues/151)。AFK 研究票: 只做事实清点,**不做 API 设计、不做实现决策**。产出供决策票 #152(API 形态)、#153(系统集与同步)、#154(despawn 清理)引用。
- 基准: 分支 `bevy_fork` @ `cbd87ac`(即 issue 描述中的 `7f8aaa9` 之后新增的 `review: #149` 提交,本清点以当前 HEAD 工作树为准)。工作树仅 `KairosEngine/Library/asset_registry.toml` 有未提交修改(与本主题无关,未引用)。
- 路径约定: 与 #140 文档一致 —— engine 侧路径均相对于 workspace 根(`kairos_engine/`、`kairos_ecs/`、`kairos_time/`、`docs/` 所在层);行号基于上述 commit。
- 第三方源码约定(下称注册表): `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`。锁定的版本(见 `Cargo.lock`): `rapier3d 0.33.0`(L4151-4153)、`parry3d 0.28.0`(L3581-3583)。rapier 引用写作 `<R3>/…` = 注册表 `rapier3d-0.33.0/src/…`;parry 写作 `<P3>/…` = `parry3d-0.28.0/src/…`。
- 方法: rapier3d/parry3d 事实全部直接阅读注册表源码逐条核对(每一条都带文件+行号);engine 侧事实用 `grep`/`read_file` 静态核对。**未运行 `cargo build/test`** —— 本文所有行为判断基于源码阅读,不含运行验证。

---

## 0. 读本文前需要的 engine 现状(对照基线)

- `PhysicsEngine` 拥有全部 rapier 集合与管线字段(私有): `rigid_body_set`、`collider_set`、`gravity: float3`、`integration_parameters`、`island_manager`、`broad_phase`、`narrow_phase`、两个 joint set、`ccd_solver`、`physics_hooks: ()`、`event_handler: ()`、`physics_pipeline`、`accumulator: f32`(`kairos_engine/src/physics.rs` L20-35)。
- `Engine::new` 创建它(`kairos_editor.rs` L37);编辑器启动时 `KairosGame::new(&mut engine)`(`kairos_editor.rs` L83)会把 demo 的 plane collider 与 ball 刚体插入物理集合(`kairos_game.rs` L228-246)。
- 但 `PhysicsEngine::update` 目前**没有任何调用者**: `KairosGame::update` 里物理系统块整体被注释(`kairos_game.rs` L291-300),即 rapier 集合在启动后被填充、从不 `step`。
- 引擎每帧走 `KairosEngine::update` → `Engine::update`(`kairos_editor.rs` L98-102、L65-70): 先跑 `Main` 调度(含 `RunFixedMainLoop` 固定步驱动,见 Q5),再 `game.update`。
- `RigidBody`/`Collider` 尚不是 ECS `Component`(`// impl Component` 被注释,`physics/rigid_body.rs` L15-16、`physics/collider.rs` L14-15),两者只持有一个 rapier handle(`physics/rigid_body.rs` L12-14、`physics/collider.rs` L11-13)。step 后写回 `LocalTransform` 的同步块是注释 TODO(`physics.rs` L101-110)。
- rapier3d 0.33 的数学类型是 glam 系: `<P3>/math/mod.rs` L27-42 从 `glamx` 再导出向量/旋转/位姿类型,dim3 下 `Vector = Vec3`(L67)、`Rotation = Rot3`(L97)、`Real = f32`(f32 feature,L13)。engine 侧直接读字段/构造(`physics.rs` L116-125:`Vector3::new(x,y,z)`、读 `v.x`、读 `r.x/r.y/r.z/r.w`),与 #140 文档 C 节的记录一致。

---

## 1. 静态 collider 语义:`ColliderSet::insert` 不带 parent 时,它到底是什么

### 1.1 rapier 0.33 的事实:standalone collider,不创建任何 rigid body

- `<R3>/geometry/collider_set.rs` L163-180 `insert` 的文档与实现:
  - 文档原话(英文): "Adds a standalone collider (not attached to any body)… Standalone colliders are useful for sensors or static collision geometry that doesn't need a body."(L163-166)
  - 实现强制 `coll.parent = None`(L172),**不触碰 `RigidBodySet`、不创建任何隐式刚体**;随后把新 collider 推入 `modified_colliders`(L177-178)。
- 所以「`ColliderSet::insert(collider)` 单独插入」的真相是:**存在一个 parent 为 `None` 的 collider(0.33 称为 standalone),RigidBodySet 里没有对应 body**。它不像某些引擎版本那样自动挂到隐式 fixed body 上 —— 0.33 没有隐式刚体这层。
- collider 的世界位姿就存在自己身上:`Collider` 结构体字段 `pos: ColliderPosition`(`<R3>/geometry/collider.rs` L51-64),`ColliderPosition(pub Pose)`(`<R3>/geometry/collider_components.rs` L154-198)。standalone 没有 parent 来推导位姿,它的 `pos` 就是世界位姿,且**永远不会被任何 body 重算**(只有"有 parent"的 collider 才会每步由 parent 位姿覆盖,见 1.3)。
- 仿真中 standalone 被当作固定几何处理,证据:
  - 窄相计算接触时,parent 缺失的 collider 其"刚体类型"默认为 `Fixed`:`rb_type1 = rb1.map(|rb| rb.body_type).unwrap_or(RigidBodyType::Fixed)`(`<R3>/geometry/narrow_phase.rs` L848-852;交叉检测同样处理,L742-751)。
  - 同 body 自碰撞忽略分支只对"两者都有同一 parent"生效(`rb_handle1 == rb_handle2 && co1.parent.is_some()`,narrow_phase.rs L842-846 接触 / L736-740 交叉);两个都无 parent 的 collider 不受此限制(静态对静态本就不产生效果)。
  - 场景查询里,`QueryFilterFlags::EXCLUDE_FIXED` 对 parentless collider 直接排除(`<R3>/pipeline/query_pipeline.rs` L611-613),与 fixed parent 的处理并列(L615-621)。注意一个微妙差异:`ONLY_FIXED`(= EXCLUDE_DYNAMIC|EXCLUDE_KINEMATIC,L590-591)的文档明说**排除 standalone collider** —— 即查询语义里 standalone 与"显式 fixed body + collider"并不完全等价(见 1.4 差异表)。
- 位姿写入 API(collider 级):`Collider::set_translation` / `set_rotation` / `set_position` 直接改 `pos.0` 并置 `ColliderChanges::POSITION`(`<R3>/geometry/collider.rs` L314-337);文档明说这些方法服务于 standalone("This directly sets world-space position… For attached colliders, modify the parent body's position instead",L314-317)。collider 写入**没有 wake 参数**(唤醒概念只属于刚体,见 Q3)。
- 写入是否被仿真"看见": 经 `collider_set[handle]`(IndexMut)访问会自动登记 modified(`<R3>/geometry/collider_set.rs` L470-475);下一次 `step` 开头取走 modified 并更新 broad-phase AABB(`<R3>/pipeline/physics_pipeline.rs` L524-532、L141-148;最终 broad-phase pass L744-777)。

### 1.2 demo plane(engine 现状)正好是这条路径

- `kairos_game.rs` L228-234: plane 用 `Collider::box_collider(...)`(包装实现在 `physics/collider.rs` L45-50,内部就是 `collider_set.insert(collider)` L47)构造 standalone,然后 `plane_collider.set_position(...)`(L234;包装 `set_position` = `collider_set[handle].set_translation(...)`,`physics/collider.rs` L58-60)**一次性写入位姿**,此后不再写。与 issue 描述的 "needs one pose write at setup, then stays static" 完全一致。
- 对照 demo 的 ball:`RigidBody::with_sphere_collider_with_material`(`physics/rigid_body.rs` L34-53)先 `rigid_body_set.insert`(L40)再 `collider_set.insert_with_parent`(L44-48)挂到 dynamic body;pose 写入走刚体 `set_translation(.., wake=false)`(`physics/rigid_body.rs` L55-58,即 `rigid_body.rs` L974-990 的 teleport 语义,见 Q3)。

### 1.3 有 parent 的 collider,位姿不归自己管

- 附着的 collider 其世界位姿每步由 parent 刚体位姿推导:
  - 用户改了刚体位姿 → step 开头 user-changes 阶段把该 body 的所有 collider `pos` 重算为 `body 位姿`(经 `pos_wrt_parent`)(`<R3>/pipeline/user_changes.rs` L82-87;reparent 场景见 `<R3>/pipeline/user_changes.rs` L19-25:`co.pos = ColliderPosition(parent_rb.pos.position * co_parent.pos_wrt_parent)`);
  - 每步积分后 `advance_to_final_positions` 对**所有活跃(awake)body** 提交新位姿并同步 collider(`<R3>/pipeline/physics_pipeline.rs` L396-410)。
- 推论(纯事实): 对附着 collider 调 `set_translation` 只产生"一步之内"的临时效果,随后会被 parent 覆盖;engine wrapper `Collider::set_position` 目前只被 standalone(plane)使用,无此问题。

### 1.4 standalone collider vs 「显式 `RigidBodyBuilder::fixed()` + `insert_with_parent`」

| 维度 | standalone(`insert`) | 显式 fixed body + `insert_with_parent` | 出处 |
|---|---|---|---|
| RigidBodySet 条目 | 无 | 有(fixed body;`is_fixed()`,`<R3>/dynamics/rigid_body.rs` L684-686) | collider_set.rs L167-180 / L200-240 |
| 窄相碰撞类型 | 按 `Fixed` 处理 | 真 `Fixed` | narrow_phase.rs L848-852 |
| 位姿来源 | 自身 `pos`(用户写) | body 位姿 × `pos_wrt_parent` | 见 1.1 / 1.3 |
| 场景查询 `EXCLUDE_FIXED` | 排除 | 排除 | query_pipeline.rs L611 / L618 |
| 场景查询 `ONLY_FIXED` | **排除**(无 parent 不满足 fixed 分支) | 包含 | query_pipeline.rs L590-591 + L615-621 |
| 参与 joint | 不能(无 body handle) | 能 | — |
| 睡眠/唤醒概念 | 无(body 级概念) | body 从不进 island(fixed,见 Q3 3.2) | manager.rs L173-175 |
| 重力/速度等 body 级属性 | 无 | 有但 fixed 下无效果(`set_linvel` 对 Fixed no-op,rigid_body.rs L900) | rigid_body.rs L891-903 |

### 对本 map 设计票的含义(#152/#153/#154 必须尊重的事实)

- **#152(API 形态)**: 一张静态地图既可以用"standalone collider 数组"(现 demo 的做法),也可以用"fixed body + collider"实现 —— 两者在碰撞、`EXCLUDE_FIXED` 查询上等价,但在 **`ONLY_FIXED` 场景查询、能否参与 joint、是否存在可引用 handle** 上不等价;选择会改变未来地面交互(如行走面、传送带)的能力边界。
- **#153(系统集与同步)**: 静态几何的位姿写入是"setup 一次写、之后别写"型:rapier 不会自己移动 standalone/fixed;每帧重复写同一值会每帧把 collider 标成 modified(见 Q3 3.3 的成本事实)。push 方向只能写 body(对有 parent 的 collider 写 collider 会被覆盖)。
- **#154(despawn 清理)**: standalone collider 没有 parent,移除它就是 `ColliderSet::remove` 一条路;fixed body 路线则还牵涉 body 的移除(见 Q2)。

---

## 2. Handle 生命周期 / remove:body 与 collider 的移除、级联与分离

### 2.1 `RigidBodySet::remove`(rapier 0.33 签名比旧版多参数)

- 签名:`remove(&mut self, handle, islands: &mut IslandManager, colliders: &mut ColliderSet, impulse_joints, multibody_joints, remove_attached_colliders: bool) -> Option<RigidBody>`(`<R3>/dynamics/rigid_body_set.rs` L200-209)。实现(L210-238):
  1. 从 slot-map 移除 body(L210);
  2. `islands.rigid_body_removed_or_disabled(handle, &rb.ids, self)` 维护 island 表(L214;实现 `<R3>/dynamics/island_manager/manager.rs` L59-100: 从 island 的 body 列表 swap_remove、必要时删除空 island、把被 swap 的 body 的 `active_set_id` 重映射);
  3. **子 collider 处理取决于 `remove_attached_colliders`**:
     - `true` → 遍历该 body 的 collider 列表逐个 `colliders.remove(collider, islands, self, false)`(L219-222),即**自动级联删除**(collider 从 ColliderSet 消失,返回值被丢弃);
     - `false` → 对每个 collider 调 `colliders.set_parent(co_handle, None, self)`(L223-229),即**脱离成 standalone 存活**;
  4. 删除该 body 相关的所有 impulse/multibody joints(L234-235)。
- 结论: 不存在"子 collider 悬空或泄漏"状态 —— body 被删时其 collider 要么被删、要么被显式转成 standalone;`collider.parent` 永远指向存在的 body(除非用户绕过 remove 直接清空集合)。

### 2.2 `ColliderSet::remove`

- 签名:`remove(&mut self, handle, islands: &mut IslandManager, bodies: &mut RigidBodySet, wake_up: bool) -> Option<Collider>`(`<R3>/geometry/collider_set.rs` L335-341)。行为(L342-367):
  - 从集合删除并返回 collider;
  - 若有 parent,在 parent body 的 collider 列表里摘除(`remove_collider_internal`,L349-353);
  - `wake_up == true` 时唤醒该 parent(L355-357;对 fixed parent 无效,见 Q3);
  - 发布到 `removed_colliders` 供下一次 step 消费(L364)。
- body↔collider 的双向链接都在 rapier 内部维护:`RigidBody` 存 collider handle 列表(`add_collider_internal`/`remove_collider_internal`,`<R3>/dynamics/rigid_body.rs` L750-779),collider 存 `ColliderParent { handle, pos_wrt_parent }`(`<R3>/geometry/collider_components.rs` L144-152)。

### 2.3 重挂/脱挂 API:`ColliderSet::set_parent`

- 签名:`set_parent(&mut self, handle, new_parent_handle: Option<RigidBodyHandle>, bodies: &mut RigidBodySet)`(`<R3>/geometry/collider_set.rs` L263-307)。
  - 同 parent 直接返回(L271-273);否则置 `ColliderChanges::PARENT`,先旧 parent 摘除(L277-281)再按目标处理:
  - `Some(h)`: 若 collider 此前有 parent,只换 handle;若此前是 standalone,`pos_wrt_parent` 取 **`Pose::IDENTITY`**(L288-291),然后 `add_collider_internal`(L294-302) —— 即 **standalone 用 `set_parent` 挂到 body 时,相对位姿按 identity,不会保留它当前的世界位置**;
  - `None`: `collider.parent = None`(L304) —— 脱挂后 collider 变 standalone;**它当时的 `pos`(世界位姿缓存)原样保留**,即"停在原地"。
- 对照 `insert_with_parent`: 首次挂接时 `pos_wrt_parent = coll.pos.0`(即把当前(世界)位姿当作相对位姿,L211-218),行为与 `set_parent` 的 attach 分支不同 —— 这是两条挂接路径的已知差异,值得 map 代码注意。
- reparent 后的第一次 step 会把 collider `pos` 重算为 `parent.pos × pos_wrt_parent`(`<R3>/pipeline/user_changes.rs` L19-25),保证宽相/窄相一致。

### 2.4 handle 的安全性(ABA)

- 两类 handle 都是 arena `(index, generation)`:`RigidBodyHandle` / `ColliderHandle`(`<R3>/dynamics/rigid_body_handle.rs`、`<R3>/geometry/collider_handle.rs`,全文;`into_raw_parts`/`from_raw_parts`),删除后槽位可被复用但 generation 不同。
- 安全读:`set.contains(h)` / `set.get(h) -> Option`(`rigid_body_set.rs` L130-132/L274-276;`collider_set.rs` L159-161/L393-395);`Index`/`IndexMut` 对无效/世代不符的 handle 是 panic 语义(不做返回,如 `rigid_body_set.rs` L429-452)。

### 2.5 engine 现状

- wrapper 层只有构造(`new` 风格静态方法)与 `set_position`,**没有任何 remove/detach/reparent 包装**(`physics/rigid_body.rs`、`physics/collider.rs` 全文可查);Component 未 impl(`rigid_body.rs` L15-16、`collider.rs` L14-15),因此**不存在"ECS despawn 时自动清理物理对象"的机制** —— 这正是 #154 的空白区。

### 对本 map 设计票的含义(#152/#153/#154 必须尊重的事实)

- **#154(despawn 清理)**: 若实体持 body+collider,despawn 侧需要显式调用 `RigidBodySet::remove(…, remove_attached_colliders: bool)` 之类的包装;`true`/`false` 对应两种截然不同的语义(级联删除 vs 变 standalone 留下静态碰撞体),决策票需要把"离开世界后碰撞是否消失"定下来。**collider 先删、body 后删、或反过来,顺序不影响一致性**(body.remove 会处理剩余 collider;collider.remove 只摘除自己)。
- **#152(API 形态)**: `RigidBodySet::remove` 与 `ColliderSet::remove` 都需要连带传 `islands`/`bodies`/joint set(borrow 结构复杂,自研 wrapper 的签名要按这个形态设计);handle 带 generation,despawn 后旧 handle 必须靠 `get`/`contains` 判活,不能直接索引。
- **#153(系统集与同步)**: reparent/detach 存在两处不对称(standalone→attach 用 identity 相对位姿;detach 保留当前位置),如果同步系统要支持"碰撞体挪到别的实体",必须先写准这一点。

---

## 3. Pose 写 API:`set_translation`/`set_rotation` 的 wake 语义,与每帧同步对睡眠/island 的影响

### 3.1 刚体位姿写 API 的行为(全部 `RigidBody` 方法,`<R3>/dynamics/rigid_body.rs`)

| 方法 | 行为 | no-op 条件 | wake 语义 | 出处 |
|---|---|---|---|---|
| `set_translation(v, wake_up)` | 同时写 `pos.position` 与 `pos.next_position`(即瞬移/teleport,含下一帧目标位);置 `RigidBodyChanges::POSITION`;重算世界质量属性 | 新值与当前 `position`/`next_position` 都相等 → 什么都不做 | `wake_up == true` 且 `is_dynamic_or_kinematic()` → `wake_up(true)`;**fixed 刚体永远不 wake** | L974-990 |
| `set_rotation(r, wake_up)` | 同上(旋转) | 同上 | 同上 | L1002-1016 |
| `set_position(pose, wake_up)` | 平移+旋转合体版 | 同上 | 同上 | L1024-1038 |
| `set_next_kinematic_translation/rotation/position` | 只写 `pos.next_position`,由引擎在下一步算所需速度(`interpolate_kinematic_velocities` 用 `inv_dt`,`physics_pipeline.rs` L412-438) | 目标与当前位置相同 | 不同则内部 `wake_up(true)`;**只对 KinematicPositionBased 有效**(其它 body 类型下是 no-op) | L1044-1079 |
| `set_linvel` / `set_angvel` | 直接设速度 | 相同值 | `wake_up` 参数为 true 时 wake;**对 Fixed / KinematicPositionBased 是 no-op** | L891-940 |

- 顺带: teleport 系列**不清零速度**(`set_translation` 不碰 `vels`);`RigidBody::sleep()` 才会清速度(`rigid_body.rs` L790-793)。想"搬完就停"必须另行 `set_linvel(ZERO, …)`。
- engine 现状: wrapper `RigidBody::set_position` 用 `set_translation(.., wake_up=false)`(`physics/rigid_body.rs` L55-58)。

### 3.2 wake 的真实时间线(与 island 管理的关系)

- `wake_up` 参数**不直接改 island**:`set_translation(wake=true)` 只把 body 上的 `activation` 翻醒并(若此前在睡)置 `RigidBodyChanges::SLEEP`(`rigid_body.rs` L802-808;`activation.wake_up(strong)`,`<R3>/dynamics/rigid_body_components.rs` L1282-1293)。
- island 级唤醒发生在**下一次 `pipeline.step` 的 user-changes 阶段**: `handle_user_changes_to_rigid_bodies` 对每个 modified body 调 `islands.rigid_body_updated`(`<R3>/pipeline/user_changes.rs` L71-77),后者**只在 changes 含 SLEEP 或 TYPE、且 body 未处于手动睡眠**时才 `islands.wake_up(...)`(`<R3>/dynamics/island_manager/manager.rs` L210-219)。`islands.wake_up` 再把它所在 island 放回 awake 列表并逐个唤醒(`<R3>/dynamics/island_manager/sleep.rs` L31-46、L109-122)。即:**唤醒是"下一次 step 开头生效",不是调用瞬间生效**。
- 睡眠 body 用 `wake=false` 瞬移: 不会醒;位置数据照写,collider 世界位姿与 broad-phase AABB 会同步(见 3.3),但该 island 不参与求解,窄相接触不重算 —— 物理状态与几何位置可能不一致(纯事实:rapier 不阻止这种用法,但不保证接触正确)。
- fixed body 从不进 island(`rigid_body_updated` 对 fixed 早退,manager.rs L173-175),也从不被 `islands.wake_up` 唤醒(sleep.rs L33-34 跳过 fixed)—— 所以对固定地面反复写位姿**不会产生任何 island 活动**。
- 一个不对称事实: 移动一个 collider(标成 modified)后,窄相 `handle_user_changes_on_colliders` 会唤醒与它接触的其它 body 的 parent(`<R3>/geometry/narrow_phase.rs` L449-470);standalone collider 自己无 parent,只走"唤醒对方"分支(L459-469)。即把静态几何挪到睡眠的动态物体上,会把后者唤醒。

### 3.3 每帧 push/pull 与睡眠/island 的相互作用(为 baseline b1 供事实)

- **push(engine → rapier)每帧重复写同一值 = 廉价但非零**: body 级 `set_translation` 有"相等则 no-op"保护(`rigid_body.rs` L975-977),完全不动标志位;但 collider 级 `set_translation` **没有相等保护**(无条件置 `ColliderChanges::POSITION`,collider.rs L318-321),且经 `collider_set[handle]` 的 IndexMut 每次都登记 modified → 每个 step 都会走 user-changes + AABB 更新成本。事实: "写一次就停" 与 "每帧都写" 在 rapier 内部成本不同。
- 对 **dynamic** body 每帧 teleport(`wake=false`): 该 body 若醒着,teleport 覆盖 `position`+`next_position` 但保留速度 → 与积分竞争(每帧位移被你覆盖,速度仍按重力/冲量累积);若它因速度为零而入睡(见下),之后继续 teleport 也**不会**把它唤醒(唤醒只看 SLEEP/TYPE 标志,manager.rs L210-219)。
- 睡眠判定只看速度与时长、**不看位置是否被外部改写**: `update_energy` 对每个 awake body 每 step 累计 `time_since_can_sleep += dt`(`<R3>/dynamics/rigid_body_components.rs` L1308-1335),低于线速度 0.4 LU/s、角速度 0.5 rad/s 达 2.0 s(默认,L1226-1239)即 eligible → 被移出 awake 列表(manager.rs L253-283;sleep.rs 整岛迁移 L49-108)。因此一个"每帧被 teleport 但速度恒为零"的 dynamic body 在默认参数下约 2 s 模拟时间后仍会睡去(除非带 wake=true 或把速度设非零 / `can_sleep(false)` 建体)。这是 per-step push 方案必须知道的睡眠耦合点。
- 每帧修改造成的 **modified-body 处理路径**: 每 step 开头 user-changes 会为每个 modified enabled body 跑 `islands.rigid_body_updated`(轻量,不唤醒,见 3.2),并同步其 collider 位姿 + 标 modified(user_changes.rs L82-87)→ broad-phase 每步刷新这些 collider 的 AABB(physics_pipeline.rs L744-777)。即每帧 push 的同步对象越多,user-changes/broad-phase 固定开销越大(与 island 数量无关)。
- **pull(rapier → engine)读回**: `RigidBody::translation()`/`rotation()`/`position()` 读 `pos.position`(rigid_body.rs L948-959、L994-996);该值在每 step 内由 solver 写 `next_position`、再由 `advance_to_final_positions` 对活跃 body 提交为 `position`(physics_pipeline.rs L396-410)。collider 侧读 `collider.translation()`/`rotation()`(collider.rs L343-355)。**睡眠 body 不经过积分,读回即上次稳定值**。
- rapier 为"每步移动"内建的通道是 kinematic body:位置式 kinematic 用 `set_next_kinematic_*`(见 3.1 表),引擎按 `next_position − position` 反算速度(L412-438);kinematic 仅在速度严格为零时可睡(`rigid_body_components.rs` L1322-1326)。速度式 kinematic 用 `set_linvel`(L891-903)。

### 对本 map 设计票的含义(#152/#153/#154 必须尊重的事实)

- **#153(系统集与同步)**: "每帧把 Transform push 给刚体"对不同 body 类型语义完全不同: fixed/standalone 无睡眠干扰但 collider 级重复写有 modified 成本;dynamic 的每帧 teleport 会与积分竞争、且不改速度时 body 仍会照常入睡(睡眠只认速度);真正按"目标位姿驱动"设计的是 kinematic position-based 通道。pull 读回对睡眠体只是"上次稳定值"。这些约束决定了同步系统必须按 body 类型分路径,而不是一个万能 setter。
- **#152(API 形态)**: wake 参数不是摆设 —— `false` 表示"静默瞬移"(睡眠体不醒),`true` 表示"瞬移并(在下一次 step)唤醒岛";对 fixed body 两者等价。wrapper 若想暴露"teleport 且确保参与仿真",需要把 wake 参数与 `RigidBodyChanges` 语义一起保留。
- **#154(despawn 清理)**: 与 wake 无关,但 `ColliderSet::remove`/`RigidBodySet::remove` 都要求把 `IslandManager` 传进来并正确维护 —— despawn 清理实现必须持有全部集合(参见 `PhysicsEngine` 字段,physics.rs L20-35)。

---

## 4. `IntegrationParameters.dt`:读取时机与 gravity 参数

### 4.1 dt 是每 step 读取一次的公开字段,step 之间可改

- `IntegrationParameters` 是 `Copy` 纯数据 struct(`<R3>/dynamics/integration_parameters.rs` L168-247),`dt: Real` 是 `pub` 字段(L178);引擎持有它的是 `PhysicsEngine.integration_parameters`(`physics.rs` L24,构造为 `IntegrationParameters::default()` L43)。
- `PhysicsPipeline::step` 的参数形态:`step(&mut self, gravity: Vector, integration_parameters: &IntegrationParameters, islands, broad_phase, narrow_phase, bodies, colliders, impulse_joints, multibody_joints, ccd_solver, hooks, events)`(`<R3>/pipeline/physics_pipeline.rs` L490-504)。
- step 一开头就把参数**整体按值拷贝**进局部变量:`let mut remaining_time = integration_parameters.dt; let mut integration_parameters = *integration_parameters;`(L608-609)。之后对 `dt` 的所有修改(CCD 子步切分,见下)都发生在这份拷贝上,**不写回调用者的结构体**。
- 推论(源码级事实): 调用者持有的 `IntegrationParameters.dt` 在**每次 `step` 调用时被读取一次**,`step` 返回后可以任意改写、下一个 `step` 用新值;不存在跨 step 的内部状态缓存 dt。engine 目前的 step 调用(`physics.rs` L83-96)每次都传 `&self.integration_parameters`,同理。
- dt 在 step 内部的消费点(全部取自该步的 dt,出处为 `<R3>/pipeline/physics_pipeline.rs`): CCD 子步拆分与 `min_ccd_dt` 下限(L611-677,仅在 `max_ccd_substeps` > 1 时发生切分;默认 `max_ccd_substeps = 1`,见 Q5);`islands.update_islands(dt, length_unit, …)`(L201-210)→ 睡眠计时累加 `update_energy(…, dt)`(见 Q3/Q5);窄相 `compute_contacts(prediction_distance, dt, …)`(L171-181);kinematic 速度插值用 `inv_dt`(L412-438);soft-CCD 预测 AABB 用 `params.dt`(`<R3>/geometry/collider.rs` L572-599);每个 active island 的 solver `init_and_solve(…, &integration_parameters, …)`(L268-281)整体消费(接触软度/正则化等,见 Q5)。

### 4.2 gravity:每 step 参数,与 IntegrationParameters 无关

- `<R3>/dynamics/integration_parameters.rs` 全文**没有 gravity 字段**;整个 crate 里 gravity 只出现在 `step` 的参数、内部力积分函数与刚体的 `gravity_scale` 字段上(见 Q3 3.2 引用范围)。`Real` 为 f32(dim3 f32 feature)。
- gravity 在 step 内被用于"活跃 body 的有效外力合成": `rb.forces.compute_effective_force_and_torque(gravity, effective_mass)`(`physics_pipeline.rs` L245-255),再乘每 body 的 `gravity_scale`(`<R3>/dynamics/rigid_body_components.rs` L898;setter `RigidBody::set_gravity_scale(scale, wake_up)` `rigid_body.rs` L726-735)。solver 每子步都以该 gravity 参数驱动(L685)。
- 也就是说:**重力不是"存在 IntegrationParameters 里的配置",而是每次 `step` 传入的独立参数**;引擎把 `gravity` 存在自己的字段(`physics.rs` L23,构造值 `(0,-9.81,0)` L42)并与 `integration_parameters` 分开传(L84-85)。
- 每 body 的 `gravity_scale` 默认 1.0,可在不改全局 gravity 的情况下按 body 关/调重力(`rigid_body_components.rs` 默认值区;`rigid_body.rs` L726-735)。

### 对本 map 设计票的含义(#152/#153 必须尊重的事实)

- **#153(系统集与同步)**: rapier 侧没有"内部固定步"概念 —— 每步的 `dt` 由调用方在 step 之间自由设定,且每步还单独给 gravity。引擎把 gravity 存字段、把 `integration_parameters` 也存字段,意味着"改 dt"只是写一个字段的事;"重力方向/大小变化"也一样(下个 step 生效)。若要在 FixedUpdate 里驱动物理(见 Q5),dt 来源(FixedTime)与 rapier dt 需要显式打通,不会自动一致。
- 想了解每步内部实际消耗的 dt(如 CCD 切分后),只能靠 `Counters`/自测;对引擎来说默认配置下(无 CCD 子步)每步 dt == 传入 dt(Q5)。

---

## 5. Fixed timestep 与 rapier 默认:64 Hz vs 60 Hz 的数值事实

### 5.1 rapier 默认参数(`<R3>/dynamics/integration_parameters.rs` Default,L303-328)

| 参数 | 默认 | 备注 |
|---|---|---|
| `dt` | **1/60 s** | L306 |
| `min_ccd_dt` | 1/60/100 s | L307 |
| `max_ccd_substeps` | **1** | 默认不做 CCD 多子步切分;每 step 全程一个 dt(L322;step 内逻辑 L611-677: `remaining_substeps=1` 直接走 `dt=remaining_time` 分支) |
| `num_solver_iterations` | 4 | L312 |
| `contact_softness` | natural_frequency 30 Hz、damping_ratio 5 | L308 + L59-64;其 `erp`/`cfm`/`erp_inv_dt` 全部以"当步 dt"现算(L79-138) |
| `warmstart_coefficient` | 1.0 | L309 |
| `length_unit` | 1.0 | L323;放大 `allowed_linear_error`/`max_corrective_velocity`/`prediction_distance`(L278-300),也参与睡眠阈值换算(Q3) |
| `normalized_allowed_linear_error` / `max_corrective_velocity` / `prediction_distance` | 0.001 / 10.0 / 0.002 | L319-321 |
| `friction_model`(dim3) | `Simplified` | L324-325 |

- 引擎现状恰好是默认值:`PhysicsEngine` 用 `IntegrationParameters::default()`(`physics.rs` L43),从未改写任何字段。

### 5.2 engine 侧的两套"固定步长"

- **kairos 固定时钟(已接线,运行中)**: `kairos_time` 的 `FixedTime` 默认 64 Hz —— `DEFAULT_FIXED_TIMESTEP = Duration::from_micros(15_625)`(`kairos_time/src/lib.rs` L149-156、L194-201),1/64 s 是 2 的幂倒数,可无损转 f32/f64 与整数微秒(L149-155 的注释)。驱动在 `kairos_editor/schedule.rs` `run_fixed_main_loop`(L206-218): 每帧把 `Time` 的 delta 累计进 `FixedTime`(L207-208),`while expend()` 每耗一个整步跑一次 `FixedUpdate` 调度(L211-214);`FixedTime::expend` 语义 `checked_sub`,余数跨帧保留、`delta()` 恒为一个 timestep(`kairos_time/src/lib.rs` L305-320)。每帧步数上限来自 `Time` 默认 250 ms max_delta 钳制 → ~16 步/帧 @64 Hz(schedule.rs L198-200 注释)。
- **physics 模块自带的累加器(已存在但未接线)**: `PhysicsEngine::update`(`physics.rs` L73-111)内部 `FIXED_DT = 1/60`(L74)、`MAX_ACCUMULATOR = 0.25`(L75),`accumulator += delta_time` 后 `while accumulator >= FIXED_DT` 调一次 `step`(L77-99)。这套累加器与 `FixedTime` 完全独立、且**没有任何调用者**(Q0)。注意它是 60 Hz、而编辑器里已接线的固定时钟是 64 Hz —— 两者当前互不连通。
- 编辑器每帧流程:`KairosEditorRuntime::redraw`(runtime.rs L246)→ `KairosEngine::update`(runtime.rs L251;kairos_editor.rs L98-102)→ `Engine::update` = 跑 `Main` 调度(内含 `RunFixedMainLoop`,kairos_editor.rs L65-70),之后 `KairosGame::update`(物理块被注释,kairos_game.rs L291-300)。

### 5.3 64 Hz 驱动 + rapier 默认 dt 的数值后果(纯算术事实)

- 若以 64 Hz 每帧跑 `step`、而 `integration_parameters.dt` 保持默认 1/60 s,则每秒模拟的物理时间 = 64 × (1/60) = 16/15 ≈ **1.0667 s**,即物理时间比固定时钟(elapsed)快约 6.7%(每 15 游戏秒多跑 1 物理秒)。要让物理时间与 `FixedTime.elapsed` 对齐,每步 dt 必须等于 FixedTime 的 timestep(1/64 s = 0.015625 s,在 f32 下可精确表示)。
- 引擎当前没有对齐问题,因为物理根本没被驱动(Q0);问题只在将来把 step 挂到 64 Hz 驱动、且忘了改 dt 时出现。
- `FixedTime::delta()` 每次成功 step 报恰好一个 timestep(`kairos_time/src/lib.rs` L312-320),若物理系统跑在 `FixedUpdate` 里,可据此拿到与步长一致的秒数。

### 5.4 睡眠阈值与 dt 的耦合(以及其它"每步 dt 可变"须尊重的点)

- 睡眠计时按**模拟时间**累计,与帧率/步频解耦: `time_since_can_sleep += dt` 每个 awake step 执行一次(`<R3>/dynamics/rigid_body_components.rs` L1308-1335)。默认 2.0 s 睡眠延迟 = 60 Hz 下约 120 步、64 Hz 下约 128 步,模拟时间语义一致(2.0 s)。dt 只改变达到该时长的步数。
- 睡眠阈值本身是速度阈值: 线速度 < 0.4 × `length_unit`(单位/秒)、角速度 < 0.5 rad/s(`rigid_body_components.rs` L1226-1239、L1318-1320);与 dt 正交,但与 `length_unit` 耦合(若地图单位不是米,需调 `length_unit` 与各 `normalized_*`)。
- 每步现算且依赖 dt 的量(步间改 dt 会改变这些量,可变步长通常不稳,但 API 不阻止): 接触软度正则化 `erp`/`cfm`(integration_parameters.rs L79-138)、kinematic 速度插值 `inv_dt`(physics_pipeline.rs L429-432)、warmstart 前提是前后步一致性(L309 系数 1.0)、CCD `min_ccd_dt`(L307)。`inv_dt()` 在 dt==0 时返回 0 而非除零(L249-256);`set_dt` 有非负断言(L260-264),但 `dt` 是 pub 字段可绕过。
- rapier `Real` = f32(dim3,f32 feature;`<P3>/math/mod.rs` L13),dt 直接以 f32 秒参与;1/64 与 1/60 在 f32 下分别精确/近似(0.015625 精确;1/60 ≈ 0.016666668 有舍入),长时间累加误差可忽略但存在。

### 对本 map 设计票的含义(#152/#153 必须尊重的事实)

- **#153(系统集与同步)**: 把 rapier `step` 接进 64 Hz `FixedUpdate` 时,`integration_parameters.dt` 与 `FixedTime::timestep()` 是两个独立的值,不会自动相等;不改 dt 会得到 ~6.7% 的物理时间漂移(5.3 的算术)。引擎里还残留一套未接线的 60 Hz 自累加器(`physics.rs` L73-99),不可与 64 Hz 固定时钟混用为同一 dt 来源。
- 睡眠(2.0 s 模拟时间)、正则化/软度(warmstart 1.0)默认都假定 dt 恒定;若决策要支持"可变 dt 或 catch-up 大步",rapier 一侧没有禁止,但睡眠计时是模拟时间累积(步数自适应),而接触软度/速度插值随 dt 变化 —— 事实层面能说的到此为止,具体取舍归设计票。
- 默认 `max_ccd_substeps = 1` 意味着(除非未来显式开启 CCD 子步)每 step 的 dt 就是调用者给的 dt,不存在"内部又偷偷切分"的情况(4.1/5.1 引用)。

---

## 附录 A:引用速查

- engine(行号 = HEAD `cbd87ac`): `kairos_engine/src/physics.rs` L20-35(L73-111 update、L74 1/60、L83-96 step 调用);`physics/rigid_body.rs` L12-58;`physics/collider.rs` L11-60;`kairos_game.rs` L228-246(L291-300 注释物理);`kairos_editor.rs` L20-47(L65-70)、L73-102;`kairos_editor/runtime.rs` L246-251;`kairos_editor/schedule.rs` L185-218;`kairos_time/src/lib.rs` L149-156、L177-201、L305-320;`Cargo.lock` L3581-3583、L4151-4153。
- rapier3d 0.33.0(`<R3> = ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rapier3d-0.33.0/src`):
  - Q1: `geometry/collider_set.rs` L163-180;`geometry/collider.rs` L51-64、L314-355;`geometry/collider_components.rs` L144-198;`geometry/narrow_phase.rs` L736-751、L842-852;`pipeline/query_pipeline.rs` L590-625。
  - Q2: `dynamics/rigid_body_set.rs` L200-238(L429-452 Index);`geometry/collider_set.rs` L263-307、L335-367;`dynamics/rigid_body.rs` L750-779;`dynamics/island_manager/manager.rs` L59-100。
  - Q3: `dynamics/rigid_body.rs` L726-735、L790-817、L891-940、L948-996、L1002-1079;`dynamics/rigid_body_components.rs` L1191-1335;`dynamics/island_manager/manager.rs` L158-220;`dynamics/island_manager/sleep.rs` L31-46、L49-122;`pipeline/user_changes.rs` L10-87;`pipeline/physics_pipeline.rs` L396-410、L412-438、L744-777;`geometry/narrow_phase.rs` L430-470;`geometry/collider_set.rs` L470-475。
  - Q4: `dynamics/integration_parameters.rs` L168-264、L278-328;`pipeline/physics_pipeline.rs` L490-504、L608-677、L201-210、L171-181、L245-255、L268-281。
  - Q5: `dynamics/integration_parameters.rs` L303-328(L306 1/60);`dynamics/rigid_body_components.rs` L1308-1335;`pipeline/physics_pipeline.rs` L611-677。
- parry3d 0.28.0(`<P3> = 同注册表/parry3d-0.28.0/src`): `math/mod.rs` L13、L27-42、L67-97(glamx 数学类型)。

## 附录 B:验证声明

- 本文 rapier/parry 部分逐条对照注册表源码(rapier3d-0.33.0 / parry3d-0.28.0,与 `Cargo.lock` 一致)阅读得出;engine 部分对照 HEAD 工作树。
- 未运行 `cargo build/test`;无运行期行为验证。若需对"standalone collider 的表现是否与 fixed 完全一致"等做行为级确认,可在独立 rapier 测试中验证(超出本清点范围)。
- 注册表目录若在读者环境不可用,上述 rapier 行号可对照 docs.rs 的 rapier3d 0.33.0 源码(同一发布版本)。
