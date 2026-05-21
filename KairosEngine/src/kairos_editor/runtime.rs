use std::{
    error::Error,
    io,
    sync::Arc,
    time::{Duration, Instant},
};

use egui::{ViewportCommand, ViewportId};
use parking_lot::Mutex;
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalSize},
    event::{KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy},
    window::{Icon, Window},
};

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

#[derive(Debug, Clone)]
pub enum KairosEditorRuntimeEvent {
    RequestRepaint {
        viewport_id: ViewportId,
        delay: Duration,
    },
}

pub struct KairosEditorRuntime {
    window: Option<Arc<Window>>,
    render_pipeline: Arc<Mutex<Option<RenderPipeline>>>,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    egui_renderer: Option<egui_wgpu::Renderer>,
    engine: KairosEngine,
    repaint_at: Option<Instant>,
    didi_exit: bool,
}

impl KairosEditorRuntime {
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
            window: None,
            render_pipeline: Arc::new(Mutex::new(None)),
            egui_ctx,
            egui_state: None,
            egui_renderer: None,
            engine: KairosEngine::new()?,
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

        let render_pipeline = pollster::block_on(RenderPipeline::new(window.clone()))?;
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

    fn draw_egui(
        render_pipeline: &RenderPipeline,
        egui_renderer: &mut egui_wgpu::Renderer,
        textures_delta: &egui::TexturesDelta,
        clipped_primitives: &[egui::ClippedPrimitive],
        pixels_per_point: f32,
    ) -> Result<(), wgpu::SurfaceError> {
        for (id, image_delta) in &textures_delta.set {
            egui_renderer.update_texture(
                &render_pipeline.device,
                &render_pipeline.queue,
                *id,
                &image_delta,
            );
        }

        let output_frame = render_pipeline.surface.get_current_texture()?;
        let target_view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [
                render_pipeline.surface_config.width,
                render_pipeline.surface_config.height,
            ],
            pixels_per_point,
        };

        let mut encoder =
            render_pipeline
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("KairosEditor egui command encoder"),
                });

        let user_cmd_buffers = egui_renderer.update_buffers(
            &render_pipeline.device,
            &render_pipeline.queue,
            &mut encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("KairosEditor egui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.015,
                            g: 0.015,
                            b: 0.018,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            let mut render_pass = render_pass.forget_lifetime();
            egui_renderer.render(&mut render_pass, clipped_primitives, &screen_descriptor);
        }

        render_pipeline.queue.submit(
            user_cmd_buffers
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );

        output_frame.present();

        for id in &textures_delta.free {
            egui_renderer.free_texture(id);
        }

        Ok(())
    }

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
        let render_result = {
            let mut render_pipeline = self.render_pipeline.lock();
            let Some(render_pipeline) = render_pipeline.as_mut() else {
                return;
            };
            render_pipeline.render()
        };
        self.handle_render_error(event_loop, window.clone(), render_result);

        if let Some(delay) = repaint_delay {
            self.set_repaint_delay_from_output(delay);
        }

        // let draw_egui_result = {
        //     let mut render_pipeline_gurad = self.render_pipeline.lock();
        //     let Some(render_pipeline) = render_pipeline_gurad.as_mut() else {
        //         return;
        //     };
        //     let Some(egui_renderer) = self.egui_renderer.as_mut() else {
        //         return;
        //     };

        //     let clipped_primitives = self
        //         .egui_ctx
        //         .tessellate(full_output.shapes, full_output.pixels_per_point);

        //     Self::draw_egui(
        //         render_pipeline,
        //         egui_renderer,
        //         &full_output.textures_delta,
        //         &clipped_primitives,
        //         full_output.pixels_per_point,
        //     )
        // };

        // self.handle_render_error(event_loop, window.clone(), draw_egui_result);
        window.request_redraw();
    }

    fn handle_render_error(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
        result: Result<(), wgpu::SurfaceError>,
    ) {
        match result {
            Ok(()) => {}
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                if let Some(render_pipeline) = self.render_pipeline.lock().as_mut() {
                    render_pipeline.set_window_resize(window.inner_size());
                }
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

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: KairosEditorRuntimeEvent) {
        match event {
            KairosEditorRuntimeEvent::RequestRepaint { viewport_id, delay } => {
                if viewport_id == ViewportId::ROOT {
                    self.queue_repaint_after(delay);
                }
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
