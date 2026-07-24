use std::{
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};

use egui::{ViewportCommand, ViewportId};
use parking_lot::Mutex;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy},
    window::{Icon, Window},
};

use crate::graphics::{
    attachment::{Attachment, AttachmentLoadAction, AttachmentStoreAction, InternalAttachmentId},
    graphics_graph::graphics_node::ColorAttachmentBind,
};
use crate::math::float4x4;
use crate::{
    graphics::graphics_graph::{GraphicsCommand, GraphicsGraph},
    kairos_paths,
    kairos_settings::EngineSettings,
};
use crate::{
    graphics::render_pipeline::RenderPipeline,
    kairos_dialog,
    kairos_editor::{KairosEngine, consts, ui::paths},
};

type RuntimeResult<T> = Result<T, Box<dyn Error>>;

fn load_icon() -> Option<Icon> {
    let bytes = std::fs::read(paths::PATH_ENGINE_ICON).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();

    Icon::from_rgba(image.into_raw(), width, height).ok()
}

fn load_engine_settings() -> RuntimeResult<EngineSettings> {
    let bytes = std::fs::read(kairos_paths::PATH_KAIROS_SETTINGS)?;
    toml::from_slice::<EngineSettings>(&bytes).map_err(|e| e.into())
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
        println!("FPS: {fps:.1}");

        self.last_report_at = now;
        self.frames_since_report = 0;
    }
}

#[derive(Debug, Clone)]
pub enum KairosEditorRuntimeEvent {
    RequestRepaint {
        viewport_id: ViewportId,
        delay: Duration,
    },
    RenderPipelineCrash,
}

pub struct KairosEditorRuntime {
    window: Option<Arc<Window>>,
    event_proxy: EventLoopProxy<KairosEditorRuntimeEvent>,
    render_pipeline: Arc<Mutex<Option<RenderPipeline>>>,
    kairos_engine: KairosEngine,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    repaint_at: Option<Instant>,
    frame_rate_counter: FrameRateCounter,
    // frame_start: Instant,
    min_frame_interval: Duration,
    didi_exit: bool,
}

impl KairosEditorRuntime {
    pub fn new(proxy: EventLoopProxy<KairosEditorRuntimeEvent>) -> RuntimeResult<Self> {
        // Temp
        // let (st, data) = crate::graphics::texture::SerializedTexture::convert_img_to_asset(&std::path::Path::new("res/textures/kairos_texture.png"))?;
        // st.save_to_file(&data)?;
        // let (st, data) = crate::graphics::texture::SerializedTexture::convert_img_to_asset(&std::path::Path::new("res/textures/white.png"))?;
        // st.save_to_file(&data)?;

        let egui_ctx = egui::Context::default();
        egui_extras::install_image_loaders(&egui_ctx);

        let egui_event_proxy = proxy.clone();
        egui_ctx.set_request_repaint_callback(move |info| {
            let _ = egui_event_proxy.send_event(KairosEditorRuntimeEvent::RequestRepaint {
                viewport_id: info.viewport_id,
                delay: info.delay,
            });
        });

        Ok(Self {
            window: None,
            event_proxy: proxy,
            render_pipeline: Arc::new(Mutex::new(None)),
            kairos_engine: KairosEngine::new(&egui_ctx)?,
            egui_ctx,
            egui_state: None,
            repaint_at: None,
            frame_rate_counter: FrameRateCounter::new(),
            // frame_start: Instant::now(),
            min_frame_interval: Duration::from_secs_f64(1.0 / 60.0),
            didi_exit: false,
        })
    }

