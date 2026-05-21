# wgpu 多线程渲染、Unity 线程模型与 Render Graph 笔记

> 来源：AI 辅助整理（2026-05-21）  
> 状态：架构学习与设计参考，非最终实现规范  
> 项目上下文：KairosEngine 使用 **wgpu 27** + **winit 0.30** + **egui-wgpu**，当前渲染入口主要位于 `src/kairos_editor/runtime.rs` 与 `src/graphics/render_pipeline.rs`

## 1. 本文讨论范围

这份文档整理了围绕以下问题的完整讨论结论：

- wgpu 的多线程渲染最佳实践是什么？
- wgpu 内部是否会自己使用多线程？
- Unity Profiler 里的 Main Thread / Render Thread 分别对应自研 Rust/wgpu 项目的哪些部分？
- 在当前项目中，Main Thread 和 Render Thread 应该如何交互？
- 是否应该把 Shadows、Scene、PostProcess、UI 分别放到独立线程？
- “高层渲染命令 → 底层 wgpu 命令”这种中间层是否必要？
- Render Graph 是什么，为什么它经常和现代渲染架构一起出现？

本文的核心结论是：

```text
当前阶段：
  Main Thread 直接驱动 wgpu 渲染即可。

中期阶段：
  抽出 FrameSnapshot / RenderPacket / DrawList，
  让逻辑数据和渲染数据分离。

复杂阶段：
  用 Worker Pool 并行做 culling、sorting、batching、buffer packing。

成熟阶段：
  引入轻量 Render Graph 管理 pass 和资源依赖；
  如 Main Thread 被 present/submit/资源上传拖住，再考虑独立 Render Thread。
```

## 2. 当前项目中的实际对应

当前代码大致是：

```text
main.rs
  event_loop.run_app(&mut runtime)

KairosEditorRuntime
  接收 winit 事件
  处理窗口生命周期
  处理 egui 输入与 UI 更新
  调用 engine.update(ctx)
  调用 render_pipeline.render()

RenderPipeline::render
  surface.get_current_texture()
  create_view()
  create_command_encoder()
  begin_render_pass()
  queue.submit()
  present()
```

也就是说，目前项目里还没有真正拆出 Unity 意义上的独立 Render Thread。

当前的线程模型更接近：

```text
OS / winit Main Thread
  ├─ Window events
  ├─ egui input
  ├─ engine.update
  └─ wgpu command recording + submit + present
```

这对于当前阶段是合理的。因为 `RenderPipeline::render()` 目前只做清屏，过早拆线程会增加同步复杂度，但收益很小。

## 3. wgpu 多线程渲染最佳实践

wgpu 的 `Device`、`Queue` 等对象是面向多线程使用设计的，可以在多个线程之间共享。实践上，不建议理解成“多个线程一起操作同一个 RenderPass”，而应该理解成：

```text
多线程准备 CPU 渲染数据
  +
可选的多线程 command recording
  +
集中 submit
```

推荐结构：

```text
Main Thread:
  输入、窗口事件、编辑器 UI、游戏/场景逻辑，产出本帧快照

Worker Pool:
  并行 culling、sorting、batching、instance packing、部分命令准备

Render Thread 或 Render Stage:
  组织 pass
  创建/更新 GPU 资源
  录制 wgpu command buffer
  queue.submit
  present
```

一些关键原则：

- 不要多个线程同时修改同一个 `CommandEncoder` 或同一个 `RenderPass`。
- Worker 可以各自创建独立的 encoder，最后汇总 `CommandBuffer`。
- `queue.submit` 次数不宜过多，通常每帧集中提交一次或少数几次。
- 不要把 `Device` 包在 `Arc<Mutex<Device>>` 里当全局大锁使用；更好的方式是共享 cloneable handle，并让资源所有权清晰。
- 多线程收益主要来自 CPU 侧准备工作，例如 culling、排序、生成 draw list，而不是把每一个 pass 都硬拆到固定线程。

