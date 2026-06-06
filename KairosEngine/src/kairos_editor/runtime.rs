use std::path::Path;
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
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy},
    window::{Icon, Window},
};

use crate::asset_loader::assets::{
    AssetsServer, MaterialAssetsSystem, MeshAssetsSystem, ShaderAssetsSystem, TextureAssetsSystem,
};
use crate::graphics::attachment::{Attachment, InternalAttachmentId};
use crate::graphics::graphics_graph::{GraphicsCommand, GraphicsGraph};
use crate::math::float4x4;
use crate::{
    graphics::render_pipeline::RenderPipeline,
    kairos_dialog,
    kairos_editor::{KairosEngine, consts, ui::paths},
};

fn load_icon() -> Option<Icon> {
    let bytes = std::fs::read(paths::PATH_ENGINE_ICON).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();

    Icon::from_rgba(image.into_raw(), width, height).ok()
}

type RuntimeResult<T> = Result<T, Box<dyn Error>>;

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
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    engine: KairosEngine,
    assets_server: AssetsServer,
    repaint_at: Option<Instant>,
    frame_rate_counter: FrameRateCounter,
    didi_exit: bool,
}

impl KairosEditorRuntime {
    pub fn new(proxy: EventLoopProxy<KairosEditorRuntimeEvent>) -> RuntimeResult<Self> {
        let egui_ctx = egui::Context::default();
        egui_extras::install_image_loaders(&egui_ctx);

        let egui_event_proxy = proxy.clone();
        egui_ctx.set_request_repaint_callback(move |info| {
            let _ = egui_event_proxy.send_event(KairosEditorRuntimeEvent::RequestRepaint {
                viewport_id: info.viewport_id,
                delay: info.delay,
            });
        });

        let mut assets_server = AssetsServer::new();
        assets_server.push(TextureAssetsSystem::new());
        assets_server.push(ShaderAssetsSystem::new());
        assets_server.push(MaterialAssetsSystem::new());
        assets_server.push(MeshAssetsSystem::new());

        Ok(Self {
            window: None,
            event_proxy: proxy,
            render_pipeline: Arc::new(Mutex::new(None)),
            egui_ctx,
            egui_state: None,
            engine: KairosEngine::new()?,
            assets_server,
            repaint_at: None,
            frame_rate_counter: FrameRateCounter::new(),
            didi_exit: false,
        })
    }

    #[cfg(target_os = "macos")]
    fn set_macos_dock_icon(path: impl AsRef<Path>) {
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

        let render_pipeline = pollster::block_on(RenderPipeline::new(window.clone()))?;
        let render_pipeline_event_proxy = self.event_proxy.clone();
        render_pipeline
            .device
            .set_device_lost_callback(move |reson, msg| {
                log::error!("GPU device lost ({reson:?}): {msg}");
                render_pipeline_event_proxy
                    .send_event(KairosEditorRuntimeEvent::RenderPipelineCrash)
                    .unwrap();
            });
        let render_pipeline_event_proxy = self.event_proxy.clone();
        render_pipeline
            .device
            .on_uncaptured_error(Arc::new(move |error| {
                log::error!("Gpu crash: {error}");
                render_pipeline_event_proxy
                    .send_event(KairosEditorRuntimeEvent::RenderPipelineCrash)
                    .unwrap();
            }));

        egui_state.set_max_texture_side(render_pipeline.max_texture_side());

        self.window = Some(window.clone());
        self.render_pipeline.lock().replace(render_pipeline);
        self.egui_state = Some(egui_state);

        window.set_visible(true);
        window.request_redraw();

        Ok(())
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };

        self.engine.update();

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
                        graphics_commands
                            .append(&mut self.engine.render_ui(&mut self.assets_server));

                        self.engine.handle_ui(&mut self.assets_server, ui);

                        self.engine.draw_ui(ui);
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
                        InternalAttachmentId::FrameBuffer_ColorAttachment,
                    );
                    let frame_buffer_attachment_id =
                        egui_graphics_command.create_color_attachment(frame_buffer_attachment);
                    let vp_id =
                        egui_graphics_command.set_view_projection_matrix(float4x4::idenity());
                    egui_graphics_command.begin_render_pass(
                        Some("Egui Graphics Render Pass"),
                        vec![frame_buffer_attachment_id],
                        None,
                        vp_id,
                        0,
                        false,
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
                    render_pipeline.present(&mut self.assets_server, output, graphics_graph);
                    frame_presented = true;
                }
                Err(error) => match error {
                    wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                        render_pipeline.set_window_resize(window.inner_size());
                    }
                    wgpu::CurrentSurfaceTexture::Occluded => {}
                    wgpu::CurrentSurfaceTexture::Timeout => {
                        log::warn!("wgpu surface timeout");
                    }
                    wgpu::CurrentSurfaceTexture::Validation => {
                        log::warn!("wgpu surface validation error");
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
        }

        window.request_redraw();

        if should_close {
            self.shutdown(event_loop);
            return;
        }

        if let Some(delay) = repaint_delay {
            self.set_repaint_delay_from_output(delay);
        }

        self.assets_server.handle();
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
            self.engine.on_exit();
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
