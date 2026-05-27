use tokio::sync::mpsc::Sender;

use crate::{
    graphics::{attachment::Attachment, mesh::Mesh},
    math::float4x4,
};

enum GraphNode {
    CreateAttachment(CreateAttachmentNode),
    SetVPMatrix(SetVPMatrixNode),
    BeginRenderPass(BeginRenderPassNode),
    EndRenderPass(EndRenderPassNode),
    Draw(DrawNode),
    BindAttachmentToEgui(BindAttachmentToEguiNode),
    CopyAttachmentToEGui(CopyAttachmentToEguiNode),
}

struct CreateAttachmentNode {
    attachment: Attachment,
    id: usize,
}
struct SetVPMatrixNode {
    matrix: float4x4,
    id: usize,
}
struct BeginRenderPassNode {
    id: usize,
    attachments: Vec<usize>,
    vp_id: usize,
    force_clear: bool,
}
struct EndRenderPassNode {
    id: usize,
}
struct DrawNode {
    mesh: Mesh,
    render_pass_id: usize,
}
struct BindAttachmentToEguiNode {
    attachment_id: usize,
}
struct CopyAttachmentToEguiNode {
    attachment_id: usize,
    egui_texture_id: egui::TextureId,
}

pub struct GraphicsCommand {
    nodes: Vec<GraphNode>,
    attachment_count: usize,
    vp_buffer_count: usize,
    render_pass_count: usize,
    cur_render_pass_id: Option<usize>,
}

impl GraphicsCommand {
    pub fn new(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            attachment_count: 0,
            vp_buffer_count: 0,
            render_pass_count: 0,
            cur_render_pass_id: None,
        }
    }

    pub fn create_attachment(&mut self, attachment: Attachment) -> usize {
        let id = self.attachment_count;
        self.attachment_count += 1;
        self.nodes
            .push(GraphNode::CreateAttachment(CreateAttachmentNode {
                attachment,
                id,
            }));
        id
    }

    pub fn set_view_projection_matrix(&mut self, matrix: float4x4) -> usize {
        let id = self.vp_buffer_count;
        self.vp_buffer_count += 1;
        self.nodes
            .push(GraphNode::SetVPMatrix(SetVPMatrixNode { matrix, id }));
        id
    }

    pub fn begin_render_pass(&mut self, attachments: Vec<usize>, vp_id: usize, force_clear: bool) {
        debug_assert!(
            self.cur_render_pass_id.is_none(),
            "begin a render pass while another render pass not be end!"
        );
        if self.cur_render_pass_id != None {
            return;
        }

        let id = self.render_pass_count;
        self.render_pass_count += 1;
        self.nodes
            .push(GraphNode::BeginRenderPass(BeginRenderPassNode {
                id,
                attachments,
                vp_id,
                force_clear,
            }));
        self.cur_render_pass_id = Some(id);
    }

    pub fn end_render_pass(&mut self) {
        debug_assert!(
            self.cur_render_pass_id.is_some(),
            "end a render pass while no render pass be opened"
        );

        let id = unsafe { self.cur_render_pass_id.unwrap_unchecked() };

        self.nodes
            .push(GraphNode::EndRenderPass(EndRenderPassNode { id }));

        self.cur_render_pass_id = None
    }

    pub fn draw(&mut self, mesh: Mesh) {
        debug_assert!(
            self.cur_render_pass_id.is_some(),
            "draw while no render pass be opened"
        );

        let render_pass_id = unsafe { self.cur_render_pass_id.unwrap_unchecked() };

        self.nodes.push(GraphNode::Draw(DrawNode {
            mesh,
            render_pass_id,
        }));
    }

    pub fn bind_attachment_to_egui(&mut self, attachment_id: usize, sender: Sender<egui::TextureId>) {
        self.nodes.push(GraphNode::BindAttachmentToEgui(BindAttachmentToEguiNode { attachment_id }));
    }

    pub fn copy_attachment_to_egui(
        &mut self,
        attachment_id: usize,
        egui_texture_id: egui::TextureId,
    ) {
        self.nodes
            .push(GraphNode::CopyAttachmentToEGui(CopyAttachmentToEguiNode {
                attachment_id,
                egui_texture_id,
            }));
    }
}

pub struct GraphicsGraph {}

impl GraphicsGraph {
    pub fn from_commands(commands: &[GraphicsCommand]) -> Self {
        let command = &commands[0];

        let create_attachment_nodes_iter = command.nodes.iter().filter(|node| {
            match node {
                GraphNode::CreateAttachment(_) => true,
                _ => false
            }
        });

        // create_attacments 纯 output 
        // begin_render_pass

        // attachment_node -> root_node (begin_render_pass) -> draw_node 

        todo!()
    }
}