## 4. wgpu 内部会自己多线程吗？

结论：**wgpu 不是 Unity 那种自动帮你维护一个 Render Thread 的引擎层框架**。

wgpu 内部确实有线程安全结构、锁、回调驱动机制，以及不同平台后端可能使用的辅助线程。但它通常不会自动把你的渲染流程拆成多个 worker 来并行执行。

需要区分三件事：

```text
1. CPU 应用线程
   你的 Rust 代码运行的线程。

2. GPU 异步执行
   queue.submit 之后，GPU 与 CPU 并行执行。

3. 驱动/平台内部线程
   Metal/Vulkan/DX12 驱动或窗口系统可能有自己的线程。
```

对于应用层架构来说，重要的是：

```text
wgpu 支持你多线程使用它，
但不会替你设计引擎的多线程渲染架构。
```

另外，`map_async`、`on_submitted_work_done` 之类回调并不是“自动在神秘后台线程里稳定运行”的模型。它们通常由 `device.poll`、`instance.poll_all` 或 `queue.submit` 等行为驱动，具体执行线程与轮询时机相关。

## 5. Unity Main Thread / Render Thread 对照

Unity Profiler 中常见的 Main Thread / Render Thread 可以这样理解：

```text
Unity Main Thread:
  游戏逻辑
  MonoBehaviour Update/LateUpdate
  Transform/Scene 管理
  Animator/Physics 集成点
  Culling 调度
  SRP Render 调用入口
  UI 逻辑

Unity Render Thread:
  接收主线程生成的渲染指令
  转换为图形 API 命令
  管理部分 GPU 资源提交
  与驱动和 swapchain/present 交互
```

在 KairosEngine 当前代码里，对应关系大致是：

| Unity 概念 | 当前 KairosEngine 对应 |
|-----------|------------------------|
| Main Thread | winit event loop 所在线程 |
| 输入事件 | `KairosEditorRuntime::window_event` |
| UI 更新 | `egui_ctx.run(... engine.update(ctx))` |
| 游戏/编辑器逻辑 | `KairosEngine::update` |
| Render Thread 工作 | 当前仍在同一线程里的 `RenderPipeline::render()` |
| Present / Swapchain | `surface.get_current_texture()` + `output.present()` |
| CommandBuffer 提交 | `queue.submit(...)` |

所以目前实际结构是：

```text
Unity Main Thread + Unity Render Thread 的大部分 CPU 工作
都还集中在 KairosEngine 的 winit 主线程中。
```

这不是问题。很多小型 wgpu 程序、编辑器原型、工具型程序一开始都这样做。

## 6. Unity Profiler 中一些等待点的类比

Unity 里常见的等待 marker 可以类比为：

| Unity marker | 含义 | wgpu 项目里的近似位置 |
|-------------|------|----------------------|
| `Gfx.WaitForCommands` | Render Thread 等 Main Thread 给命令 | Render Thread 等待 `FramePacket` |
| `Gfx.WaitForRenderThread` | Main Thread 等 Render Thread 处理完某些工作 | Main Thread 等待渲染线程释放资源或帧槽 |
| `Gfx.WaitForPresentOnGfxThread` | Present / swapchain / GPU backpressure | `get_current_texture`、`present`、`queue.submit` 附近 |

如果未来拆出 Render Thread，最需要警惕的是：

```text
Main Thread 等 Render Thread
Render Thread 等 Main Thread
CPU 等 GPU
```

多线程架构不是一定更快。它只是把可并行的工作拆开，同时引入了新的同步风险。

## 7. Main Thread 与 Render Thread 应该如何交互

如果未来要拆线程，不建议让 Render Thread 直接读取 ECS、Scene、Transform、Editor 状态等主世界数据。

推荐方式是 Main Thread 在某个帧边界产出不可变快照：

