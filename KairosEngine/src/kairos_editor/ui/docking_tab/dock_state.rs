use std::ops;

use crate::kairos_editor::ui::docking_tab::{dock_state::tree::{NodeIndex, NodePath, Tree}, surfaces::{Surface, SurfaceIndex}, translations::Translations};



pub mod tree;

/// The heart of `egui_dock`.
///
/// This structure holds a collection of surfaces, each of which stores a tree in which tabs are arranged.
///
/// Indexing it with a [`SurfaceIndex`] will yield a [`Tree`](tree::Tree) which then contains nodes and tabs.
///
/// [`DockState`] is generic, so you can use any type of data to represent a tab.
pub struct DockState<Drawer> {
    surfaces: Vec<Surface<Drawer>>,
    focused_surface: Option<SurfaceIndex>,

    /// Contains translations of text shown in [`DockArea`](super::DockArea).
    pub translations: Translations,
}

impl<Drawer> std::ops::Index<SurfaceIndex> for DockState<Drawer> {
    type Output = Tree<Drawer>;

    #[inline(always)]
    fn index(&self, index: SurfaceIndex) -> &Self::Output {
        match self.surfaces[index.0].node_tree() {
            Some(tree) => tree,
            None => {
                panic!("Tree did not exist a tree at surface index {}", index.0);
            },
        }
    }
}

impl<Drawer> ops::IndexMut<SurfaceIndex> for DockState<Drawer> {
    #[inline(always)]
    fn index_mut(&mut self, index: SurfaceIndex) -> &mut Self::Output {
        match self.surfaces[index.0].node_tree_mut() {
            Some(tree) => tree,
            None => {
                panic!("There did not exist a tree at surface index {}", index.0);
            }
        }
    }
}

impl<Drawer> ops::Index<NodePath> for DockState<Drawer> {
    type Output = Tree<Drawer>;

    #[inline(always)]
    fn index(&self, index: NodePath) -> &Self::Output {
        match self.surfaces[index.surface.0].node_tree() {
            Some(tree) => &tree[index.node],
            None => {
                panic!(
                    "There did not exist a tree at surface index {}",
                    index.surface.0
                )
            },
        }
    }
}

impl<Drawer> ops::IndexMut<NodePath> for DockState<Drawer> {
    #[inline(always)]
    fn index_mut(&mut self, index: NodePath) -> &mut Self::Output {
        match self.surfaces[index.surface.0].node_tree_mut() {
            Some(tree) => &mut tree[index.node],
            None => {
                panic!(
                    "There did not exist a tree at surface index {}",
                    index.surface.0
                )
            },
        }
    }
}

impl<Drawer> DockState<Drawer> {
    pub fn new(tabs: Vec<Drawer>) -> Self {
        Self {
            surfaces: vec![Surface::Main(Tree::new(tabs))],
            focused_surface: None,
            translations: Translations::english()
        }
    }
}