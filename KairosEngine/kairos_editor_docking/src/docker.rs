//! Dock layout state and the single drawing/interaction widget.
//!
//! `DockState<Id>` stores only layout data: surfaces, a binary split tree,
//! tab ids/order, active tab indexes and split fractions. `Id` is a stable
//! app-defined tab identity (`TabId`), so the app can serialize the layout
//! separately from the tab objects.
//!
//! [`Docker`] is the one entry point:
//! `Docker::new(id, &mut state).show(ui, &mut viewer)`. It never resolves a
//! tab object itself; titles, tab icons, bodies, close policy and window policy
//! come from the app's [`TabViewer`]. Close/focus requests are reported back
//! through the viewer instead of mutating the app's open-panel state here.

use std::ops::{Index, IndexMut};
use std::sync::Arc;

use duplicate::duplicate;
use egui::{
    Align, Button, CentralPanel, Color32, Context, CornerRadius, CursorIcon, EventFilter, Frame,
    Galley, Id, Image, ImageSource, LayerId, Layout, NumExt, Pos2, Rect, Response, ScrollArea,
    Sense, Stroke, StrokeKind, TextStyle, Ui, UiBuilder, Vec2, Visuals, WidgetText,
    emath::TSTransform,
    epaint::TextShape,
    lerp, pos2,
    style::{WidgetVisuals, Widgets},
    vec2,
};
use paste::paste;

use crate::{
    drag::{DragData, DragDropState, DragVisual, HoverData, TreeComponent, constrain_rect_to_area},
    split::{Fraction, Split},
    styles::{
        Style, TabBarStyle, TabBodyStyle, TabInteractionStyle, TabSeparatorStyle, TabStyle,
        WindowStyle,
    },
    surface::{Surface, SurfaceIndex, WindowState},
    translations::Translations,
    tree::{Node, NodeIndex, TabIndex, Tree},
    viewer::{CloseResponse, TabId, TabViewer},
};

#[derive(Clone, Copy, Debug)]
pub enum TabInsert {
    Split(Split),
    Insert(TabIndex),
    Append,
}

