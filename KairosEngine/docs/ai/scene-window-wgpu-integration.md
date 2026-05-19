# SceneWindow wgpu 集成方案

> 来源：AI 辅助设计讨论（2026-05）  
> 状态：设计草稿，未在代码库中实现

## 1. 背景

当前编辑器使用 `eframe + egui` 构建 UI，并已经在 `Cargo.toml` 中启用了 `eframe` 的 `wgpu` feature。`SceneWindow` 已作为一个 Dock Tab 存在，但目前只显示占位文本：

```rust
ui.label("TODO: Scene");
```

目标是在 `SceneWindow` Tab 内显示由 `wgpu` 渲染出来的场景视图。

## 2. 核心结论

在当前架构下，最稳妥的短期方案是：

1. 继续使用 `eframe` 管理窗口、事件循环和 egui 渲染。
2. 复用 `eframe` 已经创建好的 `wgpu::Device`、`wgpu::Queue` 和 `egui_wgpu::Renderer`。
3. 不在 `SceneWindow` 内创建新的 `wgpu::Surface`。
4. 将场景渲染到离屏 `wgpu::Texture`。
5. 把离屏纹理注册为 `egui::TextureId`。
6. 在 `SceneWindow` 的 Tab 内容区域中用 egui 绘制该纹理。

长期如果要把 KairosEngine 做成更完整的引擎编辑器，也可以丢弃 `eframe`，改为自己集成：

```text
winit + wgpu + egui + egui-winit + egui-wgpu
```

但这不是简单替换 crate，而是要自己接管 `eframe` 当前代管的整层运行时。

## 3. 为什么不要在 Tab 内创建 Surface

`wgpu::Surface` 对应的是一个平台窗口或平台绘制目标。当前应用窗口已经由 `eframe` 创建并绑定了一个 Surface。

`SceneWindow` Tab 只是 egui 布局中的一个矩形区域，不是独立 OS 窗口。因此 Tab 内显示 3D 场景时，推荐使用以下路径：

```text
SceneWindow rect
    -> 计算物理像素尺寸
    -> 渲染到离屏 Texture
    -> Texture 注册到 egui_wgpu
    -> egui 在 rect 内显示 Texture
```

这样可以避免多 Surface 管理、窗口生命周期、DPI、resize 和平台句柄等复杂问题。

## 4. 当前依赖需要注意的问题

当前 `Cargo.toml` 中同时存在：

```toml
wgpu = "28.0"
eframe = { version = "0.33.3", features = ["wgpu"] }
```

`eframe 0.33.3` 内部使用的是它自己的 `egui-wgpu` 与 `wgpu` 版本。如果项目直接依赖另一版 `wgpu`，容易出现两套 `wgpu::Device`、`wgpu::Texture` 类型不兼容的问题。

短期推荐在所有自定义渲染代码中使用 `eframe` re-export 的类型：

```rust
use eframe::wgpu;
use eframe::egui_wgpu;
```

或者移除直接的 `wgpu = "28.0"`，让项目中的 wgpu 类型统一来自 `eframe` / `egui-wgpu` 当前使用的版本。

## 5. 使用 eframe 的推荐落地方案

### 5.1 显式使用 wgpu renderer

在 `main.rs` 创建 `NativeOptions` 时显式指定：

```rust
let options = eframe::NativeOptions {
    viewport,
    renderer: eframe::Renderer::Wgpu,
    ..Default::default()
};
```

### 5.2 从 CreationContext 获取 RenderState

`eframe::CreationContext` 中包含 `wgpu_render_state`。它提供：

| 字段 | 用途 |
|------|------|
| `device` | 创建纹理、buffer、pipeline |
| `queue` | 上传 uniform、提交命令 |
| `target_format` | eframe 窗口最终渲染格式 |
| `renderer` | `egui_wgpu::Renderer`，用于注册 native texture |

可以在 `KairosEngine::new` 中获取：

