# Bevy 调度器源码学习路线

> 目标：参照 Bevy 源码，在 KairosEngine 中实现 Schedule + Executor + 冲突检测体系。
>
> 前置条件：T1-T5 已完成（Tick、ComponentTicks、Changed/Added 过滤器、UnsafeWorldCell）。

---

## 一、全链路概览

```
App::update()
    ↓
Schedule::run(&mut world)
    ↓
initialize_systems()          —— 收集 AccessFilters + 调用 System::initialize()
    ↓
build_graph()                 —— 计算 system 之间的冲突矩阵
    ↓
executor.run(world, schedule)
    ├── SingleThreadedExecutor —— 线性执行，不涉及 UnsafeWorldCell
    └── MultiThreadedExecutor  —— 按冲突图分组，组内并行使用 UnsafeWorldCell
        ↓
    System::run_unsafe(UnsafeWorldCell)
        ↓
    SystemParam::get_param(world, ...)  —— 从 UnsafeWorldCell 取出 Query/Res 等
        ↓
    用户函数
```

---

## 二、建议阅读顺序

### 第一遍：理解全貌（约 1-2 小时）

从入口走到 system 调用，先不管并行细节。

```
bevy_app/src/app.rs
  → App::update() 触发 main_schedule.run()

bevy_ecs/src/schedule/schedule.rs
  → Schedule 结构体（systems, graph, executor）
  → run() 方法：initialize_systems → build_graph → executor.run()

bevy_ecs/src/schedule/executor/single_thread.rs
  → 最简单的线性执行流程，无 UnsafeWorldCell
  → 理解 system 调用的基本循环

bevy_ecs/src/system/system.rs
  → System trait 定义（run, run_unsafe, initialize 等）
  → SystemMeta（last_run, access_filters）

bevy_ecs/src/system/function_system.rs
  → FunctionSystem：用户函数 → System
  → initialize() 做了什么
  → run_unsafe() 中 tick 管理 + SystemParam 提取
```

### 第二遍：冲突检测 + 依赖图（约 2-3 小时）

理解调度器如何判断哪些 system 可以并行。

```
bevy_ecs/src/schedule/access.rs
  → AccessFilters：每个 system 对组件/资源的读/写声明
  → Access 枚举（Read / Write）
  → 冲突判断逻辑

bevy_ecs/src/schedule/graph.rs
  → SystemGraph：系统依赖图
  → build() 方法：显式依赖（before/after）+ 隐式冲突
  → detect_conflicts()：遍历 system 对，检查访问集重叠

bevy_ecs/src/query/access.rs*
  → Query 层面如何为每个 component 标记 Access::Read / Write
  → 注意：Bevy 在 fn(Fetch) 宏里做了这件事

bevy_ecs/src/system/system_param.rs
  → SystemParam trait
  → Query<T> / Res<T> 如何实现 get_access()
```

### 第三遍：多线程执行器（约 3-4 小时）

最难的部分，涉及 UnsafeWorldCell + 线程池 + Commands 延迟刷新。

```
bevy_ecs/src/schedule/executor/mod.rs
  → SystemExecutor trait
  → ExecutorKind

bevy_ecs/src/schedule/executor/multi_threaded.rs
  → 核心逻辑（约 400 行）
  → run() → run_system_groups() 流程
  → 如何分 waves（冲突图 → 可并行组）
  → apply_deferred()：system 之间刷新 Commands
  → UnsafeWorldCell 创建时机：&mut World → cell
  → 每个 wave 启动线程池 scope 并行执行
```

### 第四遍：回看关键细节（机动）

```
bevy_ecs/src/world/unsafe_world_cell.rs
  → 你已经实现了，可以对比 Bevy 的完整方法集
  → 注意 get_resource/get_entity 等元数据方法

bevy_ecs/src/system/into_system.rs
  → IntoSystem trait，用户函数自动转为 Box<dyn System>
  → 宏展开的终点

bevy_ecs/src/schedule/applicability.rs
  → NodeId、SystemSet 等辅助类型
```

---

## 三、关键文件清单

| 文件 | 说明 | 预估行数 |
|------|------|---------|
| `bevy_app/src/app.rs` | App + update() 入口 | ~800 |
| `bevy_ecs/src/schedule/schedule.rs` | Schedule 主结构 | ~500 |
| `bevy_ecs/src/schedule/graph.rs` | 依赖图 + 冲突检测 | ~600 |
| `bevy_ecs/src/schedule/access.rs` | AccessFilters 定义 | ~200 |
| `bevy_ecs/src/schedule/executor/mod.rs` | SystemExecutor trait | ~50 |
| `bevy_ecs/src/schedule/executor/single_thread.rs` | 串行执行器 | ~80 |
| `bevy_ecs/src/schedule/executor/multi_threaded.rs` | 多线程执行器 | ~400 |
| `bevy_ecs/src/system/system.rs` | System trait | ~100 |
| `bevy_ecs/src/system/system_param.rs` | SystemParam trait | ~1000 |
| `bevy_ecs/src/system/function_system.rs` | FunctionSystem | ~300 |
| `bevy_ecs/src/system/into_system.rs` | IntoSystem trait | ~100 |
| `bevy_ecs/src/world/unsafe_world_cell.rs` | UnsafeWorldCell | ~200 |
| `bevy_ecs/src/world/mod.rs` | World（as_unsafe_world_cell 等） | ~2000 |

