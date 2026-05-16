use eframe::egui::{CentralPanel, Color32, Context, Frame, Id, Modifiers, Rect, Ui};

use crate::kairos_editor::ui::docking_tab::{dock_state::{DockState, tree::{NodeIndex, TabDestination, TabIndex, node::{Node}}}, drag_and_drop::TreeComponent, state::State, styles::{OverlayType, Style}, surfaces::SurfaceIndex, tab_drawer::TabDrawer};

pub mod window_state;
pub mod dock_state;
pub mod surfaces;
pub mod translations;
pub mod tab_drawer;
pub mod styles;
pub mod state;
pub mod drag_and_drop;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum AllowedSplits {
    /// Allow splits in any direction (horizontal and vertical).
    #[default]
    All = 0b11,

    /// Only allow split in a horizontal directions.
    LeftRightOnly = 0b10,

    /// Only allow splits in a vertical directions.
    TopBottomOnly = 0b01,

    /// Don't allow splits at all.
    None = 0b00,
}

impl AllowedSplits {
    fn from_u8(u8: u8) -> Self {
        match u8 {
            0b11 => AllowedSplits::All,
            0b10 => AllowedSplits::LeftRightOnly,
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
    secondary_button_modifiers: Modifiers,
    secondary_button_on_modifier: bool,
    secondary_button_context_menu: bool,
    allowed_splits: AllowedSplits,
    window_bounds: Option<Rect>,

    to_remove: Vec<TabRemoval>,
    to_detach: Vec<(SurfaceIndex, NodeIndex, TabIndex)>,
    new_focused: Option<(SurfaceIndex, NodeIndex)>,
    tab_hover_rect: Option<(Rect, TabIndex)>,
}

impl<'tree, Drawer> DockArea<'tree, Drawer> {
    pub fn new(id: impl Into<Id>, tree: &'tree mut DockState<Drawer>) -> DockArea<'tree, Drawer> {
        Self { 
            id: id.into(),
            dock_state: tree,
            style: None,
            show_add_popup: false,
            show_add_buttons: false,
            show_close_buttons: true,
            tab_context_menus: true,
            draggable_tabs: true,
            show_tab_name_on_hover: false,
            allowed_splits: AllowedSplits::default(),
            to_remove: Vec::new(),
            to_detach: Vec::new(),
            new_focused: None,
            tab_hover_rect: None,
            window_bounds: None,
            show_window_close_buttons: true,
            show_window_collapse_buttons: true,
            show_leaf_close_all_buttons: true,
            show_leaf_collapse_buttons: true,
            show_secondary_button_hint: true,
            secondary_button_modifiers: Modifiers::SHIFT,
            secondary_button_on_modifier: true,
            secondary_button_context_menu: true
        }
    }
}

impl<Drawer> std::fmt::Debug for DockArea<'_, Drawer> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockArea").finish_non_exhaustive()
    }
}

impl<Drawer> DockArea<'_, Drawer> {
        /// Show the `DockArea` at the top level.
    ///
    /// This is the same as doing:
    ///
    /// ```
    /// # use egui_dock::{DockArea, DockState};
    /// # use egui::{CentralPanel, Frame};
    /// # struct TabViewer {}
    /// # impl egui_dock::TabViewer for TabViewer {
    /// #     type Tab = String;
    /// #     fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText { (&*tab).into() }
    /// #     fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {}
    /// # }
    /// # let mut tree: DockState<String> = DockState::new(vec![]);
    /// # let mut tab_viewer = TabViewer {};
    /// # egui::__run_test_ctx(|ctx| {
    /// CentralPanel::default()
    ///     .frame(Frame::central_panel(&ctx.style()).inner_margin(0.))
    ///     .show(ctx, |ui| {
    ///         DockArea::new(&mut tree).show_inside(ui, &mut tab_viewer);
    ///     });
    /// # });
    /// ```
    ///
    /// So you can't use the [`CentralPanel::show`] when using `DockArea`'s one.
    ///
    /// See also [`show_inside`](Self::show_inside).
    #[inline]
    pub fn show(self, ctx: &Context, tab_viewer: &mut impl TabDrawer<Tab = Drawer>) {
        CentralPanel::default()
            .frame(
                Frame::central_panel(&ctx.style())
                    .inner_margin(0.)
                    .fill(Color32::TRANSPARENT),
            )
            .show(ctx, |ui| {
                self.show_inside(ui, tab_viewer);
            });
    }
    
