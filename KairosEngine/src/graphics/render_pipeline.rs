use std::{error::Error, sync::Arc};

use petgraph::visit::{DfsEvent, Reversed, depth_first_search};
use wgpu::{
    Adapter, AddressMode, BackendOptions, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry,
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType,
    BlendState, BufferUsages, ColorTargetState, ColorWrites, CommandBuffer, CommandEncoder,
    CommandEncoderDescriptor, CurrentSurfaceTexture, Device, ExperimentalFeatures, Extent3d, Face,
    Features, FilterMode, FragmentState, FrontFace, InstanceFlags, Limits, LoadOp,
    MemoryBudgetThresholds, MemoryHints, MipmapFilterMode, MultisampleState, Operations, Origin3d,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PolygonMode, PowerPreference,
    PresentMode, PrimitiveState, PrimitiveTopology, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipelineDescriptor, RequestAdapterOptions, SamplerBindingType,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, Surface, SurfaceConfiguration,
    SurfaceTexture, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureFormat, TextureSampleType,
    TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension, Trace,
    VertexAttribute, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::{DeviceDescriptor, SamplerDescriptor, TextureDescriptor},
};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    asset_loader::texture::TextureAssets,
    graphics::{
        attachment::{Attachment, AttachmentFormat, InternalAttachmentId},
        graphics_graph::{GraphicsGraph, RenderPassNode},
        vertex::Vertex,
    },
    math::{float4, float4x4},
};

pub struct RenderPipeline {
    window: Arc<Window>,
    pub device: Device,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,
    pub adapter: Adapter,
    pub queue: Queue,
    encoder: Option<CommandEncoder>,
    egui_renderer: egui_wgpu::Renderer,
    internal_texture_views: Vec<Option<TextureView>>,
    window_size: PhysicalSize<u32>,
    window_size_changed: bool,
    texture_bind_group_layout: BindGroupLayout,
    texture_bind_group: BindGroup,
}

