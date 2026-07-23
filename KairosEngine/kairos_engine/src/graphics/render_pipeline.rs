use std::{collections::HashMap, error::Error, path::PathBuf, sync::Arc};

use petgraph::visit::{DfsEvent, Reversed, depth_first_search};
use strum::EnumCount;
use wgpu::{
    Adapter, BackendOptions, Backends, BindGroup, BindGroupDescriptor, BindGroupEntry,
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingResource, BindingType,
    BufferUsages, ColorTargetState, ColorWrites, CommandBuffer, CommandEncoder,
    CommandEncoderDescriptor, CurrentSurfaceTexture, DepthBiasState, DepthStencilState, Device,
    ExperimentalFeatures, Extent3d, FragmentState, FrontFace, InstanceFlags, Limits, LoadOp,
    LoadOpDontCare, MemoryBudgetThresholds, MemoryHints, MultisampleState, Operations, Origin3d,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PolygonMode, PowerPreference,
    PresentMode, PrimitiveState, Queue, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
    RequestAdapterOptions, SamplerBindingType, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StencilState, StoreOp, Surface, SurfaceConfiguration, SurfaceTexture, TexelCopyBufferLayout,
    TexelCopyTextureInfo, TextureDescriptor, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension, Trace, VertexAttribute, VertexBufferLayout,
    VertexFormat, VertexState, VertexStepMode,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::{DeviceDescriptor, SamplerDescriptor},
};
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, TextureAssetsSystem, asset::AssetIndex},
    graphics::{
        attachment::{AttachmentFormat, InternalAttachmentId},
        graphics_graph::{self, GraphicsGraph, graphics_node::RenderPassNode},
        mesh::Mesh,
        render_state::RenderState,
        shader::ShaderAsset,
        texture::{Texture, format::TextureCompressionConfig},
        vertex::Vertex,
    },
    kairos_paths,
    math::{float4, float4x4},
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

struct TextureCache {
    version: u32,
    bind_group: BindGroup,
    layout: BindGroupLayout,
}

struct MeshBufferCache {
    version: u32,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

struct PreparedDrawCall {
    pipeline: wgpu::RenderPipeline,
    texture_bind_group: Option<BindGroup>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    indices_count: u32,
    instancing_buffer: wgpu::Buffer,
    instance_count: u32,
}

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

    pipeline_cache: HashMap<PipelineKey, PipelineCache>,
    texture_cache: HashMap<usize, TextureCache>,
    mesh_buffer_cache: HashMap<usize, MeshBufferCache>,
    global_vp_bind_group_layout: BindGroupLayout,

    // #1: purple fallback for errored materials.
    purple_fallback: Option<(BindGroup, BindGroupLayout)>,
    error_material_indices: std::collections::HashSet<AssetIndex>,
    white_texture_fallback: Arc<AssetHandle<TextureAssetsSystem>>,
}

impl RenderPipeline {
    pub async fn new(
        window: Arc<Window>,
        compression_config: &TextureCompressionConfig,
        assets_server: &mut AssetsServer,
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

        let adapter_features = adapter.features();
        let required_features = compression_config.adapter_features(adapter_features);

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: None,
                required_features,
                required_limits: Limits::default(),
                experimental_features: ExperimentalFeatures::default(),
                memory_hints: MemoryHints::default(),
                trace: Trace::default(),
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let format;
        if surface_caps.formats.contains(&TextureFormat::Rgba8Unorm) {
            format = TextureFormat::Rgba8Unorm;
        } else {
            format = surface_caps.formats[0];
        }
        let window_size = window.inner_size();
        let width = window_size.width;
        let height = window_size.height;
        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: PresentMode::Fifo,
            desired_maximum_frame_latency: 3,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            surface_config.format,
            egui_wgpu::RendererOptions::default(),
        );

        let global_vp_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("VP Bind Group Layout"),
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

        // #1: create purple fallback texture for errored materials.
        let purple_fb = Self::create_purple_fallback(&device, &queue);

