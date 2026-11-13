//! Console panel: the Unity/Tuanjie-style log console.
//!
//! Visual reference: the real Unity/Tuanjie 2022.3 console, captured and
//! measured in `docs/research/unity-console/README.md`. The panel is a pure view
//! over the host-owned `kairos_log` history; it keeps only ephemeral view state
//! (search, collapse, level filters, cleared cursor, selected row).
//!
//! Layout follows the reference: a 20 px toolbar (Clear / Collapse /
//! Error Pause / Editor, search, per-level count toggles) whose segments are
//! split by 1 px chrome lines, then the log list of
//! 38 px two-line entries with alternating backgrounds, then — for the selected
//! entry — the 130 px stack-trace detail pane. Clearing is a view cursor
//! (`cleared_through`), not a mutation of the host buffer.
//!
//! Appearance comes from [`ConsoleStyle`], loaded from
//! `Preferences/Styles/ConsoleStyle.ron` when this panel is initialized
//! ([`Panel::init`]). The style file holds dark + light skins; icon file paths
//! are part of it and are preloaded through `kairos_editor_widgets::ui_image`.
//! This module only wires host data, view state and interaction.

use std::collections::HashMap;

use egui::{
    Align2, CornerRadius, FontFamily, FontId, Pos2, Rect, Sense, Stroke, Ui, WidgetInfo,
    WidgetText, WidgetType, pos2, vec2,
};
use kairos_editor_core::PanelId;
use kairos_editor_widgets::color::Color32Ext;
use kairos_editor_widgets::icons::ConsoleSeverity;
use kairos_editor_widgets::style::StyleFile;
use kairos_editor_widgets::ui_image;
use kairos_log::Level;

use crate::frame::{ConsoleLine, Frame, UiOut};
use crate::panel::Panel;

use super::CONSOLE;
use super::console_style::ConsoleStyle;

/// Console panel view state. Everything here is disposable: losing it on a
/// panel close resets the console UI, never the log history.
pub(crate) struct ConsolePanel {
    search: String,
    collapse: bool,
    error_pause: bool,
    /// Which levels are visible; the count widgets double as toggles.
    levels: [bool; 3],
    /// Entries with `id <= cleared_through` are hidden by the Clear button.
    cleared_through: u64,
    selected: Option<u64>,
    /// New entries become the selection until the user clicks a row.
    follow_latest: bool,
    /// Loaded from `Preferences/Styles/ConsoleStyle.ron` in [`Panel::init`].
    style: ConsoleStyle,
}

impl Default for ConsolePanel {
    fn default() -> Self {
        Self {
            search: String::new(),
            collapse: false,
            error_pause: false,
            levels: [true; 3],
            cleared_through: 0,
            selected: None,
            follow_latest: true,
            style: ConsoleStyle::default(),
        }
    }
}

impl Panel for ConsolePanel {
    fn kind(&self) -> PanelId {
        CONSOLE
    }

    fn icon(&self) -> Option<&str> {
        Some(self.style.tab_icon.as_str())
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        "Console".into()
    }

    fn init(&mut self, ctx: &egui::Context) {
        match StyleFile::<ConsoleStyle>::load_or_default() {
            Ok(file) => self.style = file.dark,
            Err(error) => {
                kairos_log::warn!(target: "kairos_editor", "console style: {error}");
            }
        }

        ui_image::preload(ctx, &self.style.tab_icon);
        for path in self.style.icons.all() {
            ui_image::preload(ctx, path);
        }
    }

