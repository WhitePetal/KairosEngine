use crate::kairos_editor::ui::{Drawer, docking_tab::{dock_state::tree::Tree, surfaces::{Surface, SurfaceIndex}, translations::Translations}};



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