# RenderPipeline 渲染缓存讨论完整记录

> 日期：2026-06-04  
> 状态：完整对话归档，非最终实现规范  
> 范围：围绕 `src/graphics/render_pipeline.rs` 当前渲染路径，判断哪些数据应该缓存，哪些对象应保持每帧临时创建。  
> 说明：本文按对话顺序保留本轮讨论中用户与 AI 的可见内容，并补充代码路径依据。中间的终端检索和文件读取输出没有作为对话正文展开；它们只作为当时回答的依据。

## 对话 1：当前渲染中哪些数据需要缓存

### User

```text
# Context from my IDE setup:

## Active file: src/graphics/render_pipeline.rs

## Open tabs:
- render_pipeline.rs: src/graphics/render_pipeline.rs
- shader.wgsl: res/shaders/shader.wgsl
- shader.rs: src/asset_loader/assets/asset/shader.rs
- shader.rs: src/graphics/shader.rs
- scene_window.rs: src/kairos_editor/ui/scene_window.rs

## My request for Codex:
在我当前的render_pipeline.rs的渲染中，哪些数据应该是需要被缓存的？
```

### Assistant 过程消息

```text
我先看一下当前 `render_pipeline.rs` 的真实渲染路径，再结合相关 shader/资源结构来回答；这个问题很容易被泛泛地说成“缓存 pipeline 和 bind group”，但具体该缓存哪些，要看你现在每帧到底在重建什么。
```

```text
我已经看到几个明显的“每帧重建”点：attachment texture、VP uniform/bind group/layout、mesh buffer、texture sampler/bind group/layout、shader module、pipeline layout、render pipeline 都在 `present` 里沿图遍历时创建。接下来我把后半段也读完，确认这些对象是按 pass、按 renderer 还是按 draw item 重复创建。
```

```text
这里的 `GraphicsGraph` 是每帧构建的命令图，所以“graph 节点本身”和 `RenderPassColorAttachment` 这种临时描述对象不一定要缓存；真正值得缓存的是 GPU 资源和由资源组合推导出来的 pipeline 状态。我要再看一下 graph/command 的类型，避免把短生命周期命令对象误归类。
```

```text
还有一个边界点：`egui_renderer.register_native_texture` 不是普通 draw 资源，它把一个 wgpu texture view 暴露给 egui。这个要不要缓存取决于 SceneWindow 是不是只注册一次、resize 时重建一次；我顺手核对一下调用方式。
```

### Assistant

```text
就你现在这份 render_pipeline.rs 来看，最该缓存的是这些：

必须优先缓存

1. Mesh 的 GPU buffer

   现在每个 draw 都在创建 `vertex_buffer` 和 `indices_buffer`：
   render_pipeline.rs:577、render_pipeline.rs:619

   应该按 `mesh asset handle` 缓存：
   `vertex_buffer`、`index_buffer`、`index_count`。

2. Texture 的 GPU 资源

   现在每个 draw 都 `create_texture`、`queue.write_texture`、`create_view`、`create_sampler`、`create_bind_group`：
   render_pipeline.rs:481

   应该按 `texture asset handle` 缓存：
   `wgpu::Texture`、`TextureView`、`BindGroup`。
   `Sampler` 如果参数固定，可以全局只建一个。

3. ShaderModule

   现在每个 draw 都重新编译 WGSL：
   render_pipeline.rs:569

   应该按 `shader asset handle` 或 shader 源码 hash 缓存 `ShaderModule`。

4. RenderPipeline / PipelineLayout / BindGroupLayout

   现在每个 draw 都创建：
   `texture_bind_group_layout`、`pipeline_layout`、`render_pipeline`：
   render_pipeline.rs:521、render_pipeline.rs:558、render_pipeline.rs:632

   这几个很应该缓存。`RenderPipeline` 的 key 至少包含：
   shader、color attachment format、depth format/depth state、vertex layout、bind group layouts、blend、cull、topology、multisample。

也应该缓存，但内容要每帧更新

5. VP uniform buffer / bind group

   现在每个 VP 矩阵都重新创建 buffer、layout、bind group：
   render_pipeline.rs:243

   `BindGroupLayout` 应该全局缓存。
   `VP Buffer + BindGroup` 可以缓存，矩阵变了就 `queue.write_buffer` 更新内容。

6. Instance buffer 的分配

   现在每个 instanced draw 都创建 `instancing_buffer`：
   render_pipeline.rs:626

   `local_to_worlds` 本身如果每帧变化，不该缓存值；但 buffer 分配可以缓存或做动态 ring buffer。也就是：缓存 GPU buffer 容量，每帧写入当前矩阵。

Render target 也要缓存

7. 非 framebuffer 的 color/depth attachment texture

   现在 SceneWindow 的 color attachment 和 depth attachment 每帧都重新创建 texture：
   render_pipeline.rs:174、render_pipeline.rs:211

   像 SceneWindow 这种固定 `width/height/format` 的离屏渲染目标，应该缓存 `Texture + TextureView`，只在尺寸或格式变化时重建。

   但 `surface.get_current_texture()` 拿到的窗口 framebuffer 不该缓存，它是每帧 swapchain 给你的当前帧纹理。

不需要缓存

- `CommandEncoder`：每帧创建、提交后就结束。
- `RenderPass` / `RenderPassDescriptor`：每帧临时对象。
- `GraphicsGraph` / `GraphicsCommand`：这是你每帧生成的渲染命令图，可以保持临时。
- `RenderPassColorAttachment` / `RenderPassDepthStencilAttachment`：只是包装 view、load/store 操作的临时描述。
- `egui` 的 `paint_jobs`、`screen_descriptor`、`update_buffers` 结果：这些本来就是每帧 UI 输出。

一句话总结：缓存 GPU 常驻资源和昂贵状态对象，不缓存每帧命令对象。
你当前最大浪费点是：每个 draw 里反复创建 texture、mesh buffer、shader module、bind group layout、pipeline layout、render pipeline。这里先改，收益会非常明显。
```

