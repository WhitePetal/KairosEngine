# kairos_ecs 组件/系统 API 事实 + asset 就绪语义清点（render 集成前置调研）

> 范围：本文件所有结论均来自本地代码（workspace crate root = `/Users/baiaoxiang/KairosEngine/KairosEngine`，下文路径相对该目录），不依赖 bevy 文档。
> 背景：render 集成要把 `Camera` / `LODMesh` / `MaterialComponent` 变成 `#[derive(Component)]` 组件，由 PostUpdate 提取系统查询并写入一个 World 资源缓冲，UI 渲染代码随后排空；系统挂在 `kairos_editor/schedule.rs` 建好的轨道上（Main → First/PreUpdate/RunFixedMainLoop/Update/PostUpdate/Last）。

## 结论速览

1. `#[derive(Component)]` 只要求类型 `Send + Sync + 'static`（无泛型生命周期、无 `Clone/Debug` 要求）；`Camera`（纯 f32 字段）与 `LODMesh`/`MaterialComponent`（`Arc<AssetHandle<…>>` 字段）都直接满足。`AssetHandle<T>` 由 `AssetIndex` + `Option<tokio mpsc::Sender<T::DropEvent>>` 组成，是自动 `Send+Sync+'static`。除 derive 外**不需要**任何手动注册/反射步骤——组件在第一次 spawn/insert 时经 `Bundle::component_ids → register_component::<C>()` 惰性注册（`kairos_ecs/src/bundle/impls.rs:30-35`）。模型参照 `LocalTransform`（`kairos_transform/src/local_transform.rs:26-35`）。
2. 系统函数 API 齐备：`Query`/`Res`/`ResMut`/`Local`/`Commands` 参数、`Query<D, With<T>>`/`Changed<T>` 过滤器、元组批量 `add_systems`、独占系统 `fn(world: &mut World)` 都能直接使用；engine 侧在 `install` 之后通过 `world.get_resource_mut::<Schedules>().get_mut(PostUpdate).add_systems(...)` 追加系统（见 `kairos_engine/src/kairos_editor/schedule/test.rs:69-76` 的现成写法）。Commands 在 `ApplyDeferred` 同步点（默认自动插入）与 schedule 末尾（final deferred，默认开）应用。
3. kairos_ecs 没有 bevy 式缓冲 `EventWriter/EventReader`；`Event` = observer 即时触发（`event.rs:20-23`），**Message** = 拉取式缓冲通道（`MessageWriter`/`MessageReader` + `Messages<M>` 资源）。Message 需要手动 `MessageRegistry::register_message` 并且每帧跑 `message_update_system`，engine 目前**没有**挂任何 message/event 维护系统——对"UI 每帧写相机输入给 controller 系统"这种跨层传递，普通自定义 `#[derive(Resource)]` 缓冲更省事、也是 engine 内已有的惯用法（`schedule/test.rs:45-46`）。
4. `GraphicsCommand::draw`/graph 构建只搬运 `Arc<AssetHandle>`，不解析资源；真正的解析在 `render_pipeline.rs` 每帧逐实例 `assets_server.get(...)`，**没就绪就 `continue`（静默跳过该实例，不 panic、什么都不画）**（`render_pipeline.rs:655-661, 670-675, 690-692`），就绪后下一帧自动出现。因此 `KairosGame::new` 里 `load()` 后立即 spawn 实体**无崩溃风险**，只是实体会在资产就绪前若干帧不显示。

---

## (a) 组件约束

### a.1 derive 生成的代码与 trait 要求

- `Component` trait 本身：`pub trait Component: Send + Sync + 'static`（`kairos_ecs/src/component.rs:530`）。可选关联项默认全为 no-op/`None`（hooks、`register_required_components`、`map_entities`、`relationship_accessor`，`component.rs:542-576, 668-675`）。
- 派生宏（`#[proc_macro_derive(Component)]`，`kairos_ecs/macros/src/lib.rs:830-847`）：
  - 泛型生命周期必须绑 `'static`，否则编译错误"Lifetimes must be 'static"（`kairos_ecs/macro_logic/src/component.rs:177-188`）。
  - 生成的 impl 上追加 where 子句 `Self: Send + Sync + 'static`（`macro_logic/src/component.rs:266-269`）。即类型本身必须满足这三条，**不要求 Clone / Debug / Default / PartialEq**。
  - 生成的 impl 包含：`STORAGE_TYPE`（默认 `Table`，可 `#[component(storage = "SparseSet")]` 改，`lib.rs:842`；宏文档 `macros/src/lib.rs:745-750`）、`type Mutability`（默认 `Mutable`，`macro_logic/src/component.rs:284-286`）、`register_required_components`（无 `#[require]` 时为空实现，`macro_logic/src/component.rs:338-344`）、`clone_behavior()`（见 a.2）、按需的 `map_entities`/relationship impl（只有字段标 `#[entities]` 或声明 relationship 才生成，`macro_logic/src/component.rs:200-215, 305-331`）。
