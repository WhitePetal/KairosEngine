use std::{hash::Hash, sync::Arc};

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, MaterialAssetsSystem, MeshAssetsSystem},
    graphics::{
        attachment::{AttachmentLoadAction, AttachmentStoreAction},
        material::Material,
        vertex::Vertex,
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

pub enum Drawer {
    Base(BaseDraw),
    SimpleMesh(SimpleMeshDraw),
}

pub struct BaseDraw {
    pub mesh: Arc<AssetHandle<MeshAssetsSystem>>,
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
    pub local_to_world: float4x4,
}
pub struct SimpleMeshDraw {
    pub vertices: Arc<Vec<Vertex>>,
    pub indices: Arc<Vec<u16>>,
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
    pub local_to_world: float4x4,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InstancingRenderer {
    Base(BaseInstancingRenderer),
    SimpleMesh(SimpleMeshInstancingRenderer),
}
impl InstancingRenderer {
    pub fn get_vertices_indices<'a>(
        &'a self,
        assets_server: &'a AssetsServer,
    ) -> Option<(&'a Vec<Vertex>, &'a Vec<u16>)> {
        match self {
            InstancingRenderer::Base(base_instancing_renderer) => {
                base_instancing_renderer.get_vertices_indices(assets_server)
            }
            InstancingRenderer::SimpleMesh(simple_mesh_instancing_renderer) => {
                simple_mesh_instancing_renderer.get_vertices_indices()
            }
        }
    }
    pub fn get_material<'a>(&'a self, assets_server: &'a AssetsServer) -> Option<&'a Material> {
        match self {
            InstancingRenderer::Base(base_instancing_renderer) => {
                base_instancing_renderer.get_material(assets_server)
            }
            InstancingRenderer::SimpleMesh(simple_mesh_instancing_renderer) => {
                simple_mesh_instancing_renderer.get_material(assets_server)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BaseInstancingRenderer {
    pub mesh: Arc<AssetHandle<MeshAssetsSystem>>,
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
}
impl BaseInstancingRenderer {
    pub fn get_vertices_indices<'a>(
        &'a self,
        assets_server: &'a AssetsServer,
    ) -> Option<(&'a Vec<Vertex>, &'a Vec<u16>)> {
        let Some(mesh_asset) = assets_server.get(&self.mesh) else {
            return None;
        };
        Some((&mesh_asset.mesh.vertices, &mesh_asset.mesh.indices))
    }
    pub fn get_material<'a>(&'a self, assets_server: &'a AssetsServer) -> Option<&'a Material> {
        let Some(material) = assets_server.get(&self.material) else {
            return None;
        };
        Some(material)
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleMeshInstancingRenderer {
    pub vertices: Arc<Vec<Vertex>>,
    pub indices: Arc<Vec<u16>>,
    pub material: Arc<AssetHandle<MaterialAssetsSystem>>,
}
impl Hash for SimpleMeshInstancingRenderer {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.vertices) as usize).hash(state);
        (Arc::as_ptr(&self.indices) as usize).hash(state);
        self.material.hash(state);
    }
}
impl Eq for SimpleMeshInstancingRenderer {}
impl SimpleMeshInstancingRenderer {
    pub fn get_vertices_indices(&self) -> Option<(&Vec<Vertex>, &Vec<u16>)> {
        Some((&self.vertices, &self.indices))
    }
    pub fn get_material<'a>(&'a self, assets_server: &'a AssetsServer) -> Option<&'a Material> {
        let Some(material) = assets_server.get(&self.material) else {
            return None;
        };
        Some(material)
    }
}

#[derive(Debug)]
pub struct InstancingDraw {
    pub renderer: InstancingRenderer,
    pub local_to_worlds: Vec<float4x4>,
    pub sort_id: usize,
}
impl InstancingDraw {
    pub fn get_vertices_indices<'a>(
        &'a self,
        assets_server: &'a AssetsServer,
    ) -> Option<(&'a Vec<Vertex>, &'a Vec<u16>)> {
        self.renderer.get_vertices_indices(assets_server)
    }
    pub fn get_material<'a>(&'a self, assets_server: &'a AssetsServer) -> Option<&'a Material> {
        self.renderer.get_material(assets_server)
    }
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
    pub attachments: Vec<ColorAttachmentBind>,
    pub depth_stencil_attachment: Option<DepthAttachmentBind>,
    pub vp_id: VPId,
    pub draws: Vec<Drawer>,
    pub draw_instances: Vec<InstancingDraw>,
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
