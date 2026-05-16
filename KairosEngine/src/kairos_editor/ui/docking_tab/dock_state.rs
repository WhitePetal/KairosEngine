use std::ops;

use eframe::egui::Rect;

use crate::kairos_editor::ui::docking_tab::{DockArea, dock_state::tree::{NodeIndex, NodePath, TabIndex, Tree, node::{self, Node}}, surfaces::{Surface, SurfaceIndex}, translations::Translations, window_state::WindowState};



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
    type Output = Node<Drawer>;

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
    /// Create a new tree with given tabs at the main surface's root node.
    pub fn new(tabs: Vec<Drawer>) -> Self {
        Self {
            surfaces: vec![Surface::Main(Tree::new(tabs))],
            focused_surface: None,
            translations: Translations::english()
        }
    }

    /// Sets translations of text later displayed in [`DockArea`](create::kairos_engine::kairos_editor::ui::docking_tab)
    pub fn with_translations(mut self, translations: Translations) -> Self {
        self.translations = translations;
        self
    }

    /// Get an immutable borrow to the tree at the main surface.
    pub fn main_surface(&self) -> &Tree<Drawer> {
        &self[SurfaceIndex::main()]
    }

    /// Get a mutable borrow to the tree at the main surface
    pub fn main_surface_mut(&mut self) -> &mut Tree<Drawer> {
        &mut self[SurfaceIndex::main()]
    }

    /// Get the [`WindowState`] which corresponds to a [`SurfaceIndex`].
    ///
    /// Returns `None` if the surface is [`Empty`](Surface::Empty), [`Main`](Surface::Main), or doesn't exist.
    ///
    /// This can be used to modify properties of a window, e.g. size and position.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use egui_dock::DockState;
    /// # use egui::{Vec2, Pos2};
    /// let mut dock_state = DockState::new(vec![]);
    /// let mut surface_index = dock_state.add_window(vec!["Window Tab".to_string()]);
    /// let window_state = dock_state.get_window_state_mut(surface_index).unwrap();
    ///
    /// window_state.set_position(Pos2::ZERO);
    /// window_state.set_size(Vec2::splat(100.0));
    /// ```
    pub fn get_window_state_mut(&mut self, surface: SurfaceIndex) -> Option<&mut WindowState> {
        match &mut self.surfaces[surface.0] {
            Surface::Window(_, state) => Some(state),
            _ => None,
        }
    }

    /// Get the [`WindowState`] which corresponds to a [`SurfaceIndex`].
    ///
    /// Returns `None` if the surface is an [`Empty`](Surface::Empty), [`Main`](Surface::Main), or doesn't exist.
    pub fn get_window_state(&self, surface: SurfaceIndex) -> Option<&WindowState> {
        match &self.surfaces[surface.0] {
            Surface::Window(_, state) => Some(state),
            _ => None
        }
    }

    /// Returns the viewport [`Rect`] and the `Tab` inside the focused leaf node or `None` if no node is in focus.
    #[inline]
    pub fn find_active_focused(&mut self) -> Option<(Rect, &mut Drawer)> {
        self.focused_surface.and_then(|surface| self[surface].find_active_focused())
    }

    /// Get a mutable borrow to the raw surface from a surface index.
    #[inline]
    pub fn get_surface_mut(&mut self, surface: SurfaceIndex) -> Option<&mut Surface<Drawer>> {
        self.surfaces.get_mut(surface.0)
    }

    /// Get an immutable borrow to the raw surface from a surface index.
    #[inline]
    pub fn get_surface(&self, surface: SurfaceIndex) -> Option<&Surface<Drawer>> {
        self.surfaces.get(surface.0)
    }

    /// Returns true if the specified surface exists and isn't [`Empty`](Surface::Empty).
    #[inline]
    pub fn is_surface_valid(&self, surface: SurfaceIndex) -> bool {
        self.surfaces.get(surface.0).is_some_and(|surface| surface.is_empty())
    }

    /// Returns a list of all valid [`SurfaceIndex`]es.
    pub fn valid_surface_indices(&self) -> Box<[SurfaceIndex]> {
        (0..self.surfaces.len())
            .filter_map(|index| {
                let index = SurfaceIndex(index);
                self.is_surface_valid(index).then_some(index)
            })
            .collect()
    }

    /// Remove a surface based on its [`SurfaceIndex`]
    ///
    /// Returns the removed surface or `None` if it didn't exist.
    ///
    /// # Panics
    ///
    /// Panics if you try to remove the main surface: `SurfaceIndex::main()`.
    pub fn remove_surface(&mut self, surface: SurfaceIndex) -> Option<Surface<Drawer>> {
        assert!(!surface.is_main());
        (surface.0 < self.surfaces.len()).then(|| {
            self.focused_surface = Some(SurfaceIndex::main());
            if surface.0 == self.surfaces.len() - 1 {
                self.surfaces.pop().unwrap()
            } else {
                let dest = &mut self.surfaces[surface.0];
                std::mem::replace(dest, Surface::Empty)
            }
        })
    }

    /// Sets which is the active tab within a specific node on a given surface.
    #[inline]
    pub fn set_active_drawer(
        &mut self,
        (surface_index, node_index, tab_index) : (SurfaceIndex, NodeIndex, TabIndex)
    ) {
        if let Some(Node::Leaf(leaf)) = self[surface_index].nodes.get_mut(node_index.0) {
            leaf.active = tab_index;
        }
    }

    pub fn set_focused_node_and_surface(
        &mut self,
        (surface_index, node_index) : (SurfaceIndex, NodeIndex)
    ) {
        if self.is_surface_valid(surface_index) && node_index.0 < self[surface_index].len() {
            // I don't want this code to be evaluated until im absolutely sure the surface index is valid.
            if self[surface_index][node_index].is_leaf() {
                self.focused_surface = Some(surface_index);
                self[surface_index].set_focused_node(node_index);
                return;;
            }
        }
        self.focused_surface = None;
    }
}