# Editor UI 重写：kairos_engine 的约束与集成点

> **任务**：task-4 "Analyze kairos_engine constraints and integration points"。为 Lead 的 UI 架构设计文档提供“引擎侧不能改变的事实 / 必须满足的接口 / 不能踩的线”。
> **基线**：当前工作区 HEAD（2026-10）。只读分析，**未**运行 cargo build/test。
> **方法说明**：仓库有 `.codegraph/` 目录但**没有索引**（只有 `.gitignore`），`codegraph explore` 返回 "no .codegraph/ index exists"；按 AGENTS.md 回退到 read/grep，未自行 `codegraph init`（索引是 owner 的决定）。
> **证据口径**：`path:line` 均指当前 `kairos_editor/`、`kairos_engine/` 等 crate 布局（抽取后）。`docs/research/editor-crate-extraction-inventory.md` 里的行号基于**抽取前**布局（`kairos_engine/src/kairos_editor/…`），引用其结论处均已用当前代码复核。

---

## 0. 结论速览（给 Lead 的 12 条）

1. **一帧有两个不可合并的阶段**：先 `Editor::update()`（== `Engine::update()`，跑 ECS rails + extract），再 `egui_ctx.run_ui` 内的 `render_ui → handle_ui → draw_ui`，最后 `present`（`runtime.rs:242,264-270,350-351`）。UI 写在 World 上的状态（`SceneView.size`、`SceneViewInput`）**本帧不可见**，最早下一帧的 `Engine::update` 才被消费——新架构必须显式定义“intent 在 update 前应用”还是接受 1 帧延迟。
2. **现帧循环里 `render_ui` 在 `handle_ui` 之前、`draw_ui` 之前**：所以 `Drawer::render()` 里发的 Message 当帧被处理，`Drawer::ui()` 里发的 Message 下一帧才处理（`ui.rs:418`、`runtime.rs:264-270`）。这是 Messager 时序不直观的根因之一，也是 MCP “commands 本帧生效 / queries 读最新状态” hook 必须重新定义位置的原因。
3. **egui 0.35 的 `run_ui` 闭包一帧可能被调用多 pass**（默认 `max_passes=2`，见 egui 源码 `context.rs:780-860`、`memory/mod.rs:331`）。当前闭包里做 `graphics_commands.append(...render_ui())` 与 drain Messager 这类事**不是 multi-pass 安全的**；本仓库 `preferences_window.rs:229` 用了 `egui::Grid`，而 egui 的 `Grid` 首帧会 `request_discard`（egui-0.35.0/src/grid.rs:454），所以多 pass 不是理论风险。新架构必须把“每帧一次”的副作用放到 last-pass 判定之后，或显式 `max_passes=1`。
4. **引擎真值在 World，编辑器 UI 真值在 Drawer/Context 两处散落**：13 个 `Drawer` 实现、11 个 `Inspector` 实现、57 个 `Message` variant、73 处 `messager.send(` 调用点、约 365 行的单一 `handle()` match（`ui.rs:90-206,289-303,410-774`）。项目树/注册表/选中/重命名/DnD 在 `ProjectWindowModel`（`project_window.rs:49-71`），Inspector 选中在 `InspectorWindowModel`（`inspector_window.rs:47-55`），Tab 布局在 `ui::Context` 的 `DockState`，拖拽瞬态在 egui 内存（`docking_tab/state.rs:17-33`），样式 DTO 散落在各 Drawer 并各自写 TOML。
5. **现有 MCP 计划（docs/plan/kairos-editor-mcp/）已经把“状态分散”列为问题 #1**，并要求：单一 query/command 入口、Snapshot 序列化、`Drawer::as_query_handler()`、commands 在 draw 前 / queries 在 draw 后、无 `Arc<RwLock<World>>`、mock 可测（`phase3-middleware-architecture.md:11-16,105-124,176-244,278-343`）。新架构应当**用 Store 承接 QueryHandler 的职责，而不是继续把状态留在 Drawer 里**。
6. **MCP 工具目录对状态位置有“隐含假设”**：`get_project_tree/get_asset_registry/get_selected_asset` 要 ProjectWindow 的 registry+graph，`get_inspector_state` 要 InspectorWindow.selected，`get_scene_camera` 期望 SceneWindow 持有相机状态，`get_console_logs` 要 `Log` 可过滤（`phase2-tools-catalog.md:1272-1287`）。而**现状**：相机 orbit 状态在 World 组件 `EditorCameraController`（`camera.rs:141-152`），`Log` 是 editor 字段且几乎空转（见 §2.6），`get_game_state.is_running` 没有 play mode 可读。
7. **`Log` 不是 World 资源，也基本是死的**：`Log` 在 `Editor` 字段（`lib.rs:19-23`）；唯一生产者是 About 窗口每帧 `log.info(...)`（`about_window.rs:95-96`），唯一消费者是 Console 窗口每帧 `pop_front()`（`console_window.rs:78-82`）。真实日志走 `log` crate → env_logger stderr（`main.rs:9-12`）。MCP `get_console_logs`/崩溃通知要的是**可回放的结构化日志资源**，不是这个 `Log`。
8. **线程边界极硬**：winit 事件循环 + egui + wgpu + World 全在 main thread（`runtime.rs:452-537`）；资产加载在 `kairos_tasks` 的 `IoTaskPool`（ADR 0001），结果只在 `PreUpdate` 的主线程轨里插入 World（`kairos_asset/src/install.rs:15-27`）；项目树 watcher 是 notify 线程 + crossbeam（`project_watcher.rs:22,54,89-97`）；纹理句柄用 `tokio::sync::oneshot` 从 present 回传给 UI（`scene_window.rs:473-477`、`render_pipeline.rs:526-544`）。任何 effect/task 抽象都必须服从这条。
9. **`Engine::new()` 会开音频设备且 `Editor::new()` 依赖它**（`engine.rs:10-23`、`lib.rs:26-45`），所以今天没有任何测试构造 `Engine`（`camera/test.rs:10-11`、`engine/test.rs:31-33` 只测 `build_world`），`Drawer::ui` 的 `&Engine` 参数直接导致 UI 无法单测。新架构必须提供 headless/纯逻辑入口，否则“可测试”只是口号。
10. **crate 依赖方向是单向的**：`kairos_editor → kairos_engine → (asset/graphics/physics/…)`，engine 不可反向依赖 editor（`docs/agents/testing.md:50`；抽取清单 §9 #5 把“引擎→编辑器反向 install”列成必须迁出的问题）。UI 架构不能把新需求塞回 `kairos_engine` 形成反向依赖。
11. **渲染命令是“延迟执行的数据”，不是即时调用**：UI 只生产 `GraphicsCommand`，runtime 追加 egui 命令 → `GraphicsGraph::build` → `RenderPipeline::present` 在主线程同步执行，期间才创建 wgpu texture、发送 `EguiTextureHandle` oneshot（`runtime.rs:316-351`、`render_pipeline.rs:345-581`）。UI 侧不持有 GPU 资源，只有 `TextureId`。
12. **`.codegraph` 无索引 + 本清单是只读分析**：所有结论可复核，但未编译验证；§7 列出需要 Lead 拍板或需要编译确认的点。

---

## 1. 帧循环：`EditorRuntime::redraw()`（`kairos_editor/src/runtime.rs`）

### 1.1 精确顺序

```text
winit EventLoop (main thread)
└── ApplicationHandler::window_event(WindowEvent::RedrawRequested)       runtime.rs:517-519
    └── EditorRuntime::redraw()                                          runtime.rs:237-394
        ├── window = self.window.clone()                                 :238
        ├── self.editor.update()                                         :242
        │     └── Engine::update() → world.run_schedule(Main)
        │           + world.clear_trackers()                             engine.rs:43-46
        │           ├── First: time_system → Time.update(); message pass schedule.rs:321-331
        │           ├── PreUpdate: asset tracking (handle_internal_asset_events)  asset install.rs:15-27
        │           ├── RunFixedMainLoop: 0..N × (FixedUpdate, FixedPostUpdate)
        │           │       └── FixedPostUpdate: singnal_message_update_system    schedule.rs:337-339
        │           ├── Update
        │           ├── PostUpdate: asset event flush + editor camera controller  camera.rs:194-204
        │           ├── Extract: reset_camera_views / extract_scene_view / extract_game_view
        │           │                                         graphics/src/extract.rs:43-106
        │           └── Last
        ├── render_pipeline.lock()  (锁一直持有到 present 结束)            :249
        ├── RenderPipeline::get_window_surface()                         :256
        ├── egui_state.take_egui_input(&window)                          :262
        ├── self.egui_ctx.run_ui(raw_input, |ui| {                        :264-270
        │       graphics_commands.append(&mut self.editor.render_ui());  :265
        │       self.editor.handle_ui(ui);                               :267
        │       self.editor.draw_ui(ui);                                 :269
        │   });
        ├── egui_state.handle_platform_output(...)                       :272
        ├── root viewport output: Close 命令 / repaint_delay /
        │   process_viewport_commands                                     :274-300
        ├── tessellate(shapes, pixels_per_point)                          :302-306
        ├── build egui GraphicsCommand (draw_egui + output_to_framebuffer) :316-348
        ├── GraphicsGraph::build(graphics_commands)                       :350
        ├── render_pipeline.present(&self.editor.engine.world, output, graph) :351
        ├── frame_rate_counter.record_frame()                             :373-374
        ├── window.request_redraw()  ← 无条件再要一帧                       :384
        ├── if should_close → shutdown                                    :386-389
        └── set_repaint_delay_from_output(repaint_delay)                  :391-393
```

三个重绘驱动力叠加在一起：

- **无条件自驱动**：`window.request_redraw()` 在 `redraw()` 末尾无条件调用（`runtime.rs:384`），实际效果是持续渲染循环，`repaint_delay` 机制基本是冗余的。
- **egui 主动请求**：`egui_ctx.set_request_repaint_callback` → `EventLoopProxy::send_event(EditorRuntimeEvent::RequestRepaint)`（`runtime.rs:100-109`）→ `user_event` → `queue_repaint_after(delay)`（`runtime.rs:468-474,396-421`）→ `about_to_wait` 的 `drive_repaint_timer` 设 `ControlFlow::WaitUntil`（`runtime.rs:423-440,531-533`）。
- **winit 输入**：`egui_state.on_window_event` 的 `response.repaint` 也会 `request_redraw`（`runtime.rs:496-501`）。