- 因此 `Camera`（纯 `f32` 字段，`kairos_engine/src/graphics/camera.rs:6-12`，现为注释掉的 `impl Component`，`camera.rs:13-14`）、`LODMesh`（`Arc<AssetHandle<MeshAssetsSystem>>`，`lod_mesh_component.rs:6-11`）、`MaterialComponent`（`Arc<AssetHandle<MaterialAssetsSystem>>`，`material_component.rs:6-10`）把注释换成 `#[derive(Component)]`（或单独 `impl kairos_ecs::component::Component for …`）即可，无需额外 derive。模型即 `LocalTransform`（`#[derive(Component, Debug, Clone, Copy, PartialEq)]`，`kairos_transform/src/local_transform.rs:26-35`）与 `GlobalTransform`（`#[derive(Component, Debug, Clone, Copy, PartialEq)]`，`global_transform.rs:26-27`）；其 Clone/Debug 只是该类型自己的便利 derive。

### a.2 Clone/Debug 不是 derive 要求

- derive 的 `clone_behavior()` 默认用 autoderef 特化：`C: Clone` 时才选"经 Clone 克隆"，否则落到 base → `ComponentCloneBehavior::Default`（`kairos_ecs/src/component/clone.rs:197-218`）。
- 全局默认解析：开 `kairos_reflect` feature 时经反射克隆，否则 `component_clone_ignore`（无操作，`clone.rs:36-52, 184-195`）。也就是说没有 `Clone`/反射注册的组件在实体克隆/移动时只会"被忽略"，但**编译与日常 spawn/query 完全不受影响**。克隆只发生在 `EntityCloner`（场景/实体复制）路径上，本项目不涉及。

### a.3 注册：derive 之外没有必做步骤

- 组件在**第一次被 spawn/insert/query 使用**时惰性注册：`unsafe impl<C: Component> Bundle for C` 的 `component_ids` 调用 `components.register_component::<C>()`（`kairos_ecs/src/bundle/impls.rs:30-39`）；`register_component` 按 `TypeId` 查重并生成 `ComponentDescriptor`，同时把 `#[require]` 声明的必需组件与 hooks 登记好（`component/register.rs:171-178, 224-268`）。多次注册幂等（已存在直接返回既有 id）。
- 无需手动 `world.register_component`、无需 `AppTypeRegistry`/反射注册；反射只在场景克隆/动态反射路径需要，且本仓库组件（含 `LocalTransform`）并未使用。spawn 也接受单个组件或任意组件元组（tuple `Bundle` impl，`bundle/impls.rs:94-102` 一带）。

### a.4 `Arc<AssetHandle<…>>` 是否满足 Send+Sync+'static

- `AssetHandle<T>` = `{ index: AssetIndex, drop_sender: Option<tokio::sync::mpsc::Sender<T::DropEvent>> }`（`kairos_engine/src/asset_loader/assets/asset.rs:90-97`）。
- `AssetIndex` 是 `usize + u32` 的 `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]` 结构（`asset.rs:38-42`）；`DropEvent: Send`（`asset.rs:79`）；tokio `mpsc::Sender<X>` 在 `X: Send` 时同时为 `Send + Sync`。故 `AssetHandle<T>`（以及它的 `Arc`）自动满足 `Send + Sync + 'static`（auto-trait 推导，非显式 impl——这点是推断；若顾虑可用编译期断言核实）。`T` 侧 `AssetsSystem: AssetsHandler: Any + Debug + Default`（`asset.rs:205-224`），`MeshAssetsSystem`/`MaterialAssetsSystem`/其 `AssetType` 均为 `'static`（`asset_loader/assets/asset/mesh.rs:95-153`、`material.rs:125-184`）。
- 注意 `Material` 内部还持 `Option<Arc<AssetHandle<ShaderAssetsSystem>>>` / `Option<Arc<AssetHandle<TextureAssetsSystem>>>`（`kairos_engine/src/graphics/material.rs:21-26`），这些句柄同样满足上述 bound——render 提取系统只搬运句柄、不解引用内容，不引入额外 bound。

