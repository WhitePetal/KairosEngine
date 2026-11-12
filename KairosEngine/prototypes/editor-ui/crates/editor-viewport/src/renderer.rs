//! wgpu resources behind the viewport host.
//!
//! This module is the prototype stand-in for the engine's offscreen scene
//! render. It owns the GPU side of the host boundary and exposes no wgpu types
//! outside the crate; [`crate::ViewportHost`] turns its results into
//! [`egui::TextureId`]s.
//!
//! # Mapping to the real engine
//!
//! * render-graph colour attachment (offscreen LDR target) -> [`ViewportTarget`],
//!   an `Rgba8UnormSrgb` texture with `RENDER_ATTACHMENT | TEXTURE_BINDING` usage;
//! * `GraphicsCommand::bind_attachment_to_egui` ->
//!   [`egui_wgpu::Renderer::register_native_texture`] on every resize;
//! * the engine frame step that renders viewports before `BuildUi` ->
//!   [`ViewportRenderer::render`], called from the app's frame-logic phase
//!   (prototype: `eframe::App::logic`; production: RenderViewports);
//! * the engine scene render pass -> one fullscreen-triangle raymarch pass,
//!   `scene.wgsl`.
//!
//! # Colour space
//!
//! egui-wgpu samples user textures as gamma-encoded bytes (its shader expects
//! `Rgba8Unorm` data, see `egui.wgsl`). The scene writes linear light into the
//! `Rgba8UnormSrgb` attachment, so the hardware stores gamma-encoded bytes; we
//! then hand egui an `Rgba8Unorm` *view* of the same bytes. Registering the sRGB
//! view instead would make egui linearise the sample and then treat it as gamma,
//! rendering the whole viewport visibly dark.
//!
//! # Lifetime
//!
//! A [`ViewportTarget`] owns the texture, both views, its camera uniform buffer
//! and its egui registration. Dropping it (via
//! [`crate::ViewportHost::destroy`]) frees the egui texture as well. Resizing
//! frees the old egui registration and registers a new one, so the
//! `egui::TextureId` returned by `bind` can change after a resize.

use std::borrow::Cow;
use std::sync::Arc;

use egui::mutex::RwLock;
use egui_wgpu::wgpu;

use crate::camera::OrbitCamera;
use crate::error::ViewportError;

/// Offscreen colour format; matches the LDR render targets the prototype engine
/// would hand to the editor.
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Format of the view handed to egui.
///
/// Same bytes as [`COLOR_FORMAT`], reinterpreted as gamma-encoded. wgpu allows
/// texture views to change sRGB-ness only, which is exactly what we need.
const SAMPLED_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Clear colour for the offscreen target.
///
/// The fullscreen shader overwrites every pixel; a defined clear value keeps
/// debug captures stable if a pass is ever partially clipped.
const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.05,
    g: 0.055,
    b: 0.068,
    a: 1.0,
};

/// Uniform block consumed by `scene.wgsl`.
///
/// Layout mirrors the `SceneCamera` struct in the shader: five `vec4`s, 80 bytes,
/// 16-byte aligned. Keep both in sync.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneCameraUniform {
    /// xyz: eye position, w: aspect ratio.
    eye_aspect: [f32; 4],
    /// xyz: normalized forward direction, w: tan(fov_y / 2).
    forward_tan: [f32; 4],
    /// xyz: normalized right axis.
    right: [f32; 4],
    /// xyz: normalized up axis.
    up: [f32; 4],
    /// x: selected object id (-1.0 when nothing is selected);
    /// y/z: viewport size in pixels (used for supersampling); w: reserved.
    params: [f32; 4],
}

impl SceneCameraUniform {
    fn from_camera(camera: &OrbitCamera, size: [u32; 2], selected: Option<u64>) -> Self {
        let aspect = size[0] as f32 / size[1] as f32;
        let eye = camera.eye();
        let forward = camera.forward();
        let right = camera.right();
        let up = camera.up();
        Self {
            eye_aspect: [eye[0], eye[1], eye[2], aspect],
            forward_tan: [forward[0], forward[1], forward[2], camera.tan_half_fov_y()],
            right: [right[0], right[1], right[2], 0.0],
            up: [up[0], up[1], up[2], 0.0],
            params: [
                selected.map_or(-1.0, |id| id as f32),
                size[0] as f32,
                size[1] as f32,
                0.0,
            ],
        }
    }
}

