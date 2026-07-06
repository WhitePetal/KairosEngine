use std::ops::RangeInclusive;

use crate::{kairos_editor::Engine, log::Log};
use egui::{
    self, Align, Align2, Button, CentralPanel, Color32, Context, CornerRadius, CursorIcon,
    EventFilter, Frame, Id, Key, LayerId, Layout, Modifiers, NumExt, Order, Popup,
    PopupCloseBehavior, Pos2, Rect, Response, RichText, ScrollArea, Sense, Shape, Stroke,
    StrokeKind, TextStyle, Ui, UiBuilder, Vec2, Visuals, WidgetText, lerp, pos2,
    style::{WidgetVisuals, Widgets},
    vec2,
};
use emath::TSTransform;
use epaint::TextShape;

use crate::kairos_editor::{
    self,
    ui::{
        Messager,
        docking_tab::{
            dock_state::{
                DockState,
                tree::{
                    NodeIndex, TabDestination, TabIndex,
                    node::{Node, leaf_node::LeafNode},
                },
            },
            drag_and_drop::{DragData, DragDropState, HoverData, TreeComponent},
            state::State,
            styles::{
                ButtonsStyle, OverlayType, SeparatorStyle, Style, TabAddAlign, TabBarStyle,
                TabBodyStyle, TabInteractionStyle, TabStyle,
            },
            surfaces::SurfaceIndex,
            tab_drawer::{OnCloseResponse, TabDrawer},
        },
    },
};

use duplicate::duplicate;
use paste::paste;