---

## (b) 系统 API 可用面

### b.1 函数式系统参数面

- 系统参数白名单见 `kairos_ecs/src/system.rs:88-110`：`Query`、`Res`/`Option<Res>`、`ResMut`/`Option<ResMut>`、`Commands`、`Local`、`MessageReader`/`MessageWriter`、`NonSend`、元组（1–16 元素）等。
- 自定义 World 资源：`#[derive(Resource)]` + `#[derive(Default)]` 即普通 struct（例：`Trace(Vec<&'static str>)`，`kairos_engine/src/kairos_editor/schedule/test.rs:45-50`），系统内用 `ResMut<Trace>` 参数写（`schedule/test.rs:92-118`）。engine 侧安装资源用 `world.insert_resource(...)`（`kairos_editor/schedule.rs:236-237` 的 `Time`/`FixedTime`）。`ResMut` 的 trait bound 是 `Resource<Mutability = Mutable>`（默认即 Mutable，`kairos_ecs/src/system/system_param.rs:770-780`）。
- `Local<T>`：系统私有、跨调用持久状态，要求 `T: FromWorld + Send + 'static`，初值取 `T::from_world`（`kairos_ecs/src/system/system_param.rs:908-910, 1023, 1072-1082`）。函数式系统 + `Local<bool>` 的现成实例：`run_main(world: &mut World, mut run_at_least_once: Local<bool>)`（`kairos_editor/schedule.rs:164-183`）。
- Query 过滤器：`Query<&mut A, With<B>>`、`Query<&mut A, Without<B>>`（`kairos_ecs/src/schedule/tests.rs:815-820`）；`Changed<T>` 作 filter 的写法见 `kairos_ecs/src/change_detection/params.rs:667-668`（`Query<(), Changed<MyComponent>>`）。变更检测依赖每帧 tick：engine 每帧 `run_schedule(Main)` 后调用 `world.clear_trackers()`（`kairos_editor.rs:67-70`；`clear_trackers` 推进 `last_change_tick`，`kairos_ecs/src/world.rs:1797-1801`），所以 `Changed<T>` 是"自系统上次运行以来被改过"的逐帧语义。
- 系统元组批量注册：`Schedule::add_systems((sys_a, sys_b))`（`schedule/test.rs:531-539`；kairos_ecs 侧文档例 `schedule.add_systems((a, b).chain())`，`kairos_ecs/src/system.rs:56-70`）。

### b.2 Commands：什么时候真正生效

- `Commands` 是 `Deferred<CommandQueue>` 系统参数（`kairos_ecs/src/system/commands.rs:137-151`）：命令排队，"in sequence … when the `ApplyDeferred` system runs"（`commands.rs:75-99`）；`Commands::spawn`（`commands.rs:422-426`）/`spawn_batch`（`commands.rs:611-615`）可用。
- 应用到 world 的时机有三层：
  1. schedule 构建时**自动插入** `ApplyDeferred` 同步点：`ScheduleBuildSettings::auto_insert_apply_deferred` 默认 `true`（`kairos_ecs/src/schedule/schedule.rs:1646-1675`），插入规则是"有 deferred 参数的系统之后、会被其写入影响的后续系统之前"（pass 文档 `schedule/auto_insert_apply_deferred.rs:39-40`；与独占系统交互的例子 `kairos_ecs/src/schedule/schedule/tests.rs:272-287`——独占系统前必然插同步点，因为独占系统要求看到此前所有命令）。
  2. **每个 schedule 跑完后的 final deferred**：默认 `apply_final_deferred = true`（单线程执行器 `schedule/executor/single_threaded.rs:150-154, 169-174`；多线程执行器 `schedule/executor/multi_threaded.rs:304-315, 399-407`）。
  3. 独占系统自身运行完立即 `world.flush()`（`kairos_ecs/src/system/exclusive_function_system.rs:127-163`，flush 在 L158；`World::flush` 定义 `kairos_ecs/src/world.rs:3179-3183`）。