    fn ui(&mut self, ui: &mut Ui, frame: &Frame, _out: &mut UiOut) {
        let mut lines = Vec::new();
        frame
            .host
            .console_lines(&mut |entries| lines = entries.to_vec());

        ui.painter().rect_filled(
            ui.max_rect(),
            CornerRadius::ZERO,
            self.style.row_even.to_egui(),
        );
        self.toolbar(ui, &lines);

        let body = ui.available_rect_before_wrap();
        // Explicit ids keep egui's multi-pass id bookkeeping stable (the dock
        // can run the UI twice per frame; auto ids would clash between passes).
        let _ = ui.interact(body, ui.id().with("console-body"), Sense::hover());
        self.resolve_selection(&lines);

        let rows = self.rows(&lines);
        let detail_height = if rows.is_empty() {
            0.0
        } else {
            self.style
                .detail_height
                .min((body.height() - self.style.row_height).max(0.0))
        };
        let list_rect = Rect::from_min_max(body.min, pos2(body.max.x, body.max.y - detail_height));
        let detail_rect =
            Rect::from_min_max(pos2(body.min.x, body.max.y - detail_height), body.max);

        ui.scope_builder(egui::UiBuilder::new().max_rect(list_rect), |ui| {
            self.log_list(ui, &rows);
        });
        if detail_height > 0.0 {
            ui.scope_builder(egui::UiBuilder::new().max_rect(detail_rect), |ui| {
                self.detail_pane(ui, detail_rect, &lines);
            });
        }
    }
}

impl ConsolePanel {
    fn counts(&self, lines: &[ConsoleLine]) -> [usize; 3] {
        let mut counts = [0; 3];
        for line in lines.iter().filter(|line| line.id > self.cleared_through) {
            counts[level_index(line.level)] += 1;
        }
        counts
    }

    /// Unity sizes the three count toggles to their content (measured: 40 px for
    /// `39`, 49 px for `303`, 32 px for `5`); the style token is the floor so a
    /// toggle never paints its number over the next one.
    fn count_widths(&self, painter: &egui::Painter, counts: &[usize; 3]) -> [f32; 3] {
        let font = FontId::new(self.style.text_size, FontFamily::Proportional);
        let color = self.style.toolbar_text.to_egui();
        (*counts).map(|count| {
            let text = painter.layout_no_wrap(count.to_string(), font.clone(), color);
            self.style
                .count_widths
                .max(5.0 + self.style.count_icon + 5.0 + text.size().x + 5.0)
        })
    }