pub mod dock_state;
pub mod drag_and_drop;
pub mod state;
pub mod styles;
pub mod surfaces;
pub mod tab_drawer;
pub mod translations;
pub mod window_state;

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
    _show_window_close_buttons: bool,
    _show_window_collapse_buttons: bool,
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
            _show_window_close_buttons: true,
            _show_window_collapse_buttons: true,
            show_leaf_close_all_buttons: true,
            show_leaf_collapse_buttons: true,
            show_secondary_button_hint: true,
            secondary_button_modifiers: Modifiers::SHIFT,
            secondary_button_on_modifier: true,
            secondary_button_context_menu: true,
        }
    }

    /// Sets the [`DockArea`] ID. Useful if you have more than one [`DockArea`].
    #[inline(always)]
    pub fn id(mut self, id: Id) -> Self {
        self.id = id;
        self
    }

    /// Sets the look and feel of the [`DockArea`].
    #[inline(always)]
    pub fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
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
    pub fn show(
        self,
        ui: &mut Ui,
        messager: &mut Messager,
        engine: &Engine,
        log: &mut Log,
        drawers: &Vec<Box<dyn kairos_editor::ui::Drawer>>,
        tab_viewer: &mut impl TabDrawer<Tab = Drawer>,
    ) {
        CentralPanel::default()
            .frame(
                Frame::central_panel(&ui.global_style())
                    .inner_margin(0.)
                    .fill(Color32::TRANSPARENT),
            )
            .show_inside(ui, |ui| {
                self.show_inside(ui, messager, engine, log, drawers, tab_viewer);
            });
    }

    /// Shows the docking hierarchy inside a [`Ui`].
    pub fn show_inside(
        mut self,
        ui: &mut Ui,
        messager: &mut Messager,
        engine: &Engine,
        log: &mut Log,
        drawers: &Vec<Box<dyn kairos_editor::ui::Drawer>>,
        tab_drawer: &mut impl TabDrawer<Tab = Drawer>,
    ) {
        self.style
            .get_or_insert(Style::from_egui(ui.style().as_ref()));
        self.window_bounds.get_or_insert(ui.ctx().content_rect());

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
            if ui.input(|i| i.pointer.primary_released()) {
                if let Some(destination) = tab_dst {
                    let source = {
                        match state.dnd.as_ref().unwrap().drag.src {
                            TreeComponent::Tab(src_surf, src_node, src_tab) => {
                                (src_surf, src_node, src_tab)
                            }
                            _ => todo!(
                                "collections of tabs, like nodes and surfaces can't be docked (yet)"
                            ),
                        }
                    };
                    self.dock_state.move_drawer(source, destination);
                }
            }
        }

        if ui.input(|i| i.pointer.primary_released()) {
            state.reset_drag();
        }

        let style = self.style.as_ref().unwrap();
        let fade_surface =
            self.hovered_window_surface(&mut state, style.overlay.feel.fade_hold_time, ui.ctx());
        let fade_style = {
            fade_surface.is_some().then(|| {
                let mut fade_style = style.clone();
                fade_dock_style(&mut fade_style, style.overlay.surface_fade_opacity);
                (fade_style, style.overlay.surface_fade_opacity)
            })
        };

        for &surface_index in self.dock_state.valid_surface_indices().iter() {
            self.show_surface_inside(
                surface_index,
                ui,
                messager,
                engine,
                log,
                drawers,
                tab_drawer,
                &mut state,
                fade_style.as_ref().map(|(style, factor)| {
                    (style, *factor, fade_surface.unwrap_or(SurfaceIndex::main()))
                }),
            );
        }

        for removal in self.to_remove.drain(..).rev() {
            match removal {
                TabRemoval::Tab(surface, node, tab, ForcedRemoval(is_forced)) => {
                    if is_forced {
                        self.dock_state.remove_drawer((surface, node, tab));
                    } else {
                        let leaf = &mut self.dock_state[surface][node].get_leaf_mut().unwrap();
                        match tab_drawer.on_close(&mut leaf.drawers[tab.0], messager, drawers) {
                            OnCloseResponse::Close => {
                                self.dock_state.remove_drawer((surface, node, tab));
                            }
                            OnCloseResponse::Focus => {
                                leaf.active = tab;
                                self.new_focused = Some((surface, node));
                            }
                            OnCloseResponse::Ignore => {
                                // no-op
                            }
                        }
                    }
                }
                TabRemoval::Node(surface, node) => {
                    let mut all_tabs_are_closable = true;
                    for tab in self.dock_state[surface][node].iter_tabs_mut() {
                        if !(tab_drawer.is_closeable(tab)
                            && matches!(
                                tab_drawer.on_close(tab, messager, drawers),
                                OnCloseResponse::Close
                            ))
                        {
                            all_tabs_are_closable = false;
                        }
                    }
                    if all_tabs_are_closable {
                        self.dock_state.remove_leaf((surface, node));
                    }
                }
                TabRemoval::Window(surface) => {
                    let mut all_tabs_are_closable = true;
                    for node in self.dock_state[surface].iter_mut() {
                        for tab in node.iter_tabs_mut() {
                            if !(tab_drawer.is_closeable(tab)
                                && matches!(
                                    tab_drawer.on_close(tab, messager, drawers),
                                    OnCloseResponse::Close
                                ))
                            {
                                all_tabs_are_closable = false;
                            }
                        }
                    }
                    if all_tabs_are_closable {
                        self.dock_state.remove_surface(surface);
                    }
                }
            }
        }

        for (surface_index, node_index, tab_index) in self.to_detach.drain(..).rev() {
            let mouse_pos = state.last_hover_pos;
            self.dock_state.detach_drawer(
                (surface_index, node_index, tab_index),
                Rect::from_min_size(
                    mouse_pos.unwrap_or(Pos2::ZERO),
                    self.dock_state[surface_index][node_index]
                        .rect()
                        .map_or(Vec2::new(100., 150.), |rect| rect.size()),
                ),
            );
        }

        if let Some(focused) = self.new_focused {
            self.dock_state.set_focused_node_and_surface(focused);
        }

        state.store(ui.ctx(), self.id);
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

    /// Show a single surface of a [`DockState`].
    fn show_surface_inside(
        &mut self,
        surf_index: SurfaceIndex,
        ui: &mut Ui,
        messager: &mut Messager,
        engine: &Engine,
        log: &mut Log,
        drawers: &Vec<Box<dyn kairos_editor::ui::Drawer>>,
        tab_viewer: &mut impl TabDrawer<Tab = Drawer>,
        state: &mut State,
        fade_style: Option<(&Style, f32, SurfaceIndex)>,
    ) {
        if surf_index.is_main() {
            self.show_root_surface_inside(ui, messager, engine, log, drawers, tab_viewer, state);
        } else {
            self.show_window_surface(
                ui, messager, engine, log, drawers, surf_index, tab_viewer, state, fade_style,
            );
        }
    }

    fn render_nodes(
        &mut self,
        ui: &mut Ui,
        messager: &mut Messager,
        engine: &Engine,
        log: &mut Log,
        drawers: &Vec<Box<dyn kairos_editor::ui::Drawer>>,
        tab_viewer: &mut impl TabDrawer<Tab = Drawer>,
        state: &mut State,
        surf_index: SurfaceIndex,
        fade_style: Option<(&Style, f32)>,
    ) {
        // First compute all rect sizes in the node graph.
        let max_rect = self.allocate_area_for_root_node(ui, surf_index);
        for node_index in self.dock_state[surf_index].breadth_first_index_iter() {
            if self.dock_state[surf_index][node_index].is_parent() {
                self.compute_rect_sizes(ui, (surf_index, node_index), max_rect);
            }
        }

        // Then, draw the bodies of each leaves.
        for node_index in self.dock_state[surf_index].breadth_first_index_iter() {
            if self.dock_state[surf_index][node_index].is_leaf() {
                self.show_leaf(
                    ui,
                    messager,
                    engine,
                    log,
                    drawers,
                    state,
                    (surf_index, node_index),
                    tab_viewer,
                    fade_style,
                );
            }
        }

        // Finally, draw separators so that their "interaction zone" is above
        // bodies (see `SeparatorStyle::extra_interact_width`).
        let fade_style = fade_style.map(|(style, _)| style);
        for node_index in self.dock_state[surf_index].breadth_first_index_iter() {
            if self.dock_state[surf_index][node_index].is_parent() {
                self.show_separator(ui, (surf_index, node_index), fade_style);
            }
        }
    }

    fn allocate_area_for_root_node(&mut self, ui: &mut Ui, surface: SurfaceIndex) -> Rect {
        let style = self.style.as_ref().unwrap();
        let mut rect = ui.available_rect_before_wrap();

        if let Some(margin) = style.dock_area_padding {
            rect.min += margin.left_top();
            rect.max -= margin.right_bottom();
        }

        ui.painter().rect_stroke(
            rect,
            style.main_surface_border_rounding,
            style.main_surface_border_stroke,
            StrokeKind::Inside,
        );
        if surface == SurfaceIndex::main() {
            rect = rect.expand(-style.main_surface_border_stroke.width / 2.0);
        }
        ui.allocate_rect(rect, Sense::hover());

        if self.dock_state[surface].is_empty() {
            return rect;
        }
        self.dock_state[surface][NodeIndex::root()].set_rect(rect);
        rect
    }

    fn compute_rect_sizes(
        &mut self,
        ui: &Ui,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        max_rect: Rect,
    ) {
        assert!(self.dock_state[surface_index][node_index].is_parent());

        let style = self.style.as_ref().unwrap();
        let pixels_per_point = ui.ctx().pixels_per_point();

        let left_collapsed_count =
            self.dock_state[surface_index][node_index.left()].collapsed_leaf_count();
        let right_collapsed_count =
            self.dock_state[surface_index][node_index.right()].collapsed_leaf_count();
        let left_collapsed = self.dock_state[surface_index][node_index.left()].is_collapsed();
        let right_collapsed = self.dock_state[surface_index][node_index.right()].is_collapsed();

        if left_collapsed || right_collapsed {
            if let Node::Vertical(split) = &mut self.dock_state[surface_index][node_index] {
                let rect = split.rect();
                debug_assert!(!rect.any_nan() && rect.is_finite());
                let rect = expand_to_pixel(rect, pixels_per_point);

                if left_collapsed {
                    // EITHER only left collapsed OR left and right both collapsed
                    let border_y =
                        rect.min.y + (left_collapsed_count as f32) * style.tab_bar.height;
                    let left_separator_border = map_to_pixel(
                        border_y - style.separator.width * 0.5,
                        pixels_per_point,
                        f32::round,
                    );
                    let right_separator_border = map_to_pixel(
                        border_y + style.separator.width * 0.5,
                        pixels_per_point,
                        f32::round,
                    );
                    let left = rect
                        .intersect(Rect::everything_above(left_separator_border))
                        .intersect(max_rect);
                    let right = rect
                        .intersect(Rect::everything_below(right_separator_border))
                        .intersect(max_rect);
                    self.dock_state[surface_index][node_index.left()].set_rect(left);
                    self.dock_state[surface_index][node_index.right()].set_rect(right);
                } else {
                    // Only right collapsed
                    let border_y =
                        rect.max.y - (right_collapsed_count as f32) * style.tab_bar.height;
                    let left_separator_border = map_to_pixel(
                        border_y - style.separator.width * 0.5,
                        pixels_per_point,
                        f32::round,
                    );
                    let right_separator_border = map_to_pixel(
                        border_y + style.separator.width * 0.5,
                        pixels_per_point,
                        f32::round,
                    );
                    let left = rect
                        .intersect(Rect::everything_above(left_separator_border))
                        .intersect(max_rect);
                    let right = rect
                        .intersect(Rect::everything_below(right_separator_border))
                        .intersect(max_rect);
                    self.dock_state[surface_index][node_index.left()].set_rect(left);
                    self.dock_state[surface_index][node_index.right()].set_rect(right);
                }
                return;
            }
        }

        duplicate! {
            [
                orientation   dim_point  dim_size  left_of    right_of;
                [Horizontal]  [x]        [width]   [left_of]  [right_of];
                [Vertical]    [y]        [height]  [above]    [below];
            ]
            if let Node::orientation(split) = &mut self.dock_state[surface_index][node_index] {
                let rect = split.rect;
                debug_assert!(!rect.any_nan() && rect.is_finite());
                let rect = expand_to_pixel(rect, pixels_per_point);

                let midpoint = rect.min.dim_point + rect.dim_size() * split.fraction;
                let left_separator_border = map_to_pixel(
                    midpoint - style.separator.width * 0.5,
                    pixels_per_point,
                    f32::round
                );
                let right_separator_border = map_to_pixel(
                    midpoint + style.separator.width * 0.5,
                    pixels_per_point,
                    f32::round
                );

                paste! {
                    let left = rect.intersect(Rect::[<everything_ left_of>](left_separator_border)).intersect(max_rect);
                    let right = rect.intersect(Rect::[<everything_ right_of>](right_separator_border)).intersect(max_rect);
                }

                self.dock_state[surface_index][node_index.left()].set_rect(left);
                self.dock_state[surface_index][node_index.right()].set_rect(right);
            }
        }
    }

    fn show_separator(
        &mut self,
        ui: &mut Ui,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        fade_style: Option<&Style>,
    ) {
        assert!(self.dock_state[surface_index][node_index].is_parent());

        // If either of the children is collapsed, we don't want the user to interact with the separator
        if (self.dock_state[surface_index][node_index.left()].is_collapsed()
            || self.dock_state[surface_index][node_index.right()].is_collapsed())
            && self.dock_state[surface_index][node_index].is_vertical()
        {
            return;
        }

        let style = fade_style.unwrap_or_else(|| self.style.as_ref().unwrap());
        let pixels_per_point = ui.ctx().pixels_per_point();

        duplicate! {
            [
                orientation   dim_point  dim_size;
                [Horizontal]  [x]        [width];
                [Vertical]    [y]        [height];
            ]
            if let Node::orientation(split) = &mut self.dock_state[surface_index][node_index] {
                let rect = split.rect;
                let mut separator = rect;

                let midpoint = rect.min.dim_point + rect.dim_size() * split.fraction;
                separator.min.dim_point = midpoint - style.separator.width * 0.5;
                separator.max.dim_point = midpoint + style.separator.width * 0.5;

                let mut expand = Vec2::ZERO;
                expand.dim_point += style.separator.extra_interact_width / 2.0;
                let interact_rect = separator.expand2(expand);

                let response = ui.allocate_rect(interact_rect, Sense::click_and_drag())
                    .on_hover_and_drag_cursor(paste!{ CursorIcon::[<Resize orientation>]});

                let should_respond_to_arrow_keys = ui.input(|i| i.modifiers.command || i.modifiers.shift);

                if response.has_focus() {
                    // Prevent the default behaviour of removing focus from the separators when the
                    // arrow keys are pressed
                    ui.memory_mut(|m| m.set_focus_lock_filter(response.id, EventFilter {
                        horizontal_arrows: should_respond_to_arrow_keys,
                        vertical_arrows: should_respond_to_arrow_keys,
                        tab: false,
                        escape: false
                    }));
                }

                let arrow_key_offset = if response.has_focus() && should_respond_to_arrow_keys {
                    if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                        Some(egui::vec2(0., -16.))
                    } else if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                        Some(egui::vec2(0., 16.))
                    } else if ui.input(|i| i.key_pressed(Key::ArrowLeft)) {
                        Some(egui::vec2(-16., 0.))
                    } else if ui.input(|i| i.key_pressed(Key::ArrowRight)) {
                        Some(egui::vec2(16., 0.))
                    } else {
                        None
                    }
                } else {
                    None
                };

                let midpoint = rect.min.dim_point + rect.dim_size() * split.fraction;
                separator.min.dim_point = map_to_pixel(
                    midpoint - style.separator.width * 0.5,
                    pixels_per_point,
                    f32::round,
                );
                separator.max.dim_point = map_to_pixel(
                    midpoint + style.separator.width * 0.5,
                    pixels_per_point,
                    f32::round,
                );

                let color = if response.dragged() {
                    style.separator.color_dragged
                } else if response.hovered() || response.has_focus() {
                    style.separator.color_hovered
                } else {
                    style.separator.color_idle
                };

                ui.painter().rect_filled(separator, CornerRadius::ZERO, color);

                // Update 'fraction' interaction after drawing separator,
                // otherwise it may overlap on other separator / bodies when
                // shrunk fast.
                let range = rect.max.dim_point - rect.min.dim_point;
                if range > 0.0 {
                    let min = (style.separator.extra / range).min(1.0);
                    let max = 1.0 - min;
                    let (min, max) = (min.min(max), max.max(min));
                    let delta = arrow_key_offset.unwrap_or(response.drag_delta()).dim_point;
                    split.fraction = (split.fraction + delta / range).clamp(min, max);
                }

                if response.double_clicked() {
                    split.fraction = 0.5;
                }
            }
        }
    }
}