    #[cfg(target_os = "macos")]
    fn set_macos_dock_icon(path: impl AsRef<std::path::Path>) {
        use objc2::{AnyThread as _, MainThreadMarker};
        use objc2_app_kit::{NSApplication, NSImage};
        use objc2_foundation::NSString;

        let Some(mtm) = MainThreadMarker::new() else {
            log::warn!("Cannot set macOS Dock icon outside the main thread");
            return;
        };

        let path = path.as_ref();
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let path = path.to_string_lossy();
        let path = NSString::from_str(&path);

        let Some(image) = NSImage::initWithContentsOfFile(NSImage::alloc(), &path) else {
            log::warn!("Failed to load macOS Dock icon from {path}");
            return;
        };

        let app = NSApplication::sharedApplication(mtm);

        // SAFETY: AppKit owns the application object, and `image` is a valid NSImage.
        unsafe { app.setApplicationIconImage(Some(&image)) };
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> RuntimeResult<()> {
        let title = format!("{} {}", consts::APP_NAME, consts::VERSION);
        let icon = load_icon();

        let attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(LogicalSize::new(800.0, 600.0))
            .with_decorations(true)
            .with_transparent(false)
            .with_visible(false)
            .with_window_icon(icon.clone());

        let window = Arc::new(event_loop.create_window(attrs)?);
        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::WindowExtWindows;
            window.set_taskbar_icon(icon);
        }

        #[cfg(target_os = "macos")]
        {
            Self::set_macos_dock_icon(paths::PATH_ENGINE_ICON);
        }

        let mut egui_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            ViewportId::ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );

        let settings = load_engine_settings()?;
        let render_pipeline = pollster::block_on(RenderPipeline::new(
            window.clone(),
            &settings.texture_compression,
            &mut self.kairos_engine.engine.assets_server,
        ))?;
        let render_pipeline_event_proxy = self.event_proxy.clone();
        render_pipeline
            .device
            .set_device_lost_callback(move |reson, msg| {
                log::error!("GPU device lost ({reson:?}): {msg}");
                render_pipeline_event_proxy
                    .send_event(KairosEditorRuntimeEvent::RenderPipelineCrash)
                    .unwrap();
            });
        render_pipeline
            .device
            .on_uncaptured_error(Arc::new(move |error| {
                log::error!("Uncaptured GPU error: {error}");
            }));

        egui_state.set_max_texture_side(render_pipeline.max_texture_side());

        self.window = Some(window.clone());
        self.render_pipeline.lock().replace(render_pipeline);
        self.egui_state = Some(egui_state);

        window.set_visible(true);

        // Query monitor refresh rate for frame rate limiting.
        // On D3D12 flip-model swapchains, PresentMode::Fifo does not provide
        // CPU-side back-pressure; the application must throttle itself.
        if let Some(monitor) = window.current_monitor() {
            if let Some(mhz) = monitor.refresh_rate_millihertz() {
                let hz = mhz as f64 / 1000.0;
                self.min_frame_interval = Duration::from_secs_f64(1.0 / hz);
            }
        }

        window.request_redraw();

