# KairosEditorRuntime 代码走读笔记

> 来源：AI 辅助代码回顾与讲解（2026-05-20）  
> 状态：接力笔记，记录已讲解内容与后续走读入口  
> 范围：`src/main.rs` 与 `src/kairos_editor/runtime.rs` 中自有 runtime 的启动、初始化与单帧重绘流程

## 1. 背景

项目正在从：

```text
eframe + egui
```

迁移到：

```text
winit + wgpu + egui + egui-winit + egui-wgpu
```

用户明确表示目前不熟悉 `winit` 和 `wgpu`，所以本轮对话开始逐条回顾已经写出的 runtime 代码，目标不是继续大幅改动，而是理解每段代码在运行时到底做什么。

截至本笔记生成时，已讲解：

- `main` 方法整体流程
- `event_loop` 是什么
- `proxy` 是什么
- `KairosEditorRuntime::new`
- `ApplicationHandler::resumed`
- `KairosEditorRuntime::create_window`
- `KairosEditorRuntime::redraw`

后续还建议继续讲：

- `GpuState::new`
- `GpuState::paint`
- `queue_repaint_after`
- `drive_repaint_timer`
- `window_event`
- `user_event`
- `shutdown` / `exiting`

## 2. main 方法

当前 `src/main.rs` 的核心代码是：

```rust
mod kairos_dialog;
mod kairos_editor;

use winit::event_loop::EventLoop;

use crate::kairos_editor::{
    runtime::{KairosEditorRuntime, KairosEditorRuntimeEvent}
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let event_loop = EventLoop::<KairosEditorRuntimeEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    let mut runtime = KairosEditorRuntime::new(proxy).unwrap_or_else(|error| {
        kairos_dialog::error_message_window(
            "Init Failed",
            &format!("new MainEditorWindow struct Failed:\n {}", error),
        );
        panic!("new MainEditorWindow Failed: {}", error);
    });

    event_loop.run_app(&mut runtime)?;

    Ok(())
}
```

### 2.1 event_loop 是什么

`event_loop` 是应用和操作系统窗口系统之间的主消息循环。

桌面 GUI 应用不是简单地自己写：

```rust
loop {
    update();
    draw();
}
```

而是由操作系统不断发送事件，例如：

```text
应用启动 / 恢复
窗口关闭
窗口 resize
鼠标移动
键盘输入
需要重绘
定时唤醒
自定义事件
```

winit 的 `EventLoop` 负责接收这些 OS 事件，并把它们分发给实现了 `ApplicationHandler` 的 runtime。

在当前代码中：

```rust
event_loop.run_app(&mut runtime)?;
```

会进入 winit 主循环。之后 winit 会回调 `KairosEditorRuntime` 的方法：

```rust
fn resumed(...)
fn window_event(...)
fn user_event(...)
fn about_to_wait(...)
fn exiting(...)
```

所以：

```text
EventLoop 是主消息循环本体。
KairosEditorRuntime 是我们交给 winit 调度的应用状态机。
```

### 2.2 为什么是 EventLoop::<KairosEditorRuntimeEvent>

```rust
let event_loop = EventLoop::<KairosEditorRuntimeEvent>::with_user_event().build()?;
```

泛型参数 `KairosEditorRuntimeEvent` 表示这个 event loop 除了可以接收 OS 事件，还可以接收项目自定义事件。

当前自定义事件定义为：

```rust
pub enum KairosEditorRuntimeEvent {
    RequestRepaint {
        viewport_id: ViewportId,
        delay: Duration,
    }
}
```

目前它只负责一件事：让 egui 的 repaint 请求能够唤醒 winit event loop。

### 2.3 proxy 是什么

```rust
let proxy = event_loop.create_proxy();
```

`proxy` 是：

```rust
EventLoopProxy<KairosEditorRuntimeEvent>
```

可以理解成 event loop 的“事件投递口”或“遥控器”。

真正的 `event_loop` 在 `run_app` 之后由 winit 持有和运行，很多外部 callback 拿不到它本体。但这些 callback 仍然可能需要唤醒主循环，所以要通过 `proxy.send_event(...)` 发送自定义事件。