```rust
struct FramePacket {
    frame_id: u64,
    camera: CameraPacket,
    lights: Vec<LightPacket>,
    shadow_culls: Vec<ShadowCullResult>,
    draw_items: Vec<DrawItem>,
}

struct ShadowCullResult {
    light_id: LightId,
    cascade_index: u32,
    view_proj: Mat4,
    visible_items: Vec<DrawItemId>,
}

struct DrawItem {
    mesh: MeshHandle,
    material: MaterialHandle,
    transform: Mat4,
    sort_key: u64,
}
```

然后用 channel 发送给渲染侧：

```rust
enum RenderMessage {
    Frame(FramePacket),
    Resize { width: u32, height: u32 },
    Shutdown,
}
```

逻辑上类似：

```text
Main Thread:
  world update
  collect renderables
  run / schedule culling
  build FramePacket
  send RenderMessage::Frame(packet)

Render Thread:
  receive FramePacket
  update GPU buffers
  build render graph
  record commands
  submit
  present
```

这比共享一堆 `Arc<Mutex<Scene>>` 更稳，因为 Render Thread 拿到的是本帧渲染所需的不可变数据，而不是随时可能被主线程修改的世界状态。

## 8. Shadows Cull 完成后如何通知 Render Thread

用户提出的问题是：

```text
Main Thread 这边做完 Shadows Cull，
该怎么通知 Render Thread 组织 Shadows 渲染命令？
```

推荐不要理解成“做完一个 shadow cull 就立刻通知 Render Thread 开始画 shadow”。

更稳的做法是：

```text
一帧内：
  多个 worker 并行完成 Shadow Cull / Scene Cull / UI Mesh / Sorting
  汇总成完整 FramePacket
  一次性交给 Render Thread
```

因为 Render Thread 通常需要完整的帧上下文：

- shadow map 尺寸和 atlas 分配
- light/cascade 列表
- scene pass 是否读取 shadow depth
- 本帧 render target
- postprocess 是否启用
- UI 是否覆盖到 swapchain

也就是说：

```text
Shadows Cull 结果是 FramePacket 的一部分，
而不是一个立刻驱动 GPU 的独立命令。
```

除非未来做非常复杂的流水线并行，例如 frame N 的 shadow pass 和 frame N+1 的 culling 重叠，否则先按整帧 packet 传递会更简单。

## 9. 是否应该给 Shadows、Scene、PostProcess、UI 各开一个固定线程

这个方案直觉上很自然：

```text
Shadow Thread
Scene Thread
PostProcess Thread
UI Thread
```

但通常不是最优选择。

主要问题：

- 负载不均衡：Shadow 可能很重，UI 很轻，固定线程容易有线程空等。
- 依赖复杂：Scene 可能依赖 Shadow，PostProcess 依赖 Scene，UI 依赖最终 color。
- GPU submit 不适合碎片化：多个线程各自提交容易造成同步和顺序问题。
- 资源生命周期复杂：谁创建 texture，谁释放，谁保证还没被 GPU 使用？
- UI 通常和主线程输入、平台窗口事件绑定较深，不适合粗暴扔到独立线程。

更现代的做法是：

```text
Render Graph + Worker Pool + 集中 Render Thread/Render Stage
```

也就是：

```text
Pass 是任务，不是固定线程。
线程是执行资源，不是架构边界。
```

一个 Worker Pool 可以同时跑：

- shadow cascade culling
- main camera culling
- transparent sorting
- instance buffer packing
- light list building
- UI mesh preparation

任务完成后汇总到渲染侧，由 Render Graph 决定 pass 顺序。

## 10. 对用户理解的确认：高层渲染命令再转底层命令

用户的理解是：

```text
将原本 Main Thread 中的 Cull、Data Collect 等任务并行执行，
得到高层渲染命令，
然后由 Render Thread 接收并转换为底层渲染命令。
```

这个理解基本正确，但需要修正“高层渲染命令”的含义。