        Ok(())
    }

    fn update_keyboard_input(&mut self, event: KeyEvent) {
        if event.repeat {
            return;
        }
        self.kairos_engine.update_keyboard_input(event)
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };

        self.kairos_engine.update();

        // Process pending asset loads before drawing so they are available
        // this frame (e.g. for widget rect recording).
        self.kairos_engine.handle_asset_server();

        let mut should_close = false;
        let mut repaint_delay = None;
        let mut frame_presented = false;

        {
            let mut render_pipeline = self.render_pipeline.lock();
            let Some(render_pipeline) = render_pipeline.as_mut() else {
                return;
            };

            let mut graphics_commands = Vec::<GraphicsCommand>::new();

            match render_pipeline.get_window_surface() {
                Ok(output) => {
                    let Some(egui_state) = self.egui_state.as_mut() else {
                        return;
                    };

                    let raw_input = egui_state.take_egui_input(&window);

                    let full_output = self.egui_ctx.run_ui(raw_input, |ui| {
                        graphics_commands.append(&mut self.kairos_engine.render_ui());

                        self.kairos_engine.handle_ui(ui);

                        // Process asset loads triggered by handle_ui before draw_ui
                        // needs them (e.g. widget rect recording in inspectors).
                        self.kairos_engine.handle_asset_server();

                        self.kairos_engine.draw_ui(ui);
                    });

                    egui_state.handle_platform_output(&window, full_output.platform_output);

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

                    let pixels_per_point = full_output.pixels_per_point;

                    let clipped_primitives = self
                        .egui_ctx
                        .tessellate(full_output.shapes, pixels_per_point);

                    let screen_descriptor = egui_wgpu::ScreenDescriptor {
                        size_in_pixels: [
                            render_pipeline.surface_config.width,
                            render_pipeline.surface_config.height,
                        ],
                        pixels_per_point,
                    };

                    let mut egui_graphics_command = GraphicsCommand::new(2, 2, 0, 4);
                    let frame_buffer_attachment = Attachment::from_internal_id(
                        InternalAttachmentId::FrameBufferColorAttachment,
                    );
                    let frame_buffer_attachment_id =
                        egui_graphics_command.create_color_attachment(frame_buffer_attachment);
                    let frame_buffer_attachment_bind = ColorAttachmentBind::new(
                        frame_buffer_attachment_id,
                        AttachmentLoadAction::LoadClear,
                        AttachmentStoreAction::Store,
                    );
                    let vp_id =
                        egui_graphics_command.set_view_projection_matrix(float4x4::IDENTITY);
                    egui_graphics_command.begin_render_pass(
                        Some("Egui Graphics Render Pass"),
                        vec![frame_buffer_attachment_bind],
                        None,
                        vp_id,
                        0,
                    );

                    egui_graphics_command.draw_egui(
                        clipped_primitives,
                        screen_descriptor,
                        full_output.textures_delta.set,
                    );
                    egui_graphics_command.end_render_pass();
                    egui_graphics_command.output_to_framebuffer(
                        frame_buffer_attachment_id,
                        full_output.textures_delta.free,
                    );

                    graphics_commands.push(egui_graphics_command);

                    let graphics_graph = GraphicsGraph::build(graphics_commands);
                    render_pipeline.present(
                        &mut self.kairos_engine.engine.assets_server,
                        output,
                        graphics_graph,
                    );
                    frame_presented = true;
                }
                Err(error) => match error {
                    wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                        render_pipeline.set_window_resize(window.inner_size());
                    }
                    wgpu::CurrentSurfaceTexture::Occluded => {}
                    wgpu::CurrentSurfaceTexture::Timeout => {
                        println!("wgpu surface timeout");
                    }
                    wgpu::CurrentSurfaceTexture::Validation => {
                        println!("wgpu surface validation error");
                    }
                    wgpu::CurrentSurfaceTexture::Success(_)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(_) => {
                        unreachable!("success variants are returned from Ok branch")
                    }
                },
            };
        }

        if frame_presented {
            self.frame_rate_counter.record_frame();
            // Throttle to monitor refresh rate. D3D12 flip-model swapchains
            // do not block on get_current_texture(), so vsync does not provide
            // CPU back-pressure. We sleep for the remainder of the frame budget.
            // let elapsed = self.frame_start.elapsed();
            // if elapsed < self.min_frame_interval {
            //     std::thread::sleep(self.min_frame_interval - elapsed);
            // }
            // self.frame_start = Instant::now();
        }
        window.request_redraw();

        if should_close {
            self.shutdown(event_loop);
            return;
        }

        if let Some(delay) = repaint_delay {
            self.set_repaint_delay_from_output(delay);
        }

        self.kairos_engine.handle_asset_server();
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

        let now = Instant::now();

        if now >= repaint_at {
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
        if !self.didi_exit {
            self.kairos_engine.on_exit();
            self.didi_exit = true;
        }

        event_loop.exit();
    }
}

impl ApplicationHandler<KairosEditorRuntimeEvent> for KairosEditorRuntime {
    // create the window
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

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: KairosEditorRuntimeEvent) {
        match event {
            KairosEditorRuntimeEvent::RequestRepaint { viewport_id, delay } => {
                if viewport_id == ViewportId::ROOT {
                    self.queue_repaint_after(delay);
                }
            }
            KairosEditorRuntimeEvent::RenderPipelineCrash => {
                println!("KairosEditorRuntimeEvent::RenderPipelineCrash");
                self.shutdown(event_loop);
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };

        if window.id() != window_id {
            return;
        }

        if let Some(egui_state) = self.egui_state.as_mut() {
            let response = egui_state.on_window_event(&window, &event);
            if response.repaint {
                window.request_redraw();
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                self.shutdown(event_loop);
            }
            WindowEvent::Resized(physical_size) => {
                if let Some(render_pipeline) = self.render_pipeline.lock().as_mut() {
                    render_pipeline.set_window_resize(physical_size);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(render_pipeline) = self.render_pipeline.lock().as_mut() {
                    render_pipeline.set_window_resize(window.inner_size());
                }
            }
            WindowEvent::RedrawRequested => {
                self.redraw(event_loop);
            }
            WindowEvent::KeyboardInput {
                device_id: _,
                event,
                is_synthetic: _,
            } => {
                self.update_keyboard_input(event);
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