/// GPU resources shared by every viewport of one host: the scene pipeline and
/// the egui renderer handle used to register textures.
pub(crate) struct ViewportRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    egui_renderer: Arc<RwLock<egui_wgpu::Renderer>>,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    max_texture_dimension: u32,
}

impl ViewportRenderer {
    /// Builds the raymarch pipeline from the egui-wgpu render state.
    ///
    /// # Errors
    /// Reserved for future GPU capability negotiation; the prototype pipeline is
    /// always creatable on a device that already rendered egui.
    pub(crate) fn new(render_state: &egui_wgpu::RenderState) -> Result<Self, ViewportError> {
        let device = render_state.device.clone();
        let queue = render_state.queue.clone();
        let egui_renderer = Arc::clone(&render_state.renderer);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viewport_scene_shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("scene.wgsl"))),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport_camera_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<SceneCameraUniform>() as u64,
                    ),
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viewport_scene_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("viewport_scene_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                // The fullscreen triangle is generated from vertex_index.
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let max_texture_dimension = device.limits().max_texture_dimension_2d;

        Ok(Self {
            device,
            queue,
            egui_renderer,
            pipeline,
            bind_group_layout,
            max_texture_dimension,
        })
    }

    /// Creates a new viewport target of the requested size.
    ///
    /// # Errors
    /// [`ViewportError::SizeOutOfRange`] when the size exceeds the device limits.
    pub(crate) fn create_target(
        &self,
        key: u32,
        width: u32,
        height: u32,
    ) -> Result<ViewportTarget, ViewportError> {
        let size = self.validated_size(key, width, height)?;
        Ok(ViewportTarget::create(
            &self.device,
            &self.egui_renderer,
            &self.bind_group_layout,
            key,
            size,
        ))
    }

    /// Resizes an existing target if its size actually changed.
    ///
    /// # Errors
    /// [`ViewportError::SizeOutOfRange`] when the size exceeds the device limits.
    pub(crate) fn ensure_target_size(
        &self,
        target: &mut ViewportTarget,
        key: u32,
        width: u32,
        height: u32,
    ) -> Result<(), ViewportError> {
        let size = self.validated_size(key, width, height)?;
        if target.size() == size {
            return Ok(());
        }
        target.recreate(&self.device, &self.bind_group_layout, key, size);
        Ok(())
    }

    /// Writes the camera uniform, records the scene pass and submits it.
    ///
    /// Recorded on the host's own encoder and submitted immediately, which is
    /// what the app calls during the frame-logic phase (before egui's backend
    /// paints). The call never waits on the GPU.
    ///
    /// # Errors
    /// Currently infallible; the `Result` mirrors the engine's fallible render
    /// submission API so callers already handle the failure path.
    pub(crate) fn render(
        &self,
        target: &ViewportTarget,
        camera: &OrbitCamera,
        selected: Option<u64>,
    ) -> Result<(), ViewportError> {
        let uniform = SceneCameraUniform::from_camera(camera, target.size(), selected);
        self.queue
            .write_buffer(&target.uniform_buffer, 0, bytemuck::bytes_of(&uniform));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("viewport_scene_encoder"),
            });
        {
            let color_attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &target.attachment_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CLEAR_COLOR),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport_scene_render_pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &target.camera_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        Ok(())
    }

    /// Clamps a requested size into the range a texture can have.
    ///
    /// Zero dimensions are clamped to one so a collapsed panel is still a valid
    /// (if invisible) render target.
    fn validated_size(&self, key: u32, width: u32, height: u32) -> Result<[u32; 2], ViewportError> {
        let width = width.max(1);
        let height = height.max(1);
        if width > self.max_texture_dimension || height > self.max_texture_dimension {
            return Err(ViewportError::SizeOutOfRange {
                key,
                width,
                height,
                max: self.max_texture_dimension,
            });
        }
        Ok([width, height])
    }
}

