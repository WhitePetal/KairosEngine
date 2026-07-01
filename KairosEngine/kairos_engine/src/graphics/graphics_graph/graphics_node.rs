use std::{hash::Hash, sync::Arc};

use crate::{
    asset_loader::assets::{AssetHandle, MaterialAssetsSystem, MeshAssetsSystem},
    math::{float3, float4x4},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColorAttachmentId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DepthAttachmentId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VPId(pub usize);

pub enum Drawer {
    Base(BaseDraw),
    SimpleMesh(SimpleMeshDrawer),
}

pub struct BaseDraw {
    pub mesh: Arc<AssetHandle<MeshAssetsSystem>>,
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
    pub local_to_world: float4x4,
}
pub struct SimpleMeshDrawer {
    pub vertices: Arc<Vec<float3>>,
    pub indices: Arc<Vec<u16>>,
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
    pub local_to_world: float4x4,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InstancingRenderer {
    Base(BaseInstancingRenderer),
    SimpleMesh(SimpleMeshInstancingRenderer),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BaseInstancingRenderer {
    pub mesh: Arc<AssetHandle<MeshAssetsSystem>>,
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleMeshInstancingRenderer {
    pub vertices: Arc<Vec<float3>>,
    pub indices: Arc<Vec<u16>>
}
impl Hash for SimpleMeshInstancingRenderer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.indices) as usize).hash(state);
        (Arc::as_ptr(&self.indices) as usize).hash(state);
    }
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

pub struct RenderPassNode {
    pub label: Option<&'static str>,
    pub attachments: Vec<ColorAttachmentId>,
    pub depth_stencil_attachment: Option<DepthAttachmentId>,
    pub vp_id: VPId,
    pub draws: Vec<Drawer>,
    pub draw_instances: Vec<InstancingDraw>,
    pub force_clear: bool,
    pub egui_draw: Option<EguiDraw>,
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