在当前 runtime 中，`proxy` 被传入：

```rust
KairosEditorRuntime::new(proxy)
```

然后被挂到 egui 的 repaint callback 上。

### 2.4 main 的整体流程

```text
1. 初始化日志 env_logger。
2. 创建支持自定义事件的 winit EventLoop。
3. 从 EventLoop 创建 proxy。
4. 创建 KairosEditorRuntime，并把 proxy 交给它。
5. 调用 event_loop.run_app(&mut runtime) 进入 winit 主循环。
6. winit 开始回调 runtime 的生命周期方法。
7. event loop 退出后，main 返回 Ok(())。
```

注意：当前 `main` 仍是 `#[tokio::main] async fn main`，但当前入口中没有 `.await`。如果项目其他地方不需要 tokio runtime，后续可以改成普通 `fn main()`，并移除 `tokio` 依赖。

## 3. KairosEditorRuntime::new

当前核心代码：

```rust
pub fn new(proxy: EventLoopProxy<KairosEditorRuntimeEvent>) -> RuntimeResult<Self> {
    let egui_ctx = egui::Context::default();
    egui_extras::install_image_loaders(&egui_ctx);

    egui_ctx.set_request_repaint_callback(move |info| {
        let _ = proxy.send_event(KairosEditorRuntimeEvent::RequestRepaint {
            viewport_id: info.viewport_id,
            delay: info.delay,
        });
    });

    Ok(Self {
        engine: KairosEngine::new()?,
        egui_ctx,
        egui_state: None,
        window: None,
        gpu: None,
        repaint_at: None,
        didi_exit: false,
    })
}
```

### 3.1 new 的职责

`new` 只创建“不依赖 OS 窗口”的状态：

- `egui::Context`
- egui image loaders
- egui repaint callback
- `KairosEngine`
- runtime 的空壳状态

它不创建：

- winit `Window`
- `egui_winit::State`
- wgpu `Surface`
- wgpu `Device`
- wgpu `Queue`
- `egui_wgpu::Renderer`

这些都要等 `resumed` 时通过 `ActiveEventLoop` 创建。

### 3.2 egui::Context 是什么

```rust
let egui_ctx = egui::Context::default();
```

`egui::Context` 是 egui 的 UI 状态中心，负责保存：

```text
UI 内存
控件状态
窗口状态
字体和纹理状态
重绘请求
输入处理结果
每帧输出
```

它不是 OS 窗口，也不是 GPU renderer。

三层分工是：

```text
egui::Context        -> egui 的 UI 状态和每帧逻辑
egui_winit::State    -> winit 事件与 egui 输入/输出之间的桥
egui_wgpu::Renderer  -> egui 图形数据到 wgpu 绘制命令
```

### 3.3 install_image_loaders

```rust
egui_extras::install_image_loaders(&egui_ctx);
```

这句给 egui 安装图片加载能力，例如将来 UI 中使用图片 URI 或 egui_extras 支持的图片资源时，egui 才知道如何解码并注册纹理。

这件事绑定在 `egui::Context` 上。

### 3.4 repaint callback 和 proxy

```rust
egui_ctx.set_request_repaint_callback(move |info| {
    let _ = proxy.send_event(KairosEditorRuntimeEvent::RequestRepaint {
        viewport_id: info.viewport_id,
        delay: info.delay,
    });
});
```

当 egui 内部需要重绘时，会调用这个 callback。例如：

```text
鼠标 hover 状态变化
文本光标闪烁
动画
拖拽
异步图片加载完成
ctx.request_repaint()
ctx.request_repaint_after(...)
```

callback 里通过 `proxy` 向 winit event loop 发送自定义事件：

```text
egui 请求 repaint
  -> repaint callback
  -> proxy.send_event(RequestRepaint)
  -> winit event loop 被唤醒
  -> runtime.user_event(...)
  -> runtime.queue_repaint_after(delay)
  -> window.request_redraw()
  -> WindowEvent::RedrawRequested
  -> runtime.redraw(...)
```

这里使用 `move`，是因为 callback 会被 egui 保存起来。`move` 会把 `proxy` 移入闭包，让闭包自己持有它。

