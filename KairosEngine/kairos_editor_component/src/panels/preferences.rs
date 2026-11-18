//! Preferences panel: every open panel's skin, as editable fields.
//!
//! This is the 0.2.0 rewrite of the legacy `kairos_editor` `PreferencesWindow`,
//! and it keeps the capability that window existed for: pick a component on the
//! left, edit its `Preferences/Styles/*.ron` tokens as widgets on the right.
//!
//! The review that produced this shape — the legacy window's index-based
//! readback, its per-frame TOML write, its pages keyed by position in
//! `drawers`, and the parts deliberately not carried over — is
//! `docs/plan/editor-ui-architecture/preferences-panel.md`.
//!
//! # Where the fields come from
//!
//! Each panel owns its style (loaded in `Panel::init`) and derives
//! `StyleFields`, so it can describe its own tokens (`crate::skin`). The app
//! snapshots the live panels' field tables into `Frame::skins` while this panel
//! is open; this panel draws them and never touches another panel directly.
//!
//! # Live apply and save
//!
//! A control pushes `StyleAction::SetField` on every change. The app writes it
//! into the owning panel's live style, so the next pass draws with the new
//! value — that is what makes the edit immediate. The file is written once, on
//! `StyleAction::Commit`: pointer release, page switch, or closing this panel.
//! The legacy window instead pushed the whole page back every frame and let
//! the owning window write its TOML, about 60 times a second.

pub mod style;

use egui::{
    Align, Context, CornerRadius, Id, Label, Layout, Margin, Rect, Sense, Stroke, TextEdit, Ui,
    UiBuilder, WidgetText, pos2, vec2,
};
use kairos_color::Color32;
use kairos_editor_core::{Action, LayoutAction, PanelId, StyleAction, StyleValue};
use kairos_editor_widgets::button::{Button, ButtonAlign};
use kairos_editor_widgets::color::Color32Ext;
use kairos_editor_widgets::divider::Divider;
use kairos_editor_widgets::style::{Style, StyleFile};
use kairos_editor_widgets::text::TextStyle;

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;
use crate::skin::{PanelSkin, StyleField, StyleFields, StyleKind, humanize};

use self::style::PreferencesPanelStyle;

fn nav_id(panel: PanelId) -> Id {
    Id::new(("kairos.preferences.nav", panel))
}

/// Stable id of a boolean control; see [`nav_id`].
fn toggle_id(panel: PanelId, path: &str) -> Id {
    Id::new(("kairos.preferences.toggle", panel, path))
}

fn close_id() -> Id {
    Id::new("kairos.preferences.close")
}

/// The page being edited: a snapshot of one panel's field table, plus the
/// bookkeeping that decides when its file is written.
#[derive(Default)]
struct PageState {
    /// The panel the fields belong to.
    panel: Option<PanelId>,
    /// Edited in place: a drag follows the pointer even though the owning
    /// panel's next snapshot is a frame behind.
    fields: Vec<StyleField>,
    title: &'static str,
    file: &'static str,
    /// A `SetField` was pushed since the last save.
    dirty: bool,
}

impl PageState {
    /// Show `selected`'s field table, committing a pending edit on the page
    /// being left.
    fn select(&mut self, skins: &[PanelSkin], selected: Option<PanelId>, out: &mut UiOut) {
        let same = self.panel == selected && (selected.is_none() || !self.fields.is_empty());
        if same {
            return;
        }

        self.commit(out);

        match selected.and_then(|panel| skins.iter().find(|skin| skin.panel == panel)) {
            Some(skin) => {
                self.fields.clone_from(&skin.fields);
                self.title = skin.title;
                self.file = skin.file;
            }
            None => {
                self.fields.clear();
                self.title = "";
                self.file = "";
            }
        }
        self.panel = selected;
    }

