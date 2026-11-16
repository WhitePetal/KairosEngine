//! The engine's window and frame loop.
//!
//! [`EngineRunner`] is the process's main loop. It owns the winit event loop,
//! the window, the wgpu surface ([`RenderPipeline`]) and the [`Engine`], and it
//! drives [`Engine::update`] exactly once per redraw. Hosts plug in through
//! [`Layer`]: the editor draws egui there, and a game build can leave every
//! method at its default.
//!
//! The loop lives in the engine on purpose — the engine *is* the runtime. The
//! editor is a layer on top of it, not a second event loop; a game build is the
//! same loop with a layer that draws nothing.
//!
//! # Frame order
//!
//! One [`WindowEvent::RedrawRequested`] is one frame:
//!
//! 1. [`Engine::update`] — the frame boundary. The engine samples its own
//!    wall-clock delta at its `First` stage, so the host never hands it a
//!    timestamp; all it has to do is keep the frames coming.
//! 2. [`Layer::render`] — the layer turns its interface into
//!    [`GraphicsCommand`]s. The editor's egui pass is exactly this step.
//! 3. [`RenderPipeline::present`] executes the assembled graph on the
//!    swapchain.
//! 4. The window is asked for the next redraw, so vsync paces the loop.
//!
//! # Input
//!
//! Window events reach the layer first. Only what the layer does not take
//! ([`EventOutcome::consumed`]) is forwarded to the engine's input — the gate
//! that keeps typing into an editor text field from also driving the game.
//!
//! # Pacing
//!
//! The loop is continuous: every presented frame asks for the next redraw,
//! which `PresentMode::Fifo` caps at the display refresh rate. The engine
//! therefore ticks at the display rate whether or not the window receives
//! input, which is what keeps an idle editor producing frames. A lower cap or a
//! low-power idle mode would be a policy on top of this loop, not a different
//! loop.

use std::{
    error::Error,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Icon, Window},
};

use crate::{
    Engine,
    graphics::{
        graphics_graph::{GraphicsCommand, GraphicsGraph},
        render_pipeline::RenderPipeline,
        texture::format::TextureCompressionConfig,
    },
};

/// An event the loop itself reacts to, sent through an [`EventLoopProxy`] — by
/// an egui repaint callback, a GPU device-loss callback, or the host.
#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    /// Wake the loop and draw again after `delay`.
    RequestRedraw { delay: Duration },
    /// Tear the loop down (a fatal GPU error, a host-requested quit).
    Exit,
}

/// What a [`Layer`] did with one window event.
#[derive(Debug, Clone, Copy, Default)]
pub struct EventOutcome {
    /// The layer took exclusive use of the event; the engine's input must not
    /// see it.
    pub consumed: bool,
    /// The layer wants a redraw because of this event.
    pub repaint: bool,
}

/// What a [`Layer`] wants the loop to do after the frame it rendered.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameOutput {
    /// Close the window and shut the loop down.
    pub close_requested: bool,
    /// Earliest delay before the next frame the layer needs (egui's
    /// `repaint_delay`). `Some(Duration::ZERO)` means "now".
    pub repaint_delay: Option<Duration>,
}

/// The host-specific half of the loop.
///
/// Every method has a default, so a game build implements nothing. The editor
/// overrides [`window_created`](Layer::window_created) (to build its egui
/// state), [`window_event`](Layer::window_event) (to feed egui first),
/// [`render`](Layer::render) (the egui pass) and
/// [`on_exit`](Layer::on_exit) (to save its profiling capture).
pub trait Layer {
    /// The window and GPU are ready. Runs once, before the first frame.
    fn window_created(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window: &Arc<Window>,
        _max_texture_side: u32,
    ) {
    }

    /// One window event. Runs before the engine's input sees it.
    fn window_event(&mut self, _window: &Window, _event: &WindowEvent) -> EventOutcome {
        EventOutcome::default()
    }

    /// Build this frame's graphics commands.
    ///
    /// `size_in_pixels` is the swapchain size, for layers that lay themselves
    /// out in device pixels. The engine has already ticked.
    fn render(
        &mut self,
        _engine: &mut Engine,
        _window: &Window,
        _size_in_pixels: [u32; 2],
        _out: &mut Vec<GraphicsCommand>,
    ) -> FrameOutput {
        FrameOutput::default()
    }

    /// The loop is going away. Runs at most once.
    fn on_exit(&mut self, _engine: &mut Engine) {}
}

/// What the loop needs from the host to open its window and GPU.
pub struct RuntimeConfig {
    /// Window title.
    pub title: String,
    /// Initial inner size, in logical pixels.
    pub size: LogicalSize<f32>,
    /// Minimum inner size, in logical pixels, if the host wants one.
    pub min_size: Option<LogicalSize<f32>>,
    /// Window icon, if the host has one.
    pub icon: Option<Icon>,
    /// Texture compression features the render pipeline may use. The host owns
    /// this because it comes from the project's settings file.
    pub texture_compression: TextureCompressionConfig,
}

