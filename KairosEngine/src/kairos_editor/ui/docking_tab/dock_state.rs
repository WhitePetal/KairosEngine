use crate::kairos_editor::ui::docking_tab::{surfaces::{Surface, SurfaceIndex}, translations::Translations};



pub mod tree;



pub struct DockState<Drawer> {
    surfaces: Vec<Surface<Drawer>>,
    focused_surface: Option<SurfaceIndex>,

    /// Contains translations of text shown in [`DockArea`](super::DockArea).
    pub translations: Translations,
}