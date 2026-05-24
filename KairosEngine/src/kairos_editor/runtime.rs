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

use crate::{
    asset_loader::texture::TextureAssets,
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
    egui_renderer: Option<egui_wgpu::Renderer>,
    engine: KairosEngine,
    texture_assets: TextureAssets,
    repaint_at: Option<Instant>,
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

        let texture_assets = TextureAssets::new(256);

        Ok(Self {
            window: None,
            event_proxy: proxy,
            render_pipeline: Arc::new(Mutex::new(None)),
            egui_ctx,
            egui_state: None,
            egui_renderer: None,
            engine: KairosEngine::new()?,
            texture_assets,
            repaint_at: None,
            didi_exit: false,
        })
    }

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

        let render_pipeline = pollster::block_on(RenderPipeline::new(
            window.clone(),
            &mut self.texture_assets,
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
        let egui_renderer = egui_wgpu::Renderer::new(
            &render_pipeline.device,
            render_pipeline.surface_config.format,
            egui_wgpu::RendererOptions::default(),
        );

        self.window = Some(window.clone());
        self.render_pipeline.lock().replace(render_pipeline);
        self.egui_state = Some(egui_state);
        self.egui_renderer = Some(egui_renderer);

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

        let render_error = {
            let mut render_pipeline = self.render_pipeline.lock();
            let Some(render_pipeline) = render_pipeline.as_mut() else {
                return;
            };

            match render_pipeline.get_window_surface() {
                Ok((output, view, mut encoder)) => {
                    let Some(egui_state) = self.egui_state.as_mut() else {
                        return;
                    };

                    let Some(egui_renderer) = self.egui_renderer.as_mut() else {
                        return;
                    };

                    let raw_input = egui_state.take_egui_input(&window);
                    let full_output = self.egui_ctx.run_ui(raw_input, |ui| {
                        self.engine
                            .draw_ui(ui, render_pipeline, &mut encoder, egui_renderer);
                        self.engine.handle_ui(ui);
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

                    for (id, image_delta) in &full_output.textures_delta.set {
                        egui_renderer.update_texture(
                            &render_pipeline.device,
                            &render_pipeline.queue,
                            *id,
                            &image_delta,
                        );
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

                    let user_cmd_buffers = egui_renderer.update_buffers(
                        &render_pipeline.device,
                        &render_pipeline.queue,
                        &mut encoder,
                        &clipped_primitives,
                        &screen_descriptor,
                    );

                    {
                        let mut render_pass = render_pipeline
                            .render(&mut encoder, &view)
                            .forget_lifetime();

                        egui_renderer.render(
                            &mut render_pass,
                            &clipped_primitives,
                            &screen_descriptor,
                        );
                    }

                    render_pipeline.queue.submit(
                        user_cmd_buffers
                            .into_iter()
                            .chain(std::iter::once(encoder.finish())),
                    );

                    output.present();

                    for id in &full_output.textures_delta.free {
                        egui_renderer.free_texture(id);
                    }

                    window.request_redraw();
                    None
                }
                Err(error) => Some(error),
            }
        };

        if should_close {
            self.shutdown(event_loop);
            return;
        }

        if let Some(delay) = repaint_delay {
            self.set_repaint_delay_from_output(delay);
        }

        if let Some(render_error) = render_error {
            match render_error {
                wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                    if let Some(render_pipeline) = self.render_pipeline.lock().as_mut() {
                        render_pipeline.set_window_resize(window.inner_size());
                    }
                    window.request_redraw();
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
            }
        }

        self.texture_assets.handle_recves();
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