    /// Ask the app to persist the page being left, if it has an edit that was
    /// never released (typed text, arrow keys).
    fn commit(&mut self, out: &mut UiOut) {
        if !self.dirty {
            return;
        }
        if let Some(panel) = self.panel {
            out.push(Action::Style(StyleAction::Commit { panel }));
        }
        self.dirty = false;
    }
}

/// The Preferences panel.
#[crate::panel(PREFERENCES = "preferences")]
pub struct PreferencesPanel {
    style: PreferencesPanelStyle,
    /// Page list selection; falls back to the first live skin.
    selected: Option<PanelId>,
    page: PageState,
}

impl PreferencesPanel {
    /// Build the panel. The catalog factory above is the only caller.
    pub fn new() -> Self {
        Self {
            style: PreferencesPanelStyle::default(),
            selected: None,
            page: PageState::default(),
        }
    }
}

impl Default for PreferencesPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for PreferencesPanel {
    fn title(&self, _frame: &Frame) -> WidgetText {
        "Preferences".into()
    }

    /// Load the skin. A missing or broken file is logged and falls back to the
    /// code defaults; the legacy window treated it as fatal and took the editor
    /// down with it.
    fn init(&mut self, ctx: &Context) {
        match StyleFile::<PreferencesPanelStyle>::load_or_default() {
            Ok(file) => {
                self.style = if ctx.theme() == egui::Theme::Light {
                    file.light
                } else {
                    file.dark
                };
            }
            Err(error) => {
                kairos_log::warn!(target: "kairos_editor", "preferences style: {error}");
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut) {
        let Self {
            style,
            selected,
            page,
        } = self;

        // The dock builds a body `Ui` from the context style, so the app's
        // "chrome is not selectable" switch does not reach this panel.
        ui.style_mut().interaction.selectable_labels = false;

        // The dock paints a body colour behind every tab; a floating window uses
        // the panel's own. Painting it here keeps both cases identical.
        ui.painter().rect_filled(
            ui.max_rect(),
            CornerRadius::ZERO,
            style.background.to_egui(),
        );

        let skins = frame.skins;
        let mut current = match *selected {
            Some(panel) if skins.iter().any(|skin| skin.panel == panel) => Some(panel),
            _ => skins.first().map(|skin| skin.panel),
        };

        egui::Frame::NONE
            .inner_margin(Margin::same(style.padding.round() as i8))
            .show(ui, |ui| {
                // The two regions are sized by hand; egui's default gap between
                // them would make the footer miss the bottom edge by a few px.
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                let available = ui.available_size();
                let body_height = (available.y - style.footer_gap - style.footer_height).max(0.0);
                let (body, _) =
                    ui.allocate_exact_size(vec2(available.x, body_height), Sense::hover());
                let (footer_rect, _) =
                    ui.allocate_exact_size(vec2(available.x, style.footer_height), Sense::hover());

                // The footer is pinned: the page scrolls, the action never does.
                let rule_y = footer_rect.top() - style.footer_gap * 0.5;
                Divider::horizontal()
                    .color(style.divider.to_egui())
                    .paint_in(
                        ui,
                        Rect::from_min_size(pos2(body.left(), rule_y), vec2(body.width(), 1.0)),
                    );

                let nav_rect = Rect::from_min_size(
                    body.min,
                    vec2(style.nav_width.min(body.width()), body.height()),
                );
                let page_rect = Rect::from_min_max(
                    pos2(nav_rect.right() + style.column_gap, body.top()),
                    body.max,
                );

                ui.painter().rect_filled(
                    nav_rect,
                    CornerRadius::same(style.nav_corner_radius as u8),
                    style.nav_background.to_egui(),
                );
                Divider::vertical()
                    .length(body.height())
                    .color(style.divider.to_egui())
                    .paint_in(
                        ui,
                        Rect::from_min_size(
                            pos2(nav_rect.right() + style.column_gap * 0.5, body.top()),
                            vec2(1.0, body.height()),
                        ),
                    );

                ui.scope_builder(UiBuilder::new().max_rect(nav_rect.shrink(4.0)), |ui| {
                    current = page_list(ui, style, skins, current);
                });

                page.select(skins, current, out);

                ui.scope_builder(UiBuilder::new().max_rect(page_rect), |ui| {
                    page_body(ui, style, page, out);
                });

                footer(ui, style, footer_rect, out);
            });

        // A click ends the interaction: one write per gesture, however many
        // frames the drag took. Keyboard edits have no release; they persist
        // when the page changes or the panel closes.
        if page.dirty && ui.input(|input| input.pointer.any_released()) {
            page.commit(out);
        }

        *selected = current;
    }

    /// The page scrolls inside the body and the footer is pinned, so the dock
    /// must not wrap the whole panel in a second scroll area.
    fn scroll_bars(&self) -> [bool; 2] {
        [false, false]
    }

    /// The editor can edit this panel's own skin too: it is just another styled
    /// panel to the page list.
    fn skin(&self) -> Option<PanelSkin> {
        Some(PanelSkin {
            panel: PREFERENCES,
            title: "Preferences",
            file: PreferencesPanelStyle::FILE,
            fields: self.style.style_fields(),
        })
    }

    fn set_skin_field(&mut self, path: &str, value: &StyleValue) -> bool {
        self.style.set_style_field(path, value)
    }

    fn save_skin(&self, ctx: &Context) -> Result<(), String> {
        crate::skin::save_skin::<PreferencesPanelStyle>(&self.style, ctx.theme())
    }

    /// Closing with an edit still pending (a value typed but never released)
    /// saves it before the panel goes away.
    fn on_close(&mut self, _frame: &Frame, out: &mut UiOut) -> bool {
        self.page.commit(out);
        true
    }
}

/// The page list: one entry per panel with an editable skin.
fn page_list(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    skins: &[PanelSkin],
    selected: Option<PanelId>,
) -> Option<PanelId> {
    let mut chosen = selected;
    ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
    let width = ui.available_width();
    let nav = style.nav_style();

    for skin in skins {
        let is_current = Some(skin.panel) == selected;
        let response = ui.add(
            Button::new(skin.title, &nav)
                .width(width)
                .selected(is_current)
                .id(nav_id(skin.panel)),
        );
        if is_current {
            // A 2 px accent stripe reads as "this page" even when the pointer
            // is hovering another entry.
            ui.painter().rect_filled(
                Rect::from_min_size(response.rect.min, vec2(2.0, response.rect.height())),
                CornerRadius::ZERO,
                style.accent.to_egui(),
            );
        }
        if response.clicked() {
            chosen = Some(skin.panel);
        }
    }

    chosen
}

/// One page: heading, the file being written, rule, grouped rows.
fn page_body(ui: &mut Ui, style: &PreferencesPanelStyle, page: &mut PageState, out: &mut UiOut) {
    egui::ScrollArea::vertical()
        .id_salt("kairos-preferences-page")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

            let Some(panel) = page.panel else {
                TextStyle::sized(style.path.to_egui(), style.body_size).label(
                    ui,
                    "No panel skins to edit yet. Open a panel, then reopen Preferences.",
                );
                return;
            };

            TextStyle::sized(style.title.to_egui(), style.title_size)
                .strong()
                .label(ui, page.title);
            ui.add_space(2.0);
            TextStyle::sized(style.path.to_egui(), style.group_size).label(
                ui,
                format!(
                    "Preferences/Styles/{} - edits apply live, saved on release",
                    page.file
                ),
            );
            ui.add_space(style.section_spacing);
            ui.add(Divider::horizontal().color(style.divider.to_egui()));
            ui.add_space(style.section_spacing);

            let mut writer = PageWriter {
                style,
                panel,
                page,
                out,
            };
            writer.rows(ui);
        });
}

