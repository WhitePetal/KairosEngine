use std::collections::{HashMap, HashSet};

use petgraph::{
    Direction::{Incoming, Outgoing},
    graph::NodeIndex,
    stable_graph::StableDiGraph,
    visit::EdgeRef,
};

use crate::{
    graphics::{
        attachment::{Attachment, AttachmentLoadAction},
        graphics_graph::{
            GraphicsCommand,
            graphics_node::{
                BaseInstancingRenderer, GraphNode, InstancingDraw, InstancingRenderer,
                RenderPassNode, SimpleMeshInstancingRenderer,
            },
        },
    },
    math::float4x4,
};

pub struct GraphicsGraph {
    pub attachments: Vec<Attachment>,
    pub depth_attachments: Vec<Attachment>,
    pub vps: Vec<float4x4>,
    pub graph: StableDiGraph<GraphNode, usize>,
    pub ending_nodes: Vec<NodeIndex>,
    pub free_egui_textures: Vec<egui::TextureId>,
}

impl GraphicsGraph {
    pub fn build(commands: Vec<GraphicsCommand>) -> Self {
        let mut graph_attachments = Vec::<Attachment>::new();
        let mut graph_depth_attachments = Vec::<Attachment>::new();
        let mut graph_vps = Vec::<float4x4>::new();
        let mut graph_free_egui_textures = Vec::<egui::TextureId>::new();

        // build the graphs
        let mut graphics = Vec::with_capacity(commands.len());
        let mut max_capacity: usize = 0;
        let mut graph_id = 0usize;

        commands.into_iter().for_each(|command| {
            let graph_capacity = command.nodes.len() << 1;
            max_capacity = graph_capacity.max(max_capacity);
            let mut graph = StableDiGraph::with_capacity(graph_capacity, graph_capacity);

            let mut attachments = command.attachments;
            let mut depth_attachments = command.depth_attachments;
            let mut vps = command.vp_buffers;

            let mut writed_attachments =
                HashMap::<usize, NodeIndex>::with_capacity(attachments.len());

            let graph_attachments_start = graph_attachments.len();
            graph_attachments.append(&mut attachments);

            let graph_depth_attachments_start = graph_depth_attachments.len();
            graph_depth_attachments.append(&mut depth_attachments);

            let graph_vp_start = graph_vps.len();
            graph_vps.append(&mut vps);

            command.nodes.into_iter().for_each(|node| match node {
                GraphNode::RenderPass(mut render_pass_node) => {
                    let node = graph.add_node(GraphNode::None);

                    for need_attachment_bind in &render_pass_node.attachments {
                        let prev = writed_attachments.get(&need_attachment_bind.id.0);
                        if let Some(prev) = prev {
                            graph.add_edge(*prev, node, 1usize);
                        };

                        writed_attachments.insert(need_attachment_bind.id.0, node);
                    }

                    render_pass_node
                        .attachments
                        .iter_mut()
                        .for_each(|attachment| {
                            attachment.id.0 += graph_attachments_start;
                        });

                    if let Some(depth_stencil_attachment) =
                        &mut render_pass_node.depth_stencil_attachment
                    {
                        depth_stencil_attachment.id.0 =
                            depth_stencil_attachment.id.0 + graph_depth_attachments_start;
                    }

                    render_pass_node.vp_id.0 += graph_vp_start;

                    graph[node] = GraphNode::RenderPass(render_pass_node);
                }
                GraphNode::BindAttachmentToEgui(mut bind_attachment_to_egui_node) => {
                    let prev =
                        writed_attachments.get(&bind_attachment_to_egui_node.attachment_id.0);
                    bind_attachment_to_egui_node.attachment_id.0 += graph_attachments_start;

                    let node = graph.add_node(GraphNode::BindAttachmentToEgui(
                        bind_attachment_to_egui_node,
                    ));
                    if let Some(prev) = prev {
                        graph.add_edge(*prev, node, 1usize);
                    }
                }
                GraphNode::CopyAttachmentToEGui(_) => {
                    todo!()
                }
                GraphNode::None => {}
                GraphNode::OutputToFrameBuffer(output_to_frame_buffer_node) => {
                    let prev = writed_attachments.get(&output_to_frame_buffer_node.attachment_id.0);
                    let node =
                        graph.add_node(GraphNode::OutputToFrameBuffer(output_to_frame_buffer_node));
                    if let Some(prev) = prev {
                        graph.add_edge(*prev, node, 1usize);
                    }
                }
                GraphNode::FreeEguiTextureId(texture_id) => {
                    graph_free_egui_textures.push(texture_id);
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
                graph.add_edge(remap[&edge.source], remap[&edge.target], edge.weight);
            }
        });

        // optimize the graph
        // 1. 合并render_pass
        let ending_nodes = graph.externals(Outgoing).collect();
        Self::combine_nodes(&mut graph, &ending_nodes);
        // 2. 删除没有最终输出的链路
        Self::prune_graph(&mut graph, &ending_nodes);
        // 3. optimize per node in graph
        Self::optimize_nodes(&mut graph);

        // 4. sort the ending nodes ?
        // ending_nodes.sort_by_key(|node| match &graph[*node] {
        //     GraphNode::None => 100,
        //     GraphNode::RenderPass(render_pass_node) => 80,
        //     GraphNode::OutputToFrameBuffer(output_to_frame_buffer_node) => 0,
        //     GraphNode::BindAttachmentToEgui(bind_attachment_to_egui_node) => 200,
        //     GraphNode::CopyAttachmentToEGui(copy_attachment_to_egui_node) => 100,
        // });

        Self {
            graph,
            attachments: graph_attachments,
            depth_attachments: graph_depth_attachments,
            vps: graph_vps,
            ending_nodes,
            free_egui_textures: graph_free_egui_textures,
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
        // depth: must match by id, and next cannot LoadClear (would discard pre output)
        match (
            &pre_pass.depth_stencil_attachment,
            &next_pass.depth_stencil_attachment,
        ) {
            (Some(pre_depth), Some(next_depth)) => {
                if pre_depth.id != next_depth.id {
                    return false;
                }
                if let Some((load, _)) = next_depth.depth_load_store_action
                    && load == AttachmentLoadAction::LoadClear
                {
                    return false;
                }
                if let Some((load, _)) = next_depth.stencil_load_store_action
                    && load == AttachmentLoadAction::LoadClear
                {
                    return false;
                }
            }
            (None, None) => {}
            _ => return false,
        }

        // color attachments: next must be a prefix of pre (same ids, same order)
        if next_pass.attachments.len() > pre_pass.attachments.len() {
            return false;
        }
        for (pre_bind, next_bind) in pre_pass
            .attachments
            .iter()
            .zip(next_pass.attachments.iter())
        {
            if pre_bind.id != next_bind.id {
                return false;
            }
            if next_bind.load_action == AttachmentLoadAction::LoadClear {
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

    fn optimize_nodes(graph: &mut StableDiGraph<GraphNode, usize>) {
        graph.node_weights_mut().for_each(|node| match node {
            GraphNode::None => {}
            GraphNode::RenderPass(render_pass_node) => {
                let mut instances = HashMap::<InstancingRenderer, InstancingDraw>::new();
                let mut instance_count = 0;
                for draw in render_pass_node.draws.drain(..) {
                    let renderer;
                    let local_to_world;
                    match draw {
                        super::graphics_node::Drawer::Base(base_draw) => {
                            renderer = InstancingRenderer::Base(BaseInstancingRenderer {
                                mesh: base_draw.mesh.clone(),
                                material: base_draw.material.clone(),
                            });
                            local_to_world = base_draw.local_to_world;
                        }
                        super::graphics_node::Drawer::SimpleMesh(simple_mesh_draw) => {
                            renderer =
                                InstancingRenderer::SimpleMesh(SimpleMeshInstancingRenderer {
                                    vertices: simple_mesh_draw.vertices.clone(),
                                    indices: simple_mesh_draw.indices.clone(),
                                    material: simple_mesh_draw.material.clone(),
                                });
                            local_to_world = simple_mesh_draw.local_to_world;
                        }
                    }
                    if let Some(instance) = instances.get_mut(&renderer) {
                        instance.local_to_worlds.push(local_to_world);
                    } else {
                        let instance = InstancingDraw {
                            renderer: renderer.clone(),
                            local_to_worlds: vec![local_to_world],
                            sort_id: instance_count,
                        };
                        instance_count += 1;
                        instances.insert(renderer, instance);
                    }
                }
                let mut instances = instances.into_values().collect::<Vec<_>>();
                instances.sort_unstable_by(|x, y| ::core::cmp::Ord::cmp(&x.sort_id, &y.sort_id));

                render_pass_node.draw_instances = instances;
            }
            GraphNode::OutputToFrameBuffer(_) => {}
            GraphNode::BindAttachmentToEgui(_) => {}
            GraphNode::CopyAttachmentToEGui(_) => {}
            GraphNode::FreeEguiTextureId(_) => {}
        });
    }
}