    fn toolbar(&mut self, ui: &mut Ui, lines: &[ConsoleLine]) {
        let width = ui.available_width();
        // The bar is 20 px tall and is followed by Unity's 1 px chrome line
        // above the log list, so the row reserves one extra pixel below it.
        let (row, _) = ui.allocate_exact_size(
            vec2(width, self.style.toolbar_height + self.style.separator_width),
            Sense::hover(),
        );
        let bar = Rect::from_min_size(row.min, vec2(width, self.style.toolbar_height));
        let painter = ui.painter().clone();
        painter.rect_filled(bar, CornerRadius::ZERO, self.style.toolbar_bg.to_egui());
        painter.rect_filled(
            Rect::from_min_size(pos2(bar.left(), bar.bottom()), vec2(width, self.style.separator_width)),
            CornerRadius::ZERO,
            self.style.separator.to_egui(),
        );

        let font = FontId::new(self.style.text_size, FontFamily::Proportional);
        let mut x = bar.left();

        // Clear (split button: label + caret menu).
        let clear = Rect::from_min_size(
            pos2(x, bar.top()),
            vec2(self.style.clear_width, bar.height()),
        );
        let response = ui.interact(clear, ui.id().with("console-clear"), Sense::click());
        paint_hover(&painter, &response, clear, false, &self.style);
        painter.text(
            pos2(clear.left() + 5.0, clear.center().y),
            Align2::LEFT_CENTER,
            "Clear",
            font.clone(),
            self.style.toolbar_text.to_egui(),
        );
        paint_caret(
            &painter,
            pos2(clear.right() - 10.0, clear.center().y),
            self.style.toolbar_text.to_egui(),
        );
        toolbar_widget_info(&response, WidgetType::Button, "Clear", None);
        if response.clicked() {
            self.cleared_through = lines.last().map_or(0, |line| line.id);
            self.selected = None;
            self.follow_latest = true;
        }
        x = toolbar_separator(&painter, bar, clear.right(), &self.style);

        // Collapse / Error Pause toggles.
        let collapse = Rect::from_min_size(
            pos2(x, bar.top()),
            vec2(self.style.collapse_width, bar.height()),
        );
        let response = ui.interact(collapse, ui.id().with("console-collapse"), Sense::click());
        paint_hover(&painter, &response, collapse, self.collapse, &self.style);
        painter.text(
            collapse.center(),
            Align2::CENTER_CENTER,
            "Collapse",
            font.clone(),
            self.style.toolbar_text.to_egui(),
        );
        toolbar_widget_info(
            &response,
            WidgetType::SelectableLabel,
            "Collapse",
            Some(self.collapse),
        );
        if response.clicked() {
            self.collapse = !self.collapse;
        }
        x = toolbar_separator(&painter, bar, collapse.right(), &self.style);

        let pause = Rect::from_min_size(
            pos2(x, bar.top()),
            vec2(self.style.error_pause_width, bar.height()),
        );
        let response = ui.interact(pause, ui.id().with("console-error-pause"), Sense::click());
        paint_hover(&painter, &response, pause, self.error_pause, &self.style);
        painter.text(
            pause.center(),
            Align2::CENTER_CENTER,
            "Error Pause",
            font.clone(),
            self.style.toolbar_text.to_egui(),
        );
        toolbar_widget_info(
            &response,
            WidgetType::SelectableLabel,
            "Error Pause",
            Some(self.error_pause),
        );
        if response.clicked() {
            self.error_pause = !self.error_pause;
        }
        x = toolbar_separator(&painter, bar, pause.right(), &self.style);

        // Editor/Player source filter (Unity's Editor dropdown).
        let editor = Rect::from_min_size(
            pos2(x, bar.top()),
            vec2(self.style.editor_width, bar.height()),
        );
        let response = ui.interact(editor, ui.id().with("console-source"), Sense::click());
        paint_hover(&painter, &response, editor, false, &self.style);
        painter.text(
            pos2(editor.left() + 8.0, editor.center().y),
            Align2::LEFT_CENTER,
            "Editor",
            font.clone(),
            self.style.toolbar_text.to_egui(),
        );
        paint_caret(
            &painter,
            pos2(editor.right() - 13.0, editor.center().y),
            self.style.toolbar_text.to_egui(),
        );
        toolbar_widget_info(&response, WidgetType::Button, "Editor", None);
        x = toolbar_separator(&painter, bar, editor.right(), &self.style);

        // Search field fills the middle; the count buttons are pinned right.
        let counts = self.counts(lines);
        let count_widths = self.count_widths(&painter, &counts);
        let counts_width: f32 =
            count_widths.iter().sum::<f32>() + (Level::ALL.len() as f32 - 1.0) * self.style.separator_width;
        let counts_left = bar.right() - counts_width;
        let search = Rect::from_min_max(
            pos2(x + 6.0, bar.top() + 2.0),
            pos2(counts_left - 3.0, bar.bottom() - 2.0),
        );
        painter.rect_filled(
            search,
            CornerRadius::same(self.style.search_radius),
            self.style.search_bg.to_egui(),
        );
        painter.rect_stroke(
            search,
            CornerRadius::same(self.style.search_radius),
            Stroke::new(1.0, self.style.search_edge.to_egui()),
            egui::StrokeKind::Inside,
        );
        paint_magnifier(
            &painter,
            pos2(search.left() + 9.0, search.center().y),
            self.style.toolbar_text.to_egui(),
        );
        let edit = Rect::from_min_max(
            pos2(search.left() + 18.0, search.top()),
            pos2(search.right() - 4.0, search.bottom()),
        );
        ui.put(
            edit,
            egui::TextEdit::singleline(&mut self.search)
                .id(ui.id().with("console-search"))
                .font(FontId::new(self.style.text_size, FontFamily::Proportional))
                .text_color(self.style.toolbar_text.to_egui())
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::symmetric(0, 1)),
        );

