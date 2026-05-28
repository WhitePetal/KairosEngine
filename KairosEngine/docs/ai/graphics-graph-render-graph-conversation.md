# GraphicsGraph / RenderGraph 设计对话完整记录

> 日期：2026-05-27  
> 状态：完整对话归档，非最终实现规范  
> 范围：围绕 `src/graphics/render_pipeline.rs` 临时渲染管线拆分，设计 `GraphicsGraph` / `RenderGraph` 系统，让业务逻辑不直接关心 wgpu 管线细节。  
> 说明：本文按对话顺序保留本轮讨论中用户与 AI 的可见内容。中间的终端检索、文件读取和网页调研工具输出没有作为对话正文展开；它们只作为当时回答的依据。

## 对话 1：总体设计、调研与实现方向

### User

```text
# Context from my IDE setup:

## Active file: src/graphics/render_pipeline.rs

## Open tabs:
- render_pipeline.rs: src/graphics/render_pipeline.rs
- Diffuse_AlphaTest_LightBaking_Fog.shader: /Users/baiaoxiang/hwgamemain_ktv/Assets/LocalResources/Common/Shaders/Effect/Diffuse_AlphaTest_LightBaking_Fog.shader
- PBR_NoLight_New.shader: /Users/baiaoxiang/hwgamemain_ktv/Assets/LocalResources/Common/Shaders/Effect/PBR_NoLight_New.shader
- shader.wgsl: res/shaders/shader.wgsl
- camera.rs: src/graphics/camera.rs

## My request for Codex:
我现在准备对当前项目这个简单、临时的渲染管线进行拆分和重构。我当前的第一目标是设计一套GraphicsGraph/RenderGraph系统，能够让逻辑/业务代码无需关心渲染和管线代码就能进行渲染任务，同时管线可以通过GraphicsGraph来组织和优化自身内部的资源管理和渲染调度。深入理解和分析这个设计，并深度调研相关内容，给出详尽的分析报告和方案实现
```

### Assistant 过程消息

```text
我会先把当前 `render_pipeline`、相机和 shader 这条路径读清楚，再把它和业界常见的 FrameGraph/RenderGraph 设计对照起来。这样报告会落在你这个工程的真实形状上，而不是泛泛讲一套抽象架构。
```

```text
刚才的快速检索里已经看到一个重要线索：项目里之前保存过 `wgpu-threading-render-graph-notes.md`，而当前 `render_pipeline.rs` 仍然是临时直连式管线。接下来我会读那份旧笔记和现有实现，把“已有约定”和“新图系统应该承担的职责”分清。
```

```text
外部资料这边我会以一手资料为主：Frostbite 的 FrameGraph 讲座页、Unreal RDG 官方文档、Bevy 当前 RenderGraph Rustdoc、wgpu 29 的 CommandEncoder/RenderPass/TextureUsages 文档。它们能覆盖“为什么需要图”“图如何表示 pass/resource/edge”“wgpu 下哪些边界不能乱跨”这三块。
```

### Assistant

```text
我没有直接改代码，先按“设计报告 + 落地实现方案”的方式整理。核心判断是：你现在不应该先做独立 Render Thread，而应该先做一个轻量 `GraphicsGraph` 门面 + 每帧构建的 `RenderGraph`。它先解决职责边界：业务/逻辑只提交“要画什么”，渲染系统负责“怎么组织 pass、资源、wgpu 命令”。

**当前问题**
当前 [render_pipeline.rs](/Users/baiaoxiang/KairosEngine/KairosEngine/src/graphics/render_pipeline.rs:28) 把窗口 Surface、Device/Queue、纹理加载上传、bind group、相机、mesh、shader pipeline、draw pass 全塞在一个 `RenderPipeline` 里。它更像“临时 demo renderer”，不是可扩展渲染层。

帧循环在 [runtime.rs](/Users/baiaoxiang/KairosEngine/KairosEngine/src/kairos_editor/runtime.rs:205) 手动串联：获取 swapchain texture、跑 egui、更新 egui 纹理/缓冲、调用 `render_pipeline.render()`、再把 egui 画进同一个 pass、submit/present。

更关键的是 UI/业务边界已经泄露了：`Drawer::ui` 直接接收 `&mut RenderPipeline`、`&mut wgpu::CommandEncoder`、`&mut egui_wgpu::Renderer`，见 [ui.rs](/Users/baiaoxiang/KairosEngine/KairosEngine/src/kairos_editor/ui.rs:126)。`SceneWindow` 甚至在 UI 绘制函数里创建 render target 并调用渲染，见 [scene_window.rs](/Users/baiaoxiang/KairosEngine/KairosEngine/src/kairos_editor/ui/scene_window.rs:62)。这正好是你想拆掉的耦合点。

**调研结论**
Frostbite 的 FrameGraph 目标是把所有 render passes 和 resources 组织成图，让功能模块解耦同时保持效率。Unreal RDG 更明确：先把整帧 pass 记录到依赖图，再 compile，并按依赖顺序 execute；它利用整帧知识做异步计算调度、资源别名、barrier/layout 处理和验证。Bevy 的 RenderGraph 则是 Rust/wgpu 生态里很近的参考：由 nodes、edges、slots 组成，runner 每帧按 edge 顺序执行 nodes。

wgpu 层面要记住边界：`CommandEncoder` 只是记录 render/compute/copy 操作，最后 `finish()` 成 command buffer；`begin_render_pass` 后 encoder 在 pass 活着时不能再被 mutating 使用。所以 RenderGraph 里不能长期保存 `wgpu::RenderPass<'_>`，只能保存“执行时如何 begin/end pass”的节点。纹理 usage 也要由图系统统一推导/声明，比如写入后采样的 scene texture 必须同时有 `RENDER_ATTACHMENT | TEXTURE_BINDING`；`TRANSIENT` 只适合不会跨 pass 复用的 attachment，并且有约束。

**推荐架构**
```text
Game/ECS/UI logic
  -> RenderRequest / SceneViewRequest / DebugDrawRequest
  -> GraphicsGraph facade
  -> FramePacket / DrawList
  -> RenderGraphBuilder
  -> Compile: dependency sort, validation, lifetime, allocation
  -> Execute: wgpu CommandEncoder / RenderPass / ComputePass
  -> submit + present
```

我建议把名字分成两层：

`GraphicsGraph`：业务侧门面。UI、ECS、游戏逻辑只和它说“我要一个 SceneView”“我要画这些 mesh/material”“我要 picking buffer”。它不暴露 wgpu。

`RenderGraph`：渲染侧每帧 DAG。Pass 声明读写资源，graph 编译出执行顺序、临时资源生命周期和最终输出。

**当前项目第一版图**
```text
Frame begin
  import swapchain_color
  request scene_view_color for SceneWindow rect

SceneViewPass
  write scene_view_color
  draw current test mesh / future DrawList

EguiPrepare
  update egui textures/buffers
  bind scene_view_color to stable egui TextureId

EditorUiPass
  read scene_view_color through egui texture binding
  write swapchain_color

Present
```

关键变化：`SceneWindow::ui` 不再调用 `render_pipeline.render()`。它只做：
```rust
let view = graphics_ui.request_scene_view("main_scene", width, height);
ui.painter().image(view.texture_id, rect, uv, Color32::WHITE);
```
真正的 scene pass 在 egui layout 结束后，由 graph 根据本帧请求统一执行。

**核心类型草案**
```rust
pub struct GraphicsGraph {
    device: GraphicsDevice,
    render_assets: RenderAssets,
    features: Vec<Box<dyn RenderFeature>>,
}

pub struct FramePacket {
    pub frame_id: u64,
    pub views: Vec<ViewPacket>,
    pub draws: DrawList,
    pub ui: EguiFramePacket,
}

pub struct RenderGraphBuilder {
    resources: ResourceRegistry,
    passes: Vec<PassNode>,
    final_outputs: Vec<ResourceId>,
}

pub struct PassNode {
    name: &'static str,
    reads: Vec<ResourceUse>,
    writes: Vec<ResourceUse>,
    execute: Box<dyn Fn(&mut PassContext) -> anyhow::Result<()>>,
}
```

资源描述不要直接等同 wgpu 对象：
```rust
pub struct TextureDesc {
    pub size: GraphSize,          // Surface, Viewport(view_id), Fixed(w,h)
    pub format: wgpu::TextureFormat,
    pub usage: wgpu::TextureUsages,
    pub sample_count: u32,
}

pub enum ResourceSource {
    Imported,    // swapchain, egui native texture, history
    Transient,   // 本帧临时 texture/buffer
    Persistent,  // shadow atlas, history buffer, cached viewport texture
}
```

