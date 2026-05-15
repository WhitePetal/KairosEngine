use std::{fmt, ops::{Index, IndexMut}};

use eframe::egui::Rect;

use crate::kairos_editor::ui::docking_tab::{dock_state::tree::node::Node, surfaces::SurfaceIndex};

pub mod node;

/// Direction in which a new node is created relatively to the parent node at which the split occurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Split {
    Left,
    Right,
    Above,
    Below,
}
impl Split {
    /// Returns whether the split is vertical.
    pub const fn is_top_bottom(self) -> bool {
        matches!(self, Split::Above | Split::Below)
    }

    /// Returns whether the split is horizontal.
    pub const fn is_left_right(self) -> bool {
        matches!(self, Split::Left | Split::Right)
    }
}

/// Specify how a tab should be added to a Node.
pub enum TabInsert {
    /// Split the node in the given direction.
    Split(Split),

    /// Insert the tab at the given index.
    Insert(TabIndex),

    /// Append the tab to the node.
    Append,
}

/// The destination for a tab which is being moved.
pub enum TabDestination {
    /// Move to a new window with this rect.
    Window(Rect),

    /// Move to a an existing node with this insertion.
    Node(SurfaceIndex, NodeIndex, TabInsert),

    /// Move to an empty surface.
    EmptySurface(SurfaceIndex)
}

impl From<(SurfaceIndex, NodeIndex, TabInsert)> for TabDestination {
    fn from(value: (SurfaceIndex, NodeIndex, TabInsert)) -> Self {
        TabDestination::Node(value.0, value.1, value.2)
    }
}
impl From<SurfaceIndex> for TabDestination {
    fn from(value: SurfaceIndex) -> Self {
        TabDestination::EmptySurface(value)
    }
}
impl TabDestination {
    /// Returns if this tab destination is a [`Window`](TabDestination::Window).
    pub fn is_window(&self) -> bool {
        matches!(self, Self::Window(_))
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

impl<Drawer> fmt::Debug for Tree<Drawer> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tree").finish_non_exhaustive()
    }
}

impl<Drawer> Default for Tree<Drawer> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            focused_node: None,
            collapsed: false,
            collapsed_leaf_count: 0,
        }
    }
}

impl<Drawer> Index<NodeIndex> for Tree<Drawer> {
    type Output = Node<Drawer>;

    #[inline(always)]
    fn index(&self, index: NodeIndex) -> &Self::Output {
        &self.nodes[index.0]
    }
}

impl<Drawer> IndexMut<NodeIndex> for Tree<Drawer> {
    #[inline(always)]
    fn index_mut(&mut self, index: NodeIndex) -> &mut Self::Output {
        &mut self.nodes[index.0]
    }
}

impl<Drawer> Tree<Drawer> {
    /// Create a new ['Tree'] with a given 'Vec' of 'Tab's in its root node.
    #[inline(always)]
    pub fn new(tabs: Vec<Drawer>) -> Self {
        let root = Node::leaf_with(tabs);
        Self {
            nodes: vec![root],
            focused_node: None,
            collapsed: false,
            collapsed_leaf_count: 0,
        }
    }
}


impl NodeIndex {
    /// Returns the index of the root node.
    pub const fn root() -> Self {
        Self(0)
    }
}

/// A full path to locate a node in an entire dock state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodePath {
    /// Index of the surface owning the node.
    pub surface: SurfaceIndex,
    /// Index of the node in the surface tree.
    pub node: NodeIndex,
}