高层数据最好不是这种：

```text
SetPipeline
SetBindGroup
DrawIndexed
```

因为这已经太像底层 graphics API 了。这样做会让 Render Thread 真的变成“命令翻译器”，收益不大。

更好的高层数据应该是引擎语义：

```text
visible opaque draw items
visible transparent draw items
shadow caster lists
light packets
material handles
mesh handles
camera matrices
ui meshes
debug draw data
```

也就是：

```text
FramePacket / DrawList:
  描述“画什么”

RenderGraph:
  描述“按哪些阶段画，资源如何流动”

wgpu CommandEncoder:
  描述“具体怎么调用 GPU API”
```

## 11. Render Thread 只是命令翻译，是否多余

如果 Render Thread 只是把：

```text
HighLevelCommand::SetPipeline(x)
HighLevelCommand::DrawIndexed(...)
```

翻译成：

```rust
render_pass.set_pipeline(...);
render_pass.draw_indexed(...);
```

那确实很可能是“脱裤子放屁”。

在小型项目或当前 KairosEngine 阶段，Main Thread 直接录制 wgpu 命令通常更好：

```text
简单
容易调试
同步少
资源生命周期直观
性能也足够
```

Render Thread 真正有价值的场景是：

- Main Thread 经常被 `present`、swapchain、GPU backpressure 卡住。
- CPU 侧渲染准备明显很重，需要和下一帧逻辑并行。
- 资源上传、纹理销毁、pipeline cache、bind group cache 等逻辑变复杂。
- 需要清晰隔离 world state 和 renderer state。
- 想让 Main Thread 专注编辑器/逻辑，Render Thread 处理 GPU 相关工作。

因此推荐演进路线是：

```text
现在：
  Main Thread 直接调用 RenderPipeline::render()

下一步：
  抽出 FrameSnapshot / RenderPacket / DrawList，
  但仍然在主线程录制 wgpu 命令。

再下一步：
  Worker Pool 并行生成 packet 的不同部分。

最后：
  当 profiler 证明主线程被渲染提交/present/资源管理拖住时，
  再把 RenderPipeline 移到 Render Thread。
```

## 12. Render Graph 是什么

Render Graph 可以理解成：

```text
把一帧渲染拆成一组有依赖关系的渲染任务，
由系统根据资源读写关系决定执行顺序、资源生命周期和命令组织方式。
```

它不是 wgpu 自带的对象，而是引擎层架构。

手写流程可能是：

```rust
render_shadows();
render_scene();
render_post_process();
render_ui();
present();
```

Render Graph 的描述方式更像：

```text
Shadow Pass:
  write shadow_depth

Scene Pass:
  read shadow_depth
  write scene_color
  write scene_depth

PostProcess Pass:
  read scene_color
  write post_color

UI Pass:
  read post_color
  write swapchain_color

Present:
  read swapchain_color
```

由资源依赖可以自动推导：

```text
Shadow -> Scene -> PostProcess -> UI -> Present
```

## 13. Render Graph 的作用

### 13.1 管理 Pass 顺序

当渲染流程简单时，手写顺序很好。

但加入这些功能后：

```text
shadow cascades
depth prepass
gbuffer
deferred lighting
ssao
bloom
taa
motion vectors
transparent
outline
editor gizmo
picking pass
ui
debug overlays
```

手写顺序会越来越难维护。Render Graph 可以根据资源读写关系推导执行顺序。

### 13.2 管理临时资源

例如 bloom 可能需要：

```text
scene_color
bloom_downsample_0
bloom_downsample_1
bloom_downsample_2
bloom_upsample_1
bloom_upsample_0
final_color
```

Render Graph 可以知道某些 texture 只在本帧短时间使用，从而支持：

- 自动创建临时 texture
- 生命周期分析
- 未来做 texture pool / aliasing
- resize 时统一重建资源

wgpu 会隐藏很多底层 barrier 和 layout 细节，但不会替引擎设计资源生命周期。