impl<Drawer> DockArea<'_, Drawer> {
    pub(super) fn show_root_surface_inside(
        &mut self,
        ui: &mut Ui,
        messager: &mut Messager,
        engine: &Engine,
        log: &mut Log,
        drawers: &Vec<Box<dyn kairos_editor::ui::Drawer>>,
        tab_viewer: &mut impl TabDrawer<Tab = Drawer>,
        state: &mut State,
    ) {
        let surf_index = SurfaceIndex::main();

        if self.dock_state.main_surface().is_empty() {
            let rect = ui.available_rect_before_wrap();
            let response = ui.allocate_rect(rect, Sense::hover());
            if response.contains_pointer() {
                ui.memory_mut(|mem| {
                    mem.data.insert_temp(
                        self.id.with("hover_data"),
                        Some(HoverData {
                            rect,
                            dst: TreeComponent::Surface(surf_index),
                            tab: None,
                        }),
                    );
                });
            }
            return;
        }

        self.render_nodes(
            ui, messager, engine, log, drawers, tab_viewer, state, surf_index, None,
        );
    }
}

impl<Drawer> DockArea<'_, Drawer> {
    pub fn show_window_surface(
        &mut self,
        ui: &Ui,
        messager: &mut Messager,
        engine: &Engine,
        log: &mut Log,
        drawers: &Vec<Box<dyn kairos_editor::ui::Drawer>>,
        surf_index: SurfaceIndex,
        tab_viewer: &mut impl TabDrawer<Tab = Drawer>,
        state: &mut State,
        fade_style: Option<(&Style, f32, SurfaceIndex)>,
    ) {
        // Construct egui window
        let id = format!("window {surf_index:?}").into();
        let bounds = self.window_bounds.unwrap();
        let open = true;
        let window = self
            .dock_state
            .get_window_state_mut(surf_index)
            .unwrap()
            .create_window(id, bounds);

        // Calculate fading of the window (if any)
        let (fade_factor, fade_style) = match fade_style {
            Some((style, factor, surface_index)) => {
                if surface_index == surf_index {
                    (1.0, None)
                } else {
                    (factor, Some((style, factor)))
                }
            }
            None => (1.0, None),
        };

        // Get galley of currently selected node as a window title
        let title = {
            let node_id = self.dock_state[surf_index]
                .focused_leaf()
                .unwrap_or_else(|| {
                    for node_index in self.dock_state[surf_index].breadth_first_index_iter() {
                        if self.dock_state[surf_index][node_index].is_leaf() {
                            return node_index;
                        }
                    }
                    unreachable!("a window surface should never be empty")
                });
            let leaf = self.dock_state[surf_index][node_id].get_leaf_mut().unwrap();
            tab_viewer
                .title(&mut leaf.drawers[leaf.active.0], drawers)
                .color(ui.visuals().widgets.noninteractive.fg_stroke.color)
        };

        // Iterate through every node in dock_state[surf_index], and sum up the number of tabs in them
        let mut tab_count = 0;
        for node_index in self.dock_state[surf_index].breadth_first_index_iter() {
            if self.dock_state[surf_index][node_index].is_leaf() {
                tab_count += self.dock_state[surf_index][node_index].drawers_count();
            }
        }

        // Fade window frame (if necessary)
        let mut frame = Frame::window(ui.style());
        if fade_factor != 1.0 {
            frame.fill = frame.fill.linear_multiply(fade_factor);
            frame.stroke.color = frame.stroke.color.linear_multiply(fade_factor);
            frame.shadow.color = frame.shadow.color.linear_multiply(fade_factor);
        }

        let tab_bar_height = self.style.as_ref().unwrap().tab_bar.height;
        let minimized = self
            .dock_state
            .get_window_state(surf_index)
            .unwrap()
            .is_minimized();
        if minimized {
            let height = tab_bar_height;
            window
                .resizable([true, false])
                .max_height(height)
                .min_height(height)
        } else if self.dock_state[surf_index].is_collapsed() {
            let height = self.dock_state[surf_index].collapsed_leaf_count() as f32 * tab_bar_height;
            window
                .resizable([true, false])
                .max_height(height)
                .min_height(height)
        } else {
            window
        }
        .frame(frame)
        .show(ui.ctx(), |ui| {
            // Fade inner ui (if necessary)
            if fade_factor != 1.0 {
                fade_visuals(ui.visuals_mut(), fade_factor);
            }
            if minimized {
                self.minimized_body(
                    ui,
                    surf_index,
                    fade_style.map(|(style, _)| style),
                    title,
                    tab_count,
                )
            } else {
                self.render_nodes(
                    ui, messager, engine, log, drawers, tab_viewer, state, surf_index, fade_style,
                );
            }
        });

        if !open {
            self.to_remove.push(TabRemoval::Window(surf_index));
        }
    }

    fn minimized_body(
        &mut self,
        ui: &mut Ui,
        surface_index: SurfaceIndex,
        fade_style: Option<&Style>,
        title: WidgetText,
        tab_count: usize,
    ) {
        ui.horizontal(|ui| {
            let style = fade_style.unwrap_or_else(|| self.style.as_ref().unwrap());
            let (tabbar_outer_rect, _) = ui.allocate_exact_size(
                vec2(Style::TAB_EXPAND_BUTTON_SIZE, style.tab_bar.height),
                Sense::hover(),
            );
            ui.painter().rect_filled(
                tabbar_outer_rect,
                style.tab_bar.corner_radius,
                style.tab_bar.bg_fill,
            );
            self.window_expand(ui, surface_index, tabbar_outer_rect, fade_style);
            ui.label(title);
            if tab_count > 1 {
                ui.label(
                    RichText::new(format!("+{}", tab_count - 1))
                        .color(ui.visuals().weak_text_color()),
                );
            }
            ui.allocate_space(ui.available_size());
        });
    }

    /// Draws the expand window button.
    fn window_expand(
        &mut self,
        ui: &mut Ui,
        surface_index: SurfaceIndex,
        tabbar_outer_rect: Rect,
        fade_style: Option<&Style>,
    ) {
        let rect = tabbar_outer_rect;

        let ui = &mut ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center))
                .id_salt((surface_index, "window_expand")),
        );

        let (rect, mut response) = ui.allocate_exact_size(ui.available_size(), Sense::click());

        response = response.on_hover_cursor(CursorIcon::PointingHand);

        let style = fade_style.unwrap_or_else(|| self.style.as_ref().unwrap());
        let color = if response.hovered() || response.has_focus() {
            ui.painter().rect_filled(
                rect,
                CornerRadius::ZERO,
                style.buttons.minimize_window_bg_fill,
            );
            style.buttons.minimize_window_active_color
        } else {
            style.buttons.minimize_window_color
        };

        let mut arrow_rect = rect;

        rect_set_size_centered(&mut arrow_rect, Vec2::splat(Style::TAB_EXPAND_ARROW_SIZE));

        Self::draw_chevron_right(ui, &mut response, style, color, arrow_rect);

        // Draw button right border.
        ui.painter().vline(
            rect.right(),
            rect.y_range(),
            Stroke::new(
                ui.ctx().pixels_per_point().recip(),
                style.buttons.minimize_window_border_color,
            ),
        );

        if response.clicked() {
            self.window_toggle_minimized(surface_index);
        }
    }

    fn draw_chevron_right(
        ui: &mut Ui,
        response: &mut Response,
        style: &Style,
        color: Color32,
        arrow_rect: Rect,
    ) {
        ui.painter().add(Shape::convex_polygon(
            // Arrow pointing rightwards.
            vec![
                arrow_rect.left_top(),
                arrow_rect.center(),
                arrow_rect.left_bottom(),
            ],
            color,
            Stroke::NONE,
        ));

        // Chevron pointing rightwards.
        ui.painter().add(Shape::convex_polygon(
            vec![
                arrow_rect.center_top(),
                arrow_rect.right_center(),
                arrow_rect.center_bottom(),
            ],
            color,
            Stroke::NONE,
        ));
        let color = if response.hovered() || response.has_focus() {
            style.buttons.minimize_window_bg_fill
        } else {
            style.tab_bar.bg_fill
        };
        ui.painter().add(Shape::convex_polygon(
            vec![
                arrow_rect
                    .center_top()
                    .lerp(arrow_rect.center_bottom(), 0.25),
                arrow_rect.center().lerp(arrow_rect.right_center(), 0.5),
                arrow_rect
                    .center_top()
                    .lerp(arrow_rect.center_bottom(), 0.75),
            ],
            color,
            Stroke::NONE,
        ));
    }

    pub fn window_toggle_minimized(&mut self, surf_index: SurfaceIndex) {
        let minimized = self
            .dock_state
            .get_window_state(surf_index)
            .unwrap()
            .is_minimized();
        let surface = &mut self.dock_state[surf_index];

        if surface.root_node().is_some_and(|node| node.is_collapsed()) {
            // The window is already fully collapsed,
            // so `expanded_height` has already been set.
            // We don't need to set `new` either.
            if let Some(window_state) = self.dock_state.get_window_state_mut(surf_index) {
                window_state.toggle_minimized();
            }
        } else if minimized {
            if let Some(window_state) = self.dock_state.get_window_state_mut(surf_index) {
                window_state.set_new(true);
                window_state.toggle_minimized();
            }
        } else {
            let root_index = NodeIndex::root();
            let surface_height = if surface.root_node().is_some() {
                surface[root_index].rect().unwrap().height()
            } else {
                0.0
            };
            if let Some(window_state) = self.dock_state.get_window_state_mut(surf_index) {
                window_state.set_expanded_height(surface_height);
                window_state.toggle_minimized();
            }
        }
    }
}

