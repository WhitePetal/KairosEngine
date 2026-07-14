# ECS 架构中碰撞回调处理方案

> 对话时间：2025-07-10
> 主题：KairosEngine 的 ECS + rapier3d 物理引擎，如何设计碰撞回调系统。

---

## 1. 当前 KairosEngine 状态

### 已有模块

```
kairos_engine/src/
├── ecs.rs              # Archetype ECS（类似 Bevy/Flecs）
│   ├── world.rs        # World, spawn/despawn/query
│   ├── component.rs    # Component trait
│   ├── entity.rs       # Entity 类型
│   ├── table.rs        # 按组件组合分表存储
│   └── component_tuple/
│       └── query_tuple.rs  # Query, Without, With, Satisfies 等
│
├── physics.rs          # rapier3d 封装
│   ├── collider.rs     # Collider 组件（持有 ColliderHandle）
│   └── rigid_body.rs   # RigidBody 组件（持有 RigidBodyHandle）
│
└── kairos_game.rs      # 游戏主逻辑，手写 update() 顺序调用
```

### 关键局限

1. `PhysicsEngine.event_handler` 当前是 `()`，未实现碰撞回调
2. 没有 `Entity → ColliderHandle` 反向映射
3. 没有 `Added<T>` / `RemovedComponents<T>` 变更检测（无 tick 系统）
4. 没有事件系统（Bevy 的 Event<T>）
5. 没有 System 调度器（`update()` 是手写顺序调用）

---

## 2. 成熟项目的参考方案

### 项目对比总览

| 项目 | 解耦方式 | 游戏层分发方式 |
|------|---------|---------------|
| **Bevy** | `Event<T>` | `EventReader` + 各自 Query |
| **Unity DOTS** | `DynamicBuffer<PhysicsCollisionKey>` 组件 | `IJobEntity` + 各自 Query |
| **Flecs** | Observer（声明式） | term 模式匹配 |
| **EnTT** | `sigh`（信号槽） | connect 注册回调 |

### 共同模式

```
┌──────────────────────────────────────┐
│  物理层：不判断业务逻辑，只产出"碰撞事实" │
│  游戏层：每个 System 根据自己组件组合    │
│          独立解释"碰撞事实"              │
└──────────────────────────────────────┘
```

---

## 3. Bevy + bevy_rapier 深度分析

### 3.1 EventWriter/EventReader 机制

本质是 **双缓冲 + 类型索引的环形队列**：

```rust
// Bevy 内部（简化）
pub struct Events<T> {
    events_a: Vec<T>,          // 缓冲区 A
    events_b: Vec<T>,          // 缓冲区 B
    a_is_current: bool,        // 当前写入哪个
}

impl<T> Events<T> {
    fn send(&mut self, event: T) {
        // 永远写到 "当前" 缓冲区
        if self.a_is_current { self.events_a.push(event); }
        else                { self.events_b.push(event); }
    }

    fn update(&mut self) {
        // 帧末交换
        self.a_is_current = !self.a_is_current;
        // 清空新的非当前缓冲区
    }
}
```

**时序图**：

```
帧 N:
  System A: EventWriter.send()  →  push 到 buf_current
  System B: EventReader.read()  →  读 buf_prev（空的）

Events::update() → swap buffers

帧 N+1:
  System B: EventReader.read()  →  读 buf_prev（有数据了！）
```

**关键设计**：
- 读和写操作不同的 Vec，无需锁
- 每个 Reader 有独立游标（`EventCursor`），多个 System 可独立消费同一事件流
- 同一帧写入的事件不会在同帧被读到（延迟一帧）

### 3.2 Entity ↔ ColliderHandle 双向映射

bevy_rapier 维护两个 HashMap：

```rust
pub struct CollisionEntityMap {
    handle_to_entity: HashMap<ColliderHandle, Entity>,  // rapier → Bevy
    entity_to_handle: HashMap<Entity, ColliderHandle>,  // Bevy → rapier
}
```

**注册时机**：使用 `Added<Collider>` + `Without<ColliderHandle>` 查询自动发现。

**rapier 碰撞对转换**：

```rust
for pair in narrow_phase.contact_pairs() {
    let entity_a = entity_map.handle_to_entity[&pair.collider1];
    let entity_b = entity_map.handle_to_entity[&pair.collider2];
    writer.send(CollisionEvent { entity_a, entity_b, ... });
}
```

---

## 4. KairosEngine 方案设计

