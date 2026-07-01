use std::sync::Arc;

use crate::{
    asset_loader::assets::{AssetHandle, MaterialAssetsSystem, MeshAssetsSystem},
    graphics::{
        attachment::Attachment,
        graphics_graph::graphics_node::{
            BaseDraw, BindAttachmentToEguiNode, ColorAttachmentId, CopyAttachmentToEguiNode, DepthAttachmentId, Drawer, EguiDraw, GraphNode, OutputToFrameBufferNode, RenderPassNode, VPId
        },
    },
    math::float4x4,
};

enum RenderPassState {
    None,
    Writing(RenderPassNode),
    Cloused,
}

pub struct GraphicsCommand {
    pub attachments: Vec<Attachment>,
    pub depth_attachments: Vec<Attachment>,
    pub vp_buffers: Vec<float4x4>,
    pub nodes: Vec<GraphNode>,
    cur_render_pass: RenderPassState,
}

impl GraphicsCommand {
    pub fn new(
        attachments_capacity: usize,
        depth_attachments_capacity: usize,
        vp_buffers_capcacity: usize,
        nodes_capacity: usize,
    ) -> Self {
        Self {
            attachments: Vec::with_capacity(attachments_capacity),
            depth_attachments: Vec::with_capacity(depth_attachments_capacity),
            vp_buffers: Vec::with_capacity(vp_buffers_capcacity),
            nodes: Vec::with_capacity(nodes_capacity),
            cur_render_pass: RenderPassState::None,
        }
    }

    pub fn create_color_attachment(&mut self, attachment: Attachment) -> ColorAttachmentId {
        let id = self.attachments.len();
        self.attachments.push(attachment);
        ColorAttachmentId(id)
    }

    pub fn create_depth_attachment(&mut self, attachment: Attachment) -> DepthAttachmentId {
        let id = self.depth_attachments.len();
        self.depth_attachments.push(attachment);
        DepthAttachmentId(id)
    }

    pub fn set_view_projection_matrix(&mut self, matrix: float4x4) -> VPId {
        let id = self.vp_buffers.len();
        self.vp_buffers.push(matrix);
        VPId(id)
    }

    pub fn begin_render_pass(
        &mut self,
        label: Option<&'static str>,
        attachments: Vec<ColorAttachmentId>,
        depth_attachment: Option<DepthAttachmentId>,
        vp_id: VPId,
        darws_capacity: usize,
        force_clear: bool,
    ) {
        debug_assert!(
            matches!(
                self.cur_render_pass,
                RenderPassState::None | RenderPassState::Cloused
            ),
            "begin a render pass while another render pass not be end!"
        );
        debug_assert!(
            attachments
                .iter()
                .enumerate()
                .all(|(index, attachment)| !attachments[..index].contains(attachment)),
            "begin a render pass with duplicated color attachments: {attachments:?}"
        );

        let render_pass_node = RenderPassNode {
            label,
            attachments,
            depth_stencil_attachment: depth_attachment,
            vp_id,
            force_clear,
            draws: Vec::with_capacity(darws_capacity),
            draw_instances: Vec::new(),
            egui_draw: None,
            gizmo_grid: None,
        };

        self.cur_render_pass = RenderPassState::Writing(render_pass_node)
    }

    pub fn end_render_pass(&mut self) {
        debug_assert!(
            matches!(self.cur_render_pass, RenderPassState::Writing(_)),
            "end a render pass while no render pass be opened"
        );

        let render_pass = std::mem::replace(&mut self.cur_render_pass, RenderPassState::Cloused);

        let RenderPassState::Writing(render_pass) = render_pass else {
            unreachable!()
        };

        self.nodes.push(GraphNode::RenderPass(render_pass));
    }

    pub fn draw(
        &mut self,
        mesh: Arc<AssetHandle<MeshAssetsSystem>>,
        material: Arc<AssetHandle<MaterialAssetsSystem>>,
        local_to_world: float4x4,
    ) {
        debug_assert!(
            matches!(self.cur_render_pass, RenderPassState::Writing(_)),
            "draw while no render pass be opened"
        );

        let RenderPassState::Writing(render_pass) = &mut self.cur_render_pass else {
            unreachable!()
        };

        let draw_call = Drawer::BaseDraw(BaseDraw {
            mesh,
            material,
            local_to_world,
        });
        render_pass.draws.push(draw_call);
    }