impl<Drawer> DockArea<'_, Drawer> {
    pub fn show_leaf(
        &mut self,
        ui: &mut Ui,
        messager: &mut Messager,
        engine: &Engine,
        log: &mut Log,
        drawers: &Vec<Box<dyn kairos_editor::ui::Drawer>>,
        state: &mut State,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        tab_viewer: &mut impl TabDrawer<Tab = Drawer>,
        fade_style: Option<(&Style, f32)>,
    ) {
        assert!(self.dock_state[surface_index][node_index].is_leaf());
        let collapsed = self.dock_state[surface_index][node_index].is_collapsed();

        let rect = self.dock_state[surface_index][node_index]
            .rect()
            .expect("This node must be a leaf");
        let ui = &mut ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::top_down_justified(Align::Min))
                .id_salt((node_index, "node")),
        );
        let spacing = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        ui.set_clip_rect(rect);

        if self.dock_state[surface_index][node_index].drawers_count() == 0 {
            return;
        }
        let tabbar_rect = self.drawer_bar(
            ui,
            state,
            messager,
            drawers,
            (surface_index, node_index),
            tab_viewer,
            fade_style.map(|(style, _)| style),
            collapsed,
        );
        self.drawer_body(
            ui,
            messager,
            engine,
            log,
            drawers,
            state,
            (surface_index, node_index),
            tab_viewer,
            spacing,
            tabbar_rect,
            fade_style,
            collapsed,
        );

        let tabs = self.dock_state[surface_index][node_index]
            .drawers_mut()
            .expect("This node must be a leaf here");
        for (tab_index, tab) in tabs.iter_mut().enumerate() {
            if tab_viewer.force_close(tab) {
                self.to_remove.push(TabRemoval::Tab(
                    surface_index,
                    node_index,
                    TabIndex(tab_index),
                    ForcedRemoval(true),
                ));
            }
        }
    }

    fn drawer_bar(
        &mut self,
        ui: &mut Ui,
        state: &mut State,
        messager: &mut Messager,
        drawers: &Vec<Box<dyn kairos_editor::ui::Drawer>>,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        tab_viewer: &mut impl TabDrawer<Tab = Drawer>,
        fade_style: Option<&Style>,
        collapsed: bool,
    ) -> Rect {
        assert!(self.dock_state[surface_index][node_index].is_leaf());

        let style = fade_style.unwrap_or_else(|| self.style.as_ref().unwrap());
        let (tabbar_outer_rect, tabbar_response) = ui.allocate_exact_size(
            vec2(ui.available_width(), style.tab_bar.height),
            Sense::hover(),
        );
        ui.painter().rect_filled(
            tabbar_outer_rect,
            style.tab_bar.corner_radius,
            style.tab_bar.bg_fill,
        );

        let tabbar_outer_rect = tabbar_outer_rect - style.tab_bar.inner_margin;

        let mut available_width = tabbar_outer_rect.width();
        let scroll_bar_width = available_width;
        if available_width == 0.0 {
            return tabbar_outer_rect;
        }

        // Reserve space for the buttons at the ends of the tab bar.

        if self.show_add_buttons {
            available_width -= Style::TAB_ADD_BUTTON_SIZE;
        }

        if self.show_leaf_close_all_buttons {
            available_width -= Style::TAB_CLOSE_ALL_BUTTON_SIZE;
        }

        if self.show_leaf_collapse_buttons {
            available_width -= Style::TAB_COLLAPSE_BUTTON_SIZE;
        }

        let (actual_width, tab_hovered) = {
            let leaf = self.dock_state[surface_index][node_index]
                .get_leaf_mut()
                .expect("This node must be a leaf");

            let tabbar_inner_rect = Rect::from_min_size(
                (tabbar_outer_rect.min - pos2(-leaf.scroll, 0.0)
                    + vec2(
                        if self.show_leaf_collapse_buttons {
                            Style::TAB_COLLAPSE_BUTTON_SIZE
                        } else {
                            0.0
                        },
                        0.0,
                    ))
                .to_pos2(),
                vec2(tabbar_outer_rect.width(), tabbar_outer_rect.height()),
            );

            let tabs_ui = &mut ui.new_child(
                UiBuilder::new()
                    .max_rect(tabbar_inner_rect)
                    .layout(Layout::left_to_right(Align::Center))
                    .id_salt("tabs"),
            );

            let mut clip_rect = tabbar_outer_rect;
            clip_rect.set_width(available_width);
            if self.show_leaf_collapse_buttons {
                clip_rect = clip_rect.translate(vec2(Style::TAB_COLLAPSE_BUTTON_SIZE, 0.0));
            }
            tabs_ui.set_clip_rect(clip_rect);

            // Desired size for tabs in "expanded" mode.
            let prefered_width = style
                .tab_bar
                .fill_tab_bar
                .then_some(available_width / (leaf.drawers.len() as f32));

            let tab_hovered = self.drawers(
                tabs_ui,
                state,
                messager,
                drawers,
                (surface_index, node_index),
                tab_viewer,
                tabbar_outer_rect,
                prefered_width,
                fade_style,
            );

            // Draw hline from tab end to edge of tab bar.
            let px = ui.ctx().pixels_per_point().recip();
            let style = fade_style.unwrap_or_else(|| self.style.as_ref().unwrap());

            ui.painter().hline(
                tabs_ui.min_rect().right().min(clip_rect.right())..=tabbar_outer_rect.right(),
                tabbar_outer_rect.bottom() - px,
                (px, style.tab_bar.hline_color),
            );

            // Add button at the ends of the tab bar.
            if self.show_add_buttons {
                let offset = match style.buttons.add_tab_align {
                    TabAddAlign::Left => {
                        (clip_rect.width() - tabs_ui.min_rect().width()).at_least(0.0)
                    }
                    TabAddAlign::Right => 0.0,
                } + if self.show_leaf_close_all_buttons {
                    Style::TAB_CLOSE_ALL_BUTTON_SIZE
                } else {
                    0.0
                };
                self.drawer_plus(
                    ui,
                    surface_index,
                    node_index,
                    tab_viewer,
                    tabbar_outer_rect,
                    offset,
                    fade_style,
                );
            }

            if self.show_leaf_close_all_buttons {
                // Current leaf contains non-closable tabs.
                let disabled = self.dock_state[surface_index][node_index]
                    .get_leaf_mut()
                    .map(|leaf| {
                        !leaf
                            .drawers
                            .iter_mut()
                            .all(|tab| tab_viewer.is_closeable(tab))
                    })
                    .expect("This node must be a leaf");

                // Current window contains non-closable tabs.
                let close_window_disabled = disabled
                    || !self.dock_state[surface_index].iter_mut().all(|node| {
                        node.get_leaf_mut().is_none_or(|leaf| {
                            leaf.drawers
                                .iter_mut()
                                .all(|tab| tab_viewer.is_closeable(tab))
                        })
                    });

                self.drawer_close_all(
                    ui,
                    surface_index,
                    node_index,
                    tabbar_outer_rect,
                    fade_style,
                    disabled,
                    close_window_disabled,
                )
            }

            if self.show_leaf_collapse_buttons {
                self.drawer_collapse(
                    ui,
                    surface_index,
                    node_index,
                    tabbar_outer_rect,
                    fade_style,
                    collapsed,
                )
            }

            (tabs_ui.min_rect().width(), tab_hovered)
        };

        self.drawer_bar_scroll(
            ui,
            state,
            (surface_index, node_index),
            actual_width,
            available_width,
            scroll_bar_width,
            &tabbar_response,
            tab_hovered,
            fade_style,
        );

        tabbar_outer_rect
    }

    #[allow(clippy::too_many_arguments)]
    fn drawers(
        &mut self,
        tabs_ui: &mut Ui,
        state: &mut State,
        messager: &mut Messager,
        drawers: &Vec<Box<dyn kairos_editor::ui::Drawer>>,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        tab_viewer: &mut impl TabDrawer<Tab = Drawer>,
        tabbar_outer_rect: Rect,
        preferred_width: Option<f32>,
        fade: Option<&Style>,
    ) -> bool {
        let mut tab_hovered = false;

        assert!(self.dock_state[surface_index][node_index].is_leaf());

        let focused = self.dock_state.focused_leaf();
        let tabs_len = {
            let tabs = self.dock_state[surface_index][node_index]
                .drawers()
                .expect("This node must be a leaf here");
            tabs.len()
        };

        for tab_index in 0..tabs_len {
            let id = self
                .id
                .with((surface_index, "surface"))
                .with((node_index, "node"))
                .with((tab_index, "tab"));
            let tab_index = TabIndex(tab_index);
            let is_being_dragged = tabs_ui.ctx().is_being_dragged(id)
                && tabs_ui.input(|i| i.pointer.is_decidedly_dragging())
                && self.draggable_tabs;

            if is_being_dragged {
                tabs_ui.output_mut(|o| o.cursor_icon = CursorIcon::Grabbing);
            }

            let (is_active, label, tab_style, closeable) = {
                let leaf = self.dock_state[surface_index][node_index]
                    .get_leaf_mut()
                    .expect("This node must be a leaf");
                let style = fade.unwrap_or_else(|| self.style.as_ref().unwrap());
                let tab_style =
                    tab_viewer.tab_style_override(&leaf.drawers[tab_index.0], &style.tab);
                (
                    leaf.active == tab_index || is_being_dragged,
                    tab_viewer.title(&mut leaf.drawers[tab_index.0], drawers),
                    tab_style.unwrap_or(style.tab.clone()),
                    tab_viewer.is_closeable(&leaf.drawers[tab_index.0]),
                )
            };

            let show_close_button = self.show_close_buttons && closeable;

            let (response, title_id) = if is_being_dragged {
                let layer_id = LayerId::new(Order::Tooltip, id);
                let response = tabs_ui
                    .scope_builder(UiBuilder::new().layer_id(layer_id), |ui| {
                        self.drawer_title(
                            ui,
                            &tab_style,
                            id,
                            label,
                            is_active && Some((surface_index, node_index)) == focused,
                            is_active,
                            is_being_dragged,
                            preferred_width,
                            show_close_button,
                            fade,
                        )
                    })
                    .response;
                let title_id = response.id;

                let response =
                    tabs_ui.interact(response.rect, id.with("dragged"), Sense::click_and_drag());

                if let Some(pointer_pos) = tabs_ui.ctx().pointer_interact_pos() {
                    let start = *state.drag_start.get_or_insert(pointer_pos);
                    let delta = pointer_pos - start;
                    if delta.x.abs() > 30.0 || delta.y.abs() > 6.0 {
                        tabs_ui
                            .ctx()
                            .transform_layer_shapes(layer_id, TSTransform::new(delta, 1.0));

                        tabs_ui.memory_mut(|mem| {
                            mem.data.insert_temp(
                                self.id.with("drag_data"),
                                Some(DragData {
                                    src: TreeComponent::Tab(surface_index, node_index, tab_index),
                                    rect: self.dock_state[surface_index][node_index]
                                        .rect()
                                        .unwrap(),
                                }),
                            );
                        });
                    }
                }

                (response, title_id)
            } else {
                if tab_index.0 != 0 {
                    tabs_ui.allocate_space(vec2(tab_style.spacing, 0.0));
                }
                let (mut response, close_response) = self.drawer_title(
                    tabs_ui,
                    &tab_style,
                    id,
                    label,
                    is_active && Some((surface_index, node_index)) == focused,
                    is_active,
                    is_being_dragged,
                    preferred_width,
                    show_close_button,
                    fade,
                );
                let title_id = response.id;
                let close_clicked = close_response.is_some_and(|res| res.clicked());
                let is_lonely_tab = self.dock_state[surface_index].num_drawers() == 1;

                if self.show_tab_name_on_hover {
                    let tabs = self.dock_state[surface_index][node_index]
                        .drawers_mut()
                        .expect("This node must be a leaf");
                    let tab = &mut tabs[tab_index.0];
                    response = response.on_hover_ui(|ui| {
                        ui.label(tab_viewer.title(tab, drawers));
                    });
                }

                if self.tab_context_menus {
                    let eject_button =
                        Button::new(&self.dock_state.translations.tab_context_menu.eject_button);
                    let close_button =
                        Button::new(&self.dock_state.translations.tab_context_menu.close_button);

                    response.context_menu(|ui| {
                        let leaf = self.dock_state[surface_index][node_index]
                            .get_leaf_mut()
                            .expect("This node must be a leaf");
                        let tab = &mut leaf.drawers[tab_index.0];

                        tab_viewer.context_menu(ui, tab, surface_index, node_index);
                        if (surface_index.is_main() || !is_lonely_tab)
                            && tab_viewer.allowed_in_windows(tab)
                            && ui.add(eject_button).clicked()
                        {
                            self.to_detach.push((surface_index, node_index, tab_index));
                            ui.close();
                        }
                        if show_close_button && ui.add(close_button).clicked() {
                            match tab_viewer.on_close(tab, messager, drawers) {
                                OnCloseResponse::Close => self.to_remove.push(TabRemoval::Tab(
                                    surface_index,
                                    node_index,
                                    tab_index,
                                    ForcedRemoval(false),
                                )),
                                OnCloseResponse::Focus => {
                                    leaf.active = tab_index;
                                    self.new_focused = Some((surface_index, node_index));
                                }
                                OnCloseResponse::Ignore => (),
                            }
                            ui.close();
                        }
                    });
                }

                if close_clicked {
                    self.to_remove.push(TabRemoval::Tab(
                        surface_index,
                        node_index,
                        tab_index,
                        ForcedRemoval(false),
                    ));
                }

                if let Some(pos) = state.last_hover_pos {
                    // Use response.rect.contains instead of
                    // response.hovered as the dragged tab covers
                    // the underlying tab
                    if state.drag_start.is_some() && response.rect.contains(pos) {
                        self.tab_hover_rect = Some((response.rect, tab_index));
                    }
                }

                (response, title_id)
            };

            if response.hovered() {
                tab_hovered = true;
            }

            // Paint hline below each tab unless its active (or option says otherwise).
            let leaf = self.dock_state[surface_index][node_index]
                .get_leaf_mut()
                .unwrap();
            let tab = &mut leaf.drawers[tab_index.0];
            let style = fade.unwrap_or_else(|| self.style.as_ref().unwrap());
            let tab_style = tab_viewer.tab_style_override(tab, &style.tab);
            let tab_style = tab_style.as_ref().unwrap_or(&style.tab);

            if !is_active || tab_style.hline_below_active_tab_name {
                let px = tabs_ui.ctx().pixels_per_point().recip();
                tabs_ui.painter().hline(
                    response.rect.x_range(),
                    tabbar_outer_rect.bottom() - px,
                    (px, style.tab_bar.hline_color),
                );
            }

            if response.clicked()
                || (tabs_ui.memory(|m| m.has_focus(title_id))
                    && tabs_ui.input(|i| i.key_pressed(Key::Enter) || i.key_pressed(Key::Space)))
            {
                leaf.active = tab_index;
                self.new_focused = Some((surface_index, node_index));
            }

            tab_viewer.on_tab_button(tab, &response);

            if self.show_close_buttons && tab_viewer.is_closeable(tab) && response.middle_clicked()
            {
                self.to_remove.push(TabRemoval::Tab(
                    surface_index,
                    node_index,
                    tab_index,
                    ForcedRemoval(false),
                ));
            }
        }

        tab_hovered
    }

    /// Draws the tab add button.
    #[allow(clippy::too_many_arguments)]
    fn drawer_plus(
        &mut self,
        ui: &mut Ui,
        surface_index: SurfaceIndex,
        node_index: NodeIndex,
        tab_viewer: &mut impl TabDrawer<Tab = Drawer>,
        tabbar_outer_rect: Rect,
        offset: f32,
        fade_style: Option<&Style>,
    ) {
        let rect = Rect::from_min_max(
            tabbar_outer_rect.right_top() - vec2(Style::TAB_ADD_BUTTON_SIZE + offset, 0.0),
            tabbar_outer_rect.right_bottom() - vec2(offset, 2.0),
        );

        let ui = &mut ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center))
                .id_salt((node_index, "tab_add")),
        );

        let (rect, mut response) = ui.allocate_exact_size(ui.available_size(), Sense::click());

        response = response.on_hover_cursor(CursorIcon::PointingHand);

        let style = fade_style.unwrap_or_else(|| self.style.as_ref().unwrap());
        let color = if response.hovered() || response.has_focus() {
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, style.buttons.add_tab_bg_fill);
            style.buttons.add_tab_active_color
        } else {
            style.buttons.add_tab_color
        };

        let mut plus_rect = rect;

        rect_set_size_centered(&mut plus_rect, Vec2::splat(Style::TAB_ADD_PLUS_SIZE));

        ui.painter().line_segment(
            [plus_rect.center_top(), plus_rect.center_bottom()],
            Stroke::new(1.0, color),
        );
        ui.painter().line_segment(
            [plus_rect.right_center(), plus_rect.left_center()],
            Stroke::new(1.0, color),
        );

        // Draw button left border.
        ui.painter().vline(
            rect.left(),
            rect.y_range(),
            Stroke::new(
                ui.ctx().pixels_per_point().recip(),
                style.buttons.add_tab_border_color,
            ),
        );

        let popup_id = ui.id().with("tab_add_popup");
        if self.show_add_popup {
            Popup::from_toggle_button_response(&response)
                .id(popup_id)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    tab_viewer.add_popup(ui, surface_index, node_index);
                });
        }

        if response.clicked() {
            tab_viewer.on_add(surface_index, node_index);
        }
    }

    /// Draws the close all button.
    #[allow(clippy::too_many_arguments)]
    #[allow(unused_assignments)]
    fn drawer_close_all(
        &mut self,
        ui: &mut Ui,
        surface_index: SurfaceIndex,
        node_index: NodeIndex,
        tabbar_outer_rect: Rect,
        fade_style: Option<&Style>,
        disabled: bool,
        close_window_disabled: bool,
    ) {
        let rect = Rect::from_min_max(
            tabbar_outer_rect.right_top() - vec2(Style::TAB_CLOSE_ALL_BUTTON_SIZE, 0.0),
            tabbar_outer_rect.right_bottom() - vec2(0.0, 2.0),
        );

        let ui = &mut ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center))
                .id_salt((node_index, "tab_close_all")),
        );

        let (rect, mut response) = ui.allocate_exact_size(ui.available_size(), Sense::click());

        let style = fade_style.unwrap_or_else(|| self.style.as_ref().unwrap());

        // Whether we're on "secondary button mode" due to modifier keys
        let on_secondary_button = self.is_on_secondary_button(surface_index, ui, &response);

        let mut stroke_color = if disabled {
            style.buttons.close_all_tabs_disabled_color
        } else if response.hovered() || response.has_focus() {
            if !(close_window_disabled && on_secondary_button) {
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::ZERO,
                    style.buttons.close_all_tabs_bg_fill,
                );
            }
            style.buttons.close_all_tabs_active_color
        } else {
            style.buttons.close_all_tabs_color
        };

        let mut close_all_rect = rect;

        rect_set_size_centered(&mut close_all_rect, Vec2::splat(Style::TAB_CLOSE_ALL_SIZE));

        if !disabled {
            response = response.on_hover_cursor(CursorIcon::PointingHand);
        }

        if on_secondary_button {
            // Close the entire window
            if close_window_disabled {
                stroke_color = style.buttons.close_all_tabs_disabled_color;
                response = response
                    .on_hover_cursor(CursorIcon::NotAllowed)
                    .on_hover_text(
                        self.dock_state
                            .translations
                            .leaf
                            .close_all_button_disabled_tooltip
                            .as_str(),
                    );
            }
            Self::draw_close_window_symbol(ui, stroke_color, close_all_rect);
        } else {
            // Close all tabs in this leaf
            if !disabled {
                if !surface_index.is_main() && self.secondary_button_context_menu {
                    response.context_menu(|ui| {
                        ui.add_enabled_ui(!close_window_disabled, |ui| {
                            if ui
                                .button(&self.dock_state.translations.leaf.close_all_button)
                                .on_disabled_hover_text(
                                    self.dock_state
                                        .translations
                                        .leaf
                                        .close_all_button_disabled_tooltip
                                        .as_str(),
                                )
                                .clicked()
                            {
                                self.to_remove.push(TabRemoval::Window(surface_index));
                            }
                        });
                    });
                }
            } else {
                response = response
                    .on_hover_cursor(CursorIcon::NotAllowed)
                    .on_hover_text(
                        self.dock_state
                            .translations
                            .leaf
                            .close_button_disabled_tooltip
                            .as_str(),
                    );
            }

            if response.clicked() {
                if on_secondary_button {
                    if !close_window_disabled {
                        self.to_remove.push(TabRemoval::Window(surface_index));
                    }
                } else if !disabled {
                    self.to_remove
                        .push(TabRemoval::Node(surface_index, node_index));
                }
            }

            ui.painter().line_segment(
                [close_all_rect.left_top(), close_all_rect.right_bottom()],
                Stroke::new(1.0, stroke_color),
            );
            ui.painter().line_segment(
                [close_all_rect.right_top(), close_all_rect.left_bottom()],
                Stroke::new(1.0, stroke_color),
            );
        }

        // Draw button left border.
        ui.painter().vline(
            rect.left(),
            rect.y_range(),
            Stroke::new(
                ui.ctx().pixels_per_point().recip(),
                style.buttons.close_all_tabs_border_color,
            ),
        );

        if !disabled && !on_secondary_button {
            response = self.show_tooltip_hints(surface_index, response);
        }
    }

    /// Draws the collapse button.
    fn drawer_collapse(
        &mut self,
        ui: &mut Ui,
        surface_index: SurfaceIndex,
        node_index: NodeIndex,
        tabbar_outer_rect: Rect,
        fade_style: Option<&Style>,
        collapsed: bool,
    ) {
        let rect = Rect::from_min_max(
            tabbar_outer_rect.left_top(),
            tabbar_outer_rect.left_bottom() + vec2(Style::TAB_COLLAPSE_BUTTON_SIZE, 0.0),
        );

        let ui = &mut ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center))
                .id_salt((node_index, "tab_collapse")),
        );

        let (rect, mut response) = ui.allocate_exact_size(ui.available_size(), Sense::click());

        response = response.on_hover_cursor(CursorIcon::PointingHand);

        let style = fade_style.unwrap_or_else(|| self.style.as_ref().unwrap());

        // Whether we're on "secondary button mode" due to modifier keys
        let on_secondary_button = self.is_on_secondary_button(surface_index, ui, &response);

        let color = if response.hovered() || response.has_focus() {
            ui.painter().rect_filled(
                rect,
                CornerRadius::ZERO,
                style.buttons.collapse_tabs_bg_fill,
            );
            style.buttons.collapse_tabs_active_color
        } else {
            style.buttons.collapse_tabs_color
        };

        let mut arrow_rect = rect;
        rect_set_size_centered(&mut arrow_rect, Vec2::splat(Style::TAB_COLLAPSE_ARROW_SIZE));

        if on_secondary_button {
            // Collapse the entire window
            Self::draw_chevron_down(ui, style, color, arrow_rect);
        } else {
            // Draw arrow.
            Self::draw_arrow(collapsed, ui, color, arrow_rect);
        }

        // Draw button right border.
        ui.painter().vline(
            rect.right(),
            rect.y_range(),
            Stroke::new(
                ui.ctx().pixels_per_point().recip(),
                style.buttons.collapse_tabs_border_color,
            ),
        );

        if response.clicked() {
            if on_secondary_button {
                self.window_toggle_minimized(surface_index);
            } else {
                self.dock_state[surface_index][node_index].set_collapsed(!collapsed);
                self.dock_state[surface_index].node_update_collapsed(node_index);
                self.window_update_collapsed(surface_index, node_index);
            }
        }

        if !surface_index.is_main() && self.secondary_button_context_menu {
            response.context_menu(|ui| {
                if ui
                    .button(&self.dock_state.translations.leaf.minimize_button)
                    .clicked()
                {
                    ui.close();
                    self.window_toggle_minimized(surface_index);
                }
            });
        }

        if !on_secondary_button {
            self.show_tooltip_hints(surface_index, response);
        }
    }

    fn show_tooltip_hints(&mut self, surface_index: SurfaceIndex, response: Response) -> Response {
        if !surface_index.is_main()
            && self.show_secondary_button_hint
            && (self.secondary_button_context_menu || self.secondary_button_on_modifier)
        {
            let hint = if self.secondary_button_context_menu && self.secondary_button_on_modifier {
                &self
                    .dock_state
                    .translations
                    .leaf
                    .minimize_button_modifier_menu_hint
            } else if self.secondary_button_context_menu {
                &self.dock_state.translations.leaf.minimize_button_menu_hint
            } else {
                &self
                    .dock_state
                    .translations
                    .leaf
                    .minimize_button_modifier_hint
            };
            return response.on_hover_text(hint);
        }
        response
    }

    fn is_on_secondary_button(
        &self,
        surface_index: SurfaceIndex,
        ui: &mut Ui,
        response: &Response,
    ) -> bool {
        !surface_index.is_main()
            && self.secondary_button_on_modifier
            && ui.input(|i| {
                i.modifiers
                    .matches_logically(self.secondary_button_modifiers)
            })
            && (response.hovered() || response.has_focus() || response.is_pointer_button_down_on())
    }

    fn draw_close_window_symbol(ui: &mut Ui, stroke_color: Color32, close_all_rect: Rect) {
        ui.painter().add(Shape::line(
            vec![
                close_all_rect
                    .right_center()
                    .lerp(close_all_rect.right_bottom(), 0.5),
                close_all_rect.right_bottom(),
                close_all_rect.left_bottom(),
                close_all_rect.left_top(),
                close_all_rect
                    .center_top()
                    .lerp(close_all_rect.left_top(), 0.5),
            ],
            Stroke::new(1.0, stroke_color),
        ));
        ui.painter().line_segment(
            [close_all_rect.center_top(), close_all_rect.right_center()],
            Stroke::new(1.0, stroke_color),
        );
        ui.painter().line_segment(
            [close_all_rect.center(), close_all_rect.right_top()],
            Stroke::new(1.0, stroke_color),
        );
    }

    fn draw_arrow(collapsed: bool, ui: &mut Ui, color: Color32, arrow_rect: Rect) {
        ui.painter().add(Shape::convex_polygon(
            if collapsed {
                // Arrow pointing rightwards.
                vec![
                    arrow_rect.left_top(),
                    arrow_rect.right_center(),
                    arrow_rect.left_bottom(),
                ]
            } else {
                // Arrow pointing downwards.
                vec![
                    arrow_rect.left_top(),
                    arrow_rect.right_top(),
                    arrow_rect.center_bottom(),
                ]
            },
            color,
            Stroke::NONE,
        ));
    }

    fn draw_chevron_down(ui: &mut Ui, style: &Style, color: Color32, arrow_rect: Rect) {
        ui.painter().add(Shape::convex_polygon(
            // Arrow pointing downwards.
            vec![
                arrow_rect.left_top(),
                arrow_rect.right_top(),
                arrow_rect.center(),
            ],
            color,
            Stroke::NONE,
        ));

        // Chevron pointing downwards.
        ui.painter().add(Shape::convex_polygon(
            vec![
                arrow_rect.left_center(),
                arrow_rect.right_center(),
                arrow_rect.center_bottom(),
            ],
            color,
            Stroke::NONE,
        ));
        let color = style.buttons.minimize_window_bg_fill;
        ui.painter().add(Shape::convex_polygon(
            vec![
                arrow_rect
                    .left_center()
                    .lerp(arrow_rect.right_center(), 0.25),
                arrow_rect
                    .left_center()
                    .lerp(arrow_rect.right_center(), 0.75),
                arrow_rect.center().lerp(arrow_rect.center_bottom(), 0.5),
            ],
            color,
            Stroke::NONE,
        ));
    }

    /// Updates the collapsed state of the node and its parents.
    fn window_update_collapsed(&mut self, surface_index: SurfaceIndex, node_index: NodeIndex) {
        let surface = &mut self.dock_state[surface_index];
        let collapsed = surface[node_index].is_collapsed();
        if !collapsed {
            if let Some(window_state) = self.dock_state.get_window_state_mut(surface_index) {
                window_state.set_new(true);
            }
        } else if surface.root_node().is_some_and(|root| root.is_collapsed()) {
            let root_index = NodeIndex::root();
            let surface_height = if surface.root_node().is_some() {
                surface[root_index].rect().unwrap().height()
            } else {
                0.0
            };
            if let Some(window_state) = self.dock_state.get_window_state_mut(surface_index) {
                window_state.set_expanded_height(surface_height);
            }
        }
    }

    /// * `active` means "the tab that is opened in the parent panel".
    /// * `focused` means "the tab that was last interacted with".
    ///
    /// Returns the main button response plus the response of the close button, if any.
    #[allow(clippy::too_many_arguments)]
    fn drawer_title(
        &mut self,
        ui: &mut Ui,
        tab_style: &TabStyle,
        id: Id,
        label: WidgetText,
        focused: bool,
        active: bool,
        is_being_dragged: bool,
        preferred_width: Option<f32>,
        show_close_button: bool,
        fade: Option<&Style>,
    ) -> (Response, Option<Response>) {
        let style = fade.unwrap_or_else(|| self.style.as_ref().unwrap());
        let galley = label.into_galley(ui, None, f32::INFINITY, TextStyle::Button);
        let x_spacing = 8.0;
        let text_width = galley.size().x + 2.0 * x_spacing;
        let close_button_size = if show_close_button {
            Style::TAB_CLOSE_BUTTON_SIZE.min(style.tab_bar.height)
        } else {
            0.0
        };

        // Compute total width of the tab bar.
        let minimum_width = tab_style
            .minimum_width
            .unwrap_or(0.0)
            .at_least(text_width + close_button_size);
        let tab_width = preferred_width.unwrap_or(0.0).at_least(minimum_width);

        let (_, tab_rect) = ui.allocate_space(vec2(tab_width, ui.available_height()));
        let mut response = ui.interact(tab_rect, id, Sense::click_and_drag());
        if ui.ctx().dragged_id().is_none() && self.draggable_tabs {
            response = response.on_hover_cursor(CursorIcon::Grab);
        }

        let tab_style = if focused || is_being_dragged {
            if response.has_focus() {
                &tab_style.focused_with_kb_focus
            } else {
                &tab_style.focused
            }
        } else if active {
            if response.has_focus() {
                &tab_style.active_with_kb_focus
            } else {
                &tab_style.active
            }
        } else if response.hovered() {
            &tab_style.hovered
        } else if response.has_focus() {
            &tab_style.inactive_with_kb_focus
        } else {
            &tab_style.inactive
        };

        // Draw the full tab first and then the stroke on top to avoid the stroke
        // mixing with the background color.
        ui.painter()
            .rect_filled(tab_rect, tab_style.corner_radius, tab_style.bg_fill);
        let stroke_rect = rect_stroke_box(tab_rect, 1.0);
        ui.painter().rect_stroke(
            stroke_rect,
            tab_style.corner_radius,
            Stroke::new(1.0, tab_style.outline_color),
            StrokeKind::Inside,
        );
        if !is_being_dragged {
            // Make the tab name area connect with the tab ui area.
            ui.painter().hline(
                RangeInclusive::new(
                    stroke_rect.min.x + f32::max(tab_style.corner_radius.sw.into(), 1.5),
                    stroke_rect.max.x - f32::max(tab_style.corner_radius.se.into(), 1.5),
                ),
                stroke_rect.bottom(),
                Stroke::new(2.0, tab_style.bg_fill),
            );
        }

        let mut text_rect = tab_rect;
        text_rect.set_width(text_rect.width() - close_button_size);
        let text_pos = {
            let pos = Align2::CENTER_CENTER.pos_in_rect(&text_rect.shrink2(vec2(x_spacing, 0.0)));
            pos - galley.size() / 2.0
        };

        ui.painter()
            .add(TextShape::new(text_pos, galley, tab_style.text_color));

        let close_response = show_close_button.then(|| {
            let mut close_button_rect = tab_rect;
            close_button_rect.set_left(text_rect.right());
            close_button_rect =
                Rect::from_center_size(close_button_rect.center(), Vec2::splat(close_button_size));

            let close_response = ui
                .interact(close_button_rect, id.with("close-button"), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);

            let color = if close_response.hovered() || close_response.has_focus() {
                style.buttons.close_tab_active_color
            } else {
                style.buttons.close_tab_color
            };

            if close_response.hovered() || close_response.has_focus() {
                let mut corner_radius = tab_style.corner_radius;
                corner_radius.nw = 0;
                corner_radius.sw = 0;

                ui.painter().rect_filled(
                    close_button_rect,
                    corner_radius,
                    style.buttons.close_tab_bg_fill,
                );
            }

            let mut x_rect = close_button_rect;
            rect_set_size_centered(&mut x_rect, Vec2::splat(Style::TAB_CLOSE_X_SIZE));
            ui.painter().line_segment(
                [x_rect.left_top(), x_rect.right_bottom()],
                Stroke::new(1.0, color),
            );
            ui.painter().line_segment(
                [x_rect.right_top(), x_rect.left_bottom()],
                Stroke::new(1.0, color),
            );

            close_response
        });

        (response, close_response)
    }

    #[allow(clippy::too_many_arguments)]
    fn drawer_bar_scroll(
        &mut self,
        ui: &mut Ui,
        state: &State,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        actual_width: f32,
        available_width: f32,
        scroll_bar_width: f32,
        tabbar_response: &Response,
        tab_hovered: bool,
        fade_style: Option<&Style>,
    ) {
        if available_width <= 0.0 {
            return;
        }

        let leaf = self.dock_state[surface_index][node_index]
            .get_leaf_mut()
            .expect("This node must be a leaf");
        let overflow = (actual_width - available_width).at_least(0.0);
        let style = fade_style.unwrap_or_else(|| self.style.as_ref().unwrap());

        // Compare to 1.0 and not 0.0 to avoid drawing a scroll bar due
        // to floating point precision issue during tab drawing.
        if overflow > 1.0 {
            if style.tab_bar.show_scroll_bar_on_overflow {
                // Draw scroll bar
                let bar_height = 7.5;
                let (scroll_bar_rect, _scroll_bar_response) = ui.allocate_exact_size(
                    vec2(scroll_bar_width, bar_height),
                    Sense::click_and_drag(),
                );

                // Compute scroll bar handle position and size.
                let overflow_ratio = actual_width / available_width;
                let scroll_ratio = -leaf.scroll / overflow;

                let scroll_bar_handle_size = overflow_ratio.recip() * scroll_bar_rect.width();
                let scroll_bar_handle_start = lerp(
                    scroll_bar_rect.left()..=scroll_bar_rect.right() - scroll_bar_handle_size,
                    scroll_ratio,
                );
                let scroll_bar_handle_rect = Rect::from_min_size(
                    pos2(scroll_bar_handle_start, scroll_bar_rect.min.y),
                    vec2(scroll_bar_handle_size, bar_height),
                );

                let scroll_bar_handle_response = ui.interact(
                    scroll_bar_handle_rect,
                    self.id.with((node_index, "node")),
                    Sense::drag(),
                );

                // Coefficient to apply to input displacements so that we move the scroll by the correct amount.
                let points_to_scroll_coefficient =
                    overflow / (scroll_bar_rect.width() - scroll_bar_handle_size);

                leaf.scroll -=
                    scroll_bar_handle_response.drag_delta().x * points_to_scroll_coefficient;

                if let Some(pos) = state.last_hover_pos {
                    if scroll_bar_rect.contains(pos) {
                        leaf.scroll += ui
                            .input(|i| i.smooth_scroll_delta.y + i.smooth_scroll_delta.x)
                            * points_to_scroll_coefficient;
                    }
                }

                // Draw the bar.
                ui.painter()
                    .rect_filled(scroll_bar_rect, 0.0, ui.visuals().extreme_bg_color);

                ui.painter().rect_filled(
                    scroll_bar_handle_rect,
                    bar_height / 2.0,
                    ui.visuals()
                        .widgets
                        .style(&scroll_bar_handle_response)
                        .bg_fill,
                );
            }

            // Handle user input.
            if tabbar_response.hovered() || tab_hovered {
                leaf.scroll += ui.input(|i| i.smooth_scroll_delta.y + i.smooth_scroll_delta.x);
            }
        }

        leaf.scroll = leaf.scroll.clamp(-overflow, 0.0);
    }

    #[allow(clippy::too_many_arguments)]
    fn drawer_body(
        &mut self,
        ui: &mut Ui,
        messager: &mut Messager,
        engine: &Engine,
        log: &mut Log,
        drawers: &Vec<Box<dyn kairos_editor::ui::Drawer>>,
        state: &State,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        tab_viewer: &mut impl TabDrawer<Tab = Drawer>,
        spacing: Vec2,
        tabbar_rect: Rect,
        fade: Option<(&Style, f32)>,
        collapsed: bool,
    ) {
        let (body_rect, _body_response) =
            ui.allocate_exact_size(ui.available_size_before_wrap(), Sense::hover());

        let leaf = self.dock_state[surface_index][node_index]
            .get_leaf_mut()
            .expect("This node must be a leaf");
        let LeafNode {
            rect,
            viewport,
            drawers: tabs,
            active,
            ..
        } = leaf;
        if !collapsed {
            if let Some(tab) = tabs.get_mut(active.0) {
                if *viewport != body_rect {
                    *viewport = body_rect;
                    tab_viewer.on_rect_changed(tab);
                }

                if ui.input(|i| i.pointer.any_click()) {
                    if let Some(pos) = state.last_hover_pos {
                        if body_rect.contains(pos)
                            && Some(ui.layer_id()) == ui.ctx().layer_id_at(pos)
                        {
                            self.new_focused = Some((surface_index, node_index));
                        }
                    }
                }

                let (style, fade_factor) =
                    fade.unwrap_or_else(|| (self.style.as_ref().unwrap(), 1.0));
                let tabs_styles = tab_viewer.tab_style_override(tab, &style.tab);

                let tabs_style = tabs_styles.as_ref().unwrap_or(&style.tab);

                if tab_viewer.clear_background(tab) {
                    ui.painter().rect_filled(
                        body_rect,
                        tabs_style.tab_body.corner_radius,
                        tabs_style.tab_body.bg_fill,
                    );
                }

                // Construct a new ui with the correct tab id.
                //
                // We are forced to use `Ui::new` because other methods (eg: push_id) always mix
                // the provided id with their own which would cause tabs to change id when moved
                // from node to node.
                let id = self.id.with(tab_viewer.id(tab, drawers));
                ui.ctx().check_for_id_clash(id, body_rect, "a tab with id");
                let ui = &mut Ui::new(
                    ui.ctx().clone(),
                    id,
                    UiBuilder::new().max_rect(body_rect).layer_id(ui.layer_id()),
                );
                ui.set_clip_rect(Rect::from_min_max(ui.cursor().min, ui.clip_rect().max));

                // Use initial spacing for ui.
                ui.spacing_mut().item_spacing = spacing;

                // Offset the background rectangle up to hide the top border behind the clip rect.
                // To avoid anti-aliasing lines when the stroke width is not divisible by two, we
                // need to calculate the effective anti-aliased stroke width.
                let effective_stroke_width = (tabs_style.tab_body.stroke.width / 2.0).ceil() * 2.0;
                let tab_body_rect = Rect::from_min_max(
                    ui.clip_rect().min - vec2(0.0, effective_stroke_width),
                    ui.clip_rect().max,
                );
                ui.painter().rect_stroke(
                    rect_stroke_box(tab_body_rect, tabs_style.tab_body.stroke.width),
                    tabs_style.tab_body.corner_radius,
                    tabs_style.tab_body.stroke,
                    StrokeKind::Inside,
                );

                ScrollArea::new(tab_viewer.scroll_bars(tab, drawers)).show(ui, |ui| {
                    Frame::new()
                        .inner_margin(tabs_style.tab_body.inner_margin)
                        .show(ui, |ui| {
                            if fade_factor != 1.0 {
                                fade_visuals(ui.visuals_mut(), fade_factor);
                            }
                            let available_rect = ui.available_rect_before_wrap();
                            ui.expand_to_include_rect(available_rect);
                            tab_viewer.ui(ui, tab, messager, engine, log, drawers);
                        });
                });
            }
        }

        // change hover destination
        if let Some(pointer) = state.last_hover_pos {
            // Prevent borrow checker issues.
            let rect = rect.to_owned();

            // if the dragged tab isn't allowed in a window,
            // it's unnecessary to change the hover state
            let is_dragged_valid = match &state.dnd {
                Some(DragDropState {
                    drag: DragData { src, .. },
                    ..
                }) => match *src {
                    TreeComponent::Tab(d_surf, d_node, d_tab) => {
                        if let Node::Leaf(leaf) = &mut self.dock_state[d_surf][d_node] {
                            tab_viewer.allowed_in_windows(&mut leaf.drawers[d_tab.0])
                                || surface_index == SurfaceIndex::main()
                        } else {
                            true
                        }
                    }
                    _ => unreachable!("collections of nodes can't be dragged (yet)"),
                },
                _ => true,
            };

            // Use rect.contains instead of response.hovered as the dragged tab covers
            // the underlying responses.
            if state.drag_start.is_some() && rect.contains(pointer) && is_dragged_valid {
                let on_title_bar = tabbar_rect.contains(pointer);
                let (dst, tab) = {
                    match self.tab_hover_rect {
                        Some((rect, tab_index)) => (
                            TreeComponent::Tab(surface_index, node_index, tab_index),
                            Some(rect),
                        ),
                        None => (
                            TreeComponent::Node(surface_index, node_index),
                            on_title_bar.then_some(tabbar_rect),
                        ),
                    }
                };

                ui.memory_mut(|mem| {
                    mem.data.insert_temp(
                        self.id.with("hover_data"),
                        Some(HoverData { rect, dst, tab }),
                    );
                });
            }
        }
    }
}