`let _ = proxy.send_event(...)` 表示如果 event loop 已经退出，发送失败也静默忽略。退出后再 repaint 已经没有意义。

### 3.5 为什么 window / egui_state / gpu 是 None

```rust
egui_state: None,
window: None,
gpu: None,
```

因为这些资源都依赖正在运行的 winit 生命周期：

```text
Window              -> 需要 ActiveEventLoop 创建
egui_winit::State   -> 需要 window / event_loop display 信息
GpuState            -> 需要 window 创建 wgpu Surface
```

所以它们会在 `resumed -> create_window` 中填入。

## 4. resumed 与 create_window

`resumed` 当前代码：

```rust
fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
    if self.window.is_some() {
        return;
    }

    if let Err(error) = self.create_window(event_loop) {
        kairos_dialog::error_message_window(
            "Init Failed",
            &format!("Create KairosEditor window/runtime failed:\n{error}"),
        );
        self.shutdown(event_loop);
    }
}
```

### 4.1 resumed 的含义

`resumed` 是 winit `ApplicationHandler` 生命周期方法之一。

它表示应用已经处于可以创建或恢复窗口资源的阶段。桌面端通常就是应用启动后可以创建窗口；移动端这个生命周期更重要，因为应用可能暂停、恢复，surface 可能失效再重建。

### 4.2 ActiveEventLoop 是什么

`ActiveEventLoop` 是 winit 在运行中的 event loop 操作句柄。通过它可以：

```rust
event_loop.create_window(...)
event_loop.exit()
event_loop.set_control_flow(...)
```

区别：

```text
EventLoop        -> main 中创建，run_app 后由 winit 持有
ActiveEventLoop  -> run_app 期间，winit 回调时传给 runtime 的操作入口
```

### 4.3 为什么先判断 window.is_some

```rust
if self.window.is_some() {
    return;
}
```

当前 runtime 是单窗口模型。`resumed` 理论上可能被调用多次，所以这里防止重复创建窗口和 GPU surface。

### 4.4 create_window 做了什么

`create_window` 负责真正创建窗口和图形资源：

```rust
fn create_window(&mut self, event_loop: &ActiveEventLoop) -> RuntimeResult<()> {
    let title = format!("{} {}", consts::APP_NAME, consts::VERSION);

    let attrs = Window::default_attributes()
        .with_title(title)
        .with_inner_size(LogicalSize::new(800.0, 600.0))
        .with_decorations(true)
        .with_transparent(false)
        .with_window_icon(load_icon());

    let window = Arc::new(event_loop.create_window(attrs)?);

    let mut egui_state = egui_winit::State::new(
        self.egui_ctx.clone(),
        ViewportId::ROOT,
        event_loop,
        Some(window.scale_factor() as f32),
        window.theme(),
        None,
    );

    let gpu = pollster::block_on(GpuState::new(window.clone()))?;
    egui_state.set_max_texture_side(gpu.max_texture_side());

    self.window = Some(window.clone());
    self.egui_state = Some(egui_state);
    self.gpu = Some(gpu);

    window.request_redraw();

    Ok(())
}
```

步骤：

```text
1. 用 APP_NAME 和 VERSION 组成窗口标题。
2. 构造 WindowAttributes：
   - 800 x 600
   - 有系统窗口装饰
   - 不透明
   - 加载 icon
3. 通过 ActiveEventLoop 创建 OS Window。
4. 创建 egui_winit::State，作为 winit 与 egui 的输入/输出桥。
5. 通过 GpuState::new 初始化 wgpu 和 egui_wgpu renderer。
6. 把 GPU 最大纹理尺寸告诉 egui。
7. 存回 runtime 字段。
8. request_redraw 触发第一帧绘制。
```

### 4.5 为什么 window 用 Arc

```rust
let window = Arc::new(event_loop.create_window(attrs)?);
```

`Arc<Window>` 允许 runtime 和 wgpu surface 创建过程共享同一个 window 句柄。clone `Arc` 只是增加引用计数，不会复制真实窗口。

### 4.6 request_redraw 的意义

```rust
window.request_redraw();
```

