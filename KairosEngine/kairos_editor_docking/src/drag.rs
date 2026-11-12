use egui::{Color32, Context, CornerRadius, Id, LayerId, NumExt, Painter, Pos2, Rect, Stroke, Ui, Vec2, emath::{GuiRounding, inverse_lerp}};

use crate::{docker::{AllowedSplits, TabDestination, TabInsert}, styles::Style, surface::SurfaceIndex, tree::{NodeIndex, TabIndex}};

#[derive(Clone, Copy)]
pub enum TreeComponent {
    Surface(SurfaceIndex),
    Node(SurfaceIndex, NodeIndex),
    Tab(SurfaceIndex, NodeIndex, TabIndex)
}

impl TreeComponent {
    pub fn node_address(&self) -> (SurfaceIndex, Option<NodeIndex>) {
        match *self {
            TreeComponent::Surface(surface_index) => (surface_index, None),
            TreeComponent::Node(surface_index, node_index) => (surface_index, Some(node_index)),
            TreeComponent::Tab(surface_index, node_index, _) => (surface_index, Some(node_index)),
        }
    }

    pub fn is_surface(&self) -> bool {
        matches!(self, TreeComponent::Surface(_))
    }

    pub fn surface_address(&self) -> SurfaceIndex {
        match *self {
            TreeComponent::Surface(surface_index)
            | TreeComponent::Node(surface_index, _)
            | TreeComponent::Tab(surface_index, _, _) => surface_index,
        }
    }

    pub fn as_tab_destination(&self) -> TabDestination {
        match *self {
            TreeComponent::Surface(surface_index) => TabDestination::EmptySurface(surface_index),
            TreeComponent::Node(surface_index, node_index) => TabDestination::Node(surface_index, node_index, TabInsert::Append),
            TreeComponent::Tab(surface_index, node_index, tab_index) => TabDestination::Node(surface_index, node_index, TabInsert::Insert(tab_index)),
        }
    }
}

#[derive(PartialEq, Eq)]
enum LockState {
    Unlocked,
    SoftLock,
    HardLock,
}

#[derive(Clone, Copy)]
pub struct HoverData {
    pub rect: Rect,
    pub dst: TreeComponent,
    pub tab: Option<Rect>,
}

#[derive(Clone, Copy)]
pub struct DragData {
    pub src: TreeComponent,
    pub rect: Rect
}

#[derive(Clone)]
pub struct DragDropState {
    pub hover: HoverData,
    pub drag: DragData,
    pub pointer: Pos2,
    pub locked: Option<f64>,
}

impl DragDropState {
    pub fn is_locked(&self, ctx: &Context, style: &Style) -> bool {
        match self.locked.as_ref() {
            Some(lock_time) => {
                let elapsed = ctx.input(|i| (i.time - lock_time) as f32);
                ctx.request_repaint();
                elapsed < style.overlay.feel.max_preference_time
            },
            None => false,
        }
    }

    pub fn is_on_title_bar(&self) -> bool {
        self.hover.tab.is_some()
    }

    pub fn resolve(&mut self, ui: &Ui, style: &Style, allowed_splits: AllowedSplits, windows_allowed: bool, window_bounds: Rect) -> Option<TabDestination> {
        if !windows_allowed && self.hover.dst.surface_address() != SurfaceIndex::main() {
            return None;
        }
        show_highlight_rect(ui, self.hover.rect);

        if let Some(rect) = self.hover.tab {
            show_drop_rect(ui, style, rect);
            let target_lock_state = if rect.contains(self.pointer) {
                LockState::SoftLock
            } else {
                LockState::Unlocked
            };
            self.update_lock(target_lock_state, style, ui.ctx());
            return Some(self.hover.dst.as_tab_destination());
        }

        let (hover_rect, pointer) = (self.hover.rect, self.pointer);
        let center = hover_rect.center();

        let (tab_insertion, overlay_rect) = {
            let a_pos = (Pos2::new(
                inverse_lerp(hover_rect.x_range().into(), pointer.x).unwrap_or(1.0),
                inverse_lerp(hover_rect.y_range().into(), pointer.y).unwrap_or(1.0)
            ) - Pos2::new(0.5, 0.5))
            .to_pos2();

            let center_drop_rect = Rect::from_center_size(
                Pos2::ZERO,
                Vec2::splat(style.overlay.feel.center_drop_coverage),
            );
            let window_drop_rect = Rect::from_center_size(
                Pos2::ZERO,
                Vec2::splat(style.overlay.feel.window_drop_coverage),
            );

            if center_drop_rect.contains(a_pos) {
                (Some(TabInsert::Append), Rect::EVERYTHING)
            } else if window_drop_rect.contains(a_pos) {
                match windows_allowed {
                    true => (None, Rect::NOTHING),
                    false => (Some(TabInsert::Append), Rect::EVERYTHING),
                }
            } else {
                let a_pos = match allowed_splits {
                    AllowedSplits::All => a_pos,
                    AllowedSplits::LeftRightOnly => Pos2::new(a_pos.x, 0.0),
                    AllowedSplits::TopBottomOnly => Pos2::new(0.0, a_pos.y),
                    AllowedSplits::None => Pos2::ZERO,
                };
                if a_pos == Pos2::ZERO {
                    match windows_allowed {
                        true => (None, Rect::NOTHING),
                        false => (Some(TabInsert::Append), Rect::EVERYTHING),
                    }
                } else {
                    match (a_pos.x - a_pos.y > 0.0, -a_pos.x - a_pos.y > 0.0) {
                        (true, true) => (
                            Some(TabInsert::Split(crate::split::Split::Top)),
                            Rect::everything_above(center.y),
                        ),
                        (false, true) => (
                            Some(TabInsert::Split(crate::split::Split::Left)),
                            Rect::everything_left_of(center.x),
                        ),
                        (true, false) => (
                            Some(TabInsert::Split(crate::split::Split::Right)),
                            Rect::everything_right_of(center.x),
                        ),
                        (false, false) => (
                            Some(TabInsert::Split(crate::split::Split::Bottom)),
                            Rect::everything_below(center.y),
                        ),
                    }
                }
            }
        };

        let default_value = windows_allowed.then(|| TabDestination::Window(Rect::from_min_size(pointer, self.drag.rect.size())));
        let final_result = tab_insertion.map_or(default_value, |tab| match self.hover.dst {
            TreeComponent::Surface(surface_index) => Some(TabDestination::EmptySurface(surface_index)),
            TreeComponent::Node(surface_index, node_index) => Some(TabDestination::Node(surface_index, node_index, tab)),
            _ => None,
        });

        self.update_lock(LockState::SoftLock, style, ui.ctx());

        match final_result {
            Some(TabDestination::Window(rect)) => {
                let rect = self.window_preview_rect(rect);
                let rect_bounded = constrain_rect_to_area(ui, rect, window_bounds);
                show_window_rect(ui, style, rect_bounded);
            },
            Some(_) => {
                show_drop_rect(ui, style, hover_rect.intersect(overlay_rect));
            }
            None => (),
        }

        final_result
    }