## 代码路径依据

本轮回答依据的是当前 `src/graphics/render_pipeline.rs` 和相关图命令结构。以下行号来自 2026-06-04 当时的工作区快照，后续重构后可能变化。

### 每帧 attachment texture

- `present` 中遍历 `graphics_graph.attachments`，对非 internal attachment 调用 `device.create_texture`，再 `create_view`：`src/graphics/render_pipeline.rs:161-189`。
- 深度 attachment 同样每帧 `create_texture`：`src/graphics/render_pipeline.rs:210-226`。
- `SceneWindow` 当前每帧通过 `GraphicsCommand` 声明固定尺寸的 scene color/depth attachment：`src/kairos_editor/ui/scene_window.rs:278-292`。

结论：SceneWindow 这类离屏 render target 应缓存 `Texture + TextureView`，key 至少包含宽、高、format、sample count、usage、用途标识。窗口 framebuffer 来自 swapchain，不应缓存。

### 每帧 VP uniform 与 bind group

- `present` 中遍历 `graphics_graph.vps`，每个 VP 都创建 `VP Buffer`、`VP Buffer Bind Group Layout`、`VP Buffer Bind Group`：`src/graphics/render_pipeline.rs:243-279`。

结论：VP 的矩阵值可以每帧变化，但 bind group layout 不该每帧创建。可以缓存 `VP Buffer + BindGroup`，每帧用 `queue.write_buffer` 写入新矩阵。

### 每个 draw 的 texture GPU upload

- 对材质 texture 读取 CPU 侧 `texture.data` 后，每个 draw 创建 `wgpu::Texture`，并执行 `queue.write_texture`：`src/graphics/render_pipeline.rs:471-505`。
- 随后每个 draw 又创建 `TextureView`、`Sampler`、`Texture Bind Group Layout`、`Texture Bind Group`：`src/graphics/render_pipeline.rs:506-556`。

结论：CPU 侧 texture asset 加载完成后，应在 GPU 资源层按 texture handle 懒上传一次，缓存 `Texture/View/BindGroup`。固定参数 sampler 可全局共享。

### 每个 draw 的 shader 与 pipeline 状态

- 每个 draw 创建 `PipelineLayout`：`src/graphics/render_pipeline.rs:558-565`。
- 每个 draw 创建 `ShaderModule`：`src/graphics/render_pipeline.rs:569-572`。
- 每个 draw 创建 `RenderPipeline`：`src/graphics/render_pipeline.rs:632-676`。

结论：`ShaderModule` 应按 shader handle 或源码 hash 缓存；`RenderPipeline` 应按完整 pipeline key 缓存。pipeline key 至少包含 shader、入口函数、color/depth format、depth state、vertex/instance layout、bind group layouts、primitive state、blend、multisample。

### 每个 draw 的 mesh GPU buffer