        Ok(Self {
            window: window,
            device,
            surface,
            surface_config,
            adapter,
            queue,
            encoder: None,
            egui_renderer,
            internal_texture_views: vec![None; InternalAttachmentId::COUNT],
            window_size,
            window_size_changed: false,

            pipeline_cache: HashMap::new(),
            texture_cache: HashMap::new(),
            mesh_buffer_cache: HashMap::new(),
            global_vp_bind_group_layout,
            purple_fallback: Some(purple_fb),
            error_material_indices: std::collections::HashSet::new(),
            white_texture_fallback: assets_server
                .load(&PathBuf::from(kairos_paths::PATH_WHITE_TEXTURE)),
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
                let texture_desc = TextureDescriptor {
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
                };
                let texture = self.device.create_texture(&texture_desc);
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
            let depth_desc = TextureDescriptor {
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
            };
            let depth = self.device.create_texture(&depth_desc);
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

        let mut all_error_scopes: Vec<(AssetIndex, wgpu::ErrorScopeGuard)> = Vec::new();
        while let Some(node) = nodes_stack.pop() {
            let Some(node) = graph.remove_node(node) else {
                continue;
            };
            match node {
                graphics_graph::graphics_node::GraphNode::None => {}
                graphics_graph::graphics_node::GraphNode::RenderPass(render_pass_node) => {
                    let vp = vps[render_pass_node.vp_id.0].to_array();
                    let vp_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
                        label: Some("VP Buffer"),
                        contents: bytemuck::cast_slice(&vp),
                        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                    });
                    let vp_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                        label: Some("VP Bind Group"),
                        layout: &self.global_vp_bind_group_layout,
                        entries: &[BindGroupEntry {
                            binding: 0,
                            resource: vp_buffer.as_entire_binding(),
                        }],
                    });
                    let mut command_buffers = Self::handle_render_pass_node(
                        &self.device,
                        &self.queue,
                        &mut encoder,
                        &mut self.pipeline_cache,
                        &mut self.texture_cache,
                        &mut self.mesh_buffer_cache,
                        &mut self.egui_renderer,
                        &render_pass_color_attachments,
                        &render_pass_depth_attachments,
                        &self.global_vp_bind_group_layout,
                        &vp_bind_group,
                        &render_pass_node,
                        assets_server,
                        &mut self.error_material_indices,
                        &self.purple_fallback,
                        &mut all_error_scopes,
                        &self.white_texture_fallback,
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
                        // 接收方（如已关闭的 Inspector 预览）被丢弃时 send 失败：
                        // 立即释放刚注册的纹理，避免泄漏在 egui renderer 中
                        if let Err(rt_id) = sender.send(rt_id) {
                            self.egui_renderer.free_texture(&rt_id);
                        }
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

        // Process pending error scopes after GPU work is submitted.
        // Scopes must be popped in reverse-push (LIFO) order.
        all_error_scopes.reverse();
        for (material_id, scope) in all_error_scopes.drain(..) {
            if let Some(error) = pollster::block_on(scope.pop()) {
                log::error!("Material #{:?} error: {error}", material_id);
                self.error_material_indices.insert(material_id);
            }
        }

        // Must be called before present() on Windows for vsync to work correctly.
        // Tells DWM to prepare for the upcoming frame presentation.
        self.window.pre_present_notify();
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
        texture_cache: &mut HashMap<usize, TextureCache>,
        mesh_buffer_cache: &mut HashMap<usize, MeshBufferCache>,
        egui_renderer: &mut egui_wgpu::Renderer,
        render_pass_color_attachments: &Vec<RenderPassColorAttachment>,
        render_pass_depth_attachments: &Vec<RenderPassDepthStencilAttachment>,
        global_vp_bind_group_layout: &BindGroupLayout,
        vp_bind_group: &BindGroup,
        render_pass_node: &RenderPassNode,
        assets_server: &AssetsServer,
        error_material_indices: &mut std::collections::HashSet<AssetIndex>,
        purple_fallback: &Option<(BindGroup, BindGroupLayout)>,
        error_scopes: &mut Vec<(AssetIndex, wgpu::ErrorScopeGuard)>,
        white_texture_fallback: &Arc<AssetHandle<TextureAssetsSystem>>,
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

        // ---- Phase 1: Prepare all GPU resources before creating the render pass ----
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

        let mut prepared_draws: Vec<PreparedDrawCall> =
            Vec::with_capacity(render_pass_node.draw_instances.len());

        for draw in &render_pass_node.draw_instances {
            let Some(mesh) = assets_server.get(&draw.renderer.mesh) else {
                continue;
            };
            let Some(material) = assets_server.get(&draw.renderer.material) else {
                continue;
            };
            let texture_handle = match &material.texture {
                Some(texture) => texture,
                None => white_texture_fallback,
            };

            let material_id = draw.renderer.material.id();
            let material_errored = error_material_indices.contains(&material_id);

            let Some(shader_asset) = &material.shader else {
                continue;
            };
            let Some(shader) = assets_server.get(shader_asset) else {
                continue;
            };

            // --- Texture bind group ---
            let texture_bind_group: Option<BindGroup>;
            let texture_bind_group_layout: Option<&BindGroupLayout>;

            if material_errored {
                if let Some((bg, layout)) = purple_fallback.as_ref() {
                    texture_bind_group = Some(bg.clone());
                    texture_bind_group_layout = Some(layout);
                } else {
                    texture_bind_group = None;
                    texture_bind_group_layout = None;
                }
            } else {
                let Some(texture_asset) = assets_server.get(texture_handle) else {
                    continue;
                };
                let texture_id = texture_handle.id();
                let key = texture_id.index() as usize;
                let version = texture_id.version();
                let result = {
                    error_scopes.push((
                        material_id,
                        device.push_error_scope(wgpu::ErrorFilter::Validation),
                    ));
                    match texture_cache.entry(key) {
                        std::collections::hash_map::Entry::Occupied(mut entry) => {
                            let cache = entry.get();
                            if cache.version == version {
                                cache.bind_group.clone()
                            } else {
                                let (bind_group, layout) =
                                    Self::create_texture(device, queue, texture_asset);
                                entry.insert(TextureCache {
                                    version,
                                    bind_group: bind_group.clone(),
                                    layout,
                                });
                                bind_group
                            }
                        }
                        std::collections::hash_map::Entry::Vacant(entry) => {
                            let (bind_group, layout) =
                                Self::create_texture(device, queue, texture_asset);
                            entry.insert(TextureCache {
                                version,
                                bind_group: bind_group.clone(),
                                layout,
                            });
                            bind_group
                        }
                    }
                };
                texture_bind_group = Some(result);
                texture_bind_group_layout = texture_cache.get(&key).map(|c| &c.layout);
            };

            // --- Pipeline ---
            let shader_id = shader_asset.id();
            let pipeline_key = PipelineKey {
                shader_index: shader_id.index(),
                render_state: material.render_state,
            };
            let shader_version = shader_id.version();
            let pipeline = match pipeline_cache.entry(pipeline_key) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    let cache = entry.get();
                    if cache.version == shader_version {
                        cache.pipeline.clone()
                    } else {
                        error_scopes.push((
                            material_id,
                            device.push_error_scope(wgpu::ErrorFilter::Validation),
                        ));
                        let pipeline = Self::create_pipeline(
                            device,
                            global_vp_bind_group_layout,
                            texture_bind_group_layout,
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
                        entry.insert(PipelineCache {
                            version: shader_version,
                            pipeline: pipeline.clone(),
                        });
                        pipeline
                    }
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    error_scopes.push((
                        material_id,
                        device.push_error_scope(wgpu::ErrorFilter::Validation),
                    ));
                    let pipeline = Self::create_pipeline(
                        device,
                        global_vp_bind_group_layout,
                        texture_bind_group_layout,
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
                    entry.insert(PipelineCache {
                        version: shader_version,
                        pipeline: pipeline.clone(),
                    });
                    pipeline
                }
            };

            // --- Mesh buffers ---
            let mesh_id = draw.renderer.mesh.id();
            let mesh_key = mesh_id.index();
            let mesh_version = mesh_id.version();
            let indices_num = mesh.indices.len() as u32;
            let (vertex_buffer, index_buffer) = match mesh_buffer_cache.entry(mesh_key) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    let cache = entry.get();
                    if cache.version == mesh_version {
                        (cache.vertex_buffer.clone(), cache.index_buffer.clone())
                    } else {
                        let (vb, ib) = Self::create_mesh(device, mesh);
                        entry.insert(MeshBufferCache {
                            version: mesh_version,
                            vertex_buffer: vb.clone(),
                            index_buffer: ib.clone(),
                        });
                        (vb, ib)
                    }
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let (vb, ib) = Self::create_mesh(device, mesh);
                    entry.insert(MeshBufferCache {
                        version: mesh_version,
                        vertex_buffer: vb.clone(),
                        index_buffer: ib.clone(),
                    });
                    (vb, ib)
                }
            };

            // --- Instancing buffer (owned, lives until after render pass) ---
            let instancing_buffer = device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Instancing Buffer"),
                contents: bytemuck::cast_slice(&draw.local_to_worlds),
                usage: BufferUsages::VERTEX,
            });
            let instance_count = draw.local_to_worlds.len() as u32;

            prepared_draws.push(PreparedDrawCall {
                pipeline,
                texture_bind_group,
                vertex_buffer,
                index_buffer,
                indices_count: indices_num,
                instancing_buffer,
                instance_count,
            });
        }

        // --- Phase 1.5: egui update_buffers must happen before begin_render_pass ---
        // because it borrows `encoder` mutably, and begin_render_pass also needs `encoder`.
        let egui_commandbuffers = if let Some(egui_draw) = &render_pass_node.egui_draw {
            for (id, image_delta) in &egui_draw.egui_update_textures {
                egui_renderer.update_texture(device, queue, *id, &image_delta);
            }
            let cb = egui_renderer.update_buffers(
                device,
                queue,
                encoder,
                &egui_draw.paint_jobs,
                &egui_draw.screen_descriptor,
            );
            Some((cb, &egui_draw.paint_jobs, &egui_draw.screen_descriptor))
        } else {
            None
        };

        // ---- Phase 2: Render pass ----
        // egui_wgpu::Renderer::render requires RenderPass<'static>, so for egui passes
        // we still need forget_lifetime(). This is safe because egui passes don't create
        // instancing buffers inside the pass.
        if render_pass_node.egui_draw.is_some() {
            // Egui path — forget_lifetime() required by egui_wgpu API.
            let mut render_pass = encoder
                .begin_render_pass(&RenderPassDescriptor {
                    label: render_pass_node.label,
                    color_attachments: &color_attachments,
                    depth_stencil_attachment: depth_attachment,
                    ..Default::default()
                })
                .forget_lifetime();

            render_pass.set_bind_group(0, vp_bind_group, &[]);

            // Also issue any prepared regular draws (in case graph merging combined them).
            for p in &prepared_draws {
                render_pass.set_pipeline(&p.pipeline);
                if let Some(ref bg) = p.texture_bind_group {
                    render_pass.set_bind_group(1, bg, &[]);
                }
                render_pass.set_vertex_buffer(0, p.vertex_buffer.slice(..));
                render_pass.set_index_buffer(p.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.set_vertex_buffer(1, p.instancing_buffer.slice(..));
                render_pass.draw_indexed(0..p.indices_count, 0, 0..p.instance_count);
            }

            if let Some((_, paint_jobs, screen_descriptor)) = &egui_commandbuffers {
                egui_renderer.render(&mut render_pass, paint_jobs, screen_descriptor);
            }

            return egui_commandbuffers.map(|(cb, _, _)| cb);
        }

        // Regular draws path — normal lifetime, no forget_lifetime().
        // `prepared_draws` holds ownership of instancing_buffers, so they outlive
        // the render pass and are safely dropped afterwards.
        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: render_pass_node.label,
                color_attachments: &color_attachments,
                depth_stencil_attachment: depth_attachment,
                ..Default::default()
            });

            render_pass.set_bind_group(0, vp_bind_group, &[]);

            for p in &prepared_draws {
                render_pass.set_pipeline(&p.pipeline);
                if let Some(ref bg) = p.texture_bind_group {
                    render_pass.set_bind_group(1, bg, &[]);
                }
                render_pass.set_vertex_buffer(0, p.vertex_buffer.slice(..));
                render_pass.set_index_buffer(p.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.set_vertex_buffer(1, p.instancing_buffer.slice(..));
                render_pass.draw_indexed(0..p.indices_count, 0, 0..p.instance_count);
            }
        }
        // `prepared_draws` dropped here — instancing_buffers safely released after render pass.

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
            _ => todo!("Not impl frame buffer format -> attachment format convert"),
        }
    }

    fn create_purple_fallback(device: &Device, queue: &Queue) -> (BindGroup, BindGroupLayout) {
        let size = Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let tex = device.create_texture(&TextureDescriptor {
            label: Some("Purple Fallback"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let purple: [u8; 4] = [128, 0, 128, 255];
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &purple,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            size,
        );
        let view = tex.create_view(&TextureViewDescriptor::default());
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Purple Fallback Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Purple Fallback Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Purple Fallback Bind Group"),
            layout: &layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });
        (bind_group, layout)
    }

    /// #1: Clear error state for a material, re-enabling its real texture.
    pub fn clear_material_error(&mut self, material_id: AssetIndex) {
        self.error_material_indices.remove(&material_id);
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
                    offset: std::mem::offset_of!(Vertex, normal) as wgpu::BufferAddress,
                    format: VertexFormat::Float32x3,
                    shader_location: 3,
                },
                VertexAttribute {
                    offset: std::mem::offset_of!(Vertex, tangent) as wgpu::BufferAddress,
                    format: VertexFormat::Float32x4,
                    shader_location: 4,
                },
            ],
        };

        let mut depth_state = depth_state.clone();
        if let Some(depth_state) = &mut depth_state {
            depth_state.depth_compare = render_state.depth_test.map(|cmp| cmp.into());
            // wgpu: depth_write_enabled = Some(true) 要求 depth_compare 为 Some
            // （MissingDepthCompare），effective_depth_write 负责该约束。
            depth_state.depth_write_enabled = Some(render_state.depth_write_enable());
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
                topology: render_state.topology.into(),
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: render_state.cull_mod.into(),
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
                    blend: render_state.blend_mod.map(|v| v.into()),
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

    fn create_texture(
        device: &Device,
        queue: &Queue,
        texture_asset: &Texture,
    ) -> (BindGroup, BindGroupLayout) {
        // Miss or version mismatch: create GPU resources
        let texture_dimension = (texture_asset.width, texture_asset.height);
        let wgpu_fmt: wgpu::TextureFormat = texture_asset.format.into();
        let texture_size = Extent3d {
            width: texture_dimension.0,
            height: texture_dimension.1,
            depth_or_array_layers: 1,
        };
        let tex_desc = TextureDescriptor {
            label: Some("Kairos Texture"),
            size: texture_size,
            mip_level_count: texture_asset.data.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu_fmt,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        };
        let gpu_texture = device.create_texture(&tex_desc);

        // Write each mip level.
        let wgpu_fmt: wgpu::TextureFormat = texture_asset.format.into();
        for (level, level_data) in texture_asset.data.iter().enumerate() {
            let level = level as u32;
            let level_w = (texture_dimension.0 >> level).max(1);
            let level_h = (texture_dimension.1 >> level).max(1);
            let block_bytes = wgpu_fmt
                .block_copy_size(Some(wgpu::TextureAspect::All))
                .unwrap_or(4);
            let (block_w, _) = texture_asset.format.block_dimensions();
            let blocks_per_row = (level_w + block_w - 1) / block_w;
            let bytes_per_row = block_bytes * blocks_per_row;

            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &gpu_texture,
                    mip_level: level,
                    origin: Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                level_data,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(level_h),
                },
                Extent3d {
                    width: level_w,
                    height: level_h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let texture_view = gpu_texture.create_view(&TextureViewDescriptor::default());

        let sample_type = texture_asset.format.sample_type().into();
        let sampler_type = match sample_type {
            wgpu::TextureSampleType::Float { .. } => SamplerBindingType::Filtering,
            _ => SamplerBindingType::NonFiltering,
        };
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Texture Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type,
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(sampler_type),
                    count: None,
                },
            ],
        });

        let texture_sampler =
            device.create_sampler(&texture_asset.sampler.to_wgpu_descriptor("Texture Sampler"));
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &layout,
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
        (bind_group, layout)
    }

    fn create_mesh(device: &Device, mesh: &Mesh) -> (wgpu::Buffer, wgpu::Buffer) {
        let vb = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: BufferUsages::VERTEX,
        });
        let ib = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: BufferUsages::INDEX,
        });
        (vb, ib)
    }
}