#[derive(Clone, Copy, Debug)]
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
        self.drag
            .as_ref()
            .is_some_and(|drag_drop_state| drag_drop_state.is_locked(ctx, style))
    }

    fn set_drag(&mut self, drag: DragData, drop: HoverData, ctx: &Context, style: &Style) {
        if !self.is_drag_locked(ctx, style) {
            self.drag = Some(DragDropState {
                hover: drop,
                drag,
                pointer: ctx.pointer_hover_pos().unwrap_or(Pos2::ZERO),
                locked: None,
                visual: DragVisual::None,
            })
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

pub struct DockState<T: TabId> {
    surfaces: Vec<Surface<T>>,
    focused_surface: Option<SurfaceIndex>,
    pub translations: Translations,
    pub style: Style,
}

impl<T: TabId> Default for DockState<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: TabId> DockState<T> {
    pub fn new() -> Self {
        Self {
            surfaces: vec![Surface::Main(Tree::new(vec![]))],
            focused_surface: None,
            translations: Translations::english(),
            style: Style::default(),
        }
    }

    pub fn move_tab(
        &mut self,
        (src_surface, src_node, src_tab): (SurfaceIndex, NodeIndex, TabIndex),
        dst_tab: impl Into<TabDestination>,
    ) {
        match dst_tab.into() {
            TabDestination::Window(position) => {
                self.detach_tab((src_surface, src_node, src_tab), position);
                return;
            }
            TabDestination::Node(dst_surface, dst_node, dst_tab) => {
                if src_surface == dst_surface
                    && src_node == dst_node
                    && self[src_surface][src_node].tabs_count() == 1
                {
                    return;
                }

                if let Some(tab) = self[src_surface][src_node].remove_tab(src_tab) {
                    match dst_tab {
                        TabInsert::Split(split) => {
                            self[dst_surface].split(
                                dst_node,
                                split,
                                Fraction::new(0.5),
                                Node::leaf(tab),
                            );
                        }
                        TabInsert::Insert(index) => {
                            self[dst_surface][dst_node].insert_tab(index, tab)
                        }
                        TabInsert::Append => self[dst_surface][dst_node].append_tab(tab),
                    }
                }
            }
            TabDestination::EmptySurface(dst_surface) => {
                if !self[dst_surface].is_empty() {
                    return;
                }
                if let Some(tab) = self[src_surface][src_node].remove_tab(src_tab) {
                    self[dst_surface] = Tree::new(vec![tab]);
                }
            }
        }
        if self[src_surface][src_node].is_leaf() && self[src_surface][src_node].tabs_count() == 0 {
            self[src_surface].remove_leaf(src_node);
        }
        if self[src_surface].is_empty() && !src_surface.is_main() {
            self.remove_surface(src_surface);
        }
    }

    pub fn detach_tab(
        &mut self,
        (src_surface, src_node, src_tab): (SurfaceIndex, NodeIndex, TabIndex),
        window_rect: Rect,
    ) -> Option<SurfaceIndex> {
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

    fn add_window(&mut self, tabs: Vec<T>) -> SurfaceIndex {
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

    fn remove_surface(&mut self, surface: SurfaceIndex) -> Option<Surface<T>> {
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

    pub fn main_surface(&self) -> &Tree<T> {
        &self[SurfaceIndex::main()]
    }

    pub fn main_surface_mut(&mut self) -> &mut Tree<T> {
        &mut self[SurfaceIndex::main()]
    }

    pub fn focused_leaf(&self) -> Option<(SurfaceIndex, NodeIndex)> {
        let surface = self.focused_surface?;
        self[surface].focused_leaf().map(|leaf| (surface, leaf))
    }

    pub fn remove_tab(
        &mut self,
        (surface_index, node_index, tab_index): (SurfaceIndex, NodeIndex, TabIndex),
    ) -> Option<T> {
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

    pub fn set_focused_node_and_surface(
        &mut self,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
    ) {
        if self.is_surface_valid(surface_index) && node_index.0 < self[surface_index].len() {
            if self[surface_index][node_index].is_leaf() {
                self.focused_surface = Some(surface_index);
                self[surface_index].set_focused_node(node_index);
                return;
            }
        }
        self.focused_surface = None;
    }

    /// All tab ids currently present in the dock, in layout order.
    pub fn tabs(&self) -> Vec<T> {
        self.surfaces
            .iter()
            .flat_map(|surface| surface.get_tree().nodes.iter())
            .flat_map(|node| match node {
                Node::Leaf(leaf) => leaf.tabs.clone(),
                _ => Vec::new(),
            })
            .collect()
    }

    pub fn contains_tab(&self, id: T) -> bool {
        self.tabs().contains(&id)
    }

    /// Append a tab to the focused leaf, falling back to the first leaf of the
    /// main surface. Used when the app opens a panel that has no dock tab yet.
    pub fn append_tab(&mut self, id: T) {
        let main = SurfaceIndex::main();
        if self[main].is_empty() {
            self[main] = Tree::new(vec![id]);
            return;
        }
        if let Some((surface, node)) = self.focused_leaf() {
            if let Some(leaf) = self[surface][node].get_leaf_mut() {
                leaf.append_tab(id);
                return;
            }
        }
        let node = {
            let tree = &self[main];
            tree.breadth_first_index_iter()
                .find(|node| tree[*node].is_leaf())
        };
        if let Some(node) = node {
            if let Some(leaf) = self[main][node].get_leaf_mut() {
                leaf.append_tab(id);
            }
        }
    }

    /// Remove the tab with `id` from whichever leaf holds it.
    pub fn remove_tab_by_id(&mut self, id: T) -> bool {
        for surface in self.valid_surface_indices().iter().copied() {
            let node = {
                let tree = &self[surface];
                tree.breadth_first_index_iter().find(|node| {
                    tree[*node]
                        .get_leaf()
                        .is_some_and(|leaf| leaf.tabs.contains(&id))
                })
            };
            let Some(node) = node else { continue };
            let tab_index = self[surface][node]
                .get_leaf()
                .and_then(|leaf| leaf.tabs.iter().position(|tab| *tab == id));
            if let Some(tab_index) = tab_index {
                self.remove_tab((surface, node, TabIndex(tab_index)));
                return true;
            }
        }
        false
    }

    /// Id of the active tab of the focused leaf, if any.
    pub fn focused_tab(&self) -> Option<T> {
        let (surface, node) = self.focused_leaf()?;
        let leaf = self[surface][node].get_leaf()?;
        leaf.tabs.get(leaf.active.0).copied()
    }
}

impl<T: TabId> Index<SurfaceIndex> for DockState<T> {
    type Output = Tree<T>;

    fn index(&self, index: SurfaceIndex) -> &Self::Output {
        &self.surfaces[index.0].get_tree()
    }
}

impl<T: TabId> IndexMut<SurfaceIndex> for DockState<T> {
    fn index_mut(&mut self, index: SurfaceIndex) -> &mut Self::Output {
        self.surfaces[index.0].get_tree_mut()
    }
}

impl<T: TabId> Index<SurfaceIndex> for Vec<Surface<T>> {
    type Output = Tree<T>;

    fn index(&self, index: SurfaceIndex) -> &Self::Output {
        self[index.0].get_tree()
    }
}

impl<T: TabId> IndexMut<SurfaceIndex> for Vec<Surface<T>> {
    fn index_mut(&mut self, index: SurfaceIndex) -> &mut Self::Output {
        self[index.0].get_tree_mut()
    }
}

pub struct Docker<'tree, T: TabId> {
    id: Id,
    dock_state: &'tree mut DockState<T>,
    window_bounds: Option<Rect>,
    draggable_tabs: bool,
    show_tab_context_menus: bool,

    to_remove: Vec<(SurfaceIndex, NodeIndex, TabIndex)>,
    to_detach: Vec<(SurfaceIndex, NodeIndex, TabIndex)>,
    new_focused: Option<(SurfaceIndex, NodeIndex)>,
    tab_hover_rect: Option<(Rect, TabIndex)>,
}

impl<'tree, T: TabId> Docker<'tree, T> {
    pub fn new(id: impl Into<Id>, state: &'tree mut DockState<T>) -> Docker<'tree, T> {
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

impl<T: TabId> Docker<'_, T> {
    pub fn show(self, ui: &mut Ui, viewer: &mut impl TabViewer<Id = T>) {
        CentralPanel::default()
            .frame(
                Frame::central_panel(&ui.global_style())
                    .inner_margin(0.)
                    .fill(Color32::TRANSPARENT),
            )
            .show(ui, |ui| {
                self.show_inside(ui, viewer);
            });
    }

    fn show_inside(mut self, ui: &mut Ui, viewer: &mut impl TabViewer<Id = T>) {
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

        // While dragging a tab, an empty main-surface area (e.g. the hole left
        // after a tab was detached) has no leaf hover data. Fall back to the
        // main surface's focused/first leaf so dropping inside the dock docks
        // the tab back instead of creating another window.
        let hover_data =
            hover_data.or_else(|| self.fallback_main_hover(drag_data, &state, ui.max_rect()));

        let dock_bounds = ui.max_rect();

        if let Some(source) = drag_data {
            if let Some(hover) = hover_data {
                let style = &self.dock_state.style;
                state.set_drag(source, hover, ui.ctx(), style);
                let tab_dst = self.show_drag_overlay(ui, &mut state, viewer);
                if state.drag.is_some() && ui.input(|i| i.pointer.primary_down()) {
                    self.paint_drag_visual(ui, &state, viewer);
                }
                if ui.input(|i| i.pointer.primary_released()) {
                    if let Some(destination) = tab_dst {
                        if let Some(drag_state) = state.drag.as_ref() {
                            match drag_state.drag.src {
                                TreeComponent::Tab(src_surf, src_node, src_tab) => {
                                    self.dock_state
                                        .move_tab((src_surf, src_node, src_tab), destination);
                                }
                                _ => (),
                            }
                        };
                    }
                }
            } else {
                // The pointer left the dock area: keep showing the window
                // preview and, on release outside, detach the tab into a
                // floating window (Unity-like drag-out).
                let pointer = state
                    .last_hover_pos
                    .or_else(|| ui.input(|i| i.pointer.hover_pos()));
                if let Some(pointer) = pointer {
                    if ui.input(|i| i.pointer.primary_down()) {
                        self.paint_window_preview_at(ui, source, pointer, viewer);
                    }
                    if ui.input(|i| i.pointer.primary_released()) && !dock_bounds.contains(pointer)
                    {
                        if let TreeComponent::Tab(surface, node, tab) = source.src {
                            let allowed = self.dock_state[surface][node]
                                .get_leaf()
                                .and_then(|leaf| leaf.tabs.get(tab.0).copied())
                                .is_some_and(|id| viewer.allowed_in_windows(id));
                            if allowed {
                                self.to_detach.push((surface, node, tab));
                            }
                        }
                    }
                }
            }
        }

        if ui.input(|i| i.pointer.primary_released()) {
            state.reset_drag();
        }

        let style = &self.dock_state.style;
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
                ui,
                &mut state,
                surface_index,
                fade_style.as_ref().map(|(style, factor)| {
                    (style, *factor, fade_surface.unwrap_or(SurfaceIndex::main()))
                }),
                viewer,
            );
        }

        for (surface, node, tab) in self.to_remove.drain(..).rev() {
            let Some(tab_id) = self.dock_state[surface][node]
                .get_leaf()
                .and_then(|leaf| leaf.tabs.get(tab.0).copied())
            else {
                continue;
            };
            match viewer.on_close(tab_id) {
                CloseResponse::Close => {
                    self.dock_state.remove_tab((surface, node, tab));
                }
                CloseResponse::Focus => {
                    if let Some(leaf) = self.dock_state[surface][node].get_leaf_mut() {
                        leaf.active = tab;
                    }
                    self.new_focused = Some((surface, node));
                }
                CloseResponse::Ignore => {}
            }
        }

        for (surface_index, node_index, tab_index) in self.to_detach.drain(..).rev() {
            // Detach at the cursor, centred on it: the drag preview is centred
            // on the cursor and the created window would otherwise jump by half
            // its size when the mouse is released.
            let mouse_pos = state.last_hover_pos.unwrap_or(Pos2::ZERO);
            let leaf_size = self.dock_state[surface_index][node_index]
                .rect()
                .map_or(Vec2::new(100.0, 150.0), |rect| rect.size());
            let scale = if surface_index.is_main() { 0.8 } else { 1.0 };
            let window_rect = Rect::from_min_size(mouse_pos - leaf_size * scale * 0.5, leaf_size);
            self.dock_state
                .detach_tab((surface_index, node_index, tab_index), window_rect);
        }

        if let Some(focused) = self.new_focused {
            self.dock_state.set_focused_node_and_surface(focused);
        }

        state.store(ui.ctx(), self.id);
    }

    fn show_drag_overlay(
        &mut self,
        ui: &Ui,
        state: &mut State,
        viewer: &mut impl TabViewer<Id = T>,
    ) -> Option<TabDestination> {
        let Some(drag_state) = state.drag.as_mut() else {
            return None;
        };

        let DockState { surfaces, .. } = self.dock_state;

        let deserted_node = {
            match (
                drag_state.drag.src.node_address(),
                drag_state.hover.dst.node_address(),
            ) {
                ((src_surf, Some(src_node)), (dst_surf, Some(dst_node))) => {
                    src_surf == dst_surf
                        && src_node == dst_node
                        && surfaces[src_surf][src_node].tabs_count() == 1
                }
                _ => false,
            }
        };

        let allowed_splits = if drag_state.hover.dst.is_surface() || deserted_node {
            AllowedSplits::None
        } else {
            AllowedSplits::All
        };

        let allowed_in_window = match drag_state.drag.src {
            TreeComponent::Tab(surface, node, tab) => self.dock_state[surface][node]
                .get_leaf()
                .and_then(|leaf| leaf.tabs.get(tab.0).copied())
                .is_some_and(|id| viewer.allowed_in_windows(id)),
            _ => false,
        };

        if let Some(pointer) = state.last_hover_pos {
            drag_state.pointer = pointer;
        }

        let Some(window_bounds) = self.window_bounds else {
            return None;
        };
        drag_state.resolve(
            ui,
            &self.dock_state.style,
            allowed_splits,
            allowed_in_window,
            window_bounds,
        )
    }

    /// Paint the drag affordance chosen by `DragDropState::resolve`:
    ///
    /// * tab-bar insertion -> a tab ghost at the insertion slot;
    /// * split -> a preview panel filling the prospective split region;
    /// * window drop -> a floating window ghost centred on the cursor.
    ///
    /// None of these carry window controls: they are previews, not windows.
    fn paint_drag_visual(&self, ui: &Ui, state: &State, viewer: &mut impl TabViewer<Id = T>) {
        let Some(drag_state) = state.drag.as_ref() else {
            return;
        };
        let TreeComponent::Tab(surface, node, tab) = drag_state.drag.src else {
            return;
        };
        let Some((title, icon)) = self.tab_title_and_icon(viewer, (surface, node, tab)) else {
            return;
        };

        let style = &self.dock_state.style;
        match drag_state.visual {
            DragVisual::None => {}
            // Tab-bar insertion already has a live affordance: the grabbed tab
            // header follows the cursor in a tooltip layer (see `show_tabs`),
            // so do not paint a second ghost on top of it.
            DragVisual::Tab(_) => {}
            DragVisual::Panel(rect) => {
                paint_preview_panel(&self.drag_preview_ui(ui), rect, title, icon, style);
            }
            DragVisual::Window => {
                let pointer = state.last_hover_pos.unwrap_or(drag_state.pointer);
                paint_window_preview(
                    &self.drag_preview_ui(ui),
                    pointer,
                    drag_state.drag.rect.size(),
                    title,
                    icon,
                    style,
                    ui.ctx().content_rect(),
                );
            }
        }
    }

    /// Pointer left the dock area: the tab will be detached on release, so show
    /// the floating-window ghost centred on the cursor.
    fn paint_window_preview_at(
        &self,
        ui: &Ui,
        source: DragData,
        pointer: Pos2,
        viewer: &mut impl TabViewer<Id = T>,
    ) {
        let TreeComponent::Tab(surface, node, tab) = source.src else {
            return;
        };
        let Some((title, icon)) = self.tab_title_and_icon(viewer, (surface, node, tab)) else {
            return;
        };
        paint_window_preview(
            &self.drag_preview_ui(ui),
            pointer,
            source.rect.size(),
            title,
            icon,
            &self.dock_state.style,
            ui.ctx().content_rect(),
        );
    }

    /// Resolve a tab's title and icon once for the ghost painters.
    ///
    /// Both ghosts used to look the id up again and rebuild the title from
    /// `WidgetText::text()`, which dropped rich text and styling; they now share
    /// this lookup and pass the same title/icon a live tab would receive.
    fn tab_title_and_icon(
        &self,
        viewer: &mut impl TabViewer<Id = T>,
        (surface, node, tab): (SurfaceIndex, NodeIndex, TabIndex),
    ) -> Option<(WidgetText, Option<ImageSource<'static>>)> {
        let tab_id = self.dock_state[surface][node]
            .get_leaf()
            .and_then(|leaf| leaf.tabs.get(tab.0).copied())?;
        Some((viewer.title(tab_id), viewer.icon(tab_id)))
    }

    /// A `Ui` that paints into the tooltip layer shared by all drag ghosts.
    ///
    /// Ghosts need a real `Ui` (not just a `Painter`) so the shared header
    /// painter can load and paint the tab icon; it paints only and never takes
    /// part in layout.
    fn drag_preview_ui(&self, ui: &Ui) -> Ui {
        let bounds = ui.ctx().content_rect();
        let layer_id = LayerId::new(egui::Order::Tooltip, self.id.with("drag-preview"));
        let mut preview = Ui::new(
            ui.ctx().clone(),
            self.id.with("drag-preview"),
            UiBuilder::new().max_rect(bounds).layer_id(layer_id),
        );
        preview.set_clip_rect(bounds);
        preview
    }

    fn fallback_main_hover(
        &self,
        drag_data: Option<DragData>,
        state: &State,
        dock_bounds: Rect,
    ) -> Option<HoverData> {
        let source = drag_data?;
        if !matches!(source.src, TreeComponent::Tab(..)) {
            return None;
        }
        let pointer = state.last_hover_pos?;
        if !dock_bounds.contains(pointer) {
            return None;
        }

        let main = SurfaceIndex::main();
        let node = self.dock_state[main].focused_leaf().or_else(|| {
            self.dock_state[main]
                .breadth_first_index_iter()
                .find(|node| self.dock_state[main][*node].is_leaf())
        })?;
        let rect = self.dock_state[main][node].rect().unwrap_or(dock_bounds);
        Some(HoverData {
            rect,
            dst: TreeComponent::Node(main, node),
            tab: None,
            tab_bar: None,
        })
    }

    fn hovered_window_surface(
        &self,
        state: &mut State,
        hold_time: f32,
        ctx: &Context,
    ) -> Option<SurfaceIndex> {
        if let Some(drag_state) = &state.drag {
            if drag_state.is_locked(ctx, &self.dock_state.style) {
                state.window_fade = Some((
                    ctx.input(|i| i.time),
                    drag_state.hover.dst.surface_address(),
                ));
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
        _fade_style: Option<(&Style, f32, SurfaceIndex)>,
        viewer: &mut impl TabViewer<Id = T>,
    ) {
        if surf_index.is_main() {
            self.show_root_surface_inside(ui, state, viewer);
        } else {
            self.show_window_surface(ui, state, surf_index, viewer);
        }
    }

    fn show_root_surface_inside(
        &mut self,
        ui: &mut Ui,
        state: &mut State,
        viewer: &mut impl TabViewer<Id = T>,
    ) {
        let surf_index = SurfaceIndex::main();

        if self.dock_state.main_surface().is_empty() {
            let rect = ui.available_rect_before_wrap();
            let response = ui.allocate_rect(rect, Sense::hover());
            if response.contains_pointer() {
                self.store_hover_data(
                    ui,
                    HoverData {
                        rect,
                        dst: TreeComponent::Surface(surf_index),
                        tab: None,
                        tab_bar: None,
                    },
                );
            }
            return;
        }

        self.show_nodes(ui, state, surf_index, None, viewer);
    }

    fn show_window_surface(
        &mut self,
        ui: &mut Ui,
        state: &mut State,
        surf_index: SurfaceIndex,
        viewer: &mut impl TabViewer<Id = T>,
    ) {
        let bounds = self
            .window_bounds
            .unwrap_or_else(|| ui.ctx().content_rect());
        let window_style = self.dock_state.style.window.clone();

        let (position, size, minimized, maximized, pending_rect) =
            match self.dock_state.get_window_state_mut(surf_index) {
                Some(window_state) => (
                    window_state.position(),
                    window_state.size(),
                    window_state.is_minimized(),
                    window_state.is_maximized(),
                    window_state.take_pending_rect(),
                ),
                None => return,
            };

        let egui_style = ui.style().clone();
        let mut frame = egui::Frame::window(&egui_style);
        frame.inner_margin = egui::Margin::ZERO;
        frame.fill = window_style.bg_fill;
        frame.stroke = window_style.border;
        frame.corner_radius = egui::CornerRadius::ZERO;
        frame.shadow = window_style.shadow;

        let mut window = egui::Window::new("")
            .id(self.id.with(("window", surf_index.0)))
            .title_bar(false)
            .movable(false)
            .collapsible(false)
            .resizable([true, true])
            .frame(frame)
            .constrain_to(bounds);

        if let Some(pending_rect) = pending_rect {
            window = window.fixed_rect(pending_rect);
        } else if maximized {
            window = window.fixed_pos(bounds.min).fixed_size(bounds.size());
        } else {
            if let Some(position) = position {
                window = window.current_pos(position);
            }
            if minimized {
                let width = size.map_or(420.0, |size| size.x);
                window = window.fixed_size(vec2(width, window_style.title_height + 2.0));
            } else {
                window = window.default_size(size.unwrap_or(Vec2::new(420.0, 320.0)));
            }
        }

        let mut toggle_minimize = false;
        let mut toggle_maximize = false;
        let mut close_window = false;
        let mut drag_delta = Vec2::ZERO;

        let response = window.show(ui.ctx(), |ui| {
            let (bar_rect, _) = ui.allocate_exact_size(
                vec2(ui.available_width(), window_style.title_height),
                Sense::hover(),
            );
            ui.painter()
                .rect_filled(bar_rect, CornerRadius::ZERO, window_style.title_bg_fill);
            ui.painter().hline(
                bar_rect.x_range(),
                bar_rect.bottom() - 1.0,
                Stroke::new(1.0, window_style.border.color),
            );

            let controls = window_control_rects(bar_rect, &window_style);
            let drag_rect = Rect::from_min_max(
                pos2(controls.maximize.right() + 4.0, bar_rect.top()),
                bar_rect.max,
            );
            let drag_response = ui
                .interact(
                    drag_rect,
                    ui.id().with("window-drag"),
                    Sense::click_and_drag(),
                )
                .on_hover_cursor(CursorIcon::Grab);
            if drag_response.dragged() && !maximized {
                drag_delta = drag_response.drag_delta();
            }

            // All three traffic lights share one interact+paint path; only the
            // rect and the click effect differ.
            for (control, rect) in [
                (WindowControl::Minimize, controls.minimize),
                (WindowControl::Maximize, controls.maximize),
                (WindowControl::Close, controls.close),
            ] {
                let response = ui.interact(rect, ui.id().with(control), Sense::click());
                paint_window_control(
                    ui.painter(),
                    rect,
                    control,
                    &window_style,
                    response.hovered(),
                );
                if response.clicked() {
                    match control {
                        WindowControl::Minimize => toggle_minimize |= !maximized,
                        WindowControl::Maximize => toggle_maximize |= !minimized,
                        WindowControl::Close => close_window = true,
                    }
                }
            }

            if !minimized {
                self.show_nodes(ui, state, surf_index, None, viewer);
            }
        });

        let Some(response) = response else {
            return;
        };
        let rect = response.response.rect;

        if close_window {
            let removals = {
                let mut removals = Vec::new();
                for node in self.dock_state[surf_index].breadth_first_index_iter() {
                    if let Some(leaf) = self.dock_state[surf_index][node].get_leaf() {
                        for tab in 0..leaf.tabs.len() {
                            removals.push((surf_index, node, TabIndex(tab)));
                        }
                    }
                }
                removals
            };
            self.to_remove.extend(removals);
            return;
        }

        let Some(window_state) = self.dock_state.get_window_state_mut(surf_index) else {
            return;
        };

        if toggle_minimize {
            if window_state.is_minimized() {
                window_state.set_minimized(false);
                if let Some((position, size)) = window_state.restore_rect() {
                    window_state.set_position(position);
                    window_state.set_size(size);
                    window_state.set_pending_rect(Rect::from_min_size(position, size));
                    window_state.clear_restore();
                }
            } else {
                window_state.set_restore_rect(rect.min, rect.size());
                window_state.set_minimized(true);
                window_state.set_position(rect.min);
            }
        } else if toggle_maximize {
            if window_state.is_maximized() {
                window_state.set_maximized(false);
                if let Some((position, size)) = window_state.restore_rect() {
                    window_state.set_position(position);
                    window_state.set_size(size);
                    window_state.set_pending_rect(Rect::from_min_size(position, size));
                    window_state.clear_restore();
                }
            } else {
                window_state.set_restore_rect(rect.min, rect.size());
                window_state.set_maximized(true);
                window_state.set_minimized(false);
                window_state.set_position(bounds.min);
                window_state.set_size(bounds.size());
            }
        } else if !window_state.is_maximized() {
            if drag_delta != Vec2::ZERO {
                window_state.set_position(rect.min + drag_delta);
            } else if window_state.is_minimized() {
                window_state.set_position(rect.min);
            } else {
                window_state.set_rect(rect);
            }
        }
    }

    /// Park the pointer's current drop target for the next egui pass.
    ///
    /// Drag state is resolved before the leaves are drawn, so the hover target
    /// found by a leaf is stored in temporary memory and read back at the start
    /// of the following pass. Three code paths used to open-code this insert;
    /// they share it so the memory key and "replace, don't append" semantics
    /// cannot drift.
    fn store_hover_data(&self, ui: &Ui, hover: HoverData) {
        ui.memory_mut(|mem| {
            mem.data
                .insert_temp(self.id.with("hover_data"), Some(hover));
        });
    }

    fn show_nodes(
        &mut self,
        ui: &mut Ui,
        state: &mut State,
        surf_index: SurfaceIndex,
        fade_style: Option<(&Style, f32)>,
        viewer: &mut impl TabViewer<Id = T>,
    ) {
        let max_rect = self.allocate_area_for_root_node(ui, surf_index);
        for node_index in self.dock_state[surf_index].breadth_first_index_iter() {
            if self.dock_state[surf_index][node_index].is_parent() {
                self.compute_rect_size(ui, (surf_index, node_index), max_rect);
            }
        }

        for node_index in self.dock_state[surf_index].breadth_first_index_iter() {
            if self.dock_state[surf_index][node_index].is_leaf() {
                self.show_leaf(ui, state, (surf_index, node_index), fade_style, viewer)
            }
        }

        let fade_style = fade_style.map(|(style, _)| style);
        for node_index in self.dock_state[surf_index].breadth_first_index_iter() {
            if self.dock_state[surf_index][node_index].is_parent() {
                self.show_separator(ui, (surf_index, node_index), fade_style);
            }
        }
    }

    fn show_leaf(
        &mut self,
        ui: &mut Ui,
        state: &mut State,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        fade_style: Option<(&Style, f32)>,
        viewer: &mut impl TabViewer<Id = T>,
    ) {
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
            // An empty leaf is a valid drop target: keep its rect registered as
            // hover data so a tab dragged out of a floating window can return
            // to the main surface instead of becoming another window.
            let body_rect = ui.available_rect_before_wrap();
            let response = ui.allocate_rect(body_rect, Sense::hover());
            let bg = self.dock_state.style.tab.tab_body.bg_fill;
            ui.painter().rect_filled(body_rect, CornerRadius::ZERO, bg);
            if response.contains_pointer() {
                self.store_hover_data(
                    ui,
                    HoverData {
                        rect: body_rect,
                        dst: TreeComponent::Node(surface_index, node_index),
                        tab: None,
                        tab_bar: None,
                    },
                );
            }
            return;
        }
        let tabbar_rect = self.show_bar(
            ui,
            state,
            (surface_index, node_index),
            fade_style.map(|(style, _)| style),
            viewer,
        );
        self.show_body(
            ui,
            state,
            (surface_index, node_index),
            spacing,
            tabbar_rect,
            fade_style,
            viewer,
        );
    }

    fn show_bar(
        &mut self,
        ui: &mut Ui,
        state: &mut State,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        fade_style: Option<&Style>,
        viewer: &mut impl TabViewer<Id = T>,
    ) -> Rect {
        let DockState {
            surfaces, style, ..
        } = &mut self.dock_state;
        let style = fade_style.unwrap_or_else(|| style);
        let (tabbar_outer_rect, tabbar_response) = ui.allocate_exact_size(
            vec2(ui.available_width(), style.tab_bar.height),
            Sense::hover(),
        );
        paint_tab_bar(ui.painter(), tabbar_outer_rect, &style.tab_bar);

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

            let tab_hovered = self.show_tabs(
                tabs_ui,
                state,
                (surface_index, node_index),
                fade_style,
                tabbar_outer_rect,
                prefered_width,
                viewer,
            );

            // No divider line between the tab bar and the body: Unity merges
            // the focused tab into the panel body, so a black line would break
            // that continuous surface.
            (tabs_ui.min_rect().width(), tab_hovered)
        };

        self.show_bar_scroll(
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

    fn show_tabs(
        &mut self,
        tabs_ui: &mut Ui,
        state: &mut State,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        fade_style: Option<&Style>,
        tabbar_outer_rect: Rect,
        preferred_width: Option<f32>,
        viewer: &mut impl TabViewer<Id = T>,
    ) -> bool {
        let mut tab_hovered = false;

        if !matches!(self.dock_state[surface_index][node_index], Node::Leaf(_)) {
            return tab_hovered;
        }

        let focused = self.dock_state.focused_leaf();
        let tabs_len = self.dock_state[surface_index][node_index].tabs_count();
        let hide_dragged_header = state.drag.is_some() && !is_drag_over_tab_bar(state);
        // Tab rects plus whether that tab is the raised (active) one. The
        // separator shadows are drawn after the loop so they can fall away
        // from the active tab instead of onto it.
        let mut tab_visuals: Vec<(Rect, bool)> = Vec::with_capacity(tabs_len);
        // One clone per frame, not one per tab: the header painter only reads
        // the style while the loop below mutates the dock state.
        let tab_style = fade_style.unwrap_or(&self.dock_state.style).tab.clone();
        let draggable_tabs = self.draggable_tabs;

        for tab_index in 0..tabs_len {
            let tab_index = TabIndex(tab_index);
            let id = self
                .id
                .with((surface_index, "surface"))
                .with((node_index, "node"))
                .with((tab_index, "tab"));
            let is_being_dragged = tabs_ui.ctx().is_being_dragged(id)
                && tabs_ui.input(|i| i.pointer.is_decidedly_dragging())
                && self.draggable_tabs;

            if is_being_dragged {
                tabs_ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
            }

            let Some(tab_id) = self.dock_state[surface_index][node_index]
                .get_leaf()
                .and_then(|leaf| leaf.tabs.get(tab_index.0).copied())
            else {
                continue;
            };

            let is_active_tab = self.dock_state[surface_index][node_index]
                .get_leaf()
                .is_some_and(|leaf| leaf.active == tab_index);
            let is_active = is_being_dragged || is_active_tab;
            let label = viewer.title(tab_id);
            let icon = viewer.icon(tab_id);

            let (response, title_id) = if is_being_dragged {
                let layer_id = LayerId::new(egui::Order::Tooltip, id);
                let response = tabs_ui
                    .scope_builder(UiBuilder::new().layer_id(layer_id), |ui| {
                        show_tab_title(
                            ui,
                            &tab_style,
                            id,
                            icon,
                            label,
                            is_active && Some((surface_index, node_index)) == focused,
                            is_active,
                            is_being_dragged,
                            preferred_width,
                            hide_dragged_header,
                            draggable_tabs,
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
                let response = show_tab_title(
                    tabs_ui,
                    &tab_style,
                    id,
                    icon,
                    label,
                    is_active && Some((surface_index, node_index)) == focused,
                    is_active,
                    is_being_dragged,
                    preferred_width,
                    false,
                    draggable_tabs,
                );
                let title_id = response.id;
                let is_lonely_tab = self.dock_state[surface_index].tabs_count() == 1;
                viewer.on_tab_button(tab_id, &response);

                if self.show_tab_context_menus {
                    let eject_label = self
                        .dock_state
                        .translations
                        .tab_context_menu
                        .eject_button
                        .clone();
                    let close_label = self
                        .dock_state
                        .translations
                        .tab_context_menu
                        .close_button
                        .clone();
                    let mut eject_requested = false;
                    let mut close_requested = false;

                    response.context_menu(|ui| {
                        viewer.context_menu(ui, tab_id);
                        if (surface_index.is_main() || !is_lonely_tab)
                            && viewer.allowed_in_windows(tab_id)
                            && ui.add(Button::new(&eject_label)).clicked()
                        {
                            eject_requested = true;
                            ui.close();
                        }
                        if viewer.closeable(tab_id) && ui.add(Button::new(&close_label)).clicked() {
                            close_requested = true;
                            ui.close();
                        }
                    });

                    if eject_requested {
                        self.to_detach.push((surface_index, node_index, tab_index));
                    }
                    if close_requested {
                        self.to_remove.push((surface_index, node_index, tab_index));
                    }
                }

                if let Some(pos) = state.last_hover_pos {
                    if state.drag_start.is_some() && response.rect.contains(pos) {
                        self.tab_hover_rect = Some((response.rect, tab_index));
                    }
                }

                (response, title_id)
            };

            tab_visuals.push((response.rect, is_active_tab));

            if response.hovered() {
                tab_hovered = true;
            }

            if response.clicked()
                || (tabs_ui.memory(|m| m.has_focus(title_id))
                    && tabs_ui.input(|i| {
                        i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space)
                    }))
            {
                let Node::Leaf(leaf) = &mut self.dock_state[surface_index][node_index] else {
                    continue;
                };
                leaf.active = tab_index;
                self.new_focused = Some((surface_index, node_index));
            }
        }

        // Header dividers: 1px line, plus a light shadow cast *away* from the
        // raised tab so the active tab looks raised and its neighbour recessed.
        let tab_bar = fade_style
            .map(|style| &style.tab_bar)
            .unwrap_or(&self.dock_state.style.tab_bar);
        for pair in tab_visuals.windows(2) {
            let (_, left_active) = pair[0];
            let (right_rect, right_active) = pair[1];
            paint_tab_separator(
                tabs_ui.painter(),
                right_rect.left(),
                tabbar_outer_rect.top(),
                tabbar_outer_rect.bottom(),
                &tab_bar.separator,
                left_active,
                right_active,
            );
        }

        tab_hovered
    }

    fn show_body(
        &mut self,
        ui: &mut Ui,
        state: &State,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        spacing: Vec2,
        tabbar_rect: Rect,
        fade_style: Option<(&Style, f32)>,
        viewer: &mut impl TabViewer<Id = T>,
    ) {
        let (body_rect, _body_response) =
            ui.allocate_exact_size(ui.available_size_before_wrap(), Sense::hover());

        let Some(tab_id) = self.dock_state[surface_index][node_index]
            .get_leaf()
            .and_then(|leaf| leaf.tabs.get(leaf.active.0).copied())
        else {
            return;
        };

        let (style, fade_factor) = fade_style
            .map(|(style, factor)| (style.clone(), factor))
            .unwrap_or_else(|| (self.dock_state.style.clone(), 1.0));
        let tabs_style = &style.tab;
        let leaf_rect = self.dock_state[surface_index][node_index]
            .rect()
            .unwrap_or(body_rect);

        let rect_changed = {
            let Some(leaf) = self.dock_state[surface_index][node_index].get_leaf_mut() else {
                return;
            };
            let changed = leaf.viewport != body_rect;
            if changed {
                leaf.viewport = body_rect;
            }
            changed
        };
        if rect_changed {
            viewer.on_rect_changed(tab_id);
        }

        if ui.input(|i| i.pointer.any_click()) {
            if let Some(pos) = state.last_hover_pos {
                if body_rect.contains(pos) && Some(ui.layer_id()) == ui.ctx().layer_id_at(pos) {
                    self.new_focused = Some((surface_index, node_index));
                }
            }
        }

        ui.painter().rect_filled(
            body_rect,
            tabs_style.tab_body.corner_radius,
            tabs_style.tab_body.bg_fill,
        );

        let id = self.id.with(("tab-body", tab_id));
        ui.ctx().check_for_id_clash(id, body_rect, "a tab body");
        let ui = &mut Ui::new(
            ui.ctx().clone(),
            id,
            UiBuilder::new().max_rect(body_rect).layer_id(ui.layer_id()),
        );
        ui.set_clip_rect(Rect::from_min_max(ui.cursor().min, ui.clip_rect().max));
        ui.spacing_mut().item_spacing = spacing;

        let effective_stroke_width = (tabs_style.tab_body.stroke.width / 2.0).ceil() * 2.0;
        let tab_body_rect = Rect::from_min_max(
            ui.clip_rect().min - vec2(0.0, effective_stroke_width),
            ui.clip_rect().max,
        );
        ui.painter().rect_stroke(
            rect_stroke_box(tab_body_rect, tabs_style.tab_body.stroke.width),
            tabs_style.tab_body.corner_radius,
            tabs_style.tab_body.stroke,
            egui::StrokeKind::Inside,
        );

        ScrollArea::new(viewer.scroll_bars(tab_id)).show(ui, |ui| {
            Frame::new()
                .inner_margin(tabs_style.tab_body.inner_margin)
                .show(ui, |ui| {
                    if fade_factor != 1.0 {
                        fade_visuals(ui.visuals_mut(), fade_factor);
                    }
                    let available_rect = ui.available_rect_before_wrap();
                    ui.expand_to_include_rect(available_rect);
                    viewer.ui(ui, tab_id);
                });
        });

        if let Some(pointer) = state.last_hover_pos {
            if state.drag_start.is_some() && leaf_rect.contains(pointer) {
                let on_title_bar = tabbar_rect.contains(pointer);
                let (dst, tab) = match self.tab_hover_rect {
                    Some((rect, tab_index)) => (
                        TreeComponent::Tab(surface_index, node_index, tab_index),
                        Some(rect),
                    ),
                    None => (
                        TreeComponent::Node(surface_index, node_index),
                        on_title_bar.then_some(tabbar_rect),
                    ),
                };

                self.store_hover_data(
                    ui,
                    HoverData {
                        rect: leaf_rect,
                        dst,
                        tab,
                        tab_bar: (tabbar_rect != Rect::NOTHING).then_some(tabbar_rect),
                    },
                );
            }
        }
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
        fade_style: Option<&Style>,
    ) {
        if available_width <= 0.0 {
            return;
        }

        let DockState {
            style, surfaces, ..
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
                    Sense::click_and_drag(),
                );

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

                let points_to_scroll_coefficient =
                    overflow / (scroll_bar_handle_rect.width() - scroll_bar_handle_size);

                leaf.scroll -=
                    scroll_bar_handle_response.drag_delta().x * points_to_scroll_coefficient;

                if let Some(pos) = state.last_hover_pos {
                    if scroll_bar_rect.contains(pos) {
                        leaf.scroll += ui
                            .input(|i| i.smooth_scroll_delta.y + i.smooth_scroll_delta.x)
                            * points_to_scroll_coefficient;
                    }
                }

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

            if tabbar_response.hovered() || tab_hovered {
                leaf.scroll += ui.input(|i| i.smooth_scroll_delta.y + i.smooth_scroll_delta.x);
            }
        }

        leaf.scroll = leaf.scroll.clamp(-overflow, 0.0);
    }

    fn show_separator(
        &mut self,
        ui: &mut Ui,
        (surface_index, node_index): (SurfaceIndex, NodeIndex),
        fade_style: Option<&Style>,
    ) {
        let DockState {
            style, surfaces, ..
        } = self.dock_state;

        if !surfaces[surface_index][node_index].is_parent() {
            return;
        }

        let style = fade_style.unwrap_or_else(|| style);
        let pixels_per_point = ui.ctx().pixels_per_point();
        // A horizontal split draws a vertical separator; a vertical split draws
        // a horizontal one.
        let vertical_line = matches!(&surfaces[surface_index][node_index], Node::Horizontal(_));

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
                paint_split_separator_shadow(
                    ui.painter(),
                    separator,
                    vertical_line,
                    &style.separator,
                );

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
            egui::StrokeKind::Inside,
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

    fn compute_rect_size(
        &mut self,
        ui: &Ui,
        (surfce_index, node_index): (SurfaceIndex, NodeIndex),
        max_rect: Rect,
    ) {
        let DockState {
            style, surfaces, ..
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
                    midpoint + style.separator.width * 0.5,
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

// ---------------------------------------------------------------------------
// tab header: shared by the live tab strip and every drag preview
// ---------------------------------------------------------------------------
//
// A tab header is drawn from two paths: the live strip (`show_tabs` ->
// `show_tab_title`) and the drag ghosts (split preview + floating-window
// preview). They used to own separate geometry and painting (hard-coded 12 px
// font, `text + 24 px` tab width, no icon), so a ghost did not match the tab it
// stood in for. These helpers are the single definition; hit-testing and
// interaction stay in the callers.

/// Whether a tab with an optional icon actually shows the icon slot.
///
/// A style with `icon_size == 0` hides icons even when the viewer provides one;
/// both sizing and painting use this so they can never disagree.
fn has_visible_tab_icon(tab_style: &TabStyle, icon_present: bool) -> bool {
    icon_present && tab_style.icon_size > 0.0
}

/// Lay out a tab title with the dock's tab text style.
///
/// The live tab and the drag ghost both call this, so the ghost cannot end up
/// with a different font or line height than the tab it stands in for.
fn tab_title_galley(ui: &Ui, title: WidgetText) -> Arc<Galley> {
    title.into_galley(ui, None, f32::INFINITY, TextStyle::Button)
}

/// Full width of one tab: icon slot + title + symmetric padding, raised to the
/// style's minimum width (and to `preferred_width` when the bar fills tabs).
fn tab_width(
    tab_style: &TabStyle,
    has_icon: bool,
    galley: &Galley,
    preferred_width: Option<f32>,
) -> f32 {
    let content_width = tab_icon_slot_width(tab_style, has_icon) + galley.size().x;
    let minimum_width = tab_style
        .minimum_width
        .unwrap_or(0.0)
        .at_least(content_width + 2.0 * tab_style.horizontal_padding);
    preferred_width.unwrap_or(0.0).at_least(minimum_width)
}

fn tab_icon_slot_width(tab_style: &TabStyle, has_icon: bool) -> f32 {
    if has_icon {
        tab_style.icon_size.max(0.0) + tab_style.icon_spacing
    } else {
        0.0
    }
}

/// Where the icon and the title sit inside a tab rect.
struct TabHeaderLayout {
    icon_rect: Option<Rect>,
    text_pos: Pos2,
}

fn tab_header_layout(
    tab_rect: Rect,
    tab_style: &TabStyle,
    has_icon: bool,
    galley: &Galley,
) -> TabHeaderLayout {
    let icon_size = tab_style.icon_size.max(0.0);
    let icon_width = tab_icon_slot_width(tab_style, has_icon);
    let content_width = icon_width + galley.size().x;
    let content_left = tab_rect.center().x - content_width * 0.5;

    let icon_rect = has_icon.then(|| {
        Rect::from_center_size(
            pos2(content_left + icon_size * 0.5, tab_rect.center().y),
            Vec2::splat(icon_size),
        )
    });
    let text_pos = pos2(
        content_left + icon_width,
        tab_rect.center().y - galley.size().y * 0.5,
    );

    TabHeaderLayout {
        icon_rect,
        text_pos,
    }
}

/// Paint one tab header: fill, outline, coloured headerline and icon + title.
///
/// `top_inset` shifts the headerline down so a preview's window border stays
/// visible above it; live tabs pass `0.0`.
fn paint_tab_header(
    ui: &Ui,
    tab_rect: Rect,
    tab_style: &TabStyle,
    interaction: &TabInteractionStyle,
    icon: Option<&ImageSource<'static>>,
    galley: &Arc<Galley>,
    top_inset: f32,
) {
    let painter = ui.painter();
    painter.rect_filled(tab_rect, interaction.corner_radius, interaction.bg_fill);
    painter.rect_stroke(
        rect_stroke_box(tab_rect, 1.0),
        interaction.corner_radius,
        Stroke::new(1.0, interaction.outline_color),
        StrokeKind::Inside,
    );
    // Coloured line on top of the tab, drawn as a strip inside the tab so it
    // cannot bleed above the tab/into the panel below.
    let headerline = Rect::from_min_size(
        pos2(tab_rect.left(), tab_rect.top() + top_inset),
        vec2(tab_rect.width(), interaction.headerline_width),
    );
    painter.rect_filled(
        headerline,
        CornerRadius {
            nw: interaction.corner_radius.nw,
            ne: interaction.corner_radius.ne,
            sw: 0,
            se: 0,
        },
        interaction.headerline_color,
    );

    let has_icon = has_visible_tab_icon(tab_style, icon.is_some());
    let layout = tab_header_layout(tab_rect, tab_style, has_icon, galley);
    if let (Some(icon), Some(icon_rect)) = (icon, layout.icon_rect) {
        Image::new(icon.clone())
            .tint(interaction.icon_tint)
            .show_loading_spinner(false)
            .paint_at(ui, icon_rect);
    }
    painter.add(TextShape::new(
        layout.text_pos,
        galley.clone(),
        interaction.text_color,
    ));
}

/// Background strip of a tab bar (live dock bar or preview ghost).
fn paint_tab_bar(painter: &egui::Painter, rect: Rect, style: &TabBarStyle) {
    painter.rect_filled(rect, style.corner_radius, style.bg_fill);
}

/// Rect of a stroke drawn *inside* `rect` (see `StrokeKind::Inside`), expanded
/// back to the caller's coordinate space.
fn rect_stroke_box(rect: Rect, width: f32) -> Rect {
    rect.expand(-f32::ceil(width / 2.0))
}

/// Allocate, hit-test and paint one live tab header.
///
/// Interaction stays here (click/drag, cursor, context menu); the visual comes
/// from the shared tab-header helpers above so a live tab and its drag ghost
/// cannot drift apart.
#[allow(clippy::too_many_arguments)]
fn show_tab_title(
    ui: &mut Ui,
    tab_style: &TabStyle,
    id: Id,
    icon: Option<ImageSource<'static>>,
    label: WidgetText,
    focused: bool,
    active: bool,
    is_being_dragged: bool,
    preferred_width: Option<f32>,
    hidden: bool,
    draggable_tabs: bool,
) -> Response {
    let galley = tab_title_galley(ui, label);
    let has_icon = has_visible_tab_icon(tab_style, icon.is_some());
    let tab_width = tab_width(tab_style, has_icon, &galley, preferred_width);

    let (_, tab_rect) = ui.allocate_space(vec2(tab_width, ui.available_height()));
    let mut response = ui.interact(tab_rect, id, Sense::click_and_drag());
    if hidden {
        // The floating window preview is the visual for this frame; keep the
        // interaction rect so the drag stays alive.
        return response;
    }
    if ui.ctx().dragged_id().is_none() && draggable_tabs {
        response = response.on_hover_cursor(CursorIcon::Grab);
    }

    let interaction = if focused || is_being_dragged {
        &tab_style.focused
    } else if active {
        &tab_style.active
    } else if response.hovered() {
        &tab_style.hovered
    } else {
        &tab_style.inactive
    };
    paint_tab_header(
        ui,
        tab_rect,
        tab_style,
        interaction,
        icon.as_ref(),
        &galley,
        0.0,
    );

    response
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum WindowControl {
    Minimize,
    Maximize,
    Close,
}

#[derive(Clone, Copy)]
struct WindowControlRects {
    minimize: Rect,
    maximize: Rect,
    close: Rect,
}

fn window_control_rects(bar: Rect, style: &WindowStyle) -> WindowControlRects {
    let y = bar.center().y;
    let start = bar.left() + style.traffic_light_margin + style.traffic_light_radius;
    let hit = Vec2::splat((style.traffic_light_radius * 2.0).max(18.0));
    WindowControlRects {
        minimize: Rect::from_center_size(pos2(start + style.traffic_light_spacing, y), hit),
        maximize: Rect::from_center_size(pos2(start + style.traffic_light_spacing * 2.0, y), hit),
        close: Rect::from_center_size(pos2(start, y), hit),
    }
}

fn paint_window_control(
    painter: &egui::Painter,
    rect: Rect,
    control: WindowControl,
    style: &WindowStyle,
    hovered: bool,
) {
    let center = rect.center();
    let radius = style.traffic_light_radius;
    let fill = match control {
        WindowControl::Close => style.close_color,
        WindowControl::Minimize => style.minimize_color,
        WindowControl::Maximize => style.maximize_color,
    };
    painter.circle_filled(center, radius, fill);

    if !hovered {
        return;
    }

    let stroke = Stroke::new(1.2, style.glyph_color);
    let arm = radius * 0.48;
    match control {
        WindowControl::Close => {
            painter.line_segment(
                [
                    pos2(center.x - arm, center.y - arm),
                    pos2(center.x + arm, center.y + arm),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x + arm, center.y - arm),
                    pos2(center.x - arm, center.y + arm),
                ],
                stroke,
            );
        }
        WindowControl::Minimize => {
            painter.hline((center.x - arm)..=(center.x + arm), center.y, stroke);
        }
        WindowControl::Maximize => {
            painter.line_segment(
                [
                    pos2(center.x - arm, center.y),
                    pos2(center.x + arm, center.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(center.x, center.y - arm),
                    pos2(center.x, center.y + arm),
                ],
                stroke,
            );
        }
    }
}

/// Size of the floating-window drag ghost.
///
/// It is intentionally smaller than the window that will be created (Unity's
/// preview is roughly half the panel) but centred on the cursor, because that
/// is where the window will appear.
fn drag_window_preview_size(source: Vec2) -> Vec2 {
    Vec2::new(
        (source.x * 0.6).clamp(220.0, 320.0),
        (source.y * 0.55).clamp(150.0, 240.0),
    )
}

/// Tab-bar chunk of every drag preview.
///
/// Unity draws the dragged tab as the focused tab: it keeps the coloured
/// headerline (the blue top line) even while it lives in a preview window or a
/// split preview panel. The header itself comes from the shared tab-header
/// helpers, so the ghost has the same font, padding, icon and tab width as the
/// tab that will replace it after the drop.
fn paint_preview_bar(
    ui: &Ui,
    rect: Rect,
    title: WidgetText,
    icon: Option<ImageSource<'static>>,
    style: &Style,
) {
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), style.tab_bar.height));
    paint_tab_bar(ui.painter(), bar, &style.tab_bar);

    let galley = tab_title_galley(ui, title);
    let has_icon = has_visible_tab_icon(&style.tab, icon.is_some());
    let tab_width = tab_width(&style.tab, has_icon, &galley, None).min(bar.width());
    let tab_rect = Rect::from_min_size(bar.min, vec2(tab_width, bar.height()));
    paint_tab_header(
        ui,
        tab_rect,
        &style.tab,
        &style.tab.focused,
        icon.as_ref(),
        &galley,
        style.window.border.width,
    );
}

/// Floating-window ghost: tab bar only (no title bar, no system buttons) with
/// a translucent body, centred on the pointer.
fn paint_window_preview(
    ui: &Ui,
    pointer: Pos2,
    source_size: Vec2,
    title: WidgetText,
    icon: Option<ImageSource<'static>>,
    style: &Style,
    bounds: Rect,
) {
    let rect = constrain_rect_to_area(
        ui,
        Rect::from_center_size(pointer, drag_window_preview_size(source_size)),
        bounds,
    );
    let window = &style.window;
    let painter = ui.painter();
    // Blurred drop shadow, same shape the real floating windows use.
    painter.add(window.shadow.as_shape(rect, CornerRadius::ZERO));

    // Ghost body: translucent so the dock underneath stays readable, like
    // Unity's drag preview window. The opaque tab bar is painted over it.
    let body_fill = style.tab_bar.bg_fill;
    let body_fill =
        Color32::from_rgba_unmultiplied(body_fill.r(), body_fill.g(), body_fill.b(), 150);
    painter.rect_filled(rect, CornerRadius::ZERO, body_fill);
    paint_preview_bar(ui, rect, title, icon, style);

    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        window.border,
        egui::StrokeKind::Inside,
    );
}

/// Preview panel for a prospective split region: a real tab bar plus a body,
/// so the drop reads as "a new panel will appear here" instead of a colour
/// frame. Only the ghost body fill and shadow differ from the window ghost.
fn paint_preview_panel(
    ui: &Ui,
    rect: Rect,
    title: WidgetText,
    icon: Option<ImageSource<'static>>,
    style: &Style,
) {
    if rect.width() < 24.0 || rect.height() < style.tab_bar.height {
        return;
    }
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, style.tab.tab_body.bg_fill);
    paint_preview_bar(ui, rect, title, icon, style);
    painter.rect_stroke(
        rect,
        CornerRadius::ZERO,
        style.window.border,
        egui::StrokeKind::Inside,
    );
}

/// Soft shadow cast into both panels around a split separator, so the panels
/// read as two surfaces with a shadowed gutter between them.
fn paint_split_separator_shadow(
    painter: &egui::Painter,
    separator: Rect,
    vertical_line: bool,
    style: &crate::styles::SeparatorStyle,
) {
    for (offset, color) in shadow_ramp(style.shadow_width, style.shadow_color) {
        if vertical_line {
            let left = separator.left() - offset - 1.0;
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(left, separator.top()),
                    pos2(left + 1.0, separator.bottom()),
                ),
                CornerRadius::ZERO,
                color,
            );
            let right = separator.right() + offset;
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(right, separator.top()),
                    pos2(right + 1.0, separator.bottom()),
                ),
                CornerRadius::ZERO,
                color,
            );
        } else {
            let top = separator.top() - offset - 1.0;
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(separator.left(), top),
                    pos2(separator.right(), top + 1.0),
                ),
                CornerRadius::ZERO,
                color,
            );
            let bottom = separator.bottom() + offset;
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(separator.left(), bottom),
                    pos2(separator.right(), bottom + 1.0),
                ),
                CornerRadius::ZERO,
                color,
            );
        }
    }
}