- 在本引擎里每个子阶段是独立的 `Schedule`（`Main` 的 `run_main` 逐个 `try_run_schedule(label)`，`kairos_editor/schedule.rs:164-183`），所以 Update/PostUpdate 阶段排队的 Commands 最迟在该子阶段 schedule 跑完时已应用；同帧后续子阶段/帧外 UI 代码都能看到。

### b.3 独占系统（&mut World）的声明方式

- 函数式独占系统 = 第一个参数是 `&mut World` 的 `fn`（宏展开见 `kairos_ecs/src/system/exclusive_function_system.rs:251-288`；`&mut World` 是隐含参数，后面可跟 `ExclusiveSystemParam`：`Local`、`&mut QueryState`、`&mut SystemState`，见 `exclusive_system_param.rs:28-123`）。
- engine 现成实例：`fn run_main(world: &mut World, mut run_at_least_once: Local<bool>)`（`kairos_editor/schedule.rs:164`）与 `fn run_fixed_main_loop(world: &mut World)`（`kairos_editor/schedule.rs:206-218`）；裸 `fn(&mut World)` 闭包/函数的测试例 `kairos_ecs/src/schedule/tests.rs:28-30, 40-42`。独占系统以 `Schedule::add_systems(run_main)` 直接注册（`kairos_editor/schedule.rs:245-247, 260-262`）。

### b.4 install 之后往现有 schedule 加系统的 engine 侧入口

- `install`（`kairos_editor/schedule.rs:229-268`）会把九个 schedule 都建好（含空的 `PostUpdate`，`schedule.rs:249-258`），并在 `Engine::new` 时调用（`kairos_editor.rs:29-34`）。每帧驱动顺序由 `MainScheduleOrder`（默认 `[First, PreUpdate, RunFixedMainLoop, Update, PostUpdate, Last]`，`kairos_editor/schedule.rs:123-145`）+ `Main` 上的 `run_main` 保证（`schedule.rs:158-183`）；engine 每帧调用 `world.run_schedule(Main)` + `world.clear_trackers()`（`kairos_editor.rs:67-70`；`run_schedule` 本体 `kairos_ecs/src/world.rs:3951-3955`）。
- **推荐追加方式**（install 之后、任何帧前）：
  ```
  world.get_resource_mut::<Schedules>().unwrap()
      .get_mut(PostUpdate).unwrap()
      .add_systems(extract_system);
  ```
  该模式在 `kairos_editor/schedule/test.rs:69-76`（`add_trace_system`）与 `schedule/test.rs:531-539` 中原样使用。API 事实：`Schedules::get_mut(label) -> Option<&mut Schedule>`（`kairos_ecs/src/schedule/schedule.rs:169-173`）；`Schedules::entry(label)` 不存在则创建（`schedule.rs:175-180`）；也可 `Schedules::add_systems(label, systems)` 一步到位（`schedule.rs:260-269`）；`Schedule::add_systems`（`schedule.rs:471-475`）。系统加到已 run 过的 schedule 没问题——schedule 会在下次 run 前重建。
- `Schedules` 资源是 `install` 期间用 `world.get_resource_or_init::<Schedules>()` 取得（`kairos_editor/schedule.rs:239`），资源 API：`get_resource_mut`（`kairos_ecs/src/world.rs:2364-2368`）、`get_resource_or_init`（`world.rs:2435-2439`）。
- 系统可加在 schedule 已 run 过之后：`add_systems` 只是把配置写进 graph（`schedule.rs:473-479`），schedule 下次 `initialize`/run 时若 `graph.changed` 会重建（`schedule.rs:642-667`）。
- 帧外（UI 渲染代码）读/写 world 数据不走系统参数，直接 `&mut World` 方法（`resource`/`resource_mut`/`query_mut` 等）即可；引擎里已有此先例（被注释掉的 `game_window.rs:224-233` 直接 `world.query_mut::<(&LocalTransform, &mut Camera)>()`、`kairos_game.rs:314-326`）。

---

## (c) Message/事件通道

### c.1 两个通道的区别

- **Event（即时、observer 驱动）**：`World::trigger(...)` 触发时相关 `Observer` **立刻**在同一次调用里运行（`kairos_ecs/src/event.rs:20-23`）；`#[derive(Event)]`（`event.rs:30-34`）+ `world.add_observer(|ev: On<E>| …)`（`event.rs:48-50`）+ `world.trigger(...)`（`event.rs:71-73`）。**没有** bevy 式的缓冲 `EventWriter/EventReader` 系统参数。事件数据 `Event: Send + Sync + Sized + 'static`（`event.rs:93-95`）。
- **Message（缓冲、拉取式）**：模块文档明确这是"pull-based event handling"、在 schedule 固定点求值而非发送时立即处理（`kairos_ecs/src/message.rs:29-45`）。`Message` trait = `Send + Sync + 'static`（`message.rs:94`），可 `#[derive(Message)]`（`message.rs:51-56`）。

