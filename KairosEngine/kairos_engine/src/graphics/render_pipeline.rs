use std::{collections::HashMap, error::Error, sync::Arc};

use petgraph::visit::{DfsEvent, Reversed, depth_first_search};
use wgpu::{
    Adapter, AddressMode, BackendOptions, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry,
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType,
    BufferUsages, ColorTargetState, ColorWrites, CommandBuffer, CommandEncoder,
    CommandEncoderDescriptor, CurrentSurfaceTexture, DepthBiasState, DepthStencilState, Device,
    ExperimentalFeatures, Extent3d, Features, FilterMode, FragmentState, FrontFace, InstanceFlags,
    Limits, LoadOp, LoadOpDontCare, MemoryBudgetThresholds, MemoryHints, MipmapFilterMode,
    MultisampleState, Operations, Origin3d, PipelineCompilationOptions, PipelineLayoutDescriptor,
    PolygonMode, PowerPreference, PresentMode, PrimitiveState, PrimitiveTopology, Queue,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    RenderPipelineDescriptor, RequestAdapterOptions, Sampler, SamplerBindingType,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StencilState, StoreOp, Surface,
    SurfaceConfiguration, SurfaceTexture, TexelCopyBufferLayout, TexelCopyTextureInfo,
    TextureFormat, TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor,
    TextureViewDimension, Trace, VertexAttribute, VertexBufferLayout, VertexFormat, VertexState,
    VertexStepMode,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::{DeviceDescriptor, SamplerDescriptor, TextureDescriptor},
};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    asset_loader::assets::AssetsServer,
    graphics::{
        attachment::{AttachmentFormat, InternalAttachmentId},
        graphics_graph::{self, GraphicsGraph, graphics_node::RenderPassNode},
        material::Material,
        render_state::RenderState,
        shader::ShaderAsset,
        vertex::Vertex,
    },
    math::{float2, float3, float4, float4x4},
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct PipelineKey {
    shader_index: usize,
    render_state: RenderState,
}
struct PipelineCache {
    version: u32,
    pipeline: wgpu::RenderPipeline,
}

pub struct RenderPipeline {
    _window: Arc<Window>,
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

