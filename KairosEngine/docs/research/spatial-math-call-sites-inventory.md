# Wayfinder #140 — engine 内 math/spatial 调用点与 rapier 互转清点

- 关联 issue: [WhitePetal/KairosEngine#140](https://github.com/WhitePetal/KairosEngine/issues/140)（前置: #138 规划；产出将供 #141 类型/API 设计与迁移 handoff 使用）
- 基准: 分支 `bevy_fork` @ `f8f9583`（clean tree）
- 本文路径约定: 均相对于 engine workspace 根（`kairos_engine/`、`kairos_ecs/`、`docs/` 所在层）；带行号处基于上述 commit。
- 方法: `.codegraph` 目录存在但无索引（`codegraph explore` 提示不可用），因此本清点以 `grep`/`read_file` 静态阅读完成；**未运行 cargo build/test**。凡是"当前能否编译"的判断一律标注为存疑，需 handoff 前 `cargo check` 验证。

---

## A. spatial::Transform 使用点

### A0. 定义（供对照）

`kairos_engine/src/spatial/transform.rs`

- `pub struct Transform { pub position: float3, pub rotation: quaternion, pub scale: float3 }`（L5-10），`derive(Debug, Clone, Copy)` —— **无 serde/rkyv**。
- L3-4: `// TODO! // #[derive(... Component)]` —— 明确计划成为 ECS `Component`（注释掉了）。
- 方法: `new`（L12-19）、`look_at(eye,target,up)`（L21-29，内部 `quaternion::from_look` + `scale=float3::ONE`）、`get_local_to_world()`（L31-33，= `float4x4::trs(position,rotation,scale)`）。
- `kairos_engine/src/spatial.rs` L1 声明坐标系: **右手系 Y-up, -Z forward**；`float3` 的 `RIGHT/UP/FORWARD` 常量（`math/vec.rs` L523-527）与其一致。

### A1. `kairos_engine/src/graphics/camera.rs` — 相机 view（重）

- 读字段: `rotation`（L35 `transform.rotation.to_float4x4()` 后取列向量构造 right/up/forward）、`position`（L39）。**不读 scale**。
- 矩阵: 手工构造 view（L42-52，`transpose([r|u|f])` + 平移 `-R*p`，平移用 `math::dot` 投影）; `get_view_projection_matrix`（L71-73）= P × V。**不用 `get_local_to_world()`**，且与 `float4x4::trs` 语义等价（旋转+平移，scale 假定 1）。
- 构造/修改: 否，纯消费 `Transform`（按值传入，`Copy`）。
- local/world: 文档注释明确 "camera's **world-space** Transform"（L28）；view = inverse(camera_world)。隐含 camera Transform 就是世界变换（root 场景假设）。
- TODO: L15-16 `// impl Component for Camera {}`。

### A2. `kairos_engine/src/kairos_editor/ui/scene_camera.rs` — 编辑器轨道相机（重）

- **构造** Transform: `transform()`（L134-136）= `Transform::look_at(self.position(), self.pivot, float3::UP)`（pivot/eye 为世界坐标）。
- 矩阵: `view_projection()`（L138-143）临时 `Camera::new(...)` + `get_view_projection_matrix(t)`。
- 不读/不改任何已有 Transform 的字段；`position()/forward()/right()/_up()` 均用 `math::{normalize,cross,length,lerp}` 直接算 float3（L111-132）。
- local/world: 世界空间轨道相机；编辑器 3D 预览（scene_window/inspector 预览）全部经由它出 vp。
- 序列化: `SceneCamera` 本身 `#[derive(Serialize, Deserialize)]`（L9），内部存 `pivot: float3`（L17）—— **依赖 float3 的 serde 实现**。

### A3. `kairos_engine/src/audio/spatial.rs` — 空间音频（重，但大量被注释）

- 导入: L27 `spatial::Transform`; L26 `math::{Vector, float3, quaternion}`。
- 实际存活路径（函数体未被注释但无调用者）:
  - `update_listeners_inner(...)`（L187-254）签名含 `listeners: &mut [(Transform, SpatialAudioListenerComponent)]`（L191）。对每个 listener 读 `trans.position` → kira `handle.set_position`（L224）+ `info.position = trans.position`（L232/243）; 读 `trans.rotation` → `handle.set_orientation`（L225）。**只读 position/rotation，不读 scale、不用矩阵**。
  - `play_audio_volume_in_track(..., trans: &Transform, ...)`（L525-533）: 读 `trans.position`（L561/590）同步 kira `SpatialTrackHandle::set_position`。
  - `leaving_audio_volume_in_track(..., trans: &Transform, ...)`（L612-617）: 读 `trans.position`（L629/645）。
- TODO（被注释的 ECS 查询，全部以 `world.query_mut::<(&Transform, ...)>()` 形式出现）:
  - `update()` L163-184: `(&Transform, &mut SpatialAudioListenerComponent)` 按 priority 取前 k 个 listener（L165、L171-182）。
  - `update_reverbs()` L261-290: `(&SpatialAudioReverbBound, &SpatialAudioReverb)` 遍历、`bound.contains_point(listener.position)`。
  - `update_audios()` L300-313: `(&Transform, &mut SpatialAudioVolume)`。
  - `update_listener_audios()` L344-404: 同上查询 + `float3::distance_sq(listener.position, trans.position)`（L367）做距离排序/截断 —— **世界空间距离语义**。
- local/world: 全部按世界空间处理（audio listener/emitter 位置直接来自 Transform.position）。
- 顺带: 与 Transform 无关但与 math 强相关的存活代码 —— `create_listener()` L145 `manager.add_listener(float3::ZERO, quaternion::IDENTITY)`、`Tracks::use_track()` L95 `add_spatial_sub_track(listener_id, float3::ZERO, ...)`（float3/quaternion 直接喂 kira，见 C2）。

### A4. `kairos_engine/src/physics.rs` — 物理步进 + 双向同步（中）

- 导入 L10: `crate::{math::float3, physics::rigid_body::RigidBody, spatial::Transform}`。
- 读字段（注释代码）: L97-105 TODO 同步块 `word.query_mut::<(&mut Transform, &RigidBody)>()`，写 `transform.position = rigid_body.translation().into(); transform.rotation = (*rigid_body.rotation()).into();` —— 将来由 rapier 写回 Transform（**写入 position/rotation，scale 不动**）。
- local/world: rapier 刚体在世界空间，回写语义即世界空间。
- 不读 scale、不用矩阵（物理只同步 TR）。

### A5. `kairos_engine/src/kairos_game.rs` — 游戏组装（中，演示场景）

- 构造: 相机 `Transform::look_at(cam_pos, cam_target, float3::UP)`（L184）；plane `Transform::new(pos, quaternion::IDENTITY, float3::ONE)`（L237-241）；ball 同（L245-249）；注释掉的批量 spawn 里 `Transform::new(position, rotation, scale)` 带非 1 scale（L223）。
- 读字段: `plane_transform.position`（L243）/`ball_transform.position`（L255）→ 传入 physics wrapper 的 `set_position`（float3 直接跨到 rapier，见 C2）。
- local/world: 场景根实体，**local == world**（无父子层级）。
- TODO: 大量注释掉的 ECS `spawn`（相机/audio listener/reverb/批量 audio volume 等，L186-235/265-276）；`render()` L323-335 注释掉渲染查询 `(&Transform, &LODMesh, &MaterialComponent)` 并调用 **`trans.get_local_to_world()`**（L332）传给 `graphics_command.draw(mesh, material, matrix)` —— 这是唯一用到 `get_local_to_world()` 的地方。

### A6. `kairos_engine/src/kairos_editor/ui/game_window.rs` — 游戏视口（轻）

- 仅 TODO（注释）L224-250: ECS 查询 `(&Transform, &mut Camera)` 取相机，按窗口尺寸更新 `camera.aspect` 后 `camera.get_view_projection_matrix(*transform)` → `set_view_projection_matrix(vp)`。若启用，语义同 A1（世界空间相机、不读 scale）。

### 小结（A）

| 站点 | 读 pos | 读 rot | 读 scale | 需矩阵 | 构造 | local/world | 状态 |
|---|---|---|---|---|---|---|---|
| graphics/camera.rs | ✔ | ✔ | ✘ | ✔(view 手工) | — | world | 活代码 |
| editor/ui/scene_camera.rs | — | — | — | ✔(间接) | look_at | world | 活代码 |
| audio/spatial.rs | ✔ | ✔ | ✘ | ✘ | — | world(距离) | 半注释 |
| physics.rs | 写回 | 写回 | ✘ | ✘ | — | world | 注释 TODO |
| kairos_game.rs | ✔ | — | — | get_local_to_world | look_at/new | local==world | 构造活/渲染注释 |
| editor/ui/game_window.rs | — | — | — | ✔(注释) | — | world | 注释 TODO |

---

## B. spatial::AABB 使用点

### B0. 定义

`kairos_engine/src/spatial/aabb.rs`

- `pub struct AABB { pub max: float3, pub min: float3 }`（L3-6），**无任何 derive**（非 Copy/Debug/serde）。
- `contains_point(&self, point: float3) -> bool`（L11-16）: 直接访问内部 `.0`（glam Vec3A）做 `cmpge/cmp/…/all()`，属于对 float3 内部表示的紧耦合（可移植性注意点）。

### B1. 各使用点

| 站点 | 字段/方法 | 形态 | ECS 计划 |
|---|---|---|---|
| `audio/spatial/spatial_audio_reverb.rs` | `SpatialAudioReverbBound { pub aabb: AABB }`（L4-6）; 包装 `contains_point`（L11-13）; `SpatialAudioReverb::new(..., aabb)` 返回 `(Self, Bound)`（L27-47） | 存进 ECS 组件结构（注释 `// impl Component for SpatialAudioReverbBound {}` L7-8） | ✔（TODO 注释，尚未 impl） |
| `audio/spatial.rs` | 注释代码 `world.query_mut::<(&SpatialAudioReverbBound, &SpatialAudioReverb)>()` + `bound.contains_point(listener.position)`（L262-266） | 消费组件 | — |
| `graphics/mesh.rs` | `Mesh::compute_aabb()`（L202-222）: rayon `par_iter().reduce` 对 `v.position.xyz()` 用 `math::{Max,Min}` trait 归约，构造 `AABB{max,min}`; 导入 L10 `spatial::AABB` | **每次现算，不存储**（`Mesh` 只含 vertices/indices） | — |
| `graphics/mesh/wireframe.rs` | `create_wireframe_mesh_quads` L47-51: `mesh.compute_aabb()` 后 `aabb.max - aabb.min` 求 size → `size.x()/y()/z()` 求 max_extent 定线框半厚度 | 只读 extents | — |
| `kairos_editor/ui/inspector/material.rs` | `PreviewState::framing_camera(aabb, style)`（L235-247）+ `new(mesh_index, aabb, style)`（L260-270）: `(aabb.min+aabb.max)*0.5`、`aabb.max-aabb.min`、max_extent、`direction.normalize()` 算取景相机; `draw_preview` 切网格时 `mesh.compute_aabb()` 重取景（L915-921） | 只读 min/max | — |
| `kairos_editor/ui/inspector/mesh.rs` | `PreviewState::new(aabb, style)`（L86-119）: 同上 framing; `draw_preview` L154 `PreviewState::new(mesh.compute_aabb(), ...)` | 只读 min/max | — |
| `kairos_game.rs` | L201-212: 手写 `AABB { min: float3::new(-20,...), max: float3::new(20,...) }` 喂给 `SpatialAudioReverb::new`（随后 spawn 被注释 L213） | 构造字面量 | — |

### B2. 备注

- AABB 目前**既非 Mesh 持久数据、也非 ECS 组件**；唯一"接近组件"的是 `SpatialAudioReverbBound.aabb`（待 impl Component）。
- `Mesh` 磁盘资产（rkyv `.mesh_bin` + `.mesh` toml）不含 AABB —— 边框/取景每次都重新 compute。
- 仓库根 `audio/spatial_audio_listener.rs` 是**空文件死代码**（未被 `audio.rs` 声明为模块）; 真实组件在 `audio/spatial/spatial_audio_listener.rs`。迁移时勿混淆/勿理旧文件。

---

## C. rapier3d 互转点

> rapier3d 0.33 的 `rapier3d::math` 是 **glam 系**类型（re-export `parry::glamx`，`Vector3`/`Rotation` 均有公开 `x/y/z(/w)` 字段/方法），因此 engine 侧 From impl 直接读字段可行；`math.rs` 还依赖 `rapier3d::glamx::FloatExt` 提供 f32 的 `lerp`。

### C1. 定义点（全部位于 math 模块内部 converts 文件）

`kairos_engine/src/math/vec/converts.rs`

| From | To | 方向 | 备注 |
|---|---|---|---|
| `Color32` | `float4` | 本地→本地 | `/255.0` 展开（L4-13），依赖 glam::Vec4 |
| `float3` | `mint::Vector3<f32>` | 本地→mint | L15-23（kira 桥接，见 C2） |
| `float3` | `rapier3d::math::Vector3` | 本地→rapier | L25-29（显式 `Vector3::new(x,y,z)` 构造） |
| `rapier3d::math::Vector3` | `float3` | rapier→本地 | L31-35（`float3::new` 构造） |

`kairos_engine/src/math/quaternions/converts.rs`

| From | To | 方向 | 备注 |
|---|---|---|---|
| `quaternion` | `mint::Quaternion<f32>` | 本地→mint | L3-10（kira 桥接） |
| `rapier3d::math::Rotation` | `quaternion` | rapier→本地 | L12-16（**只有 rapier→quaternion 单向**，无反向 rapier From） |

同文件还有与这些无关的本地 From 集合：`math/color/converts.rs`（`Color32↔egui::Color32`、`From<Color32> for syntect::highlighting::Color`、`[u8;3]/[u8;4]→Color32`）、`math/vec.rs`（`[f32;3]→float3` L869、`[f32;4]→float4` L1345、若干 tuple→float4 L1351-1384，供 gltf 导入/Vertex 装配）。`math.rs` L12 有 `use rapier3d::glamx::FloatExt`（仅 f32 `Lerp` 用）。

### C2. 使用点

rapier 方向（float3/quaternion ↔ rapier `Vector3`/`Rotation`）：

| 站点 | 写法 | 方向 | 状态 |
|---|---|---|---|
| `physics.rs` L80 `physics_pipeline.step(self.gravity.into(), …)` | `.into()` | float3→rapier Vector3 | 活代码（gravity 字段即 `float3`） |
| `physics/rigid_body.rs` L55-57 `RigidBody::set_position(engine, position: float3)` → `rigid_body_set[..].set_translation(position.into(), false)` | `.into()` | float3→rapier Vector3 | 活代码 |
| `physics/collider.rs` L58-60 `Collider::set_position(engine, position: float3)` → `collider_set[..].set_translation(position.into())` | `.into()` | float3→rapier Vector3 | 活代码 |
| `physics.rs` L97-105（注释 TODO）`transform.position = rigid_body.translation().into(); transform.rotation = (*rigid_body.rotation()).into();` | `.into()` | rapier Vector3→float3; rapier Rotation→quaternion | 注释（将来回写 Transform） |

mint 方向（float3/quaternion → mint，kira 0.12 的 `ListenerHandle::set_position`/`set_orientation`/`AudioManager::add_listener`/`add_spatial_sub_track` 参数均为 `impl Into<Value<mint::Vector3<f32>|mint::Quaternion<f32>>>`，实测 kira 0.12 源码签名）：

| 站点 | 写法 | 状态 |
|---|---|---|
| `audio/spatial.rs` L145 `manager.add_listener(float3::ZERO, quaternion::IDENTITY)` | 直接传本地类型（走 mint From） | 活代码（`create_listener` ← `AudioEngine::create_listener` ← `kairos_game.rs` L188） |
| `audio/spatial.rs` L95 `add_spatial_sub_track(listener_id, float3::ZERO, …)` | 同上 | 仅编译（`Tracks::use_track` 调用链 `play_audio_volume_in_track` 目前无活调用） |
| `audio/spatial.rs` L224-225/561/590/629/645 `set_position/set_orientation(trans.position/rotation)` | 直接传本地类型（走 mint From） | 死调用者（未注释但无调用），需 `cargo check` 确认 mint 桥能解析 |

**使用形态结论**：engine↔rapier 全部写成 `.into()`（无一处 `From::from` 或显式构造）；**没有** rapier 侧代码反向调用这些 impl 之外的第二处。除 physics 外没有其它模块 import rapier 类型。

### C3. 跨 crate 迁移的 orphan-rule 影响

- 上述 From impl 全部声明在 `kairos_engine/src/math/*/converts.rs`——即**未来 kairos_math crate 的代码**。`From` 是外来 trait、rapier/mint/egui/syntect 是外来类型，因此：
  1. `impl From<rapier3d::math::Vector3> for float3` 与 `impl From<float3> for rapier3d::math::Vector3` 只能写在"拥有 float3 或 rapier 类型的 crate"。float3 移入 kairos_math 后，这些 impl 无法下沉到 kairos_transform/physics adapter（两处都不是本地类型）→ 要么 **kairos_math 直接依赖 rapier3d**（可 feature-gate），要么在 engine 侧改用显式构造/自由函数（`rapier3d::math::Vector3::new(v.x(),…)`），放弃 trait 转换。
  2. `impl From<quaternion> for mint::Quaternion` / `From<float3> for mint::Vector3` 同理（kira 音频桥需要）→ kairos_math 需可选 `mint` 依赖，或 audio 调用点改写为显式 mint 构造。
  3. `From<Color32> for egui::Color32` / `From<Color32> for syntect::highlighting::Color`（`math/color/converts.rs`）会把 egui、syntect 拖进 kairos_math → 编辑器侧风格字段若继续存 math::Color32，就必须保留这组 impl（feature-gated 或由 kairos_editor 自持颜色类型）。
  4. `math.rs` f32 `Lerp` 依赖 `rapier3d::glamx::FloatExt` → kairos_math 不应为 f32 lerp 依赖 rapier，改用自己的实现或 glam 的 `FloatExt`。
- 纯本地 From（`[f32;N]`、tuple、`Color32→float4`）随类型搬家无 orphan 问题。

---

## D. math wrapper 消费分布

> 口径：`use crate::math::{...}` / `crate::math::…` / 直接命名 wrapper 类型；"轻重"按每文件出现行数粗分。graphics/kairos_editor 中大量 `egui::Color32`、`emath::TSTransform` 与 math 无关，已排除。

| 模块/文件 | 用到的类型/trait | 权重 |
|---|---|---|
| **audio** — `spatial.rs`、`spatial/spatial_audio_reverb.rs` | `float3`（含 ZERO/consts）、`quaternion`（IDENTITY）、`Vector`、`AABB` | 中（kira/mint 边界） |
| **graphics** — `camera.rs` | `math::{self}`、`float3/float4/float4x4`、`Transform`、free `dot/tan/TO_RADIUS` | 重 |
| **graphics** — `mesh.rs` | `math::{self}`、`float2/float3/float4/float4x4/quaternion`、`AABB`、`Max/Min` trait、tuple/array From | 重（gltf 导入 + compute_aabb） |
| **graphics** — `mesh/wireframe.rs` | `math::{self, float3}`、normalize/cross | 中 |
| **graphics** — `vertex.rs` | `float2/float3/float4`（结构体字段 = wrapper; `#[repr(C)]` + Pod/Zeroable/serde/rkyv） | 重（数据布局） |
| **graphics** — `render_pipeline.rs` | `float4/float4x4` 仅用于 `size_of`/顶点步长与 `offset_of!(Vertex,…)` | 中（**内存布局耦合**） |
| **graphics** — `graphics_graph/{graph,graphics_command,graphics_node}.rs` | `float4x4`（vp buffers、`draw(…, local_to_world: float4x4)`、BaseDraw/InstancingDraw 字段） | 中 |
| **graphics** 其余（lod_mesh/material_component/material/texture/*/shader/attachment…） | 无 | — |
| **kairos_editor** — `syntax.rs` | `math::Color32` ×14 字段 + `from_hex` + `into()`→syntect | 重（Color32 专线） |
| **kairos_editor** — `ui/scene_camera.rs` | `math::{self, float3, float4x4}`、`Transform`、free length/normalize/cross/lerp | 重 |
| **kairos_editor** — `ui/scene_window.rs` + `gizmos/*` | `math::{self, float2, float3}`、`float4/float4x4`（含 `IDENTITY` draw）、free min/max/normalize/cross/dot/PI | 中 |
| **kairos_editor** — `ui/inspector/material.rs` | `math::{self, Vector, float3, float4x4}`、`AABB`、`Color32` 样式字段、`float4x4::IDENTITY`（预览网格 draw） | 中 |
| **kairos_editor** — `ui/inspector/mesh.rs` | `math::{Vector, float2, float3, float4, float4x4}`、`AABB`、`float4x4::IDENTITY`、`size_of::<float2/3/4>` 显示文本（L388-435）、`std::mem::offset_of`/布局展示 | 中 |
| **kairos_editor** — `ui/inspector/{code,shader,toml,audio}.rs` | `math::Color32`（字段/`from_hex` 编辑/`into()`→egui） | 轻~中 |
| **kairos_editor** — `ui/preferences_window.rs`、`project_window/{content_panel,hierarchy_panel}.rs`、`tool_bar.rs`、`ui_style_fields.rs` | `math::{self, Color32, float2, float3, float4}`（StyleField 存值 + `from_array/from_array_4` + egui 双向转） | 轻~中 |
| **kairos_editor** — `runtime.rs` | `float4x4::IDENTITY`（egui pass vp） | 轻 |
| **kairos_editor** — `ui/game_window.rs` | 仅注释代码引用 `Transform`/`Camera` | 轻（零活引用） |
| **kairos_editor** — docking_tab/egui_ext/其它 | 无（egui 自身类型） | — |
| **kairos_game.rs** | `float3, quaternion`、`AABB`、`Transform`（look_at/new/position） | 中 |
| **inputs.rs** | `float2`（`Input::Mouse(float2)` 变体 + `inject_mouse_position`） | 轻（当前无上游填充） |
| **physics** — `physics.rs`/`rigid_body.rs`/`collider.rs` | `float3`（gravity/positions）、`.into()` rapier、`Transform`（注释回写） | 中 |
| **spatial** — 定义（`spatial.rs/aabb.rs/transform.rs`） | 依赖 `math::{float3,float4x4,quaternion}` | 定义侧 |
| **asset_loader / log / main / kairos_paths / kairos_settings / kairos_ui / kairos_dialog** | 无直接引用（asset 管线通过 `Vertex` 的 serde/rkyv 间接依赖 wrapper） | — |
| **benches** — `vec_benchmark.rs` | `kairos_engine::math::{self, Vector}`，bench `float4` new/copy/dot/cross/normalize（对照手写 struct 与裸 glam） | 只读消费 |
| **benches** — `texture_encode_decode.rs` | 无 math wrapper（只走 graphics texture API + half::f16） | — |
| **kairos_engine 之外**（kairos_ecs/kairos_collections/kairos_ptr/kairos_supervisor/kairos_tasks/kairos_time/prototypes） | **无任何引用**（仅 kairos_ecs 文档示例里出现虚构 `Transform`） | — |

**序列化依赖**（wrapper 上手动实现的序列化，非 derive）：
- `float2/float3/float4`：手写 `Serialize/Deserialize` + `rkyv::Archive/Serialize/Deserialize`（`math/vec.rs`）；被 `Vertex`（serde + rkyv）→ `Mesh` 磁盘资产（`.mesh` toml / `.mesh_bin` rkyv）依赖。
- `Color32`：手写 serde（hex 字符串，`math/color/serialies.rs`）; 被编辑器风格 toml 依赖（syntax 主题、inspector/audio、tool_bar、project_window 等）。
- `float4x4/quaternion`：无 serde/rkyv（仅 Pod/Zeroable）。
- **Transform/AABB 无 serde**；编辑器序列化的是"含 float3 字段的风格/相机结构"（`SceneCamera`、`SceneWindowStyle.cam_default_*`、`MaterialInspectorStyle.camera_direction`、`MeshInspectorStyle.camera_direction`），不是 Transform 本身。
- bytemuck：float2/float4/float4x4/quaternion 有手写 `Pod+Zeroable`；**float3 没有**（`vec.rs` 中仅 L174-175/L1386-1387 两组）。而 `graphics/vertex.rs` L26-27 对手写 `unsafe impl Pod for Vertex`（含 float3 字段）——按静态检查当前应编译不过（除非另有路径），**HEAD 是否可编译存疑，handoff 前需 `cargo check`**。这直接影响 kairos_math 必须为 float3 补 bytemuck impl 的结论。

---

## E. 对 T3 API 设计与迁移 handoff 的关键结论

- **纯 TRS 即可的点**：audio（listener/volume/reverb 只需 position，外加 listener rotation）与 physics 回写（只写 position/rotation）——不需矩阵，LocalTransform 分解式 API 足够。
- **需要完整矩阵的点**：render 路径唯一入口 `get_local_to_world()`（`kairos_game.rs` 注释渲染）+ `graphics_command.draw(local_to_world: float4x4)` + gltf `node_transform_matrix`（float4x4::trs 纯函数）。相机 view 其实只消费 rotation+position，`quaternion.to_float4x4()` 已够，不必给相机配 GlobalTransform 矩阵。
- **"local == world" 假设遍布所有消费点**（kairos_game root 实体、audio 世界距离、physics 回写、camera 世界 view、编辑器 look_at 相机）——今天没有父子层级，任何 LocalTransform/GlobalTransform 拆分都先满足 root-only 场景；引入 propagation 前不要在这些站点读 GlobalTransform。
- **Transform 本身无 serde 需求**；需要 serde 的是 float3/float2/Color32（编辑器风格 toml、SceneCamera、Mesh/Vertex 资产），所以 kairos_math 的 serde/rkyv（可 feature-gate）必须随类型带走，否则 Vertex 资产与场景窗口风格文件全断。
- **`.into()` 重写点**：physics 三处活代码（`physics.rs` gravity、`rigid_body.rs`/`collider.rs` set_position）与注释同步块；从 impl 移出 kairos_engine 那一刻起这些 `.into()` 将失效，需改为 kairos_math 提供的显式转换或 rapier 构造。
- **orphan-rule 决断点**：rapier/mint/egui/syntect 互转 From 无法下沉到 adapter，只能在"kairos_math 挂可选依赖"与"engine/editor 侧显式构造（放弃 trait 转换）"之间二选一；f32 lerp 借道 `rapier3d::glamx::FloatExt` 必须顺手摘掉。
- **Audio 是隐藏硬依赖**：kira 0.12 的 listener/spatial-track API 直接吃 `mint::Vector3/Quaternion`，`float3→mint`、`quaternion→mint` 两个 From 是 audio 活代码的编译前提——mint 桥要作为 kairos_math 必选项或 audio 改造项单列。
- **布局耦合**：`render_pipeline.rs` 用 `size_of::<float4x4/float4>()` 与 `offset_of!(Vertex,…)` 描述 wgpu vertex/instance 布局；`Vertex` 为 `#[repr(C)]` + Pod。kairos_math 必须保证 float2/3/4/float4x4 尺寸/对齐与如今一致（float2=Vec2、float3=Vec3A、float4=Vec4、float4x4=Mat4 同款），并补齐 float3 的 Pod/Zeroable。
- **最受益于 GlobalTransform 的站点**：未来若引入层级，渲染（`get_local_to_world`→GlobalTransform）、相机（世界位姿）、physics 回写（父级合成）是首批切换点；audio 与编辑器 preview 相机仍只读分解字段即可。
- **迁移验收项**：`cargo check` 通过（HEAD 当前疑似有 float3 Pod / 死代码类型缺口）、`vec_benchmark` 与 `.mesh` 资产 roundtrip 不受影响、编辑器风格 toml 兼容。

---

### 统计速览

- A（Transform 使用点）: 定义 1 + 直接消费 6 个文件（camera、scene_camera、audio/spatial、physics、kairos_game、game_window 注释），外加 6 处被注释 ECS 查询。
- B（AABB 使用点）: 定义 1 + 消费 6 个文件（reverb、mesh、wireframe、inspector×2、kairos_game）。
- C（互转）: rapier 相关 From 3 个 + mint 相关 2 个（另 `Color32→float4` 属本地）；rapier 活使用点 3（全 `.into()`），mint 活使用点 1（`add_listener`）、编译态调用点若干。
- D（math 消费）: 直接引用文件 ~30（含 bench 1）；kairos_engine 外 0。