### 1.2 键盘事件走了两条路（新架构必须处理）

`window_event` 先喂 egui（`egui_state.on_window_event`，`runtime.rs:496-501`），随后把 `WindowEvent::KeyboardInput` 无条件转给 `editor.update_keyboard_input(event)` → `engine.input_engine.update_keyboard_input`（`runtime.rs:520-526`、`lib.rs:47-49`、`engine.rs:4-7`）。也就是说**同一按键同时进入 egui 焦点系统和引擎 `InputEngine`**，没有“谁有焦点”的仲裁。新 UI 架构若保留引擎输入（场景 WASD 飞行），必须显式定义 focus/捕获规则。

### 1.3 顺序上的 oddities（都被现有设计依赖或掩盖，不要无意识地继承）

1. **`render_ui` 先于 `handle_ui`、`handle_ui` 先于 `draw_ui`**（`runtime.rs:264-270`；`lib.rs:51-71`）。因此：
   - `Drawer::render()` 里 `messager.send(...)` 的 Message **当帧**在 `handle_ui` 处理（例：`SceneWindowTryReceTextureId`，`scene_window.rs:365-367`）；
   - `Drawer::ui()` 里 send 的 Message **下一帧**才处理（例：`UpdateSceneWindowSize`，`scene_window.rs:188-191`）。
2. **`Engine::update()` 在 UI 之前**。UI 本帧写入 World 的值（`SceneView.size`、`SceneViewInput`）要等到下一帧 `Engine::update()` 才参与 extract / camera controller。实测后果：
   - 场景视图尺寸变一帧后 extract 才按新尺寸出 draws；本帧 play layer 用新尺寸渲染旧 draws（`scene_window.rs:372-421` 的 attachment 尺寸 vs `SceneView.draws`）；
   - 相机 orbit/zoom/fly 输入（`ui.rs:674-685` 写 `SceneViewInput`；`camera.rs:217-243` 在 PostUpdate 消费）**延迟一帧**才生效。
3. **egui multi-pass 风险**：egui 0.35 默认 `max_passes=2`，`run_dyn` 在 `request_discard()` 时会再跑一遍闭包（egui-0.35.0/src/context.rs:780-860、memory/mod.rs:331）。当前闭包里的 `graphics_commands.append(...)`、`handle_ui`（会 drain Messager + poll watcher + 改 World）、`draw_ui`（会 `Log::pop_front`）都不是幂等的。编辑器自己没调用 `request_discard`，但 `preferences_window.rs:229` 的 `egui::Grid` 首帧会触发（egui-0.35.0/src/grid.rs:454），`egui::Resize` 容器也会（containers/resize.rs:286）——多 pass 是现实路径，不是理论风险。
4. **Message 时序被帧边界放大**：`ui::Context::handle` 的 `while let Some(msg) = pop_front()`（`ui.rs:418`）在同一帧内处理 handler 自己新 push 的 Message（如 `OpenPreferenceWindow` → push `RefershPreferenceWindow`，`ui.rs:468-473`）；draw 里新 push 的要等下一帧。
5. **Quit 跨三层**：ToolBar 发 `Message::QuitEngine`（`tool_bar.rs:163-165`）→ `handle()` 里 `ui.send_viewport_cmd(Close)`（`ui.rs:459-461`）→ runtime 从 `root_output.commands` 里找 `ViewportCommand::Close` 再 `shutdown`（`runtime.rs:274-278,386-389`）。新架构应把它收敛成显式 `AppCommand::Quit`。
6. **每帧重建 view attachment 纹理**：play layer 每帧在 `present` 里为每个 view attachment `device.create_texture`（`render_pipeline.rs:386-405`），Scene/Game 窗口的 `GraphicsCommand` 也是每帧新建（`scene_window.rs:363-479`、`game_window.rs:179-265`）。这不是 UI 层责任，但说明“视图=每帧数据”这条契约被引擎固化了。
7. **`on_exit` 是空 TODO**（`lib.rs:73-76`）：关窗没有保存/清理钩子；新架构若要求“退出前落盘”，必须自己加生命周期协议，不要指望 runtime。

### 1.4 对 UI 架构的直接推论

- 新架构需要自己的**显式帧阶段**（例如 `BeginFrame → ApplyIntents → EngineUpdate → BuildUi → SnapshotQueries → Present`），而不是继续依赖“render 先于 handle 先于 draw”这种隐式顺序。
- MCP 计划中的 “Commands 在 draw 前、Queries 在 draw 后”（`phase3-middleware-architecture.md:176-208`）在**当前代码**只能保证“UI 状态本帧生效”；对 World 生效需要把 command 应用点提前到 `Engine::update()` 之前（§4.1.6）。
- 如果保留连续 redraw（很可能为了 MCP 帧边界可预期），`repaint_delay`/timer 可以降级为“每帧都 render”的实现细节；如果改为事件驱动，必须同时删掉 `runtime.rs:384` 的无条件 `request_redraw` 并正确 honor `repaint_delay`。

---

## 2. 引擎表面：新 UI 能站上去的东西

### 2.1 `Engine::update()` 的调度铁轨（`kairos_engine/src/schedule.rs`）

```text
MainScheduleOrder::labels（每帧一次，按序）         schedule.rs:170-185
  First → PreUpdate → RunFixedMainLoop → Update → PostUpdate → Extract → Last
Startup 只在第一帧跑一次（schedule.rs:204-214）。
FixedUpdate / FixedPostUpdate 不在 labels 里，由 RunFixedMainLoop 的独占 driver
按 FixedTime 累计跑 0..N 次（schedule.rs:246-287）。
```

- `First` 承载 `time_system`（`Time` 每帧只在此推进，`schedule.rs:194-196,321-331`）和 message pass（`message_update_system`，由 `FixedPostUpdate` 的 signal 门控，`schedule.rs:316-339`；细节见 `kairos_ecs/src/message/update.rs:23-63`）。
- `Extract` 是“本帧世界只读”的边界：`graphics::install` 注册 `reset_camera_views / extract_scene_view / extract_game_view`（`kairos_graphics/src/lib.rs:75-95`、`extract.rs:36-106`），`graphics::install_assets` 注册 `collect_graphics_asset_events`（`asset_events.rs:63-116,129-152`）。
- `MainThreadExecutor` 在 bootstrap 时捕获主线程（`schedule.rs:358-360`），说明“schedule 里可以有 !Send system”是引擎明确保留的能力。
- 编辑器只注册两个东西进 rails：`camera::install` 往 `PostUpdate` 加 `editor_camera_controller_system`（`camera.rs:194-204`）；`KairosGame::new` 往 `Update` 注册 gameplay systems 并生成游戏相机/场景（`lib.rs:32-37`、`kairos_game.rs:207-229`）。**没有** editor-owned 的 stage/system 之外的钩子。

### 2.2 编辑器触碰的 World 资源

**引擎侧安装、编辑器读/写：**

| Resource / component | 编辑器访问点（代表） | 方向 | 备注 |
|---|---|---|---|
| `Time` | `inspector_window.rs:178`（`engine.time().delta_time_secs()`）；`camera.rs:219-222`（系统内 `Res<Time>`） | 读 | 只由 `First` 的 `time_system` 推进 |
| `AssetServer` | `document.rs:59`、`code.rs:64`、`shader.rs:64`、`material.rs:1015/1020/1038/1330/1352/1453/1458`、`mesh.rs:256-282`、`audio.rs:239`、`font.rs:97`、`texture/mod.rs:330/834`、`syntax.rs:427`、`editor_assets/toml.rs:124`、gizmos `grid_plane.rs:54-56`/`axes_indicator.rs:135-138` | 读/load | `load::<T>(path)` / `add(value)`，异步 |
| `Assets<Text>` | 保存：`document.rs:177`、`code.rs:226`、`shader.rs:229`；读取：`document.rs:86`、`code.rs:96`、`shader.rs:98` | 读/写 | editor 安装的 asset type（`editor_assets/text.rs:67-70`） |
| `Assets<Toml>` | `ui/inspector/toml.rs:70,94,194` | 读/写 | editor 安装（`editor_assets/toml.rs:61`） |
| `Assets<SyntaxHighlightSettings>` | `syntax.rs:436`、`code.rs:106`、`shader.rs:108` | 读 | editor 安装（`syntax.rs:359`） |
| `Assets<TextureEdit>` | `ui/inspector/texture/mod.rs:297,367,842` | 读/写 | editor 安装（`texture/mod.rs:745-747`） |
| `Assets<Material>` / `Assets<SerializedMaterial>` | `material.rs:537,1335,1357,1381,1397,1461,1500` 等 | 读/写 | 用于 Inspector 预览/保存 |
| `Assets<Mesh>` / `Assets<Texture>` | `mesh.rs:313`；`material.rs:540,913` | 读 | 预览网格/缩略图 |
| `Assets<AudioExt>/<PcmData>/<AudioAsset>`、`AudioEngine` | `audio.rs:394-437` | 读/写 | 音频 Inspector 试听 |
| `Assets<Font>` | `font.rs:97,122` | 读 | 字体 Inspector |
| `SceneView` | `ui.rs:636-641`（size/close）、`scene_window.rs:188,372,495,518`（size/camera 绑定） | 读/写 size、camera | `draws` 由 Extract 重建，UI 只读 |
| `GameView` | `ui.rs:659-663`；`game_window.rs:137,188`；`kairos_game.rs:229`（绑相机） | 读/写 size、camera | 同上 |
| `CameraView`（component） | `scene_window.rs:414-417`、`game_window.rs:230-233` | 读 | Extract 写入的 view-projection |
| `EditorCameraController` / `Camera` / `LocalTransform`（entity） | `scene_window.rs:508-518` spawn；`scene_window.rs:534-541` 改 intrinsics/tuning | 读/写 | 相机是 World entity，不是 Drawer 状态 |
| `SceneViewInput` | `ui.rs:674-685` 写；`camera.rs:217-243` 消费并清零 | 写（UI 经 Message） | editor 安装（`camera.rs:194-197`） |