### 13.3 剔除无用 Pass

如果某个 debug pass 的输出没有被最终画面读取：

```text
NormalDebug Pass -> debug_color
```

而当前 UI 没打开这个 debug view，那么 graph compile 阶段可以发现这个 pass 没有贡献，直接跳过。

这叫 pass culling。

### 13.4 让渲染功能可扩展

没有 Render Graph 时，代码容易变成：

```rust
if enable_shadow {
    render_shadow();
}

if enable_ssao {
    render_ssao();
}

if enable_bloom {
    render_bloom();
}

if enable_ui {
    render_ui();
}
```

久而久之会出现大量隐式约束：

```text
SSAO 必须在 depth prepass 后
Transparent 必须在 opaque 后
TAA 需要上一帧 history
Bloom 需要 HDR color
UI 不应该参与 TAA
Editor outline 需要 object id buffer
```

Render Graph 的价值是把这些约束显式化：

```text
Pass 声明自己读什么、写什么；
Graph 负责组织它们。
```

### 13.5 为多线程准备提供边界

Render Graph 很适合作为 Worker Pool 与 Render Thread 之间的组织层。

一个成熟结构可以是：

```text
Main Thread:
  更新游戏逻辑 / 编辑器 / ECS
  生成 FrameSnapshot

Worker Pool:
  Shadows Cull
  Scene Cull
  UI Tessellation
  Draw Sorting
  Instance Buffer Packing

Render Graph Build:
  根据本帧需要注册 pass 和资源

Render Thread:
  编译 Render Graph
  执行各个 pass
  录制 wgpu commands
  queue.submit
  present
```

## 14. Render Graph 的核心概念

### 14.1 Pass

Pass 是一个渲染、计算或拷贝阶段。

例如：

```text
ShadowPass
DepthPrepass
OpaquePass
TransparentPass
BloomDownsamplePass
BloomUpsamplePass
UIPass
```

概念上：

```rust
graph.add_pass("shadow", |pass| {
    pass.write_texture(shadow_depth);
    pass.execute(|ctx| {
        // record wgpu shadow pass commands
    });
});
```

### 14.2 Resource

Resource 是 pass 之间传递的数据：

```text
Texture
Buffer
Swapchain Image
Depth Buffer
Shadow Map
GBuffer
History Buffer
```

常见分类：

```text
Imported Resource:
  外部传入的长期资源，例如 swapchain texture、上一帧 history、长期 shadow atlas。

Transient Resource:
  本帧临时资源，例如 bloom 中间 texture、临时 depth、copy buffer。
```

### 14.3 Dependency

如果一个 pass 写资源，另一个 pass 读资源，就形成依赖：

```text
ShadowPass writes shadow_depth
ScenePass reads shadow_depth

=> ShadowPass must run before ScenePass
```

### 14.4 Compile

Render Graph 通常先 build，再 compile，最后 execute。

compile 阶段可以做：

- 推导执行顺序
- 剔除无用 pass
- 计算资源生命周期
- 分配临时 texture/buffer
- 检测非法读写
- 未来合并或重排部分 pass

### 14.5 Execute

执行阶段才真正录制 wgpu 命令：

```text
1. ShadowPass
2. ScenePass
3. PostProcessPass
4. UIPass
5. Present
```

Pass 内部才调用：

```rust
encoder.begin_render_pass(...);
render_pass.set_pipeline(...);
render_pass.set_bind_group(...);
render_pass.draw_indexed(...);
```

## 15. Render Graph 与高层渲染命令的关系

它们相关，但不是同一个东西。

`FramePacket` / `DrawList` 更像：

```text
这一帧有哪些可见物体
每个物体用哪个 mesh/material
每个 light 的 shadow cull 结果是什么
UI 生成了哪些 mesh
相机矩阵是什么
```

`RenderGraph` 更像：