struct FrameRateCounter {
    last_report_at: Instant,
    frames_since_report: u32,
}

impl FrameRateCounter {
    fn new() -> Self {
        Self {
            last_report_at: Instant::now(),
            frames_since_report: 0,
        }
    }

    fn record_frame(&mut self) {
        self.frames_since_report += 1;

        let now = Instant::now();
        let elapsed = now.duration_since(self.last_report_at);
        if elapsed < Duration::from_secs(1) {
            return;
        }

        let fps = self.frames_since_report as f64 / elapsed.as_secs_f64();
        log::debug!("FPS: {fps:.1}");

        self.last_report_at = now;
        self.frames_since_report = 0;
    }
}

/// Locks the pipeline slot, ignoring a poisoned mutex: the guarded value is a
/// plain `Option` whose only writer is the loop, so a panic elsewhere cannot
/// leave it half-updated.
///
/// A free function rather than a method so the guard borrows the field alone —
/// the loop still needs `&mut` access to the layer and the engine while it
/// holds the pipeline.
fn lock_pipeline(
    slot: &Arc<Mutex<Option<RenderPipeline>>>,
) -> std::sync::MutexGuard<'_, Option<RenderPipeline>> {
    slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The main loop: window, swapchain, input pump, and the engine's frame
/// boundary.
///
/// A host builds one through [`run`], which owns the [`EventLoop`] this type
/// drives. The engine is owned here, not by the layer: the loop calls
/// [`Engine::update`] and `present` on its behalf, and the layer only ever
/// borrows it.
pub struct EngineRunner<L: Layer> {
    engine: Engine,
    layer: L,
    config: RuntimeConfig,
    proxy: EventLoopProxy<RuntimeEvent>,
    window: Option<Arc<Window>>,
    render_pipeline: Arc<Mutex<Option<RenderPipeline>>>,
    repaint_at: Option<Instant>,
    frame_rate_counter: FrameRateCounter,
    did_exit: bool,
}

impl<L: Layer> EngineRunner<L> {
    fn new(
        engine: Engine,
        config: RuntimeConfig,
        layer: L,
        proxy: EventLoopProxy<RuntimeEvent>,
    ) -> Self {
        Self {
            engine,
            layer,
            config,
            proxy,
            window: None,
            render_pipeline: Arc::new(Mutex::new(None)),
            repaint_at: None,
            frame_rate_counter: FrameRateCounter::new(),
            did_exit: false,
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        let mut attributes = Window::default_attributes()
            .with_title(self.config.title.clone())
            .with_inner_size(self.config.size)
            .with_decorations(true)
            .with_transparent(false)
            .with_visible(false)
            .with_window_icon(self.config.icon.clone());

        if let Some(min_size) = self.config.min_size {
            attributes = attributes.with_min_inner_size(min_size);
        }

        let window = Arc::new(event_loop.create_window(attributes)?);

        let render_pipeline = pollster::block_on(RenderPipeline::new(
            window.clone(),
            &self.config.texture_compression,
            &self.engine.world,
        ))?;

        let proxy = self.proxy.clone();
        render_pipeline
            .device
            .set_device_lost_callback(move |reason, message| {
                log::error!("GPU device lost ({reason:?}): {message}");
                let _ = proxy.send_event(RuntimeEvent::Exit);
            });
        render_pipeline
            .device
            .on_uncaptured_error(Arc::new(|error| {
                log::error!("uncaptured GPU error: {error}");
            }));

        let max_texture_side = render_pipeline.max_texture_side().min(u32::MAX as usize) as u32;
        lock_pipeline(&self.render_pipeline).replace(render_pipeline);
        self.window = Some(window.clone());

        self.layer
            .window_created(event_loop, &window, max_texture_side);

        window.set_visible(true);
        window.request_redraw();

        Ok(())
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };

        // The frame boundary: the engine ticks once per redraw and samples its
        // own wall-clock delta, so the loop never hands it a timestamp.
        self.engine.update();

        let mut frame_output = FrameOutput::default();
        let mut presented = false;

        {
            let mut slot = lock_pipeline(&self.render_pipeline);
            let Some(render_pipeline) = slot.as_mut() else {
                return;
            };

            match render_pipeline.get_window_surface() {
                Ok(surface_texture) => {
                    let mut graphics_commands = Vec::<GraphicsCommand>::new();
                    frame_output = self.layer.render(
                        &mut self.engine,
                        &window,
                        [
                            render_pipeline.surface_config.width,
                            render_pipeline.surface_config.height,
                        ],
                        &mut graphics_commands,
                    );

                    let graphics_graph = GraphicsGraph::build(graphics_commands);
                    render_pipeline.present(&self.engine.world, surface_texture, graphics_graph);
                    presented = true;
                }
                Err(error) => match error {
                    wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                        render_pipeline.set_window_resize(window.inner_size());
                    }
                    wgpu::CurrentSurfaceTexture::Occluded => {}
                    wgpu::CurrentSurfaceTexture::Timeout => {
                        log::warn!("wgpu surface timeout while acquiring the next frame");
                    }
                    wgpu::CurrentSurfaceTexture::Validation => {
                        log::error!("wgpu surface validation error while acquiring the next frame");
                    }
                    wgpu::CurrentSurfaceTexture::Success(_)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(_) => {
                        unreachable!("success variants are returned from the Ok branch")
                    }
                },
            }
        }