**编译流程**
1. 收集 pass 的 `read/write`。
2. 对每个资源建立 writer -> reader、writer -> writer 依赖。
3. 从 final outputs 反向标记有效 pass，剔除无贡献 pass。
4. 拓扑排序，检测环。
5. 计算 transient resource 的 first_use/last_use。
6. 第一版只做缓存复用，不做 aliasing；第二版再做 texture pool/alias。
7. 执行时每个 pass 自己短作用域 begin/end `RenderPass`，不把 `RenderPass` 存进 graph。

**落地步骤**
1. 先把 `RenderPipeline` 拆成 `GraphicsDevice` 和 `DemoSceneRenderer`：Device/Queue/Surface 留在 device，当前五边形 shader/mesh/texture 放到 demo renderer。
2. 加 `FrameRenderRequests`：`SceneWindow` 只提交 viewport 请求，不再拿 `CommandEncoder`。
3. 改 `Drawer::ui` 参数：从 `RenderPipeline + CommandEncoder + egui_renderer` 改成 `EditorUiCtx`，里面只暴露 `request_scene_view()` 这类高层 API。
4. 做最小 `RenderGraphBuilder`：支持 imported swapchain、cached scene texture、两个 pass：scene pass 和 egui pass。
5. 把纹理上传从 `RenderPipeline::new` 移到 `RenderAssets<GpuTexture>`，保留你已有的 CPU `TextureAssets`，形成 `TextureHandle -> GpuTextureId`。
6. 等这个跑通后再加 depth、camera buffer 更新、mesh/material handle、picking、postprocess。
7. Render Thread 暂缓。你之前的项目笔记也已经指向：先 `FramePacket/DrawList`，再 Worker Pool，最后在 profiler 证明需要时拆 Render Thread。

这套方案的第一目标不是“炫技式图系统”，而是把边界立住：业务代码不碰 wgpu，UI 不负责录制 GPU 命令，渲染功能通过 pass/resource 声明参与整帧调度。这样现在能承接 SceneWindow，后面也能自然长出 shadow、picking、outline、bloom、TAA。
```

## 对话 2：具体实现方案和技术细节

### User

```text
# Context from my IDE setup:

## Active file: src/graphics/render_pipeline.rs

## Open tabs:
- render_pipeline.rs: src/graphics/render_pipeline.rs
- Diffuse_AlphaTest_LightBaking_Fog.shader: /Users/baiaoxiang/hwgamemain_ktv/Assets/LocalResources/Common/Shaders/Effect/Diffuse_AlphaTest_LightBaking_Fog.shader
- PBR_NoLight_New.shader: /Users/baiaoxiang/hwgamemain_ktv/Assets/LocalResources/Common/Shaders/Effect/PBR_NoLight_New.shader
- shader.wgsl: res/shaders/shader.wgsl
- camera.rs: src/graphics/camera.rs

## My request for Codex:
现在关注具体的实现方案和技术细节，不要改我的代码，思考和给出非常足够的详尽的代码实现和讲解
```

### Assistant

```text
可以。下面我按“能直接照着搭第一版”的粒度讲，仍然不改你的代码。

**总体形状**
你当前第一版不要做“完整 AAA RenderGraph”，而是做一个可扩展的最小图：

```text
Runtime
  负责 window / surface / egui / submit

GraphicsContext
  持有 device / queue / surface_config / resource caches

GraphicsGraph
  业务侧入口：收集本帧渲染请求

RenderGraph
  渲染侧 DAG：pass/resource/依赖/执行

RenderFeatures
  具体功能模块：SceneView、Egui、Debug、Picking、PostProcess
```

关键原则：业务代码只提交“意图”，不要拿 `wgpu::CommandEncoder`、`RenderPipeline`、`TextureView`。

---

**业务侧 API**
例如 `SceneWindow` 未来只应该这样：

```rust
pub struct EditorGraphicsUi<'a> {
    graph: &'a mut GraphicsGraph,
}

impl<'a> EditorGraphicsUi<'a> {
    pub fn request_scene_view(
        &mut self,
        id: SceneViewId,
        width: u32,
        height: u32,
        camera: CameraId,
    ) -> SceneViewHandle {
        self.graph.request_scene_view(SceneViewRequest {
            id,
            width,
            height,
            camera,
            clear_color: [0.1, 0.2, 0.3, 1.0],
        })
    }
}
```

UI 使用：

```rust
let scene_view = graphics_ui.request_scene_view(
    SceneViewId::main(),
    width,
    height,
    CameraId::editor(),
);

ui.painter().image(
    scene_view.egui_texture_id,
    rect,
    egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
    egui::Color32::WHITE,
);
```

这里 `SceneWindow` 不知道 scene texture 怎么创建，不知道 pass 怎么排，不知道 `wgpu`。

---

**GraphicsGraph**
它是本帧请求收集器，不直接执行 GPU 命令：

```rust
pub struct GraphicsGraph {
    frame_id: u64,
    scene_views: Vec<SceneViewRequest>,
    debug_draws: Vec<DebugDrawRequest>,
}

impl GraphicsGraph {
    pub fn begin_frame(&mut self) {
        self.scene_views.clear();
        self.debug_draws.clear();
        self.frame_id += 1;
    }

    pub fn request_scene_view(&mut self, req: SceneViewRequest) -> SceneViewHandle {
        self.scene_views.push(req);

        SceneViewHandle {
            egui_texture_id: egui::TextureId::Managed(0), // 第一版可先由 SceneViewCache 填真实 id
        }
    }

    pub fn build_frame_packet(&self, world: &WorldLike) -> FramePacket {
        FramePacket {
            frame_id: self.frame_id,
            scene_views: self.scene_views.clone(),
            draw_list: collect_draw_list(world),
        }
    }
}
```

`GraphicsGraph` 面向逻辑层；`RenderGraph` 面向渲染执行层。不要把这两个合成一个东西，否则很快又会把业务和 GPU 混在一起。

---

**FramePacket**
这是逻辑世界到渲染世界的边界：

```rust
pub struct FramePacket {
    pub frame_id: u64,
    pub scene_views: Vec<SceneViewRequest>,
    pub draw_list: DrawList,
}

pub struct DrawList {
    pub opaque: Vec<DrawItem>,
    pub transparent: Vec<DrawItem>,
}

pub struct DrawItem {
    pub mesh: MeshHandle,
    pub material: MaterialHandle,
    pub transform: float4x4,
    pub sort_key: u64,
}
```

这里要避免放 `&Entity`、`&Transform`、`&TextureAsset` 这种借用。`FramePacket` 应该是本帧不可变快照，未来才能安全丢给 worker 或 render thread。

---

**RenderGraph 资源**
第一版资源可以只支持 texture：

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GraphTextureId(u32);

pub enum TextureSize {
    Surface,
    Fixed { width: u32, height: u32 },
    SceneView(SceneViewId),
}

pub struct GraphTextureDesc {
    pub label: String,
    pub size: TextureSize,
    pub format: wgpu::TextureFormat,
    pub usage: wgpu::TextureUsages,
    pub sample_count: u32,
}

pub enum GraphResource {
    ImportedTexture {
        view: wgpu::TextureView,
        format: wgpu::TextureFormat,
    },
    TransientTexture {
        desc: GraphTextureDesc,
    },
    PersistentTexture {
        key: PersistentTextureKey,
        desc: GraphTextureDesc,
    },
}
```

`ImportedTexture` 用于 swapchain view。`TransientTexture` 用于本帧临时中间结果。`PersistentTexture` 用于 SceneWindow viewport、history、shadow atlas 这类跨帧缓存资源。

---

**RenderGraph Pass**
Pass 声明读写，不立刻执行：

```rust
pub enum TextureAccess {
    ReadSampled,
    WriteColorAttachment {
        load: wgpu::LoadOp<wgpu::Color>,
        store: wgpu::StoreOp,
    },
    WriteDepth,
    CopySrc,
    CopyDst,
}

pub struct TextureUse {
    pub texture: GraphTextureId,
    pub access: TextureAccess,
}

pub struct RenderPassNode {
    pub name: String,
    pub reads: Vec<TextureUse>,
    pub writes: Vec<TextureUse>,
    pub execute: Box<dyn Fn(&mut RenderPassContext) -> anyhow::Result<()>>,
}
```

注意：这里的 `RenderPassNode` 名字不是 `wgpu::RenderPass`。它只是图节点。真正的 `encoder.begin_render_pass(...)` 只能在 `execute` 里短生命周期创建。

---

**Builder 用法**
第一版 builder 大概这样：

```rust
let swapchain = graph.import_texture("swapchain", surface_view, surface_format);