impl RenderPipeline {
    pub async fn new(
        window: Arc<Window>,
        texture_assets: &mut TextureAssets,
    ) -> Result<Self, Box<dyn Error>> {
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

        let _ = adapter.features();
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

        let texture_handle = texture_assets
            .load("res/textures/kairos_texture.texture".into())
            .await?;
        let texture_handle = texture_handle.as_ref();
        let texture_asset = &texture_assets.get(texture_handle).ok_or("no data")?.texture;
        let texture_data = &texture_asset.data;

        // let texture_dimension = texture_asset.dimensions();
        let texture_dimension = (texture_asset.width, texture_asset.height);
        let texture_size = Extent3d {
            width: texture_dimension.0,
            height: texture_dimension.1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Kairos Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &texture_data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * texture_dimension.0),
                rows_per_image: Some(texture_dimension.1),
            },
            texture_size,
        );
        let texture_view = texture.create_view(&TextureViewDescriptor::default());
        let texture_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Texture Sampler"),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Nearest,
            mipmap_filter: MipmapFilterMode::Linear,
            lod_min_clamp: 0f32,
            lod_max_clamp: 0f32,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });
        let texture_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Texture Bind Group Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let texture_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&texture_sampler),
                },
            ],
        });

        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            surface_config.format,
            egui_wgpu::RendererOptions::default(),
        );

        Ok(Self {
            window,
            device,
            surface,
            surface_config,
            adapter,
            queue,
            encoder: None,
            egui_renderer,
            internal_texture_views: vec![None; InternalAttachmentId::End as usize],
            window_size,
            window_size_changed: false,
            texture_bind_group_layout,
            texture_bind_group,
        })
    }

    pub fn get_window_surface(&mut self) -> Result<SurfaceTexture, CurrentSurfaceTexture> {
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
                self.encoder = Some(encoder);
                self.internal_texture_views
                    [InternalAttachmentId::FrameBuffer_ColorAttachment as usize] = Some(view);
                Ok(output)
            }
            err => Err(err),
        }
    }

    pub fn present(&mut self, output: SurfaceTexture, graphics_graph: GraphicsGraph) {
        let Some(mut encoder) = self.encoder.take() else {
            return;
        };
        
        let attachments = graphics_graph.attachments;
        let mut attachment_views = Vec::with_capacity(attachments.len());
        attachment_views.resize(attachments.len(), None);
        let vps = graphics_graph.vps;
        let free_egui_textures = graphics_graph.free_egui_textures;

        let ending_nodes = graphics_graph.ending_nodes;
        let mut graph = graphics_graph.graph;

        let mut nodes_stack = Vec::with_capacity(graph.node_count());

        depth_first_search(Reversed(&graph), ending_nodes, |event| {
            if let DfsEvent::Discover(node, _) = event {
                nodes_stack.push(node);
            }
        });

        let mut more_command_buffers = Vec::new();
        let mut egui_free_textures = None;

        while let Some(node) = nodes_stack.pop() {
            let Some(node) = graph.remove_node(node) else {
                continue;
            };
            match node {
                super::graphics_graph::GraphNode::None => {}
                super::graphics_graph::GraphNode::RenderPass(render_pass_node) => {
                    let mut command_buffers = Self::handle_render_pass_node(
                        &self.internal_texture_views,
                        &self.device,
                        &self.queue,
                        &mut encoder,
                        &mut self.egui_renderer,
                        self.surface_config.format,
                        &self.texture_bind_group_layout,
                        &self.texture_bind_group,
                        &attachments,
                        &vps,
                        &mut attachment_views,
                        &render_pass_node,
                    );
                    if let Some(command_buffers) = &mut command_buffers {
                        more_command_buffers.append(command_buffers);
                    }
                }
                super::graphics_graph::GraphNode::OutputToFrameBuffer(
                    output_to_frame_buffer_node,
                ) => {
                    let _ = std::mem::replace(&mut egui_free_textures, Some(output_to_frame_buffer_node.egui_free_textures));
                }
                super::graphics_graph::GraphNode::BindAttachmentToEgui(
                    mut bind_attachment_to_egui_node,
                ) => {
                    let Some(Some(view)) = attachment_views
                        .get(bind_attachment_to_egui_node.attachment_id)
                        .as_ref() else {
                            unreachable!()
                        };
                    let rt_id = self.egui_renderer.register_native_texture(
                        &self.device,
                        view,
                        wgpu::FilterMode::Linear,
                    );
                    if let Some(sender) = bind_attachment_to_egui_node.sender.take() {
                        let _ = sender.send(rt_id);
                    }
                }
                super::graphics_graph::GraphNode::CopyAttachmentToEGui(
                    _copy_attachment_to_egui_node,
                ) => {}
                super::graphics_graph::GraphNode::FreeEguiTextureId(_) => unreachable!(),
            }
        }

        if more_command_buffers.len() > 0 {
            self.queue.submit(
                more_command_buffers
                    .into_iter()
                    .chain(std::iter::once(encoder.finish())),
            );
        } else {
            self.queue.submit(Some(encoder.finish()));
        }

        output.present();
        
        if let Some(egui_free_textures) = egui_free_textures {
            for id in &egui_free_textures {
                self.egui_renderer.free_texture(id);
            }
        }
        for id in &free_egui_textures {
            self.egui_renderer.free_texture(id);
        }
    }

    fn handle_render_pass_node(
        internal_texture_views: &Vec<Option<TextureView>>,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        egui_renderer: &mut egui_wgpu::Renderer,
        surface_format: TextureFormat,
        texture_bind_group_layout: &BindGroupLayout,
        texture_bind_group: &BindGroup,
        attachments: &Vec<Attachment>,
        vps: &Vec<float4x4>,
        color_attachment_views: &mut Vec<Option<TextureView>>,
        render_pass_node: &RenderPassNode,
    ) -> Option<Vec<CommandBuffer>> {
        let attachment_ids = &render_pass_node.attachments;
        let mut color_attachment_format = surface_format;

        for a_id in attachment_ids {
            let attachment = &attachments[*a_id];

            // 绑了 internal id 的，就找有没有internal texture view，有则渲染到internal
            // TODO: 获取没有 internal texture view 时，我这里应该创建？
            if let Some(internal_attachement_id) = attachment.bind_internal_id {
                if let Some(internal_texture_view) =
                    internal_texture_views.get(internal_attachement_id as usize)
                    && let Some(internal_texture_view) = internal_texture_view
                {
                    let internal_texture_view = internal_texture_view.clone();
                    color_attachment_format = internal_texture_view.texture().format();
                    let _ = std::mem::replace(
                        &mut color_attachment_views[*a_id],
                        Some(internal_texture_view),
                    );
                }
            } else {
                // create texture
                // 先写 再读
                let texture = device.create_texture(&TextureDescriptor {
                    label: attachment.label,
                    size: Extent3d {
                        width: attachment.width,
                        height: attachment.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: attachment.format.into(),
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let view = texture.create_view(&TextureViewDescriptor::default());
                let _ = std::mem::replace(
                    &mut color_attachment_views[*a_id],
                    Some(view),
                );
                color_attachment_format = texture.format();
            }
        }

        let mut color_attachments: Vec<Option<RenderPassColorAttachment>> =
            Vec::with_capacity(attachment_ids.len());

        for a_id in attachment_ids {
            let view = &color_attachment_views[*a_id];
            let Some(view) = view else {
                continue;
            };
            let render_pass_color_attachment = RenderPassColorAttachment {
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
            };
            color_attachments.push(Some(render_pass_color_attachment));
        }

        let mut render_pass = encoder
            .begin_render_pass(&RenderPassDescriptor {
                label: render_pass_node.label,
                color_attachments: &color_attachments,
                ..Default::default()
            })
            .forget_lifetime();

        // build pipeline
        if let Some(vp) = vps.get(render_pass_node.vp_id) {
            let vp = [
                vp.c0().to_array(),
                vp.c1().to_array(),
                vp.c2().to_array(),
                vp.c3().to_array(),
            ];
            let vp_buffer = device.create_buffer_init(&BufferInitDescriptor {
                label: Some("VP Buffer"),
                contents: bytemuck::cast_slice(&vp),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });
            let vp_bind_group_layout =
                device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: Some("VP Buffer Bind Group Layout"),
                    entries: &[BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });
            let vp_bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("VP Buffer Bind Group"),
                layout: &vp_bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: vp_buffer.as_entire_binding(),
                }],
            });

            let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[Some(texture_bind_group_layout), Some(&vp_bind_group_layout)],
                immediate_size: 0,
            });

            render_pass.set_bind_group(0, texture_bind_group, &[]);
            render_pass.set_bind_group(1, &vp_bind_group, &[]);

            let shader = device.create_shader_module(ShaderModuleDescriptor {
                label: Some("Shader"),
                source: ShaderSource::Wgsl(include_str!("../../res/shaders/shader.wgsl").into()),
            });

            let draws = &render_pass_node.draws;
            for draw in draws {
                let vertices = &draw.mesh.vertices;
                let indices = &draw.mesh.indices;

                let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("Vertex Buffer"),
                    contents: bytemuck::cast_slice(vertices),
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
                        VertexAttribute {
                            offset: (core::mem::size_of::<float4>() * 2) as wgpu::BufferAddress,
                            format: VertexFormat::Float32x2,
                            shader_location: 2,
                        },
                    ],
                };

                let indices_buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("Indices Buffer"),
                    contents: bytemuck::cast_slice(indices),
                    usage: BufferUsages::INDEX,
                });
                let indices_num = indices.len() as u32;

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
                            format: color_attachment_format,
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

                render_pass.set_pipeline(&pipeline);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(indices_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..indices_num, 0, 0..1);
            }
        };

        if let Some(egui_draw) = &render_pass_node.egui_draw {
            let clipped_primitives = &egui_draw.paint_jobs;
            let screen_descriptor = &egui_draw.screen_descriptor;

            for (id, image_delta) in &egui_draw.egui_update_textures {
                egui_renderer.update_texture(device, queue, *id, &image_delta);
            }

            let egui_commandbuffers = egui_renderer.update_buffers(
                device,
                queue,
                encoder,
                &clipped_primitives,
                &screen_descriptor,
            );

            egui_renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);

            return Some(egui_commandbuffers);
        }

        None
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

    pub fn get_frame_buffer_format(&self) -> AttachmentFormat {
        match self.surface_config.format {
            wgpu::TextureFormat::Rgba8Unorm => AttachmentFormat::RGBA8UNorm,
            wgpu::TextureFormat::Bgra8Unorm => AttachmentFormat::BGRA8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb => AttachmentFormat::BGRA8UnormSrgb,
            wgpu::TextureFormat::Rg11b10Ufloat => AttachmentFormat::RG11B10UFloat,
            _ => todo!(),
        }
    }
}