### c.2 Message 的 API 与用法

- 写：`MessageWriter<M>` 系统参数（内部 `ResMut<Messages<M>>`，`message/message_writer.rs:63-67`），`writer.write(msg)`（`message_writer.rs:76-78`）。
- 读：`MessageReader<M>`（内部 `Local<MessageCursor<M>>` + `Res<Messages<M>>`，`message/message_reader.rs:39-52`），`reader.read()` 按系统游标"只读一次"（`message_reader.rs:46-52`）；空时可配 `PopulatedMessageReader` 跳过（`message_reader.rs:14-16`）。多个 reader 可并行，与同类型 writer 互斥（`message/message_mutator.rs:42-61`）。
- 存储：`Messages<M>` 是 `Resource`（`message/messages.rs:96-109`），双缓冲；每个 reader"至少每帧读一次"则消息只保留跨一帧边界，若两帧不 update 老消息会被丢（`messages.rs:20-42, 74-88`）；**update 永不调用则缓冲无限增长**（`messages.rs:82`）。
- 注册与维护：`MessageRegistry::register_message::<M>(&mut world)` 负责 `init_resource::<Messages<M>>()` 并登记（`message/message_registry.rs:41-61`；反注册 `message_registry.rs:84-91`）；每帧须跑 `message_update_system`（独占系统形式，`message/update.rs:34-48`），其触发策略 `ShouldUpdateMessages` 默认 `Always`（`message_registry.rs:29-39`），另有 `signal_message_update_system` 可改成"每轮 FixedUpdate 后"（`update.rs:23-31`）。
- 库内使用实例：`message/message_reader/tests.rs`（`MessageRegistry::register_message` + `PopulatedMessageReader` 系统 + schedule run，文件 L16 起）；参数声明样例 `kairos_ecs/src/schedule/tests.rs:821-825`（`MessageReader<E>`/`MessageWriter<E>`/`ResMut<Messages<E>>`）。

### c.3 引擎现状与建议

- engine 的 schedule `install` **没有**注册任何 message/event 维护系统（`kairos_editor/schedule.rs:229-268` 只装 rails），`kairos_engine` 全目录也没有 `MessageRegistry`/`message_update_system`/`world.trigger` 的调用（`kairos_editor/ui` 里的 `Messager`/`Message` 是 UI 自带的消息类型，与 kairos_ecs 的 `Message` 无关）。若要用 Message 通道，需要自己补：某处 `MessageRegistry::register_message::<CameraInput>(&mut world)` + 把一个 `message_update_system` 挂进某个每帧子阶段（并处理写/读顺序），或者走 `FixedUpdate` 触发模式。
- 对"UI 层每帧把相机输入写进 world 供 controller 系统读"：最省事且与库内惯例一致的方案是自定义 `#[derive(Resource, Default)]` 缓冲 struct（模型：`schedule/test.rs:45-46`），UI 代码在 redraw 内（两帧之间）用 `engine.world.resource_mut::<…>()` 覆盖写入，controller/相机系统用 `Res<…>` 读、用 `ResMut<…>` 消费；帧内顺序天然是"上一帧 UI 写入 → 本帧 schedule 读"。Message 通道只有在需要"多生产者、每系统独立游标、按消息 ID 追踪"时才划算，且要承担 c.2 的注册+update 维护成本。

---

## (d) asset 就绪语义

### d.1 draw 调用链：记录 vs 解析

