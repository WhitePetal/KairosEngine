# Wayfinder #157 — audio 模块 kira/ECS 调用点与迁移事实清点

- 关联 issue: [WhitePetal/KairosEngine#157](https://github.com/WhitePetal/KairosEngine/issues/157)（map: #156）；产出将供 audio 迁移 handoff 使用。
- 基准: 分支 `bevy_fork` @ `cbd87ac`（clean tree；工作区仅剩 `KairosEngine/Library/asset_registry.toml` 本地改动与未跟踪 `.scratch/`，与本文无关）。
- 本文路径约定: 均相对于 engine workspace 根（`kairos_engine/`、`kairos_ecs/`、`kairos_transform/`、`docs/` 所在层 = 仓库 `KairosEngine/KairosEngine/`）；带行号处基于上述 commit。
- 方法: `.codegraph/` 有索引，`codegraph explore` 用于交叉验证符号/调用链；源码级事实以 `read_file`/`grep` 静态阅读完成；**未运行 cargo**（不改变编译结论判断，仅记录事实）。kira 0.12.1 源码在 sandbox 中不可读（`~/.cargo/registry` 位于工程外），故 kira 侧事实以 **docs.rs 对 kira 0.12.1 的 rustdoc 页面（含 auto-trait 清单，构建日期 2026-05-25）为准**；rustdoc 即由发布源码生成，auto-trait 节反映真实字段推导。凡"当前能否编译"判断一律标注为需 handoff 前 `cargo check -p kairos_engine` 验证。

---

## A. kira 0.12 事实

### A0. 版本与依赖

- 声明: `kairos_engine/Cargo.toml` L62 `kira = { version = "0.12.0", features = ["serde"] }`（caret 语义 = `>=0.12.0, <0.13`）。
- **实际解析: `Cargo.lock` L2591-2593 `name = "kira"`, `version = "0.12.1"`**（源 crates.io）。
- 其它依赖（docs.rs 0.12.1 页面）: glam 0.33 / mint 0.5 / atomic-arena / rtrb / triple_buffer / send_wrapper / symphonia(可选) / cpal 0.17(可选，默认开) —— engine 侧另有直连 `mint = "0.5"`（`kairos_engine/Cargo.toml` L66）。

### A1. 目标类型 trait 矩阵（docs.rs 0.12.1 rustdoc）

| 类型 | 模块路径（engine 导入点同源） | Send | Sync | Clone | Copy | Debug | 备注 |
|---|---|---|---|---|---|---|---|
| `StaticSoundHandle` | `kira::sound::static_sound` | ✔ | ✔ | ✘ | ✘ | ✔ | 无 Drop（停止后由音频线程回收） |
| `ListenerId` | `kira::listener` | ✔ | ✔ | ✔ | ✔ | ✔ | 另 `Eq + Hash`；`From<&ListenerHandle>` |
| `ListenerHandle` | `kira::listener` | ✔ | ✔ | ✘ | ✘ | ✔ | 有 `Drop`：drop = 移除该 listener |
| `SpatialTrackHandle` | `kira::track`（re-export） | ✔ | ✔ | ✘ | ✘ | ✔ | 有 `Drop`：drop = 移除该 spatial 轨道 |
| `SendTrackHandle` | `kira::track`（re-export） | ✔ | ✔ | ✘ | ✘ | ✔ | 有 `Drop`；`From<&SendTrackHandle> for SendTrackId` |
| `ReverbHandle` | `kira::effect::reverb` | ✔ | ✔ | ✘ | ✘ | ✔ | rustdoc 未列 Drop/Clone |
| `AudioManager<DefaultBackend>` | crate 根 | ✔ | ✔ | ✘ | ✘ | ✘（无 Debug） | `Send where B: Send`、`Sync where B: Sync`；`DefaultBackend` = `backend::cpal::CpalBackend`（type alias），而 **CpalBackend 自身 Send+Sync**（rustdoc auto-trait 实测）→ 具体化后 Send+Sync 成立 |

要点（供 C1）:
- 句柄全是私有字段的不透明结构（rustdoc `{ /* private fields */ }`），无公开构造。`Clone` 只在 **id** 上存在（`ListenerId`/`SendTrackId` 均 `Copy + Clone + Eq + Hash`）；控制型句柄（`ListenerHandle`/`SpatialTrackHandle`/`SendTrackHandle`）带 `Drop` 副作用、不 Clone —— 这是"id 作簿记、句柄作 RAII 控制端"的设计：空间音频簿记应走 id（引擎 `SpatialAudioVolumeTrackKey.listener_id`/`Tracks` 索引就是这么用的），句柄则必须由**单一持有者**保管（丢失 = 无声移除资源）。
- `AudioManager` 自身 Send+Sync 依赖后端 B 的 bound；`CpalBackend` 实测 Send+Sync，`MockBackend` 未逐一验证 auto-trait（仅记录存在性，见 A2）。

### A2. 无设备 mock backend 事实

**kira 0.12.1 提供**: `kira::backend::mock` 模块，含 `MockBackend`（"A backend that does not connect to any lower-level audio APIs, but allows manually calling `Renderer::on_start_processing` and `Renderer::process`"）与 `MockBackendSettings`（docs.rs 模块页，标注 "Useful for testing and benchmarking"）。即 0.12 有设备无关的测试/基准 backend，**并非只能靠 cpal 真设备**；engine 当前未使用（`AudioEngine::new` 固定 `AudioManager::<DefaultBackend>`，`kairos_engine/src/audio.rs` L40）。

### A3. C1 判定

kairos_ecs fork 的 `Component` trait bound 为 **`Send + Sync + 'static`**（`kairos_ecs/src/component.rs` L530）。上表 7 类中：
- 6 个句柄/id 类型全部 `Send + Sync + 'static`（非泛型无生命周期）→ **可直接作为组件字段/组件本体**，无需包装；
- `AudioManager<DefaultBackend>` 亦 Send+Sync，但它不是组件候选（它住在 `AudioEngine` 结构体字段里，`audio.rs` L27-30），即使入组件也满足 bound。
- **结论: C1 成立** —— 无任何 kira 类型成为 `#[derive(Component)]` 的绊脚石。工程约束只在别处：组件字段里持有句柄意味着该组件不可 Clone（句柄不 Clone），因此 `SpatialAudioVolume`/`BackgroundAudio` 不能靠 derive Clone 复制实体；`Drop` 句柄（`ListenerHandle` 等）在 ECS 里由 `SpatialAudioTracks.all_listeners: HashMap<ListenerId, (ListenerHandle, bool)>`（`spatial.rs` L126）统一持有，实体只是携带 `ListenerId`，实体删除 ≠ kira listener 删除（要由 `update_listeners_inner` 的 `retain` 收敛，见 C1）。

---

## B. kairos_ecs 新 API 事实（engine 非 schedule 代码视角）

### B0. 组件登记: derive 即注册，无需手动登记

- `kairos_ecs::component` 模块 **同路径再导出** derive: `pub use kairos_ecs_macros::Component;`（`kairos_ecs/src/component.rs` L17-21）。因此 engine 模块只需一行:
  `use kairos_ecs::component::Component;`（trait + derive 宏同一名字导入；derive 支持属性 `component`/`require`/`relationship`/`relationship_target`/`entities`，`kairos_ecs/macros/src/lib.rs` L830-833）。
- **登记是隐式的**，engine crate 无需手动 `register_component`:
  - spawn bundle 时 `Bundle::component_ids → components.register_component::<C>()`（`kairos_ecs/src/bundle/impls.rs` L33-35；元组 Bundle L92-96）;
  - `World::query` 建 `QueryState` 时 `D::init_state(world)` 亦会注册（`kairos_ecs/src/query/state.rs` L213-221 / L255-260）。
  - 仅当你想在空 world 上 `try_query` 才需先 `world.register_component::<T>()`（`world.rs` L408-410；文档例 L1931-1939）。
- 工作示例: `kairos_transform/src/local_transform.rs` L1 `use kairos_ecs::component::Component;`、L26 `#[derive(Component, Debug, Clone, Copy, PartialEq)]`。**当前 engine crate 内没有任何类型 derive Component**——所有 ECS 候选仍是旧式注释 `// impl Component for X {}`（audio 4 个 + `Camera`/`LODMesh`/`MaterialComponent`/`Collider`/`RigidBody`，见 C4 清单）；恢复时把注释换成 derive 即可，类型字段已全部 Send+Sync（`AudioAssetHandle = Arc<AssetHandle<...>>`，`audio/asset/audio.rs` L24）。

### B1. 查询 API 形态（非 schedule、持 `&mut World` 代码）

`kairos_ecs` 不再有 `World::query_mut`（旧注释代码的调用点全部失效）。现 API:

| API | 签名/位置 | 语义 |
|---|---|---|
| `World::query::<D>()` | `world.rs` L1867-1869，`&mut self` → `QueryState<D, ()>` | 建一次状态，可复用；**QueryState 无生命周期参数**（字段仅 `world_id: WorldId` 等，`query/state.rs` L107-129）→ 不长期借用 world |
| `World::query_filtered::<D, F>()` | L1891-1893 | 带 filter（`With/Without/Changed/...`，`kairos_ecs::query` 导出） |
| `World::try_query` | L1942-1944，`&self` → `Option<QueryState>` | 组件未注册时返回 None |
| `QueryState::iter(&world)` | `query/state.rs` L1200-1202 | **只读**查询；`&mut self` + `&World`；item = `D::ReadOnly::Item` |
| `QueryState::iter_mut(&mut world)` | L1209-1211 | 可变查询；**`&mut World`**；item 中的 `&mut T` 位置为 **`Mut<'w, T>`** |
| `QueryState::{single, single_mut}` | L1866-1871 / L1883-1891 | 恰好一个实体，否则 panic（引擎旧的 `.next()` 语义 ≠ single，见 P2） |
| `QueryState::{get, get_mut}` | L956-962 / L1047-1053 | 按 entity 取 |
| `QueryState::is_empty` | L522-535 | 替代旧代码 `iter.len() == 0` |
| `QueryData for &mut T` | `query/fetch.rs` L2499-2509 | `Item = Mut<'w,T>`，`ReadOnly = &T`；要求 `T: Component<Mutability = Mutable>`（derive 默认 Mutable） |
| `Mut<'w, T>` | `change_detection/params.rs` L919-922 | `Deref`/`DerefMut` 由 `change_detection_impl!`/`change_detection_mut_impl!` 宏生成（L924-925）；可 `&mut *v`/`v.deref_mut()` 递给 `&mut T` 帮助函数 |

借用的实操要点（单次 `update` 多 pass 迭代）:
- 每个 pass = `let mut q = world.query::<D>(); for ... in q.iter_mut(&mut world)`；pass 结束（迭代器/收集结果 drop）后 borrow 即释放，**多个顺序 pass 可以在同一函数里连续做**（旧注释代码 spatial.rs `update_listener_audios` 的三个 pass 结构可直接保留）。
- **`collect::<Vec<_>>()` 会延续 `&mut World` 借用到 Vec 存活期**（item 生命周期 = `&mut world` 借用）——收集含 `Mut<..>`/`&mut` 的 Vec 后，函数其余部分不能再触碰 `world`（旧代码后续确实不碰，见 C1 `update_listener_audios`）。
- 只读数据（`&SpatialAudioReverbBound, &SpatialAudioReverb`）用 `iter(&world)`，item 是普通引用，无 Mut 包装。
- `spawn_batch` 返回惰性 `SpawnBatchIter`（`world/spawn_batch.rs` L26-35，逐个 `next()` 才生成，`world.rs` L1405-1411）——**必须消费返回值**（`.collect::<Vec<_>>()`/`for_each`），旧注释代码 `engine.world.spawn_batch(...)` 裸调用不生效。

### B2. 旧 → 新替换示例（每条对应一种被注释形态）

**P1 — 多实体 `&mut` pass + Mut deref**（`audio/spatial.rs` L303-316 注释体）
```rust
// 旧（已失效）
// let volumes = world
//     .query_mut::<(&LocalTransform, &mut SpatialAudioVolume)>()
//     .into_iter()
//     .map(|(_, volume)| volume);
// for mut volume in volumes {
//     Self::update_audio_volume_state(assets_server, delta_time,
//         self.config.audio_volume_leaving_duration, volume.deref_mut());
// }
// 新
let mut volumes_query = world.query::<(&LocalTransform, &mut SpatialAudioVolume)>();
for (_, mut volume) in volumes_query.iter_mut(&mut world) {
    // volume: Mut<SpatialAudioVolume>；`&mut volume` 走 DerefMut（或保留 volume.deref_mut()）
    Self::update_audio_volume_state(assets_server, delta_time,
        self.config.audio_volume_leaving_duration, &mut volume);
}
```

**P2 — 单实体 `.next()`**（`audio.rs` L83 注释体；同形态也见于 `kairos_game.rs` render/`game_window.rs` L224-251 相机取用）
```rust
// 旧
// let background = world.query_mut::<&mut BackgroundAudio>().into_iter().next();
// 新
let mut bg_query = world.query::<&mut BackgroundAudio>();
if let Some(mut background) = bg_query.iter_mut(&mut world).next() {
    // background: Mut<BackgroundAudio>，字段经 Deref/DerefMut 访问/赋值
}
// 注意：single/single_mut 语义是“恰好一个否则 panic”，与旧 .next()（任意第一个）不同，勿直接替换
```

**P3 — 只读双组件**（`audio/spatial.rs` L264-268 注释体）
```rust
// 旧
// let reverbs = world
//     .query_mut::<(&SpatialAudioReverbBound, &SpatialAudioReverb)>()
//     .into_iter();
// 新
let mut reverbs_query = world.query::<(&SpatialAudioReverbBound, &SpatialAudioReverb)>();
for (bound, reverb) in reverbs_query.iter(&world) { /* bound/reverb 均为 & 引用 */ }
```

**P4 — 拷出 + 按 priority 顶 k 截断**（`audio/spatial.rs` L167-185 注释体）
```rust
// 旧: query_mut::<(&LocalTransform, &mut SpatialAudioListenerComponent)>() + len()==0 守卫
//     + .map(|(trans, listener)| (*trans, *listener)).collect::<Box<_>>() + select_nth_unstable_by
// 新（读路径即可，&mut 无必要——数据是 Copy 拷出）
let mut listeners_query = world.query::<(&LocalTransform, &SpatialAudioListenerComponent)>();
if listeners_query.is_empty() { return; }
let mut listeners = listeners_query
    .iter(&world)
    .map(|(trans, listener)| (*trans, *listener))   // 两类型均 Copy
    .collect::<Box<_>>();
// select_nth_unstable_by/切片逻辑原样保留（Vec 方法，与 ECS API 无关）
```

**P5 — 渲染查询 + for_each**（`kairos_game.rs` L315-325 注释体）
```rust
// 旧: engine.world.query_mut::<(&LocalTransform, &LODMesh, &MaterialComponent)>().into_iter()
//     + .for_each(...) 内 trans.compute_local_matrix() 喂 graphics_command.draw(...)
// 新
let mut renderers_query = engine.world.query::<(&LocalTransform, &LODMesh, &MaterialComponent)>();
renderers_query.iter(&engine.world).for_each(|(trans, lod, mat)| {
    graphics_command.draw(lod.lod0.clone(), mat.material.clone(), trans.compute_local_matrix());
});
```

**P6 — 距离排序候选收集**（`audio/spatial.rs` L359-386 注释体；三个顺序 pass 之一）
```rust
// 旧: 过滤 state(Playing/Paused) → map (dist_sq, trans, volume) → filter < cut_off → collect::<Vec<_>>
// 新
let mut volumes_query = world.query::<(&LocalTransform, &mut SpatialAudioVolume)>();
let mut volumes = volumes_query
    .iter_mut(&mut world)
    .filter(|(_, volume)| matches!(volume.state, AudioState::Playing | AudioState::Paused))
    .map(|(trans, volume)| (float3::distance_sq(listener.position, trans.position), trans, volume))
    .filter(|(dst, _, _)| *dst < cut_off_dst_sq)
    .collect::<Vec<_>>();
// 借用注意: 此 Vec 存活期间 world 保持 &mut 借用；旧代码函数尾前不再查 world，可直接照搬。
// 若未来需要在持有此 Vec 的同时再查 world，只能改为先收集 (dist, Entity) 再按需 get_mut。
```

---

## C. 注释/禁用代码块清点

### C1. `kairos_engine/src/audio/spatial.rs`（678 行，模块声明 L32-34）

每函数状态总表（"活" = 函数体未被注释；"无活调用者" = 调用链在 `update` 被掐断）:

| 函数 | 行号 | 函数体 | 活调用者 | 恢复后签名是否需变 |
|---|---|---|---|---|
| `SpatialAudioTracks::update` | L159-188 | **全注释** | `AudioEngine::update`（活，`audio.rs` L78-79） | 否（`&mut self, assets_server, manager, world, delta_time`） |
| `update_listeners_inner` | L190-257 | 活 | 无（被注释的 `update` 调用） | 否，但入参 `listeners: &mut [(LocalTransform, SpatialAudioListenerComponent)]` 依赖两类型 `Copy`（L194）；`world: &mut World` 仅透传给 `update_reverbs` |
| `update_reverbs` | L259-294 | **半注释**（L260-262 early-out 活；L264-293 查询体注释） | `update_listeners_inner` L236/L250（活） | 否（`listener: &mut ListenerInfo, world: &mut World`）；恢复体需 P3 只读查询 |
| `update_audios` | L296-335 | **前半注释**（L303-316 卷状态 pass）；L320-334 对每个 listener 的循环活 | 无 | 否；恢复部分需 P1 |
| `update_listener_audios` | L337-408 | **全注释**（L347-407，三个顺序 pass） | `update_audios` L321-334（活，但整链无活入口） | 否（自由函数，参数照旧）；恢复部分 = P1/P6 |
| `update_audio_volume_state` | L410-487 | **活**（含 `AudioState::Paused => todo!()` L468-470） | 无（被注释 pass 调用） | 不变（不触 world） |
| `update_audio_track_state` | L489-504 | 活 | `update_audio_volume_state`（活） | 不变 |
| `free_audio_volume_track` | L506-526 | 活 | 无 | 不变（不触 world） |
| `play_audio_volume_in_track` | L528-613 | 活 | 无 | 不变（不触 world；经 `assets_server.get::<AudioAssetsSystem>` L590-592） |
| `leaving_audio_volume_in_track` | L615-655 | 活 | 无 | 不变（不触 world） |

语义/迁移要点:
- **入口整体断链**: `update` 全注释 → `AudioEngine::update` 每帧实际空转；`update_listeners_inner`/`update_audios`/`play_audio_volume_in_track` 等"活"代码全部无活调用者（当前 cargo check 的 dead-code 警告来源）。恢复顺序 = `update` 重写查询（P4）→ 两条下游链自动复通。
- `update`（L166-187）旧语义: 查全部 listener → `len()==0` 早退 → 按 `priority` 降序 `select_nth_unstable_by` 截到 `self.listener_infos.capacity()`（注意: 预算取自 `Vec::with_capacity(max_listener_count)` 的 **capacity**，`new()` L138 保证其 = `config.max_listener_count`）→ 传入 `update_listeners_inner`。
- `update_listeners_inner`（L190-257）: 活代码。每 listener 可用 track 预算 `max_track_count / len`（L197），变少时 `truncate` 现有轨（L199-206）；`retain` 移除已消失 listener（L209-213）；`all_listeners` 重建 be_ref 标记，未标记的 `ListenerHandle` 因 `retain` 被 drop = kira listener 被移除（L217-219/L256）。**语义注意**: kira listener 生命周期跟 `SpatialAudioTracks` 走，不跟实体走——实体只持 `ListenerId`；实体消失后要等本函数跑过才真正删除 kira 侧 listener。
- `update_reverbs`（L259-294）恢复体: 对每个命中 `bound.contains_point(listener.position)` 的 reverb 写 `SendTrackHandle::set_volume(Value::FromListenerDistance(...))` + `ReverbHandle::{set_feedback,set_damping,set_mix}`（L273-292，注意 L290-292 与 L287-288 对 damping 重复 set，属旧代码冗余）。
- `update_listener_audios`（L337-408）三 pass 语义: ① 每个 volume `free_audio_volume_track`（释放本 listener 名下 `Leaved` 轨道）; ② 取 state∈{Playing,Paused} 且 `distance_sq < cut_off` 的前 k 个（k = `per_listener_track_count`），逐个 `play_audio_volume_in_track`，失败则 `leaving...`; ③ 圈外但持轨者进入 leaving。**语义注意点**:
  - `free_audio_volume_track`（L506-526）末尾无条件 `volume.audio_handles.clear()`（L525）——`audio_handles` 是全 volume 共享（不分 listener），一个 listener 释放轨道会清掉可能属于另一 listener 仍播放中的句柄，破坏完成判定（见下）。
  - `update_audio_volume_state` Playing 完成判定（L442-466）要求 `audio_handles.len() == audios.len()` 且每个非 Stopped 才算未完成；若同一 volume 同时在 2 个 listener 的 top-k 里，会 push `audios.len() × listener 数` 个句柄 → `len != audios.len()` 恒成立 → **永不 Completed**。属既有设计缺陷/未决语义，恢复时需留意。
  - `play_audio_volume_in_track`（L528-613）: 已持本 listener 轨道 → 只 `set_position` 即返回 true（L561-567）；新分配轨道但 `volume.state != Playing`（如 Paused，而 Paused 分支是 `todo!()`）→ 仍记账 `Playing(track_key)` 但不播声、返回 true（L586-611 只在 Playing 时 `track.play`）。`Volume.play` 用 `audio.sound_data.start_position(PlaybackPosition::Seconds(playing_time))`（L594-598），即每个 clip 从 volume 累计时间续播。
- 文件尾 `to_mint_vec3/to_mint_quaternion`（L658-678）已是**活代码且自注释**: 旧 `float3/quaternion → mint` From 随 kairos_math 拆分移除（注释引用 #139/#144），恢复任意被注释 kira 调用点（`set_position/set_orientation`）都走这两个显式转换（`spatial.rs` 全部旧 mint From 依赖点已由注释内 `set_position(...)` 的旧形态改成 to_mint 形式？**注意**: 被注释的 `update`/`update_listeners_inner` 等内部调用是 `to_mint_vec3(trans.position)`（L227-228 活代码），恢复注释体时保持该形式）。

### C2. `kairos_engine/src/audio.rs` — `AudioEngine::update`（L76-111）

- 当前: `update(&mut self, assets_server: &mut AssetsServer, world: &mut World, delta_time: f32)` 只调 `spatial_tracks.update(...)`（空转），随后 **L81-110 整段 BackgroundAudio 状态机注释**（`// update backgroun`）。
- 注释体需求: 旧 P2 单实体查询 `world.query_mut::<&mut BackgroundAudio>()...next()` → 新 P2 形态；`match background.state` 五态推进: `Created→(auto_play? WaitLoading)`、`WaitLoading→assets_server.get(&background.audio) 成功→self.manager.play(audio.sound_data.clone())`、`Playing→handle.state()==Stopped→Completed`、`Paused→todo!()`、`Completed→noop`。依赖: `AudioState` 变体齐全（见 C4）、`AudioAsset.sound_data: StaticSoundData`（见 C4）、`manager.play(sound_data)`（L72，活）。`self.manager.play` 与 `world` 借用互不冲突（不同对象）。
- 语义注意: 状态机把"进入 Playing"绑定在 `assets_server.get` 命中；`get` 返回 `Option<&AudioAsset>`（`asset_loader/assets.rs` L105-109），加载未完成时每帧轮询——与 `update_audio_volume_state` 的 WaitLoading 轮询同构。

### C3. `kairos_engine/src/kairos_game.rs`

- **L27-131 整段注释**: 三个旧式 `System` 包装（`AudioUpdateSystem`/`PhysicsUpdateSystem`/`InputUpdateSystem`，含 `SystemMeta`/`change_tick` 仪式）。属于 pre-fork 的 System API；当前 fork 用 `Engine::update` 跑 schedule rails（`kairos_editor.rs` L67-70）+ `KairosGame::{update,render}` 手写钩子，恢复时**不再需要**这段（直接调用子系统 `update` 即可）。
- **场景 spawn（L156-270，即 ticket 所指 ~156-270）**:
  - 资产加载活代码: `pad.audio`/`blip.audio` 经 `assets_server.load::<AudioAssetsSystem>`（L167-171）；`_mesh`/`material` 同理（L160-165）。这些以 `_` 前缀命名的局部 handle 目前**只被注释代码引用**。
  - L177 相机实体 `spawn((cam_trans, camera))`（`Camera` 需 derive；`cam_trans = LocalTransform::look_at(...)` L175）。
  - L179-187 **listener 实体**: `engine.audio_engine.create_listener()`（活）取 `ListenerId` → `spawn((cam_trans, SpatialAudioListenerComponent { listener_id, priority: 100 }))`。
  - L189-190 `BackgroundAudio::new(_background_audio, true)` + spawn。
  - L192-204 reverb: `SpatialAudioReverb::new(20.0, -12.0, 24.0, 0.2, 0.2, 0.6, AABB{...})` 返回 `(SpatialAudioReverb, SpatialAudioReverbBound)` **二元组**（`spatial_audio_reverb.rs` L27-47），`spawn(tuple)` 即同时挂两个组件。
  - L206-226 **volume 网格** `spawn_batch`: 40×40 网格，元组 = `(LocalTransform, LODMesh, Material, SpatialAudioVolume)`；**注意注释代码用 `Material::new(...)`（L222）——现名 `MaterialComponent`**（`graphics/material_component.rs`），恢复时需改名；`SpatialAudioVolume::new(smallvec![blip_audio.clone()], true, rand::random_range(0.0..5.0))`（L215-217，`playing_time` 种子 0~5s）；`spawn_batch` 在新 API **惰性**（见 B1）需消费。
  - L256-267 plane/ball 实体 spawn 注释（`plane_collider`/`ball_rigid_body` 当前在 ECS 外，仅物理引擎内部使用——物理侧 `set_position` 活，L234/L246）。
- **`update`（L272-312）**: 活代码只剩读时间（L276-277）；L279-311 三个旧 System 驱动块注释 → 新形态为直呼: `engine.audio_engine.update(&mut engine.assets_server, &mut engine.world, delta_time)`（并 physics/input 类似）。
- **`render`（L314-326）**: 函数体注释（P5 渲染查询）；`KairosGame::render(&self, engine: &mut Engine, graphics_command: &mut GraphicsCommand)` **有活调用者**（scene_window.rs L416 `game.render(engine, &mut graphics_command)`；game_window.rs L240 亦注释了同类调用）。

### C4. 侧查

- **孤儿文件确认**: `kairos_engine/src/audio/spatial_audio_listener.rs` = **3 行空文件**（无内容、无模块声明——`audio.rs` L21-25 只声明 `audio/audio_ext/background/consts/spatial`，`spatial.rs` L32-34 声明 `spatial/` 下三子模块）→ 未被编译的死文件。真实组件在 `audio/spatial/spatial_audio_listener.rs`（`SpatialAudioListenerComponent { listener_id: ListenerId, priority: u8 }`，`derive(Debug, Clone, Copy)`，L5-9；`// TODO! impl Component` L10-11）。迁移时勿理旧空文件。
- **`SpatialAudioVolume` 现状**（`audio/spatial/spatial_audio_volume.rs`）: 无任何 derive（非 Debug/Clone/Copy）；字段 `audios: SmallVec<[AudioAssetHandle; 4]>`、`audio_handles: SmallVec<[SpatialSoundHandle; 4]>`、`auto_play`、`state: AudioState`、`track_states: Vec<SpatialAudioVolumeTrackState>`、`playing_time: f32`（L33-40）。`SpatialSoundHandle` = `enum { Some(StaticSoundHandle), Err }`（L9-12，无 Debug —— 播放失败的占位）。`SpatialAudioVolume::new(audios, auto_play, start_time)`（L44-54）置 `state = Created`、`playing_time = start_time`。`SpatialAudioVolumeTrackState` = `Playing(key)/Leaving(leaving)/Leaved(key)`（L26-31，均 Copy）。
- **`AudioState` 定义**: `kairos_engine/src/audio/audio.rs` L159-166（`derive(Debug, Clone, Copy)`），五态 `Created / WaitLoading / Playing / Paused / Completed`（Paused 两个状态机里都是 `todo!()` 占位: `spatial.rs` L468-470、`audio.rs` L105）。同文件还定义了 `AudioAsset { pub sound_data: StaticSoundData }`（L153-157，Debug+Clone）与可序列化设置 `SerializedAudioAsset*`（L12-151）。
- **pad/blip 经 `AudioAssetsSystem` 的运行时形态**（`asset_loader/assets/asset/audio.rs`）: `.audio` 是 toml（`meta.source_path` + settings）→ Loader 读源文件 → `StaticSoundData::from_cursor` → `apply_to_static_sound_data`（`audio/audio.rs` L116-150）→ **运行时资产 = `AudioAsset { sound_data: StaticSoundData }`**（L94）。`AudioAssetHandle = Arc<AssetHandle<AudioAssetsSystem>>`（L24）；`AssetsServer::get::<T>(&self, &AssetHandle<T>) -> Option<&T::AssetType>`（`asset_loader/assets.rs` L105-109）。所以"loaded asset 是否暴露 `sound_data: StaticSoundData`"——**是**，且 `StaticSoundData` Clone 廉价（shared 数据，docs 顶部示例明示 clone 不复制内存）。编辑器已有活范例: `AudioInspector::play`（`kairos_editor/ui/inspector/audio.rs` L397-433）`get → audio.sound_data.clone()` → `engine.audio_engine.play_sound(sound_data)`，并把 `StaticSoundHandle` 存 `Option` 后 `stop()`（L448-453）。

---

## D. bevy_audio parity 注记（本轮形态 → bevy_audio 概念映射，仅事实）

以 bevy_audio（0.15/0.16 命名时期，与 kairos_ecs fork 对齐的 bevy 系）为参照:

| 本轮（KairosEngine）形态 | bevy_audio 概念 |
|---|---|
| `Engine` 字段 `audio_engine: AudioEngine`，内含 kira `AudioManager` + `SpatialAudioTracks`（`kairos_editor.rs` L23、`audio.rs` L27-30） | `AudioPlugin` 装入的资源: `AudioOrbiter`（0.16 起持有 kira `AudioManager`）/`AudioOutput`（主轨句柄层）——同为"单 manager 全局持有"模型 |
| `KairosGame::update(&mut self, engine: &mut Engine)` 手写逐帧驱动 `AudioEngine::update(&mut self, assets_server: &mut AssetsServer, world: &mut World, delta_time)`（现为注释态，见 C2/C3） | AudioPlugin 在 schedule（Update/PostUpdate）注册的每帧 system（播放队列、sink 状态推进、spatial 同步） |
| 组件直接携带 kira 播放句柄/状态: `BackgroundAudio.handle: Option<StaticSoundHandle>`、`SpatialSoundHandle::Some(StaticSoundHandle)`、`SpatialAudioVolumeTrackState` 簿记 | `AudioPlayer(Handle<AudioSource>)`（播放请求）→ 播放后落到 `AudioSink`（持 kira 播放句柄，可 stop/volume/state 查询）——bevy 把"请求"与"运行句柄"分两个组件/阶段，引擎则直接内联在业务组件 |
| `SpatialAudioListenerComponent { listener_id, priority }` 实体 + `AudioManager::add_listener`（`spatial.rs` L143-157，L179-187 spawn） | `SpatialListener` 组件标记 listener 实体（新版本含 listener priority）；内部同为 kira `add_listener` |
| volume 实体 = `LocalTransform` + `SpatialAudioVolume`，位置直接喂 `set_position/set_orientation`（mint 转换，`spatial.rs` L227-228/L658-678） | `SpatialAudioSource` 标记发射体；bevy 在 PostUpdate 用 `GlobalTransform` 同步 kira 位置 |
| `AssetsServer::get::<AudioAssetsSystem>` + `AudioAsset { sound_data: StaticSoundData }`、`AudioAssetHandle = Arc<...>` | `Assets<AudioSource>` + `Handle<AudioSource>`（`AudioLoader` 解码 kira `StaticSoundData`） |
| 无对应公开 API（引擎独有）: 每 listener reverb send-track + AABB zone 切换、top-k priority/距离轨道预算、leaving 淡出状态机、`AudioState` 五态 | bevy_audio 不暴露 kira 的 track/send-track/effect（reverb 等）控制面，也不内置"同 volume 多 listener 轨道复用/预算"逻辑 |

事实性差异点（只列差异，不给建议）: bevy_audio 的 kira manager/轨道细节封装在 plugin 内部资源中，引擎本轮把同一层（manager + 每 listener 轨道/效果句柄）放在 `SpatialAudioTracks` 并允许业务组件直接持有 kira 句柄；两者共享"每帧手动推进 + 资产解码为 `StaticSoundData`"的底层事实。

---

## E. 关键结论（handoff 用事实汇总）

1. **C1 成立**: 目标 kira 类型全 `Send+Sync+'static`；`AudioManager<DefaultBackend>` 亦 Send+Sync（`B=CpalBackend`，后端实测 Send+Sync）。组件化无需包装/feature；唯一工程约束是句柄不 Clone、部分句柄 Drop 带资源移除副作用 → 句柄须单持有者，簿记走 id。
2. **kira 0.12.1 有 `MockBackend`**（`kira::backend::mock`，无设备、手动驱动 Renderer；含 `MockBackendSettings`）。
3. **组件登记零成本**: `use kairos_ecs::component::Component;` + `#[derive(Component)]` 即够（derive 与 trait 同路径再导出）；spawn/query 时自动注册。engine crate 现无任何 derive(Component) 类型，audio 四个候选仍是注释 `impl Component`。
4. **旧 `world.query_mut::<Q>().into_iter()` 全部失效** → `world.query::<Q>()`（`QueryState`）+ `.iter(&world)`（只读）/`.iter_mut(&mut world)`（可变，item 中 `&mut T` = `Mut<T>`）。多顺序 pass 可同函数连续做；`collect::<Vec<_>>()` 延续 world 借用到 Vec 存活；`.next()` 单实体可用但 ≠ `single()` 语义。
5. **无干净新 API 等价物的点**: 无（查询面全部有替换）；真正"没有干净等价"的是**旧数据流设计缺陷**（见 C1: `audio_handles.clear()` 跨 listener、完成判定 `len==audios.len()` 多 listener 永不完成、Paused 两处 `todo!()`），恢复注释代码时应一并处理/确认。
6. **签名必须动的地方少**: `spatial.rs` 十个函数签名全部可保留（含 `update_listeners_inner` 的 `&mut [(LocalTransform, SpatialAudioListenerComponent)]`，前提 Copy）；要改的是"实体侧": 组件类型补 derive、`kairos_game.rs` 注释 spawn 的 `Material::new`→`MaterialComponent::new`、`spawn_batch` 需消费返回值、恢复 `KairosGame::update` 直呼 `audio_engine.update(...)`。
7. **被注释代码依赖的活符号全部健在**: `AudioAsset.sound_data: StaticSoundData`、`AssetsServer::get`、`to_mint_*`、`AudioState`、`SpatialAudioVolume::new`、`AudioManager::play`——恢复无资产/API 侧缺口；仅需按 B2 各形态改写查询并给组件加 derive。

### 统计速览

- A（kira）: 类型矩阵 7 项（6 句柄/id 全 Send+Sync+Debug、仅 id Clone；AudioManager Send/Sync 依赖后端且具体类型成立）；`MockBackend` 存在；来源 = docs.rs 0.12.1 rustdoc 页面。
- B（kairos_ecs 新 API）: 登记 = derive 即自动；查询 API 表 9 项；替换示例 6 条（P1 多实体 &mut/Mut、P2 单实体 next、P3 只读、P4 拷出+顶 k、P5 渲染 for_each、P6 距离排序 collect）。
- C（注释块）: spatial.rs 10 函数（4 个需恢复注释体: update/update_reverbs/update_audios 前半/update_listener_audios；6 个活但无调用者）；audio.rs `AudioEngine::update` 1 块（BackgroundAudio 状态机）；kairos_game.rs 场景 spawn 4 块 + 旧 System 3 块 + render 1 块；孤儿空文件 1（`audio/spatial_audio_listener.rs` 3 行）。
- D: 映射表 6 行 + 差异 2 行。
- 验证缺口: 未运行 cargo；恢复注释后需 `cargo check -p kairos_engine` 复核（当前 HEAD 基线可编译、仅警告）。