/// Opacity ramp shared by the two soft shadows (tab separator and split
/// separator): one 1 px column per step, densest at the source edge and fading
/// to transparent after `width` steps.
///
/// Both shadows are painted as black alpha strips, so only the colour's alpha
/// matters and the ramp is identical; before this helper each painter had its
/// own copy of the step/alpha loop.
fn shadow_ramp(width: f32, color: Color32) -> impl Iterator<Item = (f32, Color32)> {
    let steps = width.ceil().max(1.0) as usize;
    let base_alpha = color.a() as f32;
    (0..steps).filter_map(move |step| {
        let t = step as f32 / steps as f32;
        let alpha = (base_alpha * (1.0 - t)).round() as u8;
        (alpha > 0).then(|| (step as f32, Color32::from_black_alpha(alpha)))
    })
}

fn is_drag_over_tab_bar(state: &State) -> bool {
    state.drag.as_ref().is_some_and(|drag_state| {
        drag_state.hover.tab.is_some()
            || drag_state
                .hover
                .tab_bar
                .is_some_and(|tab_bar| tab_bar.contains(drag_state.pointer))
    })
}

fn paint_tab_separator(
    painter: &egui::Painter,
    x: f32,
    top: f32,
    bottom: f32,
    style: &TabSeparatorStyle,
    left_active: bool,
    right_active: bool,
) {
    // 1px dark line at the boundary.
    let line = Rect::from_min_max(pos2(x, top), pos2(x + style.line_width, bottom));
    painter.rect_filled(line, CornerRadius::ZERO, style.line_color);

    // The raised (active) tab casts the shadow away from itself: an active tab
    // on the left shadows the tab to its right, and vice versa. Two inactive
    // tabs get the divider line only. Columns are anchored on the line rect so
    // there is no unpainted gap between line and shadow.
    let direction = match (left_active, right_active) {
        (true, false) => 1.0,
        (false, true) => -1.0,
        _ => return,
    };

    for (offset, color) in shadow_ramp(style.shadow_width, style.shadow_color) {
        let x0 = if direction > 0.0 {
            line.right() + offset
        } else {
            line.left() - offset - 1.0
        };
        let column = Rect::from_min_max(pos2(x0, top), pos2(x0 + 1.0, bottom));
        painter.rect_filled(column, CornerRadius::ZERO, color);
    }
}