let scene_color = graph.create_persistent_texture(
    PersistentTextureKey::SceneView(scene_view.id),
    GraphTextureDesc {
        label: "SceneView.Color".into(),
        size: TextureSize::Fixed {
            width: scene_view.width,
            height: scene_view.height,
        },
        format: surface_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
             | wgpu::TextureUsages::TEXTURE_BINDING,
        sample_count: 1,
    },
);

graph.add_pass(RenderPassNode {
    name: "SceneViewPass".into(),
    reads: vec![],
    writes: vec![TextureUse {
        texture: scene_color,
        access: TextureAccess::WriteColorAttachment {
            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            store: wgpu::StoreOp::Store,
        },
    }],
    execute: Box::new(move |ctx| {
        let view = ctx.texture_view(scene_color);
        let mut pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SceneViewPass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });

        ctx.demo_scene_renderer.draw(&mut pass);
        Ok(())
    }),
});
```

---

**编译阶段**
`compile()` 第一版做四件事就够了：

```rust
pub fn compile(&self, final_outputs: &[GraphTextureId]) -> anyhow::Result<CompiledRenderGraph> {
    let live_passes = self.cull_dead_passes(final_outputs)?;
    let edges = self.build_dependency_edges(&live_passes)?;
    let order = topological_sort(&edges)?;
    let lifetimes = self.compute_resource_lifetimes(&order)?;
    Ok(CompiledRenderGraph { order, lifetimes })
}
```

依赖规则：

```text
A writes X, B reads X  => A -> B
A writes X, B writes X => A -> B，除非明确允许覆盖
A reads X, B writes X  => 如果 B 必须晚于 A，需要显式顺序或检查冲突
```

第一版不要自动重排太激进。先保证正确、可 debug。

---

**执行阶段**
执行时才创建真实资源：

```rust
pub struct RenderPassContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub resources: &'a GraphResourceCache,
    pub demo_scene_renderer: &'a DemoSceneRenderer,
}

impl CompiledRenderGraph {
    pub fn execute(&self, ctx: &mut RenderPassContext) -> anyhow::Result<()> {
        for pass_id in &self.order {
            let pass = ctx.resources.pass(*pass_id);
            ctx.encoder.push_debug_group(&pass.name);
            (pass.execute)(ctx)?;
            ctx.encoder.pop_debug_group();
        }
        Ok(())
    }
}
```

核心点：graph 不持有 `CommandEncoder`，pass 不跨帧持有 `TextureView` 的临时引用。真实资源放在 cache 里。

---

**你当前代码的迁移切法**
第一刀：把 [render_pipeline.rs](/Users/baiaoxiang/KairosEngine/KairosEngine/src/graphics/render_pipeline.rs:28) 分成：

```text
GraphicsDevice
  window/surface/adapter/device/queue/surface_config/resize/get_current_texture

DemoSceneRenderer
  当前 shader、pipeline、vertex/index、texture bind group、camera bind group

GraphicsGraph
  本帧请求

RenderGraph
  pass/resource 编译执行
```

第二刀：把 `create_render_target()` 改成缓存式资源管理。你现在每帧 `SceneWindow` 创建 texture view，这会让 egui texture 绑定生命周期非常脆弱。应该变成：

```rust
pub struct SceneViewCache {
    entries: HashMap<SceneViewId, SceneViewTarget>,
}

pub struct SceneViewTarget {
    width: u32,
    height: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    egui_texture_id: egui::TextureId,
}
```

尺寸变了才重建，没变就复用。

第三刀：`Drawer::ui` 不再传 `RenderPipeline` 和 `CommandEncoder`，而传：

```rust
pub struct EditorFrameCtx<'a> {
    pub graphics: &'a mut EditorGraphicsUi<'a>,
    pub log: &'a mut Log,
}
```

这样 UI 层和 GPU 执行层就真的分开了。

---

**最小第一版目标**
不要一上来做 shadow、deferred、aliasing。第一版只要跑通：

```text
1. swapchain imported texture
2. scene_view persistent texture
3. SceneViewPass 写 scene_view
4. EguiPass 写 swapchain
5. SceneWindow 只 request_scene_view，不碰 wgpu
```

这一步做完，你的架构就已经从“临时 render_pipeline”变成“图驱动渲染入口”了。后面加 Picking、Outline、PostProcess，只是继续注册 pass 和资源，而不是改 runtime 大主流程。
```

## 对话 3：保存本轮讨论

### User

```text
# Context from my IDE setup:

## Active file: src/graphics/render_pipeline.rs

## Open tabs:
- render_pipeline.rs: src/graphics/render_pipeline.rs
- Diffuse_AlphaTest_LightBaking_Fog.shader: /Users/baiaoxiang/hwgamemain_ktv/Assets/LocalResources/Common/Shaders/Effect/Diffuse_AlphaTest_LightBaking_Fog.shader
- PBR_NoLight_New.shader: /Users/baiaoxiang/hwgamemain_ktv/Assets/LocalResources/Common/Shaders/Effect/PBR_NoLight_New.shader
- shader.wgsl: res/shaders/shader.wgsl
- camera.rs: src/graphics/camera.rs

## My request for Codex:
把我们的这些对话保存为文档存入 docs/ai 目录，要求保留完整的对话内容
```

## 对话 4：从 GraphicsCommand 节点构建完整 GraphicsGraph

### User

```text
# Context from my IDE setup:

## Active file: src/kairos_editor/runtime.rs

## Open tabs:
- runtime.rs: src/kairos_editor/runtime.rs
- graphics_graph.rs: src/graphics/graphics_graph.rs
- mesh.rs: src/graphics/mesh.rs
- render_pipeline.rs: src/graphics/render_pipeline.rs
- graphics.rs: src/graphics.rs

## My request for Codex:
现在我的GraphicsGraph构建了几个简单的节点。@runtime.rs (214-263) 这里注释的内容就是通过GraphicsCommand描述渲染过程，并向后台添加GraphicGraph的节点，我该通过怎么样的算法来通过这些节点构建完整的GraphicsGraph，并确定图入口以及如何根据图确定执行顺序并执行？
```

### Assistant 过程消息

```text
我先把现有 `GraphicsCommand`、`GraphicsGraph`、runtime 里那段注释和渲染入口读一遍，再给你一个贴合当前代码形状的算法方案。这里关键不是泛泛讲“拓扑排序”，而是看你现在节点和资源边到底长什么样。
```

```text
我看到了：现在 `GraphicsCommand` 其实已经像一条“录制命令流”，而 `GraphicsGraph::from_commands` 还没落地。下一步我会再看现有渲染函数怎么拿 surface、encoder、egui renderer，这会决定图执行时哪些节点只是 CPU 资源准备，哪些节点必须在同一个 `wgpu::CommandEncoder` / render pass 生命周期里跑。
```

```text
仓库里已经有一份 `docs/ai/graphics-graph-render-graph-conversation.md`，这个很可能是你之前对 render graph 的思路沉淀。我会把它也读一下，避免给出一个和你已有方向打架的方案。
```

```text
当前这一版我会建议你先把 `BeginRenderPass / Draw / EndRenderPass` 编译成一个“可执行 pass”，而不是把 begin/end 当成能随便拓扑排序的独立 GPU 节点。wgpu 的生命周期限制决定了这一点：`RenderPass` 只能在 execute 时短作用域创建，用完立刻 drop。
```

```text
还有一个实现前必须注意的小坑：`cargo check --locked` 现在会在 `runtime.rs` 的 redraw 里因为外层 `render_pipeline` guard 没提前 drop 而报 E0502。这个不影响图算法本身，但真正落地前要把那段外层锁收窄或移走，否则后续编译会被它挡住。
```

### Assistant

