# eframe Runtime 迁移接力笔记

> 来源：AI 辅助对话整理（2026-05-20）  
> 状态：迁移进行中，当前代码已开始实现自有 runtime  
> 目标：方便在另一台电脑上继续 `eframe -> winit + wgpu + egui-winit + egui-wgpu` 迁移对话

## 1. 本轮对话目标

用户希望剥离 `src/main.rs` 和 `src/kairos_editor.rs` 中最后的 `eframe`，迁移到自有 runtime：

```text
winit + wgpu + egui + egui-winit + egui-wgpu
```

已知背景：

- `docking_tab.rs` 和 `drag_and_drop.rs` 已经转为纯 `egui`。
- `Cargo.toml` 已经引入 `egui-winit`、`egui-wgpu`、`wgpu`、`winit`。
- 长期目标是删除 `eframe` 依赖。
- 本轮重点是分析并设计 `KairosEditorRuntime`。

## 2. 对话中的关键结论

### 2.1 eframe 残留边界

最初残留主要在：

- `src/main.rs`
  - `eframe::run_native`
  - `eframe::NativeOptions`
  - `eframe::icon_data::from_png_bytes`
  - `eframe::{self, egui}`
- `src/kairos_editor.rs`
  - `KairosEngine::new(&eframe::CreationContext)`
  - `impl eframe::App for KairosEngine`
  - `eframe::Frame`
  - `eframe::glow::Context`
- `Cargo.toml`
  - `eframe = { version = "0.33.3", features = ["wgpu"] }`

当前代码中 `KairosEngine` 已经基本纯化为：

```rust
impl KairosEngine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>>;
    fn update(&mut self, ctx: &egui::Context);
    fn on_exit(&mut self);
}
```

### 2.2 runtime 的职责

`KairosEditorRuntime` 应该接管 `eframe` 原本代管的内容：

- 创建 winit window。
- 初始化 `egui::Context`。
- 初始化 `egui_winit::State`，负责把 winit event 翻译为 egui input。
- 初始化 wgpu：
  - `Instance`
  - `Surface`
  - `Adapter`
  - `Device`
  - `Queue`
  - `SurfaceConfiguration`
- 初始化 `egui_wgpu::Renderer`。
- 在 `WindowEvent::RedrawRequested` 中驱动一帧 egui：
  - `take_egui_input`
  - `egui_ctx.run`
  - `handle_platform_output`
  - `tessellate`
  - `update_texture`
  - `update_buffers`
  - `render`
  - `queue.submit`
  - `present`
  - `free_texture`
- 处理 `ViewportCommand::Close`，将 egui 的退出命令转成 `event_loop.exit()`。
- 处理 repaint request callback，并通过 winit user event 唤醒事件循环。

### 2.3 只支持 root viewport

当前最小 runtime 只需要支持 root viewport。多原生窗口不是本阶段目标。

未来如果要支持 egui 多 viewport，可以把这些字段：

```rust
window: Option<Arc<Window>>,
egui_state: Option<egui_winit::State>,
gpu: Option<GpuState>,
```

升级为：

```rust
HashMap<ViewportId, ViewportRuntimeState>
```

## 3. 推荐的模块结构

### 3.1 main.rs

推荐最终形态：

```rust
mod kairos_dialog;
mod kairos_editor;
mod egui_utils;

use std::error::Error;
use winit::event_loop::EventLoop;

use crate::kairos_editor::runtime::{
    KairosEditorRuntime,
    KairosEditorRuntimeEvent,
};

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let event_loop = EventLoop::<KairosEditorRuntimeEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    let mut runtime = KairosEditorRuntime::new(proxy).unwrap_or_else(|error| {
        kairos_dialog::error_message_window(
            "Init Failed",
            &format!("new KairosEditorRuntime failed:\n{error}"),
        );
        panic!("new KairosEditorRuntime failed: {error}");
    });

    event_loop.run_app(&mut runtime)?;
    Ok(())
}
```

注意：

- `run_app` 会阻塞直到窗口退出，所以窗口 icon 必须在 runtime 创建窗口前准备，不能写在 `run_app` 后面。
- 如果项目中没有其他 `tokio` 用法，`#[tokio::main]` 和 `tokio` 依赖都可以删。

### 3.2 kairos_editor.rs

推荐最终形态：

```rust
use egui::Visuals;
use kairos_engine::log::Log;

pub mod consts;
pub mod ui;
pub mod runtime;

pub struct KairosEngine {
    ui_context: ui::Context,
    log: Log,
}

impl KairosEngine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            ui_context: ui::Context::new(),
            log: Log::new(),
        })
    }

    pub(crate) fn update(&mut self, ctx: &egui::Context) {
        let mut visuals = Visuals::dark();
        visuals.button_frame = true;
        ctx.set_visuals(visuals);

        self.ui_context.handle(ctx, &mut self.log);
        self.ui_context.darw(ctx, &mut self.log);
    }

    pub(crate) fn on_exit(&mut self) {}
}
```

### 3.3 runtime.rs 核心类型

推荐 runtime 主要类型：

```rust
type RuntimeResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone)]
pub enum KairosEditorRuntimeEvent {
    RequestRepaint {
        viewport_id: egui::ViewportId,
        delay: std::time::Duration,
    },
}

pub struct KairosEditorRuntime {
    engine: KairosEngine,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    window: Option<std::sync::Arc<winit::window::Window>>,
    gpu: Option<GpuState>,
    repaint_at: Option<std::time::Instant>,
    did_exit: bool,
}

struct GpuState {
    surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: egui_wgpu::Renderer,
}
```

