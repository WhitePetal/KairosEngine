# Bevy 视图 / 相机参考：Window、Camera、extract 产物与相机控制器形态

> Wayfinder research ticket: [Research: bevy 视图/相机参考：Viewport、Camera、extract 产物与相机控制器形态](https://github.com/WhitePetal/KairosEngine/issues/159)。
> 日期：2026-09-09。上游依据：`bevyengine/bevy` 最新稳定 tag **`v0.19.1`**（release commit `b56fc29`，2026-08-12）。已在本机 `/tmp/bevy0191` 稀疏检出源码，**行号以该 tag 为准**（下文统一写 `crates/<crate>/src/<path>:<行>`，根为 `<repo>/crates/`）；示例文件在 `<repo>/examples/`。
> 目的：为 kairos 单 world + 每帧缓冲的渲染接入设计（extract 阶段写入每视图 VP 矩阵与 draw-list 缓冲、UI 窗口随后消费）建立 bevy 侧事实清单。
> 方法说明：优先 bevy 源码直读；未能从源码逐行核实的点已在文中标注 ⚠️。

---

## 0. 结论速览

1. **bevy 的"视图"不是一个新实体类型，而是主 world 的相机实体在渲染 world 中的镜像**。主 world 里一个视图 = 一个实体上 `Camera`（bevy_camera）+ `Projection` + `GlobalTransform` + `RenderTarget`（默认 = 主窗口）+ 剔除结果（`VisibleEntities`/`Frustum`）；渲染 world 里同一实体携带 `ExtractedCamera` + `ExtractedView`（extract 产物）。视图的稳定身份是 `RetainedViewEntity(main_entity, auxiliary_entity, subview_index)`——同一主实体可以展开多个"子视图"（阴影 cascade / cubemap 面 / sub-view）。
2. **窗口尺寸的权威值在主 world 的 `Window` 组件上**（`WindowResolution` 存物理宽高 + scale_factor，逻辑 = 物理 / scale）。OS 的 `Resize` 由 winit 事件循环在下一次 `app.update()` 之前直接改写 `Window` 组件，并把 `WindowResized` 写入消息通道；`camera_system`（主 world `PostUpdate`）读该消息、**事件驱动**地更新相机缓存与投影，不是每帧重算。
3. **投影矩阵（clip_from_view）在主 world 算一次并缓存在 `Camera.computed.clip_from_view`**；extract（渲染 world `ExtractSchedule`）把该缓存 + `GlobalTransform` 拷成 `ExtractedView{ clip_from_view, world_from_view, viewport, … }`；最终的 VP 乘积 `clip_from_world = clip_from_view * world_from_view⁻¹` 在渲染 world 的 Prepare 阶段（`prepare_view_uniforms`）才合成，写入 GPU `DynamicUniformBuffer`（`ViewUniforms`），每个视图一个 `ViewUniformOffset`。
4. **"每帧拷贝/覆盖"是 extract 的标准形态，有三种生命周期模式**：(a) `ExtractResource`：源变化才整体覆写目标资源；(b) 每帧先 `clear()` 再重建的资源（如 `CameraMainPassTextureFormats`）；(c) 以 entity 为键的"渲染 world 镜像资源"（如 `ExtractedWindows`：逐项 `entry` 更新 + 增删键），GPU 侧对象跨帧存活、尺寸变化时重建。渲染 phase（draw-list）是"每帧清空但保留分配"的经典范式（v0.19 已演进为 retained items + dirty 增量，见 1.3 末）。
5. **相机控制器在 bevy 里就是普通 `Update` 系统**：控制状态（orbit 半径/俯仰/方位角）是放在相机实体上的自定义 `#[derive(Component)]` 结构；调参是文件级 `const`；输入来自 `ButtonInput` / `AccumulatedMouseMotion` / `AccumulatedMouseScroll` 等资源；需要速度一致时用 `Res<Time>`（`delta_secs()`）。没有任何"控制器注册表/编辑器特权路径"。
6. **bevy 没有"typed per-window viewport 资源"这种主 world 概念**：viewport 是 `Camera.viewport: Option<Viewport>` 字段（物理像素矩形），尺寸/scale 缓存进 `Camera.computed`；渲染侧的"每窗口资源"是 `ExtractedWindows`（按 window entity 键控）。多视图的正确键控单位是**视图**（相机 entity / RetainedViewEntity），而不是窗口；多相机→同一窗口用 viewport 矩形（split-screen），多窗口用 `RenderTarget::Window(Entity(..))`。
7. **对 kairos 单 world 方案**：bevy 的"主 world 组件 + 渲染 world 每帧镜像"折叠成"主 world 组件（Camera 数据、窗口尺寸）+ 每帧提取阶段产出的、按视图键控的 `FrameView`/draw-list 帧资源"是直接对应物；建议以"视图 id = 相机实体 + 目标 + 子视图序号"为键、帧缓冲资源"clear 后重建 + 容量复用"，并把窗口尺寸变化做成逐帧前部的事件 apply。详见 §2。

---

## 1. 分节事实

### 1.1 数据模型：Window、Camera 组件与渲染 world 的 "View"

**主 world 侧的相机（bevy_camera crate，0.19 从 bevy_render 拆分出的核心相机组件）：**

```rust
// crates/bevy_camera/src/camera.rs:374-413（节选）
#[derive(Component, Debug, Reflect, Clone)]
#[require(Frustum, CameraMainTextureUsages, VisibleEntities, Transform, Visibility, RenderTarget)]
pub struct Camera {
    pub viewport: Option<Viewport>,   // 无则渲染整个 target
    pub order: isize,                 // 同一 target 内谁画在上面
    pub is_active: bool,              // false = 不渲染
    pub computed: ComputedCameraValues, // 缓存：clip_from_view / target_info / ...
    pub output_mode: CameraOutputMode,
    pub msaa_writeback: MsaaWriteback,
    pub clear_color: ClearColorConfig,
    pub invert_culling: bool,
    pub sub_camera_view: Option<SubCameraView>,
}
```

- `Camera` 的默认值：`is_active: true, order: 0, viewport: None`（`camera.rs:415-429`）。
- 相机必备的配套组件由 `#[require]` 自动补：`Frustum、CameraMainTextureUsages、VisibleEntities、Transform、Visibility、RenderTarget`（`camera.rs:376-383`）；`RenderTarget` 默认 `Self::Window(WindowRef::default())` = 主窗口（`camera.rs:1037-1041`）。
- 2D/3D 相机只是"选渲染图 + 默认投影"的标记组件：`Camera2d { require(Camera, Projection::Orthographic(OrthographicProjection::default_2d()), Frustum) }`（`crates/bevy_camera/src/components.rs:11-16`）；`Camera3d { require(Camera, Projection) }`（`components.rs:24-25`）。真正的投影数值在 `Projection` 组件（`Perspective/Orthographic/Custom` 枚举，`crates/bevy_camera/src/projection.rs:214-220`，默认透视 `projection.rs:274-278`）。
- `Viewport` 是**物理像素**矩形 `{ physical_position: UVec2, physical_size: UVec2, depth: Range<f32> }`（`bevy_camera/src/camera.rs:62-71`）；还有 0.19 新增的 `SubCameraView`（多显示器拼一个逻辑大视图时的子块，`camera.rs:146-180` 附近）与 `MainPassResolutionOverride`（渲染分辨率覆盖，`camera.rs:142-144`）。
- `RenderTarget` 可以是 `Window(WindowRef)` / `Image(Handle<Image>)` / `TextureView(ManualTextureViewHandle)` / `None{size}`（`camera.rs:890-908`）；`WindowRef` 是 `Primary`（默认，指代带 `PrimaryWindow` 标记的实体）或 `Entity(entity)`（`crates/bevy_window/src/window.rs:72-84`）。`PrimaryWindow` 只是标记组件（`window.rs:43-56`），`WindowPlugin` 在启动时用配置的 `primary_window` spawn 出该实体（`crates/bevy_window/src/lib.rs:128-137`）。

**Window 组件（主 world，权威尺寸来源）：**

```rust
// crates/bevy_window/src/window.rs:895-908（节选）
pub struct WindowResolution {
    physical_width: u32, physical_height: u32,
    scale_factor_override: Option<f32>, scale_factor: f32,
}
```

- `Window`（`window.rs:164` 起）字段：`present_mode/mode/position/resolution/title/...`，是个普通组件（`#[require(CursorOptions)]`）。逻辑/物理换算：`width() = physical_width / scale_factor`（`window.rs:939-947`），`physical_size()`（`window.rs:969-971`），`scale_factor()`（`window.rs:976-979`）。默认 1280×720 物理、scale 1.0（`window.rs:910-918`）。

**渲染 world 的 "View" 是什么：**

- 相机实体通过 `Camera` 的 `#[require] SyncToRenderWorld`（`crates/bevy_render/src/camera.rs:63-72`，注册处）被 `SyncWorldPlugin` 的 `entity_sync_system` 同步出一个渲染 world 实体（映射键 `RenderEntity`/`MainEntity`，见 `crates/bevy_render/src/sync_world.rs:124-160`）。
- 该渲染实体上每帧放：`ExtractedCamera`（`camera.rs:455` 起：`target/order/output_mode/clear_color/exposure/hdr/...`）+ `ExtractedView`（`crates/bevy_render/src/view/mod.rs:326-384`）+ 剔除产物 + `ViewUniformOffset`。`ExtractedView` 注释明确"不是所有 view 都来自相机，灯光阴影也是 view"（`camera.rs:448-454` 附近的 doc 与 `view/mod.rs:317-325`）。
- 视图身份：`RetainedViewEntity { main_entity, auxiliary_entity, subview_index }`（`view/mod.rs:275-315`），作为 render world 各类每帧资源的键（phase、GPU 缓冲等）。

**窗口尺寸何时进入主 world（相对调度的时机）：**

- bevy_winit 的 `WinitPlugin` 把 runner 换成 `winit_runner`（`crates/bevy_winit/src/lib.rs:135`），整个 winit 事件循环在**两次 `app.update()` 之间**被泵动（`crates/bevy_winit/src/state.rs:885-913`）。
- `WindowEvent::Resized` 到达时（`state.rs:200-230` 是分发入口），`react_to_resize` **立即改写** `Window.resolution`，同时 push 一个 `WindowResized` 到临时 Vec（`state.rs:248-250` 与 `state.rs:915-929`）。
- 下一次 `run_app_update`（= 一次 `app.update()`，`state.rs:766-774`）之前，`forward_bevy_events` 把这些事件 `write_message` 进世界（`state.rs:776-878`），随后才跑调度。
- 结论：**窗口新尺寸在"该尺寸生效的那一帧"的调度开始前就已写进主 world `Window` 组件**；`WindowResized` 消息则在同一帧内可被任意阶段读取（消息是双缓冲、每帧 update 的 bevy Messages，`bevy_window/src/lib.rs:107-126` 注册了全套窗口消息）。⚠️ 消息的"上一帧残留/本帧新增"边界语义未在本 ticket 深挖（可参照仓库内 bevy-app-schedule-reference.md 对 kairos Messages 的说明）。

### 1.2 VP 矩阵在哪里算（主 world 缓存 → extract → 渲染 world 合成）

**第 1 步：主 world `PostUpdate`，`camera_system`（事件驱动）。**
`crates/bevy_render/src/camera.rs:73-77`：`CameraPlugin` 在主 app 注册 `camera_system.in_set(CameraUpdateSystems)` 于 `PostStartup` 和 `PostUpdate`（后者排在 `AssetEventSystems`、`visibility::update_frusta` 之前）。
`camera_system`（`camera.rs:351` 起）的输入是主 world 的东西：`MessageReader<WindowResized/WindowCreated/WindowScaleFactorChanged>`、`Query<&Window>`、`Assets<Image>`、以及 `Query<(&mut Camera, &RenderTarget, &mut Projection)>`。它：
1. 汇总"变化了的 target 集合"（resize/created/scale_factor/image asset 事件）；
2. 对每个相机，若 target 变了 / 相机新增 / 投影变了 / viewport 变了，就 `get_render_target_info` 重新得到 `{physical_size, scale_factor}`（`camera.rs:387-427` 区域），必要时缩放并 clamp `viewport`；
3. 若逻辑 viewport 尺寸非零，就 `camera_projection.update(w, h)`（透视即重算 aspect）并 `camera.computed.clip_from_view = get_clip_from_view()`（`camera.rs:429-433`）。

即：**aspect 依赖的 viewport 尺寸 → 投影矩阵更新 → 缓存 `clip_from_view`，全部发生在主 world PostUpdate，且只在 target 变化时做**。缓存结构 `ComputedCameraValues { clip_from_view: Mat4, target_info: Option<RenderTargetInfo>, old_viewport_size, old_sub_camera_view }`（`bevy_camera/src/camera.rs:216-224`）；`Camera::clip_from_view()` 只是读缓存（`camera.rs:527-531`）。`Projection` 的矩阵由 `CameraProjection` trait 提供：`get_clip_from_view()/update(w,h)/get_frustum_corners(...)`（`bevy_camera/src/projection.rs:43-64`，透视实现 `projection.rs:336-398`）。
注意 gotcha（写进映射建议）：`Camera::logical_viewport_size()` 在 `camera_system` 首跑之前返回 `None`（`bevy_camera/src/camera.rs:471-477` 的 doc 明说）。

**第 2 步：渲染 world `ExtractSchedule`，`extract_cameras`（每帧全量重建视图产物）。**
- 提取系统本体跑在渲染 world，但通过 `Extract<Query<…>>` 参数读主 world（`ExtractSchedule` 运行时把主 world 临时塞成渲染 world 里的 `MainWorld` 资源；见 §1.3 与 `crates/bevy_render/src/extract_param.rs`）。
- `extract_cameras`（`bevy_render/src/camera.rs:473` 起）查询每个 `(Entity, RenderEntity, &Camera, &RenderTarget, &CameraRenderGraph, &GlobalTransform, &VisibleEntities, &Frustum, …)`：
  - `is_active == false` → 从渲染实体 `remove::<ExtractedCameraComponents>()`（`camera.rs:546-551`）；
  - target 尺寸为 0 → 同上移除（`camera.rs:567` 附近）；
  - 否则向渲染实体 `commands.insert((ExtractedCamera{…}, ExtractedView{…}, 剔除数据, frustum, Projection 克隆…))`（`camera.rs:624` 起）。其中：
    - `ExtractedView.clip_from_view = camera.clip_from_view()`（主 world 缓存，`camera.rs:645`）；
    - `ExtractedView.world_from_view = *transform`（主 world GlobalTransform，`camera.rs:646`）；
    - `ExtractedView.viewport = uvec4(viewport 原点 x,y, 宽,高)`（物理像素，`camera.rs:643-660` 区域）。
  - 因此 **extract 阶段不合成 VP**，只是把"投影缓存"和"位姿"搬到渲染 world 的同一实体上。
- `CameraMainPassTextureFormats`（按相机渲染实体键控的 target 格式表）每帧在 `extract_cameras` 开头 `clear()` 再重建（`camera.rs:510-511` 附近）。

**第 3 步：渲染 world `RenderSystems::PrepareResources`，`prepare_view_uniforms`（合成并上传）。**
`prepare_view_uniforms`（`view/mod.rs:998-1127`）对每个 `ExtractedView`：
- 计算 `view_from_world = world_from_view⁻¹`、`clip_from_world = clip_from_view * view_from_world`（有 temporal jitter 时用抖动后的投影，`view/mod.rs:1048-1058`）；
- 组装 `ViewUniform { clip_from_world, view_from_world, world_from_view, clip_from_view, viewport: Vec4, world_position, frame_count, … }`（结构体 `view/mod.rs:610-669`）写入 GPU `DynamicUniformBuffer` 资源 `ViewUniforms`（`view/mod.rs:671-688`），并把 `ViewUniformOffset{offset}` 组件插到视图实体上（`view/mod.rs:1102-1126`）。
- 注册：`prepare_view_uniforms.in_set(RenderSystems::PrepareResources)`（`view/mod.rs:201`）。
- 渲染调度的大步骤见 `RenderSystems` 枚举：`ExtractCommands → PrepareAssets → CreateViews → … → Queue → PhaseSort → Prepare/PrepareResources → … → Render → Cleanup → PostCleanup`（`bevy_render/src/lib.rs:157-210`）。

**主 world 侧的每帧顺序小结**（主 world 调度，非渲染 world）：`camera_system`（PostUpdate，事件驱动改投影/缓存）→ `visibility::update_frusta` 等剔除系统（PostUpdate，用新缓存更新 `Frustum`/`VisibleEntities`）→ 帧边界 extract（读主 world 状态拷进渲染 world）。

### 1.3 extract 模式与"每帧覆盖/重建"的生命周期

**ExtractSchedule 的框架**（`bevy_render/src/extract_plugin.rs`）：
- `ExtractSchedule` 是渲染 world 的一个普通调度（`extract_plugin.rs:86-87`），其命令不立即 apply（`auto_insert_apply_deferred: false` + `set_apply_final_deferred(false)`，`extract_plugin.rs:38-45`），改由渲染调度里的 `apply_extract_commands`（`RenderSystems::ExtractCommands`）在渲染线程 apply（`extract_plugin.rs:89-99`），以便与主 world 并行。
- 每帧提取 = SubApp 的 extract 回调：`entity_sync_system`（同步 `SyncToRenderWorld` 实体映射与增删，`extract_plugin.rs:62-73`）→ 把主 world 与 scratch world 对调、以 `MainWorld` 资源身份塞进渲染 world → `run_schedule(ExtractSchedule)` → 换回来（`extract_plugin.rs:115-126`）。
- 线程模型（可选 `PipelinedRenderingPlugin`）：渲染 world 被挪到渲染线程，第 N 帧渲染与第 N+1 帧模拟并行；主线程的帧顺序为 sync → extract → RenderExtractApp（默认空）→ winit 事件 → 主调度（`bevy_render/src/pipelined_rendering.rs:84-106` 的注释图 + `184-205` 的 `renderer_extract`）。**单 world 的 kairos 不需要这层并发**，只需取"extract 是一个独立阶段、读主数据写帧产物"的语义。

**每帧覆写资源的三种模式（这就是 Q3 问的"resemblance"）：**

1. `ExtractResource`（拷贝型资源）：每帧系统 `extract_resource`——目标已存在且**源 `is_changed()` 才 `*target = R::extract_resource(&source)`**，目标缺失时 `commands.insert_resource(...)`（`bevy_render/src/extract_resource.rs:56-80`）。代表：`ClearColor`（`bevy_render/src/camera.rs:63-70` 附近）、`ManualTextureViews`。
2. **clear-每帧重建**：如 `CameraMainPassTextureFormats`（`camera.rs:510` 起每帧 `clear()`）；以及历史/经典 render phase 缓冲（见下）。
3. **按 entity 键控的镜像资源（跨帧存活、逐项更新）**：`ExtractedWindows`（`view/window/mod.rs:106-109`，`HashMap<Entity, ExtractedWindow>` + `primary`）。`extract_windows`（`view/window/mod.rs:125-200`）每帧对每个主 world `Window` 做 `entry(entity).or_insert(...)` 更新 `physical_width/height`、比较 `size_changed`、清/保留 swapchain view；窗口关闭或 `RawHandleWrapper` 移除时从资源删除（`view/window/mod.rs:192-199`）。注册顺序 `extract_windows.before(extract_cameras)`（`view/window/mod.rs:38`）——`extract_cameras` 要读 `ExtractedWindows` 查 target 的 texture view/format（`camera.rs:605-620` 区域）。GPU surface（`WindowSurfaces`）也以此 entity 为键跨帧存活，尺寸变化时由 `create_surfaces`/`prepare_windows` 重建（`view/window/mod.rs:39-45`；重建与清理 `view/mod.rs:1177-1198` 的 `cleanup_view_targets_for_resize`）。
4. 渲染 world 的**临时实体清理**由 `despawn_temporary_render_entities`（`RenderSystems::PostCleanup`）负责（`extract_plugin.rs:56-58`；`sync_world::TemporaryRenderEntity` 语义）。

**draw-list（render phase）的每帧生命周期（Q3/Q5 的直接类比）：**

```rust
// crates/bevy_render/src/render_phase/mod.rs:1562-1568（doc 原文）
/// Stores the rendering instructions for a single phase that sorts items in all views.
/// They're cleared out every frame, but storing them in a resource like this
/// allows us to reuse allocations.
#[derive(Resource, Deref, DerefMut)]
pub struct ViewSortedRenderPhases<SPI>(pub HashMap<RetainedViewEntity, SortedRenderPhase<SPI>>);
```

- 即：**"每帧清空、但以 Resource 形式保留以复用分配"的按视图 draw-list 缓冲**。v0.17.3 同款注释与 `insert_or_clear`（本地缓存 `bevy_render-0.17.3` render_phase/mod.rs:1196-1218）。
- v0.19 的实际机制已细化：`SortedRenderPhase` 的 item 分两类——`add_retained`（跨帧保留，直到显式 remove）与 `add_transient`（本帧自动移除，`mod.rs:1810-1824`）；`prepare_for_new_frame` 每帧只 drain 上一帧的 transient items（`mod.rs:1587-1601`）。并配合 `DirtySpecializations`（changed/removed 实体与需整视图重排的集合，`bevy_render/src/camera.rs` 后半）做增量排队/出队，以及 `PendingQueues` 的"材料未就绪下帧重试"队列。⚠️ **未逐行核实**：v0.19.1 中 PBR mesh 的稳态帧是否真的整体跳过重排队（取决于 bevy_pbr 的 queue 系统与 GPU preprocessing 路径），本 ticket 只核实了 phase API、dirty 资源结构与 `sort_phase_system`（`mod.rs:2174-2187`）存在。

**组件级 extract 的形态**：`extract_components`（`bevy_render/src/extract_component.rs:98-113`）对每个已同步实体 `try_insert_batch((RenderEntity, Out))`，用 `Local<usize>` 记住上次容量避免重分配；`extract_component` 返回 `None` 时从渲染实体移除对应 `Target`（即"可选组件消失 → 渲染侧删组件"）。它由 `ExtractComponentPlugin` 注册进 `ExtractSchedule`（`extract_component.rs:83-95`），并自动带上 `SyncComponentPlugin` 的移除钩子（`bevy_render/src/sync_component.rs`：主 world 组件被移除 → 渲染 world 组件被删）。

### 1.4 相机控制器惯例（bevy examples 形态）

bevy **没有官方"相机控制器"插件**；每个示例在文件内自造一个小控制器。以 `examples/3d/light_probe_blending.rs` 的 orbit 相机为例：

- **状态与调参**：控制器状态是一个自定义 `#[derive(Component)] struct OrbitCamera { radius: f32, inclination: f32, azimuth: f32 }`（球坐标，`light_probe_blending.rs:136-144`），**插在相机实体上**；速度等调参是模块级 `const CAMERA_ORBIT_SPEED_*`。可切换相机时对它 `insert/remove`（`light_probe_blending.rs:534,546`）。
- **系统与输入**：`fn orbit_camera(mut cameras: Query<(&mut Transform, &mut OrbitCamera)>, mouse_buttons: Res<ButtonInput<MouseButton>>, mouse_motion: Res<AccumulatedMouseMotion>, mouse_scroll: Res<AccumulatedMouseScroll>)`（`light_probe_blending.rs:407-449`）注册在 `Update`（`light_probe_blending.rs:165`）。它改的是**相机实体的 `Transform`**（`Transform::from_translation(...).looking_at(...)`，`light_probe_blending.rs:445-447`）——`GlobalTransform`/投影/VP 全走上一节的通用管线，控制器不碰矩阵。
- **dt 从哪来**：示例普遍用 `Res<Time>` / `time.delta_secs()`（bevy_time 每帧在 `First` 由 `time_system` 推进）；如 `examples/3d/anisotropy.rs:201` 用 `Res<Time>` 的 `delta()`。键盘式移动控制器只是把 `ButtonInput<KeyCode>` 当速度源、乘以 `delta_secs()`。
- 例程组织：`add_plugins(FreeCameraPlugin)`（`light_probe_blending.rs:159`）——即控制器打包成**普通 Plugin，仍只是注册系统**。
- 典型的多相机/多窗口官方示例：`examples/3d/split_screen.rs`（4 个相机同一主窗口，靠每相机的 `Camera{ order, .. }` + 动态 viewport，`split_screen.rs:69-82` 与 `159-179` 的 `set_camera_viewports` 响应 `WindowResized` 改 `Camera.viewport`）；`examples/3d/camera_sub_view.rs`（0.19 新 sub-view 特性）；`examples/window/multiple_windows.rs`（次窗口 = 单独 spawn 的 `Window` 实体）。UI 与相机的绑定走 `UiTargetCamera(camera)` 组件（`split_screen.rs:86`）——bevy_ui 支持每个 UI 根节点指定目标相机，⚠️ 未深入核实其实现。

### 1.5 多视图分层惯例（供未来演进参考）

- **层 1 —— target（渲染到哪）**：`RenderTarget` 组件；窗口用 `WindowRef::{Primary, Entity(id)}`；渲染 world 侧按 window entity 键控 `ExtractedWindows`（swapchain/物理尺寸）。
- **层 2 —— 视图矩形（在 target 内画哪一块）**：`Camera.viewport: Option<Viewport>`（物理像素；不设 = 整个 target）。split-screen = 同 target 多个相机各设 viewport + 用 `order` 消歧义（`split_screen.rs:73-77` 注释"Renders cameras with different priorities to prevent ambiguities"）。
- **层 3 —— 视图身份（每帧产物按什么键）**：`RetainedViewEntity(main_entity, auxiliary_entity, subview_index)`——同相机可因 shadow cascade（auxiliary = 光源相机）、cubemap 面（subview_index 0..5）、sub-view 展开成多个渲染视图（`view/mod.rs:275-315` 的字段 doc）。
- 相机间遮挡/合成：同一 target 上多个 active 相机按 `Camera.order` 排序（渲染 world `sort_cameras`，`bevy_render/src/camera.rs:726` 起，`RenderSystems::CreateViews` 集，注册 `camera.rs:113`）；`order` 相同且 target 相同会告警（`camera.rs:750-780` 区域）。`CameraOutputMode::Skip` 用于"只画中间纹理、让更高 order 相机写最终 target"的合成模式（`bevy_camera/src/camera.rs:859-877`）。

---

## 2. 对 kairos 单 world 缓冲方案的映射建议

（kairos 侧依据：kairos 引擎已在 kairos_ecs 上复刻 bevy_app 的 `MainScheduleOrder` 驱动器与 `First→PreUpdate→RunFixedMainLoop→Update→PostUpdate→Last` 各标签，`kairos_engine/src/kairos_editor/schedule.rs:124-145`；时钟在 `First` 的 `time_system` 推进，`schedule.rs:147-156`；World 资源 API 齐备，`kairos_ecs/src/world.rs`：`insert_resource` 2061、`init_resource` 2048、`resource` 2285、`resource_mut` 2333、`resource_scope` 2878、`clear_trackers` 1799、`run_schedule` 3953；`kairos_ecs/src/message.rs` 有完整 Message 通道（`message_reader`/`message_writer`/`messages` 等 pub use，17-22 行）。）

### 2.1 建议的数据落点：主数据留组件，帧产物进"按视图键控"的帧资源

- bevy 中"窗口尺寸/相机参数"的**权威值永远在主 world 组件**（`Window`/`Camera`/`Projection`/`GlobalTransform`），渲染侧只有镜像。单 world 没有两套 world，因此：
  - **窗口尺寸**：`Window` 式组件（或 editor 的等价物）就是权威值，不需要任何镜像资源；extract 系统直接 `Query<&Window>`。
  - **VP 缓存**：照抄 bevy——把 `clip_from_view` 缓存在相机自己的组件字段里（对应 `Camera.computed`），由"PostUpdate 内、Update 之后"的一个 `camera_system` 等价系统在**窗口/投影变化事件到来那一帧**更新（事件驱动，不必每帧全量重算）。
  - **每帧产物**：一个帧资源，形状可直接参考 bevy 的 `ExtractedView`（主 world 侧版）：

```rust
// 建议的 kairos 形态（示意，非 bevy 代码）
#[derive(Component, Clone)] pub struct KairosView {          // 或插在相机实体上的帧组件
    pub clip_from_world: Mat4,   // = clip_from_view * view_from_world（本阶段直接算好，bevy 要到渲染 Prepare 才乘）
    pub world_from_view: Mat4,
    pub viewport: UVec4,         // 物理像素 (x, y, w, h)
    pub target_physical_size: UVec2,
}
pub struct FrameViews(pub HashMap<ViewId, KairosView>);      // ViewId = (相机实体, target, subview)
pub struct DrawList(pub Vec<DrawCommand>);                   // 每帧重建、容量复用
```

- bevy 里"合成 VP 的最后一步"发生在渲染 world 的 Prepare（因为要跟 GPU 缓冲对齐）；**单 world 没有延迟一帧的渲染 world，建议在 extract 阶段直接把 `clip_from_world` 算好写进帧资源**——这正好等价于把 bevy 的 `camera_system`（主 world 缓存投影）与 `prepare_view_uniforms`（渲染 world 合成）两步压成一步，且因为主数据同 world、不存在并发，语义更简单。剔除（`VisibleEntities`/`Frustum`）同理在 extract 前的主 world PostUpdate 算完，extract 直接读。

### 2.2 extract 阶段的位置与"每帧覆盖"纪律

- kairos 的 `MainScheduleOrder`（`schedule.rs:131-145`）里 **PostUpdate 已存在**。建议新增一个阶段标签（例如 `RenderExtract`）插在 `Update` 与 `PostUpdate` 之间，或把 extract 系统放进 PostUpdate 的一个专用 set 且排在 `camera_system`/剔除之后（bevy 的做法是 `camera_system.before(update_frusta)`，见 §1.2）。
- 逐帧生命周期参考 §1.3 的三种模式，对 kairos 的建议：
  1. `FrameViews`/每视图数据：**每帧 clear-重建但保留分配**（对应 bevy `CameraMainPassTextureFormats` 式与 phase 缓冲的"每帧清空复用分配"注释，`render_phase/mod.rs:1562-1566`）。**不要**用 `ExtractResource` 的"变了才覆写"语义做帧数据——视图集合会变（窗口关闭/相机增删），全量重建 + 容量保留最省心，符合 bevy 对 phase 缓冲的一贯注释。若日后想省排队开销，再学 v0.19 的 retained items + `DirtySpecializations` 增量（见 1.3 末）。
  2. 跨帧 GPU/图资源（若 kairos 有需保留的对象）：仿 `ExtractedWindows` 的 **entity 键控镜像资源**（entry 更新 + 删除键），删除时机挂在窗口关闭消息/组件移除钩子上（bevy 用 `WindowClosing` 消息 + `RemovedComponents`，`view/window/mod.rs:192-199`）。
- **Gotcha（bevy 明写的）**：`logical_viewport_size()` 在 camera_system 首跑前是 `None`（`bevy_camera/src/camera.rs:471-477`）——kairos 的 UI 消费方在首帧（或相机刚创建那帧）必须容忍"视图尺寸未知 → 跳过绘制"，不要 panic。

### 2.3 控制器与输入：直接照 bevy 惯例，无特殊形态

- 控制状态 = 相机实体上的自定义组件（orbit 参数等，§1.4）；调参 = const/资源；输入 = 引擎自己的输入资源（对应 bevy `ButtonInput`/`AccumulatedMouseMotion`）；`dt` 用 kairos `First` 里已推进的 `Time`（`schedule.rs:147-156`，与 bevy `time_system` 同挂 First）。控制器只写 `Transform`，绝不碰矩阵缓存——矩阵更新仍由 `camera_system` 等价系统统一做。编辑器"选中相机/临时控制"只要对该实体 insert/remove 控制器组件即可（对应 `light_probe_blending.rs:534/546` 的 insert/remove 模式）。

### 2.4 多视图演进：一开始就按"视图"键控，而非"窗口"

- bevy 的分层（§1.5）值得直接采用：**(窗口/目标) ≠ (视图) ≠ (相机)**。单个 editor 窗口未来要拆 2/4 视口时，最省事的做法就是照 split-screen：相机加 `viewport` 字段（物理矩形），多相机共享同一窗口目标，各设 `order`。
- 帧资源键（`ViewId`）现在就用 `(相机实体, target 标识, 子视图序号)` 的复合键（≈`RetainedViewEntity`），而不要用"每窗口一个 viewport 资源"的单体结构——这样分屏/多窗口/将来 shadow 或子视图都只是"同相机多视图"的自然展开，不需要改键控模型。
- 多窗口 = 多个带尺寸数据的窗口实体 + 相机 `target` 字段指向其一（对应 `WindowRef::Entity(entity)`）；"窗口尺寸变化"统一做成：帧开始的 OS 事件批 → 改 `Window` 组件 + 发消息 → extract/UI 消费。事件到达与调度的相对时机参照 §1.1 末：**尺寸在下一帧调度开始前已生效**。

### 2.5 不建议的方向

- 不要在单 world 里再仿一套"主 world / 渲染 world"实体双份镜像（`SyncToRenderWorld` + `MainWorld` 资源互换，§1.3）——那是为跨线程流水线准备的；kairos 没有第二线程消费者时，纯拷贝只添乱。
- 不要用"每窗口一个全局 viewport 资源"当主数据结构（bevy 也没有：viewport 是相机的字段，`§1.1`）；窗口只该持有尺寸/scale，视图持有矩形与矩阵。

---

## 3. 未能逐行核实 / 局限

- v0.19.1 PBR mesh 稳态帧是否真的"保留 phase item、不重排队"（只核实了 phase API、`DirtySpecializations`、`PendingQueues` 结构与 `prepare_for_new_frame` 的 transient drain 语义；queue 系统的实际调用链未追踪）。
- `bevy_ui` 的多相机渲染（`UiTargetCamera`）实现细节未核实；仅确认 split_screen 示例如此绑定 UI 到相机。
- `examples/3d/camera_sub_view.rs` 只确认文件名与 `Res<Time>` 使用，未读全文。
- bevy 消息（Messages）双缓冲的逐帧更新/清理机制未深入（建议另开 ticket 对照 kairos_ecs message 通道语义，若要精确对齐"第几帧可见"）。
- 上述 `kairos_ecs` 行号为 branch `bevy_fork` 当日磁盘状态，非发布版承诺。