- `GraphicsCommand::draw(mesh: Arc<AssetHandle<MeshAssetsSystem>>, material: Arc<AssetHandle<MaterialAssetsSystem>>, local_to_world)` 只把句柄连同矩阵压进当前 render pass 的 `draws`（`kairos_engine/src/graphics/graphics_graph/graphics_command.rs:116-132`；`BaseDraw` 定义 `graphics_node.rs:59-63`），**不做任何资源解析**。
- `GraphicsGraph::build` 阶段同样不解析：只做节点合并/裁剪，并按 `(mesh, material)` 句柄对去重合并为实例绘制（`InstancingRenderer` 持两个 `Arc<AssetHandle>`，`graphics_node.rs:65-69`；`graph.rs:297-332` 的 `optimize_nodes` 用 `HashMap<InstancingRenderer, InstancingDraw>` 聚合相同材质/网格的 `local_to_worlds`）。句柄未就绪不影响建图。
- 真正的解析发生在 `RenderPipeline::handle_render_pass_node`（`render_pipeline.rs:511-529` 接收 `assets_server: &AssetsServer`），**逐 draw instance**：

  ```
  for draw in &render_pass_node.draw_instances {
      let Some(mesh) = assets_server.get(&draw.renderer.mesh) else { continue; };      // L656-658
      let Some(material) = assets_server.get(&draw.renderer.material) else { continue; }; // L659-661
      ...
      let Some(shader_asset) = &material.shader else { continue; };   // L670-672（材质没解析出 shader 句柄）
      let Some(shader) = assets_server.get(shader_asset) else { continue; };  // L673-675（shader 未加载）
      ...
      let Some(texture_asset) = assets_server.get(texture_handle) else { continue; }; // L690-692（纹理未加载）
  ```

  结论：**未就绪 = 该实例当帧被静默跳过（continue），不 panic、不画任何东西**；就绪后 GPU 资源懒创建（mesh 顶点/索引缓冲 `create_mesh` + `mesh_buffer_cache` 按 index+version 缓存，`render_pipeline.rs:804-833`；纹理 bind group 按 version+modify_count 缓存，`render_pipeline.rs:696-737`；管线按材质 key+shader version 缓存，`render_pipeline.rs:741-802`），下一帧自动出现。每个实例（= 同 mesh+material 的一批局部矩阵）是跳过的最小单位：同材质但网格未就绪的实体会整批不画。

### d.2 load → ready 的机制与"就绪信号"

- `AssetsServer::load::<T>(path)` 同步返回 `Arc<AssetHandle<T>>`，存储立即置 `Entry::Loading`，异步 loader 由 `tokio::spawn` 在后台读文件（`asset_loader/assets/asset.rs:315-331, 343-392`，置 Loading 见 `asset.rs:350-370`；mesh loader 例 `asset_loader/assets/asset/mesh.rs:63-93`，material loader 例 `asset_loader/assets/asset/material.rs:54-122`）。
- **`AssetHandle` 上没有就绪标志/状态位**——它只有 `index` 与 `drop_sender`（`asset.rs:90-97`）。就绪与否的唯一查询方式是每帧探测 `AssetsServer::get::<T>(&handle) -> Option<&T::AssetType>`：`Entry::None | Loading` 或版本不匹配都返回 `None`，只有 `Entry::Some` 且版本一致才返回数据（`Assets::get`，`asset.rs:422-434`；`AssetsServer::get`，`assets.rs:107-118`）。
- 完成事件在主线程每帧 `AssetsServer::handle()` 排空：先处理 drop 事件、再处理 loaded 事件（带版本防串，`asset.rs:282-313`），随后处理依赖加载请求队列（`assets.rs:173-181`）。`AssetsServer::handle()` 在 redraw 流程中被多次调用：帧首 `update()` 之后（`kairos_editor/runtime.rs:251-255`，经 `KairosEngine::handle_asset_server`，`kairos_editor.rs:119-121`）、egui 闭包内（`runtime.rs:280-284`）、帧尾（`runtime.rs:416`）。
- 材质是**级联就绪**：material loader 必须先通过依赖通道请求 shader（和可选 texture）句柄，依赖请求要等下一轮 `handle()` 才被 `set_back` 处理（`asset_loader/assets.rs:35-46, 178-180`；`asset_loader/assets/asset/material.rs:54-105`），材质自身 `LoadedEvent` 才会发出。故材质 `Entry::Some` 至少发生在开载两轮 `handle()` 之后，且其 `shader`/`texture` 字段本身也各有一个“句柄在但内容未就绪”的窗口（被 d.1 的 L673-675/L690-692 兜住）。

### d.3 `KairosGame::new` 里 load 后立即 spawn 是否危险