```text
这些数据应该经过哪些 pass
pass 之间用哪些 texture/buffer 连接
最终如何产出 swapchain image
```

例子：

```text
FramePacket:
  visible_opaque_draws
  visible_transparent_draws
  shadow_cascades
  ui_meshes

RenderGraph:
  ShadowPass reads shadow_cascades, writes shadow_depth
  ScenePass reads visible_opaque_draws + shadow_depth, writes scene_color
  TransparentPass reads visible_transparent_draws + scene_color, writes scene_color
  UIPass reads ui_meshes + scene_color, writes swapchain
```

推荐分层：

```text
World / Engine 数据
        ↓
FrameSnapshot / RenderPacket
        ↓
RenderGraph
        ↓
wgpu CommandEncoder
        ↓
Queue Submit / Present
```

## 16. 适合 KairosEngine 的最小 Render Graph

当前项目不需要一上来实现完整工业级 Render Graph。

最小版本只需要做到：

- 能注册 pass
- 能声明 pass 的读写资源
- 能按依赖排序
- 能按顺序执行 pass callback

概念结构：

```rust
struct RenderGraph {
    passes: Vec<RenderPassNode>,
}

struct RenderPassNode {
    name: &'static str,
    reads: Vec<ResourceId>,
    writes: Vec<ResourceId>,
    execute: Box<dyn FnOnce(&mut RenderContext)>,
}
```

每帧：

```rust
let mut graph = RenderGraph::new();

graph.add_pass("shadow", ...);
graph.add_pass("scene", ...);
graph.add_pass("post_process", ...);
graph.add_pass("ui", ...);

let compiled = graph.compile();
compiled.execute(&mut render_context);
```

一开始甚至可以不做资源 aliasing、不做复杂 lifetime，只把 pass 顺序和资源关系显式化。

## 17. 推荐演进路线

结合当前项目状态，推荐按以下路线推进：

### 阶段 1：保持当前主线程直接渲染

继续让 `KairosEditorRuntime::redraw` 调用 `RenderPipeline::render()`。

这个阶段目标：

- 跑通 clear
- 跑通 egui pass
- 跑通第一个 mesh pass
- 跑通 depth buffer
- 跑通 resize 后资源重建

### 阶段 2：抽出 FrameSnapshot / RenderPacket

让 `engine.update` 或渲染收集阶段产出：

```text
camera
visible objects
lights
materials
meshes
ui draw data
```

渲染函数只接受 packet，不直接读世界。

### 阶段 3：引入轻量 Render Graph

先支持：

```text
ScenePass
EguiPass
Present
```

然后扩展：

```text
ShadowPass
DepthPrepass
PostProcessPass
PickingPass
GizmoPass
```

### 阶段 4：引入 Worker Pool

把明显 CPU 重的任务并行化：

```text
shadow culling
scene culling
transparent sorting
instance packing
light list building
```

但仍可由主线程最终执行 wgpu command recording。

### 阶段 5：必要时再拆 Render Thread

只有 profiler 证明主线程被渲染侧拖住时，再把 `RenderPipeline` 的所有权迁移到 Render Thread。

此时的交互应是：

```text
Main Thread:
  send RenderMessage::Frame(packet)

Render Thread:
  receive packet
  build graph
  execute graph
  submit/present
```

## 18. 最重要的判断标准

不要为了“像 Unity”而拆线程。

更好的判断方式是：

```text
如果问题是 pass 顺序和资源依赖混乱：
  引入 Render Graph。

如果问题是 culling/sorting/收集数据太慢：
  引入 Worker Pool。

如果问题是 Main Thread 被 present/submit/GPU 资源管理拖住：
  引入 Render Thread。

如果目前只是清屏、画 UI、画少量 mesh：
  保持主线程直接渲染。
```

用一句话概括：

```text
Render Graph 解决“渲染流程组织”的问题；
Worker Pool 解决“CPU 任务并行”的问题；
Render Thread 解决“主线程与 GPU 提交/呈现解耦”的问题。
```

