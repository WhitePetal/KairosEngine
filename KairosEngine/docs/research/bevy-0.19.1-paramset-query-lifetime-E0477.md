# Bevy 0.19.1 `ParamSet<(Query<&T>,)>` 生命周期 E0477 排查记录

> 研究票：定位 `kairos_ecs` fork 里 `ParamSet<(Query<&T>, …)>` 必须显式标注 `'static`，而上游 `bevy_ecs 0.19.1` 无需标注的根因。
> 日期：2026-09-14。上游基准：`bevy_ecs-0.19.1`（本地 registry 直读）。方法：受控复现 + 逐文件归一化 diff + 二分替换（含一个并行 sub-agent）。**未完成根因定位**，本文记录已确认的事实、悖论与下一步。

---

## 0. 结论速览

- **触发点**：`kairos_transform/src/systems.rs::sync_simple_transforms` 的 `ParamSet<(Query<(&LocalTransform, &mut GlobalTransform), …>, Query<(Ref<LocalTransform>, &mut GlobalTransform), …>)>` 报 E0477：`(&LocalTransform, &mut GlobalTransform)` 必须满足 `'static`。
- **义务来源（已确认）**：`kairos_ecs/src/system/system_param.rs:304`

  ```rust
  unsafe impl<D: QueryData + 'static, F: QueryFilter + 'static> SystemParam for Query<'_, '_, D, F> {
      type State = QueryState<D, F>;
  ```

  经 `ParamSet<'w,'s,T: SystemParam>` 的结构体 bound 传导为 `D: 'static` → `&T: 'static`。
- **上游能编译完全省略写法（已两次验证）**：bevy 0.19.1 在 stable `1.97.1` 与 nightly `1.99.0` 下，`fn sig<'a>(_: ParamSet<(Query<&'a A>,)>)` 都通过；kairos 在两版 rustc 下都失败。
- **触发差异尚未定位**：所有相关 trait/struct/impl 定义经归一化 diff 逐行确认为语义一致（只有 `std/core/alloc`、`crate::/bevy_ecs::`、`kairos_ptr/bevy_ptr` 等重命名差异）。行为差异被锁定为「bevy 对 `ParamSet` 结构体 bound 的 WF 检查是惰性的，kairos 是急切的」，但具体源码触发点未找到。

---

## 1. 义务链（机制）

```
ParamSet<(Query<D, F>,)>   // 命名该类型
  → 结构体 bound  T: SystemParam        （T = (Query<D, F>,)）
    → 元组 impl    (Query<D,F>,): SystemParam
      → Query impl  Query<D,F>: SystemParam
        → impl 的 where  D: QueryData + 'static
          → D: 'static                  // E0477
```

`D = (&LocalTransform, &mut GlobalTransform)` 时，`D: 'static` 要求两个引用生命周期 `'1`、`'2` 都 `: 'static`，而它们是 elided 出来的 late-bound 生命周期，无法满足。

用 `RUSTFLAGS="-Zverbose-internals"` 可确认义务是 `&ReLateParam(…'a) A: 'static`；`-Ztreat-err-as-bug` 的 backtrace 确认该义务在 `rustc_hir_analysis::check::wfcheck::check_type_wf`（函数签名类型 WF 检查）阶段产生。

---

## 2. 为什么不能简单删 `D: 'static`

删除后错误转移到 `QueryState<D,F>: 'static`（来自 `SystemParam::State: Send + Sync + 'static`）。原因：

- `QueryState<D,F>` 的字段是 `fetch_state: D::State`、`filter_state: F::State`。
- Rust 的 ADT outlives 规则：`struct S<D> { fetch: D::Assoc }` 即使 `D::Assoc` 归一化后是 `'static`（如 `ComponentId`），`S<D>: 'static` 仍要求 `D: 'static`。这是保守规则，与字段具体类型无关。
- 已实测：给 `WorldQuery::State` 加 `+ 'static` 也不改变这个结果（结构体仍要求 `D: 'static`）。

要真正去掉 `D: 'static`，唯一办法是让 `QueryState` **不再携带 `D`/`F` 泛型参数**（把 `D::State`/`F::State` 整个类型擦除为 `Box<dyn Any + Send + Sync>` 之类），这会波及整个 query 子系统，属破坏性重构，且偏离上游实现——不是合理的最小修复。

因此：**bevy 能通过，只能解释为 bevy 对 `ParamSet` 结构体 bound 的 WF 检查没有走到 Query impl 的 `D: 'static`，而不是 bevy 没有这条 `'static`。** bevy 源码里这条 `'static` 是逐字节一样的。

---

## 3. 已排除的因素

