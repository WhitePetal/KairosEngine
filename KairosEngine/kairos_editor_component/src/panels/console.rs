//! Console panel: the Unity/Tuanjie-style log console.
//!
//! Visual reference: the real Unity/Tuanjie 2022.3 console, captured and
//! measured in `docs/research/unity-console/README.md`. The panel is a pure view
//! over the host-owned `kairos_log` history; it keeps only ephemeral view state
//! (search, collapse, level filters, cleared cursor, selected row).
//!
//! Layout follows the reference: a 20 px toolbar (Clear / Collapse /
//! Error Pause / Editor, search, per-level count toggles), then the log list of
//! 38 px two-line entries with alternating backgrounds, then — for the selected
//! entry — the 130 px stack-trace detail pane. Clearing is a view cursor
//! (`cleared_through`), not a mutation of the host buffer.

use std::collections::HashMap;

use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Sense, Stroke, TextureHandle,
    TextureId, Ui, WidgetText, pos2, vec2,
};
use kairos_editor_core::PanelId;
use kairos_log::Level;

use crate::frame::{ConsoleLine, Frame, UiOut};
use crate::panel::Panel;

use super::CONSOLE;

// Unity/Tuanjie 2022.3 dark-skin console palette (docs/research/unity-console §4).
const TOOLBAR_BG: Color32 = Color32::from_rgb(53, 53, 53);
const TOOLBAR_TEXT: Color32 = Color32::from_rgb(177, 177, 177);
const TOGGLE_BG: Color32 = Color32::from_rgb(70, 70, 70);
const SEARCH_BG: Color32 = Color32::from_rgb(37, 37, 37);
const SEARCH_EDGE: Color32 = Color32::from_rgb(29, 29, 29);
const SEPARATOR: Color32 = Color32::from_rgb(32, 32, 32);
const ROW_ODD: Color32 = Color32::from_rgb(55, 55, 55);
const ROW_EVEN: Color32 = Color32::from_rgb(49, 49, 49);
const ROW_SELECTED: Color32 = Color32::from_rgb(39, 83, 124);
const TEXT: Color32 = Color32::from_rgb(192, 192, 192);
const TEXT_SELECTED: Color32 = Color32::from_rgb(241, 245, 247);
const LINK: Color32 = Color32::from_rgb(65, 110, 238);
const DETAIL_BG: Color32 = Color32::from_rgb(49, 49, 49);

const UI_TEXT_SIZE: f32 = 12.0;
const TOOLBAR_HEIGHT: f32 = 20.0;
const ROW_HEIGHT: f32 = 38.0;
const ROW_TEXT_TOP: f32 = 6.0;
const ICON_SIZE: f32 = 26.0;
const ICON_LEFT: f32 = 10.0;
const TEXT_LEFT: f32 = 41.0;
const DETAIL_HEIGHT: f32 = 130.0;
const DETAIL_PADDING: f32 = 3.0;
const COUNT_ICON: f32 = 13.0;
const CLEAR_WIDTH: f32 = 55.0;
const COLLAPSE_WIDTH: f32 = 58.0;
const ERROR_PAUSE_WIDTH: f32 = 73.0;
const EDITOR_WIDTH: f32 = 50.0;
const COUNT_WIDTHS: [f32; 3] = [40.0, 49.0, 32.0];

const UV: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));

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
    icons: Option<ConsoleIcons>,
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
            icons: None,
        }
    }
}

impl Panel for ConsolePanel {
    fn kind(&self) -> PanelId {
        CONSOLE
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        "Console".into()
    }

    fn ui(&mut self, ui: &mut Ui, frame: &Frame, _out: &mut UiOut) {
        let mut lines = Vec::new();
        frame
            .host
            .console_lines(&mut |entries| lines = entries.to_vec());
        if self.icons.is_none() {
            self.icons = Some(ConsoleIcons::load(ui.ctx()));
        }

        ui.painter()
            .rect_filled(ui.max_rect(), CornerRadius::ZERO, ROW_EVEN);
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
            DETAIL_HEIGHT.min((body.height() - ROW_HEIGHT).max(0.0))
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

    fn toolbar(&mut self, ui: &mut Ui, lines: &[ConsoleLine]) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(width, TOOLBAR_HEIGHT), Sense::hover());
        let painter = ui.painter().clone();
        painter.rect_filled(rect, CornerRadius::ZERO, TOOLBAR_BG);