**editor `install` 一次性挂载的 5 项**（`lib.rs:107-113`，顺序在 `Engine::new` 之后、`KairosGame::new` 之前）：`Assets<Text>`、`Assets<Toml>`、`Assets<SyntaxHighlightSettings>`、`Assets<TextureEdit>`、`SceneViewInput` + camera controller system（`camera.rs:194-204`）。这条“引擎 bootstrap → 宿主 install → game assembly”的顺序是**显式契约**（`lib.rs:28-37,79-106`；抽取清单 §9 #16）。

**引擎安装但编辑器不直接碰**（了解即可）：`Schedules`/`MainScheduleOrder`、`MessageRegistry`、`AssetStages`、`GraphicsAssetEvents`、`PhysicsEngine`、各种 `Assets<Font/Material/…>` 的引擎侧类型、audio 资源等。渲染缓存（pipeline/texture/bind group）**不在 World**，在 `RenderPipeline` 结构里（`render_pipeline.rs:308-317`）。

### 2.3 编辑器当前如何改 World（三条路径）

1. **`Context::handle(&mut Engine, &egui::Ui)`**：单一 giant match，直接 `engine.world.resource_mut::<SceneView>()` 等（`ui.rs:636-663`），或把 `&mut engine.world` 传给 helper（`ui.rs:686-771`）。约 365 行（`ui.rs:410-774`）。
2. **`Drawer::render(&self, engine: &mut Engine, messager: &mut Messager)`**：签名允许 `&mut Engine`，实际多用于读（`Context::render` 遍历全部 drawer，`ui.rs:776-786`），但这是“渲染阶段也能改 World”的口子。
3. **`Drawer::ui(&self, …, engine: &Engine, …)`**：只读；需要写时只能 `messager.send(...)` 或内部可变性（`Cell`/`Mutex`/`Arc<Mutex<Option<…>>>`）。

### 2.4 资产异步、热重载与 `AssetEvent` 时序

- 加载：`AssetServer::load` 把任务丢到 `kairos_tasks` 的 `IoTaskPool`（多线程，无 tokio；ADR 0001；`kairos_asset/src/lib.rs:36-43`、`asset_processor.rs:50,1351-1353`）；结果经 channel 回到主线程，在 **`PreUpdate`** 的 `handle_internal_asset_events` 里插入 `Assets<T>`（`kairos_asset/src/install.rs:15-27`）。
- 事件：store 的 `Added/Modified/Removed/Unused` 在 **`PostUpdate`** 才 flush 成 `Message<AssetEvent<T>>`（`install.rs:21-23`）；message pass 在 **下一帧 `First`** 才换页（`schedule.rs:321-331` + `kairos_ecs/src/message/update.rs:56-63`），且被 `FixedPostUpdate` 的 signal 门控（`schedule.rs:316-339`），帧内 0 个 fixed step 时会顺延。**结论：UI 读 `AssetEvent` 看到的是“上一帧”的资产变化，不要假设当帧事件。**
- 热重载：watcher 线程（`notify-debouncer-full`，300ms 去抖）→ `AssetSourceEvent` → `handle_internal_asset_events`（PreUpdate）→ 重新加载 → 新值入 store → `AssetEvent::Modified` → 图形侧 `collect_graphics_asset_events` 在 `Extract` 收集（`asset_events.rs:63-116`）→ `present` 里 `invalidate_caches` 逐出缓存（`render_pipeline.rs:345-359`）。ADR 0006 明确：主 server 忽略纯 rename；embedded meta 不热重载；编辑器项目树**另起一条** watcher（ADR 0006 deviation 8，见 `project_watcher.rs:1-17`）。
- 项目树 watcher：notify 线程 + crossbeam unbounded channel（`project_watcher.rs:22,54,89-97`），由 `Context::handle` 每帧 `poll_external_changes()` 排空（`ui.rs:414-416`；`project_window.rs:159-171`）。这条 drain 点是**当前唯一的非 Message 每帧副作用**，新架构如果移走 `Context::handle` 必须补上等价 hook。

### 2.5 线程 / 通道清单

| 机制 | 位置 | 方向 | 约束 |
|---|---|---|---|
| winit `EventLoopProxy<EditorRuntimeEvent>` | `runtime.rs:75-82,103-109,192-205,468-480` | 任意线程 → 主线程 | 唯一的“唤醒 UI”通道；目前用于 repaint 与 GPU crash |
| 无锁 main-thread 状态 | `Editor`/`ui::Context`/`EditorRuntime` | main only | 不是 Send/Sync，不能跨线程共享 |
| `kairos_tasks::IoTaskPool` | `kairos_asset` 内部 | 主 → worker（加载） → 主（PreUpdate 插值） | 不碰 World；ADR 0001 禁止 tokio |
| crossbeam channel | `project_watcher.rs:54,89-97` | notify 线程 → 主（`Context::handle`） | 只传 `()` 信号，重扫在主线程 |
| `tokio::sync::oneshot` | `scene_window.rs:473-477,545-569`、`game_window.rs:259-293`、`mesh.rs:469`、`material.rs:1228` | present（主）→ UI（主，try_recv） | 仅传 `EguiTextureHandle`；没有 `tokio::spawn` |
| `std::sync::mpsc` | `EguiTextureHandle`（`egui_texture_handle.rs:3-21`） | Drop → present 回收 texture id | Drop 可在任意线程，真正的 free 在主线程 |
| `Arc<parking_lot::Mutex<Option<…>>>` | `ui.rs:152-205`（Message payload）、`project_window.rs:60`、`tool_bar.rs:81` | 同线程为主 | 是“状态没有边界”的症状，不是既定的线程模型 |
| `Arc<Mutex<Option<RenderPipeline>>>` | `runtime.rs:87,249-371` | 主线程 | 整帧持锁；任何 effect 不得重入 |

### 2.6 `Log`：名义上的控制台缓冲，实际几乎空转

- `Log` 定义：`VecDeque<LogMessage>`（`log.rs:25-27,52-74`），`push` 同时 `eprintln!` + 入队；`pop_front` 用于消费（`log.rs:82-84`）。
- 唯一生产者：`about_window.rs:95-96`（`// TODO: test log`，每帧写一条）。
- 唯一消费者：`console_window.rs:78-82`（每帧 `while let Some(log) = log.pop_front()` 后画 label；Console 窗口关闭时**没人消费，VecDeque 无限增长**）。
- 真实日志全部走 `log` crate → `env_logger` → stderr（`main.rs:9-12`），MCP 计划也确认 crash/panic 走 notification、其余靠 `get_console_logs` 轮询（`phase2-observability.md:102-128`）。
- **结论**：MCP `get_console_logs` / `clear_console` / `wait_for_console` 所需要的能力，今天一个都不存在；新架构必须把“可观测日志”做成 Store 的只读快照 API，并由 `log` crate 的 capture layer 或显式 logger 填充。

### 2.7 渲染命令管线（UI 与 GPU 的唯一接口）

```text
Drawer::render() / Inspector::render()  ──►  GraphicsCommand（纯数据：attachment / pass / draw / egui）
                                              runtime.rs:265 收集；ui.rs:776-786 遍历全部 drawer
egui 自己                              ──►  另一个 GraphicsCommand（draw_egui + output_to_framebuffer）
                                              runtime.rs:316-348
                                      ──►  GraphicsGraph::build(commands)          runtime.rs:350
                                      ──►  RenderPipeline::present(&World, output, graph)  runtime.rs:351
                                           ├─ invalidate_caches(world)                render_pipeline.rs:355
                                           ├─ 每 attachment 新建 wgpu texture          :386-405
                                           ├─ 图节点 DFS 执行 render pass / draw       :463-549
                                           ├─ BindAttachmentToEgui → register_native_texture +
                                           │    oneshot send(EguiTextureHandle)        :526-544
                                           └─ queue.submit / error scope pop（block_on） :551-569
```

约束：UI 不能直接持有 wgpu 资源；`EguiTextureHandle` 是唯一桥（`id()` 给 `ui.painter().image` 用，Drop 归还 id）。凡是要读回 GPU（截图、picking、纹理字节）的需求，都没有现成接口，必须扩展渲染侧。

---

## 3. Model 边界证据：编辑器状态今天在哪

### 3.1 六个存放位置

| # | 位置 | 例子 | 谁改 | 能否序列化 |
|---|---|---|---|---|
| A | World resources / components | `SceneView`、`GameView`、`SceneViewInput`、`Assets<Text/Toml/TextureEdit/…>`、`EditorCameraController` | schedule systems + `Context::handle`（`&mut World`） | 部分（asset 类型可 serde） |
| B | Drawer structs 的 model | `ProjectWindowModel`、`InspectorWindowModel`、`SceneWindowModel`、`GameWindowModel`、`PreferencesModel`、`ToolBarModel` | Drawer 方法（`&self` + interior mutability）或 `Context::get_window_mut`（`ui.rs:885-897`） | 否（含 `Box<dyn Inspector>`、oneshot receiver、`Cell`） |
| C | `ui::Context` | `drawers: Vec<Box<dyn Drawer>>`、`ids: TypeIdMap<usize>`、`actives: Vec<bool>`、`tab_tree: DockState<usize>`、`layout: EditorLayout`、`dialogs`、`global_styles`、`messager`（`ui.rs:326-336`） | `Context` 方法 | 否 |
| D | `Editor` | `engine: Engine`、`ui_context: ui::Context`、`log: Log`（`lib.rs:19-23`） | `Editor` 方法 / runtime | 否 |
| E | egui 内存 | `docking_tab::state::State`（`ctx.data_mut` 里的 DnD 瞬态，`state.rs:17-33`）、各 widget 内部状态 | egui | 否（temp storage） |
| F | 磁盘 | `Library/asset_registry.toml`（`asset_registry.rs:199-208`）、`Preferences/Styles/*.toml`（`ui/paths.rs:1-30`，每个 Drawer 自己 load/save，例 `scene_window.rs:333-355`）、项目文件本身 | 各 Drawer / `AssetRegistry::save` | 是（但无统一入口/无 schema 版本） |