    pub fn draw_gizmo_grid(&mut self, gizmo_grid: GizmoGridDraw) {
        debug_assert!(
            matches!(self.cur_render_pass, RenderPassState::Writing(_)),
            "draw_gizmo_grid while no render pass be opened"
        );

        let RenderPassState::Writing(render_pass) = &mut self.cur_render_pass else {
            unreachable!()
        };
        render_pass.gizmo_grid = Some(gizmo_grid);
    }

    pub fn draw_egui(
        &mut self,
        paint_jobs: Vec<egui::ClippedPrimitive>,
        screen_descriptor: egui_wgpu::ScreenDescriptor,
        egui_update_textures: Vec<(egui::TextureId, epaint::ImageDelta)>,
    ) {
        debug_assert!(
            matches!(self.cur_render_pass, RenderPassState::Writing(_)),
            "draw while no render pass be opened"
        );

        let RenderPassState::Writing(render_pass) = &mut self.cur_render_pass else {
            unreachable!()
        };
        let draw_call = EguiDraw {
            paint_jobs,
            screen_descriptor,
            egui_update_textures,
        };
        render_pass.egui_draw = Some(draw_call);
    }

    pub fn bind_attachment_to_egui(
        &mut self,
        attachment_id: ColorAttachmentId,
        sender: tokio::sync::oneshot::Sender<egui::TextureId>,
    ) {
        debug_assert!(
            matches!(self.cur_render_pass, RenderPassState::Cloused),
            "bind attachment but render pass not close"
        );
        debug_assert!(
            {
                match self.nodes.last() {
                    Some(GraphNode::RenderPass(render_pass)) => {
                        render_pass.attachments.contains(&attachment_id)
                    }
                    _ => false,
                }
            },
            "bind attachment while pre node(if have) can't output to the attachment"
        );
        self.nodes
            .push(GraphNode::BindAttachmentToEgui(BindAttachmentToEguiNode {
                attachment_id,
                sender: Some(sender),
            }));
    }

    pub fn copy_attachment_to_egui(
        &mut self,
        attachment_id: ColorAttachmentId,
        egui_texture_id: egui::TextureId,
    ) {
        debug_assert!(
            matches!(self.cur_render_pass, RenderPassState::Cloused),
            "Output to framebuffer but render pass not close"
        );
        debug_assert!(
            matches!(self.cur_render_pass, RenderPassState::Cloused),
            "bind attachment but render pass not close"
        );
        debug_assert!(
            {
                match self.nodes.last() {
                    Some(GraphNode::RenderPass(render_pass)) => {
                        render_pass.attachments.contains(&attachment_id)
                    }
                    _ => false,
                }
            },
            "copy attachment while pre node(if have) can't output to the attachment"
        );
        self.nodes
            .push(GraphNode::CopyAttachmentToEGui(CopyAttachmentToEguiNode {
                attachment_id,
                egui_texture_id,
            }));
    }

    pub fn output_to_framebuffer(
        &mut self,
        attachment_id: ColorAttachmentId,
        egui_free_textures: Vec<epaint::TextureId>,
    ) {
        debug_assert!(
            matches!(self.cur_render_pass, RenderPassState::Cloused),
            "Output to framebuffer but render pass not close"
        );
        debug_assert!(
            {
                match self.nodes.last() {
                    Some(GraphNode::RenderPass(render_pass)) => {
                        render_pass.attachments.contains(&attachment_id)
                    }
                    _ => false,
                }
            },
            "output to framebuffer while pre node(if have) can't output to the attachment"
        );
        self.nodes
            .push(GraphNode::OutputToFrameBuffer(OutputToFrameBufferNode {
                attachment_id,
                egui_free_textures,
            }));
    }

    pub fn free_egui_texture_id(&mut self, texture_id: egui::TextureId) {
        self.nodes.push(GraphNode::FreeEguiTextureId(texture_id));
    }
}