```text
<proposed_plan>
# GraphicsCommand 到 GraphicsGraph 的编译算法

## Summary

当前不要把 `CreateAttachment -> BeginRenderPass -> Draw -> EndRenderPass` 直接当成可执行链。更稳的算法是：先把 [graphics_graph.rs](/Users/baiaoxiang/KairosEngine/KairosEngine/src/graphics/graphics_graph.rs:8) 里的命令流规范化成资源表、渲染 pass、输出 sink，再从最终输出反向裁剪活节点，最后对活的可执行节点做拓扑排序。

图入口要分清两种概念：

- **最终输出入口**：从 `BindAttachmentToEgui`、`CopyAttachmentToEGui`，以后还有 `PresentSwapchain` 这类 sink 开始倒推。
- **执行入口**：裁剪后入度为 0 的 pass，由拓扑排序自动得到，不需要手动指定单个 root。

## Key Changes

- `GraphicsCommand` 保持录制语义，但 `from_commands` 改成 `Result<GraphicsGraph, GraphBuildError>`，验证嵌套 pass、未关闭 pass、非法 id、空输出。
- 多个 command 的本地 id 必须重映射成全局 id，例如 `(command_index, local_attachment_id) -> GraphAttachmentId`，避免不同命令里的 `0` 撞在一起。
- `CreateAttachment`、`SetVPMatrix` 是资源声明，不是 GPU 执行节点；`BeginRenderPass + Draw* + EndRenderPass` 编译成一个 `RenderPassNode`。
- `Draw` 不单独拓扑排序，保留在所属 pass 的 `draws: Vec<DrawOp>` 中按录制顺序执行。
- `BindAttachmentToEguiNode` 需要保存 sender 或改成返回 `TextureId` 结果；现在 [graphics_graph.rs](/Users/baiaoxiang/KairosEngine/KairosEngine/src/graphics/graphics_graph.rs:134) 的 `sender` 参数被丢掉了。

## Algorithm

1. **Parse Command Stream**
   - 遍历所有 `GraphicsCommand.nodes`。
   - 遇到 `CreateAttachment`，登记 attachment desc。
   - 遇到 `SetVPMatrix`，登记 uniform/vp desc。
   - 遇到 `BeginRenderPass`，打开一个 `PassBuilder`，记录 color attachments、vp、`force_clear`。
   - 遇到 `Draw`，追加到当前 `PassBuilder.draws`。
   - 遇到 `EndRenderPass`，关闭并生成 `RenderPassNode`。
   - 遇到 `BindAttachmentToEgui` / `CopyAttachmentToEGui`，生成 sink node。

2. **Build Dependency Edges**
   - 对每个 pass，声明 `reads = [vp, mesh/material resources]`，`writes = [attachments]`。
   - 对每个 sink，声明 `reads = [attachment]`。
   - 维护 `last_writer[attachment]`。
   - `A writes X, B reads X` 加边 `A -> B`。
   - `A writes X, B writes X` 第一版也加边 `A -> B`，保证稳定顺序。
   - `force_clear = false` 时，pass 对 attachment 还隐含读取旧内容，必须依赖前一个 writer；`force_clear = true` 时可视为覆盖写。

3. **Determine Live Graph**
   - 最终输出是所有 sink：当前注释里的 [runtime.rs](/Users/baiaoxiang/KairosEngine/KairosEngine/src/kairos_editor/runtime.rs:260) 就是 `BindAttachmentToEgui(frame_buffer_id, ...)`。
   - 从这些 sink 沿反向边 DFS/BFS，标记所有对最终输出有贡献的 pass。
   - 未被标记的 pass 和资源声明全部裁掉。

4. **Topological Sort**
   - 对活的可执行节点做 Kahn 拓扑排序。
   - 入度为 0 的活节点就是执行入口，可能有多个。
   - 同入度节点用原始录制顺序做稳定 tie-break。
   - 排序结果数量少于活节点数量时，报 cycle error。

5. **Execute**
   - 执行前根据活资源创建或复用 GPU 资源：attachment 变成 `wgpu::Texture + TextureView`，VP 矩阵变成 uniform buffer/bind group。
   - 一个 frame 只创建一个 `CommandEncoder`，传给 `GraphicsGraph::execute(ctx)`。
   - 每个 `RenderPassNode` 在 execute 内短作用域调用 `encoder.begin_render_pass(...)`，执行全部 draw，然后 drop pass。
   - Egui sink 不开启 render pass，只负责 `register_native_texture` 或 `update_egui_texture_from_wgpu_texture`，然后把 `egui::TextureId` 送回 UI 状态。
   - 现有 [render_pipeline.rs](/Users/baiaoxiang/KairosEngine/KairosEngine/src/graphics/render_pipeline.rs:419) 的硬编码 demo draw 后续应拆成 `DemoSceneRenderer.draw(&mut RenderPass)`，由 graph pass 调用。

## Test Plan

- 单元测试：单 pass 写 attachment 后 bind 到 egui，排序应为 `ScenePass -> EguiSink`。
- 单元测试：创建 attachment 但没有 sink，应被裁剪或返回 no-output error。
- 单元测试：`Draw` 在 pass 外、嵌套 `BeginRenderPass`、缺失 `EndRenderPass` 都返回 build error。
- 单元测试：两个 pass 写同一个 attachment，稳定生成 `first -> second`。
- 集成验证：修掉 [runtime.rs](/Users/baiaoxiang/KairosEngine/KairosEngine/src/kairos_editor/runtime.rs:207) 外层 mutex guard 后跑 `cargo check --locked`；当前检查会先被 E0502 挡住。
</proposed_plan>
```

## 对话 5：不修改代码，给出详细算法讲解和代码实现

### User

```text
# Context from my IDE setup:

## Active file: src/graphics/render_pipeline.rs

## Open tabs:
- render_pipeline.rs: src/graphics/render_pipeline.rs
- runtime.rs: src/kairos_editor/runtime.rs
- graphics_graph.rs: src/graphics/graphics_graph.rs
- mesh.rs: src/graphics/mesh.rs
- graphics.rs: src/graphics.rs

## My request for Codex:
不要修改我的代码，按照给出的方案给出详尽的讲解和代码实现
```

### Assistant

可以，这里我不改你的代码，只按你现在的 `GraphicsCommand` 形状给一版“能落地的第一版实现”。核心思想先钉死：

`CreateAttachment / SetVPMatrix` 不是执行节点，它们是资源声明。  
`BeginRenderPass + Draw... + EndRenderPass` 才编译成一个真正的可执行 pass。  
`BindAttachmentToEgui / CopyAttachmentToEGui` 是最终输出 sink，用它们反向确定哪些 pass 是活的。

**1. 图结构**
第一版建议先长这样：

```rust
type NodeId = usize;
type AttachmentId = usize;
type VpId = usize;
type RenderPassId = usize;

pub struct GraphicsGraph {
    attachments: Vec<Attachment>,
    vp_matrices: Vec<float4x4>,
    passes: Vec<CompiledPass>,
    sinks: Vec<GraphSink>,
    order: Vec<NodeId>,
}

pub struct CompiledPass {
    pub id: RenderPassId,
    pub attachments: Vec<AttachmentId>,
    pub vp_id: VpId,
    pub force_clear: bool,
    pub draws: Vec<DrawOp>,
}

pub struct DrawOp {
    pub mesh: Mesh,
}

pub enum GraphSink {
    BindAttachmentToEgui {
        attachment_id: AttachmentId,
        // 第一版要保留 sender，不能像现在那样丢掉
        sender: tokio::sync::mpsc::Sender<egui::TextureId>,
    },
    CopyAttachmentToEgui {
        attachment_id: AttachmentId,
        egui_texture_id: egui::TextureId,
    },
}
```

这里 `passes` 是真正要排序和执行的节点。`attachments / vp_matrices` 只是资源表。

**2. 从命令流编译 pass**
伪代码大概是这样：

