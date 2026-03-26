use eframe::egui::{Id, Rect, Style};

use crate::kairos_editor::ui::docking_tab::{dock_state::{DockState, tree::{NodeIndex, TabIndex}}, surfaces::SurfaceIndex};

pub mod window_state;
pub mod dock_state;
pub mod surfaces;
pub mod translations;

pub enum AllowedSplits {
    /// Allow splits in any direction (horizontal and vertical).
    All = 0b11,

    /// Only allow split in a horizontal directions.
    LeafRightOnly = 0b10,

    /// Only allow splits in a vertical directions.
    TopBottomOnly = 0b01,

    /// Don't allow splits at all.
    None = 0b00,
}
impl Default for AllowedSplits {
    #[inline(always)]
    fn default() -> Self {
        AllowedSplits::All
    }
}
impl AllowedSplits {
    fn from_u8(u8: u8) -> Self {
        match u8 {
            0b11 => AllowedSplits::All,
            0b10 => AllowedSplits::LeafRightOnly,
            0b01 => AllowedSplits::TopBottomOnly,
            0b00 => AllowedSplits::None,
            _ => unreachable!("Provided an invalid value for allowed splits: {u8:0x}"),
        }
    }
}
impl std::ops::BitAnd for AllowedSplits {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self::from_u8(self as u8 & rhs as u8)
    }
}

#[derive(Debug, Clone, Copy)]
struct ForcedRemoval(pub bool);

enum TabRemoval {
    Tab(SurfaceIndex, NodeIndex, TabIndex, ForcedRemoval),
    Node(SurfaceIndex, NodeIndex),
    Window(SurfaceIndex),
}

/// Displays a [`DockState`](dock_state::DockState) in `egui`
pub struct DockArea<'tree, Drawer> {
    id: Id,
    dock_state: &'tree mut DockState<Drawer>,
    style: Option<Style>,
    show_add_popup: bool,
    show_add_buttons: bool,
    show_close_buttons: bool,
    tab_context_menus: bool,
    draggable_tabs: bool,
    show_tab_name_on_hover: bool,
    show_window_close_buttons: bool,
    show_window_collapse_buttons: bool,
    show_leaf_close_all_buttons: bool,
    show_leaf_collapse_buttons: bool,
    show_secondary_button_hint: bool,
    secondary_button_modifiers: bool,
    secondary_button_context_menu: bool,
    allowed_splits: AllowedSplits,
    window_bounds: Option<Rect>,

    to_remove: Vec<TabRemoval>,
    to_detach: Vec<(SurfaceIndex, NodeIndex, TabIndex)>,
    new_focused: Option<(SurfaceIndex, NodeIndex)>,
    tab_hover_rect: Option<(Rect, TabIndex)>,
}

// todo