这句告诉 winit 尽快给窗口发送：

```rust
WindowEvent::RedrawRequested
```

随后 runtime 会进入：

```rust
WindowEvent::RedrawRequested => {
    self.redraw(event_loop);
}
```

这就是第一帧启动点。

## 5. redraw 方法

`redraw` 是自有 runtime 的“一帧调度器”。

它在收到：

```rust
WindowEvent::RedrawRequested
```

时被调用。

当前代码：

```rust
fn redraw(&mut self, event_loop: &ActiveEventLoop) {
    let Some(window) = self.window.as_ref().cloned() else {
        return;
    };
    let Some(egui_state) = self.egui_state.as_mut() else {
        return;
    };

    let raw_input = egui_state.take_egui_input(&window);
    let full_output = self.egui_ctx.run(raw_input, |ctx| {
        self.engine.update(ctx);
    });

    egui_state.handle_platform_output(&window, full_output.platform_output);

    let mut should_close = false;
    let mut repaint_delay = None;

    if let Some(root_output) = full_output.viewport_output.get(&ViewportId::ROOT) {
        should_close = root_output
            .commands
            .iter()
            .any(|command| matches!(command, ViewportCommand::Close));

        repaint_delay = Some(root_output.repaint_delay);

        let mut actions_requested = Vec::new();
        let viewport_info = egui_state
            .egui_input_mut()
            .viewports
            .entry(ViewportId::ROOT)
            .or_default();

        egui_winit::process_viewport_commands(
            &self.egui_ctx,
            viewport_info,
            root_output.commands.iter().cloned(),
            &window,
            &mut actions_requested,
        );

        if !actions_requested.is_empty() {
            log::debug!("Ignored viewport actions: {actions_requested:?}");
        }
    }

    if should_close {
        self.shutdown(event_loop);
        return;
    }

    if let Some(delay) = repaint_delay {
        self.set_repaint_delay_from_output(delay);
    }

    let clipped_primitives =
        self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

    let Some(gpu) = self.gpu.as_mut() else {
        return;
    };

    match gpu.paint(
        &full_output.textures_delta,
        &clipped_primitives,
        full_output.pixels_per_point,
    ) {
        Ok(()) => {}
        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
            gpu.resize(window.inner_size());
            window.request_redraw();
        }
        Err(wgpu::SurfaceError::OutOfMemory) => {
            log::error!("wgpu surface out of memory");
            self.shutdown(event_loop);
        }
        Err(wgpu::SurfaceError::Timeout) => {
            log::warn!("wgpu surface timeout");
        }
        Err(wgpu::SurfaceError::Other) => {
            log::warn!("wgpu surface error");
        }
    }
}
```

### 5.1 取出 window 和 egui_state

```rust
let Some(window) = self.window.as_ref().cloned() else {
    return;
};
let Some(egui_state) = self.egui_state.as_mut() else {
    return;
};
```

一帧绘制必须依赖：

```text
window      -> 当前 OS 窗口
egui_state  -> winit 与 egui 的输入/输出桥
```

如果 `resumed` 还没有成功创建资源，就直接返回。

### 5.2 收集 egui 输入

```rust
let raw_input = egui_state.take_egui_input(&window);
```

之前在 `window_event` 中，winit 的鼠标、键盘、窗口变化事件会被喂给：

```rust
egui_state.on_window_event(&window, &event);
```

`egui_state` 会暂存这些输入。

到了 `redraw`，`take_egui_input` 会把暂存输入打包成：

```rust
egui::RawInput
```

并清空输入缓冲，供本帧 egui 使用。

### 5.3 运行一帧 egui

```rust
let full_output = self.egui_ctx.run(raw_input, |ctx| {
    self.engine.update(ctx);
});
```

这一步会：

```text
1. 把 raw_input 喂给 egui。
2. 执行 KairosEngine::update(ctx)。
3. 跑完整个编辑器 UI。
4. 收集 egui 的 FullOutput。
```

`FullOutput` 包含：

```text
platform_output  -> 光标、剪贴板、IME、打开 URL 等平台请求
textures_delta   -> egui 纹理新增/更新/释放
shapes           -> 本帧 UI 绘制形状
pixels_per_point -> DPI 缩放
viewport_output  -> viewport/window 级别输出，如 Close、repaint_delay
```

