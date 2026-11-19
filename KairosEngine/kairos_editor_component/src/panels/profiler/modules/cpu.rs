use egui::{Align2, CornerRadius, Rect, Sense, Ui};
use kairos_editor_widgets::color::ColorExt;
use kairos_profile::{Category, FrameSnapshot, ProfileSession};
use strum::IntoEnumIterator;

use crate::panels::profiler::ProfilerPanel;
use crate::panels::profiler::frame_ms;
use crate::panels::profiler::modules::ChartColumn;

/// The chart's vertical scale clamps into this range, (0.11 ms to 70 ms):
/// one hitch cannot flatten the whole graph.
const MIN_USAGE_SCALE_MS: f32 = 0.11;
const MAX_USAGE_SCALE_MS: f32 = 70.0;

/// How much of the remaining gap the chart's scale closes per pass.
const USAGE_SCALE_EASE: f32 = 0.4;

/// Module-chart guide lines: four bands chosen by the chart's maximum,
/// each with three ms/FPS readings. The two faster bands use the engine's
/// refresh targets (60/120/240 FPS, 120/240/960 FPS); the slowest keeps Unity's.
const USAGE_GRID_FAST: [(f64, &str); 3] = [
    (1.0, "1ms (1000FPS)"),
    (0.25, "0.25ms (4000FPS)"),
    (0.1, "0.1ms (10000FPS)"),
];
const USAGE_GRID_10MS: [(f64, &str); 3] = [
    (1000.0 / 120.0, "8ms (120FPS)"),
    (1000.0 / 240.0, "4ms (240FPS)"),
    (1000.0 / 960.0, "1ms (960FPS)"),
];
const USAGE_GRID_30MS: [(f64, &str); 3] = [
    (1000.0 / 60.0, "16ms (60FPS)"),
    (1000.0 / 120.0, "8ms (120FPS)"),
    (1000.0 / 240.0, "4ms (240FPS)"),
];
const USAGE_GRID_SLOW: [(f64, &str); 3] = [
    (1000.0 / 15.0, "66ms (15FPS)"),
    (1000.0 / 30.0, "33ms (30FPS)"),
    (1000.0 / 60.0, "16ms (60FPS)"),
];

/// The guide lines for a chart whose vertical scale is `scale_ms`.
fn usage_grid(scale_ms: f32) -> &'static [(f64, &'static str)] {
    if scale_ms < 1.5 {
        &USAGE_GRID_FAST
    } else if scale_ms < 10.0 {
        &USAGE_GRID_10MS
    } else if scale_ms < 30.0 {
        &USAGE_GRID_30MS
    } else {
        &USAGE_GRID_SLOW
    }
}

