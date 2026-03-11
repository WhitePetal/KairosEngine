mod kairos_dialog;


use egui::{Color32, FontFamily, FontId, Rect, UiBuilder, Vec2, Widget};
use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::WindowId,
    window::WindowAttributes,
};

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap_or_else(|error| {
        kairos_dialog::error_message_window("Init Failed", &format!("Create EventLoop Failed, error info:\n {error:?}"));
        panic!("Create EventLoop Failed");
    });
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap_or_else(|error| {
        kairos_dialog::error_message_window("Init Failed", &format!("Run App Failed, error info:\n {error:?}"));
        panic!("Run App Failed");
    });
}

#[derive(Default)]
struct App {
    window: Option<winit::window::Window>,
    window_id: Option<WindowId>,
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = match event_loop.create_window(
            WindowAttributes::default()
            .with_title("KairosEngine")
            .with_decorations(false)
        )
        {
            Ok(w) => w,
            Err(err) => {
                log::error!("create_window failed: {err:#}");
                event_loop.exit();
                return;
            }
        };

        let state = match pollster::block_on(State::new(&window)) {
            Ok(s) => s,
            Err(err) => {
                log::error!("State::new failed: {err:#}");
                event_loop.exit();
                return;
            }
        };

        self.window_id = Some(window.id());
        self.window = Some(window);
        self.state = Some(state);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        event_loop.set_control_flow(ControlFlow::Wait);

        if self.window_id != Some(window_id) {
            return;
        }

        let Some(window) = self.window.as_ref() else {
            return;
        };
        let Some(state) = self.state.as_mut() else {
            return;
        };

        state.platform.handle_event(&event);
        if state.platform.captures_event(&event) {
            window.request_redraw();
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.resize(size.width, size.height);
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if let Err(err) = state.render(window) {
                    log::error!("render error: {err:#}");
                }
            }
            _ => {}
        }
    }
}

struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    surface_format: wgpu::TextureFormat,

    platform: Platform,
    egui_rpass: RenderPass,
    screen_descriptor: ScreenDescriptor,
    start_time: std::time::Instant,
}

impl State {
    async fn new(window: &winit::window::Window) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let scale_factor_f64 = window.scale_factor();
        let scale_factor_f32 = scale_factor_f64 as f32;

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window)?;
        // `wgpu::Surface` 与 `Window` 生命周期绑定；这里窗口会被 move 进 `event_loop.run`
        // 的闭包并活到程序结束，所以把它提升为 'static 是安全的。
        let surface: wgpu::Surface<'static> = unsafe { std::mem::transmute(surface) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("wgpu device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                    trace: wgpu::Trace::Off,
                },
            )
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let platform = Platform::new(PlatformDescriptor {
            physical_width: surface_config.width,
            physical_height: surface_config.height,
            scale_factor: scale_factor_f64,
            font_definitions: egui::FontDefinitions::default(),
            style: Default::default(),
        });

        let egui_rpass = RenderPass::new(&device, surface_format, 1);

        let screen_descriptor = ScreenDescriptor {
            physical_width: surface_config.width,
            physical_height: surface_config.height,
            scale_factor: scale_factor_f32,
        };

        Ok(Self {
            surface,
            device,
            queue,
            surface_config,
            surface_format,
            platform,
            egui_rpass,
            screen_descriptor,
            start_time: std::time::Instant::now(),
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);

        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);

        self.screen_descriptor.physical_width = width;
        self.screen_descriptor.physical_height = height;
    }

    fn render(&mut self, window: &winit::window::Window) -> anyhow::Result<()> {
        println!("render");
        self.platform
            .update_time(self.start_time.elapsed().as_secs_f64());

        self.platform.begin_pass();
        let ctx = self.platform.context();

        egui::TopBottomPanel::top("top").show(&ctx, |ui| {
            self.draw_title_bar(ui);
            ui.heading("Hello egui");
        });
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.label(String::from("This is a Default Window Example base egui_wgpu_backend + winit."));
        });

        let full_output = self.platform.end_pass(Some(window));
        let paint_jobs = ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

        let output_frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok(()),
            Err(wgpu::SurfaceError::Other) => return Ok(()),
            Err(wgpu::SurfaceError::OutOfMemory) => {
                return Err(anyhow::anyhow!("wgpu surface out of memory"))
            }
        };

        let view = output_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            drop(rpass);
        }

        self.egui_rpass
            .add_textures(&self.device, &self.queue, &full_output.textures_delta)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        self.egui_rpass
            .update_buffers(&self.device, &self.queue, &paint_jobs, &self.screen_descriptor);
        self.egui_rpass
            .execute(
                &mut encoder,
                &view,
                &paint_jobs,
                &self.screen_descriptor,
                None,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        self.egui_rpass
            .remove_textures(full_output.textures_delta)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        self.queue.submit([encoder.finish()]);
        output_frame.present();

        Ok(())
    }

    fn draw_title_bar(&mut self, ui: &mut egui::Ui) {
        // 1. 设置顶部栏样式 (背景色、尺寸、圆角)
        let title_bar_height = 32.0;
        let title_bar_rect = Rect::from_min_size(
            ui.min_rect().min, 
            Vec2::new(ui.available_width(), title_bar_height)
        );

        // 绘制顶部栏背景
        ui.painter().rect_filled(
            title_bar_rect, 
            0.0, // 圆角 (0.0 为直角)
            Color32::from_rgb(40, 40, 40)
        );

        ui.scope_builder(
            UiBuilder::new().id_salt("title_bar"), 
            |ui| {
                ui.set_height(title_bar_height);
                ui.horizontal(|ui| {
                    // 左对齐: 窗口标题
                    ui.add_space(8.0); // 左边距
                    ui.label(egui::RichText::new("Custome Title Bar Example")
                        .font(FontId::new(14.0, FontFamily::Proportional))
                        .color(Color32::WHITE));
                    
                    // 右对齐: 控制按钮 (最小化、关闭)
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center), 
                        |ui| {
                        // 最小化按钮
                        if ui.button("-").clicked()
                        {
                                
                        }
                    })
                }
            )}
        );
    }
}