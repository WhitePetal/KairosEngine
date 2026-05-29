use std::{
    collections::{HashMap, HashSet},
    hint::unreachable_unchecked,
};

use petgraph::{
    Direction::{self, Incoming, Outgoing},
    graph::NodeIndex,
    stable_graph::StableDiGraph,
    visit::EdgeRef,
};
use tokio::sync::mpsc::Sender;

use crate::{
    graphics::{attachment::Attachment, mesh::Mesh},
    math::float4x4,
};

pub struct BaseDraw {
    pub mesh: Mesh,
}
struct EguiDraw {
    paint_jobs: Vec<egui::ClippedPrimitive>,
    screen_descriptor: egui_wgpu::ScreenDescriptor,
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
    pub attachments: Vec<usize>,
    pub vp_id: usize,
    pub draws: Vec<BaseDraw>,
    pub force_clear: bool,
    pub egui_draw: Option<EguiDraw>,
}
pub struct OutputToFrameBufferNode {
    attachment_id: usize,
}
pub struct BindAttachmentToEguiNode {
    attachment_id: usize,
    sender: Sender<egui::TextureId>,
}
pub struct CopyAttachmentToEguiNode {
    attachment_id: usize,
    egui_texture_id: egui::TextureId,
}

enum RenderPassState {
    None,
    Writing(RenderPassNode),
    Cloused,
}

pub struct GraphicsCommand {
    attachments: Vec<Attachment>,
    vp_buffers: Vec<float4x4>,
    nodes: Vec<GraphNode>,
    cur_render_pass: RenderPassState,
}

pub struct GraphicsGraph {
    pub attachments: Vec<Attachment>,
    pub vps: Vec<float4x4>,
    pub graph: StableDiGraph<GraphNode, usize>,
    pub ending_nodes: Vec<NodeIndex>,
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
            cur_render_pass: RenderPassState::None,
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
        label: Option<&'static str>,
        attachments: Vec<usize>,
        vp_id: usize,
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

        let render_pass_node = RenderPassNode {
            label,
            attachments,
            vp_id,
            force_clear,
            draws: Vec::with_capacity(darws_capacity),
            egui_draw: None,
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
            unsafe { unreachable_unchecked() }
        };

        self.nodes.push(GraphNode::RenderPass(render_pass));
    }

    pub fn draw(&mut self, mesh: Mesh) {
        debug_assert!(
            matches!(self.cur_render_pass, RenderPassState::Writing(_)),
            "draw while no render pass be opened"
        );

        let RenderPassState::Writing(render_pass) = &mut self.cur_render_pass else {
            unsafe { unreachable_unchecked() }
        };

        let draw_call = BaseDraw { mesh };
        render_pass.draws.push(draw_call);
    }

    pub fn draw_egui(
        &mut self,
        paint_jobs: Vec<egui::ClippedPrimitive>,
        screen_descriptor: egui_wgpu::ScreenDescriptor,
    ) {
        debug_assert!(
            matches!(self.cur_render_pass, RenderPassState::Writing(_)),
            "draw while no render pass be opened"
        );

        let RenderPassState::Writing(render_pass) = &mut self.cur_render_pass else {
            unsafe { unreachable_unchecked() }
        };
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
                sender,
            }));
    }

    pub fn copy_attachment_to_egui(
        &mut self,
        attachment_id: usize,
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

    pub fn output_to_framebuffer(&mut self, attachment_id: usize) {
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
            }));
    }
}