        self.paint_counts(ui, bar, counts_left, &counts, &count_widths);
    }

    fn paint_counts(
        &mut self,
        ui: &mut Ui,
        bar: Rect,
        counts_left: f32,
        counts: &[usize; 3],
        widths: &[f32; 3],
    ) {
        let painter = ui.painter().clone();
        // A segmented control: one separator leads into the group, then one
        // between every pair of toggles; Unity draws all of them as chrome lines.
        let mut x = toolbar_separator(&painter, bar, counts_left - self.style.separator_width, &self.style);

        let font = FontId::new(self.style.text_size, FontFamily::Proportional);
        for (index, level) in Level::ALL.into_iter().enumerate() {
            let segment =
                Rect::from_min_size(pos2(x, bar.top()), vec2(widths[index], bar.height()));
            let response = ui.interact(
                segment,
                ui.id().with(("console-level", index)),
                Sense::click(),
            );
            if response.clicked() {
                self.levels[index] = !self.levels[index];
            }

            let visible = self.levels[index];

            let bg = if visible {
                self.style.toggle_bg
            } else {
                self.style.toolbar_bg
            };
            painter.rect_filled(segment, CornerRadius::ZERO, bg.to_egui());

            let label = format!("{} {}", level_name(level), counts[index]);
            toolbar_widget_info(
                &response,
                WidgetType::SelectableLabel,
                &label,
                Some(visible),
            );
            let icon_rect = Rect::from_min_size(
                pos2(
                    segment.left() + 5.0,
                    segment.center().y - self.style.count_icon * 0.5,
                ),
                vec2(self.style.count_icon, self.style.count_icon),
            );
            ui_image::paint(
                ui,
                icon_rect,
                self.style.icons.severity(severity(level), visible),
                egui::Color32::WHITE,
            );
            painter.text(
                pos2(icon_rect.right() + 5.0, segment.center().y),
                Align2::LEFT_CENTER,
                counts[index].to_string(),
                font.clone(),
                self.style.toolbar_text.to_egui(),
            );

            x = if index < 2 {
                toolbar_separator(&painter, bar, segment.right(), &self.style)
            } else {
                segment.right()
            };
        }
    }

    fn log_list(&mut self, ui: &mut Ui, rows: &[(&ConsoleLine, usize)]) {
        egui::ScrollArea::vertical()
            .id_salt("kairos-console-log")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (index, (line, count)) in rows.iter().enumerate() {
                    self.row(ui, index, line, *count);
                }
            });
    }

    fn rows<'a>(&self, lines: &'a [ConsoleLine]) -> Vec<(&'a ConsoleLine, usize)> {
        let needle = self.search.to_ascii_lowercase();
        let visible = |line: &ConsoleLine| {
            line.id > self.cleared_through
                && self.levels[level_index(line.level)]
                && (needle.is_empty() || line.message.to_ascii_lowercase().contains(&needle))
        };

        let mut rows: Vec<(&ConsoleLine, usize)> = Vec::new();
        let mut collapsed: HashMap<(Level, &str), usize> = HashMap::new();
        for line in lines.iter().filter(|line| visible(line)) {
            if self.collapse {
                let key = (line.level, line.message.as_str());
                if let Some(&index) = collapsed.get(&key) {
                    // Collapsed entries keep their first position but show the
                    // newest occurrence (time) and a repeat badge.
                    rows[index].0 = line;
                    rows[index].1 += 1;
                    continue;
                }
                collapsed.insert(key, rows.len());
            }
            rows.push((line, 1));
        }
        rows
    }

    fn row(&mut self, ui: &mut Ui, index: usize, line: &ConsoleLine, count: usize) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(width, self.style.row_height), Sense::hover());
        let response = ui.interact(rect, ui.id().with(("console-row", line.id)), Sense::click());
        if response.clicked() {
            self.selected = Some(line.id);
            self.follow_latest = false;
        }

        let selected = self.selected == Some(line.id);
        let background = if selected {
            self.style.row_selected.to_egui()
        } else if index % 2 == 0 {
            self.style.row_odd.to_egui()
        } else {
            self.style.row_even.to_egui()
        };
        let text_color = if selected {
            self.style.text_selected.to_egui()
        } else {
            self.style.text.to_egui()
        };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::ZERO, background);

        let icon_rect = Rect::from_min_size(
            pos2(rect.left() + self.style.icon_left, rect.top() + 6.0),
            vec2(self.style.icon_size, self.style.icon_size),
        );
        ui_image::paint(
            ui,
            icon_rect,
            self.style.icons.severity(severity(line.level), true),
            egui::Color32::WHITE,
        );

        let font = FontId::new(self.style.text_size, FontFamily::Proportional);
        let text = format!("[{}] {}", line.time, line.message);
        let galley = painter.layout(
            text,
            font,
            text_color,
            rect.width() - self.style.text_left - 10.0,
        );
        let clip = Rect::from_min_max(
            pos2(
                rect.left() + self.style.text_left,
                rect.top() + self.style.row_text_top - 2.0,
            ),
            pos2(rect.right() - 10.0, rect.bottom()),
        );
        let text_painter = painter.with_clip_rect(clip);
        text_painter.galley(
            pos2(
                rect.left() + self.style.text_left,
                rect.top() + self.style.row_text_top,
            ),
            galley,
            text_color,
        );

        if self.collapse && count > 1 {
            let badge_rect = Rect::from_min_size(
                pos2(
                    rect.right() - self.style.badge_size - 8.0,
                    rect.center().y - self.style.badge_size * 0.5,
                ),
                vec2(self.style.badge_size, self.style.badge_size),
            );
            ui_image::paint(
                ui,
                badge_rect,
                self.style.icons.badge(),
                egui::Color32::WHITE,
            );
            let badge_text = if count > 99 {
                "99+".to_owned()
            } else {
                count.to_string()
            };
            painter.text(
                badge_rect.center(),
                Align2::CENTER_CENTER,
                badge_text,
                FontId::new(self.style.badge_text_size, FontFamily::Proportional),
                self.style.text.to_egui(),
            );
        }
    }

    fn detail_pane(&mut self, ui: &mut Ui, rect: Rect, lines: &[ConsoleLine]) {
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::ZERO, self.style.detail_bg.to_egui());
        painter.rect_filled(
            Rect::from_min_size(rect.min, vec2(rect.width(), 1.0)),
            CornerRadius::ZERO,
            self.style.separator.to_egui(),
        );

        let Some(line) = self.selected_line(lines) else {
            return;
        };
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect.shrink2(vec2(self.style.detail_padding, 5.0))),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("kairos-console-detail")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let font = FontId::new(self.style.text_size, FontFamily::Proportional);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&line.message)
                                    .font(font.clone())
                                    .color(self.style.text.to_egui()),
                            )
                            .wrap(),
                        );
                        for stack_line in &line.stack {
                            paint_stack_line(ui, stack_line, &self.style);
                        }
                    });
            },
        );
    }

    /// Keeps the selection valid: falls back to the newest visible entry so the
    /// detail pane always shows something meaningful.
    fn resolve_selection(&mut self, lines: &[ConsoleLine]) {
        let valid = |line: &ConsoleLine| {
            line.id > self.cleared_through
                && self.levels[level_index(line.level)]
                && !line.message.is_empty()
        };
        if !self.follow_latest
            && self
                .selected
                .and_then(|id| lines.iter().find(|line| line.id == id))
                .is_some_and(valid)
        {
            return;
        }
        self.selected = lines
            .iter()
            .rev()
            .find(|line| valid(line))
            .map(|line| line.id);
    }

    fn selected_line<'a>(&self, lines: &'a [ConsoleLine]) -> Option<&'a ConsoleLine> {
        let id = self.selected?;
        lines.iter().find(|line| line.id == id)
    }
}