        let font = FontId::new(UI_TEXT_SIZE, FontFamily::Proportional);
        let mut x = rect.left();

        // Clear (split button: label + caret menu).
        let clear = Rect::from_min_size(pos2(x, rect.top()), vec2(CLEAR_WIDTH, TOOLBAR_HEIGHT));
        let response = ui.interact(clear, ui.id().with("console-clear"), Sense::click());
        paint_hover(&painter, &response, clear, false);
        painter.text(
            pos2(clear.left() + 5.0, clear.center().y),
            Align2::LEFT_CENTER,
            "Clear",
            font.clone(),
            TOOLBAR_TEXT,
        );
        paint_caret(
            &painter,
            pos2(clear.right() - 10.0, clear.center().y),
            TOOLBAR_TEXT,
        );
        if response.clicked() {
            self.cleared_through = lines.last().map_or(0, |line| line.id);
            self.selected = None;
            self.follow_latest = true;
        }
        x = clear.right() + 1.0;

        // Collapse / Error Pause toggles.
        let collapse =
            Rect::from_min_size(pos2(x, rect.top()), vec2(COLLAPSE_WIDTH, TOOLBAR_HEIGHT));
        let response = ui.interact(collapse, ui.id().with("console-collapse"), Sense::click());
        paint_hover(&painter, &response, collapse, self.collapse);
        painter.text(
            collapse.center(),
            Align2::CENTER_CENTER,
            "Collapse",
            font.clone(),
            TOOLBAR_TEXT,
        );
        if response.clicked() {
            self.collapse = !self.collapse;
        }
        x = collapse.right() + 1.0;

        let pause =
            Rect::from_min_size(pos2(x, rect.top()), vec2(ERROR_PAUSE_WIDTH, TOOLBAR_HEIGHT));
        let response = ui.interact(pause, ui.id().with("console-error-pause"), Sense::click());
        paint_hover(&painter, &response, pause, self.error_pause);
        painter.text(
            pause.center(),
            Align2::CENTER_CENTER,
            "Error Pause",
            font.clone(),
            TOOLBAR_TEXT,
        );
        if response.clicked() {
            self.error_pause = !self.error_pause;
        }
        x = pause.right() + 1.0;

        // Editor/Player source filter (Unity's Editor dropdown).
        let editor = Rect::from_min_size(pos2(x, rect.top()), vec2(EDITOR_WIDTH, TOOLBAR_HEIGHT));
        let response = ui.interact(editor, ui.id().with("console-source"), Sense::click());
        paint_hover(&painter, &response, editor, false);
        painter.text(
            pos2(editor.left() + 8.0, editor.center().y),
            Align2::LEFT_CENTER,
            "Editor",
            font.clone(),
            TOOLBAR_TEXT,
        );
        paint_caret(
            &painter,
            pos2(editor.right() - 13.0, editor.center().y),
            TOOLBAR_TEXT,
        );
        x = editor.right() + 1.0;

