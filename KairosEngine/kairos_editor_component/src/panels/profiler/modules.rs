use egui::{Align2, CornerRadius, Rect, Sense, Ui};
use kairos_editor_widgets::{
    color::Color32Ext,
    divider::Divider,
    min_max_scroll::{MinMaxScroll, ScrollWindow},
    timeline_axis::{TimelineRuler, TimelineRulerStyle},
    ui_image,
};
use kairos_profile::{ProfileSession};

use crate::panels::profiler::{
    ProfilerPanel, WHEEL_NOTCH_POINTS, ZOOM_PER_NOTCH, axis::ModuleAxis
};

mod cpu;
mod gpu;

/// One frame's column on the module area's usage chart.
struct ChartColumn {
    /// Index into ProfileSession::frames.
    frame: usize,
    /// Left edge, in chart columns (the plot starts at zero).
    x: f32,
    /// Column width: the frame's wall time at the axis' zoom.
    width: f32,
    /// Stacked height of the frame's visible categories, in ms.
    ms: f32,
}

impl ProfilerPanel {
    /// The module pane: a pinned time axis on top, a pinned zoom bar at the
    /// bottom, and between them a window onto the module cards.
    ///
    /// The axis labels the capture's real clock times and compresses the pause
    /// between two takes into a break band (see the axis module). The bar under
    /// the cards is a min-max scroller over the whole capture: its thumb pans the
    /// axis and its ends zoom it. The cards paint at their skin heights while the
    /// pane can show them; a shorter pane fits the CPU card to its viewport (see
    /// [`Self::module_cpu_fit`]) and clips the stack below, and the wheel over the
    /// name column and the pane's vertical bar move it.
    pub(super) fn module_area(&mut self, ui: &mut Ui, area: Rect, session: &ProfileSession) {
        if area.width() <= 0.0 || area.height() <= 0.0 {
            return;
        }
        let width = area.width();
        let bar_width = self.style.scrollbar_width.min(width);
        let name_width = self.style.module_name_width.min(width);
        let chart_left = (area.left() + name_width)
            .min(area.right() - bar_width)
            .max(area.left());
        let chart_right = (area.right() - bar_width).max(chart_left);
        let chart_width = (chart_right - chart_left).max(0.0);

        let axis_height = self.module_axis_height().min(area.height());
        let axis_row =
            Rect::from_min_max(area.min, egui::pos2(area.right(), area.top() + axis_height));
        let bar_height = self
            .module_bar_height()
            .min((area.bottom() - axis_row.bottom()).max(0.0));
        let bar_row = Rect::from_min_max(
            egui::pos2(area.left(), area.bottom() - bar_height),
            area.max,
        );
        let cards = Rect::from_min_max(
            egui::pos2(area.left(), axis_row.bottom() + self.style.separator_width),
            egui::pos2(chart_right, bar_row.top()),
        );
        let vertical_bar = Rect::from_min_max(
            egui::pos2(chart_right, cards.top()),
            egui::pos2(area.right(), cards.bottom()),
        );
        let separator_width = self.style.separator_width;
        // The card stack this pane shows. The CPU card is the pane's reason to
        // exist, so it takes its skin height only while the viewport can show it
        // whole and otherwise shrinks to the viewport: the chart's bars grow from
        // the plot's bottom, so clipping the card at the pane's edge would hide
        // the short bars of a fast frame. The GPU card keeps its height under it,
        // so a short pane still scrolls down to the GPU.
        let cards_height = cards.height();
        let cpu_height = self.module_cpu_fit(cards_height);
        let gpu_height = self.style.module_gpu_height.max(0.0);
        let content_height = cpu_height + separator_width + gpu_height;
        let max_scroll = (content_height - cards_height).max(0.0);

        // The axis reads the takes, then places its window: the default
        // last-frames view, or the Live follow once that window is left.
        self.module_axis.sync(session);
        self.module_axis.layout(chart_width);
        if !session.frames.is_empty() && !self.module_view_started {
            self.module_axis
                .show_last_frames(&session.frames, self.style.window_fram_count.max(1));
            self.module_view_started = true;
        } else if self.follow_latest && self.module_default_window {
            self.module_axis
                .show_last_frames(&session.frames, self.style.window_fram_count.max(1));
        } else if self.follow_latest {
            self.module_axis.follow_end();
        }
        if self.reveal_frame {
            self.reveal_frame = false;
            if let Some(frame) = session.frames.get(self.selected_frame) {
                if let Some((_, recorded)) = self.module_axis.time_of_source(frame.start_ns as f64)
                {
                    self.module_axis.reveal(recorded);
                }
            }
        }

        // Chrome fills: everything here is panel chrome except the chart columns
        // of the axis row, which carry the clock.
        let chrome = self.style.timeline_name_bg.to_egui();
        let separator_color = self.style.separator.to_egui();
        ui.painter()
            .rect_filled(axis_row, CornerRadius::ZERO, chrome);
        ui.painter()
            .rect_filled(bar_row, CornerRadius::ZERO, chrome);
        if axis_height > 0.0 {
            self.paint_module_axis(ui, axis_row, chart_left, chart_right);
            Divider::horizontal()
                .thickness(separator_width)
                .color(separator_color)
                .paint_in(
                    ui,
                    Rect::from_min_size(
                        egui::pos2(area.left(), axis_row.bottom()),
                        egui::vec2(width, separator_width),
                    ),
                );
        }

        // The zoom bar: a min-max scroller over the recorded time. Dragging its
        // thumb pans the window (and stops following the newest frame); dragging
        // either end zooms it.
        if bar_height > 0.0 && chart_width > 0.0 && !session.frames.is_empty() {
            let bar = Rect::from_min_max(
                egui::pos2(chart_left, bar_row.top()),
                egui::pos2(chart_right, bar_row.bottom()),
            );
            let was_left = self.module_axis.left();
            let mut window = self.module_axis.window();
            let changed = MinMaxScroll::horizontal(&self.scroll_style())
                .resizable(true)
                .show(
                    ui,
                    ui.id().with("profiler.modules.hscroll"),
                    bar,
                    &mut window,
                )
                .changed();
            if changed {
                self.module_axis.apply_window(window.value, window.span);
                self.module_default_window = false;
                if (self.module_axis.left() - was_left).abs() > 1.0 {
                    self.follow_latest = false;
                }
                ui.ctx().request_repaint();
            }
        }

        // The cards: the pane is the clip, and the pane's vertical bar moves the
        // stack.
        if cards_height > 0.0 && cards.width() > 0.0 {
            self.module_scroll = self.module_scroll.clamp(0.0, max_scroll);
            self.paint_module_scroll_bar(ui, vertical_bar, cards_height, content_height);

            let content = Rect::from_min_size(
                egui::pos2(cards.left(), cards.top() - self.module_scroll),
                egui::vec2(cards.width(), content_height),
            );
            // A child UI inherits its parent's painter clip; shrink it to the
            // pane so the part of the stack below the pane's edge (the GPU card,
            // or a scrolled stack) cannot paint over the ruler or the zoom bar.
            // Reserve the whole pane, not just the card viewport, in the child:
            // the parent cursor must still end where the module band ends, or the
            // splitter below would sit at the card viewport's bottom instead.
            let _ = ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
                ui.advance_cursor_after_rect(area);
                ui.shrink_clip_rect(cards);
                let cpu = Rect::from_min_size(content.min, egui::vec2(content.width(), cpu_height));
                self.paint_cpu_module(ui, cpu, session);
                Divider::horizontal()
                    .thickness(separator_width)
                    .color(separator_color)
                    .paint_in(
                        ui,
                        Rect::from_min_size(
                            egui::pos2(content.left(), cpu.bottom()),
                            egui::vec2(content.width(), separator_width),
                        ),
                    );
                let gpu = Rect::from_min_size(
                    egui::pos2(content.left(), cpu.bottom() + separator_width),
                    egui::vec2(content.width(), gpu_height),
                );
                self.paint_gpu_module(ui, gpu);
            });
        }

        // The selected-frame cursor: one line from the clock row down through
        // the cards, riding the middle of the frame's bar. It is drawn after the
        // cards so it stays visible over the bars and the break bands, and it is
        // what the drag on the chart grabs.
        if let Some(column) = self.module_cursor_column(session) {
            let x = chart_left + column;
            if x >= chart_left && x <= chart_right {
                let bottom = if cards_height > 0.0 {
                    cards.bottom()
                } else {
                    axis_row.bottom()
                };
                let clip = Rect::from_min_max(
                    egui::pos2(chart_left, axis_row.top()),
                    egui::pos2(chart_right, bottom),
                );
                ui.painter().with_clip_rect(clip).rect_filled(
                    Rect::from_min_size(
                        egui::pos2(x - 1.0, axis_row.top()),
                        egui::vec2(2.0, bottom - axis_row.top()),
                    ),
                    CornerRadius::ZERO,
                    self.style.chart_cursor.to_egui(),
                );
            }
        }

        // Wheel: over the chart columns it zooms the time axis around the
        // pointer (its horizontal part pans); over the name column or the
        // vertical bar it scrolls the cards; over the zoom bar it pans.
        let pointer = ui.input(|input| input.pointer.hover_pos());
        let wheel = ui.input(|input| input.smooth_scroll_delta);
        if !session.frames.is_empty() && wheel != egui::Vec2::ZERO {
            if let Some(pos) = pointer {
                let over_chart = pos.x >= chart_left && pos.x < chart_right;
                let over_time = over_chart && (axis_row.contains(pos) || cards.contains(pos));
                let used = if over_time {
                    if wheel.y != 0.0 {
                        let factor = ZOOM_PER_NOTCH.powf(f64::from(wheel.y) / WHEEL_NOTCH_POINTS);
                        self.module_axis.zoom_around(pos.x - chart_left, factor);
                    }
                    if wheel.x != 0.0 {
                        self.module_axis.pan_screen(wheel.x);
                        self.follow_latest = false;
                    }
                    self.module_default_window = false;
                    true
                } else if over_chart && bar_row.contains(pos) {
                    let step = wheel.y / WHEEL_NOTCH_POINTS as f32 * chart_width * 0.1;
                    self.module_axis.pan_screen(step);
                    self.module_default_window = false;
                    self.follow_latest = false;
                    true
                } else if cards.contains(pos)
                    || vertical_bar.contains(pos)
                    || axis_row.contains(pos)
                {
                    self.module_scroll = (self.module_scroll - wheel.y).clamp(0.0, max_scroll);
                    true
                } else {
                    false
                };
                if used {
                    ui.input_mut(|input| input.smooth_scroll_delta = egui::Vec2::ZERO);
                    ui.ctx().request_repaint();
                }
            }
        }

        // Middle-drag (or alt-drag) pans the time columns, like the timeline.
        let alt = ui.input(|input| input.modifiers.alt);
        let panning =
            ui.input(|input| input.pointer.middle_down() || (alt && input.pointer.primary_down()));
        if panning {
            if let Some(pos) = pointer {
                let time_area = Rect::from_min_max(
                    egui::pos2(chart_left, axis_row.top()),
                    egui::pos2(chart_right, cards.bottom()),
                );
                if time_area.contains(pos) {
                    let delta = ui.input(|input| input.pointer.delta().x);
                    if delta != 0.0 {
                        self.module_axis.pan_screen(delta);
                        self.module_default_window = false;
                        self.follow_latest = false;
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                        ui.ctx().request_repaint();
                    }
                }
            }
        }

        // A click or drag on the clock ruler selects the frame recorded at that
        // time; a double-click restores the default last-frames window.
        if chart_width > 0.0 && axis_height > 0.0 && !session.frames.is_empty() {
            let ruler_track = Rect::from_min_max(
                egui::pos2(chart_left, axis_row.top()),
                egui::pos2(chart_right, axis_row.bottom()),
            );
            let response = ui.interact(
                ruler_track,
                ui.id().with("profiler.modules.ruler"),
                Sense::click_and_drag(),
            );
            if response.double_clicked() {
                self.module_default_window = true;
                self.module_view_started = true;
                self.module_axis
                    .show_last_frames(&session.frames, self.style.window_fram_count.max(1));
                ui.ctx().request_repaint();
            } else if response.clicked() || response.dragged() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let x = (pos.x - chart_left).clamp(0.0, chart_width);
                    self.select_module_frame_at(x, session);
                }
            }
            if response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
        }
    }

    /// The module pane's vertical scroll bar: the shared non-resizable
    /// MinMaxScroll, so it looks and behaves like the timeline's bars.
    fn paint_module_scroll_bar(
        &mut self,
        ui: &mut Ui,
        bar: Rect,
        viewport_height: f32,
        content_height: f32,
    ) {
        if bar.width() <= 0.0 || bar.height() <= 0.0 {
            return;
        }
        let span = f64::from(viewport_height.min(content_height).max(0.0));
        let mut window = ScrollWindow::new(
            f64::from(self.module_scroll),
            span,
            (0.0, f64::from(content_height.max(0.0))),
        );
        let _ = MinMaxScroll::vertical(&self.scroll_style()).show(
            ui,
            ui.id().with("profiler.modules.scroll"),
            bar,
            &mut window,
        );
        self.module_scroll = window.value as f32;
    }

    /// The CPU Usage card's height in a card viewport `viewport` points tall: its
    /// skin height, or the whole viewport when the pane is shorter.
    ///
    /// The card yields to the pane instead of being clipped, because a usage
    /// chart is read from its zero line up: the bars grow from the plot's bottom,
    /// so a card cut off at the pane's edge hides the short bars a high frame rate
    /// produces and reads as "CPU Usage recorded nothing".
    pub(super) fn module_cpu_fit(&self, viewport: f32) -> f32 {
        self.style.module_cpu_height.clamp(0.0, viewport.max(0.0))
    }

    /// The module area's pinned time axis: real clock ticks over the chart
    /// columns, with a break band for every pause between two takes.
    ///
    /// The axis and its ruler are
    /// [kairos_editor_widgets::timeline_axis]; this panel supplies the capture's
    /// takes and its clock labels.
    fn paint_module_axis(&self, ui: &Ui, row: Rect, chart_left: f32, chart_right: f32) {
        let track = Rect::from_min_max(
            egui::pos2(chart_left, row.top()),
            egui::pos2(chart_right.max(chart_left), row.bottom()),
        );
        let style = TimelineRulerStyle {
            background: self.style.ruler_bg.to_egui(),
            text: self.style.ruler_text.to_egui(),
            break_background: self.style.break_bg.to_egui(),
            font: self.small_font(),
        };
        let labels = self.module_axis.labels();
        TimelineRuler::new(
            &self.module_axis,
            self.module_axis.ruler_scale(),
            &labels,
            &style,
        )
        .show(ui, track);

        // Rule between the name column and the clock columns, like the ruler
        // over the timeline's tracks.
        self.paint_vertical_rule(
            ui,
            egui::pos2(track.left() - self.style.separator_width, row.top()),
            row.height(),
            self.style.separator.to_egui(),
        );
    }

    /// Select the frame under a chart column: clicking a usage chart at a time
    /// selects the frame recorded then. The gesture belongs to the chart
    /// columns, not to a particular bar, so a GPU chart gets it for free once it
    /// has bars.
    fn select_module_frame_at(&mut self, x: f32, session: &ProfileSession) {
        let Some((take, capture)) = self.module_axis.source_at_screen(x) else {
            return;
        };
        let Some(index) = frame_at_capture(&self.module_axis, session, take, capture) else {
            return;
        };
        self.selected_frame = index;
        self.follow_latest = false;
    }

    /// One module title row: badge artwork, name, and the current-value chip.
    ///
    /// The badge is a skin texture (`icons.cpu_badge` / `icons.gpu_badge`): the
    /// mark's box and placement are style tokens, so the artwork itself decides
    /// what the module's identity looks like and a skin can swap it without
    /// touching this row.
    fn paint_module_header(
        &self,
        ui: &Ui,
        header: Rect,
        name_col: Rect,
        badge: &str,
        title: &str,
        value: &str,
    ) {
        let painter = ui.painter();
        let badge_rect = Rect::from_min_size(
            egui::pos2(
                header.left() + self.style.module_badge_inset,
                header.center().y - self.style.module_badge_height * 0.5,
            ),
            egui::vec2(
                self.style.module_badge_width.max(0.0),
                self.style.module_badge_height.max(0.0),
            ),
        );
        // The artwork is pre-coloured for its skin, so the tint is white.
        ui_image::paint(ui, badge_rect, badge, egui::Color32::WHITE);
        painter.text(
            egui::pos2(
                badge_rect.right() + self.style.module_badge_gap,
                header.center().y,
            ),
            Align2::LEFT_CENTER,
            title,
            self.toolbar_font(),
            self.style.timeline_text.to_egui(),
        );

        // Value chip at the start of the chart area, like the reference.
        self.paint_chip(
            ui,
            egui::pos2(name_col.right() + 8.0, header.center().y),
            value,
        );
    }
}

/// The frame recorded at `capture` inside the take with index `take`, for a
/// click on a module chart.
///
/// A click between two frames (or in the margin before a take's first frame)
/// falls back to the neighbouring frame of the same take; a click that maps to
/// another take selects nothing. The axis already rejected clicks on a break
/// band.
fn frame_at_capture(
    axis: &ModuleAxis,
    session: &ProfileSession,
    take: usize,
    capture: f64,
) -> Option<usize> {
    let frames = &session.frames;
    if frames.is_empty() {
        return None;
    }
    let in_take = |index: usize| -> bool {
        frames.get(index).is_some_and(|frame| {
            axis.time_of_source(frame.start_ns as f64)
                .is_some_and(|(frame_take, _)| frame_take == take)
        })
    };
    let next = frames.partition_point(|frame| (frame.start_ns as f64) <= capture);
    if let Some(index) = next.checked_sub(1) {
        if in_take(index) {
            return Some(index);
        }
    }
    if in_take(next) {
        return Some(next);
    }
    None
}