    /// Shows the docking hierarchy inside a [`Ui`].
    pub fn show_inside(mut self, ui: &mut Ui, tab_drawer: &mut impl TabDrawer<Tab = Drawer>) {
        self.style
            .get_or_insert(Style::from_egui(ui.style().as_ref()));

        let mut state = State::load(ui.ctx(), self.id);

        // Delay hover position one frame. On touch screens hover_pos() is None when any_released()
        if !ui.input(|i| i.pointer.any_released()) {
            state.last_hover_pos = ui.input(|i| i.pointer.hover_pos());
        }

        let (drag_data, hover_data) = ui.memory_mut(|mem| {
            (
                mem.data.remove_temp(self.id.with("drag_data")).flatten(),
                mem.data.remove_temp(self.id.with("hover_data")).flatten(),
            )
        });

        if let (Some(source), Some(hover)) = (drag_data, hover_data) {
            let style = self.style.as_ref().unwrap();
            state.set_drag_and_drop(source, hover, ui.ctx(), style);
            let tab_dst = self.show_drag_drop_overlay(ui, &mut state, tab_drawer);
        }
    }

    /// Returns some when windows are fading, and what surface index is being hovered over
    #[inline(always)]
    fn hovered_window_surface(
        &self,
        state: &mut State,
        hold_time: f32,
        ctx: &Context,
    ) -> Option<SurfaceIndex> {
        if let Some(dnd_state) = &state.dnd {
            if dnd_state.is_locked(self.style.as_ref().unwrap(), ctx) {
                state.window_fade =
                    Some((ctx.input(|i| i.time), dnd_state.hover.dst.surface_address()));
            }
        }

        state.window_fade.and_then(|(time, surface)| {
            ctx.request_repaint();
            (hold_time > (ctx.input(|i| i.time) - time) as f32).then_some(surface)
        })
    }

    /// Resolve where a dragged tab would land given it's dropped this frame, returns `None` when the resulting drop is an invalid move.
    fn show_drag_drop_overlay(
        &mut self,
        ui: &Ui,
        state: &mut State,
        tab_viewer: &impl TabDrawer<Tab = Drawer>,
    ) -> Option<TabDestination> {
        let drag_state = state.dnd.as_mut().unwrap();
        let style = self.style.as_ref().unwrap();

        let deserted_node = {
            match (
                drag_state.drag.src.node_address(),
                drag_state.hover.dst.node_address(),
            ) {
                ((src_surf, Some(src_node)), (dst_surf, Some(dst_node))) => {
                    src_surf == dst_surf
                        && src_node == dst_node
                        && self.dock_state[src_surf][src_node].drawers_count() == 1
                }
                _ => false,
            }
        };

        // Not all scenarios can house all splits.
        let restricted_splits = if drag_state.hover.dst.is_surface() || deserted_node {
            AllowedSplits::None
        } else {
            AllowedSplits::All
        };
        let allowed_splits = self.allowed_splits & restricted_splits;

        let allowed_in_window = match drag_state.drag.src {
            TreeComponent::Tab(surface, node, tab) => {
                let Node::Leaf(leaf) = &mut self.dock_state[surface][node] else {
                    unreachable!("tab drags can only come from leaf nodes")
                };
                tab_viewer.allowed_in_windows(&mut leaf.drawers[tab.0])
            }
            _ => todo!("collections of tabs, like nodes or surfaces, can't be dragged! (yet)"),
        };

        if let Some(pointer) = state.last_hover_pos {
            drag_state.pointer = pointer;
        }

        let window_bounds = self.window_bounds.unwrap();
        match (style.overlay.overlay_type, drag_state.is_on_title_bar()) {
            (OverlayType::HighlightedAreas, _) | (_, true) => drag_state.resolve_traditional(
                ui,
                style,
                allowed_splits,
                allowed_in_window,
                window_bounds,
            ),
            (OverlayType::Widgets, false) => drag_state.resolve_icon_based(
                ui,
                style,
                allowed_splits,
                allowed_in_window,
                window_bounds,
            ),
        }
    }
}