        // Search field fills the middle; the count buttons are pinned right.
        let counts_width: f32 = COUNT_WIDTHS.iter().sum::<f32>() + 2.0;
        let counts_left = rect.right() - counts_width;
        let search = Rect::from_min_max(
            pos2(x + 6.0, rect.top() + 2.0),
            pos2(counts_left - 8.0, rect.top() + TOOLBAR_HEIGHT - 2.0),
        );
        painter.rect_filled(search, CornerRadius::same(3), SEARCH_BG);
        painter.rect_stroke(
            search,
            CornerRadius::same(3),
            Stroke::new(1.0, SEARCH_EDGE),
            egui::StrokeKind::Inside,
        );
        paint_magnifier(&painter, pos2(search.left() + 9.0, search.center().y));
        let edit = Rect::from_min_max(
            pos2(search.left() + 18.0, search.top()),
            pos2(search.right() - 4.0, search.bottom()),
        );
        ui.put(
            edit,
            egui::TextEdit::singleline(&mut self.search)
                .id(ui.id().with("console-search"))
                .font(FontId::new(UI_TEXT_SIZE, FontFamily::Proportional))
                .text_color(TOOLBAR_TEXT)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::symmetric(0, 1)),
        );

        self.paint_counts(ui, rect, counts_left, &self.counts(lines));
    }

    fn paint_counts(&mut self, ui: &mut Ui, toolbar: Rect, counts_left: f32, counts: &[usize; 3]) {
        let painter = ui.painter().clone();
        painter.rect_filled(
            Rect::from_min_size(
                pos2(counts_left - 1.0, toolbar.top()),
                vec2(1.0, TOOLBAR_HEIGHT),
            ),
            CornerRadius::ZERO,
            SEPARATOR,
        );

        let font = FontId::new(UI_TEXT_SIZE, FontFamily::Proportional);
        let icons = self
            .icons
            .as_ref()
            .expect("icons loaded before the toolbar");
        let mut x = counts_left;
        for (index, level) in Level::ALL.into_iter().enumerate() {
            let segment = Rect::from_min_size(
                pos2(x, toolbar.top()),
                vec2(COUNT_WIDTHS[index], TOOLBAR_HEIGHT),
            );
            painter.rect_filled(segment, CornerRadius::ZERO, TOGGLE_BG);
            let response = ui.interact(
                segment,
                ui.id().with(("console-level", index)),
                Sense::click(),
            );
            if response.clicked() {
                self.levels[index] = !self.levels[index];
            }

            let visible = self.levels[index];
            let icon_rect = Rect::from_min_size(
                pos2(segment.left() + 5.0, segment.center().y - COUNT_ICON * 0.5),
                vec2(COUNT_ICON, COUNT_ICON),
            );
            painter.image(icons.icon(level, visible), icon_rect, UV, Color32::WHITE);
            painter.text(
                pos2(icon_rect.right() + 5.0, segment.center().y),
                Align2::LEFT_CENTER,
                counts[index].to_string(),
                font.clone(),
                TOOLBAR_TEXT,
            );

            x = segment.right();
            if index < 2 {
                painter.rect_filled(
                    Rect::from_min_size(pos2(x, toolbar.top()), vec2(1.0, TOOLBAR_HEIGHT)),
                    CornerRadius::ZERO,
                    TOOLBAR_BG,
                );
                x += 1.0;
            }
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
        let (rect, _) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::hover());
        let response = ui.interact(rect, ui.id().with(("console-row", line.id)), Sense::click());
        if response.clicked() {
            self.selected = Some(line.id);
            self.follow_latest = false;
        }

        let selected = self.selected == Some(line.id);
        let background = if selected {
            ROW_SELECTED
        } else if index % 2 == 0 {
            ROW_ODD
        } else {
            ROW_EVEN
        };
        let text_color = if selected { TEXT_SELECTED } else { TEXT };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::ZERO, background);

        let icons = self.icons.as_ref().expect("icons loaded before the list");
        let icon_rect = Rect::from_min_size(
            pos2(rect.left() + ICON_LEFT, rect.top() + 6.0),
            vec2(ICON_SIZE, ICON_SIZE),
        );
        painter.image(icons.icon(line.level, true), icon_rect, UV, Color32::WHITE);

        let font = FontId::new(UI_TEXT_SIZE, FontFamily::Proportional);
        let text = format!("[{}] {}", line.time, line.message);
        let galley = painter.layout(text, font, text_color, rect.width() - TEXT_LEFT - 10.0);
        let clip = Rect::from_min_max(
            pos2(rect.left() + TEXT_LEFT, rect.top() + ROW_TEXT_TOP - 2.0),
            pos2(rect.right() - 10.0, rect.bottom()),
        );
        let text_painter = painter.with_clip_rect(clip);
        text_painter.galley(
            pos2(rect.left() + TEXT_LEFT, rect.top() + ROW_TEXT_TOP),
            galley,
            text_color,
        );

        if self.collapse && count > 1 {
            let badge_rect = Rect::from_min_size(
                pos2(rect.right() - 28.0, rect.center().y - 10.0),
                vec2(20.0, 20.0),
            );
            painter.image(icons.badge(), badge_rect, UV, Color32::WHITE);
            let badge_text = if count > 99 {
                "99+".to_owned()
            } else {
                count.to_string()
            };
            painter.text(
                badge_rect.center(),
                Align2::CENTER_CENTER,
                badge_text,
                FontId::new(10.0, FontFamily::Proportional),
                TEXT,
            );
        }
    }

    fn detail_pane(&mut self, ui: &mut Ui, rect: Rect, lines: &[ConsoleLine]) {
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::ZERO, DETAIL_BG);
        painter.rect_filled(
            Rect::from_min_size(rect.min, vec2(rect.width(), 1.0)),
            CornerRadius::ZERO,
            SEPARATOR,
        );

        let Some(line) = self.selected_line(lines) else {
            return;
        };
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect.shrink2(vec2(DETAIL_PADDING, 5.0))),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("kairos-console-detail")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let font = FontId::new(UI_TEXT_SIZE, FontFamily::Proportional);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&line.message)
                                    .font(font.clone())
                                    .color(TEXT),
                            )
                            .wrap(),
                        );
                        for stack_line in &line.stack {
                            paint_stack_line(ui, stack_line);
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

fn paint_stack_line(ui: &mut Ui, line: &str) {
    let font = FontId::new(UI_TEXT_SIZE, FontFamily::Proportional);

    // Unity colours the trailing `(at file:line)` of a frame as a source link
    // while the symbol itself stays grey.
    match line.find(" (at ") {
        Some(index) => {
            let symbol = &line[..index];
            let location = &line[index + 1..];
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add(
                    egui::Label::new(egui::RichText::new(symbol).font(font.clone()).color(TEXT))
                        .wrap(),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!(" {location}"))
                            .font(font)
                            .color(LINK),
                    )
                    .wrap(),
                );
            });
        }
        None => {
            ui.add(egui::Label::new(egui::RichText::new(line).font(font).color(TEXT)).wrap());
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

fn paint_hover(painter: &egui::Painter, response: &egui::Response, rect: Rect, active: bool) {
    if active || response.hovered() {
        painter.rect_filled(rect, CornerRadius::ZERO, TOGGLE_BG);
    }
}

fn paint_caret(painter: &egui::Painter, center: Pos2, color: Color32) {
    let points = vec![
        pos2(center.x - 3.5, center.y - 1.5),
        pos2(center.x + 3.5, center.y - 1.5),
        pos2(center.x, center.y + 2.5),
    ];
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

fn paint_magnifier(painter: &egui::Painter, center: Pos2) {
    painter.circle_stroke(
        center + vec2(-1.0, -1.0),
        4.0,
        Stroke::new(1.2, TOOLBAR_TEXT),
    );
    painter.line_segment(
        [center + vec2(1.8, 1.8), center + vec2(4.2, 4.2)],
        Stroke::new(1.2, TOOLBAR_TEXT),
    );
}

/// The real Unity/Tuanjie console icons, extracted from the editor's serialized
/// resources (`docs/research/unity-console/icons/`) and embedded here so the
/// panel renders the exact artwork instead of a lookalike.
struct ConsoleIcons {
    info: TextureHandle,
    warning: TextureHandle,
    error: TextureHandle,
    info_off: TextureHandle,
    warning_off: TextureHandle,
    error_off: TextureHandle,
    badge: TextureHandle,
}

impl ConsoleIcons {
    fn load(ctx: &egui::Context) -> Self {
        Self {
            info: load_png(
                ctx,
                "kairos-console-info",
                include_bytes!("../../assets/icons/console/info.png"),
            ),
            warning: load_png(
                ctx,
                "kairos-console-warning",
                include_bytes!("../../assets/icons/console/warning.png"),
            ),
            error: load_png(
                ctx,
                "kairos-console-error",
                include_bytes!("../../assets/icons/console/error.png"),
            ),
            info_off: load_png(
                ctx,
                "kairos-console-info-off",
                include_bytes!("../../assets/icons/console/info_off.png"),
            ),
            warning_off: load_png(
                ctx,
                "kairos-console-warning-off",
                include_bytes!("../../assets/icons/console/warning_off.png"),
            ),
            error_off: load_png(
                ctx,
                "kairos-console-error-off",
                include_bytes!("../../assets/icons/console/error_off.png"),
            ),
            badge: load_png(
                ctx,
                "kairos-console-badge",
                include_bytes!("../../assets/icons/console/count_badge.png"),
            ),
        }
    }

    fn icon(&self, level: Level, visible: bool) -> TextureId {
        match (level, visible) {
            (Level::Info, true) => self.info.id(),
            (Level::Info, false) => self.info_off.id(),
            (Level::Warning, true) => self.warning.id(),
            (Level::Warning, false) => self.warning_off.id(),
            (Level::Error, true) => self.error.id(),
            (Level::Error, false) => self.error_off.id(),
        }
    }

    fn badge(&self) -> TextureId {
        self.badge.id()
    }
}

fn load_png(ctx: &egui::Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let image = image::load_from_memory(bytes)
        .unwrap_or_else(|error| panic!("embedded console icon {name} is not a valid PNG: {error}"))
        .to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    ctx.load_texture(name, pixels, egui::TextureOptions::LINEAR)
}
