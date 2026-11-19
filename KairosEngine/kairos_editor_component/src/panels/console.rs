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
//! entry — the stack-trace detail pane. Unity fixes that pane at 130 px; Kairos
//! lets the user drag its top edge to resize it. Clearing is a view cursor
//! (`cleared_through`), not a mutation of the host buffer.
//!
//! Appearance comes from [`ConsoleStyle`], loaded from
//! `Preferences/Styles/ConsoleStyle.ron` when this panel is initialized
//! ([`Panel::init`]). The style file holds dark + light skins; icon file paths
//! are part of it and are preloaded through `kairos_editor_widgets::ui_image`.
//! This module only wires host data, view state and interaction.

pub mod style;

use kairos_editor_widgets::color::ColorExt;
use std::collections::HashMap;

use egui::{Align2, CornerRadius, Rect, Sense, Stroke, Ui, WidgetText, WidgetType, pos2, vec2};
use kairos_editor_widgets::button::{Button, ButtonAlign, ButtonStyle};
use kairos_editor_widgets::paint::{labelled_widget_info, magnifier};
use kairos_editor_widgets::splitter::Splitter;
use kairos_editor_widgets::style::StyleFile;
use kairos_editor_widgets::text::TextStyle;
use kairos_editor_widgets::ui_image;
use kairos_log::Level;
use strum::{EnumCount, IntoEnumIterator};

use crate::frame::{ConsoleLine, Frame, UiOut};
use crate::panel::Panel;
use crate::panels::console::style::ConsoleStyle;

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
    /// Stack-trace pane height after the user drags its top edge; `None` means
    /// the style's `detail_height` applies.
    detail_height: Option<f32>,
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
            detail_height: None,
        }
    }
}

#[crate::panel(CONSOLE = "console", skin = ConsoleStyle)]
impl Panel for ConsolePanel {
    fn icon(&self) -> Option<&str> {
        Some(self.style.tab_icon.as_str())
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        "Console".into()
    }

    /// The Console skin, editable from Preferences.

    fn init(&mut self, ctx: &egui::Context) {
        match StyleFile::<ConsoleStyle>::load_or_default() {
            Ok(file) => {
                self.style = if ctx.theme() == egui::Theme::Light {
                    file.light
                } else {
                    file.dark
                };
            }
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
            self.pane_height(body.height())
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
            // Interact after the panes so the splitter wins the hit test against
            // the log rows and the detail pane's scroll area: egui gives the
            // last widget the pointer.
            self.splitter(ui, body, detail_height);
        }
    }
}

impl ConsolePanel {
    /// Resolved stack-trace pane height for this frame: the style default until
    /// the user drags the splitter, then the dragged height. The clamp keeps one
    /// log row visible above the pane and gives the pane a row's worth of height.
    fn pane_height(&self, body_height: f32) -> f32 {
        let max = (body_height - self.style.row_height).max(0.0);
        let min = self.style.row_height.min(max);
        self.detail_height
            .unwrap_or(self.style.detail_height)
            .clamp(min, max)
    }

    /// The draggable edge between the log list and the stack-trace pane. Unity
    /// fixes the pane at 130 px; in Kairos it grows when the edge is dragged up,
    /// and a double-click restores the style default (as the dock's splitters
    /// do). The edge is a shared Splitter in its overlay form: its band is the
    /// drag target, but it takes no room from either pane, so the 1 px line
    /// stays exactly on the boundary the panes were laid out around.
    fn splitter(&mut self, ui: &mut Ui, body: Rect, detail_height: f32) {
        let top = body.max.y - detail_height;
        let grab = self.style.splitter_grab;
        let band = Rect::from_min_max(pos2(body.min.x, top - grab), pos2(body.max.x, top + grab));
        let mut list_height = body.height() - detail_height;
        let response = Splitter::horizontal()
            .line(self.style.separator_width)
            .grab(2.0 * grab)
            .limits(self.style.row_height, self.style.row_height)
            .colors(
                self.style.separator.to_egui(),
                self.style.toggle_bg.to_egui(),
                self.style.toolbar_text.to_egui(),
            )
            .reset_to(body.height() - self.style.detail_height)
            .show_in(
                ui,
                ui.id().with("console-detail-splitter"),
                band,
                &mut list_height,
                body.height(),
            );
        if response.double_clicked() {
            // None keeps following the style default, so a skin edit moves
            // the restored pane too.
            self.detail_height = None;
        } else if response.changed() {
            self.detail_height = Some(body.height() - list_height);
        }
    }

