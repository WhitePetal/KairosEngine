use std::{collections::HashMap, mem::take};

use petgraph::{
    graph::{DiGraph, NodeIndex},
    visit::EdgeRef,
};
use tokio::sync::mpsc::Sender;

use crate::{
    graphics::{attachment::Attachment, mesh::Mesh},
    math::float4x4,
};

struct BaseDraw {
    mesh: Mesh,
}
struct EguiDraw {
    paint_jobs: Vec<egui::ClippedPrimitive>,
    screen_descriptor: egui_wgpu::ScreenDescriptor,
}

enum GraphNode {
    None,
    RenderPass(RenderPassNode),
    BindAttachmentToEgui(BindAttachmentToEguiNode),
    CopyAttachmentToEGui(CopyAttachmentToEguiNode),
}

struct RenderPassNode {
    attachments: Vec<usize>,
    vp_id: usize,
    draws: Vec<BaseDraw>,
    egui_draw: Option<EguiDraw>,
}
struct BindAttachmentToEguiNode {
    attachment_id: usize,
    sender: Sender<egui::TextureId>,
}
struct CopyAttachmentToEguiNode {
    attachment_id: usize,
    egui_texture_id: egui::TextureId,
}

pub struct GraphicsCommand {
    attachments: Vec<Attachment>,
    vp_buffers: Vec<float4x4>,
    nodes: Vec<GraphNode>,
    cur_render_pass: Option<RenderPassNode>,
}

pub struct GraphicsGraph {
    graph: DiGraph<GraphNode, usize>,
}

impl GraphicsCommand {
    pub fn new(
        attachments_capacity: usize,
        vp_buffers_capcacity: usize,
        nodes_capacity: usize,
    ) -> Self {
        Self {
            attachments: Vec::with_capacity(attachments_capacity),
            vp_buffers: Vec::with_capacity(vp_buffers_capcacity),
            nodes: Vec::with_capacity(nodes_capacity),
            cur_render_pass: None,
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

    pub fn begin_render_pass(
        &mut self,
        attachments: Vec<usize>,
        vp_id: usize,
        darws_capacity: usize,
        force_clear: bool,
    ) {
        debug_assert!(
            self.cur_render_pass.is_none(),
            "begin a render pass while another render pass not be end!"
        );

        let render_pass_node = RenderPassNode {
            attachments,
            vp_id,
            draws: Vec::with_capacity(darws_capacity),
            egui_draw: None,
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
        let draw_call = BaseDraw { mesh };
        render_pass.draws.push(draw_call);
    }

    pub fn draw_egui(
        &mut self,
        paint_jobs: Vec<egui::ClippedPrimitive>,
        screen_descriptor: egui_wgpu::ScreenDescriptor,
    ) {
        debug_assert!(
            self.cur_render_pass.is_some(),
            "draw while no render pass be opened"
        );

        let render_pass = unsafe { self.cur_render_pass.as_mut().unwrap_unchecked() };
        let draw_call = EguiDraw {
            paint_jobs,
            screen_descriptor,
        };
        render_pass.egui_draw = Some(draw_call);
    }

    pub fn bind_attachment_to_egui(
        &mut self,
        attachment_id: usize,
        sender: Sender<egui::TextureId>,
    ) {
        self.nodes
            .push(GraphNode::BindAttachmentToEgui(BindAttachmentToEguiNode {
                attachment_id,
                sender,
            }));
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
    pub fn build(commands: Vec<GraphicsCommand>) -> Self {
        // build the graphs
        let mut graphics = Vec::with_capacity(commands.len());
        let mut max_capacity: usize = 0;
        commands.into_iter().for_each(|command| {
            let graph_capacity = command.nodes.len() << 1;
            max_capacity = graph_capacity.max(max_capacity);
            let mut graph = DiGraph::with_capacity(graph_capacity, graph_capacity);
            let attachments = command.attachments;
            let mut writed_attachments =
                HashMap::<usize, NodeIndex>::with_capacity(attachments.len());
            command.nodes.into_iter().for_each(|node| match node {
                GraphNode::RenderPass(render_pass_node) => {
                    let node = graph.add_node(GraphNode::None);

                    for need_attachment_id in &render_pass_node.attachments {
                        let prev = writed_attachments.get(&need_attachment_id);
                        if let Some(prev) = prev {
                            graph.add_edge(*prev, node, 1usize);
                        };

                        writed_attachments.insert(*need_attachment_id, node);
                    }

                    graph[node] = GraphNode::RenderPass(render_pass_node);
                }
                GraphNode::BindAttachmentToEgui(bind_attachment_to_egui_node) => {
                    let prev = writed_attachments.get(&bind_attachment_to_egui_node.attachment_id);
                    let node = graph.add_node(GraphNode::BindAttachmentToEgui(
                        bind_attachment_to_egui_node,
                    ));
                    if let Some(prev) = prev {
                        graph.add_edge(*prev, node, 1usize);
                    }
                }
                GraphNode::CopyAttachmentToEGui(copy_attachment_to_egui_node) => {
                    todo!()
                }
                GraphNode::None => {}
            });

            graphics.push(graph);
        });

        // combine graphs
        let mut graph = DiGraph::<GraphNode, usize>::with_capacity(max_capacity, max_capacity);
        graphics.into_iter().for_each(|mut g| {
            let mut remap = HashMap::new();
            for idx in g.node_indices() {
                let weight = std::mem::replace(&mut g[idx], GraphNode::None);
                remap.insert(idx, graph.add_node(weight));
            }
            for edge in g.edge_references() {
                let a = remap[&edge.source()];
                let b = remap[&edge.target()];
                graph.add_edge(a, b, *edge.weight());
            }
        });

        // optimize the graph
        // 1. 删除没有最终输出的链路
        // 2. 合并render_pass

        // optimize per node in graph

        Self { graph }
    }
}