```rust
pub fn from_commands(commands: Vec<GraphicsCommand>) -> Result<GraphicsGraph, GraphBuildError> {
    let mut attachments = Vec::new();
    let mut vp_matrices = Vec::new();
    let mut passes = Vec::new();
    let mut sinks = Vec::new();

    for command in commands {
        let mut open_pass: Option<CompiledPass> = None;

        for node in command.nodes {
            match node {
                GraphNode::CreateAttachment(n) => {
                    ensure_id(n.id, attachments.len())?;
                    attachments.push(n.attachment);
                }

                GraphNode::SetVPMatrix(n) => {
                    ensure_id(n.id, vp_matrices.len())?;
                    vp_matrices.push(n.matrix);
                }

                GraphNode::BeginRenderPass(n) => {
                    if open_pass.is_some() {
                        return Err(GraphBuildError::NestedRenderPass);
                    }

                    validate_attachments(&attachments, &n.attachments)?;
                    validate_vp(&vp_matrices, n.vp_id)?;

                    open_pass = Some(CompiledPass {
                        id: n.id,
                        attachments: n.attachments,
                        vp_id: n.vp_id,
                        force_clear: n.force_clear,
                        draws: Vec::new(),
                    });
                }

                GraphNode::Draw(n) => {
                    let Some(pass) = open_pass.as_mut() else {
                        return Err(GraphBuildError::DrawOutsideRenderPass);
                    };

                    if pass.id != n.render_pass_id {
                        return Err(GraphBuildError::DrawPassMismatch);
                    }

                    pass.draws.push(DrawOp { mesh: n.mesh });
                }

                GraphNode::EndRenderPass(n) => {
                    let Some(pass) = open_pass.take() else {
                        return Err(GraphBuildError::EndWithoutBegin);
                    };

                    if pass.id != n.id {
                        return Err(GraphBuildError::EndPassMismatch);
                    }

                    passes.push(pass);
                }

                GraphNode::BindAttachmentToEgui(n) => {
                    validate_attachment(&attachments, n.attachment_id)?;
                    sinks.push(GraphSink::BindAttachmentToEgui {
                        attachment_id: n.attachment_id,
                        sender: n.sender,
                    });
                }

                GraphNode::CopyAttachmentToEGui(n) => {
                    validate_attachment(&attachments, n.attachment_id)?;
                    sinks.push(GraphSink::CopyAttachmentToEgui {
                        attachment_id: n.attachment_id,
                        egui_texture_id: n.egui_texture_id,
                    });
                }
            }
        }

        if open_pass.is_some() {
            return Err(GraphBuildError::UnclosedRenderPass);
        }
    }

    let order = compile_order(&passes, &sinks)?;

    Ok(GraphicsGraph {
        attachments,
        vp_matrices,
        passes,
        sinks,
        order,
    })
}
```

你现在的 `BindAttachmentToEguiNode` 只有 `attachment_id`，但函数参数里有 `sender`。这说明 struct 应该改成：

```rust
struct BindAttachmentToEguiNode {
    attachment_id: usize,
    sender: Sender<egui::TextureId>,
}
```

否则 graph 执行完没地方把 `TextureId` 送回 UI。

**3. 构建依赖边**
第一版只需要 attachment 依赖就够了：

```rust
fn build_edges(
    passes: &[CompiledPass],
    sinks: &[GraphSink],
) -> Vec<(NodeId, NodeId)> {
    let mut edges = Vec::new();
    let mut last_writer: HashMap<AttachmentId, NodeId> = HashMap::new();

    for (pass_index, pass) in passes.iter().enumerate() {
        for &attachment in &pass.attachments {
            if let Some(prev_writer) = last_writer.insert(attachment, pass_index) {
                edges.push((prev_writer, pass_index));
            }
        }
    }

    let sink_base = passes.len();

    for (sink_index, sink) in sinks.iter().enumerate() {
        let sink_node = sink_base + sink_index;
        let attachment_id = match sink {
            GraphSink::BindAttachmentToEgui { attachment_id, .. } => *attachment_id,
            GraphSink::CopyAttachmentToEgui { attachment_id, .. } => *attachment_id,
        };

        if let Some(writer) = last_writer.get(&attachment_id) {
            edges.push((*writer, sink_node));
        }
    }

    edges
}
```

你现在这个例子里会形成：

```text
ScenePass writes frame_buffer
BindAttachmentToEgui reads frame_buffer

=> ScenePass -> BindAttachmentToEgui
```

所以执行顺序就是先画 scene，再注册或更新 egui texture。

**4. 拓扑排序**
入口不手写，入度为 0 的活节点就是入口：

```rust
fn topo_sort(node_count: usize, edges: &[(NodeId, NodeId)]) -> Result<Vec<NodeId>, GraphBuildError> {
    let mut indegree = vec![0; node_count];
    let mut nexts = vec![Vec::new(); node_count];

    for &(from, to) in edges {
        indegree[to] += 1;
        nexts[from].push(to);
    }

    let mut queue = VecDeque::new();
    for node in 0..node_count {
        if indegree[node] == 0 {
            queue.push_back(node);
        }
    }

    let mut order = Vec::new();

    while let Some(node) = queue.pop_front() {
        order.push(node);

        for &next in &nexts[node] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                queue.push_back(next);
            }
        }
    }

    if order.len() != node_count {
        return Err(GraphBuildError::Cycle);
    }

    Ok(order)
}
```

真正更完整的一版，还要从 `sinks` 反向裁剪 dead pass。但你现在只有一个 scene pass，一个 egui sink，可以第二步再加。

**5. 执行模型**
执行时不要保存 `wgpu::RenderPass`。每个 pass 在 execute 里临时打开：

```rust
impl GraphicsGraph {
    pub fn execute(&mut self, ctx: &mut GraphExecuteContext) -> Result<(), GraphExecuteError> {
        for &node_id in &self.order {
            if node_id < self.passes.len() {
                self.execute_pass(node_id, ctx)?;
            } else {
                let sink_id = node_id - self.passes.len();
                self.execute_sink(sink_id, ctx)?;
            }
        }

        Ok(())
    }

    fn execute_pass(
        &self,
        pass_id: usize,
        ctx: &mut GraphExecuteContext,
    ) -> Result<(), GraphExecuteError> {
        let pass = &self.passes[pass_id];

        let color_view = ctx.attachment_view(pass.attachments[0])?;
        let mut render_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GraphicsGraph Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if pass.force_clear {
                        wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });

        ctx.demo_renderer.set_vp(pass.vp_id, &mut render_pass);

        for draw in &pass.draws {
            ctx.demo_renderer.draw_mesh(&mut render_pass, &draw.mesh);
        }

        Ok(())
    }
}
```

`GraphExecuteContext` 可以先简单点：

```rust
pub struct GraphExecuteContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub egui_renderer: &'a mut egui_wgpu::Renderer,
    pub attachment_cache: &'a mut AttachmentCache,
    pub demo_renderer: &'a mut DemoSceneRenderer,
}
```

**6. 你这段注释对应的结果**
`runtime.rs` 那段命令最终会被编译成：

```text
资源:
  Attachment[0] = frame_buffer
  VP[0] = camera view projection

Pass:
  Pass[0]:
    write Attachment[0]
    read VP[0]
    draws = [mesh]

Sink:
  Sink[0]:
    read Attachment[0]
    bind/update to egui

Order:
  Pass[0] -> Sink[0]
```

这就是第一版 GraphicsGraph 的完整闭环。后面加后处理就是：

```text
ScenePass -> BloomPass -> ToneMappingPass -> EguiSink
```

加 picking 就是：

```text
SceneColorPass -> EguiSink
PickingPass -> ReadbackSink
```

核心算法不变，只是资源读写边更多。

## 对话 6：基于 petgraph 重新生成完整方案和代码实现

### User

```text
# Context from my IDE setup:

## Active file: src/graphics/graphics_graph.rs

## Open tabs:
- graphics_graph.rs: src/graphics/graphics_graph.rs
- render_pipeline.rs: src/graphics/render_pipeline.rs
- runtime.rs: src/kairos_editor/runtime.rs
- mesh.rs: src/graphics/mesh.rs
- graphics.rs: src/graphics.rs

## My request for Codex:
项目里有 petgraph crate, 基于此重新生成完整详尽的方案讲解和完整详尽的代码实现
```

### Assistant 过程消息

```text
我先重新读一下当前 `graphics_graph.rs` 和 `petgraph` 版本/API 形状。你这次明确提到要基于 `petgraph`，所以我会把方案收敛到“用 petgraph 表达依赖图，而不是自己手写拓扑排序”的版本。
```

```text
我已经确认了：项目里是 `petgraph = "0.8.3"`，可以直接用 `petgraph::graph::DiGraph` 和 `petgraph::algo::toposort`。一个很重要的实现约束是你现在 `GraphicsCommand` 里 `DrawNode` 持有 `Mesh`，所以完整实现最好让 `from_commands` 消费 `Vec<GraphicsCommand>`，否则不能从 `&[GraphicsCommand]` 里把 mesh 安全搬进编译后的 graph。
```

### Assistant

可以。下面这版是**基于 `petgraph` 的完整第一版设计**，我不会改你的代码，只给你一份可以对照实现的目标代码。

核心变化是：`petgraph::DiGraph` 只表达“可执行节点之间的依赖”，而不是把每个资源也塞成图节点。也就是说：

```text
CreateAttachment / SetVPMatrix
  -> 资源表

BeginRenderPass + Draw* + EndRenderPass
  -> 一个 RenderPass 执行节点

BindAttachmentToEgui / CopyAttachmentToEGui
  -> 输出 Sink 节点

petgraph edge
  -> A 必须在 B 之前执行
```

