use crate::kairos_editor::ui::docking_tab::dock_state::tree::node::{leaf_node::LeafNode, split_node::SplitNode};



pub mod leaf_node;
pub mod split_node;





/// Represents an abstract node of a [`Tree`](crate::Tree).
#[derive(Clone, Debug)]
pub enum Node<Drawer> {
    /// Empty Node
    Empty,

    /// Contains the actual tabs.
    Leaf(LeafNode<Drawer>),

    /// Parent node in the vertical orientation.
    Vertical(SplitNode),

    /// Parent node in the horizontal orientation.
    Horizontal(SplitNode),
}