### 3.2 B 类：Drawer 里的状态（要被 Store 接管的候选）

- **ProjectWindow**（`project_window.rs:49-71`）：`style`、`asset_registry`、`project_path_graph`、`selected_node`、`active_directory`、`renaming_node`、`renaming_buffer: Option<Arc<Mutex<String>>>`、`force_expand_to: Cell<Option<NodeIndex>>`、`drag: Option<Drag<PathBuf>>`、`watcher`、`refresh_pending: Cell<bool>`。其中 **selection / active_directory / renaming / drag 是编辑器状态**；registry/graph 是数据模型；watcher 是 I/O 资源。
- **InspectorWindow**（`inspector_window.rs:47-55`）：`style`、`selected: Option<InspectorNodeInfo>`。`InspectorNodeInfo` 内 `inspector: Box<dyn Inspector>`（`inspector_window.rs:24-30`）——**选中 + 全部编辑草稿状态都在这一个 Box 里**（各 inspector 自己持有 `Arc<Mutex<Option<…>>>` 草稿，如 `document.rs:37,170`、`texture/mod.rs:80,189`、`material.rs:290`），保存时再经 Message 把 `Arc<Mutex<Option<...>>>` 传回 handler 写 World（`ui.rs:152-205,686-771`）。
- **SceneWindow**（`scene_window.rs:54-65`）：`style`、`rt_handle: Option<EguiTextureHandle>`、`egui_bind_tex_recever: Option<oneshot::Receiver<...>>`、`gizmos`。`rt_handle/receiver` 是渲染握手状态。
- **GameWindow**（`game_window.rs:29-37`）：同上（style + rt_handle + receiver）。
- **PreferencesWindow**（`preferences_window.rs:47-51`）：`style`、`style_pages: Option<Vec<StylePage>>`（所有 Drawer 的 style 字段集合）、`selected_id`。
- **ToolBar**（`tool_bar.rs:34-36,76-82`）：`style` + 预加载 icon + `icon_texture: Mutex<Option<egui::TextureHandle>>`（egui 资源缓存放 Drawer）。
- **Dialog**（`dialog.rs:14-35`）：`ConfirmDialogWindow` 里存 `Cell<Option<Message>>` + `Cell<Option<FnOnce>>` 回调——**“消息回调机制”的缩影**：闭包/Message 存进 UI 结构，点击时再 fire。
- 其余：About/Hierarchy/Console/Layout 容器（`layout.rs:18-21` 的四个空 struct 只是占位 Drawer）。

### 3.3 C/D 类：`ui::Context` 与 `Editor`

- `ui::Context` 是**唯一事实上的“编辑器单例”**：但它用 `TypeIdMap<usize>` 做 drawer 索引（`ui.rs:305-336,788-798`），用 `actives: Vec<bool>` 表示关闭（`ui.rs:855-863`），用 `DockState<usize>` 表示 tab 布局（`ui.rs:332`），用 `EditorLayout` 存四个 `Zone`（`layout.rs:199-235`），用 `dialogs: Vec<Box<dyn dialog::Dialog>>` 存模态（`ui.rs:334,400-407`）。这些就是 MCP `get_editor_state` / `get_dock_layout` 要的数据，但**没有 snapshot API**。
- `Editor` 只聚了 `engine + ui_context + log` 三个字段（`lib.rs:19-23`），生命周期钩子仅 `on_exit` TODO（`lib.rs:73-76`）。

### 3.4 E 类：egui 内存里的“隐形状态”

- 停靠系统的拖拽/悬停瞬态放在 egui `Context::data_mut` 的 temp storage 里（`docking_tab/state.rs:17-33`），不是 `DockState` 的字段。
- 每个 `egui::CollapsingHeader`/`ScrollArea`/`TextEdit` 的展开、滚动、光标都在 egui 自己的 memory 里；`ProjectWindowModel.force_expand_to`（`Cell<Option<NodeIndex>>`，`project_window.rs:61-63`）就是用 `Cell` 绕过 `&self` 向 egui 反向“推状态”的例子。
- 对 Store 的含义：**能进 Store 的是“业务状态”，纯 widget 瞬态不该被 Store 复制**；但“展开/选中/滚动位置”若属于可序列化布局，则必须从 egui memory 提升到 Store 并在 draw 时做一次同步。

### 3.5 支持“单一 EditorState/Store”的 10 条证据

1. MCP 计划把“状态分散在 `KairosEngine → ui_context → 各 Drawer`”列为架构问题 #1（`phase3-middleware-architecture.md:11-16`）。
2. Query 派发计划是“遍历 drawer，第一个能答的赢”（`phase3-middleware-architecture.md:85-93,161-171`），这要求**每个状态都有唯一 owner**，否则同一查询会被多个 drawer 抢答；现状 `ProjectWindow` 与 `InspectorWindow` 都持有 selection 的一部分（后者是复制品，`ui.rs:545-556`）。
3. 跨模块状态靠 `Context::handle` 里的 `get_window_mut::<T>()` 直接下钻（`ui.rs:885-897`）：`Selection` 流是 ProjectWindow → InspectorWindow（`ui.rs:545-556`），`UpdateUIStyle` 流是 PreferencesWindow → 所有 Drawer（`ui.rs:493-514`）。这些“横向访问”没有事务边界。
4. 同一条状态有多个副本：`SceneView.size`（World）是唯一真值，但 SceneWindow 每次 draw 用 `engine.world.resource::<SceneView>()` 读它（`scene_window.rs:188`）；`SceneViewInput` 用“累加-清零”协议（`camera.rs:154-180`）来掩盖“UI 写、系统消费”的时序。
5. 保存路径把 UI 草稿以 `Arc<Mutex<Option<T>>>` 塞进 Message（`ui.rs:152-205`），handler 再 match 回具体类型并写 World（`ui.rs:686-771`）；如果状态集中，保存就是一个 `save(asset_id, value)` command。
6. 每个 Drawer 的 `get_style_fileds()/update_style()`（`ui.rs:284-286`；实现例 `scene_window.rs:219-356`）把“样式状态”复制成 `StyleField` DTO，再由 Preferences 汇总后回写每个 Drawer 并各自写 TOML（`ui.rs:477-514`）。样式没有单一 store，也没有 schema/版本。
7. `Log` 作为 editor 字段被一路 `&mut` 穿过 `Context::darw → DockArea → TabDrawer → Drawer::ui`（`ui.rs:227,268,368`；`docking_tab/tab_drawer.rs:25-33`；`docking_tab.rs:188,209,458,478,771,808,1092,2276`），只为了 Console 能 `pop_front`；这正是“状态而非服务”的典型。
8. 3 个窗口模型的状态其实是同一份“编辑器操作意图”：selection（ProjectWindow/InspectorWindow）、tab 布局（Context）、camera（World 组件 + SceneWindow style）、preferences（PreferencesWindow）——MCP `get_editor_state` 需要一次性读出它们（`phase2-tools-catalog.md:993-1044`），没有 store 就只能拼凑。
9. 可测试性：`Context::new` 一创建就压入 7 条 bootstrap Message（`ui.rs:342-349`），UI 的初始状态只能靠“跑第一帧”才出现；没有 store 无法对“启动后应有哪些 tab/selection”做纯逻辑断言。
10. 重写目标本身：用户认为 Messager（= 状态变更机制）与 MVC/state 结构过时；`Message` + `get_window_mut` + `Box<dyn Drawer>` 的组合就是现状的“模型层”。把 model 从 Drawer 抽出是重写的核心动作。

### 3.6 不该进 Store 的东西（边界）

- **引擎真值不能镜像**：`SceneView.draws`（每帧重建）、`CameraView`（Extract 派生）、`Assets<*>`（异步加载/热重载）、`Time`（每帧推进）——Store 只能持有**当帧快照**或查询入口，不能持有会被引擎修改的副本。
- **GPU 资源**：`EguiTextureHandle`/`TextureId`、RenderPipeline 缓存；只允许“句柄 + 生命周期由渲染侧管”的桥接。
- **纯 egui 瞬态**：widget hover/动画/滚动偏移，若无持久化需求不要提升。
- **后台线程句柄**（`ProjectTreeWatcher`、oneshot receiver）：属于 host/IO 服务，不是 Model 数据；Store 内应只保留“watcher 是否可用/最近一次变更序号”这类可快照信息。

---

## 4. 既有计划施加的约束

### 4.1 `docs/plan/kairos-editor-mcp/`（6 份文档）

#### 4.1.1 问题定义与状态分布（`phase3-middleware-architecture.md:11-42`）

计划把现状画成 `KairosEngine{engine, game, ui_context, log}`，并指出：
- 编辑器状态散落在 `ui_context → 各 Drawer（ProjectWindow.model / InspectorWindow.model / SceneWindow / GameWindow / ConsoleWindow / ToolBar / DockState）`；
- MCP Server 在独立进程/线程，不能直接访问编辑器内存；
- 所有状态读写必须在 egui render loop 中执行；
- **架构约束：不用 `Arc<RwLock<World>>`，模块独立，可测试**。

#### 4.1.2 `EditorChannel` 契约（`phase3-middleware-architecture.md:96-159,278-299`）

```text
EditorQuery  (read-only, 每个查询一个 enum variant)
EditorCommand(side effects, 需要 &mut)
EditorResponse(类型化结果；Error(String) 兜底)
ChannelMessage::Query(q, oneshot) / Command(c, oneshot)
EditorChannel::query/command  → mpsc send + oneshot await
编辑器侧 EditorChannelPlugin::process_*  → try_recv 循环 + dispatch_* + oneshot reply
```

原型已可编译、6/6 测试通过（`docs/plan/kairos-editor-mcp/handoff.md:31-33,42-54`；原型代码 `prototypes/kairos-editor-mcp-middleware/src/{types,channel}.rs`）。**这是新架构必须兼容的 API 边界**：即便内部换成 Store，`EditorQuery/EditorCommand/EditorResponse` 的语义与“快照序列化、无生命周期引用”的要求不变。

#### 4.1.3 帧边界语义（`phase3-event-loop-integration.md:19-105`）

