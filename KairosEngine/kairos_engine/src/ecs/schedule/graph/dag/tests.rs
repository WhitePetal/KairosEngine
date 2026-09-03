use std::ops::DerefMut;

use crate::ecs::schedule::graph::{Dag, Direction, GraphNodeId, UnGraph, index};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TestNode(u32);

impl GraphNodeId for TestNode {
    type Adjacent = (TestNode, Direction);
    type Edge = (TestNode, TestNode);

    fn kind(&self) -> &'static str {
        "test node"
    }
}

#[test]
fn mark_dirty() {
    {
        let mut dag = Dag::<TestNode>::new();
        dag.add_node(TestNode(1));
        assert!(dag.is_dirty());
    }
    {
        let mut dag = Dag::<TestNode>::new();
        dag.add_edge(TestNode(1), TestNode(2));
        assert!(dag.is_dirty());
    }
    {
        let mut dag = Dag::<TestNode>::new();
        dag.deref_mut();
        assert!(dag.is_dirty());
    }
    {
        let mut dag = Dag::<TestNode>::new();
        let _ = dag.graph_mut();
        assert!(dag.is_dirty());
    }
}

#[test]
fn toposort() {
    let mut dag = Dag::<TestNode>::new();
    dag.add_edge(TestNode(1), TestNode(2));
    dag.add_edge(TestNode(2), TestNode(3));
    dag.add_edge(TestNode(1), TestNode(3));

    assert_eq!(
        dag.toposort().unwrap(),
        &[TestNode(1), TestNode(2), TestNode(3)]
    );
    assert_eq!(
        dag.get_toposort().unwrap(),
        &[TestNode(1), TestNode(2), TestNode(3)]
    );
}

#[test]
fn analyze() {
    let mut dag1 = Dag::<TestNode>::new();
    dag1.add_edge(TestNode(1), TestNode(2));
    dag1.add_edge(TestNode(2), TestNode(3));
    dag1.add_edge(TestNode(1), TestNode(3)); // redundant edge

    let analysis1 = dag1.analyze().unwrap();

    assert!(analysis1.reachable().contains(index(0, 1, 3)));
    assert!(analysis1.reachable().contains(index(1, 2, 3)));
    assert!(analysis1.reachable().contains(index(0, 2, 3)));

    assert!(analysis1.connected().contains(&(TestNode(1), TestNode(2))));
    assert!(analysis1.connected().contains(&(TestNode(2), TestNode(3))));
    assert!(analysis1.connected().contains(&(TestNode(1), TestNode(3))));

    assert!(
        !analysis1
            .disconnected()
            .contains(&(TestNode(2), TestNode(1)))
    );
    assert!(
        !analysis1
            .disconnected()
            .contains(&(TestNode(3), TestNode(2)))
    );
    assert!(
        !analysis1
            .disconnected()
            .contains(&(TestNode(3), TestNode(1)))
    );

    assert!(
        analysis1
            .transitive_edges()
            .contains(&(TestNode(1), TestNode(3)))
    );

    assert!(analysis1.check_for_redundant_edges().is_err());

    let mut dag2 = Dag::<TestNode>::new();
    dag2.add_edge(TestNode(3), TestNode(4));

    let analysis2 = dag2.analyze().unwrap();

    assert!(analysis2.check_for_redundant_edges().is_ok());
    assert!(analysis1.check_for_cross_dependencies(&analysis2).is_ok());

    let mut dag3 = Dag::<TestNode>::new();
    dag3.add_edge(TestNode(1), TestNode(2));

    let analysis3 = dag3.analyze().unwrap();

    assert!(analysis1.check_for_cross_dependencies(&analysis3).is_err());

    dag1.remove_redundant_edges(&analysis1);
    let analysis1 = dag1.analyze().unwrap();
    assert!(analysis1.check_for_redundant_edges().is_ok());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Node {
    Key(Key),
    Value(Value),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Key(u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Value(u32);

impl GraphNodeId for Node {
    type Adjacent = (Node, Direction);
    type Edge = (Node, Node);

    fn kind(&self) -> &'static str {
        "node"
    }
}

impl TryInto<Key> for Node {
    type Error = Value;

    fn try_into(self) -> Result<Key, Value> {
        match self {
            Node::Key(k) => Ok(k),
            Node::Value(v) => Err(v),
        }
    }
}

impl TryInto<Value> for Node {
    type Error = Key;

    fn try_into(self) -> Result<Value, Key> {
        match self {
            Node::Value(v) => Ok(v),
            Node::Key(k) => Err(k),
        }
    }
}

impl GraphNodeId for Key {
    type Adjacent = (Key, Direction);
    type Edge = (Key, Key);

    fn kind(&self) -> &'static str {
        "key"
    }
}

impl GraphNodeId for Value {
    type Adjacent = (Value, Direction);
    type Edge = (Value, Value);

    fn kind(&self) -> &'static str {
        "value"
    }
}

impl From<Key> for Node {
    fn from(key: Key) -> Self {
        Node::Key(key)
    }
}

impl From<Value> for Node {
    fn from(value: Value) -> Self {
        Node::Value(value)
    }
}

