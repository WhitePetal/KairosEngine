# Bevy App 主调度 / Startup 机制参照 + kairos_ecs 引导资源核实

> Wayfinder research ticket: [Research: bevy_app 主调度/Startup 机制参照 + kairos_ecs 引导资源核实](https://github.com/WhitePetal/KairosEngine/issues/128)（map #127）。
> 日期：2026-09-08。上游依据：`bevyengine/bevy` 最新稳定 tag **`v0.19.1`**（源码直读，行号以该 tag 为准）；本地依据：`kairos_ecs` / `kairos_tasks` crate（branch `bevy_fork`，行号以当时磁盘为准）。
> 目的：为在 kairos_engine 内实现 bevy_app 式骨架（无 `Plugin` trait、不动子系统）建立执行前的事实清单。

---

## 0. 结论速览

- bevy 的"Main 调度 + 有序子调度"**不是调度图内嵌套节点**，而是 **bevy_app 层的驱动器系统逐帧顺序调用**：`Main` 调度图里有一个普通系统 `Main::run_main`，它读 `MainScheduleOrder` 资源，逐个 `world.try_run_schedule(label)` 跑子调度。
- `Startup` 阶段**恰好一次**由 `run_main` 系统上的持久状态 `Local<bool> run_at_least_once` 保证：第一次执行 `run_main` 时先按 `startup_labels`（`PreStartup → Startup → PostStartup`）依次跑一遍，再跑主 `labels`；标志置位后永不再跑。
- kairos_ecs 侧已具备全部所需原料：`Schedule` / `Schedules`（World 资源）/ `World::run_schedule` / `try_run_schedule` / `schedule_scope`，且 `Schedules::add_systems`/`entry` 会按需自动创建 schedule、`Schedules` 资源由 `get_resource_or_init` 按需自动插入。**全仓库不存在 `MainScheduleOrder`**，引擎需自己实现"顺序表 + 驱动器"。
- 引导资源清单：默认多线程 executor **不需要**引擎预置任何 World 资源——`ComputeTaskPool` 是进程级 `OnceLock` 惰性自建；`MainThreadExecutor` 以 `Option` 读取，缺失时回退到"调用线程的 `ThreadExecutor`"。仅当调度内含**非 Send 系统**且驱动线程可能不是主线程时，才必须预插 `MainThreadExecutor`（须在主线程构造）。
- 逐帧边界：`Schedule::run` **不会**调用 `World::clear_trackers`（整个 kairos_ecs 生产代码无调用点）。bevy 中"每帧一次 clear_trackers"由 `App::update` 收尾负责；kairos 引擎驱动代码必须自己每帧调用一次。
- 顺序驱动多个子调度时，每个 `Schedule::run` 在返回前已把本调度挂起的 deferred 全部冲刷（executor `apply_final_deferred` 默认 `true`），因此"A 调度命令在 B 调度开始时必已生效"。

---

## 1. bevy_app 侧事实

### 1.1 数据模型：World 在 SubApp 内，schedule 按 label 存 World 资源

- `App` 不直接持有 `World`：`App` 持有 `SubApps { main: SubApp, ... }`，`World` 是 `SubApp` 的私有字段（`bevy_app/src/sub_app.rs` `SubApp { world: World, ... }`）。
- 每个 sub-app 要逐帧跑的调度以 **`Option<InternedScheduleLabel>`** 存在 `SubApp::update_schedule` 字段；`App::default` 置为 `Some(Main.intern())`（`bevy_app/src/app.rs` `App::default`）。
- **`Schedules` 是一个 `#[derive(Default, Resource)]` 的 World 资源**：`HashMap<InternedScheduleLabel, Schedule>`（bevy_ecs `schedule.rs`），`SubApp::default` 里 `world.init_resource::<Schedules>()`。它存"除当前正在运行的那个之外的所有 schedule"。

### 1.2 逐帧执行与 clear_trackers 位置

- `App::update` 只做：构建插件期间的 panic 检查，然后 `self.sub_apps.update()`（`app.rs`）。
- `SubApps::update`（`bevy_app/src/sub_app.rs`）：
  1. `self.main.run_default_schedule()` → 读 `update_schedule` label → `world.run_schedule(label)`（即跑 `Main`）；
  2. 逐个子 sub-app `extract()` + `update()`；
  3. **最后** `self.main.world.clear_trackers()` —— 保证每个 `App::update` 只清一次 change-detection tracker。
  - 注意：`run_default_schedule` 文档明确 "Does not clear internal trackers"；清 tracker 是 update 的职责。
- runner 与 `App` 解耦：`App::run` 用 `mem::replace` 取出 `self.runner`（默认 `run_once`），换入 `App::empty()` 后调用 runner；`finish()`/`cleanup()` 在 runner 内、首帧 `update()` 之前完成。headless 循环 runner 为 `ScheduleRunnerPlugin`（`RunMode::Loop`：每 tick 一次 `app.update()` + `should_exit()` 检查）。

### 1.3 MainScheduleOrder：顺序驱动器，而非图内嵌套

```rust
// bevy_app/src/main_schedule.rs
#[derive(Resource, Debug)]
pub struct MainScheduleOrder {
    pub labels: Vec<InternedScheduleLabel>,          // 主阶段顺序
    pub startup_labels: Vec<InternedScheduleLabel>,  // startup 阶段顺序
}
```

- 默认 `labels = [First, PreUpdate, RunFixedMainLoop, Update, SpawnScene, PostUpdate, Last]`；`startup_labels = [PreStartup, Startup, PostStartup]`。
- 初始化者是 **`MainSchedulePlugin`**（`App::default` 里 `add_plugins(MainSchedulePlugin)`）。其 `build`：建 `Main`/`FixedMain`/`RunFixedMainLoop` 三个 schedule 并 `add_schedule`；三个 schedule 均显式设 `SingleThreadedExecutor`（facilitator 调度简单顺序执行即可）；`init_resource::<MainScheduleOrder>()`；`add_systems(Main, Main::run_main)`、`add_systems(FixedMain, FixedMain::run_fixed_main)`。
- 插件/用户改顺序 = 改资源：`insert_after / insert_before / insert_startup_after / insert_startup_before`（纯 `Vec` 插入）。`bevy_state` 的 `StateTransition` 就是插进该 order（不在默认列表）。
- 执行者 `Main::run_main`（**结论：是 (ii) 驱动器逐次调用，不是 (i) 大图嵌套**）：

```rust
impl Main {
    pub fn run_main(world: &mut World, mut run_at_least_once: Local<bool>) {
        if !*run_at_least_once {
            world.resource_scope(|world, order: Mut<MainScheduleOrder>| {
                for &label in &order.startup_labels {
                    let _ = world.try_run_schedule(label);
                }
            });
            *run_at_least_once = true;
        }
        world.resource_scope(|world, order: Mut<MainScheduleOrder>| {
            for &label in &order.labels {
                let _ = world.try_run_schedule(label);
            }
        });
    }
}
```

- 每个 label（`First`/`Update`/…）在 `Schedules` 资源里是**独立**的 `Schedule`，互不嵌套。
- 系统注册：`App::add_systems(Update, …)` → `Schedules::add_systems(label, systems)` → `self.entry(label)`，**label 不存在时自动 `Schedule::new(label)`**；显式建空 schedule 用 `init_schedule`/`add_schedule`。
- schedule **按需存在**：从无人加系统的 label，`try_run_schedule` 返回 `Err` 被 `let _ =` 吞掉跳过。

### 1.4 Startup 恰一次：`Local<bool>`，时机 = 首帧 Main 内、先于主阶段

- 不存在单独的 `run_startup_system`；路径是 `App::run → runner → App::update → world.run_schedule(Main) → Main 图里的 run_main 系统`。
- **"恰好一次" = `run_main` 系统的 `Local<bool> run_at_least_once`**。`Local` 状态跨多次 schedule 运行持久，整个 App 生命周期只翻转一次。
- 时机：**第一个 `App::update`（第一帧）期间**，`run_main` 同一系统执行里，startup 块先于主 `labels` 循环，因此 Startup 早于同帧的 `First`。第二帧起不再跑。
- 想"额外再跑一次"的插件往 `startup_labels` 插入新 label。

### 1.5 bevy_ecs 图层面佐证

- `NodeId` 只有 `System(SystemKey)` / `Set(SystemSetKey)` 两种变体，**没有 schedule/sub-schedule 节点**；`ApplyDeferred` 在该版本是一种特殊 `System`（build pass 自动插入、executor 识别处理），也不是节点。
- 结论：**有序串行子调度完全是 bevy_app 层的驱动器循环**；bevy_ecs 只提供 `try_schedule_scope` 机制（运行期间把目标 schedule 从 `Schedules` 临时取出、跑完放回，天然支持串行/嵌套调用）。

---

## 2. kairos_ecs 侧事实

（路径相对 `KairosEngine/kairos_ecs/src/`，branch `bevy_fork`）

### 2.1 Executor 与引导资源清单

`MultiThreadedExecutor`（`schedule/executor/multi_threaded.rs`，`pub struct` 约 L101）的 `fn run`：

1. **空 schedule 提前返回**（`schedule.systems.is_empty()` → return），早于任何资源获取——空跑一个调度零资源依赖。
2. `MainThreadExecutor` 以 **`Option`** 读取（无错误路径）：
   ```rust
   let thread_executor = world
       .get_resource::<MainThreadExecutor>()
       .map(|e| e.0.clone());
   ```
3. `ComputeTaskPool` **不是 World 资源，是进程级全局 static，惰性自建**：
   ```rust
   ComputeTaskPool::get_or_init(TaskPool::default).scope_with_executor(false, thread_executor, |scope| { … });
   ```
   第一次 run 自动 `TaskPool::default()` 建池（`kairos_tasks/src/usages.rs`：`OnceLock` + `get_or_init`）。
4. 非 Send 系统走 `spawn_on_external`（external executor = `MainThreadExecutor`）；排他系统（含 `ApplyDeferred`）走 `spawn_on_scope`；普通 Send 系统走池线程。**只有"含非 Send 系统的非空调度"才真正依赖 `MainThreadExecutor` 指向主线程**。
5. `MainThreadExecutor` 缺失时 `thread_executor = None` → `kairos_tasks` 的 `scope_with_executor` 用"**调用 `run` 的当前线程**"的 `THREAD_EXECUTOR` 作为 fallback（external 与 scope 同一）。即：若引擎始终在主线程驱动调度且无非 Send 系统，不插该资源也无碍；若在别的线程驱动且含 `!Send` 系统，非 Send 系统会跑在驱动线程而非主线程（语义偏差）。

`MainThreadExecutor` 定义与构造：

- 定义于 `schedule/executor/multi_threaded.rs`（约 L853）：
  ```rust
  #[derive(Resource, Clone)]
  pub struct MainThreadExecutor(pub Arc<ThreadExecutor<'static>>);
  ```
  `Default`/`new` → `MainThreadExecutor(TaskPool::get_thread_executor())`。
- `TaskPool::get_thread_executor()`（`kairos_tasks/src/task_pool.rs`）：取 `thread_local!` 里当前线程的 `THREAD_EXECUTOR`——**每个线程各有一个**；所以 `MainThreadExecutor::new()` 捕获的是"构造时所在线程"的执行器，**引导时须在主线程构造再插入 World**。
- 公开路径 `kairos_ecs::schedule::MainThreadExecutor`（`executor.rs` `pub use multi_threaded::{MainThreadExecutor, MultiThreadedExecutor}`）；**不在 prelude**。

bootstrap 全链结论（首次跑一个 `Schedule`）：

- `Schedule::run`（`schedule/schedule.rs`）= `world.check_change_ticks()` → `self.initialize(world)`（懒构建 graph/executable + 首次 `executor.init`）→ `executor.run(...)`。
- World 里需要的：(1) 该 `Schedule`（直接 `&mut World` 跑或放进 `Schedules` 用 `World::run_schedule`）；(2) 含非 Send 系统时才建议 `insert_resource(MainThreadExecutor::new())`。`Schedules` 资源与 `ComputeTaskPool` 均自动按需创建/自建。

`SingleThreadedExecutor`（`executor/single_threaded.rs`）：不读任何 World 资源，顺序执行 + `apply_final_deferred` 收尾。选择方式：`executor.rs` 的 `default_executor()` **当前代码无条件返回 `MultiThreadedExecutor`**（doc 声称 Wasm/关 `multi_threaded` feature 时返回单线程，但无对应 cfg 分支且 Cargo 无该 feature——**文档与实现不符**）；想用单线程需显式 `schedule.set_executor(SingleThreadedExecutor::new())`。

### 2.2 按 label 注册与运行的 API 面

`Schedule`（`schedule.rs`）：`label`、`graph`、`executable`、`executor`、`executor_initialized`；`Schedule::new(label)` 即建默认 executor 并注册默认 build pass。

`Schedules` 资源（`schedule.rs`，`FixedHashMap<InternedScheduleLabel, Schedule>`）公开方法：`new / insert / reinsert / remove / remove_temporarily / remove_entry / get_temporarily_removed / get_empty_labels / contains / get / get_mut / entry / iter / iter_mut / check_change_ticks / configure_schedules / allow_ambiguous_component / allow_ambiguous_resource / add_systems / remove_systems_in_set / configure_sets / ignore_ambiguity`。
（仓库里**没有** `add_systems_to_empty`、`labels()`、`Schedules::run` 之类方法；运行一律经 `World`。）

`World`（`world.rs`）：

- `add_schedule(schedule)`：`get_resource_or_init::<Schedules>().insert(schedule)`。
- `run_schedule(label)`：`schedule_scope(label, |w, s| s.run(w))`——label 缺失 **panic**。
- `try_run_schedule(label) -> Result<(), TryRunScheduleError>`：不 panic，返回 Err。
- `schedule_scope` / `try_schedule_scope`：把目标 schedule 从 `Schedules` **临时摘下 → 跑闭包 → reinsert**；运行期间若有人重插同 label 会 warn。对"正在运行的同一 label"递归调用会失败（已被临时摘下）。
- 无 `run_schedule_if_exists`。
- **`World::new`/`Default` 不预置 `Schedules`**；`get_resource_or_init`（world.rs）在缺省时自动创建插入。所以"先 add/entry 再 run_schedule"是最稳路径；直接对空 World `run_schedule` 会 panic。
- 系统内运行另一调度：只有 **`Commands::run_schedule(label)`**（排队到下一个 apply 点执行，label 缺失仅 `warn`）；普通系统参数无 `&mut World`，没有同步直跑 API。**能立即驱动子调度的只有拿到 `&mut World` 的代码 = 排他系统或引擎驱动层**。
- 全仓库（kairos_ecs + kairos_engine）grep `MainScheduleOrder` 零命中；kairos_engine 下无任何 `run_schedule`/`Schedules`/`clear_trackers` 用法。→ 顺序驱动必须由引擎代码写。

### 2.3 ApplyDeferred 与顺序驱动多个调度时的 tick/冲刷语义

自动插入：

- `ApplyDeferred` 是特殊 `System`（flags `NON_SEND | EXCLUSIVE`，自身 run 为 no-op，executor 遇到它时统一 apply 挂起缓冲）。
- 自动插入 = build pass（`schedule/auto_insert_apply_deferred.rs`，默认开启：`ScheduleBuildSettings::new` 中 `auto_insert_apply_deferred: true`）：基于扁平化依赖图，在"有 deferred 缓冲的系统与**有顺序依赖的后继**"之间的边上插同步点。无依赖的并行系统之间不插。
- 插入发生在 **schedule 构建期**（首次 run 或 graph 变更后的那次 run），不是每次 run。
- **schedule 结尾没有自动插入的 ApplyDeferred 节点**；收尾冲刷由 executor 的 `apply_final_deferred`（默认 `true`）完成——每次 `run` 结束把所有仍挂起的 deferred 一次 apply 干净。

change tick：

- 每个函数系统运行取 `world.increment_change_tick()` 并记入 `last_run`——**每系统运行都 bump 一次 World change tick**（与 bevy 一致）；N 个子调度连续跑不会"合并" tick。排他系统用 `last_change_tick_scope` 包裹并在结束时 `flush` + increment。
- `Schedule::run` 入口调 `world.check_change_ticks()`，但它是**低频钳制**（`CHECK_TICK_THRESHOLD = 518_400_000`，绝大多数帧 no-op），不是帧边界。
- **`World::clear_trackers` 在 kairos_ecs 生产代码里没有任何调用点**（`lifecycle.rs` doc 明示：standalone 使用需自己调；bevy 里由 `App::update` 每帧自动调）。→ kairos 引擎驱动代码必须每帧一次 `world.clear_trackers()`（更新 `last_change_tick`，供"帧外直接 World 访问/RemovedComponents"做按帧变更检测）。
- 顺序驱动推论：(a) 各子调度系统逐条 +1 tick，与 bevy 行为一致；(b) 帧边界工作（clear_trackers）需引擎自理；(c) 每个子调度的 `executor.run` 同步阻塞、返回前冲刷干净 → 按驱动顺序（A 后 B），A 中 `Commands` 排队的写一定在 B 开始前可见。
- 已知边界/歧义：
  - 若把某调度 `set_apply_final_deferred(false)`，结尾不冲刷且 `unapplied_systems` 不清空，deferred 可能残留到下一次 run 的同步点——跨 run/跨调度存活。**引擎侧保持默认 true**。
  - `try_schedule_scope` 会把"正在跑的调度"临时摘出 `Schedules`，故 `Schedule::run` 开头 `check_change_ticks` 实际钳制的是**其它**已存回的调度；正在跑的调度自身的 tick 钳制发生在下次跑其它调度时。阈值极高，通常可忽略。

### 2.4 动态性与 gotcha

- **支持首跑后动态 add_systems**：`add_systems`/`configure_sets` 置 `graph.changed = true`，下次 `run` 的 `initialize` 内重建 executable（复用已缓存 SystemState）并 `executor.init` 重算冲突位集。
- executor 每次 run 复用（重置计数），不重建。
- **系统绑定 World**：把在某 World 上初始化/跑过的 `Schedule` 换到另一 World 跑会 panic（"mismatched World"断言）；`Schedule::systems()` 首跑前返回 `ScheduleNotInitialized`。

---

## 3. 对 kairos_engine 骨架落地的推论

1. **顺序表资源 + 驱动器系统直接照搬可行**：`MainScheduleOrder` 只需"主顺序 Vec + startup 顺序 Vec"两个有序 label 表，不需要 trait；kairos 可把它做成普通资源（或引擎字段）并由 `Engine::new` 铺好。
2. **驱动器 = 一个系统**（等价 `run_main`）：把子调度编译进同一张图没有收益（图里无嵌套调度节点），且让"子调度各自独立按需编译、驱动器逐次 `try_run_schedule`"更简单；空子调度零成本（executor 空跑早退）。
3. **Startup once 用驱动器上的持久状态**（等价 `Local<bool>`）：首次驱动帧先循环 startup 顺序再循环主顺序，天然满足"第一帧、先于 First、只一次"。
4. **调度容器 = `Schedules` World 资源**（`World::run_schedule` 的摘除/放回机制使"驱动器跑别人时自己正在被跑"不自锁）；`add_systems`/`entry` 按 label 自动建调度、`init_schedule` 显式建空调度。
5. **引导资源**：kairos_ecs 多线程 executor 不吃任何"必须预置"的 World 资源；为稳妥与未来非 Send 系统，引导时在主线程 `insert_resource(MainThreadExecutor::new())` 即可，`ComputeTaskPool` 无需理会（惰性自建）。
6. **每帧边界由引擎自持**：驱动代码在全部子调度跑完后调一次 `world.clear_trackers()`（对应 bevy `App::update` 收尾）；`FixedUpdate` 每帧空跑一次不需特殊处理（空调度早退 + 每系统才 bump tick）。
7. **帧循环 rails**（`KairosEngine::update` 帧首 `engine.update()` 再 `game.update()`）与上述驱动器二选一/叠加时注意：驱动器系统本身由"每帧跑一次的调度"承载即可，或引擎直接顺序 `world.run_schedule(label)`——kairos_ecs 两者都支持，取决于骨架选择；子调度内 deferred 跨调度可见性有保证（每 run 收尾冲刷）。