- 计划 hook：`process_commands` 在 `render_ui()/handle_ui()/draw_ui()` 之前；`process_queries` 在 `draw_ui()` 之后。
- 语义：commands 本帧渲染可见；queries 读 draw 之后的最新状态；单帧最大延迟 16ms；`try_recv` 一帧内消费全部积压；unbounded channel。
- 三种时序：逐操作（3 帧/3 次 await）、batch（1 帧）、无 await 连发（不确定）。
- 超时：普通工具无显式超时（依赖客户端/崩溃时 channel drop）；wait 类工具自带 `timeout_secs`（默认 5s，poll 100ms，最小 16ms）。
- 编辑器重启：MCP 不自动重连，Agent 重新 `attach`（`phase3-event-loop-integration.md:256-294`）。

#### 4.1.4 可观测性（`phase2-observability.md`）

- `query_tree/get_node` 覆盖 widget 定位；`screenshot` 视觉验证；日志注入靠 Agent 改源码 + 重新编译，再 `get_console_logs`；
- 操作日志走 **stderr**（stdout 被 JSON-RPC 独占）；
- crash/panic → MCP notification（编辑器要装 panic hook；GPU device lost 已有 callback，`runtime.rs:192-205`）；layout overflow/GPU error/asset 失败 → 轮询 `get_console_logs`；
- 明确“不新增 MCP 工具”，但**要求 `get_console_logs` 可 level/pattern/after/limit 过滤**（`phase2-tools-catalog.md:899-990`）。

#### 4.1.5 工具/资源对状态的要求（缺口表）

| MCP 工具/资源（`phase2-tools-catalog.md:1048-1069,1127-1170`） | 期望的数据源（catalog Appendix A，`:1272-1287`） | 当前代码事实 | 缺口 |
|---|---|---|---|
| `get_project_tree` / `editor://project/tree` | ProjectWindow 的 `ProjectPathGraph` | 在 `ProjectWindowModel`（`project_window.rs:52`） | 需要从 Store 导出快照；`NodeIndex` 跨重扫不稳定（`project_window.rs:156-158,193-199`），snapshot 应用 GUID/路径 |
| `get_asset_registry` / `get_asset_info` | `AssetRegistry` | 在 `ProjectWindowModel`（`project_window.rs:51`），持久化 `Library/asset_registry.toml`（`asset_registry.rs:202-208`） | 需要 snapshot + 按 kind/search 过滤 |
| `select_asset` / `get_selected_asset` | ProjectWindow.selected | `project_window.rs:54` | Store 化；且 Inspector 是复制品，要保证一处真值 |
| `open_asset` / `inspect_asset` / `get_inspector_state` | InspectorWindow.selected + per-kind Inspector | `inspector_window.rs:47-55`、`ui/inspector.rs:24-50` | Inspector 草稿状态不可序列化；需要“字段模型”供 MCP 读写（P1 `set_asset_field`） |
| `get_scene_camera` / `camera_orbit/zoom/fly` / `scene_screenshot` | catalog 假设 SceneWindow 持有 `SceneCamera`（`:1281`） | **实际**在 World：`EditorCameraController` 组件（`camera.rs:141-152`）+ `SceneView.camera`（`scene_window.rs:495-518`）；输入经 `SceneViewInput`（`ui.rs:674-685`） | 相机状态归属需统一（World 还是 Store）；commands 必须落到“Engine::update 之前”才能当帧生效 |
| `get_game_state` / `game_screenshot` | GameWindow viewport + running | `GameView.size` 在 World（`ui.rs:659-663`、`game_window.rs:137`)；游戏永远运行（`KairosGame::new` 在启动时装配 demo scene + 相机，`lib.rs:32-37`、`kairos_game.rs:207-229`） | **没有 play/stop 状态**；`is_running` 无数据源 |
| `get_console_logs` / `clear_console` / `wait_for_console` | `Log`（`LogEntry{level,message,caller,timestamp}`，原型 `types.rs:109-115`） | `Log` 只有 `VecDeque<LogMessage>`（`log.rs:25-27`），无过滤/无 timestamp/无 clear/被 Console pop 掉；真实日志在 stderr | 需要结构化、环形、可过滤、可 snapshot 的日志资源 |
| `get_editor_state` / `editor://state` | open tabs + project root + selection + peer info | tabs 在 `ui::Context`（`DockState`/`actives`/`layout`，`ui.rs:326-336,855-863`）；project root = graph 的 `scan_root()`（`project_path_tree.rs:85-87`，默认 cwd）；selection 见上 | 需要 Store 提供聚合 snapshot；**没有 uptime/peer info**（MCP Server 侧可补） |
| `editor://layout` / `open_tab/close_tab`（P1） | DockState + WindowState | `DockState<usize>` + `actives: Vec<bool>`（`ui.rs:330-331`）；`WindowState` 是 per-window egui 状态（`docking_tab/window_state.rs:4-26`） | 需要 layout 的稳定 ID（drawer 的 TypeId/名称），不能暴露内部 `usize` |
| P2 preferences / layout 持久化 | 各 Drawer style TOML | 每 Drawer 自己 load/save（例 `scene_window.rs:299-356`、`paths.rs:1-30`） | 需要统一 config store + schema 版本 |

#### 4.1.6 计划与当前代码的 3 处不一致（新设计要修正，不要照抄）

1. **`handle_asset_server()` 已不存在**：`phase3-middleware-architecture.md:176-208` 的示意里仍有 `self.kairos_engine.handle_asset_server()`；现状资产 pump 全在 `Engine::update()` 的 rails 内（`kairos_asset/src/install.rs:15-27`、`runtime.rs:242`），UI 之前已经跑完。新设计不应再引入“每帧手动 pump 资产”的钩子。
2. **`Drawer` 上的 `as_query_handler()` 与“单一状态入口”冲突**：计划让每个 Drawer 自己答 query（`phase3-middleware-architecture.md:105-124,161-171`），但本次重写的目标正是把状态从 Drawer 抽走。建议把 `QueryHandler` 挂在 Store（或 Store + World snapshot 组合）上，`Drawer` 只保留 `as_query_handler` 兼容 shim（可返回 None）。
3. **camera 状态位置**：计划假设 `SceneWindowModel` 持有相机（catalog `:1281`），现状在 World 组件 + `SceneView.camera`。新设计必须先定“相机状态的唯一 owner”，再决定 MCP `get_scene_camera` 的 snapshot 来源。

### 4.2 `docs/plan/kairos-editor-claw/`（与编辑器 UI 基本无关）

- 目录名是 `kairos-editor-claw`，文档内实体名一律是 `kairos_editor_claw`（`claw-phase1-requirements.md:57` 起）。
- 它做的是“飞书/微信 → 后台 daemon → **Zed 的 AI Agent**”的远程控制（`README.md:21-33`、`handoff-claw.md:7-15`），**目标不是 Kairos editor**；集成手段是 Fork Zed + IPC（`D-001`，`claw-phase1-requirements.md:150-170`），Agent 后端是可插拔 trait（`D-002`，`:200-218`）。
- 对本次 UI 架构**没有直接约束**；只有两点模式可借：
  1. 外部自动化面要**独立于宿主**、走“trait 边界 + 通道”（它的测试接缝就是 `AgentBackend` trait，`handoff-claw.md:51`）；对应编辑器侧就是 `EditorChannel` 与 `QueryHandler` 的 trait 边界；
  2. 外部工具的生命周期（cargo build/run、进程监控、崩溃通知）由独立进程负责，编辑器只暴露稳定接口（MCP 计划已经承担这一层）。
- **不要**把 claw 依赖引进 `kairos_editor`；也不要在 UI 架构里为 claw 预留“Zed 专用”逻辑。

### 4.3 `docs/research/editor-crate-extraction-inventory.md`

虽然写于抽取前，但它的依赖/借用面清点与新架构直接相关，且当前代码结构与其结论一致（已抽查）：

- **`&Engine` 面**（§7.3，`:993-1042`）：`Drawer::ui(..., engine: &Engine, ...)`、`Drawer::render(..., engine: &mut Engine, ...)`、`Context::darw(&mut self, ui, engine: &Engine, log)`、`Context::handle(&mut self, engine: &mut Engine, ui)`、`Context::render(&mut self, engine: &mut Engine)`、`Context::show_tab<T>(&mut self, engine: &mut Engine, ...)`；`DockArea` 全程传 `&Engine`。**`&mut Engine` 只在 handle/render/show_tab 出现**，这就是“UI 不能直接改 World”的现有做法。
- **`engine.world` 面**（§7.4，`:1044-1055`）：`resource::<SceneView/GameView/...>`、`resource_mut::<SceneView/GameView/Assets<...>/AudioEngine>`、`get_resource_or_init::<SceneViewInput>`、以及把 `&mut engine.world` 传给 helper（`ui.rs:686-771`、`runtime.rs:187-191,351`）。
- **crate 依赖方向**（§9 #5/#16、`docs/agents/testing.md:50`）：`kairos_editor` 单向依赖 `kairos_engine`；引擎侧只保留“引擎 bootstrap”，编辑器自己的 assets/camera 通过 `kairos_editor::install` 挂载（`lib.rs:107-113`）。**任何“让引擎调用 UI 回调”的设计都违反这个方向。**
- **潜伏问题**（§9，`:1170-1190`）里与新架构相关的：#10 `main.rs` 的 `#[tokio::main]` 让 tokio 成为编辑器直接依赖（现状仍是，`main.rs:7-8`；`Cargo.toml` `tokio features=["full"]`）；#15 `KairosGame` 留引擎但只被编辑器路径调用（现状 `lib.rs:37`）；#16 install 顺序是隐式前置条件链（现状 `lib.rs:28-37,79-106`）；#11 `kairos_engine::math` 依赖 egui 颜色转换（UI 库渗进引擎，新架构不应扩大这种耦合）。
- **测试事实**（§7.6，`:1069-1082`）：没有任何测试构造 `Engine`；`Engine::new()` 被刻意回避（`camera/test.rs:10-11`、`engine/test.rs:31-33`）。这直接支撑 §6 的“headless seam”要求。

### 4.4 `docs/research/egui-mcp-analysis.md`