---

## 四、分支/版本选择

**不要看 main 分支**，Bevy main 上 `schedule/` 层在 v0.13→v0.15 经历了大规模重构，当前 main 可能还在变动。

推荐 tag：

```bash
git checkout v0.15.0
```

v0.15.0 的 Schedule/Executor 层结构稳定，文档也最多。理解 v0.15 后再看 main 的 diff 即可跟上新变化。

---

## 五、你当前代码与 Bevy 的对应关系

| Bevy | 你的项目 | 状态 |
|------|---------|------|
| `Tick` + `ComponentTicks` | `change_detection/tick.rs` | ✅ |
| `DetectChanges` trait | `change_detection/access.rs` 中的 `DetectChanges` / `DetectChangesMut` | ✅ |
| `Changed<T>` / `Added<T>` | `component_tuple::query_tuple.rs` 中的 `FetchChanged` / `FetchAdded` | ✅ |
| `UnsafeWorldCell` | `world/unsafe_world_cell.rs` | ✅ |
| `System` trait | `system.rs` 中的 `System` | ✅ |
| `FunctionSystem` | `system.rs` 中的 `FunctionSystem` | ✅ |
| `SystemMeta` | `system.rs` 中的 `SystemMeta` | ✅ 但不完整（缺 access_filters） |
| `AccessFilters` | 有 `for_each_borrow` 作为基础 | ⚠️ 未封装 |
| `Query<T>` | `QueryBorrow` / `QueryMut` | ✅ |
| `SystemParam` | 无（手动调 query） | ❌ |
| `Schedule` + `Executor` | 无 | ❌ |
| `IntoSystem` / 宏 | 无 | ❌ |
| System 生命周期（last_run 管理） | `system.rs` 中 System::run | ⚠️ 已实现但未接入调度器 |

---

## 附录：AccessFilters 与 `for_each_borrow` 的关系

### AccessFilters 是什么

`AccessFilters` 是 Bevy 用来 **描述一个 system 对 World 中哪些组件做读/写访问** 的数据结构。整个调度器的冲突检测就是围绕它做的。

分为四层（文件 `bevy_ecs/src/query/access.rs`）：

```
AccessFilters（直白版）
  ├── Access: 读/写哪些组件
  ├── AccessFilters: With / Without 过滤条件
  ├── FilteredAccess: Access + 过滤条件（一个 query 的完整访问描述）
  └── FilteredAccessSet: 多个 FilteredAccess（整个 system 的总访问描述）
```

核心——`Access`：

```rust
pub struct Access {
    reads: InvertibleComponentIdSet,   // 读/写这个集合里的组件
    writes: InvertibleComponentIdSet,  // 写这个集合里的组件
    archetypal: ComponentIdSet,       // 只查询是否出现，不读取值（如 Has<T>）
}
```

冲突检测的逻辑：

```rust
pub fn is_compatible(&self, other: &Access) -> bool {
    // 我写的东西，你不能读或写
    self.writes.is_disjoint(&other.reads)
    // 你写的东西，我不能读或写
    && other.writes.is_disjoint(&self.reads)
}
```

就这么两行。两个 system 冲突，**当且仅当** 一个 system 写了一个组件，另一个 system 读或写了同一个组件。

`InvertibleComponentIdSet` 比普通 bitset 多了一个能力：它能表示"除了这几个以外的所有"，用于 `read_all()` 和 `write_all()` 的特殊情况（比如 `&mut World` 独占的情况）。

---

### 和 `for_each_borrow` 的关系

你现有的 `for_each_borrow` 已经在做 **收集原始信息** 这一件事：

```rust
// 你的代码 —— query_tuple.rs
Q::Fetch::for_each_borrow(|type_id, is_unique| {
    // type_id：组件类型
    // is_unique: true → &mut T（写）；false → &T（读）
});
```

**对应关系：**

```
for_each_borrow(callback)             → 逐组件收集
    │
    │  每个回调：
    │    is_unique == true   → 相当于 calls access.add_write(type_id)
    │    is_unique == false  → 相当于 calls access.add_read(type_id)
    │
    ▼
Access { reads: Set<TypeId>, writes: Set<TypeId> }  → 存储后做冲突检测
```

**唯一的差距**：`for_each_borrow` 把数据丢给了回调，你没有用一个结构体来 **存储** 和 **比较** 这些数据。

---

### 你需要做的封装

把 `for_each_borrow` 的收集结果存起来：