这样最贴合你当前的命令结构。

**图入口**
不要手写 root。算法是：

1. 所有 `BindAttachmentToEgui / CopyAttachmentToEGui` 是最终输出。
2. 从这些 sink 反向遍历依赖，得到 live nodes。
3. live nodes 中没有 live incoming edge 的节点，就是图入口。
4. 用 `petgraph::algo::toposort` 得到执行顺序。

下面是完整目标实现草稿：

```rust
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
};

use petgraph::{
    algo::toposort,
    graph::{DiGraph, NodeIndex},
    Direction,
};
use tokio::sync::mpsc::Sender;

use crate::{
    graphics::{attachment::Attachment, mesh::Mesh},
    math::float4x4,
};

pub type AttachmentId = usize;
pub type VpMatrixId = usize;
pub type RenderPassId = usize;

enum GraphNode {
    CreateAttachment(CreateAttachmentNode),
    SetVPMatrix(SetVPMatrixNode),
    BeginRenderPass(BeginRenderPassNode),
    EndRenderPass(EndRenderPassNode),
    Draw(DrawNode),
    BindAttachmentToEgui(BindAttachmentToEguiNode),
    CopyAttachmentToEGui(CopyAttachmentToEguiNode),
}

struct CreateAttachmentNode {
    attachment: Attachment,
    id: usize,
}

struct SetVPMatrixNode {
    matrix: float4x4,
    id: usize,
}

struct BeginRenderPassNode {
    id: usize,
    attachments: Vec<usize>,
    vp_id: usize,
    force_clear: bool,
}

struct EndRenderPassNode {
    id: usize,
}

struct DrawNode {
    mesh: Mesh,
    render_pass_id: usize,
}

struct BindAttachmentToEguiNode {
    attachment_id: usize,
    sender: Sender<egui::TextureId>,
}

struct CopyAttachmentToEguiNode {
    attachment_id: usize,
    egui_texture_id: egui::TextureId,
}

pub struct GraphicsCommand {
    nodes: Vec<GraphNode>,
    attachment_count: usize,
    vp_buffer_count: usize,
    render_pass_count: usize,
    cur_render_pass_id: Option<usize>,
}

impl GraphicsCommand {
    pub fn new(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            attachment_count: 0,
            vp_buffer_count: 0,
            render_pass_count: 0,
            cur_render_pass_id: None,
        }
    }

    pub fn create_attachment(&mut self, attachment: Attachment) -> AttachmentId {
        let id = self.attachment_count;
        self.attachment_count += 1;
        self.nodes.push(GraphNode::CreateAttachment(CreateAttachmentNode {
            attachment,
            id,
        }));
        id
    }

    pub fn set_view_projection_matrix(&mut self, matrix: float4x4) -> VpMatrixId {
        let id = self.vp_buffer_count;
        self.vp_buffer_count += 1;
        self.nodes.push(GraphNode::SetVPMatrix(SetVPMatrixNode {
            matrix,
            id,
        }));
        id
    }

    pub fn begin_render_pass(
        &mut self,
        attachments: Vec<AttachmentId>,
        vp_id: VpMatrixId,
        force_clear: bool,
    ) {
        debug_assert!(self.cur_render_pass_id.is_none());

        if self.cur_render_pass_id.is_some() {
            return;
        }

        let id = self.render_pass_count;
        self.render_pass_count += 1;

        self.nodes.push(GraphNode::BeginRenderPass(BeginRenderPassNode {
            id,
            attachments,
            vp_id,
            force_clear,
        }));

        self.cur_render_pass_id = Some(id);
    }

    pub fn end_render_pass(&mut self) {
        debug_assert!(self.cur_render_pass_id.is_some());

        let Some(id) = self.cur_render_pass_id.take() else {
            return;
        };

        self.nodes
            .push(GraphNode::EndRenderPass(EndRenderPassNode { id }));
    }

    pub fn draw(&mut self, mesh: Mesh) {
        debug_assert!(self.cur_render_pass_id.is_some());

        let Some(render_pass_id) = self.cur_render_pass_id else {
            return;
        };

        self.nodes.push(GraphNode::Draw(DrawNode {
            mesh,
            render_pass_id,
        }));
    }

    pub fn bind_attachment_to_egui(
        &mut self,
        attachment_id: AttachmentId,
        sender: Sender<egui::TextureId>,
    ) {
        self.nodes
            .push(GraphNode::BindAttachmentToEgui(BindAttachmentToEguiNode {
                attachment_id,
                sender,
            }));
    }

    pub fn copy_attachment_to_egui(
        &mut self,
        attachment_id: AttachmentId,
        egui_texture_id: egui::TextureId,
    ) {
        self.nodes
            .push(GraphNode::CopyAttachmentToEGui(CopyAttachmentToEguiNode {
                attachment_id,
                egui_texture_id,
            }));
    }
}

pub struct GraphicsGraph {
    attachments: Vec<Attachment>,
    vp_matrices: Vec<float4x4>,
    graph: DiGraph<GraphExecNode, DependencyEdge>,
    sinks: Vec<NodeIndex>,
    entries: Vec<NodeIndex>,
    execution_order: Vec<NodeIndex>,
}

pub struct CompiledRenderPass {
    pub id: RenderPassId,
    pub attachments: Vec<AttachmentId>,
    pub vp_id: VpMatrixId,
    pub force_clear: bool,
    pub draws: Vec<DrawOp>,
    record_order: usize,
}

pub struct DrawOp {
    pub mesh: Mesh,
}

pub enum GraphSink {
    BindAttachmentToEgui {
        attachment_id: AttachmentId,
        sender: Sender<egui::TextureId>,
    },
    CopyAttachmentToEgui {
        attachment_id: AttachmentId,
        egui_texture_id: egui::TextureId,
    },
}

enum GraphExecNode {
    RenderPass(CompiledRenderPass),
    Sink(GraphSinkNode),
}

struct GraphSinkNode {
    sink: GraphSink,
    record_order: usize,
}

#[derive(Clone, Copy, Debug)]
enum DependencyEdge {
    AttachmentAccess { attachment_id: AttachmentId },
}

struct OpenPass {
    local_id: usize,
    pass: CompiledRenderPass,
}

#[derive(Default)]
struct CommandRemap {
    attachments: HashMap<usize, AttachmentId>,
    vp_matrices: HashMap<usize, VpMatrixId>,
}

#[derive(Debug)]
pub enum GraphicsGraphError {
    EmptyCommands,
    DuplicateAttachmentId { command: usize, id: usize },
    DuplicateVpId { command: usize, id: usize },
    InvalidAttachmentId { command: usize, id: usize },
    InvalidVpId { command: usize, id: usize },
    NestedRenderPass { command: usize },
    DrawOutsideRenderPass { command: usize },
    DrawPassMismatch { command: usize },
    EndWithoutBegin { command: usize },
    EndPassMismatch { command: usize },
    UnclosedRenderPass { command: usize },
    MissingOutput,
    MissingWriterForSink { attachment_id: AttachmentId },
    Cycle { node: NodeIndex },
}

impl fmt::Display for GraphicsGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for GraphicsGraphError {}

impl GraphicsGraph {
    pub fn from_commands(commands: Vec<GraphicsCommand>) -> Result<Self, GraphicsGraphError> {
        if commands.is_empty() {
            return Err(GraphicsGraphError::EmptyCommands);
        }

        let mut builder = GraphicsGraphBuilder::new();

        for (command_index, command) in commands.into_iter().enumerate() {
            builder.ingest_command(command_index, command)?;
        }

        builder.finish()
    }

    pub fn execution_order(&self) -> impl Iterator<Item = &GraphExecNode> {
        self.execution_order.iter().map(|node| &self.graph[*node])
    }

    pub fn entries(&self) -> &[NodeIndex] {
        &self.entries
    }

    pub fn execute<R>(&self, runner: &mut R) -> Result<(), R::Error>
    where
        R: GraphicsGraphRunner,
    {
        for (id, attachment) in self.attachments.iter().enumerate() {
            runner.ensure_attachment(id, attachment)?;
        }

        for (id, matrix) in self.vp_matrices.iter().enumerate() {
            runner.ensure_vp_matrix(id, matrix)?;
        }

        for node in &self.execution_order {
            match &self.graph[*node] {
                GraphExecNode::RenderPass(pass) => {
                    runner.execute_render_pass(pass)?;
                }
                GraphExecNode::Sink(sink) => match &sink.sink {
                    GraphSink::BindAttachmentToEgui {
                        attachment_id,
                        sender,
                    } => {
                        runner.bind_attachment_to_egui(*attachment_id, sender)?;
                    }
                    GraphSink::CopyAttachmentToEgui {
                        attachment_id,
                        egui_texture_id,
                    } => {
                        runner.copy_attachment_to_egui(*attachment_id, *egui_texture_id)?;
                    }
                },
            }
        }

        Ok(())
    }
}

pub trait GraphicsGraphRunner {
    type Error;

    fn ensure_attachment(
        &mut self,
        id: AttachmentId,
        attachment: &Attachment,
    ) -> Result<(), Self::Error>;

    fn ensure_vp_matrix(
        &mut self,
        id: VpMatrixId,
        matrix: &float4x4,
    ) -> Result<(), Self::Error>;

    fn execute_render_pass(&mut self, pass: &CompiledRenderPass) -> Result<(), Self::Error>;

    fn bind_attachment_to_egui(
        &mut self,
        attachment_id: AttachmentId,
        sender: &Sender<egui::TextureId>,
    ) -> Result<(), Self::Error>;

    fn copy_attachment_to_egui(
        &mut self,
        attachment_id: AttachmentId,
        egui_texture_id: egui::TextureId,
    ) -> Result<(), Self::Error>;
}

struct GraphicsGraphBuilder {
    attachments: Vec<Attachment>,
    vp_matrices: Vec<float4x4>,
    graph: DiGraph<GraphExecNode, DependencyEdge>,
    sinks: Vec<NodeIndex>,
    last_access: HashMap<AttachmentId, NodeIndex>,
    last_writer: HashMap<AttachmentId, NodeIndex>,
    next_pass_id: usize,
    next_record_order: usize,
}

impl GraphicsGraphBuilder {
    fn new() -> Self {
        Self {
            attachments: Vec::new(),
            vp_matrices: Vec::new(),
            graph: DiGraph::new(),
            sinks: Vec::new(),
            last_access: HashMap::new(),
            last_writer: HashMap::new(),
            next_pass_id: 0,
            next_record_order: 0,
        }
    }

    fn ingest_command(
        &mut self,
        command_index: usize,
        command: GraphicsCommand,
    ) -> Result<(), GraphicsGraphError> {
        let mut remap = CommandRemap::default();
        let mut open_pass: Option<OpenPass> = None;

        for node in command.nodes {
            match node {
                GraphNode::CreateAttachment(node) => {
                    if remap.attachments.contains_key(&node.id) {
                        return Err(GraphicsGraphError::DuplicateAttachmentId {
                            command: command_index,
                            id: node.id,
                        });
                    }

                    let global_id = self.attachments.len();
                    self.attachments.push(node.attachment);
                    remap.attachments.insert(node.id, global_id);
                }

                GraphNode::SetVPMatrix(node) => {
                    if remap.vp_matrices.contains_key(&node.id) {
                        return Err(GraphicsGraphError::DuplicateVpId {
                            command: command_index,
                            id: node.id,
                        });
                    }

                    let global_id = self.vp_matrices.len();
                    self.vp_matrices.push(node.matrix);
                    remap.vp_matrices.insert(node.id, global_id);
                }

                GraphNode::BeginRenderPass(node) => {
                    if open_pass.is_some() {
                        return Err(GraphicsGraphError::NestedRenderPass {
                            command: command_index,
                        });
                    }

                    let mut attachments = Vec::with_capacity(node.attachments.len());
                    for local_id in node.attachments {
                        let Some(&global_id) = remap.attachments.get(&local_id) else {
                            return Err(GraphicsGraphError::InvalidAttachmentId {
                                command: command_index,
                                id: local_id,
                            });
                        };
                        attachments.push(global_id);
                    }

                    let Some(&vp_id) = remap.vp_matrices.get(&node.vp_id) else {
                        return Err(GraphicsGraphError::InvalidVpId {
                            command: command_index,
                            id: node.vp_id,
                        });
                    };

                    let pass = CompiledRenderPass {
                        id: self.next_pass_id,
                        attachments,
                        vp_id,
                        force_clear: node.force_clear,
                        draws: Vec::new(),
                        record_order: self.alloc_record_order(),
                    };

                    self.next_pass_id += 1;

                    open_pass = Some(OpenPass {
                        local_id: node.id,
                        pass,
                    });
                }

                GraphNode::Draw(node) => {
                    let Some(open_pass) = open_pass.as_mut() else {
                        return Err(GraphicsGraphError::DrawOutsideRenderPass {
                            command: command_index,
                        });
                    };

                    if open_pass.local_id != node.render_pass_id {
                        return Err(GraphicsGraphError::DrawPassMismatch {
                            command: command_index,
                        });
                    }

                    open_pass.pass.draws.push(DrawOp { mesh: node.mesh });
                }

                GraphNode::EndRenderPass(node) => {
                    let Some(open_pass) = open_pass.take() else {
                        return Err(GraphicsGraphError::EndWithoutBegin {
                            command: command_index,
                        });
                    };

                    if open_pass.local_id != node.id {
                        return Err(GraphicsGraphError::EndPassMismatch {
                            command: command_index,
                        });
                    }

                    self.add_render_pass(open_pass.pass);
                }

                GraphNode::BindAttachmentToEgui(node) => {
                    let Some(&attachment_id) = remap.attachments.get(&node.attachment_id) else {
                        return Err(GraphicsGraphError::InvalidAttachmentId {
                            command: command_index,
                            id: node.attachment_id,
                        });
                    };

                    self.add_sink(GraphSink::BindAttachmentToEgui {
                        attachment_id,
                        sender: node.sender,
                    })?;
                }

                GraphNode::CopyAttachmentToEGui(node) => {
                    let Some(&attachment_id) = remap.attachments.get(&node.attachment_id) else {
                        return Err(GraphicsGraphError::InvalidAttachmentId {
                            command: command_index,
                            id: node.attachment_id,
                        });
                    };

                    self.add_sink(GraphSink::CopyAttachmentToEgui {
                        attachment_id,
                        egui_texture_id: node.egui_texture_id,
                    })?;
                }
            }
        }

        if open_pass.is_some() {
            return Err(GraphicsGraphError::UnclosedRenderPass {
                command: command_index,
            });
        }

        Ok(())
    }

    fn add_render_pass(&mut self, pass: CompiledRenderPass) {
        let attachments = pass.attachments.clone();
        let node = self.graph.add_node(GraphExecNode::RenderPass(pass));

        for attachment_id in attachments {
            if let Some(previous) = self.last_access.insert(attachment_id, node) {
                self.graph.add_edge(
                    previous,
                    node,
                    DependencyEdge::AttachmentAccess { attachment_id },
                );
            }

            self.last_writer.insert(attachment_id, node);
        }
    }

    fn add_sink(&mut self, sink: GraphSink) -> Result<(), GraphicsGraphError> {
        let attachment_id = match &sink {
            GraphSink::BindAttachmentToEgui { attachment_id, .. } => *attachment_id,
            GraphSink::CopyAttachmentToEgui { attachment_id, .. } => *attachment_id,
        };

        let Some(writer) = self.last_writer.get(&attachment_id).copied() else {
            return Err(GraphicsGraphError::MissingWriterForSink { attachment_id });
        };

        let node = self.graph.add_node(GraphExecNode::Sink(GraphSinkNode {
            sink,
            record_order: self.alloc_record_order(),
        }));

        self.graph.add_edge(
            writer,
            node,
            DependencyEdge::AttachmentAccess { attachment_id },
        );

        self.last_access.insert(attachment_id, node);
        self.sinks.push(node);

        Ok(())
    }

    fn finish(self) -> Result<GraphicsGraph, GraphicsGraphError> {
        if self.sinks.is_empty() {
            return Err(GraphicsGraphError::MissingOutput);
        }

        let live_nodes = collect_live_nodes(&self.graph, &self.sinks);
        let sorted = toposort(&self.graph, None)
            .map_err(|cycle| GraphicsGraphError::Cycle {
                node: cycle.node_id(),
            })?;

        let execution_order = sorted
            .into_iter()
            .filter(|node| live_nodes.contains(node))
            .collect::<Vec<_>>();

        let mut entries = live_nodes
            .iter()
            .copied()
            .filter(|node| {
                self.graph
                    .neighbors_directed(*node, Direction::Incoming)
                    .all(|parent| !live_nodes.contains(&parent))
            })
            .collect::<Vec<_>>();

        entries.sort_by_key(|node| record_order(&self.graph[*node]));

        Ok(GraphicsGraph {
            attachments: self.attachments,
            vp_matrices: self.vp_matrices,
            graph: self.graph,
            sinks: self.sinks,
            entries,
            execution_order,
        })
    }

    fn alloc_record_order(&mut self) -> usize {
        let order = self.next_record_order;
        self.next_record_order += 1;
        order
    }
}

fn collect_live_nodes(
    graph: &DiGraph<GraphExecNode, DependencyEdge>,
    sinks: &[NodeIndex],
) -> HashSet<NodeIndex> {
    let mut live = HashSet::new();
    let mut stack = sinks.to_vec();

    while let Some(node) = stack.pop() {
        if !live.insert(node) {
            continue;
        }

        for parent in graph.neighbors_directed(node, Direction::Incoming) {
            stack.push(parent);
        }
    }

    live
}

fn record_order(node: &GraphExecNode) -> usize {
    match node {
        GraphExecNode::RenderPass(pass) => pass.record_order,
        GraphExecNode::Sink(sink) => sink.record_order,
    }
}
```

