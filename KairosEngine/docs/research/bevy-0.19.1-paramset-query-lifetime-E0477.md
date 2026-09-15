# Bevy 0.19.1 `ParamSet<(Query<&T>,)>` 生命周期 E0477 排查记录

> 研究票：定位 `kairos_ecs` fork 里 `ParamSet<(Query<&T>, …)>` 必须显式标注 `'static`，而上游 `bevy_ecs 0.19.1` 无需标注的根因。
> 日期：2026-09-14 → 2026-09-15。上游基准：`bevy_ecs-0.19.1`（本地 registry 直读）。方法：受控复现 + 逐文件归一化 diff + 二分替换（含一个并行 sub-agent）+ rustc 源码取证。**根因已定位**（见 §0.1 与 §4）。

---

## 0. 结论速览

- **根因（已定位）**：这不是 `kairos_ecs` 的源码 bug，而是 **rustc 1.97.1 内建的一个「implied-bounds 兼容性 hack」，按字面量 crate 名 `bevy_ecs` + 类型名 `ParamSet` 匹配**。详见 §0.1 与 §4。
- **触发点**：`kairos_transform/src/systems.rs::sync_simple_transforms` 的 `ParamSet<(Query<(&LocalTransform, &mut GlobalTransform), …>, Query<(Ref<LocalTransform>, &mut GlobalTransform), …>)>` 报 E0477：`(&LocalTransform, &mut GlobalTransform)` 必须满足 `'static`。
- **义务来源（已确认）**：`kairos_ecs/src/system/system_param.rs:304`

  ```rust
  unsafe impl<D: QueryData + 'static, F: QueryFilter + 'static> SystemParam for Query<'_, '_, D, F> {
      type State = QueryState<D, F>;
  ```

  经 `ParamSet<'w,'s,T: SystemParam>` 的结构体 bound 传导为 `D: 'static` → `&T: 'static`。
- **上游能编译完全省略写法（已两次验证）**：bevy 0.19.1 在 stable `1.97.1` 与 nightly `1.99.0` 下，`fn sig<'a>(_: ParamSet<(Query<&'a A>,)>)` 都通过；kairos 在两版 rustc 下都失败。
- **触发差异（已定位）**：所有相关 trait/struct/impl 定义经归一化 diff 逐行确认为语义一致（只有 `std/core/alloc`、`crate::/bevy_ecs::`、`kairos_ptr/bevy_ptr` 等重命名差异）——因为差异根本不在 `.rs` 里，而在 rustc 的 metadata 驱动的特判里。行为差异「bevy 惰性 / kairos 急切」的触发点是 **crate 名**。

---

## 0.1 根因：rustc 的 `bevy_ecs::ParamSet` implied-bounds 兼容 hack

rustc 1.97.1 在 `compiler/rustc_trait_selection/src/traits/query/type_op/implied_outlives_bounds.rs` 里对**恰好叫做 `ParamSet`、且定义在恰好叫做 `bevy_ecs` 的 crate 里的类型**做特殊处理（`ContainsBevyParamSet` 访问者）：

```rust
// 伪码（实际源码见该文件）
fn visit_ty(&mut self, t) {
    if let ty::Adt(def, _) = t.kind() {
        if tcx.item_name(def.did()) == sym::ParamSet
            && tcx.crate_name(def.did().krate) == sym::bevy_ecs
        {
            return ControlFlow::Break(());   // 命中 hack
        }
    }
}
```

命中后，rustc 把已经登记的 `TypeOutlives` 区域义务（即 `&'a A: 'static`）**转成 implied bounds（假设）而不是强制执行**——这就是 bevy 侧「惰性」的真正来源。`rustc_hir_analysis::check::wfcheck` 里是一个两阶段区域解析：先按正确语义跑（这里会出 E0477），失败后再带 hack 重跑一次。

**三个独立实证**：

1. 对 bevy 复现 crate 加 `-Zno-implied-bounds-compat`（关闭该 hack）→ 立即报 E0477，与 kairos 完全一致。
2. 把一个**忠实最小复现**的 `[package] name` 改成 `bevy_ecs`（其余源码一字不动）→ E0477 消失、编译通过；改回 `kairos_ecs` / 任意其它名 → E0477 复现。
3. `rustup run nightly rustc -Z help` 明确列出 `-Zno-implied-bounds-compat=val -- disable the compatibility version of the implied_bounds_ty query`。

因此：`kairos_ecs` 之所以「必须标 `'static` 而 bevy 不用」，唯一原因是 **fork 把 crate 名从 `bevy_ecs` 改成了 `kairos_ecs`**，使 rustc 的特判不再命中。

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

## 4. 关键行为差异（已解开）

在 bevy 0.19.1 里：

- `assert_param::<(Query<&'a A>,)>()`（显式 `P: SystemParam` bound）→ **失败**，要求 `'a: 'static`。
- `fn sig<'a>(_: ParamSet<(Query<&'a A>,)>)`（命名 `ParamSet` 类型，结构体 WF bound）→ **通过**。

即：同一个义务 `(Query<&'a A>,): SystemParam`，走显式 bound 会失败，走 `ParamSet` 结构体 WF 命名却通过。kairos 里两者都失败。

**这不再是一个悖论**：`assert_param` 的显式 bound 不经过 `ParamSet` 类型本身，因此不命中 rustc 的 `bevy_ecs::ParamSet` 特判；而 `fn sig` 的参数类型里出现了 `ParamSet`，bevy 侧命中特判、被降级为 implied bounds；kairos 侧因 crate 名不符而不命中、按正确语义报 E0477。

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

- 根因已定位：rustc 的 `bevy_ecs::ParamSet` implied-bounds 兼容 hack（见 §0.1）。
- 本次会话未改调用处、未改 `kairos_ecs` 侧实现（所有实验改动已还原）。
- `kairos_transform/src/systems.rs::sync_simple_transforms` 目前仍是**未加 `'static` 的失败写法**，`cargo check -p kairos_transform` 会报 E0477。

## 7. 修复选项（按推荐度）

1. **类型擦除（真正的修法，破坏性重构）**：让 `QueryState` 不再携带 `D`/`F` 泛型参数（`D::State`/`F::State` 擦除为 `Box<dyn Any + Send + Sync>` 之类），从而 `QueryState: 'static` 不再要求 `D: 'static`。这是 bevy 上游本应做但一直没做的（rustc 注释里就是 `FIXME: Once we fix bevy, …`）；副作用是波及整个 query 子系统，改动面大。
2. **调用处 `'static` 标注（工作区级绕过）**：`Query<'static, 'static, D, F>`（或 `&'static T` data 借用）。用户已明确拒绝，但这是不改 crate 名前提下唯一「零重构」手段。
3. **改 crate 名**：把 fork 的 `[package] name` / `extern crate self as` 改回 `bevy_ecs` 以命中 hack——不合理，破坏 fork 独立性，仅作反证用。

结论：**在「不标 `'static`、不破坏性重构、不改 crate 名」的约束下，`kairos_ecs` 侧没有可落地的修复**。要真正根治必须走选项 1（类型擦除）。

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