```rust
use std::any::TypeId;
use std::collections::HashSet;

#[derive(Default, Clone)]
pub struct Access {
    reads: HashSet<TypeId>,
    writes: HashSet<TypeId>,
}

impl Access {
    pub fn add_read(&mut self, type_id: TypeId) {
        self.reads.insert(type_id);
    }

    pub fn add_write(&mut self, type_id: TypeId) {
        self.reads.insert(type_id); // 写隐含着读
        self.writes.insert(type_id);
    }

    /// 收集一个 Query 的所有访问信息
    pub fn from_query<Q: Query>() -> Self {
        let mut access = Access::default();
        Q::Fetch::for_each_borrow(|type_id, is_unique| {
            if is_unique {
                access.add_write(type_id);
            } else {
                access.add_read(type_id);
            }
        });
        access
    }

    /// 冲突检测：核心逻辑
    pub fn is_compatible(&self, other: &Access) -> bool {
        self.writes.is_disjoint(&other.reads)
        && other.writes.is_disjoint(&self.reads)
    }
}
```

然后加到 `SystemMeta` 里：

```rust
// 你的 system.rs
pub struct SystemMeta {
    pub last_run: Tick,          // ✅ 已有
    pub access: Access,          // 新增：这个 system 对所有组件的访问集
}
```

在 `System::initialize()` 时收集，在调度器判断冲突时比较：

```rust
fn can_run_in_parallel(a: &SystemMeta, b: &SystemMeta) -> bool {
    a.access.is_compatible(&b.access)
}
```

---

### 三层对比

| 层面 | Bevy | 你的代码 | 差距 |
|------|------|---------|------|
| 收集 | `SystemParam` 内部调用 `access.add_read/write` | `Q::Fetch::for_each_borrow` | 已有 ✅ |
| 存储 | `FilteredAccessSet: Vec<FilteredAccess>` | 无结构体 | ⚠️ 需封装 |
| 冲突判断 | `Access::is_compatible()` | 无 | ⚠️ 需实现 |
| With/Without 过滤 | `AccessFilters { with, without }` | `With<Q>` / `Without<Q>` 在 query 层 | 先不做 |
| 调度器使用 | `SystemGraph` 构建时调用 `is_compatible` | 无调度器 | ❌ |

**一句话**：`for_each_borrow` 是你的"传感器"，`Access` 是"存储器"，`is_compatible` 是"决策器"。传感器有了，补上存储器和决策器就能让调度器工作。

---

## 六、实现建议

### 6.1 优先实现 AccessFilters

这是整个调度器的基础。利用你现有的 `for_each_borrow` 机制，改造 `SystemMeta`：

```rust
// 你已有：
Q::Fetch::for_each_borrow(|type_id, is_unique| {
    // is_unique: true=写, false=读
});
```

封装成：

```rust
pub struct AccessFilters {
    reads: HashSet<TypeId>,
    writes: HashSet<TypeId>,
}

impl AccessFilters {
    pub fn add_read(&mut self, type_id: TypeId);
    pub fn add_write(&mut self, type_id: TypeId);
    pub fn conflicts_with(&self, other: &Self) -> bool;
}
```

### 6.2 先实现串行执行器

从最简单的开始，`SingleThreadedExecutor` 核心逻辑约 50 行。

### 6.3 再做多线程执行器

步骤：
1. 构建冲突矩阵 → 贪心或简单图着色分出并行组（waves）
2. 组内用 std::thread::scope + UnsafeWorldCell 并行
3. 组间串行 + 应用 Commands

### 6.4 SystemParam 是可选的

如果暂时不想引入复杂的泛型 trait 体系，可以继续手动调 `world.query()`。 
Schedule/Executor 本身可以独立于 SystemParam 工作——只要 System trait 能返回 AccessFilters 就够了。

### 6.5 最小原型验证

建议先在 `.scratch/` 或 `prototypes/` 下写一个 200-300 行的原型，验证：
- 冲突检测逻辑正确
- 并行 system 的数据隔离
- Commands 延迟刷新

跑通后再集成到主代码。

---

## 七、关键注意点

1. **UnsafeWorldCell 不应暴露给普通用户**。它应该是 `pub(crate)`，由调度器内部创建。用户始终通过安全的 `Query<T>`、`Res<T>` 等接触数据。

2. **tick 管理的位置**：Bevy 在 `System::run_unsafe` 中做 `increment_change_tick()`，而不是在 Schedule 层面。这样做是为了每个 system 拿到自己独立的 `this_run`，避免并行 system 互相干扰。

3. **apply_deferred 是必须的**：System 之间不能直接共享 `Commands`。每个 system 把自己的 command 写入自己的 buffer，System A 和 System B 都执行完后，调度器统一 flush。Bevy 通过 `Deferred` + `RenderCommand` 机制实现。

4. **冲突检测的粒度**：Bevy v0.15 使用 `TypeId` 级别的冲突检测。更精细的粒度（同一组件的不同字段）不在调度器层面做，而是在查询层面（`Query::for_each_borrow` 的原子借用）。

5. **Bevy 在 debug 模式做了大量安全断言**（如 `allows_mutable_access`）。如果遇到奇怪的问题，先切到 debug 模式运行。

---

## 八、参考文档

- 设计文档：`docs/research/bevy-change-detection.md`
- handoff：`docs/plan/hierarchy/ecs/handoff.md`
- Bevy v0.15.0 tag: `https://github.com/bevyengine/bevy/tree/v0.15.0`
- 相关 AI 笔记: `docs/ai/ecs-learning-resources-conversation.md`