**这套代码怎么跑你的注释例子**

你的命令流会被编译成：

```text
resources:
  Attachment[0] = frame_buffer
  VP[0] = camera matrix

petgraph nodes:
  NodeA = RenderPass {
    writes: Attachment[0],
    reads: VP[0],
    draws: [mesh],
  }

  NodeB = BindAttachmentToEgui {
    reads: Attachment[0],
  }

petgraph edges:
  NodeA -> NodeB
```

`toposort` 结果就是：

```text
RenderPass -> BindAttachmentToEgui
```

`entries` 是：

```text
[RenderPass]
```

也就是说，入口不是你手动指定的 `root_node`，而是由“最终输出 sink 反向裁剪 + 入度判断”自动得到。

**为什么不用资源节点**

你可能会想做成：

```text
CreateAttachment -> BeginRenderPass -> Draw -> EndRenderPass -> BindEgui
```

但这在 render graph 里不够好。因为 `BeginRenderPass / EndRenderPass` 不是独立调度单元，wgpu 的 `RenderPass<'_>` 生命周期必须被限制在一次 execute 调用里。正确形状是：

```text
AttachmentDesc
VpDesc
MeshDraw

合成一个 RenderPassNode
```

然后 graph 只调度：

```text
RenderPassNode -> SinkNode
```