fn fade_dock_style(style: &mut Style, factor: f32) {
    style.main_surface_border_stroke.color = style
        .main_surface_border_stroke
        .color
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
    style.icon_tint = style.icon_tint.linear_multiply(factor);
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{ColorImage, FullOutput, RawInput, TextureOptions, load::SizedTexture, pos2, vec2};

    fn run_paint(f: impl FnOnce(&Ui)) -> FullOutput {
        let ctx = Context::default();
        let mut input = RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        let f = std::cell::RefCell::new(Some(f));
        ctx.run_ui(input, |ui| {
            if let Some(f) = f.borrow_mut().take() {
                f(ui);
            }
        })
    }

    fn texture_icon(ctx: &Context, name: &str) -> (egui::TextureHandle, ImageSource<'static>) {
        let texture = ctx.load_texture(
            name,
            ColorImage::filled([16, 16], Color32::WHITE),
            TextureOptions::default(),
        );
        let source = ImageSource::Texture(SizedTexture::new(texture.id(), Vec2::splat(16.0)));
        (texture, source)
    }

    fn find_icon(shape: &egui::Shape, texture_id: egui::TextureId) -> Option<Rect> {
        match shape {
            egui::Shape::Rect(rect)
                if rect
                    .brush
                    .as_ref()
                    .is_some_and(|brush| brush.fill_texture_id == texture_id) =>
            {
                Some(rect.rect)
            }
            egui::Shape::Vec(shapes) => {
                shapes.iter().find_map(|shape| find_icon(shape, texture_id))
            }
            _ => None,
        }
    }

    fn find_text_pos(shape: &egui::Shape) -> Option<Pos2> {
        match shape {
            egui::Shape::Text(text) => Some(text.pos),
            egui::Shape::Vec(shapes) => shapes.iter().find_map(find_text_pos),
            _ => None,
        }
    }

    /// Both drag ghosts used to paint their own title-only tab header. They now
    /// go through the shared tab-header helpers, so the ghost carries the same
    /// icon+title layout as the live tab that will replace it.
    #[test]
    fn split_preview_paints_the_shared_header_with_icon_and_title() {
        let ctx = Context::default();
        let (texture, icon) = texture_icon(&ctx, "split-preview-icon");
        let style = Style::default();
        let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(240.0, 160.0));

        let output = run_paint(|ui| {
            paint_preview_panel(ui, rect, WidgetText::from("tab 4"), Some(icon), &style);
        });

        let icon_rect = output
            .shapes
            .iter()
            .find_map(|clipped| find_icon(&clipped.shape, texture.id()))
            .expect("split preview should paint the tab icon");
        assert!((icon_rect.width() - 16.0).abs() < 0.5, "{icon_rect:?}");
        assert!((icon_rect.height() - 16.0).abs() < 0.5, "{icon_rect:?}");
        assert!(
            rect.contains_rect(icon_rect),
            "{icon_rect:?} not in {rect:?}"
        );
        assert!(
            icon_rect.bottom() <= rect.top() + style.tab_bar.height,
            "icon is not in the preview tab bar: {icon_rect:?}"
        );

        let text_pos = output
            .shapes
            .iter()
            .find_map(|clipped| find_text_pos(&clipped.shape))
            .expect("split preview should paint the tab title");
        assert!(
            text_pos.x >= icon_rect.right(),
            "icon {icon_rect:?}, title {text_pos:?}"
        );
    }

    #[test]
    fn window_preview_paints_the_shared_header_with_icon_and_title() {
        let ctx = Context::default();
        let (texture, icon) = texture_icon(&ctx, "window-preview-icon");
        let style = Style::default();
        let pointer = pos2(400.0, 300.0);
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));

        let output = run_paint(|ui| {
            paint_window_preview(
                ui,
                pointer,
                Vec2::new(400.0, 300.0),
                WidgetText::from("tab 4"),
                Some(icon),
                &style,
                bounds,
            );
        });

        let icon_rect = output
            .shapes
            .iter()
            .find_map(|clipped| find_icon(&clipped.shape, texture.id()))
            .expect("window preview should paint the tab icon");
        assert!((icon_rect.width() - 16.0).abs() < 0.5, "{icon_rect:?}");
        assert!((icon_rect.height() - 16.0).abs() < 0.5, "{icon_rect:?}");
        assert!(bounds.contains_rect(icon_rect), "{icon_rect:?}");

        let text_pos = output
            .shapes
            .iter()
            .find_map(|clipped| find_text_pos(&clipped.shape))
            .expect("window preview should paint the tab title");
        assert!(
            text_pos.x >= icon_rect.right(),
            "icon {icon_rect:?}, title {text_pos:?}"
        );
    }

    #[test]
    fn tab_width_reserves_an_icon_slot_and_symmetric_padding() {
        run_paint(|ui| {
            let style = TabStyle::default();
            let galley = tab_title_galley(ui, WidgetText::from("Console"));
            let text_only = tab_width(&style, false, &galley, None);
            let with_icon = tab_width(&style, true, &galley, None);

            assert!(
                with_icon - text_only - style.icon_size - style.icon_spacing < 0.01,
                "text {text_only}, icon {with_icon}"
            );
            assert!(
                text_only >= galley.size().x + 2.0 * style.horizontal_padding - 0.01,
                "padding missing from {text_only}"
            );
        });
    }

    #[test]
    fn tab_icon_sits_left_of_a_centered_title_block() {
        run_paint(|ui| {
            let style = TabStyle::default();
            let galley = tab_title_galley(ui, WidgetText::from("Console"));
            let tab_rect = Rect::from_min_size(pos2(100.0, 10.0), vec2(120.0, 24.0));
            let layout = tab_header_layout(tab_rect, &style, true, &galley);

            let icon_rect = layout.icon_rect.expect("icon slot");
            assert!((icon_rect.width() - style.icon_size).abs() < 0.01);
            assert!((icon_rect.height() - style.icon_size).abs() < 0.01);
            assert!((icon_rect.center().y - tab_rect.center().y).abs() < 0.01);
            assert!(layout.text_pos.x >= icon_rect.right());
            assert!(tab_rect.contains_rect(icon_rect));

            // Icon + title are centered as one block, exactly like the live tab.
            let content_left = layout.text_pos.x - style.icon_size - style.icon_spacing;
            let content_right = layout.text_pos.x + galley.size().x;
            assert!(
                ((content_left + content_right) * 0.5 - tab_rect.center().x).abs() < 0.01,
                "content block is not centered: {content_left}..{content_right}"
            );
        });
    }

    #[test]
    fn zero_icon_size_hides_the_tab_icon_slot() {
        let mut style = TabStyle::default();
        style.icon_size = 0.0;
        assert!(!has_visible_tab_icon(&style, true));
        assert!(has_visible_tab_icon(&TabStyle::default(), true));
        assert!(!has_visible_tab_icon(&TabStyle::default(), false));
    }
}