/// Draws one page's field rows.
struct PageWriter<'a> {
    style: &'a PreferencesPanelStyle,
    panel: PanelId,
    page: &'a mut PageState,
    out: &'a mut UiOut,
}

impl PageWriter<'_> {
    fn rows(&mut self, ui: &mut Ui) {
        let style = self.style;
        let mut last_group: Option<String> = None;

        for index in 0..self.page.fields.len() {
            let group = self.page.fields[index].group().to_owned();
            if last_group.as_deref() != Some(group.as_str()) {
                if !group.is_empty() {
                    ui.add_space(style.section_spacing);
                    TextStyle::sized(style.group.to_egui(), style.group_size)
                        .strong()
                        .label(ui, humanize(&group));
                    ui.add_space(style.row_gap);
                }
                last_group = Some(group);
            }
            self.row(ui, index);
            ui.add_space(style.row_gap);
        }
    }

    fn row(&mut self, ui: &mut Ui, index: usize) {
        let style = self.style;
        let panel = self.panel;
        let name = self.page.fields[index].name.clone();
        let mut changed = false;

        // Two columns while the page has room for both; below that the control
        // drops under its label, because a docked Preferences tab can be far
        // narrower than the floating default.
        let two_columns = ui.available_width() >= style.label_width + style.field_width + 24.0;
        if two_columns {
            ui.horizontal_top(|ui| {
                row_label(ui, style, &name, style.label_width);
                ui.add_space(8.0);
                changed = draw_control(ui, style, panel, &mut self.page.fields[index]);
            });
        } else {
            row_label(ui, style, &name, 0.0);
            ui.add_space(2.0);
            changed = draw_control(ui, style, panel, &mut self.page.fields[index]);
        }

        if changed {
            let (path, value) = {
                let field = &self.page.fields[index];
                (field.path.clone(), field.value.clone())
            };
            self.out
                .push(Action::Style(StyleAction::SetField { panel, path, value }));
            self.page.dirty = true;
        }
    }
}