- **交互方式**：AccessKit 树是 retained 快照，egui 的 immediate-mode `Response` 不参与；事件注入是 `egui::Event` 队列，下一帧生效（`:155-185`）。**因此新 UI 的交互控件必须是“egui 原生、可被 AccessKit 看见”的**；自绘控件/GPU 视图（Scene/Game）对 AccessKit 不可见，只能靠截图或专用通道。
- **集成点**：`egui_inspection::InspectionPlugin`（`ctx.add_plugin(...)`）+ `EGUI_INSPECTION=1`，或复用 `UiServer` + 自定义 `Transport`（`:222-230,245-249`）。
- **架构选择**：Design A（独立 MCP Server + 复用 UiServer + 独立 EditorTools）推荐；编辑器专属状态走 side channel/独立协议，不改 egui_inspection（`:267-323,340-380`）。
- **风险**（`:522-535`）：AccessKit 覆盖率需审计；Scene 交互需 GPU picking；ECS 状态外部读取需要“只读查询/命令通道”（**不要 `Arc<RwLock<World>>`**）；单 viewport 假设（当前 runtime 只处理 `ViewportId::ROOT`，`runtime.rs:274-300`）；`egui_inspection` 0.35 尚早期、协议可能变。
- **版本事实**：workspace 当前没有任何 `egui_inspection`/`egui_mcp`/`egui_kittest` 依赖（grep 所有 Cargo.toml 无命中）；接入 MCP 时是新依赖，不影响现有架构。

---

## 5. 线程与借用约束

### 5.1 主线程模型

```text
main thread (winit EventLoop + #[tokio::main] wrapper)
├── EditorRuntime (ApplicationHandler, 非 Send)       runtime.rs:452-537
│   ├── egui::Context / egui_winit::State            :89-90
│   ├── Arc<Mutex<Option<RenderPipeline>>> (wgpu)    :87
│   └── Editor { Engine{World, InputEngine}, ui::Context, Log }   lib.rs:19-23
├── IoTaskPool (kairos_tasks)  ← asset load / processor   (worker threads)
├── notify watcher thread      ← project tree events       project_watcher.rs
└── EventLoopProxy<EditorRuntimeEvent>  ← 唯一跨线程唤醒 UI 的通道   runtime.rs:75-82
```

**不可违反的规则（由代码事实推出）：**

1. World / `egui::Context` / wgpu 只能在 main thread 触达；不存在 `Send` 的 World 访问器（`Engine` 无 `Sync` 共享面；`RenderPipeline::present` 需要 `&World` 且在主线程被调用）。
2. 后台线程只能通过 channel 回传**纯数据**（`Send + 'static`）；资产加载结果由 `handle_internal_asset_events` 在 `PreUpdate` 主线程插入（`kairos_asset/src/install.rs:15-27`）。
3. 跨线程唤醒 UI 唯一正路是 `EventLoopProxy`（`runtime.rs:103-109,468-480`）；不要从后台线程直接调 egui 的 repaint API（repaint callback 已把唤醒转给 proxy，语义会乱）。
4. 帧内不得阻塞：`render_pipeline` 被整帧持锁（`runtime.rs:249-371`），`present` 内已有 `pollster::block_on`（`render_pipeline.rs:565`）；再加 `recv()`/`block_on` 会把 MCP 请求延迟直接放大到 GPU 提交上。
5. 纹理/GPU 资源生命周期只能在 main thread 完成：`EguiTextureHandle` 由 present 创建（`render_pipeline.rs:526-544`），Drop 只把 id 发回 channel（`egui_texture_handle.rs:3-21`），真正的 `free_texture` 在下次 present 的主线程做（`render_pipeline.rs:357-359,576-580`）。
6. `Log` 是 `VecDeque`（非 Sync）；MCP 查询必须在主线程做 snapshot，再 `Send` 到 server 线程。

### 5.2 effect / task 抽象必须满足的规则

1. **命令是数据**：`enum EditorCommand`（或等价）必须 `Send`，不含 `&mut`/生命周期/闭包；执行器在主线程帧内按阶段应用。
2. **有结果**：每个 command 配 `oneshot<Result<Response, Error>>`；失败不 panic（当前反例：`ui.rs:865-869` `create_ui_failed` 直接 panic；`tool_bar.rs:206-208` `todo!()` 可从 UI/MCP 触发）。
3. **有幂等/幂等边界**：跨多 pass、重放、编辑器重启时，command 需带 id/版本号；对资产的写操作以 `AssetPath`/GUID 为目标，不用 `NodeIndex` 这类会随重扫失效的索引（现状 `project_window.rs:156-158,193-199` 已有这个教训）。
4. **阶段明确**：同一条 command 必须能声明“何时生效”——UI-only（draw 前）、World-before-update（影响本帧 extract）、Frame-end（只影响下一帧）。当前架构没有这个概念，是 1 帧延迟的根源。
5. **可观测**：command/query 的 dispatch 结果、耗时、错误要进结构化日志（stderr 操作日志 + 未来 `get_console_logs`）。
6. **不可重入**：effect 执行期间不得再次触发同一 effect 队列的递归 drain（现状 `Context::handle` 的 `while pop_front` 会在 handler 内部 push 新消息时递归消费，`ui.rs:418-473`；新架构要显式定义“本轮新产生 effect 是否本轮执行”）。
7. **可取消**：长时间任务（资产加载、cargo build、processor、截图）要能取消/超时，且取消后不留半状态；现状 asset 加载由 handle 保活（ADR 0001），编辑器侧没有统一任务表。
8. **单 owner 写**：每个状态字段只有一个写入者（Store 的 reducer/command handler），避免 `SceneView.size` 这种“UI 写、系统读、窗口也读”的隐式协议扩散。

---

## 6. 测试现状与可测试性

### 6.1 基线事实

- 编辑器现有 **74 个测试**：`project_path_tree` 32、audio inspector 23、material inspector 4、ide_detection 3、camera 3、project_watcher 2，其余 7 个单测（`grep -c "#[test]" kairos_editor/src`；`docs/agents/testing.md:96-100` 记 74 个）。
- 运行方式：`cargo test-crate kairos_editor`（= `cargo test --lib --bins --tests -p`，`docs/agents/testing.md:5-15`），**不含 doctest**；全量 `cargo test-full` ~183s 是 merge gate，不是实现步骤（`:72-76`）。文档明确：**本仓库没有 CI**（`:102`）。
- 全部编辑器测试都是 **World 级 / 纯逻辑**测试：`camera/test.rs:1-15` 明说“bare World、headless、刻意不调 `Engine::new()`（会建 audio device）、不碰 wgpu”；它手工搭 `First/PostUpdate/Extract` 三段并 `clear_trackers()`（`camera/test.rs:68-88`）。`engine/test.rs:31-33` 只测 `build_world()` 的资源挂载。
- 没有任何测试构造 `Editor`、`EditorRuntime`、`ui::Context`，也没有任何 egui/window/GPU 测试；`egui::Context` 只在 `Editor::new`/`ui::Context::new`/`runtime` 里出现（grep 结果）。

### 6.2 为什么当前 UI 代码不可测

1. `Editor::new` 需要 `egui::Context`，并且内部 `Engine::new()` 会开音频设备（`lib.rs:26-45`、`engine.rs:10-23`）；打不开设备就构造失败。
2. `Drawer::ui` 的签名强制 `&Engine` + `&mut Log` + `&mut Messager` + `&UIReader`（`ui.rs:262-269`），没有“只拿 `&World` + 纯状态”的入口。
3. `ui::Context::new` 读 cwd 下的 `Preferences/Styles/*.toml` 与字体文件（`ui.rs:339-353`、`global_styles.rs:15-27`、各窗口 `Style::new()`）；`ProjectPathGraph::new` 扫描 cwd（`project_path_tree.rs:42-44`）；测试没有注入点。
4. 错误处理是 native dialog + `panic!`（`ui.rs:865-869`），创建失败无法断言。
5. 渲染需要 `RenderPipeline`（wgpu device/surface），`EditorRuntime::redraw` 与 window 强绑定（`runtime.rs:153-228`）。

### 6.3 egui 0.35 的 headless 能力（可用的测试基础）

- `egui::Context::default()` + `ctx.run_ui(RawInput::default(), |ui| { ... })` 可以在**没有 backend/renderer** 的情况下跑一帧并产出 `FullOutput`（egui-0.35.0/src/context.rs:773-800 的 doc example；本机 cargo registry）。
- 但 `run_ui` 一帧可能多 pass（`max_passes=2`，memory/mod.rs:331；`run_dyn` 循环 context.rs:810-860），写测试时要注意副作用次数。
- 文本/字体：需要 `egui::Context` 装字体（默认字体即可）；图片 loader 需要 `egui_extras::install_image_loaders`（`runtime.rs:100-101`），headless 下可跳过或只测布局。
- 结论：**“纯逻辑 Store + headless egui pass”是可测的**，前提是把 `Engine`/`Log`/fs 从绘制签名里剥掉；不能指望直接测现有 `Drawer`。

### 6.4 新架构必须提供的 test seam

| Seam | 用途 | 现状对应 |
|---|---|---|
| **纯逻辑 core**（Store + reducers + commands/queries） | 状态迁移、选择、布局、日志过滤，无需 egui/wgpu | 不存在；现存最接近的是 `project_path_tree` 的 32 个纯测试 |
| **headless egui pass**（`egui::Context::default()` + `RawInput`） | 面板构建、AccessKit 节点、布局不变量 | 不存在；需要 `Drawer::ui` 去掉 `&Engine` 依赖 |
| **World-level harness**（bare `World` + 手工 stage，照 `camera/test.rs`） | camera controller、extract、view 契约 | 已有（`camera/test.rs`、`engine/test.rs`） |
| **mock EditorChannel/QueryHandler**（照 MCP 计划） | dispatch 逻辑、错误路径、超时 | 原型已有（`prototypes/.../types.rs:215-218`；`phase3-middleware-architecture.md:303-343`） |
| **MCP e2e**（egui_inspection + AccessKit 树 + 日志） | UI 交互验收的**唯一**计划路径（`phase1-requirements.md:154-159`） | 未接入；需要 `InspectionPlugin` + 新依赖 |

### 6.5 测试命令约定（重写后仍适用）

