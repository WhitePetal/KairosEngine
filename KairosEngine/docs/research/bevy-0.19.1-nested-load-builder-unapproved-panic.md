# Bevy 0.19.1 `NestedLoadBuilder` 未批准路径 panic 核实（`UuidNotSupportedError`）

> 研究票：核实 `bevy_asset 0.19.1` 是否存在真实代码 bug —— `NestedLoadBuilder` 的延迟加载方法（`load` / `load_erased`，即 `loader_builders.rs::load_internal` 路径）在遇到未批准路径、且服务端 `UnapprovedPathMode` 拒绝该路径时，是否会在 `(&handle).try_into().unwrap()` 处 panic。
> 日期：2026-09-13。上游基准：`bevyengine/bevy` tag **`v0.19.1`**（逐文件直读，行号以该 tag 为准）。
> 方法：只使用一手来源（tag 对应的 raw 源码 + GitHub API）。**未运行 `cargo build/test`**；全部行为结论来自静态读码，并已由上游 issue/PR 交叉验证。

---

## 0. 结论速览

- **判定：panic bug（真实代码 bug，`v0.19.1` 含有此 bug）。**
- **决定性证据（一句话）**：`LoadContext.dependencies` 的声明类型是 `HashSet<ErasedAssetIndex>`（`loader.rs` L377），这强制 `(&handle).try_into()` 解析到 `impl TryFrom<&UntypedHandle> for ErasedAssetIndex`，其 `type Error = UuidNotSupportedError`（`id.rs` L436-444）；而 `load_with_meta_transform` 在拒绝未批准路径时恰好返回 `UntypedHandle::Uuid { .. }`（`server/mod.rs` L549-552），于是 `loader_builders.rs` L251 的 `.unwrap()` 必然 panic。
- **触发面**：不限于 `NestedLoadBuilder`。`LoadContext::load`（`loader.rs` L684）内部就是 `self.load_builder().load(path)`，所以任何 `AssetLoader` 里调用 `load_context.load(未批准路径)` 都会走到同一 unwrap。兄弟方法 `load_untyped`（L134）有同源 bug。
- **默认即命中**：`UnapprovedPathMode` 的默认值是 `Forbid`（`lib.rs` L282-283），因此这是默认配置下的 bug；`Deny` 且不 override 同样命中。`override_unapproved()` 只对 `Deny` 有效，对 `Forbid` 无效。
- **上游已确认并修复，但修复晚于 0.19.1**：issue [#21584](https://github.com/bevyengine/bevy/issues/21584)（`P-Crash` / `C-Bug`）已关闭；PR [#25435](https://github.com/bevyengine/bevy/pull/25435)（merge commit `aa3c9f02ca315171339e2e088a4775940bcedb6b`）于 **2026-08-19** 合并，把 `.try_into().unwrap()` 改为 `if let Ok(index) = ErasedAssetIndex::try_from(&handle)`。而 `v0.19.1` 的发布提交在 **2026-08-12**，早于修复，因此该 tag 仍是坏代码。

---

## 1. 基准与来源

- Tag 解析（`https://api.github.com/repos/bevyengine/bevy/git/ref/tags/v0.19.1`）：

  - `ref = refs/tags/v0.19.1`
  - `object.sha = b56fc29d3016e641754765244b5ba3f9cc504671`
  - `object.type = commit`

- 该提交即 "Release Bevy 0.19.1"，作者 Alice Cecile，提交时间 **2026-08-12T23:58:07Z**（`https://api.github.com/repos/bevyengine/bevy/commits/b56fc29d3016e641754765244b5ba3f9cc504671`）。
- 一手源码（raw，tag 固定为 `v0.19.1`）：
  - `https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_asset/src/loader.rs`
  - `https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_asset/src/loader_builders.rs`
  - `https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_asset/src/server/mod.rs`
  - `https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_asset/src/id.rs`
  - `https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_asset/src/handle.rs`
  - `https://raw.githubusercontent.com/bevyengine/bevy/v0.19.1/crates/bevy_asset/src/lib.rs`
- 上游跟踪：issue `https://github.com/bevyengine/bevy/issues/21584`、PR `https://github.com/bevyengine/bevy/pull/25435`、diff `https://github.com/bevyengine/bevy/pull/25435.diff`。
- 说明：`v0.19.x` 中该类型已从旧的 `NestedLoader` 改名为 `NestedLoadBuilder`；issue #21584 报的是 v0.18.0-dev 时代的 `NestedLoader`，同一条 unwrap 逻辑延续到了 0.19.1，只是行号不同。

---

## 2. 决定性事实 1：`LoadContext.dependencies` 的确切类型

`crates/bevy_asset/src/loader.rs` L368-377：

```rust
pub struct LoadContext<'a> {
    pub(crate) asset_server: &'a AssetServer,
    /// Specifies whether dependencies that are loaded deferred should be loaded.
    ///
    /// This allows us to skip loads for cases where we're never going to use the asset and we just
    /// need the dependency information, for example during asset processing.
    pub(crate) should_load_dependencies: bool,
    populate_hashes: bool,
    asset_path: AssetPath<'static>,
    pub(crate) dependencies: HashSet<ErasedAssetIndex>,
```

**字段类型是 `HashSet<ErasedAssetIndex>`，不是 `HashSet<UntypedAssetId>`。** 这是本题的枢轴事实。`insert(index)` 要求 `index: ErasedAssetIndex`，因此 `index` 的类型被钉死为 `ErasedAssetIndex`，从而决定了 `(&handle).try_into()` 必须解析到返回 `ErasedAssetIndex` 的那个 `TryFrom`。

（旁证：同一文件 `LoadedAsset` / `ErasedLoadedAsset` 的对应字段也是 `HashSet<ErasedAssetIndex>`，见 `loader.rs` L146 与 L222。）

---

## 3. 决定性事实 2：`(&handle).try_into()` 解析到哪个 impl，是否可失败

`crates/bevy_asset/src/id.rs` L436-444：

```rust
impl TryFrom<&UntypedHandle> for ErasedAssetIndex {
    type Error = UuidNotSupportedError;

    fn try_from(handle: &UntypedHandle) -> Result<Self, Self::Error> {
        match handle {
            UntypedHandle::Strong(handle) => Ok(Self::new(handle.index, handle.type_id)),
            UntypedHandle::Uuid { .. } => Err(UuidNotSupportedError),
        }
    }
}
```

错误类型定义（`id.rs` L456-458）：

```rust
#[derive(Error, Debug)]
#[error("Attempted to create a TypedAssetIndex from a Uuid")]
pub(crate) struct UuidNotSupportedError;
```

由于目标类型被 `dependencies.insert` 钉为 `ErasedAssetIndex`，这里解析到的是上面这个**显式**实现，`Error = UuidNotSupportedError`，对 `UntypedHandle::Uuid { .. }` 返回 `Err`。

### 为什么不是「永不会 panic」的 `Infallible` 路径

存在另一条可能被误认为会命中的路径：`handle.rs` L627-632 提供了

```rust
impl From<&UntypedHandle> for UntypedAssetId {
    #[inline]
    fn from(value: &UntypedHandle) -> Self {
        value.id()
    }
}
```

它会通过 std 的 blanket `impl<T, U: Into<T>> TryFrom<U> for T` 派生出一个 `TryFrom<&UntypedHandle> for UntypedAssetId`，其 `Error = Infallible` —— 若 `dependencies` 是 `HashSet<UntypedAssetId>`，则 `.unwrap()` 永不 panic，原假设就不成立。

但这在本 call site **不成立**：`dependencies` 是 `HashSet<ErasedAssetIndex>`（第 2 节），目标类型是 `ErasedAssetIndex`；而 `ErasedAssetIndex` 只有手写的可失败 `TryFrom<&UntypedHandle>`，并没有 `From<&UntypedHandle> for ErasedAssetIndex`。因此目标是可失败的 `ErasedAssetIndex` 实现，`Infallible` 那条路径不会被选中。所有其他 `(&handle).try_into().unwrap()` 调用点（如 `server/mod.rs` 内多处）之所以安全，是因为它们插入的是 `ErasedAssetIndex` 且上游保证返回 `Strong`；本处唯一的破例是 `load_with_meta_transform` 会返回 `Uuid`。

| 目标类型 | 命中的 impl | `Error` | `.unwrap()` 会 panic？ |
|---|---|---|---|
| `ErasedAssetIndex`（本题实际） | `impl TryFrom<&UntypedHandle> for ErasedAssetIndex`（`id.rs` L436） | `UuidNotSupportedError` | **会**（Uuid handle 时） |
| `UntypedAssetId`（若字段是它） | blanket via `impl From<&UntypedHandle> for UntypedAssetId`（`handle.rs` L627） | `Infallible` | 不会 |

---

## 4. 决定性事实 3：`load_with_meta_transform` 在未批准路径上的返回

`crates/bevy_asset/src/server/mod.rs` L529-555（`load_with_meta_transform` 头部与拒绝分支）：

```rust
    pub(crate) fn load_with_meta_transform<'a, G: Send + Sync + 'static>(
        &self,
        path: impl Into<AssetPath<'a>>,
        type_id: TypeId,
        type_name: Option<&str>,
        meta_transform: Option<MetaTransform>,
        guard: G,
        override_unapproved: bool,
    ) -> UntypedHandle {
        let path = path.into().into_owned();
        if path.path() == Path::new("") {
            error!("Attempted to load an asset with an empty path \"{path}\"!");
            return UntypedHandle::default_for_type(type_id);
        }

        if path.is_unapproved() {
            match (&self.data.unapproved_path_mode, override_unapproved) {
                (UnapprovedPathMode::Allow, _) | (UnapprovedPathMode::Deny, true) => {}
                (UnapprovedPathMode::Deny, false) | (UnapprovedPathMode::Forbid, _) => {
                    error!("Asset path {path} is unapproved. See UnapprovedPathMode for details.");
                    return UntypedHandle::Uuid {
                        type_id,
                        uuid: AssetId::<()>::DEFAULT_UUID,
                    };
                }
            }
        }
```

`UntypedHandle::Uuid` 的定义（`handle.rs` L477-486）：

```rust
pub enum UntypedHandle {
    /// A strong handle, which will keep the referenced [`Asset`] alive until all strong handles are dropped.
    Strong(Arc<StrongHandle>),
    /// A UUID handle, which does not keep the referenced [`Asset`] alive.
    Uuid {
        /// An identifier that records the underlying asset type.
        type_id: TypeId,
        /// The UUID provided during asset registration.
        uuid: Uuid,
    },
}
```

注意 `UntypedHandle::default_for_type`（`handle.rs` L491-496）同样返回 `Uuid` 变体：

```rust
    /// Returns the equivalent of [`Handle`]'s default implementation for the given type ID.
    pub fn default_for_type(type_id: TypeId) -> Self {
        Self::Uuid {
            type_id,
            uuid: AssetId::<()>::DEFAULT_UUID,
        }
    }
```

所以「空路径」与「未批准路径」两条分支都返回 `Uuid`。区别在于：`load_internal` 里空路径是**提前 `return`**，根本走不到 unwrap（见第 5 节）；未批准路径则继续往下走到 `.unwrap()`。

`AssetPath::is_unapproved`（`path.rs`）在路径上溯出 source 根（`..` 下溢/绝对路径）时返回 `true`，与本 bug 的触发条件一致。

---

## 5. 决定性事实 4：可达性走查（`load` / `load_erased` → unwrap）

完整调用链（全部为 tag `v0.19.1` 行号）：

```text
NestedLoadBuilder::load        (loader_builders.rs L91)
NestedLoadBuilder::load_erased (loader_builders.rs L103)
        │  两者都调用
        ▼
NestedLoadBuilder::load_internal (loader_builders.rs L224-254)
        │
        ├─ L231-234  path 为空 → 早退 return UntypedHandle::default_for_type(type_id)  // 不 panic
        │
        ├─ L235-243  if self.load_context.should_load_dependencies {
        │                asset_server.load_with_meta_transform(...)   // ← 未批准路径在此返回 Uuid
        │            } else {
        │                asset_server.get_or_create_path_handle_erased(...) // 一直 Strong
        │            }
        │
        └─ L249-252  let index = (&handle).try_into().unwrap();   // ← panic 点
                     self.load_context.dependencies.insert(index);
```

`loader_builders.rs` L224-254 原文：

```rust
    fn load_internal<'a>(
        self,
        type_id: TypeId,
        type_name: Option<&str>,
        path: AssetPath<'a>,
    ) -> UntypedHandle {
        let path = path.to_owned();
        if path.path() == Path::new("") {
            error!("Attempted to load an asset with an empty path \"{path}\"!");
            return UntypedHandle::default_for_type(type_id);
        }
        let handle = if self.load_context.should_load_dependencies {
            self.load_context.asset_server.load_with_meta_transform(
                path,
                type_id,
                type_name,
                self.meta_transform,
                (),
                self.override_unapproved,
            )
        } else {
            self.load_context
                .asset_server
                .get_or_create_path_handle_erased(path, type_id, type_name, self.meta_transform)
        };
        // `load_with_meta_transform` and `get_or_create_path_handle` always returns a Strong
        // variant, so we are safe to unwrap.
        let index = (&handle).try_into().unwrap();
        self.load_context.dependencies.insert(index);
        handle
    }
```

关键点：注释里写死的「always returns a Strong variant, so we are safe to unwrap」是错的 —— `load_with_meta_transform` 在 `(Deny, false)` / `(Forbid, _)` 下返回 `Uuid`。这条注释也是上游 PR 修复时一并删除的。

**`should_load_dependencies` 在正常加载下为 `true`**：`AssetServer::load` 的加载任务最终进入 `AssetServer::load_internal`，其中调用 `load_with_settings_loader_and_reader(..., load_dependencies = true, ...)`（`server/mod.rs` L871-877 的实参 `true`），进而 `LoadContext::new(..., should_load_dependencies = true, ...)`。只有 asset processor 路径才会传 `false`（此时走 `get_or_create_path_handle_erased`，不命中本 bug）。因此常规 `AssetServer::load` 触发的嵌套加载确实走 `load_with_meta_transform` 分支。

**没有上游 guard 拦截未批准情况**：`load_internal` 只在空路径早退；未批准判断完全在 `load_with_meta_transform` 内部完成（先 `error!` 日志、再返回 `Uuid`），调用方不会提前返回，因此必然落到 L251 的 unwrap。

**`override_unapproved` 不能救 `Forbid`**：`load_builder().override_unapproved()` 只把 `override_unapproved` 置 `true`，匹配分支对 `(Deny, true)` 放行，但对 `(Forbid, _)` 仍拒绝 —— 即 `Forbid` 下即使 override 也会返回 `Uuid` 并 panic。

**`LoadContext::load` 同样中招**：`loader.rs` L684-686 的

```rust
    pub fn load<'b, A: Asset>(&mut self, path: impl Into<AssetPath<'b>>) -> Handle<A> {
        self.load_builder().load(path)
    }
```

说明这个 bug 不限于直接使用 `NestedLoadBuilder`；任何现有 `AssetLoader` 中的 `load_context.load(...)` 传未批准路径都会触发。

---

## 6. 决定性事实 5：默认 `UnapprovedPathMode` = `Forbid`

`crates/bevy_asset/src/lib.rs` L271-284：

```rust
/// The default value is [`Forbid`](UnapprovedPathMode::Forbid).
///
/// See [`AssetPath::is_unapproved`](crate::AssetPath::is_unapproved)
#[derive(Clone, Default)]
pub enum UnapprovedPathMode {
    /// Unapproved asset loading is allowed. This is strongly discouraged.
    Allow,
    /// Fails to load any asset that is unapproved, unless [`LoadBuilder::override_unapproved`] is
    /// used.
    Deny,
    /// Fails to load any asset that is unapproved.
    #[default]
    Forbid,
}
```

`#[default]` 在 `Forbid` 上，故 `UnapprovedPathMode::default() == Forbid`。`AssetPlugin::default()` 亦使用 `UnapprovedPathMode::default()`。**结论：这是默认配置下就会命中的 bug**，未批准路径不需要用户显式配置 `Deny`。

---

## 7. 上游是否已注意到 / 已修复

- **Issue**：`bevyengine/bevy#21584`，"NestedLoader can panic from UnapprovedPathMode"，标签含 `C-Bug`、`P-Crash`、`A-Assets`，2025-10-17 提交，2026-08-19 关闭。报告者给出的 panic 现场（v0.18.0-dev）：

  ```text
  thread 'IO Task Pool (3)' panicked at .../crates/bevy_asset/src/loader_builders.rs:321:42:
  called `Result::unwrap()` on an `Err` value: UuidNotSupportedError
  ... ERROR bevy_asset::server: Failed to load asset 'primary', asset loader '...' panicked
  ```

  这正是本题推演出的 `UuidNotSupportedError`。在 0.19.1 里，对应的 `load_internal` unwrap 行是 L251（`load_untyped` 的是 L134）。

- **修复 PR**：`bevyengine/bevy#25435`，"Don't panic when a nested load rejects an unapproved path"，2026-08-19 由 `alice-i-cecile` 合并，merge commit `aa3c9f02ca315171339e2e088a4775940bcedb6b`。PR 描述明确写道：
  - 嵌套加载未批准依赖时 panic，被 catch 成 `AssetLoaderPanic`，导致外层资产也加载失败；
  - **默认配置（`Forbid`）即受影响**；`Deny` 无 override 同样；
  - 顶层 `AssetServer::load` 对同一路径只是返回 default handle，不 panic，二者行为不一致。

- **修复 diff**（`https://github.com/bevyengine/bevy/pull/25435.diff`，`crates/bevy_asset/src/loader_builders.rs`）把该处改为：

  ```rust
  // `load_with_meta_transform` returns a default `Uuid` handle when it refuses to start the
  // load, for example because the path is unapproved. There is no load to track as a
  // dependency in that case, and the reason has already been logged.
  if let Ok(index) = ErasedAssetIndex::try_from(&handle) {
      self.load_context.dependencies.insert(index);
  }
  ```

  并新增两个回归测试：`unapproved_path_deny_does_not_panic_in_nested_load` 与 `unapproved_path_deny_does_not_panic_in_nested_untyped_load`（后者覆盖 `load_untyped`）。

- **与 0.19.1 的先后关系**：`v0.19.1` 发布提交日期 **2026-08-12**，修复合并日期 **2026-08-19** —— 修复在 tag 之后，因此 `v0.19.1` 必定仍含该 bug（并且我们直接从 tag 源码读到了 `.try_into().unwrap()`，不依赖日期推断）。

---

## 8. 影响与边界（事实陈述）

- panic 发生在 `AssetLoader` 的 async 执行体内；`AssetServer::load_with_settings_loader_and_reader` 用 `AssertUnwindSafe(...).catch_unwind()` 包裹 loader（`server/mod.rs` L1649 起），因此通常表现为 **外层资产以 `AssetLoadError::AssetLoaderPanic` 失败**，而不是直接进程崩溃；若 `panic = "abort"`，则会直接 abort。无论哪种，都是「本应被干净拒绝的未批准路径」变成了 panic。
- 顶层 `asset_server.load(unapproved_path)` 不 panic（返回 default handle），嵌套加载 panic —— 两条路径行为不一致（PR 描述亦确认此点）。
- 兄弟入口 `NestedLoadBuilder::load_untyped`（`loader_builders.rs` L112，unwrap 在 L134）走 `load_unknown_type_with_meta_transform`，其拒绝分支返回 `Handle::default()`（`server/mod.rs` L635-641），同样得到 `Uuid` → 同样 panic。本调研聚焦的 `load` / `load_erased` 之外，这一点值得一并记录。

---

## 9. 最终判定

**panic bug** —— 0.19.1 真实存在该 panic；唯一决定性证据：`LoadContext.dependencies: HashSet<ErasedAssetIndex>`（`loader.rs` L377）使 `(&handle).try_into()` 解析到 `Error = UuidNotSupportedError` 的 `TryFrom<&UntypedHandle> for ErasedAssetIndex`（`id.rs` L436-444），而未批准路径下 `load_with_meta_transform` 返回 `UntypedHandle::Uuid`（`server/mod.rs` L547-552），于是 `loader_builders.rs` L251 的 `.unwrap()` panic（上游 issue #21584 复现、PR #25435 修复，修复晚于 0.19.1）。