| 因素 | 结论 |
|---|---|
| `Component` derive / 手写 `impl Component` | 手写同样失败，排除 |
| rustc 版本（stable 1.97.1 vs nightly 1.99.0） | 两侧都试过，排除 |
| `variadics_please` 2.0.0 vs 1.1.0 | 换 1.1.0 仍失败，排除 |
| `#![no_std]` / std | 最小复现加 `#![no_std]` 仍失败，排除 |
| 新旧 trait solver（`-Znext-solver=globally`） | 两侧都试过，排除 |
| `WorldQuery::State` 加 `+ 'static` | 实测仍要求 `D: 'static`，排除 |
| `query` 模块根缺失 derive 宏 re-export | 补上仍失败，排除 |
| `component/info.rs`、`register/required/clone/constants.rs`、`world/unsafe_world_cell/deferred_world/identifier.rs`、`change_detection/traits.rs`、`query/access(_iter).rs` | 逐一替换（bevy→kairos）后 E0477 仍在，排除 |

已逐一确认为**语义一致**（归一化 diff 只看 `trait/struct/impl/type` 签名，剔除注释、`std/core/alloc`、命名）：`system/system_param.rs`、`system/query.rs`、`query/world_query.rs`、`query/fetch.rs`、`query/filter.rs`、`query/state.rs`、`component.rs`（根）、`component/info.rs`（`ComponentId`）、`change_detection/params.rs`、`change_detection/tick.rs`、`UnsafeWorldCell`、`DeferredWorld`、`SystemMeta`、`WorldId`。

---

## 4. 关键行为差异（悖论）

在 bevy 0.19.1 里：

- `assert_param::<(Query<&'a A>,)>()`（显式 `P: SystemParam` bound）→ **失败**，要求 `'a: 'static`。
- `fn sig<'a>(_: ParamSet<(Query<&'a A>,)>)`（命名 `ParamSet` 类型，结构体 WF bound）→ **通过**。

即：**同一个义务 `(Query<&'a A>,): SystemParam`，走显式 bound 会失败，走 `ParamSet` 结构体 WF 命名却通过**。kairos 里两者都失败。

这指向 rustc 对「结构体 where 子句的 WF 义务」与「显式 trait bound」使用了不同的（更懒的）解析；但 `ParamSet` 结构体与 `SystemParam` trait 定义两侧逐字节一致，故触发该差异的根源仍在某个尚未定位的 fork 差异点。

---

## 5. 关键旁证

对比全 crate 的 `'static` 出现位置，kairos 的**测试/文档**里多出：

```
ParamSet<(Query<&'static mut A>,)>
ParamSet<(Query<&'static mut A>, Query<&'static B>)>
Query<&'static mut Health, With<Enemy>>
```

而 bevy 对应位置是 `Query<&mut A>` / `Query<&mut Health>`（无 `'static`）。说明 **fork 作者当年就撞上这个问题，只在测试/文档里用 `'static` 绕过，没有修根因**。当前 WIP 的 `kairos_transform/src/helper.rs` 里 `Query<'w, 's, &'static ChildOf>` / `&'static LocalTransform` 也是同一绕过写法。

---

## 6. 当前状态

- 本次会话未改调用处、未改 `kairos_ecs` 侧实现（所有实验改动已还原）。
- WIP（transform 传播系统 + `Parallel` 从 `kairos_ecs` 迁到 `kairos_tasks`）已随本次一并提交。
- `kairos_transform/src/systems.rs::sync_simple_transforms` 目前仍是**未加 `'static` 的失败写法**，`cargo check -p kairos_transform` 会报 E0477（这是预期，用于继续复现）。
- 已知可编译的临时写法（用户明确不想用）：`Query<'static, 'static, D, F>`（只内联 world/state 两个生命周期，data 借用照旧省略）。

---

## 7. 建议的下一步

1. **rustc trait solver 取证**：对 bevy 和 kairos 各跑一次
   `RUSTFLAGS="-Znext-solver=globally -Zdump-solver-proof-tree=all" cargo +nightly check -p …`
   对比 `ParamSet<(Query<&'a A>,)>` 的 WF obligation 链，看 bevy 在哪一步跳过 `D:'static`。
2. **全 crate 结构级 diff（variance 方向）**：只保留 `struct/trait/impl/type/where` 签名做归一化，重点比对 `QueryState`/`Query`/`ParamSet`/`UnsafeWorldCell`/`SystemMeta` 里 `PhantomData` 用法、`&mut` vs `&`、`UnsafeCell` 包裹等会影响 variance 的字段。
3. 若需先落地可编译，唯一已知手段仍是调用处 `Query<'static, 'static, D, F>` 标注（用户已拒绝，故未采用）。

---

## 8. 复现步骤

```rust
// kairos_ecs 探针（在 kairos_ecs 内作为 test / 独立 crate 均可）
use kairos_ecs::prelude::*;
#[derive(Component)] struct A;
fn sig<'a>(_: ParamSet<(Query<&'a A>,)>) {}
fn nested(mut set: ParamSet<(Query<&A>,)>) { let _ = set.p0(); }
```

- kairos：`cd KairosEngine && cargo check -p kairos_ecs --test paramset_probe` → 期望 E0477。
- bevy 对照：给 `kairos_ecs` 的 dev-dependencies 加 `bevy_ecs = "=0.19.1"`，同样两个函数用 `bevy_ecs::prelude::*` → 期望通过。
- 诊断：`RUSTFLAGS="-Zverbose-internals" cargo +nightly check …`。