这会让后面扩展后处理、picking、shadow 都简单很多。下一步真正接入 `wgpu` 时，只要实现 `GraphicsGraphRunner`，把 `execute_render_pass` 里转成 `encoder.begin_render_pass(...) + draw mesh` 就行。

## 对话 7：保存 petgraph 后续对话到 docs/ai

### User

```text
# Context from my IDE setup:

## Active file: src/graphics/graphics_graph.rs

## Open tabs:
- graphics_graph.rs: src/graphics/graphics_graph.rs
- render_pipeline.rs: src/graphics/render_pipeline.rs
- runtime.rs: src/kairos_editor/runtime.rs
- mesh.rs: src/graphics/mesh.rs
- graphics.rs: src/graphics.rs

## My request for Codex:
把我们的对完完整的保存入docs/ai目录下
```

### Assistant 过程消息

```text
我先按你们项目里已有的 `docs/ai` 归档规则看一下 README 和相关 handoff 习惯。因为你说的是“完整保存”，我会把计划落成“新增一份对话归档 + 更新索引”的形式，但当前仍处在 Plan Mode，我不会直接写文件。
```

```text
我找到了现有目标文档：`docs/ai/graphics-graph-render-graph-conversation.md` 已经是同一主题的完整对话归档，README 也已有索引。最干净的保存方式是追加本轮关于 `runtime.rs` 注释、图入口、执行顺序、以及 `petgraph` 版本实现的后续对话，而不是再开一个重复主题的新文件。
```

### Assistant

```text
<proposed_plan>
# 保存 GraphicsGraph / petgraph 后续对话

## Summary

将本轮关于 `GraphicsCommand`、`GraphicsGraph`、图入口、执行顺序，以及基于 `petgraph` 的完整实现方案，追加保存到现有文档 `docs/ai/graphics-graph-render-graph-conversation.md`。该文档已经是同主题的完整对话归档，因此不新建重复文件。

## Key Changes

- 在 `docs/ai/graphics-graph-render-graph-conversation.md` 末尾追加后续对话：
  - `对话 4`：从 `runtime.rs` 注释命令流出发，解释如何构建完整 `GraphicsGraph`、确定入口、拓扑排序与执行。
  - `对话 5`：不改代码前提下，给出非 `petgraph` 版本的详细算法讲解和代码草案。
  - `对话 6`：基于项目已有 `petgraph = "0.8.3"`，给出 `DiGraph + toposort` 的完整设计与代码实现。
  - `对话 7`：记录本次“保存到 `docs/ai`”请求。
- 保持现有归档风格：中文正文、按 User / Assistant 结构保存可见对话内容，省略终端工具原始输出，只把关键发现写入过程说明或正文。
- 更新 `docs/ai/README.md` 现有索引行，将说明扩展为包含 `petgraph` 版本的 GraphicsGraph 编译/执行方案，避免未来检索时漏掉这次补充。

## Test Plan

- 检查新增内容只追加到目标文档尾部，未重写旧对话。
- 检查 README 中 `graphics-graph-render-graph-conversation.md` 链接仍然有效。
- 运行 `git diff -- docs/ai/graphics-graph-render-graph-conversation.md docs/ai/README.md`，确认改动范围只有本次归档和索引描述。
- 文档变更不需要跑 `cargo check`。

## Assumptions

- 目标是“完整保留当前这轮可见对话”，不是整理成精简设计文档。
- 使用现有 `graphics-graph-render-graph-conversation.md` 追加，而不是新建 `petgraph-graphics-graph.md`。
- README 只更新同一行描述，不新增重复索引项。
</proposed_plan>
```

### User

```text
# Context from my IDE setup:

## Active file: src/graphics/graphics_graph.rs

## Open tabs:
- graphics_graph.rs: src/graphics/graphics_graph.rs
- render_pipeline.rs: src/graphics/render_pipeline.rs
- runtime.rs: src/kairos_editor/runtime.rs
- mesh.rs: src/graphics/mesh.rs
- graphics.rs: src/graphics.rs

## My request for Codex:
PLEASE IMPLEMENT THIS PLAN:
# 保存 GraphicsGraph / petgraph 后续对话

## Summary

将本轮关于 `GraphicsCommand`、`GraphicsGraph`、图入口、执行顺序，以及基于 `petgraph` 的完整实现方案，追加保存到现有文档 `docs/ai/graphics-graph-render-graph-conversation.md`。该文档已经是同主题的完整对话归档，因此不新建重复文件。

## Key Changes

- 在 `docs/ai/graphics-graph-render-graph-conversation.md` 末尾追加后续对话：
  - `对话 4`：从 `runtime.rs` 注释命令流出发，解释如何构建完整 `GraphicsGraph`、确定入口、拓扑排序与执行。
  - `对话 5`：不改代码前提下，给出非 `petgraph` 版本的详细算法讲解和代码草案。
  - `对话 6`：基于项目已有 `petgraph = "0.8.3"`，给出 `DiGraph + toposort` 的完整设计与代码实现。
  - `对话 7`：记录本次“保存到 `docs/ai`”请求。
- 保持现有归档风格：中文正文、按 User / Assistant 结构保存可见对话内容，省略终端工具原始输出，只把关键发现写入过程说明或正文。
- 更新 `docs/ai/README.md` 现有索引行，将说明扩展为包含 `petgraph` 版本的 GraphicsGraph 编译/执行方案，避免未来检索时漏掉这次补充。

## Test Plan

- 检查新增内容只追加到目标文档尾部，未重写旧对话。
- 检查 README 中 `graphics-graph-render-graph-conversation.md` 链接仍然有效。
- 运行 `git diff -- docs/ai/graphics-graph-render-graph-conversation.md docs/ai/README.md`，确认改动范围只有本次归档和索引描述。
- 文档变更不需要跑 `cargo check`。

## Assumptions

- 目标是“完整保留当前这轮可见对话”，不是整理成精简设计文档。
- 使用现有 `graphics-graph-render-graph-conversation.md` 追加，而不是新建 `petgraph-graphics-graph.md`。
- README 只更新同一行描述，不新增重复索引项。
```