这三者相关，但不是同一个东西，也不应该在项目早期一次性全部上齐。

## 19. 参考资料

wgpu / WebGPU：

- [wgpu docs.rs](https://docs.rs/wgpu/)
- [wgpu 27.0.1 crate](https://docs.rs/crate/wgpu/27.0.1)
- [wgpu wiki: Do's and Don'ts](https://github.com/gfx-rs/wgpu/wiki/Do%27s-and-Dont%27s)
- [wgpu wiki: Encapsulating Graphics Work](https://github.com/gfx-rs/wgpu/wiki/Encapsulating-Graphics-Work)
- [WebGPU Specification](https://www.w3.org/TR/webgpu/)
- [WGSL Specification](https://www.w3.org/TR/WGSL/)

Unity：

- [Unity RenderingThreadingMode](https://docs.unity.cn/ScriptReference/Rendering.RenderingThreadingMode.html)
- [Unity NativeGraphicsJobs](https://docs.unity.cn/2023.1/Documentation/ScriptReference/Rendering.RenderingThreadingMode.NativeGraphicsJobs.html)
- [Unity PlayerSettings.graphicsJobs](https://docs.unity.cn/2020.1/Documentation/ScriptReference/PlayerSettings-graphicsJobs.html)
- [Unity CPU Profiler module](https://docs.unity.cn/2023.3/Documentation/Manual/ProfilerCPU.html)
- [Unity profiler markers](https://docs.unity.cn/Manual/profiler-markers.html)

可选阅读：

- Bevy `bevy_render` 的 RenderGraph / Phase 设计
- Unreal Engine Render Dependency Graph, RDG
- Unity URP/HDRP Render Graph 相关文档

## 20. Render Graph 是否会导致 Main Thread 与 Render Thread 串行接力

一个很重要的问题是：

```text
如果需要先构建出 Render Graph，
Render Thread 才能知道渲染指令的执行顺序；
而 Render Graph 又需要等 Main Thread 执行完所有渲染阶段逻辑后才能完整构建，
那是否就只能等 Main Thread 执行完，Render Thread 才接力执行？
这样是不是无法做到较好的负载均衡和并行？
```

这个担心是成立的。

如果流程设计成：

```text
Main Thread:
  Update
  Cull
  Collect
  Sort
  Build RenderGraph
  Send to Render Thread

Render Thread:
  Execute RenderGraph
  Record wgpu commands
  Submit
```

那么它确实是：

```text
Main Thread 先跑完
Render Thread 再接力
```

这种模型只能把 `present`、`submit`、GPU resource 管理等工作从主线程挪走，但不能很好地做到同一帧内的 CPU 负载均衡。

所以更准确的理解是：

```text
Render Thread 不是用来吃掉所有渲染 CPU 工作的；
负载均衡主要靠 Worker Pool；
Render Thread 更像 GPU 提交协调者。
```

### 20.1 Render Graph 可以拆成三层

Render Graph 不一定等于“Main Thread 完整算完后生成的一整张最终图”。

更好的拆法是：

```text
1. 静态图结构
   这一类相机/管线大概有哪些 pass：
   Shadow -> Depth -> Scene -> PostProcess -> UI

2. 每帧图实例
   本帧哪些 feature 开启？
   几个 light？
   几个 cascade？
   输出到哪个 target？

3. 每个 pass 的数据 payload
   visible draw list
   sorted transparent list
   shadow caster list
   ui meshes
```

其中第 1 层通常早就知道，不需要等 Main Thread 跑完。

第 2 层也经常可以较早构建，因为它主要依赖相机、渲染设置、窗口大小、feature 开关。

真正耗时的通常是第 3 层：

```text
Cull
Collect
Sort
Batch
Pack Instance Buffer
Build DrawList
```

这些不应该全在 Main Thread 上串行做，而应该拆成 worker tasks。

### 20.2 更合理的帧结构

更现代的结构接近：

```text
Main Thread:
  处理输入、编辑器、世界更新
  产出 FrameSnapshot
  调度渲染准备任务

Worker Pool:
  Shadow Cull
  Scene Cull
  Transparent Sort
  UI Mesh Build
  Instance Packing

Render Graph:
  早早知道 pass 结构
  每个 pass 持有它需要等待的数据句柄

Render Thread:
  执行上一帧或当前已就绪的图
  等待必要数据
  录制 wgpu commands
  submit / present
```

也就是说，Render Graph 里不一定一开始就塞满最终数据。它可以先表达：

```text
ShadowPass:
  reads shadow_cull_result_task

ScenePass:
  reads scene_cull_result_task
  reads shadow_depth

UIPass:
  reads ui_mesh_task
```

等对应 worker task 完成后，pass 才真正 encode。

### 20.3 真正的并行通常来自跨帧流水线

很多引擎会接受 1 帧左右的 pipeline latency：

```text
时间片 T:

Main Thread:
  正在更新 Frame N + 1

Worker Pool:
  正在准备 Frame N + 1 的 culling/sorting

Render Thread:
  正在提交 Frame N

GPU:
  正在执行 Frame N - 1
```

这样并行度才真正出来：

```text
CPU Main 不等 Render
Render 不等 Main 的当前帧
GPU 不等 CPU 当前帧
```

代价是：

```text
输入到画面可能多 1 帧延迟
资源生命周期更复杂
需要 frames-in-flight 管理
```

所以它不是免费收益，而是用延迟和复杂度换取吞吐。

### 20.4 同一帧内可以并行，但范围有限

同一帧内可以并行的主要是 CPU 准备任务：

```text
Shadow Cull      \
Scene Cull        \
UI Mesh Build      -> 汇总 FramePacket
Transparent Sort  /
Instance Packing /
```

但 GPU pass 的逻辑顺序通常不能随便并行：

```text
Shadow 必须先于读取 shadow map 的 Scene
Scene 必须先于 PostProcess
PostProcess 必须先于 UI composite
```

所以 Render Graph 解决的是：

```text
哪些东西必须串行
哪些东西可以提前准备
哪些 pass 可以跳过
哪些资源什么时候有效
```

它不是魔法并行器。

### 20.5 修正后的理解

不应该理解成：

```text
Main Thread 完整构建 Render Graph
Render Thread 才开始工作
```

更理想的结构是：

```text
Main Thread:
  产出稳定的 FrameSnapshot

Render System:
  立即根据 pipeline/settings 构建 graph skeleton

Worker Pool:
  并行填充每个 pass 需要的数据

Render Thread:
  消费已经完成的数据
  按 graph dependency 录制和提交命令
```

Render Graph 负责“依赖关系”，Worker Pool 负责“负载均衡”，Render Thread 负责“提交和 GPU 资源边界”。

### 20.6 对 KairosEngine 的补充建议

当前不需要急着拆 Render Thread。

更合适的路线是：

```text
第一步：
  主线程直接 render，先把 ScenePass / EguiPass / Depth / Resize 跑稳。

第二步：
  抽 FrameSnapshot / RenderPacket。
  让渲染不直接读 editor/world 状态。

第三步：
  加轻量 Render Graph。
  先解决 pass 顺序和资源关系，不追求多线程。

第四步：
  加 Worker Pool。
  把 culling/sorting/packing 并行化。

第五步：
  profiler 证明主线程被 submit/present/资源上传拖住后，
  再拆 Render Thread。
```

总结：

```text
如果没有 Worker Pool 和跨帧流水线，
Render Thread 很容易只是把主线程后半段搬到另一个线程，
整体并不会自动变快。

Render Graph 的价值不是直接提升并行度，
而是先把渲染依赖讲清楚；
有了这个结构，后面才更容易安全地并行。
```