#[inline(always)]
pub fn expand_to_pixel(mut rect: Rect, ppi: f32) -> Rect {
    rect.min = map_to_pixel_pos(rect.min, ppi, f32::floor);
    rect.max = map_to_pixel_pos(rect.max, ppi, f32::ceil);
    rect
}

#[inline(always)]
pub fn map_to_pixel_pos(mut pos: Pos2, ppi: f32, map: fn(f32) -> f32) -> Pos2 {
    pos.x = map_to_pixel(pos.x, ppi, map);
    pos.y = map_to_pixel(pos.y, ppi, map);
    pos
}

#[inline(always)]
pub fn map_to_pixel(point: f32, ppi: f32, map: fn(f32) -> f32) -> f32 {
    map(point * ppi) / ppi
}

pub fn rect_set_size_centered(rect: &mut Rect, size: Vec2) {
    let center = rect.center();
    rect.set_width(size.x);
    rect.set_height(size.y);
    rect.set_center(center);
}

/// Shrink a rectangle so that the stroke is fully contained inside
/// the original rectangle.
pub fn rect_stroke_box(rect: Rect, width: f32) -> Rect {
    rect.expand(-f32::ceil(width / 2.0))
}

/// Fade a `egui_dock::Style` to a certain opacity
pub fn fade_dock_style(style: &mut Style, factor: f32) {
    style.main_surface_border_stroke.color = style
        .main_surface_border_stroke
        .color
        .linear_multiply(factor);
    fade_tab_style(&mut style.tab, factor);
    fade_button_style(&mut style.buttons, factor);
    fade_seperator_style(&mut style.separator, factor);
    fade_tab_bar_style(&mut style.tab_bar, factor);
}

