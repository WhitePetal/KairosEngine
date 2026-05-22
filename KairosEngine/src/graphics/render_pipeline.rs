use std::{error::Error, sync::Arc};

use wgpu::{
    Adapter, BackendOptions, Backends, BlendState, Buffer, BufferUsages, ColorTargetState,
    ColorWrites, CommandEncoder, CommandEncoderDescriptor, CurrentSurfaceTexture, Device,
    ExperimentalFeatures, Extent3d, Face, Features, FragmentState, FrontFace, InstanceFlags,
    Limits, LoadOp, MemoryBudgetThresholds, MemoryHints, MultisampleState, Operations,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PolygonMode, PowerPreference,
    PresentMode, PrimitiveState, PrimitiveTopology, Queue, RenderPass, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor,
    ShaderSource, StoreOp, Surface, SurfaceConfiguration, SurfaceTexture, TextureUsages,
    TextureView, TextureViewDescriptor, Trace, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexState, VertexStepMode,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::{DeviceDescriptor, TextureDescriptor},
};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{graphics::vertex::Vertex, math::float4};

pub struct RenderPipeline {
    window: Arc<Window>,
    pub device: Device,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,
    pub adapter: Adapter,
    pub queue: Queue,
    window_size: PhysicalSize<u32>,
    window_size_changed: bool,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: Buffer,
    indices_buffer: Buffer,
    indices_num: u32,
}

impl RenderPipeline {
    pub async fn new(window: Arc<Window>) -> Result<Self, Box<dyn Error>> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: Backends::all(),
            flags: InstanceFlags::default(),
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
            backend_options: BackendOptions::from_env_or_default(),
            display: None,
        });
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await?;

        let features = adapter.features();
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: None,
                required_features: Features::empty(),
                required_limits: Limits::default(),
                experimental_features: ExperimentalFeatures::default(),
                memory_hints: MemoryHints::default(),
                trace: Trace::default(),
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let window_size = window.inner_size();
        let width = window_size.width;
        let height = window_size.height;
        println!("surface_caps.formats: {:?}", surface_caps.formats);
        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_caps.formats[0],
            width,
            height,
            present_mode: PresentMode::Fifo,
            desired_maximum_frame_latency: 3,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shader"),
            source: ShaderSource::Wgsl(include_str!("../../res/shaders/shader.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        const VERTICES: &[Vertex] = &[
            Vertex {
                position: float4::new(-0.0868241, 0.49240386, 0.0, 1.0),
                color: float4::new(0.5, 0.0, 0.5, 0.0),
            }, // A
            Vertex {
                position: float4::new(-0.49513406, 0.06958647, 0.0, 1.0),
                color: float4::new(0.5, 0.0, 0.5, 0.0),
            }, // B
            Vertex {
                position: float4::new(-0.21918549, -0.44939706, 0.0, 1.0),
                color: float4::new(0.5, 0.0, 0.5, 0.0),
            }, // C
            Vertex {
                position: float4::new(0.35966998, -0.3473291, 0.0, 1.0),
                color: float4::new(0.5, 0.0, 0.5, 0.0),
            }, // D
            Vertex {
                position: float4::new(0.44147372, 0.2347359, 0.0, 1.0),
                color: float4::new(0.5, 0.0, 0.5, 0.0),
            }, // E
        ];
        const INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4];

        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: BufferUsages::VERTEX,
        });
        let vertex_buffer_layout = VertexBufferLayout {
            array_stride: core::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    format: VertexFormat::Float32x4,
                    shader_location: 0,
                },
                VertexAttribute {
                    offset: core::mem::size_of::<float4>() as wgpu::BufferAddress,
                    format: VertexFormat::Float32x4,
                    shader_location: 1,
                },
            ],
        };

        let indices_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Indices Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: BufferUsages::INDEX,
        });
        let indices_num = INDICES.len() as u32;

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[vertex_buffer_layout],
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format: surface_config.format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::all(),
                })],
            }),
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            window,
            device,
            surface,
            surface_config,
            adapter,
            queue,
            window_size,
            window_size_changed: false,
            pipeline,
            vertex_buffer,
            indices_buffer,
            indices_num,
        })
    }

    pub fn get_window_surface(
        &mut self,
    ) -> Result<(SurfaceTexture, TextureView, CommandEncoder), CurrentSurfaceTexture> {
        if self.window_size_changed {
            self.resize_surface();
        }

        match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(output) | CurrentSurfaceTexture::Suboptimal(output) => {
                let view = output
                    .texture
                    .create_view(&TextureViewDescriptor::default());
                let encoder = self
                    .device
                    .create_command_encoder(&CommandEncoderDescriptor {
                        label: Some("RenderPipeline Command Encoder"),
                    });
                Ok((output, view, encoder))
            }
            err => Err(err),
        }
    }

    pub fn create_render_target(&mut self, label: &str, width: u32, height: u32) -> TextureView {
        // match self.scene_view.clone() {
        //     Some(scene_view) => {
        //         scene_view
        //     },
        //     None => {
        let texture = self.device.create_texture(&TextureDescriptor {
            label: Some(label),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.surface_config.format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        texture.create_view(&TextureViewDescriptor::default())
        // },
        // }
    }

    pub fn render<'encoder>(
        &mut self,
        encoder: &'encoder mut CommandEncoder,
        view: &TextureView,
    ) -> RenderPass<'encoder> {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: StoreOp::Store,
                },
            })],
            ..Default::default()
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.indices_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.indices_num, 0, 0..1);

        render_pass
    }

    pub fn submit(&self, output: SurfaceTexture, encoder: CommandEncoder) {
        self.queue.submit(Some(encoder.finish()));
        output.present();
    }

    pub fn set_window_resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        self.window_size = size;
        self.window_size_changed = true;
    }

    pub fn resize_surface(&mut self) {
        self.surface_config.width = self.window_size.width;
        self.surface_config.height = self.window_size.height;
        self.surface.configure(&self.device, &self.surface_config);
        self.window_size_changed = false;
    }

    pub fn max_texture_side(&self) -> usize {
        self.device.limits().max_texture_dimension_2d as usize
    }
}
