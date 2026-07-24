use std::{hash::Hash, sync::Arc};

use crate::{
    asset_loader::assets::{AssetHandle, MaterialAssetsSystem, MeshAssetsSystem},
    graphics::{
        attachment::{AttachmentLoadAction, AttachmentStoreAction},
        egui_texture_handle::EguiTextureHandle,
    },
    math::float4x4,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColorAttachmentId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColorAttachmentBind {
    pub id: ColorAttachmentId,
    pub load_action: AttachmentLoadAction,
    pub store_action: AttachmentStoreAction,
}
impl ColorAttachmentBind {
    pub fn new(
        id: ColorAttachmentId,
        load: AttachmentLoadAction,
        store: AttachmentStoreAction,
    ) -> Self {
        Self {
            id,
            load_action: load,
            store_action: store,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DepthAttachmentId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DepthAttachmentBind {
    pub id: DepthAttachmentId,
    pub depth_load_store_action: Option<(AttachmentLoadAction, AttachmentStoreAction)>,
    pub stencil_load_store_action: Option<(AttachmentLoadAction, AttachmentStoreAction)>,
}
impl DepthAttachmentBind {
    pub fn new(
        id: DepthAttachmentId,
        depth_load_store: Option<(AttachmentLoadAction, AttachmentStoreAction)>,
        stencil_load_store: Option<(AttachmentLoadAction, AttachmentStoreAction)>,
    ) -> Self {
        Self {
            id,
            depth_load_store_action: depth_load_store,
            stencil_load_store_action: stencil_load_store,
        }
    }
}

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

#[derive(Debug)]
pub struct InstancingDraw {
    pub renderer: InstancingRenderer,
    pub local_to_worlds: Vec<float4x4>,
    pub sort_id: usize,
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
}

pub struct RenderPassNode {
    pub label: Option<&'static str>,
    pub attachments: Vec<ColorAttachmentBind>,
    pub depth_stencil_attachment: Option<DepthAttachmentBind>,
    pub vp_id: VPId,
    pub draws: Vec<BaseDraw>,
    pub draw_instances: Vec<InstancingDraw>,
    pub egui_draw: Option<EguiDraw>,
}
pub struct OutputToFrameBufferNode {
    pub attachment_id: ColorAttachmentId,
    pub egui_free_textures: Vec<epaint::TextureId>,
}
pub struct BindAttachmentToEguiNode {
    pub attachment_id: ColorAttachmentId,
    pub sender: Option<tokio::sync::oneshot::Sender<EguiTextureHandle>>,
}
pub struct CopyAttachmentToEguiNode {
    pub attachment_id: ColorAttachmentId,
    pub egui_texture_id: egui::TextureId,
}
