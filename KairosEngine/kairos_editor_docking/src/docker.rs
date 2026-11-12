use std::ops::{Index, IndexMut, RangeInclusive};

use duplicate::duplicate;
use egui::{Align, Align2, Button, CentralPanel, Color32, Context, CornerRadius, CursorIcon, EventFilter, Frame, Id, LayerId, Layout, NumExt, Pos2, Rect, Response, ScrollArea, Sense, Stroke, TextStyle, Ui, UiBuilder, Vec2, Visuals, WidgetText, emath::TSTransform, epaint::TextShape, lerp, pos2, style::{WidgetVisuals, Widgets}, vec2};
use paste::paste;

use crate::{drag::{DragData, DragDropState, HoverData, TreeComponent}, split::{Fraction, Split}, styles::{Style, TabBodyStyle, TabInteractionStyle, TabStyle}, surface::{Surface, SurfaceIndex, WindowState}, tab::{TabState}, translations::Translations, tree::{LeafNode, Node, NodeIndex, TabIndex, Tree}};

#[derive(Clone, Copy)]
pub enum TabInsert {
    Split(Split),
    Insert(TabIndex),
    Append,
}

#[derive(Clone, Copy)]
pub enum TabDestination {
    Window(Rect),
    Node(SurfaceIndex, NodeIndex, TabInsert),
    EmptySurface(SurfaceIndex),
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum AllowedSplits {
    #[default]
    All = 0b11,
    LeftRightOnly = 0b10,
    TopBottomOnly = 0b01,
    None = 0b00,
}

#[derive(Clone, Copy)]
struct ForcedRemoval(pub bool);

enum TabRemoval {
    Tab(SurfaceIndex, NodeIndex, TabIndex, ForcedRemoval),
    Node(SurfaceIndex, NodeIndex),
    Window(SurfaceIndex),
}

#[derive(Clone, Default)]
struct State {
    drag_start: Option<Pos2>,
    last_hover_pos: Option<Pos2>,
    drag: Option<DragDropState>,
    pub window_fade: Option<(f64, SurfaceIndex)>,
}

impl State {
    fn load(ctx: &Context, id: Id) -> Self {
        ctx.data_mut(|d| {
            d.get_temp(id).unwrap_or(Self {
                drag_start: None,
                last_hover_pos: None,
                drag: None,
                window_fade: None,
            })
        })
    }

    fn is_drag_locked(&self, ctx: &Context, style: &Style) -> bool {
        self.drag.as_ref().is_some_and(|drag_drop_state| drag_drop_state.is_locked(ctx, style))
    }

    fn set_drag(&mut self, drag: DragData, drop: HoverData, ctx: &Context, style: &Style) {
        if !self.is_drag_locked(ctx, style) {
            self.drag = Some(DragDropState { hover: drop, drag, pointer: ctx.pointer_hover_pos().unwrap_or(Pos2::ZERO), locked: None })
        }
    }

    fn reset_drag(&mut self) {
        self.drag = None;
        self.window_fade = None;
        self.drag_start = None;
    }

    pub fn store(self, ctx: &Context, id: Id) {
        ctx.data_mut(|d| d.insert_temp(id, self));
    }
}

pub struct DockState{
    surfaces: Vec<Surface>,
    focused_surface: Option<SurfaceIndex>,
    pub translations: Translations,
    pub style: Style,
}

impl DockState {
    pub fn new() -> Self {
        Self {
            surfaces: vec![Surface::Main(Tree::new(vec![]))],
            focused_surface: None,
            translations: Translations::english(),
            style: Style::default(),
        }
    }

    pub fn move_tab(&mut self, (src_surface, src_node, src_tab): (SurfaceIndex, NodeIndex, TabIndex), dst_tab: impl Into<TabDestination>) {
        match dst_tab.into() {
            TabDestination::Window(position) => {
                self.detach_tab((src_surface, src_node, src_tab), position);
                return;
            },
            TabDestination::Node(dst_surface, dst_node, dst_tab) => {
                if src_surface == dst_surface && src_node == dst_node && self[src_surface][src_node].tabs_count() == 1 {
                    return;
                }

                if let Some(tab) = self[src_surface][src_node].remove_tab(src_tab) {
                    match dst_tab {
                        TabInsert::Split(split) => {
                            self[dst_surface].split(dst_node, split, Fraction::new(0.5), Node::leaf(tab));
                        },
                        TabInsert::Insert(index) => {
                            self[dst_surface][dst_node].insert_tab(index, tab)
                        },
                        TabInsert::Append => self[dst_surface][dst_node].append_tab(tab),
                    }
                }
            },
            TabDestination::EmptySurface(dst_surface) => {
                if !self[dst_surface].is_empty() {
                    return;
                }
                if let Some(tab) = self[src_surface][src_node].remove_tab(src_tab) {
                    self[dst_surface] = Tree::new(vec![tab]);
                }
            },
        }
        if self[src_surface][src_node].is_leaf() && self[src_surface][src_node].tabs_count() == 0 {
            self[src_surface].remove_leaf(src_node);
        }
        if self[src_surface].is_empty() && !src_surface.is_main() {
            self.remove_surface(src_surface);
        }
    }