### 4.1 推荐方案：数据驱动（碰撞标记组件）

物理层只记录碰撞事实，游戏层各 System 独立消费：

```rust
// 碰撞标记组件
#[derive(Component)]
struct CollisionThisFrame {
    hits: Vec<CollisionHit>,
}

// 物理层产出
physics_engine.update() → 
  for pair in contact_pairs {
      world.insert_one(a, CollisionThisFrame { hits: ... });
      world.insert_one(b, CollisionThisFrame { hits: ... });
  }

// 游戏层消费
fn bullet_system(world: &mut World) {
    for (entity, bullet, collision) in
        world.query_mut::<(Entity, &Bullet, &CollisionThisFrame)>()
    {
        for hit in &collision.hits {
            // 扣血
        }
        world.despawn(entity);
    }
}
```

### 4.2 碰撞事件缓冲区（多线程兼容）

即使目前是单线程，设计上预留双缓冲：

```rust
pub struct PhysicsEngine {
    collision_events_a: Vec<CollisionEvent>,
    collision_events_b: Vec<CollisionEvent>,
    front_is_a: bool,
}

impl PhysicsEngine {
    // 物理线程写（写到 back buffer）
    pub fn write_event(&mut self, event: CollisionEvent) {
        if self.front_is_a { &mut self.collision_events_b }
        else              { &mut self.collision_events_a }
        .push(event);
    }

    // 物理 step 后交换
    pub fn swap_events(&mut self) {
        self.front_is_a = !self.front_is_a;
        // 清空新的 back buffer
    }

    // 游戏线程读（读 front buffer，无需锁）
    pub fn read_events(&self) -> &[CollisionEvent] {
        if self.front_is_a { &self.collision_events_a }
        else              { &self.collision_events_b }
    }
}
```

### 4.3 自动注册：标记组件 + Without 查询

```rust
// 标记组件
#[derive(Component)]
pub struct ColliderRegistered;

impl PhysicsEngine {
    pub fn sync_colliders(&mut self, world: &mut World) {
        // 找到有 Collider 但还没注册的实体
        for (entity, collider) in world
            .query_mut::<(Entity, &Collider)>()
            .without::<(ColliderRegistered,)>()
            .into_iter()
        {
            self.entity_map.register(entity, collider.handle);
            world.insert_one(entity, ColliderRegistered).ok();
        }
    }
}
```

**原理**：KairosEngine 已有 `Without<Q, R>`（`query_tuple.rs:420-481`），其 `access()` 在表级别判断——如果表里有 `R` 类型则跳过此表。

---

## 5. 组件移除/Entity 销毁的清理机制

### 5.1 核心挑战

```
world.despawn(bullet) → rapier 里的 ColliderHandle 必须同步清理
```

### 5.2 Bevy 的做法：RemovedComponents\<T\>

Bevy 在 despawn/remove 时，把 Entity push 到**按组件类型分桶**的 buffer 中：

```rust
// 内存布局
removed_components: HashMap<ComponentId, Vec<Entity>>
    ├── ColliderId    → [e1, e5, e9]
    ├── RigidBodyId   → [e1, e5]
    └── AudioSourceId → [e3]

// 消费
RemovedComponents<Collider>.read()  → 直接读 [e1, e5, e9]，零过滤
RemovedComponents<AudioSource>.read() → 直接读 [e3]，零过滤
```

### 5.3 KairosEngine 推荐设计

**按 TypeId 分桶，不单独设 `despawned_entities`**：

```rust
pub struct World {
    // ... 现有字段

    /// 按组件类型分桶的移除记录
    /// despawn 时: entity 被 push 到所有组件类型的桶
    /// remove_one<T> 时: entity 只被 push 到 T 的桶
    removed_components: HashMap<TypeId, Vec<Entity>>,
}
```

**关键：记录时机在移除之前**：

```rust
impl World {
    pub fn despawn(&mut self, entity: Entity) -> Result<(), NoSuchId> {
        self.flush();

        let entity_data = self.entity_datas.get(entity).ok_or(NoSuchId)?;

        // ★ 1. 先记录（table 完整，可枚举组件类型）
        let table = &self.table_graph[entity_data.table_index];
        for type_info in table.types() {
            self.removed_components
                .entry(type_info.id())
                .or_default()
                .push(entity);
        }

        // ★ 2. 再移除（此时 table 信息仍可用）
        let moved = self.entities.free(entity)?;
        // ... 原有 table.remove_entity 逻辑

        Ok(())
    }

    pub fn drain_removed<T: Component>(&mut self) -> impl Iterator<Item = Entity> + '_ {
        self.removed_components
            .remove(&TypeId::of::<T>())
            .unwrap_or_default()
            .into_iter()
    }
}
```