- 只测改动的 crate：`cargo test-crate kairos_editor`；若改了 engine/asset 等被依赖 crate，再加 `cargo check --workspace --all-targets`（`docs/agents/testing.md:5-15,61-70`）。
- 不要从 workspace root 跑裸 `cargo test`；不要跑 `cargo test-full`（`:13,72-76`）。
- 新 UI crate 若引入渲染/窗口相关 dev-dependency，必须确认 `cargo test-crate` 不要求 GPU/window（例如不要用需要 device 的集成测试当默认测试）。

---

## 7. 待 Lead 决策 / 未验证项

1. **相机状态归属**：World 组件（现状，`camera.rs:141-152`）还是 Store（MCP catalog 假设）？两者只能选一个真值；若保留 World，则 MCP `get_scene_camera` 必须从 World snapshot 提取，camera commands 走“BeforeEngineUpdate”阶段。
2. **帧循环形态**：保留连续 `request_redraw`（对 MCP 帧语义最友好，但 CPU 永远跑满）还是事件驱动（省电，但 MCP 每帧语义要靠 repaint 保证）。与 §1.4 的 `render_ui/handle/draw` 重构一起决定。
3. **Engine 更新位置**：是否把 `Engine::update()` 移到 UI 之后（消除 1 帧延迟，但会改变 extract/present 的时序与 UI 读 World 的“本帧数据”语义）？这是本清单里风险最高的顺序变更，需要单独 ADR/测试。
4. **UI crate 形态**：`kairos_editor` 内部分层（core/ui/host）还是新建 `kairos_editor_ui`/`kairos_editor_core` crate？受 §8 MUST-7 约束；MCP 计划预期新增 `kairos_editor_mcp` crate（`phase4-work-schedule.md:71-87`）。
5. **`Drawer` 去留**：保留为“渲染适配器”（推荐，兼容 MCP 的 `as_query_handler` shim）还是改成声明式面板树。
6. **Log 方案**：自研 ring buffer + 显式 logger，还是 `tracing/log` capture layer + `tracing-subscriber`？后者与 env_logger 会有重复初始化问题，需要选一个。
7. **MCP 计划是否需要同步更新**：`phase3-middleware-architecture.md:176-208` 的 `handle_asset_server` 与 `Drawer::as_query_handler` 分发在新架构下需要修订（§4.1.6）；建议在 Lead 设计文档里标注“计划过期点”。
8. **MCP 通道实现**：tokio mpsc + oneshot（计划原型）vs crossbeam + oneshot（更贴现有 editor 依赖）；二者都满足“非阻塞 try_recv”，但 tokio mpsc 的 `blocking_recv` 在渲染线程绝对不能用。
9. **play mode / scene 保存**：当前无 play/stop、`New Scene` 是 `todo!()`、`on_exit` 无保存（`tool_bar.rs:206-208`、`lib.rs:73-76`）；MCP P1/P2 的 `run_game/stop_game/save_scene` 依赖这些功能先存在。
10. **需编译确认（本清单未做）**：`egui_inspection 0.35` 与当前 egui 0.35/winit 0.30/wgpu 29 的兼容性；`run_ui` 多 pass 在现有 widget 组合（egui_extras/egui_commonmark/syntect）下是否会被触发；新 crate 拆分对 `cargo test-crate` 的影响。

---

## 8. 集成要求（must / should / must-not）

> 这里只写“引擎/现有计划强加”的要求；不决定 UI 内部形态（那属于 Lead 的设计文档）。

### MUST（不做就会与引擎/既有计划冲突）

1. **单一状态入口**：提供唯一的 `EditorStore/EditorState`（或等价）作为编辑器自有状态的唯一真值；MCP `EditorQuery/EditorCommand` 只对 Store（+ 引擎快照）派发，不再遍历 `Drawer` 找 handler。依据：`phase3-middleware-architecture.md:11-16`（状态分散是问题 #1）、`:85-93`（first-handler-wins）、本次重写目标本身。
2. **帧阶段显式化**：定义并实现有序阶段（至少：**ApplyCommands/Intents → EngineUpdate → UiBuild → Snapshot/Queries → Present**），每个阶段有名字、有类型、有测试。命令若要影响本帧 World/extract，必须在 `Engine::update()` 之前应用（现状 `runtime.rs:242` 在 UI 前；MCP 计划的伪代码在 draw 前，两者不一致，见 §4.1.6）。
3. **写入者唯一 + 主线程**：World 只在 main thread、只在帧阶段内被修改；后台线程只回传纯数据（`Send`），插入点仍是 `PreUpdate`/`Startup` 的 schedule（`kairos_asset/src/install.rs:15-27`）。禁止在 `Drawer::ui` 阶段直接改 World（现状靠 `&Engine` 保证，新架构要用类型系统保证）。
4. **查询=同步快照**：所有 MCP query/编辑器查询在同一帧内同步生成 owned snapshot（serde 友好），不带 `&World`/引用/锁；禁止 `Arc<RwLock<World>>`（用户决策，`phase1-requirements.md:249-257`、`phase3-middleware-architecture.md:215-244`）。
5. **命令=有结果、可失败、不 panic**：每个 command 返回 `Result`；UI 创建失败/菜单占位（`tool_bar.rs:206-208` `todo!()`）、`create_ui_failed` 的 `panic!`（`ui.rs:865-869`）不允许在 UI/命令路径存在；错误进日志 + UI 提示。
6. **保留引擎 bootstrap 契约**：`Engine::new()`（含 rails/asset/font/physics/graphics）→ `kairos_editor::install(&mut world)`（Text/Toml/Syntax/TextureEdit/SceneViewInput/camera）→ `KairosGame::new(&mut engine)` 的安装顺序不可变（`lib.rs:26-45,107-113`；`engine.rs:49-105`；抽取清单 §9 #16）。新 UI 架构不得把 editor 资源塞回 engine bootstrap，也不得颠倒顺序。
7. **crate 依赖方向**：engine 不得依赖 editor/新 UI crate；UI 新 crate 单向依赖 engine（`docs/agents/testing.md:50`；抽取清单 §9 #5）。跨边界传 snapshot/DTO，不传闭包/引用。
8. **渲染命令仍是唯一 UI→GPU 路径**：UI 只产出 `GraphicsCommand`/`EguiTextureHandle` id；不直接持有 wgpu 资源；present 顺序、attachment 归属、纹理回收（oneshot/mpsc）语义不变（§2.7、§5.1）。
9. **对齐 egui multi-pass 安全**：帧级副作用（命令收集、消息 drain、watcher drain、Log 消费、状态写入）必须只在 last pass 执行，或显式 `max_passes=1`。否则重复 draw/present 是现实风险（§1.3-3）。
10. **日志可观测**：提供结构化、环形、可过滤（level/time/pattern/after/limit/caller）、可 clear、不会因窗口关闭而丢失的日志资源；Console 只读快照，不再 `pop_front`（`console_window.rs:78-82`；MCP `get_console_logs/clear_console/wait_for_console`，`phase2-tools-catalog.md:899-990`）。同时要把 `log` crate 的宏输出接入（或明确只收显式 logger）。
11. **保留 `SceneView`/`GameView` 的 size/camera 契约**：窗口开/关→size 归零、tab 首次打开→绑定 editor camera、extract 读 `view.camera` 派生 projection（`ui.rs:621-663`；`scene_window.rs:494-520`；`extract.rs:85-106`）。新架构可以把“开启/关闭视图”改成 command，但必须等价地写这两个资源，否则 play layer/render rail 什么都不画。
12. **相机输入的时序契约**：orbit/zoom/fly 若仍走 `SceneViewInput`（`camera.rs:154-180`），必须在 `Engine::update()` 的 `PostUpdate` 消费前写入；若改为直接写 `EditorCameraController`，必须保持“无输入帧不 stamp transform”（`camera.rs:239-241` 只在变化时写，避免 change tick 噪声）。
13. **headless/test seam**：提供不打开音频、不建窗口、不依赖 cwd 的构造路径（Store 构造 + `World` bootstrap + 可注入 config），使 `cargo test-crate` 能覆盖状态与查询逻辑（§6）。
14. **MCP/host 生命周期**：定义 command/query 通道的启动、关闭、崩溃语义（channel drop → oneshot error；`phase3-event-loop-integration.md:124-134`）；退出前有显式 `on_exit`/`shutdown`（现状 TODO，`lib.rs:73-76`）。MCP 不自动重连，编辑器重启由 Agent 负责。
15. **保留单 viewport 假设直到明确扩展**：当前 runtime 只处理 `ViewportId::ROOT`（`runtime.rs:274-300`），MCP resize/screenshot 也只按单 viewport 设计（`egui-mcp-analysis.md:245-249`）。若新架构要支持多窗口，必须同时扩展 runtime 的 `process_viewport_commands` 与 MCP 的 target 模型。

### SHOULD（会显著降低后续成本）

