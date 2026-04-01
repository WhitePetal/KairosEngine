use crate::kairos_editor::ui::docking_tab::{dock_state::tree::Tree, window_state::WindowState};

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
}