/// One viewport's offscreen colour target and its egui registration.
pub(crate) struct ViewportTarget {
    size: [u32; 2],
    /// Offscreen colour texture the two views below are created from.
    ///
    /// wgpu views keep their texture alive internally; owning it here makes the
    /// resource lifetime explicit and enables future pixel readback.
    _texture: wgpu::Texture,
    /// sRGB view used as the render pass colour attachment.
    attachment_view: wgpu::TextureView,
    /// Gamma-encoded view handed to egui (same bytes as `attachment_view`).
    sample_view: wgpu::TextureView,
    /// egui registration of `sample_view`.
    egui_texture: egui::TextureId,
    /// Shared egui renderer, used to free the texture on resize/drop.
    egui_renderer: Arc<RwLock<egui_wgpu::Renderer>>,
    /// Per-viewport camera uniform buffer.
    uniform_buffer: wgpu::Buffer,
    /// Bind group pairing the uniform buffer with the scene pipeline.
    camera_bind_group: wgpu::BindGroup,
}

impl ViewportTarget {
    fn create(
        device: &wgpu::Device,
        egui_renderer: &Arc<RwLock<egui_wgpu::Renderer>>,
        bind_group_layout: &wgpu::BindGroupLayout,
        key: u32,
        size: [u32; 2],
    ) -> Self {
        let color = create_color_target(device, egui_renderer, key, size);
        let (uniform_buffer, camera_bind_group) =
            create_camera_resources(device, bind_group_layout, key);
        Self {
            size,
            _texture: color.texture,
            attachment_view: color.attachment_view,
            sample_view: color.sample_view,
            egui_texture: color.egui_texture,
            egui_renderer: Arc::clone(egui_renderer),
            uniform_buffer,
            camera_bind_group,
        }
    }

    /// Replaces the texture, views, uniforms and egui registration in place.
    ///
    /// The old egui registration is freed first; the subsequent calls are
    /// infallible because the size was validated by the caller.
    fn recreate(
        &mut self,
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        key: u32,
        size: [u32; 2],
    ) {
        let egui_renderer = Arc::clone(&self.egui_renderer);
        // Free before dropping the texture and views the bind group references.
        egui_renderer.write().free_texture(&self.egui_texture);

        let color = create_color_target(device, &egui_renderer, key, size);
        let (uniform_buffer, camera_bind_group) =
            create_camera_resources(device, bind_group_layout, key);

        self.size = size;
        self._texture = color.texture;
        self.attachment_view = color.attachment_view;
        self.sample_view = color.sample_view;
        self.egui_texture = color.egui_texture;
        self.uniform_buffer = uniform_buffer;
        self.camera_bind_group = camera_bind_group;
    }

    pub(crate) fn size(&self) -> [u32; 2] {
        self.size
    }

    pub(crate) fn egui_texture(&self) -> egui::TextureId {
        self.egui_texture
    }
}

impl Drop for ViewportTarget {
    fn drop(&mut self) {
        self.egui_renderer.write().free_texture(&self.egui_texture);
    }
}

/// Resources that make up a freshly allocated offscreen colour target.
struct ColorTarget {
    texture: wgpu::Texture,
    attachment_view: wgpu::TextureView,
    sample_view: wgpu::TextureView,
    egui_texture: egui::TextureId,
}

fn create_color_target(
    device: &wgpu::Device,
    egui_renderer: &Arc<RwLock<egui_wgpu::Renderer>>,
    key: u32,
    size: [u32; 2],
) -> ColorTarget {
    let texture_label = format!("viewport_color_{key}");
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(texture_label.as_str()),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[SAMPLED_FORMAT],
    });

    let attachment_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("viewport_color_attachment_view"),
        ..wgpu::TextureViewDescriptor::default()
    });
    let sample_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("viewport_color_egui_view"),
        format: Some(SAMPLED_FORMAT),
        ..wgpu::TextureViewDescriptor::default()
    });

    let egui_texture = egui_renderer.write().register_native_texture(
        device,
        &sample_view,
        wgpu::FilterMode::Linear,
    );

    ColorTarget {
        texture,
        attachment_view,
        sample_view,
        egui_texture,
    }
}

fn create_camera_resources(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    key: u32,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer_label = format!("viewport_camera_uniforms_{key}");
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(buffer_label.as_str()),
        size: std::mem::size_of::<SceneCameraUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group_label = format!("viewport_camera_bind_group_{key}");
    let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(bind_group_label.as_str()),
        layout: bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });
    (uniform_buffer, camera_bind_group)
}
