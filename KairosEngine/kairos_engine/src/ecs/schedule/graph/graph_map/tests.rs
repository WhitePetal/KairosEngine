use slotmap::SlotMap;

use crate::ecs::schedule::{NodeId, SystemKey, graph::DiGraph};

/// The `Graph` type _must_ preserve the order that nodes are inserted in if
/// no removals occur. Removals are permitted to swap the latest node into the
/// location of the removed node.
#[test]
fn node_order_preservation() {
    use NodeId::System;

    let mut slotmap = SlotMap::<SystemKey, ()>::with_key();
    let mut graph = DiGraph::<NodeId>::default();

    let sys1 = slotmap.insert(());
    let sys2 = slotmap.insert(());
    let sys3 = slotmap.insert(());
    let sys4 = slotmap.insert(());

    graph.add_node(System(sys1));
    graph.add_node(System(sys2));
    graph.add_node(System(sys3));
    graph.add_node(System(sys4));

    assert_eq!(
        graph.nodes().collect::<Vec<_>>(),
        vec![System(sys1), System(sys2), System(sys3), System(sys4)]
    );

    graph.remove_node(System(sys1));

    assert_eq!(
        graph.nodes().collect::<Vec<_>>(),
        vec![System(sys4), System(sys2), System(sys3)]
    );

    graph.remove_node(System(sys4));

    assert_eq!(
        graph.nodes().collect::<Vec<_>>(),
        vec![System(sys3), System(sys2)]
    );

    graph.remove_node(System(sys2));

    assert_eq!(graph.nodes().collect::<Vec<_>>(), vec![System(sys3)]);

    graph.remove_node(System(sys3));

    assert_eq!(graph.nodes().collect::<Vec<_>>(), vec![]);
}

/// Nodes that have bidirectional edges (or any edge in the case of undirected graphs) are
/// considered strongly connected. A strongly connected component is a collection of
/// nodes where there exists a path from any node to any other node in the collection.
#[test]
fn strongly_connected_components() {
    use NodeId::System;

    let mut slotmap = SlotMap::<SystemKey, ()>::with_key();
    let mut graph = DiGraph::<NodeId>::default();

    let sys1 = slotmap.insert(());
    let sys2 = slotmap.insert(());
    let sys3 = slotmap.insert(());
    let sys4 = slotmap.insert(());
    let sys5 = slotmap.insert(());
    let sys6 = slotmap.insert(());

    graph.add_edge(System(sys1), System(sys2));
    graph.add_edge(System(sys2), System(sys1));

    graph.add_edge(System(sys2), System(sys3));
    graph.add_edge(System(sys3), System(sys2));

    graph.add_edge(System(sys4), System(sys5));
    graph.add_edge(System(sys5), System(sys4));

    graph.add_edge(System(sys6), System(sys2));

    let sccs = graph
        .iter_sccs()
        .map(|scc| scc.to_vec())
        .collect::<Vec<_>>();

    assert_eq!(
        sccs,
        vec![
            vec![System(sys3), System(sys2), System(sys1)],
            vec![System(sys5), System(sys4)],
            vec![System(sys6)]
        ]
    );
}