impl GraphicsGraph {
    pub fn build(commands: Vec<GraphicsCommand>) -> Self {
        let mut graph_attachments = Vec::<Attachment>::new();
        let mut graph_vps = Vec::<float4x4>::new();

        // build the graphs
        let mut graphics = Vec::with_capacity(commands.len());
        let mut max_capacity: usize = 0;
        let mut graph_id = 0usize;

        commands.into_iter().for_each(|command| {
            let graph_capacity = command.nodes.len() << 1;
            max_capacity = graph_capacity.max(max_capacity);
            let mut graph = StableDiGraph::with_capacity(graph_capacity, graph_capacity);

            let mut attachments = command.attachments;
            let mut vps = command.vp_buffers;

            let mut writed_attachments =
                HashMap::<usize, NodeIndex>::with_capacity(attachments.len());

            let graph_attachments_start = graph_attachments.len();
            graph_attachments.append(&mut attachments);

            let graph_vp_start = graph_vps.len();
            graph_vps.append(&mut vps);

            command.nodes.into_iter().for_each(|node| match node {
                GraphNode::RenderPass(mut render_pass_node) => {
                    let node = graph.add_node(GraphNode::None);

                    for need_attachment_id in &render_pass_node.attachments {
                        let prev = writed_attachments.get(&need_attachment_id);
                        if let Some(prev) = prev {
                            graph.add_edge(*prev, node, 1usize);
                        };

                        writed_attachments.insert(*need_attachment_id, node);
                    }

                    render_pass_node
                        .attachments
                        .iter_mut()
                        .for_each(|attachment| *attachment += graph_attachments_start);

                    render_pass_node.vp_id += graph_vp_start;

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
                GraphNode::OutputToFrameBuffer(output_to_frame_buffer_node) => {
                    let prev = writed_attachments.get(&output_to_frame_buffer_node.attachment_id);
                    let node =
                        graph.add_node(GraphNode::OutputToFrameBuffer(output_to_frame_buffer_node));
                    if let Some(prev) = prev {
                        graph.add_edge(*prev, node, 1usize);
                    }
                }
            });

            graphics.push(graph);
            graph_id += 1;
        });

        // combine graphs
        let mut graph =
            StableDiGraph::<GraphNode, usize>::with_capacity(max_capacity, max_capacity);
        graphics.into_iter().for_each(|g| {
            let mut remap = HashMap::new();

            let (nodes, edges) = g.into_nodes_edges_iters();

            for node in nodes {
                remap.insert(node.index, graph.add_node(node.weight));
            }

            for edge in edges {
                graph.add_edge(edge.source, edge.target, edge.weight);
            }
        });

        // optimize the graph
        // 1. 合并render_pass
        let ending_nodes = graph.externals(Outgoing).collect();
        Self::combine_nodes(&mut graph, &ending_nodes);
        // 2. 删除没有最终输出的链路
        Self::prune_graph(&mut graph, &ending_nodes);
        // 3. optimize per node in graph

        Self {
            graph,
            attachments: graph_attachments,
            vps: graph_vps,
            ending_nodes,
        }
    }

    fn prune_graph(graph: &mut StableDiGraph<GraphNode, usize>, outpus: &[NodeIndex]) {
        let mut keep = HashSet::<NodeIndex>::new();
        let mut stack = outpus.to_vec();

        while let Some(node) = stack.pop() {
            if !keep.insert(node) {
                continue;
            }

            for dependency in graph.neighbors_directed(node, Incoming) {
                stack.push(dependency);
            }
        }

        graph.retain_nodes(|_, node| keep.contains(&node));
    }

    fn combine_nodes(graph: &mut StableDiGraph<GraphNode, usize>, ending_nodes: &Vec<NodeIndex>) {
        for ending_node in ending_nodes {
            Self::merge_pre_node(graph, *ending_node);
        }
    }

    fn merge_pre_node(graph: &mut StableDiGraph<GraphNode, usize>, next_node: NodeIndex) {
        let mut next_node = next_node;
        if let GraphNode::RenderPass(next_pass) = &graph[next_node] {
            let pre_node = graph
                .neighbors_directed(next_node, Incoming)
                .find(|pre_node| {
                    if let GraphNode::RenderPass(pre_pass) = &graph[*pre_node] {
                        Self::can_merge_render_pass(pre_pass, next_pass)
                    } else {
                        false
                    }
                });

            if let Some(pre_node) = pre_node {
                Self::merge_render_pass_pair(graph, pre_node, next_node);
                next_node = pre_node;
            };
        };

        let pre_nodes = graph
            .edges_directed(next_node, Incoming)
            .map(|edge| edge.source())
            .collect::<Vec<NodeIndex>>();
        for pre_node in pre_nodes {
            Self::merge_pre_node(graph, pre_node);
        }
    }

    fn can_merge_render_pass(pre_pass: &RenderPassNode, next_pass: &RenderPassNode) -> bool {
        for need_attachment_id in &next_pass.attachments {
            if !pre_pass.attachments.contains(need_attachment_id) {
                return false;
            }
        }

        true
    }

    fn merge_render_pass_pair(
        graph: &mut StableDiGraph<GraphNode, usize>,
        pre_node: NodeIndex,
        next_node: NodeIndex,
    ) {
        let GraphNode::RenderPass(mut next_pass) =
            std::mem::replace(&mut graph[next_node], GraphNode::None)
        else {
            return;
        };

        let GraphNode::RenderPass(pre_pass) = &mut graph[pre_node] else {
            graph[next_node] = GraphNode::RenderPass(next_pass);
            return;
        };

        // 1. next_pass 内数据转移到 pre_pass
        pre_pass.draws.append(&mut next_pass.draws);
        if next_pass.egui_draw.is_some() {
            pre_pass.egui_draw = next_pass.egui_draw.take();
        }

        // 2. pre_pass output -> next_pass output
        let outgong = graph
            .edges_directed(next_node, Outgoing)
            .map(|edge| (edge.target(), *edge.weight()))
            .collect::<Vec<_>>();

        for (target, edge) in outgong {
            graph.update_edge(pre_node, target, edge);
        }

        graph.remove_node(next_node);
    }
}
