# Handoff: ECS Change Detection

> Generated 2026-07-27. Full Bevy-aligned implementation, split into 6 tickets.

## Background

KairosEngine 需要为 ECS 添加组件变更检测能力，作为 Hierarchy 面板的前置依赖（D-004）。经过完整的 Bevy 调研和设计对齐，确定了 per-system `last_run` + dual-tick (`ComponentTicks { added, changed }`) 的技术方案。

设计文档：`docs/research/bevy-change-detection.md`

## Architecture Decisions

- **存储**：`ComponentTicks { added: Tick, changed: Tick }` 每组件实例两个 u32，Bevy 一致
- **比较**：`Tick::is_newer_than(last_run, this_run)` — wrapping 算术，Bevy 一致
- **写入标记**：`Mut<T>::DerefMut` 自动调用 `set_changed(this_run)`，Bevy 一致
- **过滤**：`Changed<T>` / `Added<T>` 通过 `Fetch::filter_fetch(&self, row) -> bool` 实现，**零装箱**
- **System**：每个 System 持有独立的 `last_run` tick，Bevy 一致
- **并发**：`change_tick: AtomicU32` + `UnsafeWorldCell`，Bevy 一致
- **不做的**：tick 溢出扫描（`check_change_ticks`）、`Spawned` 过滤器、`changed_by` debug — 留 Phase 2

## Published Tickets

Parent: #50 (ECS Change Detection)

| # | Title | Blocked by |
|---|-------|-----------|
| #109 | T1 — Tick 基础类型 + 存储层 | None |
| #110 | T2 — Fetch/Query 改造 + Changed/Added 过滤器 | #109 |
| #111 | T5 — UnsafeWorldCell | #109 |
| #112 | T4 — System 层 | #110 |
| #113 | T3 — Mut\<T\>/Ref\<T\> Access Wrappers | #110 |
| #114 | T6 — 调用点迁移 + 集成测试 | #113 |

执行顺序（frontier first）：#109 -> #110 -> (#111, #112, #113) -> #114

## Key Design Details (not captured in tickets)

### Tick::is_newer_than 核心比较逻辑

```rust
pub fn is_newer_than(self, last_run: Tick, this_run: Tick) -> bool {
    let ticks_since_change = this_run.relative_to(self).0.min(MAX_CHANGE_AGE);
    let ticks_since_system = this_run.relative_to(last_run).0.min(MAX_CHANGE_AGE);
    ticks_since_change < ticks_since_system
}
```

"组件被修改的时间距离现在" 是否小于 "系统上次运行的时间距离现在"？是 → detected。

### System 生命周期

```
initialize(): last_run = change_tick.relative_to(Tick::MAX)  // 负无穷
run(): this_run = world.increment_change_tick()
      注入 (last_run, this_run) -> 所有查询
      执行 system 体
      last_run = this_run
```

### 多 System 并发

- 只读 system（`&T` 查询）可以并行：通过 `UnsafeWorldCell` 共享 `&World`
- 写 system（`&mut T` 查询）必须独占
- 调度器根据 `Fetch::for_each_borrow` 提供的 `(TypeId, is_unique)` 信息判断冲突
- `AtomicBorrow` 已在 column 级别做运行时冲突检测

### 过滤器不装箱

`Fetch::filter_fetch(&self, row) -> bool` 是 Sized 结构体上的普通方法，零开销。`Changed<T>` 和 `Added<T>` 是通过 `Fetch` 泛型实例化的编译期类型。

## Affected Files (broad stroke)

| File | T1 | T2 | T3 | T4 | T5 | T6 |
|------|:--:|:--:|:--:|:--:|:--:|:--:|
| `ecs/change_detection/tick.rs` | ✓ | | | | | |
| `ecs/change_detection/access.rs` | | | ✓ | | | |
| `ecs/change_detection/filters.rs` | | ✓ | | | | |
| `ecs/change_detection/mod.rs` | ✓ | ✓ | ✓ | | | |
| `ecs/table.rs` | ✓ | | | | | |
| `ecs/world.rs` | ✓ | | | | | ✓ |
| `ecs/unsafe_world_cell.rs` | | | | | ✓ | |
| `ecs/system.rs` | | | | ✓ | | |
| `ecs/component_tuple/query_tuple.rs` | | ✓ | ✓ | | | |
| `ecs/component_tuple.rs` | | ✓ | ✓ | | | |
| `audio.rs` | | | | | | ✓ |
| `audio/spatial.rs` | | | | | | ✓ |
| `physics.rs` | | | | | | ✓ |
| `kairos_editor/ui/game_window.rs` | | | | | | ✓ |

## Suggested Skills

- `codebase-design` — deep-module 接口设计，适用于 Tick/ComponentTicks 的公开接口设计
- `tdd` — T1 到 T6 每张票按测试驱动开发
- `research` — 如有疑问，参考 `docs/research/bevy-change-detection.md` 中的 Bevy 源码引用
- `diagnosing-bugs` — 如果编译错误或运行失败，按诊断流程处理

## Related Documents

- `docs/plan/hierarchy/handoff.md` — 上游 hierarchy 面板交接文档
- `docs/plan/hierarchy/hierarchy-phase1-requirements.md` — D-004 明确变更检测为前置依赖
- `docs/research/bevy-change-detection.md` — Bevy 源码级调研
- `docs/plan/hierarchy/README.md` — 主索引