    fn counts(&self, lines: &[ConsoleLine]) -> [usize; 3] {
        let mut counts = [0; 3];
        for line in lines.iter().filter(|line| line.id > self.cleared_through) {
            counts[level_index(line.level)] += 1;
        }
        counts
    }

    /// Reusable text style for the Console toolbar chrome (labels, counts).
    ///
    /// The panel paints the same 12 px text in a handful of places; one
    /// `TextStyle` per colour replaces the repeated `FontId + color` pairs.
    fn toolbar_text(&self) -> TextStyle {
        TextStyle::sized(self.style.toolbar_text.to_egui(), self.style.text_size)
    }

    /// Reusable text style for log rows and the stack-trace pane.
    fn body_text(&self) -> TextStyle {
        TextStyle::sized(self.style.text.to_egui(), self.style.text_size)
    }

    /// Reusable style for the Console toolbar buttons ("Clear", "Collapse",
    /// "Error Pause", "Editor"): square, Console-hover fill, exact row height.
    fn button_style(&self) -> ButtonStyle {
        ButtonStyle::new(self.toolbar_text())
            .fills(
                egui::Color32::TRANSPARENT,
                self.style.toggle_bg.to_egui(),
                self.style.toggle_bg.to_egui(),
            )
            .corner_radius(CornerRadius::ZERO)
            .height(self.style.toolbar_height)
            .padding(vec2(5.0, 0.0))
    }

