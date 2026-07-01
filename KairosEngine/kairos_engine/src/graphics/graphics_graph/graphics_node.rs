use std::sync::Arc;

use crate::{
    asset_loader::assets::{AssetHandle, MaterialAssetsSystem, MeshAssetsSystem},
    math::float4x4,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColorAttachmentId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DepthAttachmentId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VPId(pub usize);

pub struct BaseDraw {
    pub mesh: Arc<AssetHandle<MeshAssetsSystem>>,
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
    pub local_to_world: float4x4,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstancingRenderer {
    pub mesh: Arc<AssetHandle<MeshAssetsSystem>>,
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
}
pub struct InstancingDraw {
    pub renderer: InstancingRenderer,
    pub local_to_worlds: Vec<float4x4>,
}
pub struct EguiDraw {
    pub paint_jobs: Vec<egui::ClippedPrimitive>,
    pub screen_descriptor: egui_wgpu::ScreenDescriptor,
    pub egui_update_textures: Vec<(epaint::TextureId, epaint::ImageDelta)>,
}

pub enum GraphNode {
    None,
    RenderPass(RenderPassNode),
    OutputToFrameBuffer(OutputToFrameBufferNode),
    BindAttachmentToEgui(BindAttachmentToEguiNode),
    CopyAttachmentToEGui(CopyAttachmentToEguiNode),
    FreeEguiTextureId(egui::TextureId),
}

/// Minimal vertex type for gizmo geometry (position only).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct GizmoVertex {
    pub position: [f32; 3],
}
unsafe impl bytemuck::Zeroable for GizmoVertex {}
unsafe impl bytemuck::Pod for GizmoVertex {}

/// Grid gizmo draw data — vertices + indices for a quad on the XZ plane.
pub struct GizmoGridDraw {
    pub vertices: Vec<GizmoVertex>,
    pub indices: Vec<u16>,
}

pub struct RenderPassNode {
    pub label: Option<&'static str>,
    pub attachments: Vec<ColorAttachmentId>,
    pub depth_stencil_attachment: Option<DepthAttachmentId>,
    pub vp_id: VPId,
    pub draws: Vec<BaseDraw>,
    pub draw_instances: Vec<InstancingDraw>,
    pub force_clear: bool,
    pub egui_draw: Option<EguiDraw>,
    pub gizmo_grid: Option<GizmoGridDraw>,
}
pub struct OutputToFrameBufferNode {
    pub attachment_id: ColorAttachmentId,
    pub egui_free_textures: Vec<epaint::TextureId>,
}
pub struct BindAttachmentToEguiNode {
    pub attachment_id: ColorAttachmentId,
    pub sender: Option<tokio::sync::oneshot::Sender<egui::TextureId>>,
}
pub struct CopyAttachmentToEguiNode {
    pub attachment_id: ColorAttachmentId,
    pub egui_texture_id: egui::TextureId,
}