/// Paints Unity's 1 px chrome separator at `x` and returns where the next
/// toolbar segment starts.
fn toolbar_separator(painter: &egui::Painter, bar: Rect, x: f32, style: &ConsoleStyle) -> f32 {
    painter.rect_filled(
        Rect::from_min_size(pos2(x, bar.top()), vec2(style.separator_width, bar.height())),
        CornerRadius::ZERO,
        style.separator.to_egui(),
    );
    x + style.separator_width
}

/// Registers a hand-painted control with egui. A painter-only control is missing
/// from the accessibility tree and from label/role queries; the built-in widgets
/// report this automatically.
fn toolbar_widget_info(
    response: &egui::Response,
    widget: WidgetType,
    label: &str,
    selected: Option<bool>,
) {
    response.widget_info(|| match selected {
        Some(selected) => WidgetInfo::selected(widget, true, selected, label),
        None => WidgetInfo::labeled(widget, true, label),
    });
}

fn paint_stack_line(ui: &mut Ui, line: &str, style: &ConsoleStyle) {
    let font = FontId::new(style.text_size, FontFamily::Proportional);

    // Unity colours the trailing `(at file:line)` of a frame as a source link
    // while the symbol itself stays grey.
    match line.find(" (at ") {
        Some(index) => {
            let symbol = &line[..index];
            let location = &line[index + 1..];
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(symbol)
                            .font(font.clone())
                            .color(style.text.to_egui()),
                    )
                    .wrap(),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!(" {location}"))
                            .font(font)
                            .color(style.link.to_egui()),
                    )
                    .wrap(),
                );
            });
        }
        None => {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(line)
                        .font(font)
                        .color(style.text.to_egui()),
                )
                .wrap(),
            );
        }
    }
}