```rust
pub fn new(cc: &eframe::CreationContext) -> Result<Self, Box<dyn std::error::Error>> {
    let render_state = cc
        .wgpu_render_state
        .clone()
        .ok_or("eframe is not running with wgpu renderer")?;

    let ui_context = ui::Context::new(render_state);
    let log = Log::new();

    Ok(Self { ui_context, log })
}
```

然后将 `render_state` 继续传入 `SceneWindow`。

### 5.3 让 Drawer 支持可变状态

当前 `Drawer::update` 使用 `&self`：

```rust
fn update(&self, ...);
```

`SceneWindow` 后续需要维护 viewport 尺寸、texture id、camera 状态、GPU target 等数据，更合理的是改成：

```rust
fn update(&mut self, ...);
```

对应地，`KairosTabDrawer` 和 `DockArea` 中的 `drawers` 需要从只读引用改为可变引用。短期也可以使用 `RefCell` 绕过，但长期会让 UI 状态管理更别扭。

### 5.4 SceneWindow 的建议结构

```rust
pub struct SceneWindow {
    model: SceneWindowModel,
    render_state: eframe::egui_wgpu::RenderState,
    viewport: Option<SceneViewportTarget>,
    renderer: SceneViewportRenderer,
}

struct SceneViewportTarget {
    size: [u32; 2],
    color: eframe::wgpu::Texture,
    color_view: eframe::wgpu::TextureView,
    depth: eframe::wgpu::Texture,
    depth_view: eframe::wgpu::TextureView,
    texture_id: eframe::egui::TextureId,
}

struct SceneViewportRenderer {
    // pipeline、camera buffer、bind group、mesh buffers 等
}
```

离屏颜色纹理建议先使用：

```rust
wgpu::TextureFormat::Rgba8Unorm
```

因为 `egui_wgpu 0.33.3` 的 `register_native_texture` 要求 native texture 是 `Rgba8Unorm`。

### 5.5 每帧渲染流程

`SceneWindow::update` 中的核心流程：

```rust
let ui = ui.unwrap();

let available = ui.available_size_before_wrap();
let (rect, response) = ui.allocate_exact_size(
    available,
    egui::Sense::click_and_drag(),
);

let pixels_per_point = ctx.pixels_per_point();
let width = (rect.width() * pixels_per_point).round().max(1.0) as u32;
let height = (rect.height() * pixels_per_point).round().max(1.0) as u32;

self.resize_viewport_if_needed([width, height]);
self.handle_scene_input(ctx, rect, &response);
self.render_scene([width, height]);

ui.painter().image(
    self.viewport.as_ref().unwrap().texture_id,
    rect,
    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
    egui::Color32::WHITE,
);

ctx.request_repaint();
```

### 5.6 resize 时更新 egui texture

第一次创建 viewport 时：

```rust
let texture_id = render_state.renderer.write().register_native_texture(
    &render_state.device,
    &color_view,
    wgpu::FilterMode::Linear,
);
```

尺寸变化重新创建 `color_view` 后：

```rust
render_state.renderer.write().update_egui_texture_from_wgpu_texture(
    &render_state.device,
    &color_view,
    wgpu::FilterMode::Linear,
    texture_id,
);
```

### 5.7 DockArea 中的滚动条问题

当前 Tab 内容区会经过 `ScrollArea`。对 Scene Viewport 来说，滚动条通常是不需要的，并且鼠标滚轮要用于相机缩放或移动。

建议给 `Drawer` 增加：

```rust
fn scroll_bars(&self) -> [bool; 2] {
    [true, true]
}
```

`SceneWindow` 返回：

```rust
fn scroll_bars(&self) -> [bool; 2] {
    [false, false]
}
```

然后在 `KairosTabDrawer` 中转发给具体 drawer。

## 6. SceneViewportRenderer 职责

`SceneViewportRenderer` 不应处理 egui 布局，它只负责 GPU 渲染：

1. 创建 render pipeline。
2. 创建 camera uniform buffer。
3. 创建 mesh/material/light 等 bind group。
4. 每帧更新 camera uniform。
5. 开启离屏 render pass。
6. clear 背景。
7. 绘制网格、模型、选中对象、gizmo 等。
8. 提交 command buffer。

大致接口：