### 5.4 处理平台输出

```rust
egui_state.handle_platform_output(&window, full_output.platform_output);
```

这会把 egui 的平台请求应用到 winit window，例如：

```text
设置鼠标光标形状
复制文本到剪贴板
打开 URL
设置 IME 输入区域
```

注意：`handle_platform_output` 不会替我们完整处理 root `ViewportCommand::Close`，所以后面还要单独读取 `viewport_output`。

### 5.5 处理 root viewport 输出

```rust
if let Some(root_output) = full_output.viewport_output.get(&ViewportId::ROOT) {
    ...
}
```

当前 runtime 只支持主窗口，也就是：

```rust
ViewportId::ROOT
```

这里做三件事。

第一，检查关闭命令：

```rust
should_close = root_output
    .commands
    .iter()
    .any(|command| matches!(command, ViewportCommand::Close));
```

项目 UI 中可能会调用：

```rust
ctx.send_viewport_cmd(egui::ViewportCommand::Close);
```

runtime 必须把它翻译成：

```rust
event_loop.exit()
```

否则 egui 只是发出了关闭意图，winit app 不会真正退出。

第二，读取下一次 repaint 延迟：

```rust
repaint_delay = Some(root_output.repaint_delay);
```

这用于后续安排下一帧，例如动画、输入光标闪烁、tooltip 延迟显示等。

第三，交给 `egui_winit::process_viewport_commands` 处理其他窗口命令：

```rust
egui_winit::process_viewport_commands(
    &self.egui_ctx,
    viewport_info,
    root_output.commands.iter().cloned(),
    &window,
    &mut actions_requested,
);
```

它可以处理部分窗口命令，例如：

```text
修改标题
修改窗口大小
修改可见性
拖动窗口
设置光标可见性
设置 IME
```

某些命令会进入 `actions_requested`，当前 runtime 只是 debug 记录，还没有具体实现。

### 5.6 关闭与 repaint 调度

```rust
if should_close {
    self.shutdown(event_loop);
    return;
}
```

如果本帧 UI 请求关闭应用，就调用 `shutdown` 并停止本帧后续 GPU 绘制。

```rust
if let Some(delay) = repaint_delay {
    self.set_repaint_delay_from_output(delay);
}
```

这会把 egui 输出的 repaint delay 转成 runtime 内部的 `repaint_at`。后续 `about_to_wait` 会根据它调用：

```rust
event_loop.set_control_flow(ControlFlow::WaitUntil(...))
```

或立即 `request_redraw`。

### 5.7 tessellate

```rust
let clipped_primitives =
    self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
```

egui 生成的 `shapes` 是高层绘制描述，例如：

```text
矩形
文字
线条
圆角
图片
裁剪区域
```

GPU 不直接认识这些高层 shape。`tessellate` 会把它们转换成更接近 GPU 绘制所需的：

```rust
Vec<egui::ClippedPrimitive>
```

也就是带裁剪区域的三角形网格 / 绘制 primitive。

`pixels_per_point` 用于处理 DPI 缩放。

### 5.8 调用 GpuState::paint

```rust
match gpu.paint(
    &full_output.textures_delta,
    &clipped_primitives,
    full_output.pixels_per_point,
) {
    ...
}
```

这里进入真正 wgpu 绘制阶段。

传入：

```text
textures_delta      -> egui 纹理变化，如字体图集、图片纹理
clipped_primitives  -> tessellate 后的绘制 primitive
pixels_per_point    -> DPI 缩放
```

`GpuState::paint` 内部负责：

```text
1. 更新 egui 纹理。
2. 获取 surface 当前帧。
3. 创建 TextureView。
4. 创建 CommandEncoder。
5. 更新 egui 顶点/索引 buffer。
6. 开启 render pass。
7. 调用 egui_wgpu::Renderer::render。
8. 提交 queue。
9. present 到窗口。
10. 释放 egui 不再使用的纹理。
```

### 5.9 SurfaceError 处理