fn level_index(level: Level) -> usize {
    match level {
        Level::Info => 0,
        Level::Warning => 1,
        Level::Error => 2,
    }
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::Info => "Info",
        Level::Warning => "Warning",
        Level::Error => "Error",
    }
}

fn severity(level: Level) -> ConsoleSeverity {
    match level {
        Level::Info => ConsoleSeverity::Info,
        Level::Warning => ConsoleSeverity::Warning,
        Level::Error => ConsoleSeverity::Error,
    }
}

fn paint_hover(
    painter: &egui::Painter,
    response: &egui::Response,
    rect: Rect,
    active: bool,
    style: &ConsoleStyle,
) {
    if active || response.hovered() {
        painter.rect_filled(rect, CornerRadius::ZERO, style.toggle_bg.to_egui());
    }
}

fn paint_caret(painter: &egui::Painter, center: Pos2, color: egui::Color32) {
    let points = vec![
        pos2(center.x - 3.5, center.y - 1.5),
        pos2(center.x + 3.5, center.y - 1.5),
        pos2(center.x, center.y + 2.5),
    ];
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

fn paint_magnifier(painter: &egui::Painter, center: Pos2, color: egui::Color32) {
    painter.circle_stroke(center + vec2(-1.0, -1.0), 4.0, Stroke::new(1.2, color));
    painter.line_segment(
        [center + vec2(1.8, 1.8), center + vec2(4.2, 4.2)],
        Stroke::new(1.2, color),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::HostView;
    use kairos_editor_core::EditorState;
    use kairos_editor_widgets::style::Style;

    struct EmptyHost;

    impl HostView for EmptyHost {}

    /// Host with a fixed console history, for painting tests that need lines.
    struct LinesHost(Vec<ConsoleLine>);

    impl HostView for LinesHost {
        fn console_lines(&self, f: &mut dyn FnMut(&[ConsoleLine])) {
            f(&self.0);
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct PaintedRect {
        rect: Rect,
        fill: egui::Color32,
    }

    fn flatten(shape: &egui::Shape, out: &mut Vec<PaintedRect>) {
        match shape {
            egui::Shape::Rect(rect) => out.push(PaintedRect {
                rect: rect.rect,
                fill: rect.fill,
            }),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| flatten(shape, out)),
            _ => {}
        }
    }

    /// Draws one frame of the console and returns every filled rect it painted.
    fn paint_console(host: &dyn HostView) -> Vec<PaintedRect> {
        let state = EditorState::default();
        let frame = Frame::with_host(&state, host);
        let mut panel = ConsolePanel::default();
        let mut out = UiOut::new();

        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)));
        let output = ctx.run_ui(input, |ui| panel.ui(ui, &frame, &mut out));

        let mut painted = Vec::new();
        for clipped in &output.shapes {
            flatten(&clipped.shape, &mut painted);
        }
        painted
    }

    #[test]
    fn toolbar_paints_unitys_separators_between_its_items() {
        let painted = paint_console(&EmptyHost);
        let style = ConsoleStyle::dark();
        let separator = style.separator.to_egui();

        let bar = painted
            .iter()
            .find(|painted| painted.fill == style.toolbar_bg.to_egui())
            .expect("toolbar background")
            .rect;

        let vertical: Vec<f32> = painted
            .iter()
            .filter(|painted| {
                painted.fill == separator
                    && (painted.rect.width() - style.separator_width).abs() < 0.5
                    && painted.rect.height() > 10.0
            })
            .map(|painted| painted.rect.left())
            .collect();

        let count_group = style.count_widths * Level::ALL.len() as f32 + 2.0 * style.separator_width;
        let expected = [
            bar.left() + style.clear_width,
            bar.left() + style.clear_width + style.separator_width + style.collapse_width,
            bar.left()
                + style.clear_width
                + style.collapse_width
                + style.error_pause_width
                + 2.0 * style.separator_width,
            bar.left()
                + style.clear_width
                + style.collapse_width
                + style.error_pause_width
                + style.editor_width
                + 3.0 * style.separator_width,
            bar.right() - count_group - style.separator_width,
            bar.right() - count_group + style.count_widths,
            bar.right() - count_group + style.count_widths + style.separator_width + style.count_widths,
        ];
        for x in expected {
            assert!(
                vertical.iter().any(|painted| (painted - x).abs() < 0.5),
                "missing separator at x = {x}: {vertical:?}"
            );
        }

        // Unity closes the toolbar with a full-width 1 px line above the list.
        assert!(
            painted.iter().any(|painted| painted.fill == separator
                && (painted.rect.height() - style.separator_width).abs() < 0.5
                && (painted.rect.width() - bar.width()).abs() < 0.5
                && (painted.rect.top() - bar.bottom()).abs() < 0.5),
            "missing the separator below the toolbar"
        );
    }

    #[test]
    fn count_toggle_width_follows_its_number() {
        let style = ConsoleStyle::dark();
        let separator = style.separator.to_egui();
        let mut lines: Vec<ConsoleLine> = (0..303)
            .map(|_| ConsoleLine::new(Level::Warning, "warning"))
            .chain((0..5).map(|_| ConsoleLine::new(Level::Error, "error")))
            .collect();
        for (index, line) in lines.iter_mut().enumerate() {
            line.id = index as u64 + 1;
        }
        let painted = paint_console(&LinesHost(lines));

        let mut vertical: Vec<f32> = painted
            .iter()
            .filter(|painted| {
                painted.fill == separator
                    && (painted.rect.width() - style.separator_width).abs() < 0.5
                    && painted.rect.height() > 10.0
            })
            .map(|painted| painted.rect.left())
            .collect();
        vertical.sort_by(|a, b| a.partial_cmp(b).expect("finite coordinates"));

        // The last three lines delimit the count group: one leading separator
        // and two between the toggles. Info is empty ("0"), warning holds 303.
        let count = &vertical[vertical.len() - 3..];
        let info_width = count[1] - count[0] - style.separator_width;
        let warning_width = count[2] - count[1] - style.separator_width;
        assert!(
            warning_width > info_width + 4.0,
            "the 303 toggle should grow: info {info_width}, warning {warning_width}"
        );
    }
}