1. **Store 分层**：`EditorStore`（业务状态：selection/tabs/layout/preferences/日志/命令队列）+ `EngineSnapshot`（每帧从 World 提取的只读视图：`SceneView.size`、camera orbit、asset 状态）+ `UiTransient`（egui 内存里的 hover/DnD，若无需持久化就不进 Store）。
2. **命令/意图的显式阶段字段**：`enum When { UiOnly, BeforeEngineUpdate, NextFrame, EndOfFrame }`，消除“为什么相机慢一帧”的隐式知识。
3. **渲染意图 data-driven**：`render_ui` 从 Store 读“哪些视图可见/尺寸/相机”后生成 `GraphicsCommand`，不再让每个窗口在 `render()` 里自己读 World 拼命令（现状 `scene_window.rs:358-480`、`game_window.rs:174-266` 各自持有这套逻辑）。
4. **布局/窗口状态可持久化**：`DockState`/`EditorLayout`/`WindowState` 目前只在内存（`ui.rs:330-332`、`window_state.rs`）；MCP `get_dock_layout`/P2 preferences 需要稳定 ID + 可序列化 schema（用 drawer 名称/角色，不用 `usize`/`TypeId`）。
5. **样式配置统一加载**：把 `Preferences/Styles/*.toml` 的 load/save 收敛到 config service（现状每个 `Style::new()` 自己读，例 `scene_window.rs:67-98`、`global_styles.rs:14-27`），并允许测试注入。
6. **输入焦点仲裁**：明确 egui 焦点 vs 引擎 `InputEngine` 的规则（当前键盘双投递，`runtime.rs:496-526`），把“场景捕获 WASD”建模成 UI focus scope。
7. **把 `KairosGame`/play mode 显式化**：现状 demo scene 在启动时装配、无 play/stop（`lib.rs:32-37`、`kairos_game.rs:207-229`），MCP `get_game_state.is_running` 无数据源；新架构应先定义 PlayMode（Stopped/Playing/Paused）再谈工具。
8. **错误与诊断面板**：`create_ui_failed` 现在是 native dialog + panic（`ui.rs:865-869`）；重写后应有可观测的错误通道（日志/通知），供 MCP assertions 使用。
9. **资源/句柄的所有权注释**：对 `EguiTextureHandle`、`AssetHandle`、`oneshot receiver` 写清 owner/生命周期与“何时可以丢”。
10. **性能备注**：每帧重建 attachment 纹理（`render_pipeline.rs:386-405`）与新 UI 的重绘频率耦合；如果新架构引入更多面板/动画，应把“只在脏时重绘/缓存 texture”列入设计（当前连续 `request_redraw` 会一直跑满）。

### MUST NOT（明确禁止）

1. **禁止 `Arc<RwLock<World>>`/跨线程共享 World**（用户决策 + 引擎形态不支持；`phase1-requirements.md:249-257`）。
2. **禁止把编辑器状态继续留在 `Drawer`/`ui::Context` 的私有字段里**，或新增 `Cell/Mutex` 内部可变性来绕过 `&self`；也禁止把 `egui ctx.data_mut` 当业务状态存储（现状 DnD 是瞬态，可以；selection/tabs 不行）。
3. **禁止在 draw（egui pass）阶段直接改 World**；所有 World mutation 必须在显式阶段、单一写入者、主线程。
4. **禁止让 UI 代码 `panic!/todo!/unwrap()` 处理可恢复错误**（反例 `ui.rs:865-869`、`tool_bar.rs:206-208`）；UI 触发的 panic 会直接杀编辑器，MCP 的安全模型会失效。
5. **禁止在渲染循环里阻塞等待**（channel `recv`、`block_on`、`join`、sleep）；MCP 请求/资产加载必须是轮询/异步/超时。
6. **禁止 UI/渲染代码直接创建/销毁 wgpu 资源或跨线程使用 `TextureId`/`EguiTextureHandle`**。
7. **禁止把 editor 专属资源/系统反向装进 `kairos_engine` 的 bootstrap**，或让 `kairos_engine` 依赖 `kairos_editor`/UI crate（抽取时已明确迁出，`lib.rs:79-106` 注释 + 抽取清单 §9 #5）。
8. **禁止用 `NodeIndex`/`usize` 这类不稳定索引作为命令/查询/持久化 ID**（项目树重扫会重建索引，`project_window.rs:156-158,193-199`；应使用 GUID/AssetPath/稳定字符串）。
9. **禁止让 Console/日志消费方 `pop` 掉状态**（`console_window.rs:78-82`）；日志是只读历史，不是 UI 的输出队列。
10. **禁止在 present/渲染线程之外偷跑帧逻辑**（例如后台线程直接调 `Engine::update` 或 `World` 系统）；`MainThreadExecutor`（`schedule.rs:358-360`）已经明示 system 可能要求主线程。
11. **禁止把 `tokio` 引入 `kairos_asset` 的加载路径**（ADR 0001）；编辑器的 `#[tokio::main]` 是 host 层现状，可以被 MCP 复用，但不要扩散成引擎/资产依赖（抽取清单 §9 #10 已把 tokio 列为编辑器直接依赖的观察项）。
12. **禁止扩大“引擎依赖 UI 库”的耦合**：`kairos_engine::math` 目前有 egui Color32 转换（`math/color/converts.rs`，抽取清单 §9 #11）；新架构不要把更多 egui 类型（Response/Ui/TextureId）漏进引擎 API。

---

## 附录 A：关键证据索引（file:line）

| 主题 | 位置 |
|---|---|
| 帧循环主流程 | `kairos_editor/src/runtime.rs:237-394`（引擎 update :242；run_ui 闭包 :264-270；present :350-351；无条件 repaint :384） |
| winit 事件与重绘驱动 | `runtime.rs:100-109,423-440,452-537` |
| Editor 各阶段入口 | `kairos_editor/src/lib.rs:51-76`（update/handle_ui/draw_ui/render_ui/on_exit） |
| Drawer trait / Messager / Context | `kairos_editor/src/ui.rs:255-336` |
| Message enum（57 variant）与 handle（~365 行） | `ui.rs:90-206,410-774` |
| 初始 UI 由 7 条 Message 启动 | `ui.rs:338-366` |
| Drawer 全量 render | `ui.rs:776-786` |
| `&Engine` / `&mut Engine` 面 | `ui.rs:262-271,368-374,410-414,776-779,831-853` |
| 引擎 update / rails | `kairos_engine/src/engine.rs:43-46`、`schedule.rs:170-185,204-223,246-287,302-361` |
| 引擎 bootstrap 顺序 | `engine.rs:49-105`、`lib.rs:26-45,107-113` |
| SceneView/GameView/ViewportSize | `kairos_graphics/src/view_port.rs:11-16,68-91` |
| extract 系统 | `kairos_graphics/src/extract.rs:36-106` |
| 图形资产事件收集 | `kairos_graphics/src/asset_events.rs:63-116,129-152` |
| present / 纹理回收 / oneshot | `kairos_graphics/src/render_pipeline.rs:345-581,526-544,357-359` |
| 资产安装/事件阶段 | `kairos_asset/src/install.rs:15-27,56-67`、`event.rs:23-37` |
| 消息帧语义 | `kairos_ecs/src/message/update.rs:23-63`、`schedule.rs:316-339` |
| 项目树 watcher | `kairos_editor/src/project_watcher.rs:22,54,89-97`、`ui.rs:414-416`、`project_window.rs:159-200` |
| Log | `kairos_engine/src/log.rs:25-27,52-84`、`console_window.rs:78-82`、`about_window.rs:95-96` |
| ProjectWindow 状态 | `project_window.rs:49-71,104-124,150-239` |
| InspectorWindow 状态 | `inspector_window.rs:24-55,99-136,173-179` |
| SceneWindow 状态/渲染/相机 | `scene_window.rs:54-65,126-201,358-480,494-543` |
| GameWindow 状态/渲染 | `game_window.rs:29-37,174-266` |
| Preferences/ToolBar 状态 | `preferences_window.rs:47-63`、`tool_bar.rs:34-36,76-82,204-209` |
| Dock/layout 状态 | `ui.rs:326-336`、`layout.rs:199-235`、`docking_tab/dock_state.rs:24-30`、`docking_tab/state.rs:17-33`、`docking_tab/window_state.rs:4-26` |
| 磁盘状态 | `asset_registry.rs:199-284`、`ui/paths.rs:1-30` |
| 编辑器 camera controller | `camera.rs:141-243` |
| 测试基线 | `docs/agents/testing.md:5-15,61-102`；`camera/test.rs:1-15,68-88`；`engine/test.rs:31-33` |
| MCP 中间件 | `docs/plan/kairos-editor-mcp/phase3-middleware-architecture.md:11-16,96-208,278-343` |
| MCP 帧边界/超时 | `docs/plan/kairos-editor-mcp/phase3-event-loop-integration.md:19-134,256-294` |
| MCP 可观测性 | `docs/plan/kairos-editor-mcp/phase2-observability.md:17-128` |
| MCP 工具/资源 | `docs/plan/kairos-editor-mcp/phase2-tools-catalog.md:899-990,991-1075,1127-1170,1272-1287` |
| MCP 需求/决策 | `docs/plan/kairos-editor-mcp/phase1-requirements.md:154-159,190-257`、`handoff.md:42-54` |
| egui-mcp | `docs/research/egui-mcp-analysis.md:155-185,222-230,245-249,267-390,522-535` |
| 抽取清单（借用/依赖/潜伏问题） | `docs/research/editor-crate-extraction-inventory.md:993-1055,1107-1190` |
| claw（仅模式参考） | `docs/plan/kairos-editor-claw/README.md:21-33`、`claw-phase1-requirements.md:150-218`、`handoff-claw.md:40-51` |
| egui multi-pass | egui-0.35.0（cargo registry）`src/context.rs:780-860`、`src/memory/mod.rs:331` |

## 附录 B：编辑器侧 `World` 访问点汇总（grep 事实）

- `world.resource::<T>()`（T 命中）：`SceneView`、`GameView`、`AssetServer`、`Assets<Text/Toml/Material/Mesh/Texture/SerializedMaterial/AudioExt/PcmData/AudioAsset/Font/SyntaxHighlightSettings/TextureEdit>`、`CameraView`（经 entity）。
- `world.resource_mut::<T>()`：`SceneView`、`GameView`、`Assets<Text/Toml/Material/SerializedMaterial/TextureEdit>`、`AudioEngine`。
- `world.get_resource_or_init::<T>()`：`SceneViewInput`（`ui.rs:675,679,683`）、`Schedules`（仅 install 期）。
- `world.spawn(...)` / `entity_mut`：editor camera（`scene_window.rs:508-518`）、preview/wireframe mesh（`mesh.rs:150`）、gizmo mesh（`grid_plane.rs:56`、`axes_indicator.rs:138`）。
- 直接调用 `world.run_schedule`：生产路径**没有**（全部经 `Engine::update()`，`engine.rs:43-46`）；测试里手工跑 stage（`camera/test.rs:90+`、`editor_assets/text.rs:126-142`、`ui/inspector/texture/mod.rs:818+`）。
- 完整清单命令（可复跑）：
  ```sh
  grep -rn "resource\(_mut\)\?::<\|get_resource_or_init::<" kairos_editor/src --include=*.rs
  grep -rn "engine\.world\|engine\.time()\|engine\.input_engine" kairos_editor/src --include=*.rs
  ```