- `KairosGame::new`（`kairos_engine/src/kairos_game.rs:137-270`）当前在 `Engine` 构建后立刻 `assets_server.load::<MeshAssetsSystem>(...)`/`load::<MaterialAssetsSystem>(...)`（`kairos_game.rs:160-165, 250-253`）——实际 spawn 目前全被注释（`kairos_game.rs:254-267, 177, 206-226`），但把注释打开后就是"spawn 时句柄处于 Loading"的场景。
- 由 d.1：**没有风险**。实体带着 Loading 的句柄进入 PostUpdate 提取系统 → 写入缓冲 → UI 排空成 draw call → 管线 `get()` 得 `None` → 跳过；之后资产就绪（`handle()` 排空 → `Entry::Some`），同一条 `Arc<AssetHandle>` 不变，下一帧起自动出现在画面。唯一代价是该实体在就绪前若干帧不渲染（加载通常是几十~几百 ms 量级）。`asset.rs:90-97` 的句柄是纯索引 + drop 通知，与内容状态解耦，这也是它能"先 spawn、后就绪"的原因。
- 一个帧内时序事实供集成参考：redraw 顺序是 `Engine::update()`（= run_schedule(Main) + clear_trackers，含 PostUpdate 提取）→ `handle_asset_server()` → egui 内 `render_ui()`（GameWindow::render 组装命令，含排空缓冲）→ `GraphicsGraph::build` + `render_pipeline.present(&mut assets_server, …)`（`runtime.rs:246-256, 277-287, 365-372`；`present` 把 `&mut AssetsServer` 传进 `handle_render_pass_node`，`render_pipeline.rs:524`）。即"提取系统看到的就绪状态"滞后"本帧 handle() 排空结果"一拍：本帧内新完成加载的资产最早下帧被提取/显示；但这不影响正确性（只多等一帧）。

---

## 对"提取系统 + 缓冲 + spawn 时机"的具体影响

1. **提取系统（PostUpdate）签名可行**：`fn extract_scene(world-buffer: ResMut<SceneRenderBuffer>, meshes: Query<(&LocalTransform, &LODMesh, &MaterialComponent)>, cameras: Query<(&LocalTransform, &Camera)>)`——所需参数/过滤器（含 `With<Camera>` 之类）在 b.1 全部有据；系统按 b.4 的 `get_resource_mut::<Schedules>().get_mut(PostUpdate).add_systems(...)` 挂上即可，`install`（含空 PostUpdate）在 `Engine::new` 时已完成（`kairos_editor/schedule.rs:229-268`、`kairos_editor.rs:34`）。
2. **缓冲资源**：自定义 `#[derive(Resource, Default)]`（如 `Vec<(float4x4, Arc<AssetHandle<…>>, Arc<AssetHandle<…>>)>`）每帧由提取系统**整写重建**（`ResMut` 清空重填），UI 排空代码（GameWindow::render 内、`game_window.rs:224-250` 处替换被注释逻辑）在 schedule 跑完后再读——不依赖 Commands/变更检测，无同步坑。若想省写可用 `Changed<LocalTransform>` 做增量，但每帧整写在该缓冲很小的情况下更简单。
3. **spawn 时机**：`KairosGame::new` 中 load 后立即 spawn 安全（d.3）；甚至建议 spawn 时**不需要**等待任何"就绪事件"——渲染管线自会跳过未就绪实例（d.1）。真正的约束在别处：提取/渲染拿不到 `AssetsServer`（它不是 World 资源，是 `Engine` 字段，`kairos_editor.rs:20-26`），所以"只把就绪的实体放进缓冲"这类优化做不了也不必要——由管线每帧 `get()` 裁决即可。
4. **相机输入（UI → controller）**：优先普通 Resource 缓冲（c.3）；若未来要 Message，需要补 `MessageRegistry::register_message` + 每帧 `message_update_system`（engine 目前无此接线，c.2/c.3）。
5. **变更检测**：`Changed<T>` 的语义是“自系统上次运行以来被写”（kairos_ecs 逐系统记录 last_run、逐数据记录 change tick）；engine 每帧 `run_schedule(Main)` 之后 `clear_trackers()`（`kairos_editor.rs:67-70`；`world.rs:1797-1801`）把 world 的变更基线（`last_change_tick`）前移并刷新 removed 缓冲，是每帧轨道的收尾步（`schedule/test.rs:6-7` 就把“一帧”定义为 run_schedule + clear_trackers）。对“每帧无条件全量重写缓冲”的提取系统可以完全不用 `Changed`；若以后做“相机 aspect 变了才重算 VP”之类的优化，应把写入放在同帧更早的阶段（如 Update），让同帧靠后的系统能检测到——不要跨帧依赖。