        if presented {
            self.frame_rate_counter.record_frame();
        }

        window.request_redraw();

        if frame_output.close_requested {
            self.shutdown(event_loop);
            return;
        }

        if let Some(delay) = frame_output.repaint_delay {
            self.set_repaint_delay_from_output(delay);
        }
    }

    fn queue_repaint_after(&mut self, delay: Duration) {
        if delay == Duration::ZERO {
            self.repaint_at = None;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }

        if delay == Duration::MAX {
            return;
        }

        let now = Instant::now();
        let at = now.checked_add(delay).unwrap_or(now);

        self.repaint_at = Some(match self.repaint_at {
            Some(existing) => existing.min(at),
            None => at,
        });
    }

    fn set_repaint_delay_from_output(&mut self, delay: Duration) {
        self.repaint_at = None;
        self.queue_repaint_after(delay);
    }

    fn drive_repaint_timer(&mut self, event_loop: &ActiveEventLoop) {
        let Some(repaint_at) = self.repaint_at else {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        };

        if Instant::now() >= repaint_at {
            self.repaint_at = None;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::Poll);
        } else {
            event_loop.set_control_flow(ControlFlow::WaitUntil(repaint_at));
        }
    }

    fn shutdown(&mut self, event_loop: &ActiveEventLoop) {
        if !self.did_exit {
            self.layer.on_exit(&mut self.engine);
            self.did_exit = true;
        }

        event_loop.exit();
    }
}

impl<L: Layer> ApplicationHandler<RuntimeEvent> for EngineRunner<L> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        if let Err(error) = self.create_window(event_loop) {
            log::error!("creating the window/GPU runtime failed: {error}");
            self.shutdown(event_loop);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: RuntimeEvent) {
        match event {
            RuntimeEvent::RequestRedraw { delay } => self.queue_repaint_after(delay),
            RuntimeEvent::Exit => self.shutdown(event_loop),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.clone() else {
            return;
        };

        if window.id() != window_id {
            return;
        }

        // The layer sees the event first: egui's text focus is what decides
        // whether a key press should also reach the engine's input.
        let outcome = self.layer.window_event(&window, &event);
        if outcome.repaint {
            window.request_redraw();
        }

        match event {
            WindowEvent::CloseRequested => self.shutdown(event_loop),
            WindowEvent::Resized(physical_size) => {
                if let Some(render_pipeline) = lock_pipeline(&self.render_pipeline).as_mut() {
                    render_pipeline.set_window_resize(physical_size);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(render_pipeline) = lock_pipeline(&self.render_pipeline).as_mut() {
                    render_pipeline.set_window_resize(window.inner_size());
                }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            WindowEvent::KeyboardInput { event, .. } if !outcome.consumed => {
                self.forward_key(event);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.drive_repaint_timer(event_loop);
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.shutdown(event_loop);
    }
}

impl<L: Layer> EngineRunner<L> {
    /// Feeds one key press into the engine's input.
    ///
    /// Repeats are dropped: the engine's input tracks physical key state, which
    /// an auto-repeat does not change.
    fn forward_key(&mut self, event: KeyEvent) {
        if event.repeat {
            return;
        }

        self.engine.input_engine.update_keyboard_input(event);
    }
}

/// Runs the engine's window loop until the host closes it.
///
/// Creates the winit event loop, lets `make_layer` build the host layer (and
/// add whatever it needs to `engine` before the window opens), then drives the
/// loop. Returns when the loop exits.
///
/// `make_layer` receives the event-loop proxy because a layer that redraws on
/// demand — egui, through `Context::set_request_repaint_callback` — has to wire
/// its wake-ups before the loop starts.
pub fn run<L, F>(
    mut engine: Engine,
    config: RuntimeConfig,
    make_layer: F,
) -> Result<(), Box<dyn Error>>
where
    L: Layer,
    F: FnOnce(&mut Engine, &EventLoopProxy<RuntimeEvent>) -> Result<L, Box<dyn Error>>,
{
    let event_loop = EventLoop::<RuntimeEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    let layer = make_layer(&mut engine, &proxy)?;

    let mut runner = EngineRunner::new(engine, config, layer, proxy);
    event_loop.run_app(&mut runner)?;

    Ok(())
}