    pub fn detach_tab(&mut self, (src_surface, src_node, src_tab): (SurfaceIndex, NodeIndex, TabIndex), window_rect: Rect) -> Option<SurfaceIndex> {
        let Some(tab) = self[src_surface][src_node].remove_tab(src_tab) else {
            return None;
        };
        let surface_index = self.add_window(vec![tab]);

        let Some(state) = self.get_window_state_mut(surface_index) else {
            return None;
        };
        state.set_position(window_rect.min);
        if src_surface.is_main() {
            state.set_size(window_rect.size() * 0.8);
        } else {
            state.set_size(window_rect.size());
        }

        if self[src_surface][src_node].is_leaf() && self[src_surface][src_node].tabs_count() == 0 {
            self[src_surface].remove_leaf(src_node);
        }
        if self[src_surface].is_empty() && !src_surface.is_main() {
            self.remove_surface(src_surface);
        }
        Some(surface_index)
    }

    fn add_window(&mut self, tabs: Vec<TabState>) -> SurfaceIndex {
        let surface = Surface::Window(Tree::new(tabs), WindowState::new());
        let index = self.surfaces.len();
        self.surfaces.push(surface);
        SurfaceIndex(index)
    }

    fn get_window_state_mut(&mut self, surface: SurfaceIndex) -> Option<&mut WindowState> {
        match &mut self.surfaces[surface.0] {
            Surface::Window(_, state) => Some(state),
            Surface::Main(_) => None,
        }
    }

    fn remove_surface(&mut self, surface: SurfaceIndex) -> Option<Surface> {
        if surface.is_main() {
            return None;
        }
        (surface.0 < self.surfaces.len()).then(|| {
            self.focused_surface = Some(SurfaceIndex::main());
            if surface.0 == self.surfaces.len() - 1 {
                self.surfaces.pop().unwrap()
            } else {
                let dest = &mut self.surfaces[surface.0];
                std::mem::replace(dest, Surface::Window(Tree::new(vec![]), WindowState::new()))
            }
        })
    }

    pub fn valid_surface_indices(&self) -> Box<[SurfaceIndex]> {
        (0..self.surfaces.len())
            .filter_map(|index| {
                let index = SurfaceIndex(index);
                self.is_surface_valid(index).then_some(index)
            })
            .collect()
    }

    fn is_surface_valid(&self, surface: SurfaceIndex) -> bool {
        self.surfaces
            .get(surface.0)
            .is_some_and(|surface| !surface.is_empty())
    }

    pub fn main_surface(&self) -> &Tree {
        &self[SurfaceIndex::main()]
    }

    pub fn main_surface_mut(&mut self) -> &mut Tree {
        &mut self[SurfaceIndex::main()]
    }

    pub fn focused_leaf(&self) -> Option<(SurfaceIndex, NodeIndex)> {
        let surface = self.focused_surface?;
        self[surface].focused_leaf().map(|leaf| (surface, leaf))
    }

    pub fn remove_tab(&mut self, (surface_index, node_index, tab_index): (SurfaceIndex, NodeIndex, TabIndex)) -> Option<TabState> {
        let removed_tab = self[surface_index].remove_tab((node_index, tab_index));
        if !surface_index.is_main() && self[surface_index].is_empty() {
            self.remove_surface(surface_index);
        }
        removed_tab
    }

    pub fn remove_leaf(&mut self, (surface_index, node_index): (SurfaceIndex, NodeIndex)) {
        self[surface_index].remove_leaf(node_index);
        if !surface_index.is_main() && self[surface_index].is_empty() {
            self.remove_surface(surface_index);
        }
    }

    pub fn set_focused_node_and_surface(&mut self, (surface_index, node_index): (SurfaceIndex, NodeIndex)) {
        if self.is_surface_valid(surface_index) && node_index.0 < self[surface_index].len() {
            if self[surface_index][node_index].is_leaf() {
                self.focused_surface = Some(surface_index);
                self[surface_index].set_focused_node(node_index);
                return;
            }
        }
        self.focused_surface = None;
    }
}

impl Index<SurfaceIndex> for DockState {
    type Output = Tree;

    fn index(&self, index: SurfaceIndex) -> &Self::Output {
        &self.surfaces[index.0].get_tree()
    }
}

impl IndexMut<SurfaceIndex> for DockState {
    fn index_mut(&mut self, index: SurfaceIndex) -> &mut Self::Output {
        self.surfaces[index.0].get_tree_mut()
    }
}

impl Index<SurfaceIndex> for Vec<Surface> {
    type Output = Tree;

    fn index(&self, index: SurfaceIndex) -> &Self::Output {
        self[index.0].get_tree()
    }
}

impl IndexMut<SurfaceIndex> for Vec<Surface> {
    fn index_mut(&mut self, index: SurfaceIndex) -> &mut Self::Output {
        self[index.0].get_tree_mut()
    }
}

pub struct Docker<'tree> {
    id: Id,
    dock_state: &'tree mut DockState,
    window_bounds: Option<Rect>,
    draggable_tabs: bool,
    show_tab_context_menus: bool,