- 每个 draw 从 mesh asset 取 CPU vertices/indices，再创建 vertex/index buffer：`src/graphics/render_pipeline.rs:574-623`。

结论：mesh 的 GPU buffer 是典型常驻资源，应按 mesh handle 缓存 `vertex_buffer`、`index_buffer`、`index_count`。mesh 资源变更或热重载时再失效重建。

### 每个 draw 的 instance buffer

- `GraphicsGraph::optimize_nodes` 会把相同 mesh/material 的 draw 合并到 `InstancingDraw.local_to_worlds`。
- 但执行时每个 instanced draw 仍创建新的 instance buffer：`src/graphics/render_pipeline.rs:626-630`。

结论：instance 矩阵内容通常每帧变化，不缓存矩阵值；但可以缓存或池化 buffer 分配。第一版可按 draw item 创建，后续用 per-frame ring buffer 或 staging/streaming buffer 优化。

## 推荐缓存分层

### 常驻 GPU 资产缓存

适合放入 `RenderAssets` 或类似 GPU 资产层：

```text
MeshHandle -> GpuMesh {
  vertex_buffer,
  index_buffer,
  index_count,
}

TextureHandle -> GpuTexture {
  texture,
  view,
  bind_group,
}

ShaderHandle -> ShaderModule
```

这些资源的生命周期跟资产加载、卸载、热重载相关，不跟单帧 `GraphicsGraph` 绑定。

### Pipeline 状态缓存

适合放在 renderer / pipeline cache 中：

```text
PipelineKey {
  shader,
  vertex_layout_id,
  instance_layout_id,
  color_formats,
  depth_format,
  depth_state,
  primitive_state,
  blend_state,
  multisample,
  bind_group_layout_ids,
} -> wgpu::RenderPipeline
```

`BindGroupLayout`、`PipelineLayout` 也应缓存。当前 shader 的 group 约定是：

```text
group(0): texture_2d + sampler
group(1): VP uniform buffer
```

因此第一版可以先固定两个 bind group layout；等材质系统扩展后再做 layout 反射或材质模板。

### Frame / View 缓存

适合缓存“分配”，每帧更新“内容”：

```text
SceneViewId + size + format -> CachedRenderTarget {
  color_texture,
  color_view,
  depth_texture,
  depth_view,
  egui_texture_id,
}

ViewId -> VpUniform {
  buffer,
  bind_group,
}
```

SceneWindow resize 或 format 改变时重建 render target，并同步释放旧的 egui texture id。

### 每帧临时对象

这些不应做长期缓存：

```text
CommandEncoder
RenderPass
RenderPassDescriptor
RenderPassColorAttachment
RenderPassDepthStencilAttachment
GraphicsCommand
GraphicsGraph
egui paint_jobs / screen_descriptor
```

它们是“本帧命令描述”和“录制过程对象”，提交后自然结束。

## 建议落地顺序

1. 先做 `GpuMesh` 缓存，消掉每个 draw 的 vertex/index buffer 创建。
2. 再做 `GpuTexture` 缓存，消掉每个 draw 的 texture 创建和 `queue.write_texture`。
3. 建固定的 texture bind group layout、VP bind group layout、sampler。
4. 缓存 shader module。
5. 用 `PipelineKey` 缓存 render pipeline。
6. 缓存 SceneWindow 的 offscreen color/depth target，只在尺寸变化时重建。
7. 最后再优化 VP buffer 和 instance buffer 的分配策略。

这个顺序的好处是：先解决最明显的重复 GPU 资源创建，再处理 pipeline 状态，最后优化每帧动态数据写入。

## 对话 2：保存本轮记录

### User

```text
# Context from my IDE setup:

## Active file: src/kairos_editor/runtime.rs

## Open tabs:
- runtime.rs: src/kairos_editor/runtime.rs
- render_pipeline.rs: src/graphics/render_pipeline.rs
- shader.wgsl: res/shaders/shader.wgsl
- shader.rs: src/asset_loader/assets/asset/shader.rs
- shader.rs: src/graphics/shader.rs

## My request for Codex:
将这轮对话完整的保存入 docs/ai 目录中
```

### Assistant 处理结果

本请求创建了当前文档，并在 `docs/ai/README.md` 中补充索引入口。保存时没有修改 `src/graphics/render_pipeline.rs` 或 `src/kairos_editor/runtime.rs` 中已有的工作区改动。
