use crate::ecs::schedule::graph::{DiGraph, Direction, GraphNodeId, tarjan_scc::new_tarjan_scc};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Node(i32);

impl GraphNodeId for Node {
    type Adjacent = (Node, Direction);
    type Edge = (Node, Node);
    fn kind(&self) -> &'static str {
        ""
    }
}

#[test]
fn a_b_c_a() {
    let mut graph = DiGraph::<Node>::with_capacity(3, 3);
    graph.add_node(Node(1));
    graph.add_node(Node(2));
    graph.add_node(Node(3));
    graph.add_edge(Node(1), Node(2));
    graph.add_edge(Node(2), Node(3));
    graph.add_edge(Node(3), Node(1));

    let mut tarjan = new_tarjan_scc(&graph);
    let scc = tarjan.next().unwrap();
    let none = tarjan.next();
    assert_eq!(scc.len(), 3);
    assert!(scc.contains(&Node(1)));
    assert!(scc.contains(&Node(2)));
    assert!(scc.contains(&Node(3)));
    assert!(none.is_none());
}