impl ProfilerPanel {
    pub(super) fn paint_cpu_module(&mut self, ui: &mut Ui, row: Rect, session: &ProfileSession) {
        let name_width = self.style.module_name_width.min(row.width());
        let name_col =
            Rect::from_min_max(row.min, egui::pos2(row.left() + name_width, row.bottom()));
        // The chart starts below the module title row.
        let chart = Rect::from_min_max(egui::pos2(name_col.right(), row.top()), row.max);
        let header = Rect::from_min_size(
            row.min,
            egui::vec2(row.width(), self.style.module_header_height),
        );
        let separator_color = self.style.separator.to_egui();

        // Column rule between the legend and the chart, inside the name column.
        self.paint_vertical_rule(
            ui,
            egui::pos2(name_col.right() - self.style.separator_width, row.top()),
            row.height(),
            separator_color,
        );

        let value = session
            .frames
            .get(self.selected_frame)
            .map_or_else(|| "no frames".to_owned(), |frame| frame_cost_label(frame));
        self.paint_module_header(
            ui,
            header,
            name_col,
            &self.style.icons.cpu_badge,
            "CPU Usage",
            &value,
        );

        // Legend: Unity's category rows with the selected frame's cost, then the
        // frame's Scripts systems by cost. Zero-cost categories keep their row,
        // like Unity: the legend is a fixed vocabulary, not a ranking.
        let frame = session.frames.get(self.selected_frame);
        let visible = ((row.height() - self.style.module_header_height)
            / self.style.legend_row_height.max(1.0))
        .floor()
        .max(0.0) as usize;
        let labels = ui.painter().with_clip_rect(name_col);
        let row_center = |index: usize| {
            egui::pos2(
                name_col.left() + 12.0,
                row.top()
                    + self.style.module_header_height
                    + (index as f32 + 0.5) * self.style.legend_row_height,
            )
        };
        let swatch_width = self.style.legend_row_height * 0.7;
        let swatch_label_space = swatch_width * 2.3;
        let mut used = 0;
        for category in Category::iter().take(visible) {
            let center = row_center(used);
            let row_rect = Rect::from_min_max(
                egui::pos2(
                    name_col.left(),
                    center.y - self.style.legend_row_height * 0.5,
                ),
                egui::pos2(
                    name_col.right(),
                    center.y + self.style.legend_row_height * 0.5,
                ),
            );
            // Legend swatches are toggles: a click takes the category
            // out of the chart and leaves the row behind, dimmed.
            let response = ui.interact(
                row_rect,
                ui.id().with(("profiler.category", category as usize)),
                Sense::click(),
            );
            if response.hovered() {
                ui.painter().rect_filled(
                    row_rect,
                    CornerRadius::ZERO,
                    self.style.hover_bg.to_egui(),
                );
            }
            if response.clicked() {
                let hidden = &mut self.hidden[category as usize];
                *hidden = !*hidden;
            }
            let hidden = self.hidden[category as usize];
            let swatch = self.category_color(category);
            labels.rect_filled(
                Rect::from_center_size(center, egui::vec2(swatch_width, swatch_width)),
                CornerRadius::same(1),
                if hidden {
                    swatch.gamma_multiply(0.3)
                } else {
                    swatch
                },
            );
            labels.text(
                egui::pos2(name_col.left() + swatch_label_space, center.y),
                Align2::LEFT_CENTER,
                category.label(),
                self.toolbar_font(),
                if hidden {
                    self.style.ruler_text.to_egui()
                } else {
                    self.style.timeline_text.to_egui()
                },
            );
            if let Some(frame) = frame {
                labels.text(
                    egui::pos2(name_col.right() - swatch_width, center.y),
                    Align2::RIGHT_CENTER,
                    format!("{:.2}ms", self.category_ms(frame, category)),
                    self.small_font(),
                    self.style.ruler_text.to_egui(),
                );
            }
            used += 1;
        }

        // Systems get whatever rows are left. The full ranking belongs in a
        // details pane; this is the top of it, which is
        // the refinement of Unity's `Scripts` category.
        let systems = self.system_entries();
        let rows_left = visible.saturating_sub(used);
        if rows_left > 0 && !systems.is_empty() && !self.hidden[Category::Scripts as usize] {
            let shown = if systems.len() > rows_left {
                rows_left.saturating_sub(1)
            } else {
                systems.len()
            };
            for (name, ns) in systems.iter().take(shown) {
                let center = row_center(used);
                labels.rect_filled(
                    Rect::from_center_size(center, egui::vec2(swatch_width, swatch_width)),
                    CornerRadius::same(1),
                    self.category_color(Category::Scripts),
                );
                labels.text(
                    egui::pos2(name_col.left() + swatch_label_space, center.y),
                    Align2::LEFT_CENTER,
                    name,
                    self.toolbar_font(),
                    self.style.timeline_text.to_egui(),
                );
                labels.text(
                    egui::pos2(name_col.right() - swatch_width, center.y),
                    Align2::RIGHT_CENTER,
                    format!("{:.2}ms", *ns as f32 / 1_000_000.0),
                    self.small_font(),
                    self.style.ruler_text.to_egui(),
                );
                used += 1;
            }
            if systems.len() > shown {
                let center = row_center(used);
                labels.text(
                    egui::pos2(name_col.left() + swatch_label_space, center.y),
                    Align2::LEFT_CENTER,
                    format!("+{} more", systems.len() - shown),
                    self.small_font(),
                    self.style.ruler_text.to_egui(),
                );
            }
        }

        // Clicking or dragging the chart selects the frame recorded at that
        // time: the gesture is about the time column, not about hitting a bar,
        // so a GPU chart gets it for free once it has bars. Dragging scrubs the
        // selection, which is also how the selected-frame cursor moves.
        let response = ui.interact(
            chart,
            ui.id().with("profiler.module.cpu"),
            Sense::click_and_drag(),
        );
        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let x = (pos.x - chart.left()).clamp(0.0, chart.width());
                self.select_module_frame_at(x, session);
            }
        }
        if response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        } else if response.hovered() {
            let pointer = ui.input(|input| input.pointer.hover_pos());
            if let (Some(pointer), Some(cursor)) = (pointer, self.module_cursor_column(session)) {
                if (pointer.x - chart.left() - cursor).abs() <= 3.0 {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
            }
        }
        self.paint_cpu_chart(ui, chart, session);
    }

    /// The usage chart: one stacked bar per frame, positioned on the module
    /// area's time axis so the bars line up with the clock ruler above them,
    /// with Unity's ms/FPS guide lines and the selected-frame cursor.
    ///
    /// The vertical scale is the tallest visible bar divided by the 80% fill
    /// target (clamped to Unity's module-chart range), so the graph uses the
    /// chart's height whatever the frame rate is.
    fn paint_cpu_chart(&mut self, ui: &Ui, plot: Rect, session: &ProfileSession) {
        if plot.width() <= 0.0 || plot.height() <= 0.0 {
            return;
        }
        let painter = ui.painter().with_clip_rect(plot);
        let text = self.style.ruler_text.to_egui();
        if session.frames.is_empty() || self.module_axis.is_empty() {
            painter.text(
                plot.center(),
                Align2::CENTER_CENTER,
                "Waiting for the first captured frame…",
                self.small_font(),
                text,
            );
            return;
        }

        // Where every visible frame's column is. A bar's left edge follows its
        // frame start, so the bars sit at the times the ruler labels.
        let zoom = self.module_axis.zoom();
        let mut columns: Vec<ChartColumn> = Vec::new();
        for segment in self.module_axis.visible_segments() {
            let (capture_start, capture_end) = segment.source;
            let frames = &session.frames;
            let lo = frames.partition_point(|frame| (frame.end_ns as f64) <= capture_start);
            let hi = frames.partition_point(|frame| (frame.start_ns as f64) <= capture_end);
            for (offset, frame) in frames[lo..hi].iter().enumerate() {
                let x = segment.x.0 + ((frame.start_ns as f64 - capture_start) * zoom) as f32;
                let width = (((frame.end_ns - frame.start_ns) as f64 * zoom) as f32).max(1.0);
                columns.push(ChartColumn {
                    frame: lo + offset,
                    x,
                    width,
                    ms: self.bar_ms(frame),
                });
            }
        }

        // While recording, the tallest bar can change every pass; the scale
        // eases towards its target instead of snapping. Its first measurement
        // snaps, and a cleared capture resets it to that state.
        let tallest = columns
            .iter()
            .map(|column| column.ms)
            .fold(0.0_f32, f32::max);
        let target = (tallest.max(0.001) / self.style.usage_bar_fill.max(0.1))
            .clamp(MIN_USAGE_SCALE_MS, MAX_USAGE_SCALE_MS);
        if self.module_scale_ms <= 0.0 {
            self.module_scale_ms = target;
        } else {
            let next = self.module_scale_ms + (target - self.module_scale_ms) * USAGE_SCALE_EASE;
            self.module_scale_ms = if (next - target).abs() < 0.005 {
                target
            } else {
                next
            };
            if (self.module_scale_ms - target).abs() > 0.005 {
                ui.ctx().request_repaint();
            }
        }
        let scale = self.module_scale_ms.max(0.001);

        // Break bands first: they are the compressed pauses, and two takes'
        // bars never touch across them.
        for band in self.module_axis.breaks() {
            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(plot.left() + band.x, plot.top()),
                    egui::vec2(band.width, plot.height()),
                ),
                CornerRadius::ZERO,
                self.style.break_bg.to_egui(),
            );
        }

        // Clock grid: a faint vertical line under every labelled ruler tick, so
        // a bar reads against the time axis above it.
        let grid = self.style.chart_grid.to_egui().gamma_multiply(0.45);
        let ruler = self.module_axis.ruler_scale();
        for tick in self
            .module_axis
            .ticks(&ruler)
            .into_iter()
            .filter(|tick| tick.major)
        {
            let x = plot.left() + tick.x;
            if x <= plot.left() || x >= plot.right() {
                continue;
            }
            painter.rect_filled(
                Rect::from_min_size(egui::pos2(x, plot.top()), egui::vec2(1.0, plot.height())),
                CornerRadius::ZERO,
                grid,
            );
        }

        // One stacked bar per frame. Unity anchors Others at the bottom and
        // stacks up the legend, so the first row is the topmost band.
        for column in &columns {
            let frame = &session.frames[column.frame];
            let mut bottom = plot.bottom();
            for category in Category::iter().rev() {
                if self.hidden[category as usize] {
                    continue;
                }
                let ms = self.category_ms(frame, category);
                if ms <= 0.0 {
                    continue;
                }
                let height = (ms / scale * plot.height()).max(0.5);
                painter.rect_filled(
                    Rect::from_min_max(
                        egui::pos2(plot.left() + column.x, bottom - height),
                        egui::pos2(plot.left() + column.x + column.width, bottom),
                    ),
                    CornerRadius::ZERO,
                    self.category_color(category),
                );
                bottom -= height;
            }
        }

        // ms/FPS guide lines, placed under the 80% scale. They are
        // drawn over the bars so their chips stay readable. A pane barely taller
        // than its card header leaves a short plot that bunches the tiers up, so
        // a chip whose label would collide with the one above it steps aside and
        // leaves its line — the way the clock ruler drops colliding labels.
        let mut reserved: Vec<(f32, f32)> = Vec::new();
        for (ms, label) in usage_grid(scale) {
            if *ms > f64::from(scale) {
                continue;
            }
            let y = plot.bottom() - (*ms as f32 / scale * plot.height());
            self.paint_chart_guide(ui, plot, y, label, &mut reserved);
        }
    }

    /// The selected frame's Scripts systems, highest first.
    fn system_entries(&self) -> &[(String, u64)] {
        self.frame
            .as_ref()
            .map_or(&[], |frame| frame.systems.as_slice())
    }
}

/// Module chip: `6.2ms (161FPS)`. Sub-0.1 ms frames are reported as
/// `<0.1ms`: the FPS equivalent would be noise.
fn frame_cost_label(frame: &FrameSnapshot) -> String {
    let ms = frame_ms(frame);
    if ms < 0.1 {
        "<0.1ms".to_owned()
    } else {
        format!("{ms:.1}ms ({:.0}FPS)", 1000.0 / ms)
    }
}