## 4. 一帧渲染流程

`WindowEvent::RedrawRequested` 中应调用 `redraw`，流程为：

```rust
let raw_input = egui_state.take_egui_input(&window);

let full_output = egui_ctx.run(raw_input, |ctx| {
    engine.update(ctx);
});

egui_state.handle_platform_output(&window, full_output.platform_output);

// 处理 root viewport commands，尤其是 ViewportCommand::Close。

let clipped_primitives =
    egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

gpu.paint(
    &full_output.textures_delta,
    &clipped_primitives,
    full_output.pixels_per_point,
)?;
```

`GpuState::paint` 中应按这个顺序：

```text
1. renderer.update_texture(...) for textures_delta.set
2. surface.get_current_texture()
3. create TextureView
4. create CommandEncoder
5. renderer.update_buffers(...)
6. begin_render_pass(...)
7. render_pass.forget_lifetime()
8. renderer.render(...)
9. queue.submit(user_cmd_buffers + encoder.finish())
10. output_frame.present()
11. renderer.free_texture(...) for textures_delta.free
```

`egui_wgpu::Renderer::render` 在 `egui-wgpu 0.33.3` 中要求：

```rust
&mut wgpu::RenderPass<'static>
```

所以需要：

```rust
let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor { ... });
let mut render_pass = render_pass.forget_lifetime();
renderer.render(&mut render_pass, clipped_primitives, &screen_descriptor);
```

## 5. root ViewportCommand::Close

`ui::Context` 中已有：

```rust
ctx.send_viewport_cmd(egui::ViewportCommand::Close);
```

自有 runtime 必须显式读取：

```rust
if let Some(root_output) = full_output.viewport_output.get(&ViewportId::ROOT) {
    let should_close = root_output
        .commands
        .iter()
        .any(|command| matches!(command, ViewportCommand::Close));

    if should_close {
        self.shutdown(event_loop);
        return;
    }
}
```

否则菜单或按钮发出的关闭命令只会停留在 egui output 中，不会真正退出应用。

## 6. 当前代码快照和状态

截至本笔记生成时：

- `src/kairos_editor/runtime.rs` 已经有一版 `KairosEditorRuntime` 和 `GpuState` 实现。
- `src/main.rs` 已经改为 `EventLoop::<KairosEditorRuntimeEvent>::with_user_event().build()?`。
- `src/kairos_editor.rs` 已经移除了 `eframe::CreationContext` 和 `eframe::App`。
- `cargo check` 可以通过。
- 仍有 warning 和一些迁移尾巴需要清理。

## 7. 下一步建议清单

### 7.1 清理 main.rs

当前 `src/main.rs` 中 `event_loop.run_app(&mut runtime)?;` 后还有一段 icon 加载代码。它不会参与窗口创建，应删除。

推荐同时把：

```rust
#[tokio::main]
async fn main()
```

改为普通：

```rust
fn main()
```

如果没有其他 `tokio` 使用，也从 `Cargo.toml` 删除 `tokio`。

### 7.2 清理 runtime.rs warning

当前可见 warning / 小问题：

- `didi_exit` 建议改名为 `did_exit`。
- `event_loop::{self, ...}` 中的 `self` 未使用。
- `GpuState::new` 中算出了 `format`，但当前代码没有写回：

```rust
config.format = format;
config.view_formats = vec![format];
```

- `proxy.send_event(...)` 的返回值需要显式忽略：

```rust
let _ = proxy.send_event(...);
```

- `adapter` 字段如果暂时不用，可以删掉；如果后续调试 GPU 信息，可以保留并接受 warning。
- `WindowEvent::Resized` / `ScaleFactorChanged` 后建议 `window.request_redraw()`。

### 7.3 避免 exiting 里递归 exit

当前代码中：

```rust
fn exiting(&mut self, event_loop: &ActiveEventLoop) {
    self.shutdown(event_loop);
}
```

而 `shutdown` 内部又调用 `event_loop.exit()`。更干净的做法是拆出：

```rust
fn call_on_exit_once(&mut self) {
    if !self.did_exit {
        self.engine.on_exit();
        self.did_exit = true;
    }
}

fn shutdown(&mut self, event_loop: &ActiveEventLoop) {
    self.call_on_exit_once();
    event_loop.exit();
}

fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
    self.call_on_exit_once();
}
```

### 7.4 删除 eframe

确认代码不再引用 `eframe` 后：

```toml
# 删除
eframe = { version = "0.33.3", features = ["wgpu"] }
```

然后运行：

```bash
cargo check
```

`Cargo.lock` 会随依赖变化更新。

## 8. 给下一次对话的启动提示

可以在另一台电脑上这样继续：

```text
请阅读 docs/ai/eframe-runtime-migration-notes.md，并检查当前 src/main.rs、src/kairos_editor.rs、src/kairos_editor/runtime.rs、Cargo.toml。
继续完成 eframe 剥离：清理 main.rs 中 run_app 后的 icon 代码，修复 runtime.rs warning，确认 ViewportCommand::Close、resize、repaint、texture update/free 流程正确，然后删除 Cargo.toml 中 eframe 依赖并 cargo check。
```

## 9. 相关文件

- `src/main.rs`
- `src/kairos_editor.rs`
- `src/kairos_editor/runtime.rs`
- `src/kairos_editor/ui.rs`
- `src/kairos_editor/ui/docking_tab.rs`
- `src/kairos_editor/ui/docking_tab/drag_and_drop.rs`
- `Cargo.toml`
- `Cargo.lock`