/// A row's name, either in the fixed label column or at its natural width.
fn row_label(ui: &mut Ui, style: &PreferencesPanelStyle, text: &str, width: f32) {
    let label = TextStyle::sized(style.label.to_egui(), style.body_size);
    if width > 0.0 {
        ui.add_sized(
            vec2(width, style.row_height),
            Label::new(label.rich(text)).truncate(),
        );
    } else {
        label.label(ui, text);
    }
}

/// Close, right-aligned. Every edit is already written when the interaction
/// ends, so there is nothing to apply or cancel here.
fn footer(ui: &mut Ui, style: &PreferencesPanelStyle, rect: Rect, out: &mut UiOut) {
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(Button::new("Close", &style.button_style()).id(close_id()))
                .clicked()
            {
                out.push(Action::Layout(LayoutAction::ClosePanel {
                    panel: PREFERENCES,
                }));
            }
        });
    });
}

/// One token's editor. Returns whether it changed the field (which is updated
/// in place so the page keeps its value between frames).
fn draw_control(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    panel: PanelId,
    field: &mut StyleField,
) -> bool {
    match field.kind {
        StyleKind::Float => {
            let &StyleValue::Float(current) = &field.value else {
                return false;
            };
            let mut value = current;
            let changed = ui
                .scope(|ui| {
                    control_visuals(ui, style);
                    ui.add_sized(
                        vec2(style.field_width, style.row_height),
                        egui::DragValue::new(&mut value).speed(drag_speed(current)),
                    )
                    .changed()
                })
                .inner;
            if changed {
                field.value = StyleValue::Float(value);
            }
            changed
        }
        StyleKind::Integer { max } => {
            let &StyleValue::Integer(current) = &field.value else {
                return false;
            };
            let mut value = current;
            let changed = ui
                .scope(|ui| {
                    control_visuals(ui, style);
                    ui.add_sized(
                        vec2(style.field_width, style.row_height),
                        egui::DragValue::new(&mut value).range(0..=max).speed(1.0),
                    )
                    .changed()
                })
                .inner;
            if changed {
                field.value = StyleValue::Integer(value);
            }
            changed
        }
        StyleKind::Toggle => {
            let &StyleValue::Toggle(on) = &field.value else {
                return false;
            };
            let response = ui.add(
                Button::new(if on { "On" } else { "Off" }, &style.toggle_style())
                    .width(64.0)
                    .align(ButtonAlign::Center)
                    .selected(on)
                    .id(toggle_id(panel, &field.path)),
            );
            if response.clicked() {
                field.value = StyleValue::Toggle(!on);
                true
            } else {
                false
            }
        }
        StyleKind::Text => {
            let StyleValue::Text(current) = &field.value else {
                return false;
            };
            let mut text = current.clone();
            let changed = ui
                .scope(|ui| {
                    control_visuals(ui, style);
                    let font = TextStyle::sized(style.control_value.to_egui(), style.body_size)
                        .font_id(ui.style());
                    ui.add_sized(
                        vec2(ui.available_width().max(120.0), style.row_height),
                        TextEdit::singleline(&mut text).font(font),
                    )
                    .changed()
                })
                .inner;
            if changed {
                field.value = StyleValue::Text(text);
            }
            changed
        }
        StyleKind::Color => {
            let &StyleValue::Color([r, g, b, a]) = &field.value else {
                return false;
            };
            let mut color = Color32::new(r, g, b, a).to_egui();
            if ui.color_edit_button_srgba(&mut color).changed() {
                field.value = StyleValue::Color([color.r(), color.g(), color.b(), color.a()]);
                true
            } else {
                false
            }
        }
    }
}