fn fade_tab_bar_style(style: &mut TabBarStyle, factor: f32) {
    style.hline_color = style.hline_color.linear_multiply(factor);
    style.bg_fill = style.bg_fill.linear_multiply(factor);
}

fn fade_seperator_style(style: &mut SeparatorStyle, factor: f32) {
    style.color_idle = style.color_idle.linear_multiply(factor);
    style.color_hovered = style.color_hovered.linear_multiply(factor);
    style.color_dragged = style.color_dragged.linear_multiply(factor);
}

fn fade_button_style(style: &mut ButtonsStyle, factor: f32) {
    style.close_tab_color = style.close_tab_color.linear_multiply(factor);
    style.close_tab_active_color = style.close_tab_active_color.linear_multiply(factor);
    style.close_tab_bg_fill = style.close_tab_bg_fill.linear_multiply(factor);
    style.add_tab_color = style.add_tab_color.linear_multiply(factor);
    style.add_tab_active_color = style.add_tab_active_color.linear_multiply(factor);
    style.add_tab_bg_fill = style.add_tab_bg_fill.linear_multiply(factor);
    style.add_tab_border_color = style.add_tab_border_color.linear_multiply(factor);
}

fn fade_tab_style(style: &mut TabStyle, factor: f32) {
    fade_tab_interaction_style(&mut style.active, factor);
    fade_tab_interaction_style(&mut style.inactive, factor);
    fade_tab_interaction_style(&mut style.focused, factor);
    fade_tab_interaction_style(&mut style.hovered, factor);
    fade_tab_body_style(&mut style.tab_body, factor);
}