    to_remove: Vec<TabRemoval>,
    to_detach: Vec<(SurfaceIndex, NodeIndex, TabIndex)>,
    new_focused: Option<(SurfaceIndex, NodeIndex)>,
    tab_hover_rect: Option<(Rect, TabIndex)>,
}

impl<'tree> Docker<'tree> {
    pub fn new(id: impl Into<Id>, state: &'tree mut DockState) -> Docker<'tree> {
        Self {
            id: id.into(),
            dock_state: state,
            window_bounds: None,
            draggable_tabs: true,
            show_tab_context_menus: true,

            to_remove: Vec::new(),
            to_detach: Vec::new(),
            new_focused: None,
            tab_hover_rect: None,
        }
    }
}

impl Docker<'_> {
    pub fn show(
        self,
        ui: &mut Ui,
    ) {
        CentralPanel::default()
            .frame(
                Frame::central_panel(&ui.global_style())
                    .inner_margin(0.)
                    .fill(Color32::TRANSPARENT),
            )
            .show(ui, |ui| {
                self.show_inside(ui);
            });
    }

    fn show_inside(
        mut self,
        ui: &mut Ui
    ) {
        self.window_bounds.get_or_insert(ui.ctx().content_rect());

        let mut state = State::load(ui.ctx(), self.id);

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
            let style = &self.dock_state.style;
            state.set_drag(source, hover, ui.ctx(), style);
            let tab_dst = self.show_drag_overlay(ui, &mut state);
            if ui.input(|i| i.pointer.primary_released()) {
                if let Some(destination) = tab_dst {
                    if let Some(drag_state) = state.drag.as_ref() {
                        match drag_state.drag.src {
                            TreeComponent::Tab(src_surf, src_node, src_tab) => {
                                self.dock_state.move_tab((src_surf, src_node, src_tab), destination);
                            }
                            _ => (),
                        }
                    };
                }
            }
        }

        if ui.input(|i| i.pointer.primary_released()) {
            state.reset_drag();
        }

        let style = &self.dock_state.style;
        let fade_surface = self.hovered_window_surface(&mut state, style.overlay.feel.fade_hold_time, ui.ctx());
        let fade_style = {
            fade_surface.is_some().then(|| {
                let mut fade_style = style.clone();
                fade_dock_style(&mut fade_style, style.overlay.surface_fade_opacity);
                (fade_style, style.overlay.surface_fade_opacity)
            })
        };

        for &surface_index in self.dock_state.valid_surface_indices().iter() {
            self.show_surface_inside(
                ui,
                &mut state,
                surface_index,
                fade_style.as_ref().map(|(style, factor)| {
                    (style, *factor, fade_surface.unwrap_or(SurfaceIndex::main()))
                })
            );
        }

        for removal in self.to_remove.drain(..).rev() {
            match removal {
                TabRemoval::Tab(surface, node, tab, ForcedRemoval(is_forced)) => {
                    if is_forced {
                        self.dock_state.remove_tab((surface, node, tab));
                    } else {
                        if let Some(_) = &mut self.dock_state[surface][node].get_leaf_mut() {
                            self.dock_state.remove_tab((surface, node, tab));
                        }
                    }
                },
                TabRemoval::Node(surface, node) => {
                    self.dock_state.remove_leaf((surface, node));
                },
                TabRemoval::Window(surface) => {
                    self.dock_state.remove_surface(surface);
                },
            }
        }

        for (surface_index, node_index, tab_index) in self.to_detach.drain(..).rev() {
            let mouse_pos = state.last_hover_pos;
            self.dock_state.detach_tab(
                (surface_index, node_index, tab_index),
                Rect::from_min_size(
                    mouse_pos.unwrap_or(Pos2::ZERO),
                    self.dock_state[surface_index][node_index].rect().map_or(Vec2::new(100.0, 150.0),
                        |rect| rect.size()
                    ))
            );
        }

        if let Some(focused) = self.new_focused {
            self.dock_state.set_focused_node_and_surface(focused);
        }