    /// Unity sizes the three count toggles to their content (measured: 40 px for
    /// `39`, 49 px for `303`, 32 px for `5`); the style token is the floor so a
    /// toggle never paints its number over the next one.
    fn count_widths(&self, ui: &Ui, counts: &[usize; 3]) -> [f32; 3] {
        let text_style = self.toolbar_text();
        (*counts).map(|count| {
            let text = text_style.layout(ui, count.to_string());
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
            vec2(
                width,
                self.style.toolbar_height + self.style.separator_width,
            ),
            Sense::hover(),
        );
        let bar = Rect::from_min_size(row.min, vec2(width, self.style.toolbar_height));
        let painter = ui.painter().clone();
        painter.rect_filled(bar, CornerRadius::ZERO, self.style.toolbar_bg.to_egui());
        painter.rect_filled(
            Rect::from_min_size(
                pos2(bar.left(), bar.bottom()),
                vec2(width, self.style.separator_width),
            ),
            CornerRadius::ZERO,
            self.style.separator.to_egui(),
        );

        let button = self.button_style();
        let text = self.toolbar_text();
        let mut x = bar.left();

        // Clear (split button: label + caret menu). The buttons are positioned
        // on the measured Unity grid; the reusable Button widget owns the
        // hover/pressed fill, the label and the caret.
        let clear = Rect::from_min_size(
            pos2(x, bar.top()),
            vec2(self.style.clear_width, bar.height()),
        );
        let response = ui.put(
            clear,
            Button::new("Clear", &button)
                .width(clear.width())
                .height(clear.height())
                .padding(vec2(5.0, 0.0))
                .caret()
                .id(ui.id().with("console-clear")),
        );
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
        let response = ui.put(
            collapse,
            Button::new("Collapse", &button)
                .width(collapse.width())
                .height(collapse.height())
                .align(ButtonAlign::Center)
                .selected(self.collapse)
                .id(ui.id().with("console-collapse")),
        );
        if response.clicked() {
            self.collapse = !self.collapse;
        }
        x = toolbar_separator(&painter, bar, collapse.right(), &self.style);

        let pause = Rect::from_min_size(
            pos2(x, bar.top()),
            vec2(self.style.error_pause_width, bar.height()),
        );
        let response = ui.put(
            pause,
            Button::new("Error Pause", &button)
                .width(pause.width())
                .height(pause.height())
                .align(ButtonAlign::Center)
                .selected(self.error_pause)
                .id(ui.id().with("console-error-pause")),
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
        ui.put(
            editor,
            Button::new("Editor", &button)
                .width(editor.width())
                .height(editor.height())
                .padding(vec2(8.0, 0.0))
                .caret()
                .caret_inset(13.0)
                .id(ui.id().with("console-source")),
        );
        x = toolbar_separator(&painter, bar, editor.right(), &self.style);

        // Search field fills the middle; the count buttons are pinned right.
        let counts = self.counts(lines);
        let count_widths = self.count_widths(ui, &counts);
        let counts_width: f32 = count_widths.iter().sum::<f32>()
            + (Level::COUNT as f32 - 1.0) * self.style.separator_width;
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
        magnifier(
            &painter,
            pos2(search.left() + 9.0, search.center().y),
            text.text_color(),
        );
        let edit = Rect::from_min_max(
            pos2(search.left() + 18.0, search.top()),
            pos2(search.right() - 4.0, search.bottom()),
        );
        ui.put(
            edit,
            egui::TextEdit::singleline(&mut self.search)
                .id(ui.id().with("console-search"))
                .font(text.font_id(ui.style()))
                .text_color(text.text_color())
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
        let mut x = toolbar_separator(
            &painter,
            bar,
            counts_left - self.style.separator_width,
            &self.style,
        );

        let text = self.toolbar_text();
        for (index, level) in Level::iter().enumerate() {
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
            labelled_widget_info(
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
                self.style.icons.severity(level, visible),
                egui::Color32::WHITE,
            );
            text.paint(
                ui,
                pos2(icon_rect.right() + 5.0, segment.center().y),
                Align2::LEFT_CENTER,
                counts[index].to_string(),
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
        let text_style = if selected {
            TextStyle::sized(self.style.text_selected.to_egui(), self.style.text_size)
        } else {
            self.body_text()
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
            self.style.icons.severity(line.level, true),
            egui::Color32::WHITE,
        );

        let text = format!("[{}] {}", line.time, line.message);
        let galley =
            text_style.layout_wrapped(ui, text, rect.width() - self.style.text_left - 10.0);
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
            text_style.text_color(),
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
            TextStyle::sized(self.style.text.to_egui(), self.style.badge_text_size).paint(
                ui,
                badge_rect.center(),
                Align2::CENTER_CENTER,
                badge_text,
            );
        }
    }

    fn detail_pane(&mut self, ui: &mut Ui, rect: Rect, lines: &[ConsoleLine]) {
        // The 1 px edge above the pane belongs to the splitter, which paints it
        // in its hover and drag colours.
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, self.style.detail_bg.to_egui());

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
                        let text = self.body_text();
                        ui.add(egui::Label::new(text.rich(line.message.as_str())).wrap());
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
        Rect::from_min_size(
            pos2(x, bar.top()),
            vec2(style.separator_width, bar.height()),
        ),
        CornerRadius::ZERO,
        style.separator.to_egui(),
    );
    x + style.separator_width
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

fn paint_stack_line(ui: &mut Ui, line: &str, style: &ConsoleStyle) {
    let text = TextStyle::sized(style.text.to_egui(), style.text_size);
    let link = TextStyle::sized(style.link.to_egui(), style.text_size);

    // Unity colours the trailing `(at file:line)` of a frame as a source link
    // while the symbol itself stays grey.
    match line.find(" (at ") {
        Some(index) => {
            let symbol = &line[..index];
            let location = &line[index + 1..];
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add(egui::Label::new(text.rich(symbol)).wrap());
                ui.add(egui::Label::new(link.rich(format!(" {location}"))).wrap());
            });
        }
        None => {
            ui.add(egui::Label::new(text.rich(line)).wrap());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::HostView;
    use egui::Pos2;
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

    /// The headless canvas the harness renders into.
    const CANVAS_WIDTH: f32 = 800.0;
    const CANVAS_HEIGHT: f32 = 600.0;

    /// A console panel run frame after frame on the same context, for tests that
    /// need pointer state to persist across frames (drag, double click).
    struct ConsoleHarness<'a> {
        ctx: egui::Context,
        state: EditorState,
        panel: ConsolePanel,
        host: &'a dyn HostView,
        out: UiOut,
    }

    impl<'a> ConsoleHarness<'a> {
        fn new(host: &'a dyn HostView) -> Self {
            Self {
                ctx: egui::Context::default(),
                state: EditorState::default(),
                panel: ConsolePanel::default(),
                host,
                out: UiOut::new(),
            }
        }

        /// Runs one frame with `events` and returns every filled rect it painted.
        fn frame(&mut self, events: Vec<egui::Event>) -> Vec<PaintedRect> {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(
                Pos2::ZERO,
                vec2(CANVAS_WIDTH, CANVAS_HEIGHT),
            ));
            input.events = events;

            let Self {
                ctx,
                state,
                panel,
                host,
                out,
            } = self;
            let frame = Frame::with_host(state, *host);
            let output = ctx.run_ui(input, |ui| panel.ui(ui, &frame, &mut *out));

            let mut painted = Vec::new();
            for clipped in &output.shapes {
                flatten(&clipped.shape, &mut painted);
            }
            painted
        }
    }

    /// Draws one frame of the console and returns every filled rect it painted.
    fn paint_console(host: &dyn HostView) -> Vec<PaintedRect> {
        ConsoleHarness::new(host).frame(Vec::new())
    }

    fn pointer_move(pos: Pos2) -> egui::Event {
        egui::Event::PointerMoved(pos)
    }

    fn pointer_button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Host with `count` visible rows; the console needs a selection to show the
    /// stack-trace pane.
    fn lines_host(count: usize) -> LinesHost {
        let mut lines: Vec<ConsoleLine> = (0..count)
            .map(|_| ConsoleLine::new(Level::Info, "info"))
            .collect();
        for (index, line) in lines.iter_mut().enumerate() {
            line.id = index as u64 + 1;
        }
        LinesHost(lines)
    }

    /// The stack-trace pane's painted background. `detail_bg` happens to equal
    /// `row_even`, so take the lowermost rect of that colour: the pane sits on
    /// the body's bottom edge.
    fn detail_pane_rect(painted: &[PaintedRect], style: &ConsoleStyle) -> Rect {
        painted
            .iter()
            .filter(|painted| painted.fill == style.detail_bg.to_egui())
            .map(|painted| painted.rect)
            .max_by(|a, b| a.bottom().total_cmp(&b.bottom()))
            .expect("stack-trace pane")
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

        let count_group = style.count_widths * Level::COUNT as f32 + 2.0 * style.separator_width;
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
            bar.right() - count_group
                + style.count_widths
                + style.separator_width
                + style.count_widths,
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
    fn clicking_clear_hides_the_rows() {
        let style = ConsoleStyle::dark();
        let host = lines_host(4);
        let mut harness = ConsoleHarness::new(&host);

        let before = harness.frame(Vec::new());
        assert!(
            before
                .iter()
                .any(|painted| painted.fill == style.row_odd.to_egui()),
            "the fixture should paint log rows before Clear"
        );

        // The Clear segment is the first measured toolbar button.
        let clear = pos2(style.clear_width * 0.5, style.toolbar_height * 0.5);
        harness.frame(vec![pointer_move(clear), pointer_button(clear, true)]);
        harness.frame(vec![pointer_button(clear, false)]);

        let after = harness.frame(Vec::new());
        assert!(
            !after
                .iter()
                .any(|painted| painted.fill == style.row_odd.to_egui()),
            "Clear should hide every log row"
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

    /// Presses the splitter at `grab` and drags the pointer to `end`, releasing
    /// there. The first +10 px move only starts the drag, so the pane follows the
    /// pointer from that point on.
    fn drag_splitter(harness: &mut ConsoleHarness<'_>, grab: Pos2, end: Pos2) {
        harness.frame(vec![pointer_move(grab), pointer_button(grab, true)]);
        harness.frame(vec![pointer_move(pos2(grab.x, grab.y - 10.0))]);
        harness.frame(vec![pointer_move(end)]);
        harness.frame(vec![pointer_button(end, false)]);
    }

    #[test]
    fn dragging_the_splitter_resizes_the_stack_pane() {
        let style = ConsoleStyle::dark();
        let host = lines_host(4);
        let mut harness = ConsoleHarness::new(&host);

        let before = detail_pane_rect(&harness.frame(Vec::new()), &style);
        assert!(
            (before.height() - style.detail_height).abs() < 0.5,
            "the pane should start at the style height, got {}",
            before.height()
        );

        // The pane follows the pointer from the press: 40 px up grows it 40 px.
        let grab = pos2(before.center().x, before.top());
        drag_splitter(&mut harness, grab, pos2(grab.x, grab.y - 40.0));
        let after = detail_pane_rect(&harness.frame(Vec::new()), &style);
        assert!(
            (after.height() - (style.detail_height + 40.0)).abs() < 1.0,
            "dragging 40 px up should grow the pane to {} px, got {}",
            style.detail_height + 40.0,
            after.height()
        );
    }

    #[test]
    fn dragging_the_splitter_past_the_top_clamps_without_a_dead_zone() {
        let style = ConsoleStyle::dark();
        let host = lines_host(4);
        let mut harness = ConsoleHarness::new(&host);

        let before = detail_pane_rect(&harness.frame(Vec::new()), &style);
        let grab = pos2(before.center().x, before.top());
        harness.frame(vec![pointer_move(grab), pointer_button(grab, true)]);
        harness.frame(vec![pointer_move(pos2(grab.x, grab.y - 10.0))]);
        // Far past the clamp: the pane stops one row short of the body, so the
        // log list keeps its last row.
        harness.frame(vec![pointer_move(pos2(grab.x, 0.0))]);
        // Dragging back down moves the edge immediately instead of replaying the
        // overshoot first.
        harness.frame(vec![pointer_move(pos2(grab.x, 20.0))]);
        let settled = harness.frame(vec![pointer_button(pos2(grab.x, 20.0), false)]);

        let body_height = CANVAS_HEIGHT - (style.toolbar_height + style.separator_width);
        let expected = body_height - style.row_height - 20.0;
        let after = detail_pane_rect(&settled, &style);
        assert!(
            (after.height() - expected).abs() < 1.0,
            "after clamping and reversing 20 px the pane should be {expected} px, got {}",
            after.height()
        );
    }

    #[test]
    fn double_clicking_the_splitter_restores_the_style_height() {
        let style = ConsoleStyle::dark();
        let host = lines_host(4);
        let mut harness = ConsoleHarness::new(&host);

        let before = detail_pane_rect(&harness.frame(Vec::new()), &style);
        let grab = pos2(before.center().x, before.top());
        drag_splitter(&mut harness, grab, pos2(grab.x, grab.y - 40.0));
        let dragged = detail_pane_rect(&harness.frame(Vec::new()), &style);
        assert!(
            dragged.height() > style.detail_height + 20.0,
            "drag should have resized the pane, got {}",
            dragged.height()
        );

        // Two clicks in successive frames on the moved edge are a double click.
        let edge = pos2(grab.x, CANVAS_HEIGHT - dragged.height());
        for pressed in [true, false, true, false] {
            let mut events = vec![pointer_button(edge, pressed)];
            if pressed {
                events.insert(0, pointer_move(edge));
            }
            harness.frame(events);
        }

        let after = detail_pane_rect(&harness.frame(Vec::new()), &style);
        assert!(
            (after.height() - style.detail_height).abs() < 0.5,
            "double-click should restore {} px, got {}",
            style.detail_height,
            after.height()
        );
    }
}