fn fade_tab_interaction_style(style: &mut TabInteractionStyle, factor: f32) {
    style.outline_color = style.outline_color.linear_multiply(factor);
    style.bg_fill = style.bg_fill.linear_multiply(factor);
    style.text_color = style.text_color.linear_multiply(factor);
}

fn fade_tab_body_style(style: &mut TabBodyStyle, factor: f32) {
    style.stroke.color = style.stroke.color.linear_multiply(factor);
    style.bg_fill = style.bg_fill.linear_multiply(factor);
}

/// Fade a `egui::style::Visuals` to a certain opacity
pub(super) fn fade_visuals(visuals: &mut Visuals, factor: f32) {
    if let Some(override_text_color) = &mut visuals.override_text_color {
        *override_text_color = override_text_color.linear_multiply(factor);
    }
    visuals.hyperlink_color = visuals.hyperlink_color.linear_multiply(factor);
    visuals.faint_bg_color = visuals.faint_bg_color.linear_multiply(factor);
    visuals.extreme_bg_color = visuals.extreme_bg_color.linear_multiply(factor);
    visuals.code_bg_color = visuals.code_bg_color.linear_multiply(factor);
    visuals.warn_fg_color = visuals.warn_fg_color.linear_multiply(factor);
    visuals.error_fg_color = visuals.error_fg_color.linear_multiply(factor);
    visuals.window_fill = visuals.window_fill.linear_multiply(factor);
    visuals.panel_fill = visuals.window_fill.linear_multiply(factor);
    fade_widgets(&mut visuals.widgets, factor);
}

fn fade_widgets(widgets: &mut Widgets, factor: f32) {
    fade_widget_visuals(&mut widgets.noninteractive, factor);
    fade_widget_visuals(&mut widgets.inactive, factor);
    fade_widget_visuals(&mut widgets.hovered, factor);
    fade_widget_visuals(&mut widgets.active, factor);
    fade_widget_visuals(&mut widgets.open, factor);
}

fn fade_widget_visuals(visuals: &mut WidgetVisuals, factor: f32) {
    visuals.bg_fill = visuals.bg_fill.linear_multiply(factor);
    visuals.weak_bg_fill = visuals.weak_bg_fill.linear_multiply(factor);
    visuals.bg_stroke.color = visuals.bg_stroke.color.linear_multiply(factor);
    visuals.fg_stroke.color = visuals.fg_stroke.color.linear_multiply(factor);
}
