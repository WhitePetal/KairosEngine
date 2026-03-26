use crate::kairos_editor::ui::docking_tab::{dock_state::tree::Tree, window_state::WindowState};

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

pub enum Surface<Drawer> {
    Empty,
    Main(Tree<Drawer>),
    Window(Tree<Drawer>, WindowState)
}