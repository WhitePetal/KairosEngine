use std::ops::{Index, IndexMut};

use crate::kairos_editor::ui::docking_tab::{dock_state::tree::{NodeIndex, Tree, node::Node}, window_state::WindowState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceIndex(pub usize);

impl From<usize> for SurfaceIndex {
    fn from(value: usize) -> Self {
        SurfaceIndex(value)
    }
}

impl SurfaceIndex {
    /// Returns the index of the main surface.
    #[inline(always)]
    pub const fn main() -> Self {
        Self(0)
    }

    /// Returns if this index is `SurfaceIndex::main()`/
    #[inline(always)]
    pub const fn is_main(self) -> bool {
        self.0 == Self::main().0
    }
}

/// A [`Surface`] is the highest level component in a [`DockState`](crate::DockState). [`Surface`]s represent an area
/// in which nodes are placed.
///
/// Typically, you're only using one surface, which is the main surface. However, if you drag
/// a tab out in a way which creates a window, you also create a new surface in which nodes can appear.
#[derive(Debug, Clone)]
pub enum Surface<Drawer> {
    Empty,
    Main(Tree<Drawer>),
    Window(Tree<Drawer>, WindowState)
}

impl<Drawer> Index<NodeIndex> for Surface<Drawer> {
    type Output = Node<Drawer>;

    fn index(&self, index: NodeIndex) -> &Self::Output {
        match self {
            Surface::Empty => panic!("indexed on empty surface"),
            Surface::Main(tree) | Surface::Window(tree, _) => &tree[index],
        }
    }
}
impl<Drawer> IndexMut<NodeIndex> for Surface<Drawer> {
    fn index_mut(&mut self, index: NodeIndex) -> &mut Self::Output {
        match self {
            Surface::Empty => panic!("indexed on empty surface"),
            Surface::Main(tree) | Surface::Window(tree, _) => &mut tree[index],
        }
    }
}

impl<Drawer> Surface<Drawer> {
    /// Is this surface [`Empty`](Self::Empty) (in practice null)?
    pub const fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Get acess to the node tree of this surface.
    pub fn node_tree(&self) -> Option<&Tree<Drawer>> {
        match self {
            Surface::Empty => None,
            Surface::Main(tree) => Some(tree),
            Surface::Window(tree, _) => Some(tree),
        }
    }

    /// Get mutable access to the node tree of this surface.
    pub fn node_tree_mut(&mut self) -> Option<&mut Tree<Drawer>> {
        match self {
            Surface::Empty => None,
            Surface::Main(tree) => Some(tree),
            Surface::Window(tree, _) => Some(tree),
        }
    }

    /// Returns an [`Iterator`] of nodes in this surface's tree.
    ///
    /// If the surface is [`Empty`](Self::Empty), then the returned [`Iterator`] will be empty.
    pub fn iter_nodes(&self) -> impl Iterator<Item = &Node<Drawer>> {
        match self.node_tree() {
            Some(tree) => tree.iter(),
            None => core::slice::Iter::default(),
        }
    }

    /// Returns a mutable [`Iterator`] of nodes in this surface's tree.
    ///
    /// If the surface is [`Empty`](Self::Empty), then the returned [`Iterator`] will be empty.
    pub fn iter_nodes_mut(&mut self) -> impl Iterator<Item = &mut Node<Drawer>> {
        match self.node_tree_mut() {
            Some(tree) => tree.iter_mut(),
            None => core::slice::IterMut::default(),
        }
    }

    /// Returns an [`Iterator`] of **all** tabs in this surface's tree,
    /// and indices of containing nodes.
    pub fn iter_all_drawers(&self) -> impl Iterator<Item = (NodeIndex, &Drawer)> {
        self.iter_nodes()
            .enumerate()
            .flat_map(|(index, node)| node.iter_tabs().map(move |tab| (NodeIndex(index), tab)))
    }

    /// Returns a mutable [`Iterator`] of **all** tabs in this surface's tree,
    /// and indices of containing nodes.
    pub fn iter_all_drawers_mut(&mut self) -> impl Iterator<Item = (NodeIndex, &mut Drawer)> {
        self.iter_nodes_mut()
            .enumerate()
            .flat_map(|(index, node)| node.iter_tabs_mut().map(move |tab| (NodeIndex(index), tab)))
    }

    /// Returns a new [`Surface`] while mapping and filtering the tab type.
    /// Any remaining empty [`Node`]s and are removed, and if this [`Surface`] remains empty,
    /// it'll change to [`Surface::Empty`].
    pub fn filter_map_drawers<F, NewTab>(&self, function: F) -> Surface<NewTab>
    where
        F: FnMut(&Drawer) -> Option<NewTab>,
    {
        match self {
            Surface::Empty => Surface::Empty,
            Surface::Main(tree) => Surface::Main(tree.filter_map_drawers(function)),
            Surface::Window(tree, window_state) => {
                let tree = tree.filter_map_drawers(function);
                if tree.is_empty() {
                    Surface::Empty
                } else {
                    Surface::Window(tree, window_state.clone())
                }
            }
        }
    }

    /// Returns a new [`Surface`] while mapping the tab type.
    pub fn map_drawers<F, NewTab>(&self, mut function: F) -> Surface<NewTab>
    where
        F: FnMut(&Drawer) -> NewTab,
    {
        self.filter_map_drawers(move |tab| Some(function(tab)))
    }

    /// Returns a new [`Surface`] while filtering the tab type.
    /// Any remaining empty [`Node`]s and are removed, and if this [`Surface`] remains empty,
    /// it'll change to [`Surface::Empty`].
    pub fn filter_tabs<F>(&self, mut predicate: F) -> Surface<Drawer>
    where
        F: FnMut(&Drawer) -> bool,
        Drawer: Clone,
    {
        self.filter_map_drawers(move |tab| predicate(tab).then(|| tab.clone()))
    }

    /// Removes all tabs for which `predicate` returns `false`.
    /// Any remaining empty [`Node`]s and are also removed, and if this [`Surface`] remains empty,
    /// it'll change to [`Surface::Empty`].
    pub fn retain_drawers<F>(&mut self, predicate: F)
    where
        F: FnMut(&mut Drawer) -> bool,
    {
        if let Surface::Main(tree) | Surface::Window(tree, _) = self {
            tree.retain_tabs(predicate);
            if tree.is_empty() {
                *self = Surface::Empty;
            }
        }
    }
}