```rust
impl SceneViewportRenderer {
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        size: [u32; 2],
    ) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("scene viewport encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene viewport pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.08,
                            b: 0.09,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            // pass.set_pipeline(...);
            // pass.set_bind_group(...);
            // pass.draw_indexed(...);
        }

        queue.submit(Some(encoder.finish()));
    }
}
```

## 7. PaintCallback 的位置

`egui_wgpu::Callback` 可以把自定义 wgpu 绘制插入 egui 的主 render pass，但它更适合轻量自定义绘制。

对完整 3D Scene Window 来说，离屏纹理方案更稳，因为它允许单独管理：

| 能力 | 离屏 Texture 方案 |
|------|------------------|
| depth buffer | 容易 |
| MSAA | 容易 |
| picking buffer | 容易 |
| HDR / tone mapping | 容易 |
| resize | 可控 |
| 多个 scene viewport | 可控 |

因此建议先用离屏纹理，后续只在有明确需求时再考虑 `PaintCallback`。

## 8. 丢弃 eframe 的可行性

可以丢弃 `eframe`，只使用 `egui` 并接入自己的 `wgpu`。但要明确：`egui` 只是即时模式 UI 库，不负责窗口、事件循环、GPU 初始化和平台输入。

丢弃 `eframe` 后，项目需要自己维护：

| 模块 | 职责 |
|------|------|
| `winit` | 窗口、事件循环、输入事件、DPI、resize |
| `wgpu` | Surface、Adapter、Device、Queue、Swapchain、场景渲染 |
| `egui` | UI 状态、布局、控件 |
| `egui-winit` | 将 winit 输入转换给 egui |
| `egui-wgpu` | 将 egui shapes/textures 渲染到 wgpu render pass |

对应依赖形态：

```toml
egui = "0.33.3"
egui-winit = "0.33.3"
egui-wgpu = "0.33.3"
winit = "0.30"
wgpu = "27.0"
```

版本必须保持对齐，避免 `egui-wgpu` 和引擎渲染器使用不同版本的 `wgpu::Device`。

## 9. 自己集成 egui + wgpu 的帧流程

如果未来移除 `eframe`，主循环大致变成：

```text
winit event loop
    -> egui_winit 处理输入事件
    -> resize 时重配 wgpu surface
    -> begin frame
        -> egui_ctx.run(raw_input, |ctx| draw_editor_ui(ctx))
        -> tessellate egui shapes
        -> 渲染 engine scene / scene viewport
        -> 渲染 egui UI
        -> present surface frame
```

简化伪代码：

```rust
let raw_input = egui_winit_state.take_egui_input(&window);

let full_output = egui_ctx.run(raw_input, |ctx| {
    editor_ui.draw(ctx);
});

egui_winit_state.handle_platform_output(&window, full_output.platform_output);

let paint_jobs = egui_ctx.tessellate(
    full_output.shapes,
    window.scale_factor() as f32,
);

let frame = surface.get_current_texture()?;
let view = frame.texture.create_view(&Default::default());

let mut encoder = device.create_command_encoder(&Default::default());

// 1. render engine scene or scene viewport textures
// 2. update egui textures/buffers
// 3. render egui on top

queue.submit(Some(encoder.finish()));
frame.present();
```

## 10. 迁移建议

短期建议继续使用 `eframe`，先完成：

1. `SceneWindow` 显示离屏 wgpu clear color。
2. 显示一个三角形或 cube。
3. 支持 resize。
4. 支持相机输入。
5. 接入真实 scene/world 数据。

中长期当以下需求出现时，再考虑移除 `eframe`：

1. 引擎需要完全控制主循环。
2. 需要统一管理渲染 frame graph。
3. 需要多窗口或多 viewport 的底层控制。
4. 需要更深的 GPU profiler、swapchain、present mode、帧同步策略。
5. `eframe` 的封装开始限制引擎架构。

结论：当前阶段推荐用 `eframe` 快速打通 SceneWindow；当渲染管线和资源系统成型后，再迁移到 `winit + wgpu + egui-winit + egui-wgpu` 会更稳。