        state.store(ui.ctx(), self.id);
    }

    fn show_drag_overlay(&mut self, ui: &Ui, state: &mut State) -> Option<TabDestination> {
        let Some(drag_state) = state.drag.as_mut() else {
            return None;
        };

        let DockState {
            surfaces,
            ..
        } = self.dock_state;

        let deserted_node = {
            match (
                drag_state.drag.src.node_address(),
                drag_state.hover.dst.node_address(),
            ) {
                ((src_surf, Some(src_node)), (dst_surf, Some(dst_node))) => {
                    src_surf == dst_surf && src_node == dst_node && surfaces[src_surf][src_node].tabs_count() == 1
                }
                _ => false
            }
        };

        let allowed_splits = if drag_state.hover.dst.is_surface() || deserted_node {
            AllowedSplits::None
        } else {
            AllowedSplits::All
        };

        let allowed_in_window = match drag_state.drag.src {
            crate::drag::TreeComponent::Tab(_, _, _) => true,
            _ => false
        };

        if let Some(pointer) = state.last_hover_pos {
            drag_state.pointer = pointer;
        }

        let Some(window_bounds) = self.window_bounds else {
            return None;
        };
        drag_state.resolve(ui, &self.dock_state.style, allowed_splits, allowed_in_window, window_bounds)
    }

    fn hovered_window_surface(&self, state: &mut State, hold_time: f32, ctx: &Context) -> Option<SurfaceIndex> {
        if let Some(drag_state) = &state.drag {
            if drag_state.is_locked(ctx, &self.dock_state.style) {
                state.window_fade = Some((ctx.input(|i| i.time), drag_state.hover.dst.surface_address()));
            }
        }

        state.window_fade.and_then(|(time, surface)| {
            ctx.request_repaint();
            (hold_time > (ctx.input(|i| i.time) - time) as f32).then_some(surface)
        })
    }

    fn show_surface_inside(
        &mut self,
        ui: &mut Ui,
        state: &mut State,
        surf_index: SurfaceIndex,
        _fade_style: Option<(&Style, f32, SurfaceIndex)>
    ) {
        if surf_index.is_main() {
            self.show_root_surface_inside(ui, state);
        } else {
            self.show_window_surface(ui);
        }
    }

    fn show_root_surface_inside(&mut self, ui: &mut Ui, state: &mut State) {
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
                        })
                    );
                });
            }
            return;
        }

        self.show_nodes(ui, state, surf_index, None);
    }

    fn show_window_surface(&mut self, _ui: &mut Ui) {
        // todo!()
    }

    fn show_nodes(&mut self, ui: &mut Ui, state: &mut State, surf_index: SurfaceIndex, fade_style: Option<(&Style, f32)>) {
        let max_rect = self.allocate_area_for_root_node(ui, surf_index);
        for node_index in self.dock_state[surf_index].breadth_first_index_iter() {
            if self.dock_state[surf_index][node_index].is_parent() {
                self.compute_rect_size(ui, (surf_index, node_index), max_rect);
            }
        }

        for node_index in self.dock_state[surf_index].breadth_first_index_iter() {
            if self.dock_state[surf_index][node_index].is_leaf() {
                self.show_leaf(ui, state, (surf_index, node_index), fade_style)
            }
        }

        let fade_style = fade_style.map(|(style, _)| style);
        for node_index in self.dock_state[surf_index].breadth_first_index_iter() {
            if self.dock_state[surf_index][node_index].is_parent() {
                self.show_separator(ui, (surf_index, node_index), fade_style);
            }
        }
    }

    fn show_leaf(&mut self, ui: &mut Ui, state: &mut State, (surface_index, node_index): (SurfaceIndex, NodeIndex), fade_style: Option<(&Style, f32)>) {
        if !self.dock_state[surface_index][node_index].is_leaf() {
            return;
        }

        let Some(rect) = self.dock_state[surface_index][node_index].rect() else {
            return;
        };
        let ui = &mut ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::top_down_justified(Align::Min))
                .id_salt((node_index, "node")),
        );
        let spacing = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        ui.set_clip_rect(rect);

        if self.dock_state[surface_index][node_index].tabs_count() == 0 {
            return;
        }
        let tabbar_rect = self.show_bar(ui, state, (surface_index, node_index), fade_style.map(|(style, _)| style));
        self.show_body(ui, state, (surface_index, node_index), spacing, tabbar_rect, fade_style);
    }

    fn show_bar(&mut self, ui: &mut Ui, state: &mut State, (surface_index, node_index): (SurfaceIndex, NodeIndex), fade_style: Option<&Style>) -> Rect {
        let DockState {
            surfaces,
            style,
            ..
        } = &mut self.dock_state;
        let style = fade_style.unwrap_or_else(|| style);
        let (tabbar_outer_rect, tabbar_response) = ui.allocate_exact_size(
            vec2(ui.available_width(), style.tab_bar.height),
            Sense::hover()
        );
        ui.painter().rect_filled(
            tabbar_outer_rect,
            style.tab_bar.corner_radius,
            style.tab_bar.bg_fill,
        );

        let tabbar_outer_rect = tabbar_outer_rect - style.tab_bar.inner_margin;

        let available_width = tabbar_outer_rect.width();
        let scroll_bar_width = available_width;
        if available_width == 0.0 {
            return tabbar_outer_rect;
        }

        let Node::Leaf(leaf) = &mut surfaces[surface_index][node_index] else {
            return tabbar_outer_rect;
        };

        let (actual_width, tab_hovered) = {
            let tabbar_inner_rect = Rect::from_min_size(
                (tabbar_outer_rect.min - pos2(-leaf.scroll, 0.0)).to_pos2(),
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
            tabs_ui.set_clip_rect(clip_rect);

            let prefered_width = style
                .tab_bar
                .fill_tab_bar
                .then_some(available_width / (leaf.tabs.iter().len() as f32));

            let tab_hovered = self.show_tabs(tabs_ui, state, (surface_index, node_index), fade_style, tabbar_outer_rect, prefered_width);

            let px = ui.ctx().pixels_per_point().recip();
            let style = fade_style.unwrap_or_else(|| &self.dock_state.style);

            ui.painter().hline(tabs_ui.min_rect().right().min(clip_rect.right())..=tabbar_outer_rect.right(), tabbar_outer_rect.bottom() - px, (px, style.tab_bar.hline_color));

            (tabs_ui.min_rect().width(), tab_hovered)
        };

        self.show_bar_scroll(ui, state, (surface_index, node_index), actual_width, available_width, scroll_bar_width, &tabbar_response, tab_hovered, fade_style);

        tabbar_outer_rect
    }

    fn show_tabs(&mut self, tabs_ui: &mut Ui, state: &mut State, (surface_index, node_index): (SurfaceIndex, NodeIndex), fade_style: Option<&Style>, tabbar_outer_rect: Rect, preferred_width: Option<f32>) -> bool {
        let mut tab_hovered = false;

        if !matches!(self.dock_state[surface_index][node_index], Node::Leaf(_)) {
            return tab_hovered;
        }

        let focused = self.dock_state.focused_leaf();
        let tabs_len = self.dock_state[surface_index][node_index].tabs_count();

        for tab_index in 0..tabs_len {
            let tab_index = TabIndex(tab_index);
            let id = self.id.with((surface_index, "surface")).with((node_index, "node")).with((tab_index, "tab"));
            let is_being_dragged = tabs_ui.ctx().is_being_dragged(id) && tabs_ui.input(|i| i.pointer.is_decidedly_dragging()) && self.draggable_tabs;

            if is_being_dragged {
                tabs_ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
            }

            let DockState {
                style,
                surfaces,
                ..
            } = self.dock_state;

            let (is_active, label, tab_style) = {
                let Node::Leaf(leaf) = &mut surfaces[surface_index][node_index] else {
                    continue;
                };
                let style = fade_style.unwrap_or_else(|| style);
                (
                    leaf.active == tab_index || is_being_dragged,
                    leaf.tabs[tab_index.0].title(),
                    style.tab.clone(),
                )
            };

            let (response, title_id) = if is_being_dragged {
                let layer_id = LayerId::new(egui::Order::Tooltip, id);
                let response = tabs_ui
                    .scope_builder(UiBuilder::new().layer_id(layer_id), |ui| {
                        self.show_tab_title(
                            ui,
                            &tab_style,
                            id,
                            label,
                            is_active && Some((surface_index, node_index)) == focused,
                            is_active,
                            is_being_dragged,
                            preferred_width,
                        )
                    })
                    .response;
                let title_id = response.id;

                let response = tabs_ui.interact(response.rect, id.with("dragged"), Sense::click_and_drag());

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
                                    rect: self.dock_state[surface_index][node_index].rect().unwrap()
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
                let response = self.show_tab_title(
                    tabs_ui,
                    &tab_style,
                    id,
                    label,
                    is_active && Some((surface_index, node_index)) == focused,
                    is_active,
                    is_being_dragged,
                    preferred_width,
                );
                let title_id = response.id;
                let is_lonely_tab = self.dock_state[surface_index].tabs_count() == 1;

                if self.show_tab_context_menus {
                    let eject_button = Button::new(&self.dock_state.translations.tab_context_menu.eject_button);
                    let close_button = Button::new(&self.dock_state.translations.tab_context_menu.close_button);

                    let Node::Leaf(leaf) = &mut self.dock_state[surface_index][node_index] else {
                        continue;
                    };
                    response.context_menu(|ui| {
                        let tab = &mut leaf.tabs[tab_index.0];
                        tab.context_menu(ui);
                        if (surface_index.is_main() || !is_lonely_tab) && ui.add(eject_button).clicked() {
                            self.to_detach.push((surface_index, node_index, tab_index));
                            ui.close();
                        }
                        if ui.add(close_button).clicked() {
                            self.to_remove.push(TabRemoval::Tab(surface_index, node_index, tab_index, ForcedRemoval(false)));
                            ui.close();
                        }
                    });
                }

                if let Some(pos) = state.last_hover_pos {
                    if state.drag_start.is_some() && response.rect.contains(pos) {
                        self.tab_hover_rect = Some((response.rect, tab_index));
                    }
                }

                (response, title_id)
            };

            if response.hovered() {
                tab_hovered = true;
            }

            if !is_active || tab_style.hline_below_active_tab_name {
                let px = tabs_ui.ctx().pixels_per_point().recip();
                let style = fade_style.unwrap_or_else(|| &self.dock_state.style);
                tabs_ui.painter().hline(response.rect.x_range(), tabbar_outer_rect.bottom() - px, (px, style.tab_bar.hline_color));
            }

            if response.clicked()
                || (tabs_ui.memory(|m| m.has_focus(title_id))
                    && tabs_ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space))
                ) {
                let Node::Leaf(leaf) = &mut self.dock_state[surface_index][node_index] else {
                    continue;
                };
                leaf.active = tab_index;
                self.new_focused = Some((surface_index, node_index));
            }
        }

        tab_hovered
    }

    fn show_tab_title(
        &mut self,
        ui: &mut Ui,
        tab_style: &TabStyle,
        id: Id,
        label: WidgetText,
        focused: bool,
        active: bool,
        is_being_dragged: bool,
        preferred_width: Option<f32>,
    ) -> Response {
        let galley = label.into_galley(ui, None, f32::INFINITY, TextStyle::Button);
        let x_spacing = 8.0;
        let text_width = galley.size().x + 2.0 * x_spacing;

        let minimum_width = tab_style.minimum_width.unwrap_or(0.0).at_least(text_width);
        let tab_width = preferred_width.unwrap_or(0.0).at_least(minimum_width);

        let (_, tab_rect) = ui.allocate_space(vec2(tab_width, ui.available_height()));
        let mut response = ui.interact(tab_rect, id, Sense::click_and_drag());
        if ui.ctx().dragged_id().is_none() && self.draggable_tabs {
            response = response.on_hover_cursor(egui::CursorIcon::Grab);
        }

        let tab_style = if focused || is_being_dragged {
            &tab_style.focused
        } else if active {
            &tab_style.active
        } else if response.hovered() {
            &tab_style.hovered
        } else {
            &tab_style.inactive
        };

        ui.painter().rect_filled(tab_rect, tab_style.corner_radius, tab_style.bg_fill);
        let stroke_rect = rect_stroke_box(tab_rect, 1.0);
        ui.painter().rect_stroke(stroke_rect, tab_style.corner_radius, Stroke::new(1.0, tab_style.outline_color), egui::StrokeKind::Inside);
        ui.painter().hline(
            RangeInclusive::new(
                stroke_rect.max.x,
                stroke_rect.min.x
            ),
            stroke_rect.top(),
            Stroke::new(tab_style.corner_radius.ne.into(), tab_style.headerline_color),
        );

        let mut text_rect = tab_rect;
        text_rect.set_width(text_rect.width());
        let text_pos = {
            let pos = Align2::CENTER_CENTER.pos_in_rect(&text_rect.shrink2(vec2(x_spacing, 0.0)));
            pos - galley.size() / 2.0
        };

        ui.painter().add(TextShape::new(text_pos, galley, tab_style.text_color));

        response
    }

    fn show_bar_scroll(
        &mut self,
        ui: &mut Ui,
        state: &State,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        actual_width: f32,
        available_width: f32,
        scroll_bar_width: f32,
        tabbar_response: &Response,
        tab_hovered: bool,
        fade_style: Option<&Style>
    ) {
        if available_width <= 0.0 {
            return;
        }

        let DockState {
            style,
            surfaces,
            ..
        } = self.dock_state;

        let Node::Leaf(leaf) = &mut surfaces[surface_index][node_index] else {
            return;
        };
        let overflow = (actual_width - available_width).at_least(0.0);
        let style = fade_style.unwrap_or_else(|| style);

        if overflow > 1.0 {
            if style.tab_bar.show_scroll_bar_on_overflow {
                let bar_height = 7.5;
                let (scroll_bar_rect, _scroll_bar_response) = ui.allocate_exact_size(
                    vec2(scroll_bar_width, bar_height),
                    Sense::click_and_drag()
                );

                let overflow_ratio = actual_width / available_width;
                let scroll_ratio = -leaf.scroll / overflow;

                let scroll_bar_handle_size = overflow_ratio.recip() * scroll_bar_rect.width();
                let scroll_bar_handle_start = lerp(scroll_bar_rect.left()..=scroll_bar_rect.right() - scroll_bar_handle_size, scroll_ratio);
                let scroll_bar_handle_rect = Rect::from_min_size(pos2(scroll_bar_handle_start, scroll_bar_rect.min.y), vec2(scroll_bar_handle_size, bar_height));

                let scroll_bar_handle_response = ui.interact(scroll_bar_handle_rect, self.id.with((node_index, "node")), Sense::drag());

                let points_to_scroll_coefficient = overflow / (scroll_bar_handle_rect.width() - scroll_bar_handle_size);

                leaf.scroll -= scroll_bar_handle_response.drag_delta().x * points_to_scroll_coefficient;

                if let Some(pos) = state.last_hover_pos {
                    if scroll_bar_rect.contains(pos) {
                        leaf.scroll += ui.input(|i| i.smooth_scroll_delta.y + i.smooth_scroll_delta.x) * points_to_scroll_coefficient;
                    }
                }

                ui.painter().rect_filled(scroll_bar_rect, 0.0, ui.visuals().extreme_bg_color);

                ui.painter().rect_filled(scroll_bar_handle_rect, bar_height / 2.0, ui.visuals().widgets.style(&scroll_bar_handle_response).bg_fill);
            }

            if tabbar_response.hovered() || tab_hovered {
                leaf.scroll += ui.input(|i| i.smooth_scroll_delta.y + i.smooth_scroll_delta.x);
            }
        }

        leaf.scroll = leaf.scroll.clamp(-overflow, 0.0);
    }

    fn show_body(&mut self, ui: &mut Ui, state: &State, (surface_index, node_index): (SurfaceIndex, NodeIndex), spacing: Vec2, tabbar_rect: Rect, fade_style: Option<(&Style, f32)>) {
        let (body_rect, _body_response) = ui.allocate_exact_size(ui.available_size_before_wrap(), Sense::hover());

        let DockState {
            style,
            surfaces,
            ..
        } = self.dock_state;

        let Node::Leaf(leaf) = &mut surfaces[surface_index][node_index] else {
            return;
        };
        let LeafNode {
            rect,
            viewport,
            tabs,
            active,
            ..
        } = leaf;
        if let Some(tab) = tabs.get_mut(active.0) {
            if *viewport != body_rect {
                *viewport = body_rect;
                tab.on_rect_changed();
            }

            if ui.input(|i| i.pointer.any_click()) {
                if let Some(pos) = state.last_hover_pos {
                    if body_rect.contains(pos) && Some(ui.layer_id()) == ui.ctx().layer_id_at(pos) {
                        self.new_focused = Some((surface_index, node_index));
                    }
                }
            }

            let (style, fade_factor) = fade_style.unwrap_or_else(|| (style, 1.0));
            let tabs_style = &style.tab;
            ui.painter().rect_filled(body_rect, tabs_style.tab_body.corner_radius, tabs_style.tab_body.bg_fill);

            let id = self.id.with(Id::new(tab.title().text()));
            ui.ctx().check_for_id_clash(id, body_rect, "a tab with id");
            let ui = &mut Ui::new(
                ui.ctx().clone(),
                id,
                UiBuilder::new().max_rect(body_rect).layer_id(ui.layer_id())
            );
            ui.set_clip_rect(Rect::from_min_max(ui.cursor().min, ui.clip_rect().max));

            ui.spacing_mut().item_spacing = spacing;

            let effective_stroke_width = (tabs_style.tab_body.stroke.width / 2.0).ceil() * 2.0;
            let tab_body_rect = Rect::from_min_max(ui.clip_rect().min - vec2(0.0, effective_stroke_width), ui.clip_rect().max);
            ui.painter().rect_stroke(rect_stroke_box(tab_body_rect, tabs_style.tab_body.stroke.width), tabs_style.tab_body.corner_radius, tabs_style.tab_body.stroke, egui::StrokeKind::Inside);

            ScrollArea::new(tab.scroll_bars()).show(ui, |ui| {
                Frame::new()
                    .inner_margin(tabs_style.tab_body.inner_margin)
                    .show(ui, |ui| {
                        if fade_factor != 1.0 {
                            fade_visuals(ui.visuals_mut(), fade_factor);
                        }
                        let available_rect = ui.available_rect_before_wrap();
                        ui.expand_to_include_rect(available_rect);
                        tab.ui(ui);
                    });
            });
        }

        if let Some(pointer) = state.last_hover_pos {
            let rect = rect.to_owned();

            if state.drag_start.is_some() && rect.contains(pointer) {
                let on_title_bar = tabbar_rect.contains(pointer);
                let (dst, tab) = {
                    match self.tab_hover_rect {
                        Some((rect, tab_index)) => (
                            TreeComponent::Tab(surface_index, node_index, tab_index),
                            Some(rect),
                        ),
                        None => (
                            TreeComponent::Node(surface_index, node_index),
                            on_title_bar.then_some(tabbar_rect)
                        ),
                    }
                };

                ui.memory_mut(|mem| {
                    mem.data.insert_temp(self.id.with("hover_data"), Some(HoverData { rect, dst, tab }))
                });
            }
        }
    }

    fn show_separator(&mut self, ui: &mut Ui, (surface_index, node_index): (SurfaceIndex, NodeIndex), fade_style: Option<&Style>) {
        let DockState {
            style,
            surfaces,
            ..
        } = self.dock_state;

        if !surfaces[surface_index][node_index].is_parent() {
            return;
        }

        let style = fade_style.unwrap_or_else(|| style);
        let pixels_per_point = ui.ctx().pixels_per_point();

        duplicate! {
            [
                orientation    dim_point    dim_size;
                [Horizontal]   [x]          [width];
                [Vertical]     [y]          [height];
            ]
            if let Node::orientation(split) = &mut surfaces[surface_index][node_index] {
                let rect = split.rect;
                let mut separator = rect;

                let midpoint = rect.min.dim_point + rect.dim_size() * split.fraction.value();
                separator.min.dim_point = midpoint - style.separator.width * 0.5;
                separator.max.dim_point = midpoint + style.separator.width * 0.5;

                let mut expand = Vec2::ZERO;
                expand.dim_point += style.separator.extra_interact_width / 2.0;
                let interact_rect = separator.expand2(expand);

                let response = ui.allocate_rect(interact_rect, Sense::click_and_drag()).on_hover_and_drag_cursor(paste! { CursorIcon::[<Resize orientation>] });

                let should_respond_to_arrow_keys = ui.input(|i| i.modifiers.command || i.modifiers.shift);

                if response.has_focus() {
                    ui.memory_mut(|m| m.set_focus_lock_filter(response.id, EventFilter {
                        horizontal_arrows: should_respond_to_arrow_keys,
                        vertical_arrows: should_respond_to_arrow_keys,
                        tab: false,
                        escape: false
                    }));
                }

                let arrow_key_offset = if response.has_focus() && should_respond_to_arrow_keys {
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                        Some(egui::vec2(0.0, -16.0))
                    } else if ui.input(|i| i.key_released(egui::Key::ArrowDown)) {
                        Some(egui::vec2(0.0, 16.0))
                    } else if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                        Some(egui::vec2(-16.0, 0.0))
                    } else if ui.input(|i| i.key_pressed(egui::Key::AltRight)) {
                        Some(egui::vec2(16.0, 0.0))
                    } else {
                        None
                    }
                } else {
                    None
                };

                let midpoint = rect.min.dim_point + rect.dim_size() * split.fraction.value();
                separator.min.dim_point = map_to_pixel(midpoint - style.separator.width * 0.5, pixels_per_point, f32::round);
                separator.max.dim_point = map_to_pixel(midpoint + style.separator.width * 0.5, pixels_per_point, f32::round);

                let color = if response.dragged() {
                    style.separator.color_dragged
                } else if response.hovered() {
                    style.separator.color_hovered
                } else {
                    style.separator.color_idle
                };

                ui.painter().rect_filled(separator, CornerRadius::ZERO, color);

                let range = rect.max.dim_point - rect.min.dim_point;
                if range > 0.0 {
                    let min = (style.separator.extra / range).min(1.0);
                    let max = 1.0 - min;
                    let (min, max) = (min.min(max), max.max(min));
                    let delta = arrow_key_offset.unwrap_or(response.drag_delta()).dim_point;
                    split.fraction = (split.fraction.value() + delta / range).clamp(min, max).into();
                }

                if response.double_clicked() {
                    split.fraction = 0.5.into();
                }
            }
        }
    }

    fn allocate_area_for_root_node(&mut self, ui: &mut Ui, surface: SurfaceIndex) -> Rect {
        let style = &self.dock_state.style;
        let mut rect = ui.available_rect_before_wrap();

        if let Some(margin) = style.dock_area_padding {
            rect.min += margin.left_top();
            rect.max -= margin.right_bottom();
        }

        ui.painter().rect_stroke(
            rect,
            style.main_surface_border_rounding,
            style.main_surface_border_stroke,
            egui::StrokeKind::Inside
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

    fn compute_rect_size(&mut self, ui: &Ui, (surfce_index, node_index): (SurfaceIndex, NodeIndex), max_rect: Rect) {
        let DockState {
            style,
            surfaces,
            ..
        } = self.dock_state;

        if !surfaces[surfce_index][node_index].is_parent() {
            return;
        }

        let pixels_per_point = ui.ctx().pixels_per_point();

        duplicate! {
            [
                orientation    dim_point    dim_size    left_of    right_of;
                [Horizontal]   [x]          [width]     [left_of]  [right_of];
                [Vertical]     [y]          [height]    [above]      [below];
            ]
            if let Node::orientation(split) = &mut surfaces[surfce_index][node_index] {
                let rect = split.rect;
                let rect = expand_to_pixel(rect, pixels_per_point);

                let fraction: f32 = split.fraction.into();
                let midpoint = rect.min.dim_point + rect.dim_size() * fraction;
                let left_separator_border = map_to_pixel(
                    midpoint - style.separator.width * 0.5,
                    pixels_per_point,
                    f32::round
                );
                let right_separator_border = map_to_pixel(
                    midpoint - style.separator.width * 0.5,
                    pixels_per_point,
                    f32::round
                );

                paste! {
                    let left = rect.intersect(Rect::[<everything_ left_of>](left_separator_border)).intersect(max_rect);
                    let right = rect.intersect(Rect::[<everything_ right_of>](right_separator_border)).intersect(max_rect);
                }

                surfaces[surfce_index][node_index.left()].set_rect(left);
                surfaces[surfce_index][node_index.right()].set_rect(right);
            }
        }
    }
}

fn fade_dock_style(style: &mut Style, factor: f32) {
    style.main_surface_border_stroke.color = style
        .main_surface_border_stroke.color
        .linear_multiply(factor);
    fade_tab_style(&mut style.tab, factor);
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

fn expand_to_pixel(mut rect: Rect, ppi: f32) -> Rect {
    rect.min = map_to_pixel_pos(rect.min, ppi, f32::floor);
    rect.max = map_to_pixel_pos(rect.max, ppi, f32::ceil);
    rect
}

fn map_to_pixel_pos(mut pos: Pos2, ppi: f32, map: fn(f32) -> f32) -> Pos2 {
    pos.x = map_to_pixel(pos.x, ppi, map);
    pos.y = map_to_pixel(pos.y, ppi, map);
    pos
}

fn map_to_pixel(point: f32, ppi: f32, map: fn(f32) -> f32) -> f32 {
    map(point * ppi) / ppi
}

fn rect_stroke_box(rect: Rect, width: f32) -> Rect {
    rect.expand(-f32::ceil(width / 2.0))
}

fn fade_visuals(visuals: &mut Visuals, factor: f32) {
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