**消费端**：

```rust
// PhysicsEngine：只读关心的桶
impl PhysicsEngine {
    pub fn sync(&mut self, world: &mut World) {
        for entity in world.drain_removed::<Collider>() {
            if let Some(handle) = self.entity_map.take_collider(entity) {
                self.collider_set.remove(handle, &mut self.rigid_body_set, true);
            }
        }
        for entity in world.drain_removed::<RigidBody>() {
            if let Some(handle) = self.entity_map.take_rigid_body(entity) {
                self.rigid_body_set.remove(handle, ...);
            }
        }
    }
}

// AudioEngine
fn sync_audio(world: &mut World, audio: &mut AudioEngine) {
    for entity in world.drain_removed::<AudioSource>() {
        audio.stop_source(entity);
    }
}
```

### 5.4 为什么不需要 `despawned_entities`

```
despawn 语义 = 所有组件被隐式 remove
→ 已经在 removed_components 的各类型桶中记录了
→ 没有 System 需要额外遍历 despawned_entities
→ 每个 System 只读自己关心类型的桶
```

### 5.5 带数据的移除记录

**当前不需要**。KairosEngine 现有组件清理都不依赖组件内数据：

| 组件 | 清理方式 |
|------|---------|
| Collider | entity_map 已有 ColliderHandle |
| RigidBody | entity_map 已有 RigidBodyHandle |
| AudioSource | AudioEngine 内部映射 |
| Mesh/Material | AssetServer 引用计数管理 |

未来若需要（如自定义脚本 on_destroy 回调），可扩展为 `Vec<(Entity, T)>` 带数据的桶，但需要组件类型注册表（`ComponentMeta`）。

---

## 6. 多线程性能分析

### 6.1 方案 B（缓冲批量） vs 方案 C（即时回调）

| 维度 | 方案 B（缓冲+批量） | 方案 C（即时回调） |
|------|-------------------|-------------------|
| despawn 开销 | `Vec::push`（纳秒级） | HashMap 查找 + 虚调用 |
| 缓存局部性 | ✅ 连续内存 | ❌ 随机堆访问 |
| rapier 效率 | ✅ 可批量 remove | ❌ 逐个 O(n) swap_remove |
| 编译器优化 | ✅ 可内联、可向量化 | ❌ 虚函数屏障 |
| 估计耗时（100 实体） | ~10-20µs | ~50-100µs |

**方案 B 全面胜出**，差距约 3-5 倍。

### 6.2 按类型分桶的性能

```
假设一帧内: despawn 10 实体, remove 5 Collider, remove 2 AudioSource

不分桶（全局 buffer + 过滤）:
  Physics: 遍历 17 条 → 过滤出 15 条       ← 2 条 AudioSource 白遍历
  Audio:   遍历 17 条 → 过滤出 2 条         ← 15 条 白遍历
  总计: 34 次迭代 + 34 次 TypeId 比较

分桶:
  Physics: drain_removed::<Collider>: 15 条 + drain_removed::<RigidBody>: 10 条
  Audio:   drain_removed::<AudioSource>: 2 条
  总计: 27 次迭代 + 0 次 TypeId 比较
```

---

## 7. 实现路线图

```
Phase 1（当前）:
  ├── PhysicsEngine 加 Vec<CollisionEvent> 缓冲区
  ├── 加 Entity ↔ ColliderHandle 双向映射
  ├── 加 ColliderRegistered 标记组件
  └── sync_colliders(): Without<ColliderRegistered> 自动注册

Phase 2（多线程前）:
  ├── World 加 removed_components: HashMap<TypeId, Vec<Entity>>
  ├── despawn/remove 时自动记录
  └── 各 System 通过 drain_removed::<T>() 消费

Phase 3（长期）:
  ├── 支持带数据的移除记录 Vec<(Entity, T)>
  └── 组件类型注册表 ComponentMeta
```

---

## 8. 参考资料

- Bevy ECS: https://bevyengine.org/
- bevy_rapier: https://github.com/dimforge/bevy_rapier
- rapier3d: https://rapier.rs/
- Flecs: https://github.com/SanderMertens/flecs
- Unity DOTS: https://docs.unity3d.com/Packages/com.unity.entities@1.0/manual/