```rust
Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
    gpu.resize(window.inner_size());
    window.request_redraw();
}
```

窗口 resize、最小化、显示器切换等情况下，surface 可能丢失或过期。这里通过重新配置 surface 并请求下一帧来恢复。

```rust
Err(wgpu::SurfaceError::OutOfMemory) => {
    log::error!("wgpu surface out of memory");
    self.shutdown(event_loop);
}
```

显存/资源不足通常难以可靠恢复，所以直接退出。

```rust
Err(wgpu::SurfaceError::Timeout) => {
    log::warn!("wgpu surface timeout");
}
```

获取当前 surface frame 超时，通常跳过这一帧。

```rust
Err(wgpu::SurfaceError::Other) => {
    log::warn!("wgpu surface error");
}
```

其他 surface 错误，当前只记录日志。

### 5.10 redraw 总流程

```text
WindowEvent::RedrawRequested
  -> redraw

redraw:
  1. 取 window / egui_state
  2. egui_state.take_egui_input(window)
  3. egui_ctx.run(raw_input, |ctx| engine.update(ctx))
  4. egui_state.handle_platform_output(...)
  5. 检查 root viewport commands
     - Close -> shutdown
     - repaint_delay -> 安排下一次重绘
  6. egui_ctx.tessellate(shapes, pixels_per_point)
  7. gpu.paint(textures_delta, clipped_primitives, pixels_per_point)
  8. 根据 SurfaceError 决定恢复、跳帧或退出
```

一句话总结：

```text
redraw 是自有 runtime 的一帧调度器：
它把 winit 输入喂给 egui，执行 KairosEngine UI，处理 egui 输出，
再把 egui 生成的绘制数据交给 wgpu 画到窗口上。
```

## 6. 已记录但尚未展开讲解的代码

后续建议继续从这里开始：

1. `GpuState::new`
   - `wgpu::Instance`
   - `create_surface`
   - `request_adapter`
   - `request_device`
   - `SurfaceConfiguration`
   - `egui_wgpu::Renderer::new`
2. `GpuState::paint`
   - `update_texture`
   - `get_current_texture`
   - `CommandEncoder`
   - `update_buffers`
   - `RenderPassDescriptor`
   - `forget_lifetime`
   - `queue.submit`
   - `present`
   - `free_texture`
3. `window_event`
   - 事件先给 `egui_state.on_window_event`
   - 再处理 `CloseRequested`、`Resized`、`ScaleFactorChanged`、`RedrawRequested`
4. `user_event`
   - `RequestRepaint`
   - `queue_repaint_after`
5. `about_to_wait`
   - `drive_repaint_timer`
   - `ControlFlow::Wait`
   - `ControlFlow::WaitUntil`
   - `ControlFlow::Poll`
6. `shutdown` / `exiting`
   - 避免重复调用 `engine.on_exit`
   - 当前字段名 `didi_exit` 后续可改成 `did_exit`

## 7. 当前观察到的小清单

- `main.rs` 当前仍使用 `#[tokio::main] async fn main`，但入口里暂无 `.await`。
- `runtime.rs` 中 `didi_exit` 建议改名为 `did_exit`。
- `runtime.rs` 顶部导入 `event_loop::{self, ...}` 中的 `self` 若仍未使用，可删。
- `GpuState::new` 中当前查找了 sRGB format，但没有写回 `config.format`；如果要优先使用 sRGB，应显式：

```rust
let format = caps
    .formats
    .iter()
    .copied()
    .find(|format| format.is_srgb())
    .unwrap_or(config.format);

config.format = format;
config.view_formats = vec![format];
```

- `window_event` 中 resize 后可以考虑补 `window.request_redraw()`，让 resize 后尽快重绘。

## 8. 给下一次对话的启动提示

可以这样继续：

```text
请阅读 docs/ai/runtime-code-walkthrough.md。
我们已经讲过 main、KairosEditorRuntime::new、resumed/create_window、redraw。
接下来从 GpuState::new 开始，逐行解释 wgpu 初始化流程，并说明 Instance、Surface、Adapter、Device、Queue、SurfaceConfiguration、egui_wgpu::Renderer 分别是什么。
```