/// Units per point of drag: a 3 px padding needs a finer hand than a 130 px
/// pane height, and the style knows its own magnitude.
fn drag_speed(value: f32) -> f64 {
    (value.abs() / 50.0).clamp(0.01, 1.0) as f64
}

/// Point egui's standard widgets at this panel's palette for one control.
fn control_visuals(ui: &mut Ui, style: &PreferencesPanelStyle) {
    let widgets = &mut ui.visuals_mut().widgets;
    widgets.inactive.bg_fill = style.control_rail.to_egui();
    widgets.inactive.bg_stroke = Stroke::new(1.0, style.control_edge.to_egui());
    widgets.hovered.bg_fill = style.control_hover.to_egui();
    widgets.hovered.bg_stroke = Stroke::new(1.0, style.accent.to_egui());
    widgets.active.bg_fill = style.control_active.to_egui();
    widgets.active.bg_stroke = Stroke::new(1.0, style.accent.to_egui());
    ui.visuals_mut().selection.bg_fill = style.accent.to_egui();
    ui.visuals_mut().override_text_color = Some(style.control_value.to_egui());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::HostView;
    use crate::panels;
    use kairos_editor_core::EditorState;

    struct NoHost;

    impl HostView for NoHost {}

    fn fields() -> Vec<StyleField> {
        vec![
            StyleField {
                path: "height".to_owned(),
                name: "Height".to_owned(),
                value: StyleValue::Float(20.0),
                kind: StyleKind::Float,
            },
            StyleField {
                path: "enabled".to_owned(),
                name: "Enabled".to_owned(),
                value: StyleValue::Toggle(false),
                kind: StyleKind::Toggle,
            },
            StyleField {
                path: "icons.warning".to_owned(),
                name: "Warning".to_owned(),
                value: StyleValue::Text("warning.png".to_owned()),
                kind: StyleKind::Text,
            },
        ]
    }

    fn skins() -> Vec<PanelSkin> {
        vec![
            PanelSkin {
                panel: panels::CONSOLE,
                title: "Console",
                file: "ConsoleStyle.ron",
                fields: fields(),
            },
            PanelSkin {
                panel: panels::PROFILER,
                title: "Profiler",
                file: "Profiler.ron",
                fields: fields(),
            },
        ]
    }

    /// Headless panel harness: one egui context, one frame's worth of skins and
    /// one panel. Actions are collected, not applied: they are the panel's only
    /// output.
    struct Harness {
        ctx: Context,
        size: egui::Vec2,
        state: EditorState,
        skins: Vec<PanelSkin>,
        panel: PreferencesPanel,
    }

    impl Harness {
        fn new(skins: Vec<PanelSkin>) -> Self {
            // The default floating rect is 42 % of a 1280x820 editor (537x344);
            // the dock's title bar takes 28 px, so the body sees about 316.
            Self::with_size(skins, vec2(537.0, 316.0))
        }

        fn with_size(skins: Vec<PanelSkin>, size: egui::Vec2) -> Self {
            Self {
                ctx: Context::default(),
                size,
                state: EditorState::new(),
                skins,
                panel: PreferencesPanel::new(),
            }
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> (egui::FullOutput, Vec<Action>) {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                events,
                ..Default::default()
            };

            let host = NoHost;
            let Self {
                ctx,
                state,
                skins,
                panel,
                ..
            } = self;
            let frame = Frame::with_host(state, &host).with_skins(skins);
            let mut out = UiOut::new();
            let output = ctx.run_ui(input, |ui| panel.ui(ui, &frame, &mut out));
            (output, out.take())
        }

        fn rect_of(&self, id: Id) -> egui::Rect {
            self.ctx
                .read_response(id)
                .unwrap_or_else(|| panic!("{id:?} was laid out in a previous pass"))
                .rect
        }

        /// Press and release on a widget across passes, the way a real click
        /// arrives; returns the pass that handled the release.
        fn click(&mut self, id: Id) -> (egui::FullOutput, Vec<Action>) {
            let rect = self.rect_of(id);
            self.frame(vec![egui::Event::PointerMoved(rect.center())]);
            self.frame(vec![pointer(rect.center(), true)]);
            self.frame(vec![pointer(rect.center(), false)])
        }
    }

    fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Every text run the pass painted, flattened out of the shape tree.
    fn painted_text(output: &egui::FullOutput) -> String {
        fn flatten(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_owned()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        flatten(shape, out);
                    }
                }
                _ => {}
            }
        }

        let mut out = Vec::new();
        for clipped in &output.shapes {
            flatten(&clipped.shape, &mut out);
        }
        out.join("\n")
    }

    #[test]
    fn the_page_list_shows_every_styled_panel() {
        let mut harness = Harness::new(skins());

        let (output, actions) = harness.frame(Vec::new());
        let text = painted_text(&output);

        assert!(text.contains("Console"), "{text}");
        assert!(text.contains("Profiler"), "{text}");
        // The first page's heading, file and rows.
        assert!(
            text.contains("Preferences/Styles/ConsoleStyle.ron"),
            "{text}"
        );
        assert!(text.contains("Height"), "{text}");
        assert!(text.contains("Enabled"), "{text}");
        // Nested tokens get a group heading and a leaf row.
        assert!(text.contains("Icons"), "{text}");
        assert!(text.contains("Warning"), "{text}");
        assert!(
            actions.is_empty(),
            "a plain pass emits nothing: {actions:?}"
        );
    }

    #[test]
    fn clicking_another_panel_shows_its_file() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (output, actions) = harness.click(nav_id(panels::PROFILER));
        let text = painted_text(&output);

        assert!(text.contains("Preferences/Styles/Profiler.ron"), "{text}");
        assert!(!text.contains("ConsoleStyle.ron"), "{text}");
        assert!(
            actions.is_empty(),
            "choosing a page is view state, not an action: {actions:?}"
        );
    }

    /// A click ends the interaction: the value is set, then the save is asked
    /// for — once, not once per frame.
    #[test]
    fn a_toggle_click_sets_the_field_and_commits_on_release() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (_, actions) = harness.click(toggle_id(panels::CONSOLE, "enabled"));

        assert_eq!(
            actions,
            vec![
                Action::Style(StyleAction::SetField {
                    panel: panels::CONSOLE,
                    path: "enabled".to_owned(),
                    value: StyleValue::Toggle(true),
                }),
                Action::Style(StyleAction::Commit {
                    panel: panels::CONSOLE,
                }),
            ]
        );
        assert!(
            matches!(harness.panel.page.fields[1].value, StyleValue::Toggle(true)),
            "the page keeps the edited value between frames"
        );
    }

    /// Switching pages saves the page being left, if it has an edit that was
    /// never released.
    #[test]
    fn a_pending_edit_is_saved_when_the_page_changes() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());
        harness.panel.page.dirty = true;

        let (_, actions) = harness.click(nav_id(panels::PROFILER));

        assert_eq!(
            actions,
            vec![Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE,
            })],
            "the old page's file is written once, the new page adds nothing"
        );
        assert!(!harness.panel.page.dirty);
    }

    #[test]
    fn closing_with_a_pending_edit_saves_it() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());
        harness.panel.page.dirty = true;

        let state = EditorState::new();
        let host = NoHost;
        let frame = Frame::with_host(&state, &host);
        let mut out = UiOut::new();

        assert!(
            harness.panel.on_close(&frame, &mut out),
            "close is not vetoed"
        );
        assert_eq!(
            out.as_slice(),
            &[Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE,
            })]
        );
        assert!(
            !harness.panel.page.dirty,
            "the same edit is not committed twice"
        );
    }

    #[test]
    fn close_asks_the_layout_to_close_the_panel() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (_, actions) = harness.click(close_id());

        assert_eq!(
            actions,
            vec![Action::Layout(LayoutAction::ClosePanel {
                panel: PREFERENCES
            })]
        );
    }

    #[test]
    fn an_empty_skin_list_explains_itself() {
        let mut harness = Harness::new(Vec::new());

        let (output, _) = harness.frame(Vec::new());

        assert!(
            painted_text(&output).contains("No panel skins to edit"),
            "{}",
            painted_text(&output)
        );
    }

    /// A docked Preferences tab can be much shorter than the floating default.
    /// The footer is pinned, so the action stays on screen while the page
    /// scrolls behind it.
    #[test]
    fn the_footer_survives_a_short_window() {
        let mut harness = Harness::with_size(skins(), vec2(460.0, 180.0));
        harness.frame(Vec::new());

        let rect = harness.rect_of(close_id());
        assert!(
            rect.max.y <= 180.0 && rect.max.x <= 460.0,
            "Close is outside the panel: {rect:?}"
        );
    }

    /// The editor edits its own skin too: it is just another panel in the list.
    #[test]
    fn the_preferences_panel_lists_its_own_skin() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let own = harness.panel.skin().expect("preferences has a skin");
        assert_eq!(own.panel, PREFERENCES);
        assert_eq!(own.file, "PreferencesPanel.ron");
        assert!(own.fields.iter().any(|field| field.path == "nav_width"));
    }

    /// A narrow page puts the control under its label instead of clipping it
    /// off the right edge; a wide page keeps the two-column layout.
    #[test]
    fn a_narrow_page_stacks_the_control_under_its_label() {
        let mut wide = Harness::new(skins());
        wide.frame(Vec::new());
        let wide_toggle = wide.rect_of(toggle_id(panels::CONSOLE, "enabled"));

        // 400 px: 118 nav + gaps leaves ~240 px of page, below the two-column
        // threshold, so the control drops to its own line.
        let mut narrow = Harness::with_size(skins(), vec2(400.0, 316.0));
        narrow.frame(Vec::new());
        let narrow_toggle = narrow.rect_of(toggle_id(panels::CONSOLE, "enabled"));

        assert!(
            wide_toggle.left() > narrow_toggle.left() + 100.0,
            "wide rows place the control after the label column ({wide_toggle:?}), narrow rows do not ({narrow_toggle:?})"
        );
        assert!(
            narrow_toggle.right() <= 400.0,
            "the stacked control stays inside the panel: {narrow_toggle:?}"
        );
    }
}
