use petgraph::graph::DiGraph;
use tokio::sync::mpsc::Sender;

use crate::{
    graphics::{attachment::Attachment, mesh::Mesh}, kairos_editor::ui::docking_tab::dock_state::tree::node, math::float4x4
};

struct BaseDraw {
    mesh: Mesh,
}

enum GraphNode {
    RenderPass(RenderPassNode),
    BindAttachmentToEgui(BindAttachmentToEguiNode),
    CopyAttachmentToEGui(CopyAttachmentToEguiNode),
}

struct  RenderPassNode {
    attachments: Vec<usize>,
    vp_id: usize,
    draws: Vec<BaseDraw>,
}
struct BindAttachmentToEguiNode {
    attachment_id: usize,
    sender: Sender<egui::TextureId>,
}
struct CopyAttachmentToEguiNode {
    attachment_id: usize,
    egui_texture_id: egui::TextureId,
}

pub struct GraphicsGraph {
    attachments: Vec<Attachment>,
    vp_buffers: Vec<float4x4>,
    nodes: Vec<GraphNode>,
    cur_render_pass: Option<RenderPassNode>,
    graph: DiGraph<GraphNode, usize>
}

impl GraphicsGraph {
    pub fn new(
        attachments_capacity: usize,
        vp_buffers_capcacity: usize,
        nodes_capacity: usize
    ) -> Self {
        Self {
            attachments: Vec::with_capacity(attachments_capacity),
            vp_buffers: Vec::with_capacity(vp_buffers_capcacity),
            nodes: Vec::with_capacity(nodes_capacity),
            cur_render_pass: None,
            graph: DiGraph::with_capacity(nodes_capacity << 1, nodes_capacity << 1)
        }
    }

    pub fn create_attachment(&mut self, attachment: Attachment) -> usize {
        let id = self.attachments.len();
        self.attachments.push(attachment);
        id
    }

    pub fn set_view_projection_matrix(&mut self, matrix: float4x4) -> usize {
        let id = self.vp_buffers.len();
        self.vp_buffers.push(matrix);
        id
    }

    pub fn begin_render_pass(&mut self, attachments: Vec<usize>, vp_id: usize, darws_capacity: usize, force_clear: bool) {
        debug_assert!(
            self.cur_render_pass.is_none(),
            "begin a render pass while another render pass not be end!"
        );

        let render_pass_node = RenderPassNode {
            attachments,
            vp_id,
            draws: Vec::with_capacity(darws_capacity),
        };

        self.cur_render_pass = Some(render_pass_node)
    }

    pub fn end_render_pass(&mut self) {
        debug_assert!(
            self.cur_render_pass.is_some(),
            "end a render pass while no render pass be opened"
        );

        let render_pass = self.cur_render_pass.take();
        let render_pass = unsafe { render_pass.unwrap_unchecked() };

        self.nodes.push(GraphNode::RenderPass(render_pass));
    }

    pub fn draw(&mut self, mesh: Mesh) {
        debug_assert!(
            self.cur_render_pass.is_some(),
            "draw while no render pass be opened"
        );

        let render_pass = unsafe { self.cur_render_pass.as_mut().unwrap_unchecked() };
        let draw_call = BaseDraw {
            mesh
        };
        render_pass.draws.push(draw_call);
    }

    pub fn bind_attachment_to_egui(&mut self, attachment_id: usize, sender: Sender<egui::TextureId>) {
        self.nodes.push(GraphNode::BindAttachmentToEgui(BindAttachmentToEguiNode { attachment_id, sender }));
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

impl GraphicsGraph {
    pub fn build(&mut self) {
        // build the graph
        for node in &self.nodes {
            match node {
                GraphNode::RenderPass(render_pass_node) => {
                    let node = self.graph.add_node(GraphNode::RenderPass(render_pass_node));

                },
                GraphNode::BindAttachmentToEgui(bind_attachment_to_egui_node) => todo!(),
                GraphNode::CopyAttachmentToEGui(copy_attachment_to_egui_node) => todo!(),
            }
        }

        // optimize the graph

        // optimize per node in graph
    }
}