    pipeline_cache: HashMap<PipelineKey, PipelineCache>,
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

        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            surface_config.format,
            egui_wgpu::RendererOptions::default(),
        );

        Ok(Self {
            _window: window,
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

            pipeline_cache: HashMap::new(),
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
                    [InternalAttachmentId::FrameBufferColorAttachment as usize] = Some(view);
                Ok(output)
            }
            err => Err(err),
        }
    }

    pub fn present(
        &mut self,
        assets_server: &mut AssetsServer,
        output: SurfaceTexture,
        graphics_graph: GraphicsGraph,
    ) {
        let Some(mut encoder) = self.encoder.take() else {
            return;
        };

        let attachments = graphics_graph.attachments;
        let mut attachment_views = Vec::with_capacity(attachments.len());
        let mut render_pass_color_attachments = Vec::with_capacity(attachments.len());
        let depth_attachments = graphics_graph.depth_attachments;
        let mut depth_attachment_views = Vec::with_capacity(depth_attachments.len());
        let mut render_pass_depth_attachments = Vec::with_capacity(depth_attachments.len());
        let vps = graphics_graph.vps;
        let mut vp_bind_groups = Vec::with_capacity(vps.len());

        // create res
        for attachment in attachments {
            // 绑了 internal id 的，就找有没有internal texture view，有则渲染到internal
            // TODO: 获取没有 internal texture view 时，我这里应该创建？
            if let Some(internal_attachement_id) = attachment.bind_internal_id {
                if let Some(internal_texture_view) = self
                    .internal_texture_views
                    .get(internal_attachement_id as usize)
                    && let Some(internal_texture_view) = internal_texture_view
                {
                    let internal_texture_view = internal_texture_view.clone();
                    attachment_views.push(internal_texture_view);
                }
            } else {
                let texture = self.device.create_texture(&TextureDescriptor {
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
                attachment_views.push(view);
            }
        }
        for view in &attachment_views {
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
            render_pass_color_attachments.push(render_pass_color_attachment);
        }

        for attachment in depth_attachments {
            let depth = self.device.create_texture(&TextureDescriptor {
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
            let view = depth.create_view(&TextureViewDescriptor::default());
            depth_attachment_views.push(view);
        }
        for view in &depth_attachment_views {
            let attachment = RenderPassDepthStencilAttachment {
                view: &view,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),
                    store: StoreOp::Store,
                }),
                stencil_ops: Some(Operations {
                    load: LoadOp::DontCare(LoadOpDontCare::default()),
                    store: StoreOp::Store,
                }),
            };
            render_pass_depth_attachments.push(attachment);
        }

        for vp in vps {
            let vp = [
                vp.c0().to_array(),
                vp.c1().to_array(),
                vp.c2().to_array(),
                vp.c3().to_array(),
            ];
            let vp_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
                label: Some("VP Buffer"),
                contents: bytemuck::cast_slice(&vp),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            });
            let vp_bind_group_layout =
                self.device
                    .create_bind_group_layout(&BindGroupLayoutDescriptor {
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
            let vp_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("VP Buffer Bind Group"),
                layout: &vp_bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: vp_buffer.as_entire_binding(),
                }],
            });

            vp_bind_groups.push((vp_bind_group_layout, vp_bind_group));
        }

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
                graphics_graph::graphics_node::GraphNode::None => {}
                graphics_graph::graphics_node::GraphNode::RenderPass(render_pass_node) => {
                    let mut command_buffers = Self::handle_render_pass_node(
                        &self.device,
                        &self.queue,
                        &mut encoder,
                        &mut self.pipeline_cache,
                        &mut self.egui_renderer,
                        &render_pass_color_attachments,
                        &render_pass_depth_attachments,
                        &vp_bind_groups,
                        &render_pass_node,
                        assets_server,
                    );
                    if let Some(command_buffers) = &mut command_buffers {
                        more_command_buffers.append(command_buffers);
                    }
                }
                graphics_graph::graphics_node::GraphNode::OutputToFrameBuffer(
                    output_to_frame_buffer_node,
                ) => {
                    let _ = std::mem::replace(
                        &mut egui_free_textures,
                        Some(output_to_frame_buffer_node.egui_free_textures),
                    );
                }
                graphics_graph::graphics_node::GraphNode::BindAttachmentToEgui(
                    mut bind_attachment_to_egui_node,
                ) => {
                    let Some(attachment) = render_pass_color_attachments
                        .get(bind_attachment_to_egui_node.attachment_id.0)
                    else {
                        continue;
                    };
                    let rt_id = self.egui_renderer.register_native_texture(
                        &self.device,
                        attachment.view,
                        wgpu::FilterMode::Linear,
                    );
                    if let Some(sender) = bind_attachment_to_egui_node.sender.take() {
                        let _ = sender.send(rt_id);
                    }
                }
                graphics_graph::graphics_node::GraphNode::CopyAttachmentToEGui(
                    _copy_attachment_to_egui_node,
                ) => {}
                graphics_graph::graphics_node::GraphNode::FreeEguiTextureId(_) => unreachable!(),
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
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        pipeline_cache: &mut HashMap<PipelineKey, PipelineCache>,
        egui_renderer: &mut egui_wgpu::Renderer,
        render_pass_color_attachments: &Vec<RenderPassColorAttachment>,
        render_pass_depth_attachments: &Vec<RenderPassDepthStencilAttachment>,
        vp_bind_groups: &Vec<(BindGroupLayout, BindGroup)>,
        render_pass_node: &RenderPassNode,
        assets_server: &mut AssetsServer,
    ) -> Option<Vec<CommandBuffer>> {
        let attachment_ids = &render_pass_node.attachments;

        let color_attachments = attachment_ids
            .iter()
            .map(|bind| {
                let mut attachment = render_pass_color_attachments[bind.id.0].clone();
                match bind.load_action {
                    super::attachment::AttachmentLoadAction::Load => {
                        attachment.ops.load = LoadOp::Load;
                    }
                    super::attachment::AttachmentLoadAction::LoadClear => {
                        attachment.ops.load = LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        });
                    }
                    super::attachment::AttachmentLoadAction::DontCare => {
                        attachment.ops.load = LoadOp::DontCare(LoadOpDontCare::default());
                    }
                }
                match bind.store_action {
                    super::attachment::AttachmentStoreAction::Store => {
                        attachment.ops.store = StoreOp::Store;
                    }
                    super::attachment::AttachmentStoreAction::Discard => {
                        attachment.ops.store = StoreOp::Discard;
                    }
                }
                // attachment.ops = load_stores[i];
                Some(attachment)
            })
            .collect::<Vec<_>>();

        let (depth_attachment, depth_state) = {
            if let Some(depth_bind) = render_pass_node.depth_stencil_attachment {
                let mut depth_attachment = render_pass_depth_attachments[depth_bind.id.0].clone();
                depth_attachment.depth_ops =
                    depth_bind
                        .depth_load_store_action
                        .map(|(load_action, store_action)| {
                            let load = match load_action {
                                super::attachment::AttachmentLoadAction::Load => LoadOp::Load,
                                super::attachment::AttachmentLoadAction::LoadClear => {
                                    LoadOp::Clear(1.0)
                                }
                                super::attachment::AttachmentLoadAction::DontCare => {
                                    LoadOp::DontCare(LoadOpDontCare::default())
                                }
                            };
                            let store = match store_action {
                                super::attachment::AttachmentStoreAction::Store => StoreOp::Store,
                                super::attachment::AttachmentStoreAction::Discard => {
                                    StoreOp::Discard
                                }
                            };
                            Operations { load, store }
                        });
                depth_attachment.stencil_ops =
                    depth_bind
                        .stencil_load_store_action
                        .map(|(load_action, store_action)| {
                            let load = match load_action {
                                super::attachment::AttachmentLoadAction::Load => LoadOp::Load,
                                super::attachment::AttachmentLoadAction::LoadClear => {
                                    LoadOp::Clear(0)
                                }
                                super::attachment::AttachmentLoadAction::DontCare => {
                                    LoadOp::DontCare(LoadOpDontCare::default())
                                }
                            };
                            let store = match store_action {
                                super::attachment::AttachmentStoreAction::Store => StoreOp::Store,
                                super::attachment::AttachmentStoreAction::Discard => {
                                    StoreOp::Discard
                                }
                            };
                            Operations { load, store }
                        });

                let depth_state = DepthStencilState {
                    format: depth_attachment.view.texture().format(),
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: StencilState::default(),
                    bias: DepthBiasState::default(),
                };
                (Some(depth_attachment), Some(depth_state))
            } else {
                (None, None)
            }
        };

        let mut render_pass = encoder
            .begin_render_pass(&RenderPassDescriptor {
                label: render_pass_node.label,
                color_attachments: &color_attachments,
                depth_stencil_attachment: depth_attachment,

                ..Default::default()
            })
            .forget_lifetime();

        // build pipeline
        if let Some((vp_bind_group_layout, vp_bind_group)) =
            vp_bind_groups.get(render_pass_node.vp_id.0)
        {
            render_pass.set_bind_group(0, vp_bind_group, &[]);

            let instancing_vertex_buffer_layout = VertexBufferLayout {
                array_stride: core::mem::size_of::<float4x4>() as wgpu::BufferAddress,
                attributes: &[
                    VertexAttribute {
                        offset: 0,
                        format: VertexFormat::Float32x4,
                        shader_location: 5,
                    },
                    VertexAttribute {
                        offset: std::mem::size_of::<float4>() as wgpu::BufferAddress,
                        format: VertexFormat::Float32x4,
                        shader_location: 6,
                    },
                    VertexAttribute {
                        offset: (std::mem::size_of::<float4>() * 2) as wgpu::BufferAddress,
                        format: VertexFormat::Float32x4,
                        shader_location: 7,
                    },
                    VertexAttribute {
                        offset: (std::mem::size_of::<float4>() * 3) as wgpu::BufferAddress,
                        format: VertexFormat::Float32x4,
                        shader_location: 8,
                    },
                ],
                step_mode: VertexStepMode::Instance,
            };

            let draws = &render_pass_node.draw_instances;
            for draw in draws {
                let Some((vertices, indices)) = draw.get_vertices_indices(assets_server) else {
                    // println!("fk mesh is none! {:?}", draw);
                    continue;
                };
                let Some(material) = draw.get_material(assets_server) else {
                    // println!("fk material is none! {:?}", draw);
                    continue;
                };

                let Some(shader_asset) = &material.shader else {
                    // println!("fk shader asset is none! {:?}", draw);
                    continue;
                };
                let Some(shader) = assets_server.get(shader_asset) else {
                    // println!("fk shader is none! {:?}", draw);
                    continue;
                };

                let texture_data = Self::create_texture(device, queue, material, assets_server);
                if let Some((texture_bind_group_layout, texture_view, texture_sampler)) =
                    &texture_data
                {
                    let texture_bind_group_descriptor = BindGroupDescriptor {
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
                    };
                    let texture_bind_group =
                        device.create_bind_group(&texture_bind_group_descriptor);
                    render_pass.set_bind_group(1, &texture_bind_group, &[]);
                }

                let shader_id = shader_asset.id();
                let pipeline_key = PipelineKey {
                    shader_index: shader_id.index(),
                    render_state: material.render_state,
                };
                let shader_version = shader_id.version();
                match pipeline_cache.entry(pipeline_key) {
                    std::collections::hash_map::Entry::Occupied(mut entry) => {
                        let cache = entry.get();
                        if cache.version == shader_version {
                            render_pass.set_pipeline(&cache.pipeline);
                        } else {
                            let pipeline = Self::create_pipeline(
                                device,
                                vp_bind_group_layout,
                                texture_data.map(|(layout, ..)| layout).as_ref(),
                                shader,
                                &depth_state,
                                &pipeline_key.render_state,
                                instancing_vertex_buffer_layout.clone(),
                                color_attachments[0]
                                    .as_ref()
                                    .unwrap()
                                    .view
                                    .texture()
                                    .format(),
                            );
                            render_pass.set_pipeline(&pipeline);

                            entry.insert(PipelineCache {
                                version: shader_version,
                                pipeline,
                            });
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let pipeline = Self::create_pipeline(
                            device,
                            vp_bind_group_layout,
                            texture_data.map(|(layout, ..)| layout).as_ref(),
                            shader,
                            &depth_state,
                            &pipeline_key.render_state,
                            instancing_vertex_buffer_layout.clone(),
                            color_attachments[0]
                                .as_ref()
                                .unwrap()
                                .view
                                .texture()
                                .format(),
                        );
                        render_pass.set_pipeline(&pipeline);

                        entry.insert(PipelineCache {
                            version: shader_version,
                            pipeline,
                        });
                    }
                };

                let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("Vertex Buffer"),
                    contents: bytemuck::cast_slice(vertices),
                    usage: BufferUsages::VERTEX,
                });

                let indices_buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("Indices Buffer"),
                    contents: bytemuck::cast_slice(indices),
                    usage: BufferUsages::INDEX,
                });
                let indices_num = indices.len() as u32;

                let instancing_buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("Instancing Bufferr"),
                    contents: bytemuck::cast_slice(&draw.local_to_worlds),
                    usage: BufferUsages::VERTEX,
                });

                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_vertex_buffer(1, instancing_buffer.slice(..));
                render_pass.set_index_buffer(indices_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..indices_num, 0, 0..draw.local_to_worlds.len() as u32);
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

    fn create_texture(
        device: &Device,
        queue: &Queue,
        material: &Material,
        assets_server: &AssetsServer,
    ) -> Option<(BindGroupLayout, TextureView, Sampler)> {
        if let Some(texture) = &material.texture {
            if let Some(texture) = assets_server.get(texture) {
                let texture = &texture.texture;
                let texture_data = &texture.data;

                // let texture_dimension = texture_asset.dimensions();
                let texture_dimension = (texture.width, texture.height);
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

                return Some((texture_bind_group_layout, texture_view, texture_sampler));
            }
        }
        None
    }

    fn create_pipeline(
        device: &Device,
        vp_bind_group_layout: &BindGroupLayout,
        texture_bind_group_layout: Option<&BindGroupLayout>,
        shader: &ShaderAsset,
        depth_state: &Option<DepthStencilState>,
        render_state: &RenderState,
        instancing_vertex_buffer_layout: VertexBufferLayout,
        render_target_format: TextureFormat,
    ) -> wgpu::RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[Some(vp_bind_group_layout), texture_bind_group_layout],
            immediate_size: 0,
        });

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shader"),
            source: ShaderSource::Wgsl((&shader.shader_string).into()),
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
                VertexAttribute {
                    offset: (core::mem::size_of::<float4>() * 2 + core::mem::size_of::<float2>())
                        as wgpu::BufferAddress,
                    format: VertexFormat::Float32x3,
                    shader_location: 3,
                },
                VertexAttribute {
                    offset: (core::mem::size_of::<float4>() * 2
                        + core::mem::size_of::<float2>()
                        + core::mem::size_of::<float3>())
                        as wgpu::BufferAddress,
                    format: VertexFormat::Float32x4,
                    shader_location: 4,
                },
            ],
        };

        let mut depth_state = depth_state.clone();
        if let Some(depth_state) = &mut depth_state {
            depth_state.depth_compare = render_state.depth_test;
            depth_state.depth_write_enabled = Some(render_state.depth_write);
        }

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[vertex_buffer_layout, instancing_vertex_buffer_layout],
            },
            primitive: PrimitiveState {
                topology: render_state.topology,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: render_state.cull_mod,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format: render_target_format,
                    blend: render_state.blend_mod,
                    write_mask: ColorWrites::all(),
                })],
            }),
            depth_stencil: depth_state,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        pipeline
    }
}
