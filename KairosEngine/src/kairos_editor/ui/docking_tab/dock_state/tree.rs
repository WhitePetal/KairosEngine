use crate::kairos_editor::ui::docking_tab::dock_state::tree::node::Node;

pub mod node;


#[derive(Clone, Debug)]
pub struct TabIndex(pub usize);

impl From<usize> for TabIndex {
    #[inline(always)]
    fn from(index: usize) -> Self {
        TabIndex(index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeIndex(pub usize);

impl From<usize> for NodeIndex {
    #[inline(always)]
    fn from(value: usize) -> Self {
        NodeIndex(value)
    }
}


#[derive(Clone)]
pub struct Tree<Drawer> {
    // Binary tree vector
    pub nodes: Vec<Node<Drawer>>,
    focused_node: Option<NodeIndex>,
    // Whether all subnodes of the tree is collapsed
    collapsed: bool,
    collapsed_leaf_count: i32,
}