    fn update_lock(&mut self, target_state: LockState, style: &Style, ctx: &Context) {
        match self.locked.as_mut() {
            Some(lock_time) => {
                if target_state == LockState::HardLock {
                    *lock_time = ctx.input(|i| i.time);
                }
                let window_hold = if !self.hover.dst.surface_address().is_main() {
                    ctx.request_repaint();
                    self.is_locked(ctx, style)
                } else {
                    false
                };
                if target_state == LockState::Unlocked && !window_hold {
                    self.locked = None
                }
            },
            None => {
                if target_state != LockState::Unlocked {
                    self.locked = Some(ctx.input(|i| i.time));
                }
            },
        }
    }

    fn window_preview_rect(&self, rect: Rect) -> Rect {
        if self.drag.src.surface_address() == SurfaceIndex::main() {
            Rect::from_min_size(rect.min, rect.size() * 0.8)
        } else {
            rect
        }
    }
}

fn make_overlay_painter(ui: &Ui) -> Painter {
    let id = Id::new("overly");
    let layer_id = LayerId::new(egui::Order::Foreground, id);
    ui.ctx().layer_painter(layer_id)
}

fn show_highlight_rect(ui: &Ui, rect: Rect) {
    let painter = make_overlay_painter(ui);
    painter.rect(
        rect,
        CornerRadius::same(0),
        Color32::TRANSPARENT,
        Stroke::NONE,
        egui::StrokeKind::Inside
    );
}

fn show_drop_rect(ui: &Ui, style: &Style, rect: Rect) {
    let painter = make_overlay_painter(ui);
    painter.rect_filled(rect, CornerRadius::same(0), style.overlay.selection_color);
}

fn show_window_rect(ui: &Ui, style: &Style, rect: Rect) {
    let painter = make_overlay_painter(ui);
    painter.rect_stroke(
        rect,
        CornerRadius::same(0),
        Stroke::new(
            style.overlay.selection_stroke_width,
            style.overlay.selection_color,
        ),
        egui::StrokeKind::Inside
    );
}

fn constrain_rect_to_area(ui: &Ui, rect: Rect, mut bounds: Rect) -> Rect {
    if rect.width() > bounds.width() {
        let screen_rect = ui.ctx().content_rect();
        (bounds.min.x, bounds.max.x) = (screen_rect.min.x, screen_rect.max.x);
    }
    if rect.height() > bounds.height() {
        let screen_rect = ui.ctx().content_rect();
        (bounds.min.y, bounds.max.y) = (screen_rect.min.y, screen_rect.max.y);
    }

    let mut pos = rect.min;

    let margin_x = (rect.width() - bounds.width()).at_least(0.0);
    let margin_y = (rect.height() - bounds.height()).at_least(0.0);

    pos.x = pos.x.at_most(bounds.right() + margin_x - rect.width()); // move left if needed
    pos.x = pos.x.at_least(bounds.left() - margin_x); // move right if needed
    pos.y = pos.y.at_most(bounds.bottom() + margin_y - rect.height()); // move right if needed
    pos.y = pos.y.at_least(bounds.top() - margin_y); // move down if needed

    pos = pos.round_to_pixels(ui.painter().pixels_per_point());

    Rect::from_min_size(pos, rect.size())
}
