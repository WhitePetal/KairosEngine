# Bevy 固定步长时间驱动机制参照（Time 资源族 / FixedUpdate / RunFixedMainLoop）

> Wayfinder research ticket: [FixedUpdate 固定步长驱动定稿与接入](https://github.com/WhitePetal/KairosEngine/issues/132)（map #131，设计 grilling 事实底稿）。
> 日期：2026-09-09。上游依据：`bevyengine/bevy` 稳定 tag **`v0.19.1`**（raw.githubusercontent.com 源码直读，行号以该 tag 为准，逐文件与原文核对；并经本地完整 checkout `/Users/baiaoxiang/bevy`（同 tag）抽查复核，行号一致）；本地依据：`kairos_engine` crate（branch `bevy_fork`，行号以当时磁盘为准）。
> 目的：为在 kairos_engine 落地 bevy 式固定步长驱动建立 **bevy v0.19.1 侧** 事实清单，回答"固定步长由谁推进、每帧跑几次、暂停/变速时怎么办、空内容时成本多少"等问题；本文只记录事实，不给出 kairos 设计建议。

> 直读文件（均 fetch 自 `https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/…`，生产代码部分与 tag 原文逐行核对后编号）：
> `bevy_time/src/lib.rs`、`bevy_time/src/time.rs`、`bevy_time/src/fixed.rs`、`bevy_time/src/virt.rs`、`bevy_time/src/real.rs`、`bevy_app/src/main_schedule.rs`。
> 一个重要的版本事实先放在这里：**v0.19.1 里固定步长驱动器（accumulate + 0..n 次 while 循环）已经整个住在 `bevy_time`（`run_fixed_main_schedule`）里**，bevy_app 只提供标签、顺序表与"每帧跑一次 RunFixedMainLoop 调度"的壳。

---

## 0. 结论速览

- **Time 资源族 = 4 个时钟资源 + 1 个策略资源**：普通 `Time`（泛型 `Time<T: Default = ()>`，`time.rs:L192` 的默认实例）、`Time<Real>`、`Time<Virtual>`、`Time<Fixed>`，由 `TimePlugin::build` 逐个 `init_resource`（`bevy_time/src/lib.rs:L66-72`），另有 `TimeUpdateStrategy`（`lib.rs:L106-123`）。`delta/elapsed/elapsed_wrapped` 等通用字段在泛型 `Time<T>` 本体（`time.rs:L192-204`）；每族语义在 **context 类型**：`Real{startup, first_update, last_update}`（`real.rs:L45-49`）、`Virtual{max_delta, paused, relative_speed, effective_speed}`（`virt.rs:L75-80`）、`Fixed{timestep, overstep}`（`fixed.rs:L69-72`）。
- 普通 `Time`（"默认时间"）= `Time<Virtual>` 的 `as_generic()` 快照；**FixedMain 每执行一步前被换成 `Time<Fixed>` 快照，循环结束立刻还原为 Virtual 快照**（`fixed.rs:L250-257`）。因此 `FixedUpdate` 里的 `Res<Time>.delta()` 恒等于一个固定步长，而 `Update` 里是虚拟 delta。
- **推进顺序（每帧）**：`time_system` 挂在 **`First`** 集合 `TimeSystems`（`lib.rs:L82-87`）——先以渲染世界发来的 `Instant` 或 `Instant::now()` 推进 `Time<Real>`（`lib.rs:L171-178`），再 `update_virtual_time` 把 Real delta 经 **max_delta 钳制 + effective_speed 缩放** 喂给 `Time<Virtual>` 并写回普通 `Time`（`lib.rs:L186` → `virt.rs:L280-284`、`L238-263`）。**`Time<Fixed>` 不在 First 推进**；它的 accumulator 在 `RunFixedMainLoop` 里由驱动器以 **`Time<Virtual>::delta()`** 为输入累加（`fixed.rs:L243-247`）。
- **暂停语义** = `Time<Virtual>::pause()`（`virt.rs:L215-217`）→ 下一次推进时 `effective_speed = 0`（paused 分支 `virt.rs:L250-254`）→ Virtual delta = 0 → 固定循环累加 0 → `expend()` 恒假 → **暂停期间 FixedUpdate 不运行**（bevy 自己的文档直说："If the virtual clock is paused, the FixedUpdate schedule will not run"，`fixed.rs:L47-52`）。0.19 没有 `time_scale` 这个名字：变速是 `Time<Virtual>::set_relative_speed`（`virt.rs:L201-205`，断言 `>=0` 且有限）；`relative_speed = 0` 与 pause 对固定循环效果相同（delta 皆 0，且 `was_paused()` 以 `effective_speed == 0.0` 判定，`virt.rs:L233-235`）。**结论：固定时间从 virtual delta 推进，不是 real delta**——bevy 没有任何"暂停/0 速时固定循环继续跑"的内置开关。
- **默认固定步长是 64 Hz（15625 µs），不是 60 Hz**：`Time<Fixed>::DEFAULT_TIMESTEP`（`fixed.rs:L76`），理由注释：60 Hz 与显示器刷新率存在"一帧 2 步 / 一帧 0 步交替"的病态交互，且 15625µs 是 2 的幂、可无损转 f32/f64（`fixed.rs:L24-30`）。`main_schedule.rs` 的 `FixedMain` 文档同样写 "64 Hz by default"（`main_schedule.rs:L150-158`）。
- **accumulator = `Time<Fixed>` context 的 `overstep` 字段**：`accumulate_overstep` 做 `overstep += delta`（`fixed.rs:L189-191`）；私有 `expend()` 每步 `overstep.checked_sub(timestep)`，成功则 `overstep -= timestep` 并 `advance_by(timestep)`（`fixed.rs:L216-227`）；余数（< 一个 timestep）**跨帧保留不回零**。暴露给用户：`overstep()`（`fixed.rs:L181-183`）、`overstep_fraction()`（`fixed.rs:L205-207`）、`discard_overstep()`（`fixed.rs:L197-200`）。
- **死亡螺旋保护不在固定循环内**（v0.19.1 的 while 没有独立步数上限），而在 **`Time<Virtual>::max_delta` 默认 250 ms**（`virt.rs:L86`）对每帧虚拟 delta 的钳制（`virt.rs:L238-249`）——文档明说 "This also indirectly limits the maximum number of fixed update steps that can run in a single update"（`virt.rs:L106-108`）。64 Hz 下每帧新增 overstep ≤ 250 ms → 单帧至多 16 个整步。
- **主顺序表**默认 `[First, PreUpdate, RunFixedMainLoop, Update, SpawnScene, PostUpdate, Last]`（`main_schedule.rs:L224-232`，`RunFixedMainLoop` 在 PreUpdate 与 Update 之间）；`Main::run_main` 逐 label `try_run_schedule`（`main_schedule.rs:L290-305`）。`FixedUpdate` 这个 label **不在主顺序表里**，它在 `FixedMainScheduleOrder.labels = [FixedFirst, FixedPreUpdate, FixedUpdate, FixedPostUpdate, FixedLast]`（`main_schedule.rs:L358-364`）里，只在"每帧 0..n 次"的 FixedMain 循环中被间接跑到。
- **FixedMain 内容为空 / overstep 不足时**：驱动器系统 `run_fixed_main_schedule` 仍每帧执行（挂在每帧必跑的 `RunFixedMainLoop` 调度中），但 while 体一次不跑——成本只是几次资源访问 + 一次 `try_schedule_scope` 摘/放 `FixedMain`；**没有任何 warn 路径、也没有"首帧强制跑一次"逻辑**。

---

## 1. Time 资源族形状（Q1）

### 1.1 四个时钟都是"泛型 `Time<T>` + context"组合

```rust
// bevy_time/src/time.rs
#[derive(Resource, Debug, Copy, Clone)]
pub struct Time<T: Default = ()> {          // L192：默认类型参数 () → 普通 `Time` 就是 Time<()>
    context: T,                              // L193：族语义挂在 context 上
    wrap_period: Duration,                   // L194：elapsed_wrapped 取模周期，默认 1 小时
    delta: Duration, …                       // L195-203：delta/elapsed 及其 f32/f64/wrapped 派生值
}
```

- 普通 `Time` **不是独立类型**，就是 `Time<()>`（`time.rs:L192`）。`TimePlugin` 插入的四个资源：`Time`、`Time<Real>`、`Time<Virtual>`、`Time<Fixed>`（`lib.rs:L68-71`）。prelude 导出 `Time, Real, Virtual, Fixed, Timer, TimerMode`（`lib.rs:L36-39`）。
- context 各族：`Real{startup, first_update, last_update}` 存墙钟 `Instant`（`real.rs:L45-49`，`Default` 里 `startup: Instant::now()`，`real.rs:L51-59`）；`Virtual` 四个字段（`virt.rs:L75-80`）；`Fixed{timestep, overstep}` 两个字段（`fixed.rs:L69-72`）。
- 推进原语在泛型层：`advance_by(delta)` 写 `delta` 字段、`elapsed += delta`、重算 wrapped（`time.rs:L223-233`）；`as_generic()` 丢掉 context 拷一份 `Time<()>`（`time.rs:L354`）。

### 1.2 普通 `Time` 何时是 Virtual、何时是 Fixed

- 文档语义："`Time` … contains `Time<Virtual>` except inside the `FixedMain` schedule when it contains `Time<Fixed>`"（`time.rs:L20-22`）。实现见驱动器（§4）：FixedMain 每步跑前 `*Time = Time<Fixed>::as_generic()`，循环结束 `*Time = Time<Virtual>::as_generic()`（`fixed.rs:L250-257`）。
- 推论：`FixedUpdate` 系统里 `Res<Time>` 与 `Res<Time<Fixed>>` 读到的 delta/elapsed 相同（都等于最近一步的 timestep）；`Res<Time<Virtual>>` 才是"游戏当前虚拟时刻"（含 paused/effective_speed 查询，`time.rs:L146-155` 的文档示例正是讲这个坑）。
- `Time<Fixed>` 的 `delta()`/`elapsed()` 只在**真的跑了一步**时变化：`expend()` 成功路径调 `advance_by(timestep)`（`fixed.rs:L221`），故每步 delta = 一个 timestep、elapsed 精确 +1 timestep（文档 `fixed.rs:L42-45`）。某帧 0 步时二者保持上一帧值不动。

### 1.3 Time<Fixed> 与"主循环消费的 accumulator"是同一个对象

- 是的：`overstep` 字段就住在 `Time<Fixed>` 的 context 里（`fixed.rs:L71`）；驱动器每帧 `accumulate_overstep(virtual_delta)`（`fixed.rs:L245-247`），同一个资源随后被 `expend()` 逐步消费（`fixed.rs:L250-255`）。**没有第二份"引擎侧 accumulator"**。
- 反过来 `max_delta` 不在 `Time<Fixed>` 上，而在 `Time<Virtual>`（`virt.rs:L76`）——喂给固定循环的每帧虚拟 delta 已经被它钳过了。

---

## 2. 每帧推进顺序，与"暂停时 Fixed 是否继续"的精确轨迹（Q2）

### 2.1 time_system（First）只推 Real + Virtual，不推 Fixed

- 注册：`app.add_systems(First, time_system.in_set(TimeSystems).ambiguous_with(message_update_system))`（`lib.rs:L82-87`）。First 在每帧主循环里最先跑（顺序表 `main_schedule.rs:L224-232`）。
- 系统体：按 `TimeUpdateStrategy` 决定 `Time<Real>` 的推进方式——`Automatic` 用渲染世界 channel 里的 `Instant`，收不到则 `Instant::now()`（`lib.rs:L157-178`）；`ManualDuration(d)` 每帧加定值 d（测试用）；`FixedTimesteps(n)` 每帧加 `fixed_time.timestep() * n`（`lib.rs:L181-183`，此时才读 `Res<Time<Fixed>>`）。
- 然后 `update_virtual_time(&mut time, &mut virtual_time, &real_time)`（`lib.rs:L186`）：
  ```rust
  // bevy_time/src/virt.rs
  pub fn update_virtual_time(current: &mut Time, virt: &mut Time<Virtual>, real: &Time<Real>) {  // L280
      let raw_delta = real.delta();
      virt.advance_with_raw_delta(raw_delta);   // L282
      *current = virt.as_generic();             // L283：普通 Time 快照 = Virtual
  }
  ```
- `Time<Real>::update_with_instant`：**首次调用只记录 first/last update 即返回，delta 保持 0**（`real.rs:L99-107`；文档 `real.rs:L43-52`）——Automatic 模式下第 1 帧 Real/Virtual 都不前进。
- `advance_with_raw_delta`（`virt.rs:L238-263`）三步：① `raw_delta` 超 `max_delta`（默认 250 ms，`virt.rs:L86`）则钳到 max_delta 并 `debug!`（`virt.rs:L239-249`）；② `effective_speed = paused ? 0.0 : relative_speed`（`virt.rs:L250-254`）；③ `delta = clamped.mul_f64(effective_speed)`（`1.0` 时原值避免舍入，`virt.rs:L255-260`）→ `advance_by(delta)`。

### 2.2 固定循环的输入 = Virtual delta；暂停/0 速 → 0 步

- 驱动器每帧从 `Time<Virtual>::delta()` 取输入（`fixed.rs:L244`），**不是 Real delta**。Real 只间接经 Virtual（钳制 + 缩放）影响 Fixed。
- "暂停"完整轨迹：某帧 `Update`（或任何系统）调 `Time<Virtual>::pause()`（`virt.rs:L215-217`，只置 context 标志；文档注明不影响"正在处理的这一帧"的 delta，`virt.rs:L33-35`）→ 下一帧 `First` 的 `advance_with_raw_delta` 走 paused 分支得 delta = 0（`virt.rs:L250-262`）→ 下一帧 `RunFixedMainLoop` 里 `accumulate_overstep(0)`（`fixed.rs:L243-247`）→ `expend()` 恒假（除非残留 overstep ≥ timestep）→ FixedMain 0 次。
- `time_scale = 0`（kairos 命名）在 bevy 0.19 对应 `set_relative_speed(0.0)`（合法：只断言 `>= 0` 且有限，`virt.rs:L201-205`）：effective_speed = 0 → 同上 delta = 0。区别只在 `was_paused()`（`effective_speed == 0.0`，`virt.rs:L233-235`）对两者都返回 true——即 bevy 语境里"以 0 速运行"与"暂停"在时间上不可区分。
- **回答本设计最关心的问题**：暂停（或 0 速）时 `Time<Fixed>` 不积累、FixedUpdate 不运行；恢复后从断点继续（虚拟 elapsed 不跳、残留 overstep ≤ 1 步照常消费），**没有"暂停期间补跑固定步"的追赶机制**。若想要"UI 暂停但物理固定步继续"，bevy 0.19 没有内置开关（固定输入就是虚拟时间），需要自定义时钟或直接驱动 `Time<Fixed>::accumulate_overstep`（该方法是 pub 的，`fixed.rs:L189-191`，文档注明"ordinarily run_fixed_main_schedule 负责计算 overstep"）。

---

## 3. Time<Fixed> 精确语义（Q3）

- 配置入口：`from_duration` / `from_seconds` / `from_hz`（`fixed.rs:L83-109`）；运行时改步长 `set_timestep`（非零断言，`fixed.rs:L128-135`）、`set_timestep_hz`（`fixed.rs:L172-176`）。文档：改动"immediately for the next run"，残留 overstep 按新步长继续处理（`fixed.rs:L56-66`）。
- 默认值：
  ```rust
  // bevy_time/src/fixed.rs
  pub struct Fixed {                // L69-72
      timestep: Duration,           // L70
      overstep: Duration,           // L71 —— 每帧累加、逐步扣减的 accumulator
  }
  impl Time<Fixed> {
      const DEFAULT_TIMESTEP: Duration = Duration::from_micros(15625);  // L76：64 Hz
      fn expend(&mut self) -> bool {                                     // L216-227
          let timestep = self.timestep();
          if let Some(new_value) = self.context_mut().overstep.checked_sub(timestep) {
              self.context_mut().overstep = new_value;
              self.advance_by(timestep);                                // L221：delta/elapsed 前移一个步长
              true
          } else {
              false                                                     // 余量不足一步 → 本帧不再跑
          }
      }
  }
  ```
- accumulator 逻辑：`accumulate_overstep` 纯加（`fixed.rs:L189-191`）；每步由 `expend()` **预付式扣减**——只有凑满一整步才扣并推进 elapsed，从不"超扣"（`checked_sub`），失败时余数保留（测试 `test_expend` 佐证，`fixed.rs` test 模块）。`Default for Fixed` = timestep `DEFAULT_TIMESTEP` + overstep `ZERO`（`fixed.rs:L230-237`）。
- 暴露给固定循环外的方法：`overstep()`（`fixed.rs:L181-183`）、`overstep_fraction[_f64]()`（`fixed.rs:L205-213`）、`discard_overstep()`（饱和减法，`fixed.rs:L197-200`）。
- **无独立 overstep 上限**：v0.19.1 里没有"overstep 超过 X 就丢弃/钳制"的逻辑。上限效果完全来自 Virtual.max_delta = 250 ms（`virt.rs:L86`）在**进入 accumulator 之前**的钳制（`virt.rs:L238-249`）+ `set_max_delta` 可调/`Duration::MAX` 可禁用（`virt.rs:L140-143`、文档 `L133-134`）。64 Hz 下每帧注入 ≤ 250 ms → 至多 16 整步。

---

## 4. RunFixedMainLoop / FixedMain::run_fixed_main 循环逻辑（Q4）

### 4.1 壳（bevy_app）：三个标签 + 两个顺序表资源

- `MainScheduleOrder` 默认主表 `[First, PreUpdate, RunFixedMainLoop, Update, SpawnScene, PostUpdate, Last]`，`RunFixedMainLoop` 位居 PreUpdate 与 Update 之间（`main_schedule.rs:L224-232`）。`Main::run_main` 每帧逐个 `try_run_schedule`（`main_schedule.rs:L290-305`）。
- `MainSchedulePlugin::build`：`Main` / `FixedMain` / `RunFixedMainLoop` 三张 schedule 均设 `SingleThreadedExecutor`（"facilitator" 顺序调度，`main_schedule.rs:L314-319`）并 `add_schedule`（`L321-323`）；`init_resource::<MainScheduleOrder>()` + `init_resource::<FixedMainScheduleOrder>()`（`L324-325`）；`add_systems(Main, Main::run_main)`、`add_systems(FixedMain, FixedMain::run_fixed_main)`（`L326-327`）；给 `RunFixedMainLoop` 配链式集合 `BeforeFixedMainLoop → FixedMainLoop → AfterFixedMainLoop`（`L329-337`）。枚举三变体定义在 `main_schedule.rs:L439/L470/L488`；Before/After 集合每帧恰一次、变步长（文档 `L402-405`）。
- `FixedMainScheduleOrder.labels` 默认 = `[FixedFirst, FixedPreUpdate, FixedUpdate, FixedPostUpdate, FixedLast]`（`main_schedule.rs:L358-364`）。`FixedUpdate` 标签本体定义在 `main_schedule.rs:L132-133`。

### 4.2 驱动器（bevy_time）：每帧一次的系统，内部 while 0..n 次

- `TimePlugin` 把 `run_fixed_main_schedule` 加进 **`RunFixedMainLoop` 调度的 `FixedMainLoop` 集合**（`lib.rs:L89-92`）。注意：驱动逻辑（accumulate + while）在这个系统里，属于 bevy_time，bevy_app 只负责"每帧跑一次 RunFixedMainLoop 调度"。
  ```rust
  // bevy_time/src/fixed.rs
  pub fn run_fixed_main_schedule(world: &mut World) {          // L243
      let delta = world.resource::<Time<Virtual>>().delta();   // L244：唯一输入 = 本帧虚拟 delta
      world.resource_mut::<Time<Fixed>>().accumulate_overstep(delta);  // L245-247
      // Run the schedule until we run out of accumulated time
      let _ = world.try_schedule_scope(FixedMain, |world, schedule| {  // L250
          while world.resource_mut::<Time<Fixed>>().expend() {         // L251：每步成功即扣一个 timestep
              *world.resource_mut::<Time>() = world.resource::<Time<Fixed>>().as_generic();  // L252
              schedule.run(world);                                     // L253：跑一整轮 FixedMain
          }
      });
      *world.resource_mut::<Time>() = world.resource::<Time<Virtual>>().as_generic();        // L257：还原默认时间
  }
  ```
- `try_schedule_scope(FixedMain, …)`：把 `FixedMain` 从 `Schedules` 资源**临时摘下 → 闭包里任意次 `schedule.run` → 放回**，因此 while 多轮互不冲突、也允许嵌套驱动（与本仓库先期研究《bevy-app-schedule-reference.md》§1.5/§2.2 对 kairos_ecs `try_schedule_scope` 的描述同构）。
- `FixedMain` 图内唯一的系统 = `FixedMain::run_fixed_main`（`main_schedule.rs:L327` 注册；实现 L391-400）：读 `FixedMainScheduleOrder`，按 `FixedFirst → FixedPreUpdate → FixedUpdate → FixedPostUpdate → FixedLast` 逐个 `try_run_schedule`。
- **每帧步数判定与终止条件**：进入 while 前先 `accumulate_overstep(virtual_delta)`；while 条件 = `expend()` = "overstep 还够一整步吗"。成功一次扣一个 timestep 并推进 Fixed 时钟；终止于 `overstep < timestep`。因此每帧步数 = floor(本帧注入 delta + 上帧残留 overstep)/timestep，**无下限（0 步合法）、无上限常量**（隐含上限来自 max_delta，见 §3）。整个循环结束后普通 `Time` 还原为 Virtual 快照（`fixed.rs:L257`）——同一帧随后的 `Update` 里 `Res<Time>` 又是虚拟时间。
- 时间推进归属小结：Fixed 时钟的 elapsed 只在这些 while 步里前进；**不存在"每帧固定 +1 步"的兜底**。若连续若干帧虚拟 delta 都 < timestep，FixedUpdate 连续 0 步，属正常（文档 `fixed.rs:L35-40`："may run 0, 1 or more times during a single update"）。
- 顺带：`TimePlugin` 还给 `FixedPostUpdate` 挂 `signal_message_update_system` 并让消息注册表从 `Waiting` 起步（`lib.rs:L94-98`）——bevy message 系统按固定步调更新，属于 0.19 事件模型的配套细节，与步长驱动本体无关。

---

## 5. FixedMain 内容为空 / 零 overstep 时的行为（Q5）

- **驱动器每帧照跑**：`RunFixedMainLoop` 恒在主顺序表里（`main_schedule.rs:L224-232`），所以 `run_fixed_main_schedule` 每帧执行一次。零 overstep 时：`accumulate_overstep(0)`（`fixed.rs:L245-247`）→ `try_schedule_scope` 进入闭包 → `while …expend()` 首次即假 → FixedMain **一步不跑**（`fixed.rs:L250-255`）。成本 = 一次 `Time<Virtual>` 读取 + 一次 `Time<Fixed>` 写 + 一次 Schedules 摘/放与闭包调用；无任何系统被执行。
- **FixedMain 各子标签为空**（无人往 FixedFirst..FixedLast 加系统）：当 overstep 凑够一步时，FixedMain 会整轮跑（其唯一系统 `FixedMain::run_fixed_main` 顺序 `try_run_schedule` 五个空标签，`main_schedule.rs:L393-399`）——比 kairos 现在"每帧空跑一次 FixedUpdate"多出"驱动器系统 + 每步整轮 FixedMain（1 系统 + 5 空标签）"的开销，但仍无用户系统运行、无 panic/warn 路径（缺失标签被 `let _ =` 吞掉，`main_schedule.rs:L396`）。
- **无"首帧强制至少一步"**：Automatic 策略下第 1 帧 Real delta = 0（`real.rs:L99-107`）→ 注入 0 → 0 步。需要确定性/首帧即步进测试时用 `TimeUpdateStrategy::ManualDuration` 或 `FixedTimesteps`（`lib.rs:L118-122`；bevy 自己的测试即如此，见 `lib.rs` test 模块 `fixed_main_schedule_should_run_with_time_plugin_enabled`）。`FixedTimesteps(n)` 直接把每帧 Real delta 定为 `timestep × n`（`lib.rs:L181-183`），保证"每帧固定循环恰 n 次"。

---

## 6. 版本脆弱性提醒（Q6，非逐版直读，仅为防锚定）

- ≤0.13：`Time` 是单一资源（字段含 `time_scale` / `paused` / `max_delta`），固定更新是"主顺序表里每帧跑一次"的形态；记忆里的"`Time::time_scale()`、FixedUpdate 每帧 1 次"都是这个旧心智模型。
- 0.14：Time generic 化（Real/Virtual/Fixed context、`TimeUpdateStrategy`），改名 `pause/unpause`、`set_relative_speed`（`time_scale` 概念并进 Virtual context），`Time<Fixed>` 与 `overstep` 出现。
- 0.15-0.19：固定更新语义定形为"RunFixedMainLoop 标签 + FixedMain 子表 + 内部 0..n 次 while"；驱动系统主体从 bevy_app 迁到 bevy_time 的 `run_fixed_main_schedule`（v0.19.1 现状，本文 §4）；默认步长在 0.19 文档/代码中是 **64 Hz**。若照 0.14/0.15 时期资料核对本仓库代码，注意 label 归属、驱动函数所在 crate、以及"FixedUpdate 是否直接在主顺序表里"三处都已变过。

---

## 7. 与 kairos 现状的差异点（仅记录事实）

（本地路径相对 `KairosEngine/`，branch `bevy_fork`）

1. **主顺序表不同**：kairos `MainScheduleOrder.labels` 默认 = `[First, PreUpdate, FixedUpdate, Update, PostUpdate, Last]`（`kairos_engine/src/kairos_editor/schedule.rs:L99-113`，`FixedUpdate` 在 L106），无 `RunFixedMainLoop`、无 `SpawnScene`；bevy v0.19.1 是 `[First, PreUpdate, RunFixedMainLoop, Update, SpawnScene, PostUpdate, Last]`（`main_schedule.rs:L224-232`）。
2. **FixedUpdate 只是占位空标签**：kairos 注释明示"fixed-timestep semantics are out of scope for the skeleton"（`schedule.rs:L17-18`、L43-45）；`run_main` 每帧把所有 label 各 `try_run_schedule` 一次（`schedule.rs:L121-140`），即 FixedUpdate **每帧恰一次、无 0..n 语义**。
3. **无 FixedMain / 子阶段**：kairos 没有 `FixedMain`、`FixedMainScheduleOrder`、`FixedFirst…FixedLast`、`RunFixedMainLoop` 这些标签/资源（bevy 侧见 `main_schedule.rs:L94-104`、L106-160、L349-367）；bevy 里驱动循环的系统 `run_fixed_main_schedule`（`fixed.rs:L243-258`）在 kairos 全仓库不存在。
4. **引擎时间是完全不同的模型**：kairos `Time` 是 `Engine` 的普通结构体字段（`kairos_engine/src/kairos_editor.rs:L20-27`，字段 L21；`Engine::new` 里 `Time::new()`，L30-31），不是 ECS 资源；字段为 `start_time/pre_time/total_time/delta_time/total_frame/time_scale/paused`（`kairos_engine/src/timer.rs:L3-15`）。推进 = 每次调用取 `Instant::now()` 差（`timer.rs:L32-46`）；`paused` 时 delta 置 0（L39-42），否则 `raw_delta × time_scale`（L44-45）。**没有** `Time<Real>/<Virtual>/<Fixed>` 族、`overstep` accumulator、`max_delta` 钳制、`relative_speed/effective_speed`、`TimeUpdateStrategy`。
5. **推进时机与调度关系相反**：bevy 在 `First`（帧首）由 `time_system` 推进时间（`lib.rs:L82-87`），随后才跑其余阶段；kairos 帧序是 `KairosEngine::update` 先 `engine.update()`（跑 `Main` 调度 rails + `clear_trackers`，`kairos_editor.rs:L57-60`），再 `game.update(&mut engine)`（`kairos_editor.rs:L88-92`），而 `engine.time.update()` 在 `KairosGame::update` 开头手动调用（`kairos_engine/src/kairos_game.rs:L281-283`）——即 ECS 调度系统跑完后时间才推进，与 bevy "时间先行"相反。
6. **无渲染世界时间通道**：bevy 的 `TimeReceiver/TimeSender`、`create_time_channels`（`lib.rs:L125-142`）与 `TimeUpdateStrategy::Automatic` 的渲染世界链路（`lib.rs:L157-169`）在 kairos 无对应物。