#[test]
fn group_by_key() {
    let mut dag = Dag::<Node>::new();
    dag.add_edge(Node::Key(Key(1)), Node::Value(Value(10)));
    dag.add_edge(Node::Key(Key(1)), Node::Value(Value(11)));
    dag.add_edge(Node::Key(Key(2)), Node::Value(Value(20)));
    dag.add_edge(Node::Key(Key(2)), Node::Key(Key(1)));
    dag.add_edge(Node::Value(Value(10)), Node::Value(Value(11)));

    let groups = dag.group_by_key::<Key, Value>(2).unwrap();
    assert_eq!(groups.len(), 2);

    let group_key1 = groups.get(&Key(1)).unwrap();
    assert!(group_key1.contains(&Value(10)));
    assert!(group_key1.contains(&Value(11)));

    let group_key2 = groups.get(&Key(2)).unwrap();
    assert!(group_key2.contains(&Value(10)));
    assert!(group_key2.contains(&Value(11)));
    assert!(group_key2.contains(&Value(20)));
}

#[test]
fn flatten() {
    let mut dag = Dag::<Node>::new();
    dag.add_edge(Node::Key(Key(1)), Node::Value(Value(10)));
    dag.add_edge(Node::Key(Key(1)), Node::Value(Value(11)));
    dag.add_edge(Node::Key(Key(2)), Node::Value(Value(20)));
    dag.add_edge(Node::Key(Key(2)), Node::Value(Value(21)));
    dag.add_edge(Node::Value(Value(30)), Node::Key(Key(1)));
    dag.add_edge(Node::Key(Key(1)), Node::Value(Value(40)));

    let groups = dag.group_by_key::<Key, Value>(2).unwrap();
    let flattened = groups.flatten(dag, |_key, _values, _dag, _temp| {});

    assert!(flattened.contains_node(Value(10)));
    assert!(flattened.contains_node(Value(11)));
    assert!(flattened.contains_node(Value(20)));
    assert!(flattened.contains_node(Value(21)));
    assert!(flattened.contains_node(Value(30)));
    assert!(flattened.contains_node(Value(40)));

    assert!(flattened.contains_edge(Value(30), Value(10)));
    assert!(flattened.contains_edge(Value(30), Value(11)));
    assert!(flattened.contains_edge(Value(10), Value(40)));
    assert!(flattened.contains_edge(Value(11), Value(40)));
}

#[test]
fn flatten_undirected() {
    let mut dag = Dag::<Node>::new();
    dag.add_edge(Node::Key(Key(1)), Node::Value(Value(10)));
    dag.add_edge(Node::Key(Key(1)), Node::Value(Value(11)));
    dag.add_edge(Node::Key(Key(2)), Node::Value(Value(20)));
    dag.add_edge(Node::Key(Key(2)), Node::Value(Value(21)));

    let groups = dag.group_by_key::<Key, Value>(2).unwrap();

    let mut ungraph = UnGraph::<Node>::default();
    ungraph.add_edge(Node::Value(Value(10)), Node::Value(Value(11)));
    ungraph.add_edge(Node::Key(Key(1)), Node::Value(Value(30)));
    ungraph.add_edge(Node::Value(Value(40)), Node::Key(Key(2)));
    ungraph.add_edge(Node::Key(Key(1)), Node::Key(Key(2)));

    let flattened = groups.flatten_undirected(&ungraph);

    assert!(flattened.contains_edge(Value(10), Value(11)));
    assert!(flattened.contains_edge(Value(10), Value(30)));
    assert!(flattened.contains_edge(Value(11), Value(30)));
    assert!(flattened.contains_edge(Value(40), Value(20)));
    assert!(flattened.contains_edge(Value(40), Value(21)));
    assert!(flattened.contains_edge(Value(10), Value(20)));
    assert!(flattened.contains_edge(Value(10), Value(21)));
    assert!(flattened.contains_edge(Value(11), Value(20)));
    assert!(flattened.contains_edge(Value(11), Value(21)));
}

#[test]
fn overlapping_groups() {
    let mut dag = Dag::<Node>::new();
    dag.add_edge(Node::Key(Key(1)), Node::Value(Value(10)));
    dag.add_edge(Node::Key(Key(1)), Node::Value(Value(11)));
    dag.add_edge(Node::Key(Key(2)), Node::Value(Value(11))); // overlap
    dag.add_edge(Node::Key(Key(2)), Node::Value(Value(20)));
    dag.add_edge(Node::Key(Key(1)), Node::Key(Key(2)));

    let groups = dag.group_by_key::<Key, Value>(2).unwrap();
    let analysis = dag.analyze().unwrap();

    let result = analysis.check_for_overlapping_groups(&groups);
    assert!(result.is_err());
}

#[test]
fn disjoint_groups() {
    let mut dag = Dag::<Node>::new();
    dag.add_edge(Node::Key(Key(1)), Node::Value(Value(10)));
    dag.add_edge(Node::Key(Key(1)), Node::Value(Value(11)));
    dag.add_edge(Node::Key(Key(2)), Node::Value(Value(20)));
    dag.add_edge(Node::Key(Key(2)), Node::Value(Value(21)));

    let groups = dag.group_by_key::<Key, Value>(2).unwrap();
    let analysis = dag.analyze().unwrap();

    let result = analysis.check_for_overlapping_groups(&groups);
    assert!(result.is_ok());
}
