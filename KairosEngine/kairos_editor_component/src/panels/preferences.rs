//! Preferences panel: every open panel's skin, as editable fields.
//!
//! This is the 0.2.0 rewrite of the legacy kairos_editor PreferencesWindow,
//! and it keeps the capability that window existed for: pick a component on
//! the left, edit its Preferences/Styles/*.ron tokens as widgets on the right.
//!
//! The review that produced this shape — the legacy window's index-based
//! readback, its per-frame TOML write, its pages keyed by position in
//! drawers, and the parts deliberately not carried over — is
//! docs/plan/editor-ui-architecture/preferences-panel.md.
//!
//! # Where the pages come from
//!
//! Each panel owns its style (loaded in Panel::init) and derives StyleRow, so
//! the style draws its own page. While this panel is open the app keeps one
//! page per live panel (a typed copy of its style) and appends the shared
//! styles' pages after them; this panel copies the page it shows and draws it.
//! The app takes the copies once, not per frame: it rebuilds the snapshot when
//! the page list changes and refreshes a page when an owner is reset, and this
//! panel copies from the snapshot only when the selected page changes.
//!
//! Editor-shell styles (the toolbar and the color picker) have no panel
//! instance. They live in the app's crate::shared_style registry and are
//! appended as fixed pages after the panel pages; their writes travel through
//! the separate SharedStyleAction path, including a Reset to code defaults.
//!
//! # The page
//!
//! A style walks itself into a row sink, so the page has no field tree of its
//! own: a nested RON struct is a full-width foldout band with a triangle, leaf
//! rows alternate between two greys, names are left-aligned, and a selected
//! field gets a blue label and a blue value outline. The row widget is the
//! widget kit's `kairos_editor_widgets::form::FormRows`; it owns that layout, the
//! fold/selection state and the edit buffers, and draws it from the shared
//! `Form` skin — not from this panel's style.
//!
//! # Live apply and save
//!
//! A control edits the page's copy; when the copy changed, the page hands the
//! whole value back (`StyleAction::Replace`), so the next pass draws from the
//! owner's fresh style — that is what makes the edit immediate. The file is
//! written once, on `StyleAction::Commit`: pointer release, the end of a typed
//! edit, page switch, or closing this panel. A Reset is the other write: it
//! replaces the whole style with the theme's code defaults and saves it in the
//! same click.

pub mod style;

use kairos_editor_widgets::color::ColorExt;

use egui::{
    Align, Context, CornerRadius, Id, Key, Margin, Rect, Sense, Ui, UiBuilder, WidgetText, pos2,
    vec2,
};
use kairos_editor_core::style::Skin;
use kairos_editor_core::{Action, SharedStyleAction, StyleAction};
use kairos_editor_widgets::button::Button;
use kairos_editor_widgets::color_picker::ColorPickerStyle;
use kairos_editor_widgets::divider::Divider;
use kairos_editor_widgets::field::FieldStyle;
use kairos_editor_widgets::form::{FormRows, FormState, FormStyle};
use kairos_editor_widgets::style::StyleFile;
use kairos_editor_widgets::text::TextStyle;

use crate::PageSkin;
use crate::frame::{Frame, UiOut};
use crate::panel::Panel;
use crate::shared_style::ListStyle;
use crate::skin::SkinId;
use crate::style_row::StylePage;

use self::style::PreferencesPanelStyle;


fn nav_id(page: impl Into<SkinId>) -> Id {
    Id::new(("kairos.preferences.nav", page.into()))
}

fn reset_id(page: impl Into<SkinId>) -> Id {
    Id::new(("kairos.preferences.reset", page.into()))
}

/// The page being edited: the owner's style, copied, plus the bookkeeping that
/// decides when the copy goes back and when its file is written.
#[derive(Default)]
struct PageState {
    /// The page the copy belongs to.
    key: Option<SkinId>,
    title: String,
    file: &'static str,
    /// The typed page this page draws: the owner's style, copied, edited here
    /// and handed back whole.
    page: Option<Box<dyn StylePage>>,
    /// The page's cross-pass state (fold, selection, edit, dirty).
    form: FormState,
    /// The page handed a new value over this pass, so a commit in the same
    /// pass must not hand it over twice.
    typed_changed: bool,
    /// The owner replaced the whole style (a factory reset). The page's copy is
    /// otherwise re-read only on a page change, so the next pass takes the
    /// owner's value (the reset values) instead of drawing the old copy.
    stale: bool,
}

impl PageState {
    /// Show the selected page's copy, committing a pending edit on the page
    /// being left. The copy is re-read on a page change, or after a reset
    /// marked it stale; ordinary edits stay in the local copy.
    fn select(&mut self, pages: &[&PageSkin], selected: Option<SkinId>, out: &mut UiOut) {
        let page = selected.and_then(|key| pages.iter().find(|page| page.id == key));

        if self.key == selected {
            let Some(page) = page else {
                return;
            };
            if self.stale {
                // The owner replaced the whole style: take its new value and
                // drop the page state (a fold or a selection may name a row the
                // new value does not have).
                self.take(page);
                self.form = FormState::default();
            }
            return;
        }

        self.commit(out);

        match page {
            Some(page) => self.take(page),
            None => {
                self.page = None;
                self.title.clear();
                self.file = "";
            }
        }
        self.form = FormState::default();
        self.typed_changed = false;
        self.key = selected;
        // A new page starts unfolded and unselected. Carrying either over
        // would highlight a path that only happens to have the same name.
        self.stale = false;
    }

    /// Take a fresh copy of the owner's style and its heading.
    fn take(&mut self, page: &PageSkin) {
        self.page = Some(page.page.clone_box());
        self.title = page.title.to_owned();
        self.file = page.file;
        self.stale = false;
    }

    /// The owner replaced the whole style. Drop an in-flight edit (its value no
    /// longer belongs to the new one) and re-read the owner's copy on the next
    /// pass.
    fn reset(&mut self) {
        self.form = FormState::default();
        self.typed_changed = false;
        self.stale = true;
    }

    /// Ask the app to persist the page being left, if it has an edit that was
    /// never released (typed text, arrow keys).
    fn commit(&mut self, out: &mut UiOut) {
        if !self.form.dirty {
            return;
        }
        // The page hands its copy over before the file write: the commit
        // persists the value the page just drew, not a frame behind it.
        if let (Some(key), Some(page)) = (self.key, self.page.as_deref())
            && !self.typed_changed
        {
            out.push(replace_action(key, page));
        }
        match self.key {
            Some(SkinId::Panel(panel)) => {
                out.push(Action::Style(StyleAction::Commit { panel }));
            }
            Some(SkinId::Shared(style)) => {
                out.push(Action::SharedStyle(SharedStyleAction::Commit { style }));
            }
            None => {}
        }
        self.form.dirty = false;
    }
}

/// The Preferences panel.
pub struct PreferencesPanel {
    style: PreferencesPanelStyle,
    /// Page list selection; falls back to the first available page.
    selected: Option<SkinId>,
    page_state: PageState,
}

impl PreferencesPanel {
    /// Build the panel. The catalog factory above is the only caller.
    pub fn new() -> Self {
        Self {
            style: PreferencesPanelStyle::default(),
            selected: None,
            page_state: PageState::default(),
        }
    }
}

impl Default for PreferencesPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[crate::panel(PREFERENCES = "preferences", skin = PreferencesPanelStyle)]
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
            page_state,
        } = self;

        // The dock builds a body Ui from the context style, so the app's
        // "chrome is not selectable" switch does not reach this panel.
        ui.style_mut().interaction.selectable_labels = false;

        // The dock paints a body colour behind every tab; a floating window uses
        // the panel's own. Painting it here keeps both cases identical.
        ui.painter().rect_filled(
            ui.max_rect(),
            CornerRadius::ZERO,
            style.background.to_egui(),
        );

        // Panel pages first (the existing editor order), then the shared
        // styles, which are always present even when no panel is open.
        let pages: Vec<&PageSkin> = frame
            .page_skins
            .iter()
            .collect();

        let picker_style = frame.color_picker_style();
        let field_style = frame.field_style();
        let list_style = frame.list_style();
        let form_style = frame.form_style().tokens();
        let mut current = match *selected {
            Some(page) if pages.iter().any(|entry| entry.id == page) => Some(page),
            _ => pages.first().map(|page| page.id),
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
                let (rule_y, _) =
                    ui.allocate_exact_size(vec2(available.x, style.footer_gap), Sense::hover());
                Divider::horizontal()
                    .color(style.divider.to_egui())
                    .paint_in(
                        ui,
                        Rect::from_min_size(
                            rule_y.min + vec2(0.0, style.footer_gap * 0.5),
                            vec2(
                                rule_y.width(),
                                style.devider_width.min(style.footer_gap * 0.5),
                            ),
                        ),
                    );
                let (footer_rect, _) =
                    ui.allocate_exact_size(vec2(available.x, style.footer_height), Sense::hover());

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
                            vec2(
                                style.devider_width.min(style.column_gap * 0.5),
                                body.height(),
                            ),
                        ),
                    );

                ui.scope_builder(
                    UiBuilder::new().max_rect(nav_rect.shrink(style.nav_shrink)),
                    |ui| {
                        current = page_list(ui, style, &pages, current);
                    },
                );

                page_state.select(&pages, current, out);

                ui.scope_builder(UiBuilder::new().max_rect(page_rect), |ui| {
                    page_body(
                        ui,
                        style,
                        &field_style,
                        list_style,
                        &form_style,
                        &picker_style,
                        page_state,
                        out,
                    );
                });

                footer(ui, style, footer_rect, page_state, out);
            });

        // A click ends the interaction: one write per gesture, however many
        // frames the drag took. Keyboard edits have no release; they persist
        // when the page changes or the panel closes, and Enter/Escape ends a
        // color hex or numeric edit immediately.
        let released = ui.input(|input| input.pointer.any_released());
        let keyboard_end =
            ui.input(|input| input.key_pressed(Key::Enter) || input.key_pressed(Key::Escape));
        if page_state.form.dirty && (released || keyboard_end) {
            page_state.commit(out);
        }

        *selected = current;
    }

    /// The page scrolls inside the body and the footer is pinned, so the dock
    /// must not wrap the whole panel in a second scroll area.
    fn scroll_bars(&self) -> [bool; 2] {
        [false, false]
    }

    /// Closing with an edit still pending (a value typed but never released)
    /// saves it before the panel goes away.
    fn on_close(&mut self, _frame: &Frame, out: &mut UiOut) -> bool {
        self.page_state.commit(out);
        true
    }
}

/// The page list: one entry per panel with an editable skin, then the shared
/// styles (toolbar, color picker), which are always present.
///
/// Rows sit on a fixed pitch: the rule between two entries is allocated like
/// any other widget, so its height is part of the list instead of a hidden
/// `item_spacing` gap that would shift every later entry down. The list also
/// scrolls inside the nav column, so a growing page count cannot paint over
/// the page body or the footer.
fn page_list(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    pages: &[&PageSkin],
    selected: Option<SkinId>,
) -> Option<SkinId> {
    let mut chosen = selected;
    let nav = style.nav_style();

    egui::ScrollArea::vertical()
        .id_salt("kairos-preferences-pages-list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let width = ui.available_width();

            for (index, page) in pages.iter().enumerate() {
                if index > 0 {
                    ui.add(
                        Divider::horizontal()
                            .thickness(style.devider_width)
                            .color(style.divider.to_egui()),
                    );
                }

                let is_current = Some(page.id) == selected;
                let response = ui.add(
                    Button::new(&page.title, &nav)
                        .width(width)
                        .selected(is_current)
                        .id(nav_id(page.id)),
                );
                if is_current {
                    // A 2 px accent stripe reads as "this page" even when the pointer
                    // is hovering another entry.
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            response.rect.min,
                            vec2(style.accent_width.min(width), response.rect.height()),
                        ),
                        CornerRadius::ZERO,
                        style.accent.to_egui(),
                    );
                }
                if response.clicked() {
                    chosen = Some(page.id);
                }
            }
        });

    chosen
}

/// One page: heading, the file being written, rule, grouped rows.
#[allow(clippy::too_many_arguments)] // four style views plus the page and its buffers
fn page_body(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    field_style: &FieldStyle,
    list_style: &ListStyle,
    form_style: &FormStyle,
    picker_style: &ColorPickerStyle,
    page_state: &mut PageState,
    out: &mut UiOut,
) {
    let page_key = page_state.key;
    egui::ScrollArea::vertical()
        .id_salt("kairos-preferences-page-body")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

            if page_key.is_none() {
                TextStyle::sized(style.path.to_egui(), style.body_size).label(
                    ui,
                    "No skins to edit yet. Open a panel, then reopen Preferences.",
                );
                return;
            }

            TextStyle::sized(style.title.to_egui(), style.title_size)
                .strong()
                .label(ui, page_state.title.clone());
            ui.add_space(style.section_spacing * 0.3);
            TextStyle::sized(style.path.to_egui(), style.body_size).label(
                ui,
                format!(
                    "Preferences/Styles/{} - edits apply live, saved on release",
                    page_state.file
                ),
            );
            ui.add_space(style.section_spacing);
            ui.add(Divider::horizontal().color(style.divider.to_egui()));

            typed_rows(
                ui,
                style,
                field_style,
                list_style,
                form_style,
                picker_style,
                page_state,
                out,
            );
        });
}

/// The widget id seed of one typed page: every row id derives from it, so focus
/// and the selection survive page changes.
fn form_seed(key: SkinId) -> Id {
    Id::new(("kairos.preferences.form", key))
}

/// A whole-style write for the page's owner. The page drew its copy, so the
/// new value goes back in one piece; no path and no per-token value.
fn replace_action(key: SkinId, page: &dyn StylePage) -> Action {
    let blob: Box<dyn kairos_editor_core::style::StyleBlob> = page.clone_box();
    let skin = Skin::from_blob(blob);
    match key {
        SkinId::Panel(panel) => Action::Style(StyleAction::Replace { panel, skin }),
        SkinId::Shared(style) => Action::SharedStyle(SharedStyleAction::Replace { style, skin }),
    }
}

/// One typed page's rows: the style draws itself into [`FormRows`], and the
/// page hands its copy back when it changed.
#[allow(clippy::too_many_arguments)]
fn typed_rows(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    field_style: &FieldStyle,
    list_style: &ListStyle,
    form_style: &FormStyle,
    picker_style: &ColorPickerStyle,
    page: &mut PageState,
    out: &mut UiOut,
) {
    let Some(key) = page.key else {
        return;
    };
    let row_width = (ui.available_width() - style.scrollbar_gap).max(1.0);
    let chrome = list_style.tokens(field_style);
    let PageState {
        page: typed,
        form,
        ..
    } = page;
    let Some(typed) = typed.as_mut() else {
        return;
    };

    let changed;
    let finished;
    let dirty;
    let clicked;
    {
        let mut rows = FormRows::new(
            ui,
            form_seed(key),
            form_style,
            field_style,
            &chrome,
            picker_style,
            form,
            row_width,
        );
        typed.draw_page(&mut rows);
        changed = rows.changed();
        finished = rows.finished();
        dirty = rows.dirty();
        clicked = rows.clicked();
    }

    if changed {
        out.push(replace_action(key, typed.as_ref()));
    }
    page.typed_changed = changed;
    // A click that no row, group or list consumed means "elsewhere on the
    // page": drop the selection, so the blue highlight follows what the user
    // last touched instead of staying on a stale row.
    if !clicked && ui.input(|input| input.pointer.any_pressed()) {
        page.form.selected = None;
    }
    if finished && dirty {
        page.commit(out);
    }
}

/// Restore-to-defaults, right-aligned. Closing already belongs to the dock
/// (the tab's × and the floating window's close control), and every edit is
/// written when its interaction ends, so the footer has nothing to apply,
/// cancel or re-read. Reset replaces the page's live style with its code
/// defaults and asks the app to write the file as part of the same reset.
fn footer(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    rect: Rect,
    page: &mut PageState,
    out: &mut UiOut,
) {
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            let Some(key) = page.key else {
                return;
            };
            if ui
                .add(
                    Button::new("Reset", &style.button_style())
                        .id(reset_id(key))
                        .tooltip("Restore this page's factory style and save it"),
                )
                .clicked()
            {
                page.reset();
                match key {
                    SkinId::Panel(panel) => {
                        out.push(Action::Style(StyleAction::Reset { panel }));
                    }
                    SkinId::Shared(shared) => {
                        out.push(Action::SharedStyle(SharedStyleAction::Reset {
                            style: shared,
                        }));
                    }
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::HostView;
    use crate::panels;
    use egui::{Color32 as EguiColor32, Key, Modifiers, Stroke, vec2};
    use kairos_color::Color;
    use kairos_editor_core::{EditorState, PanelId};
    use kairos_editor_widgets::color_picker::{
        ColorProperty, color_hex_id, color_mode_id, color_swatch_id,
    };
    use kairos_editor_widgets::field::enum_item_id;
    use kairos_editor_widgets::style::Style;

    use crate::shared_style::{
        COLOR_PICKER, ColorPickerSkin, EditorFormStyle, FONTS, FORM, LIST, ListStyle, SharedStyles,
    };
    use crate::skin::{PageSkin, SkinId};
    use crate::style_row::StyleRow;

    struct NoHost;

    impl HostView for NoHost {}

    // ---------------------------------------------------------------------
    // Fixtures: the pages are real style types, drawn by the real walk.
    // ---------------------------------------------------------------------

    /// A page that draws itself: two scalar tokens.
    #[derive(Clone, Debug, PartialEq, StyleRow)]
    struct TypedStyle {
        height: f32,
        enabled: bool,
    }

    /// The two-deep nested group the foldout and indentation tests walk.
    #[derive(Clone, Debug, PartialEq, StyleRow)]
    struct Textures {
        tint: String,
    }

    #[derive(Clone, Debug, PartialEq, StyleRow)]
    struct Icons {
        warning: String,
        error: String,
        textures: Textures,
    }

    /// The workhorse fixture: one row per leaf kind plus a two-level group,
    /// in the order the rows are asserted.
    #[derive(Clone, Debug, PartialEq, StyleRow)]
    struct FixtureStyle {
        height: f32,
        enabled: bool,
        icons: Icons,
    }

    fn fixture() -> FixtureStyle {
        FixtureStyle {
            height: 20.0,
            enabled: false,
            icons: Icons {
                warning: "warning.png".to_owned(),
                error: "error.png".to_owned(),
                textures: Textures {
                    tint: "tint.png".to_owned(),
                },
            },
        }
    }

    /// Colours only: the color picker tests.
    #[derive(Clone, Debug, PartialEq, StyleRow)]
    struct ColorStyle {
        background: Color,
        glow: ColorProperty,
    }

    fn color_style() -> ColorStyle {
        ColorStyle {
            background: Color::from_srgb8([20, 140, 220, 255]),
            glow: ColorProperty::hdr(Color::new(2.0, 1.0, 0.5, 1.0)),
        }
    }

    /// A long page: the colour rows sit at the top, the list below the fold.
    #[derive(Clone, Debug, PartialEq, StyleRow)]
    struct LongStyle {
        background: Color,
        glow: ColorProperty,
        tokens: Vec<f32>,
    }

    fn long_style() -> LongStyle {
        LongStyle {
            background: Color::from_srgb8([20, 140, 220, 255]),
            glow: ColorProperty::hdr(Color::new(2.0, 1.0, 0.5, 1.0)),
            tokens: (0..40).map(|index| index as f32).collect(),
        }
    }

    /// A style enum with a payload: the menu row plus the active variant's
    /// rows, in the enum's own payload scope.
    #[derive(Clone, Debug, PartialEq, StyleRow, strum::VariantNames)]
    enum Mode {
        Off,
        Soft { radius: f32, samples: u32 },
    }

    #[derive(Clone, Debug, PartialEq, StyleRow)]
    struct EnumStyle {
        mode: Mode,
    }

    fn enum_style(mode: Mode) -> EnumStyle {
        EnumStyle { mode }
    }

    /// An empty style: a page-list row with no token rows of its own.
    #[derive(Clone, Debug, Default, PartialEq, StyleRow)]
    struct EmptyStyle {}

    fn page_skin(
        id: SkinId,
        title: &str,
        file: &'static str,
        style: impl crate::style_row::StylePage + 'static,
    ) -> PageSkin {
        PageSkin {
            id,
            title: title.to_owned(),
            file,
            page: Box::new(style),
        }
    }

    /// Two live panels, both drawn by the workhorse fixture.
    fn skins() -> Vec<PageSkin> {
        vec![
            page_skin(
                SkinId::Panel(panels::CONSOLE),
                "Console",
                "ConsoleStyle.ron",
                fixture(),
            ),
            page_skin(
                SkinId::Panel(panels::PROFILER),
                "Profiler",
                "Profiler.ron",
                fixture(),
            ),
        ]
    }

    /// One page per built-in panel, enough rows to overflow the nav column.
    fn every_panel() -> Vec<PageSkin> {
        [
            panels::ABOUT,
            panels::CONSOLE,
            panels::HIERARCHY,
            panels::INSPECTOR,
            panels::PREFERENCES,
            panels::PROFILER,
            panels::PROJECT,
            panels::SCENE,
        ]
        .into_iter()
        .map(|panel| {
            page_skin(
                SkinId::Panel(panel),
                &format!("Panel {}", panel.0),
                "Test.ron",
                EmptyStyle {},
            )
        })
        .collect()
    }

    fn color_skins() -> Vec<PageSkin> {
        vec![page_skin(
            SkinId::Panel(panels::CONSOLE),
            "Console",
            "ConsoleStyle.ron",
            color_style(),
        )]
    }

    /// The picker opens near the top of the page while its own lower rows sit
    /// outside the page viewport.
    fn long_color_skin() -> Vec<PageSkin> {
        vec![page_skin(
            SkinId::Panel(panels::CONSOLE),
            "Console",
            "ConsoleStyle.ron",
            long_style(),
        )]
    }

    /// Headless panel harness: one egui context, one page list and one panel.
    /// The list is the app's: the open panels' pages, then the shell's shared
    /// styles, which exist even when no panel is open.
    struct Harness {
        ctx: Context,
        /// The panel's rect; the screen can be larger so a floating picker has
        /// room outside the panel, like it does in the editor.
        size: egui::Vec2,
        screen: egui::Vec2,
        state: EditorState,
        pages: Vec<PageSkin>,
        /// The live shared styles the pass reads; tests may edit one to prove
        /// the page is wired to the registry and not to local constants.
        shared: SharedStyles,
        panel: PreferencesPanel,
    }

    impl Harness {
        fn new(pages: Vec<PageSkin>) -> Self {
            // The default floating rect is 42 % of a 1280x820 editor (537x344);
            // the dock's title bar takes 28 px, so the body sees about 316.
            Self::with_size(pages, vec2(537.0, 316.0))
        }

        fn with_size(pages: Vec<PageSkin>, size: egui::Vec2) -> Self {
            Self::with_screen(pages, size, size)
        }

        fn with_screen(pages: Vec<PageSkin>, size: egui::Vec2, screen: egui::Vec2) -> Self {
            Self {
                ctx: Context::default(),
                size,
                screen,
                state: EditorState::new(),
                pages,
                shared: SharedStyles::default(),
                panel: PreferencesPanel::new(),
            }
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> (egui::FullOutput, Vec<Action>) {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.screen)),
                events,
                ..Default::default()
            };

            let host = NoHost;
            let Self {
                ctx,
                size,
                screen,
                state,
                pages,
                shared,
                panel,
            } = self;
            let size = *size;
            let panel_count = pages.len();
            let mut list = std::mem::take(pages);
            list.extend(shared.pages());
            let frame = Frame::new(state, &host, shared).with_pages(&list);
            let mut out = UiOut::new();
            let output = ctx.run_ui(input, |ui| {
                if *screen == size {
                    panel.ui(ui, &frame, &mut out);
                } else {
                    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
                    let mut panel_ui =
                        ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(*ui.layout()));
                    panel_ui.set_clip_rect(rect);
                    panel.ui(&mut panel_ui, &frame, &mut out);
                }
            });
            *pages = list.into_iter().take(panel_count).collect();
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
            self.click_at(rect.center())
        }

        fn click_at(&mut self, pos: egui::Pos2) -> (egui::FullOutput, Vec<Action>) {
            self.frame(vec![egui::Event::PointerMoved(pos)]);
            self.frame(vec![pointer(pos, true)]);
            self.frame(vec![pointer(pos, false)])
        }

        /// The copy the selected page is drawing.
        fn page_style<T: 'static>(&self) -> &T {
            self.panel
                .page_state
                .page
                .as_deref()
                .expect("a page is selected")
                .as_any()
                .downcast_ref::<T>()
                .expect("the selected page is another style type")
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

    // ---------------------------------------------------------------------
    // Widget ids: the row walk seeds every id from the page, so a test reads
    // the same id the widget used.
    // ---------------------------------------------------------------------

    fn seed(key: impl Into<SkinId>) -> Id {
        form_seed(key.into())
    }

    fn field(key: impl Into<SkinId>, path: &str) -> Id {
        seed(key).with(("field", path))
    }

    fn label(key: impl Into<SkinId>, path: &str) -> Id {
        seed(key).with(("label", path))
    }

    fn group(key: impl Into<SkinId>, path: &str) -> Id {
        seed(key).with(("group", path))
    }

    fn toggle(key: impl Into<SkinId>, path: &str) -> Id {
        seed(key).with(("toggle", path))
    }

    fn list_add(key: impl Into<SkinId>, path: &str) -> Id {
        seed(key).with(("list.add", path))
    }

    fn list_remove(key: impl Into<SkinId>, path: &str) -> Id {
        seed(key).with(("list.remove", path))
    }

    fn list_handle(key: impl Into<SkinId>, path: &str) -> Id {
        seed(key).with(("list.handle", path))
    }

    fn list_chrome(key: impl Into<SkinId>, path: &str) -> Id {
        seed(key).with(("list", path))
    }

    // ---------------------------------------------------------------------
    // Painted output: what the pass actually drew.
    // ---------------------------------------------------------------------

    #[derive(Clone, Debug)]
    struct PaintedText {
        text: String,
        rect: egui::Rect,
    }

    #[derive(Clone, Debug)]
    struct PaintedRect {
        rect: egui::Rect,
        fill: EguiColor32,
        stroke: Stroke,
    }

    fn painted_texts(output: &egui::FullOutput) -> Vec<PaintedText> {
        fn walk(shape: &egui::Shape, out: &mut Vec<PaintedText>) {
            match shape {
                egui::Shape::Text(text) => out.push(PaintedText {
                    text: text.galley.text().to_owned(),
                    rect: egui::Rect::from_min_size(text.pos, text.galley.size()),
                }),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }

        let mut out = Vec::new();
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    /// A text run plus the clip the pass painted it through. The ghost's
    /// clipping is invisible in the flattened text list, so the drag tests
    /// inspect this to prove the whole block survives the scroll viewport.
    #[derive(Clone, Debug)]
    struct ClippedText {
        text: String,
        rect: egui::Rect,
        clip: egui::Rect,
    }

    fn painted_clipped_texts(output: &egui::FullOutput) -> Vec<ClippedText> {
        fn walk(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<ClippedText>) {
            match shape {
                egui::Shape::Text(text) => out.push(ClippedText {
                    text: text.galley.text().to_owned(),
                    rect: egui::Rect::from_min_size(text.pos, text.galley.size()),
                    clip,
                }),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, clip, out);
                    }
                }
                _ => {}
            }
        }

        let mut out = Vec::new();
        for clipped in &output.shapes {
            walk(&clipped.shape, clipped.clip_rect, &mut out);
        }
        out
    }

    fn painted_rects(output: &egui::FullOutput) -> Vec<PaintedRect> {
        fn walk(shape: &egui::Shape, out: &mut Vec<PaintedRect>) {
            match shape {
                egui::Shape::Rect(rect) => out.push(PaintedRect {
                    rect: rect.rect,
                    fill: rect.fill,
                    stroke: rect.stroke,
                }),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, out);
                    }
                }
                _ => {}
            }
        }

        let mut out = Vec::new();
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    /// Every text run the pass painted, flattened out of the shape tree.
    fn painted_text(output: &egui::FullOutput) -> String {
        painted_texts(output)
            .into_iter()
            .map(|run| run.text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn run<'a>(runs: &'a [PaintedText], text: &str) -> &'a PaintedText {
        runs.iter().find(|run| run.text == text).unwrap_or_else(|| {
            panic!(
                "no {text:?} text run: {:?}",
                runs.iter().map(|run| &run.text).collect::<Vec<_>>()
            )
        })
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    // ---------------------------------------------------------------------
    // Page list and navigation
    // ---------------------------------------------------------------------

    #[test]
    fn the_page_list_shows_every_style_and_its_rows() {
        let mut harness = Harness::new(skins());

        let (output, actions) = harness.frame(Vec::new());
        let text = painted_text(&output);

        assert!(text.contains("Console"), "{text}");
        assert!(text.contains("Profiler"), "{text}");
        // Shared styles are always listed after the live panel styles.
        assert!(text.contains("Toolbar"), "{text}");
        assert!(text.contains("Color Picker"), "{text}");
        // The first page's heading, file and rows.
        assert!(
            text.contains("Preferences/Styles/ConsoleStyle.ron"),
            "{text}"
        );
        assert!(text.contains("Height"), "{text}");
        assert!(text.contains("Enabled"), "{text}");
        // A nested struct is a foldout band, not a dim label.
        assert!(text.contains("Icons"), "{text}");
        assert!(text.contains("Warning"), "{text}");
        assert!(text.contains("Textures"), "{text}");
        assert!(text.contains("Tint"), "{text}");
        assert!(
            actions.is_empty(),
            "a plain pass emits nothing: {actions:?}"
        );
    }

    /// A row owns exactly its own height: the rule between two entries is a
    /// widget, so it cannot hide a per-row gap that shifts every later entry
    /// down until one lands outside the nav column.
    #[test]
    fn the_page_list_rows_are_separated_by_an_allocated_rule() {
        let mut harness = Harness::new(skins());
        let (output, _) = harness.frame(Vec::new());

        let console = harness.rect_of(nav_id(panels::CONSOLE));
        let profiler = harness.rect_of(nav_id(panels::PROFILER));
        let style = PreferencesPanelStyle::dark();

        let rule = painted_rects(&output)
            .into_iter()
            .find(|painted| {
                painted.fill == style.divider.to_egui()
                    && approx(painted.rect.width(), console.width())
                    && painted.rect.top() >= console.bottom() - 0.5
                    && painted.rect.bottom() <= profiler.top() + 0.5
                    && painted.rect.height() > 0.5
            })
            .expect("a rule is painted between the two entries");
        assert!(
            approx(profiler.top() - console.bottom(), rule.rect.height()),
            "the whole gap between entries is the painted rule, not item_spacing"
        );
        assert!(
            approx(rule.rect.left(), console.left()) && approx(rule.rect.right(), console.right()),
            "the rule spans the entry"
        );
    }

    /// A long list is clipped to the nav column and scrolls, instead of painting
    /// entries over the page body and the footer.
    #[test]
    fn a_page_list_too_long_for_the_nav_column_scrolls_inside_it() {
        let mut harness = Harness::with_size(every_panel(), vec2(460.0, 180.0));
        let (output, _) = harness.frame(Vec::new());
        let style = PreferencesPanelStyle::dark();
        let nav = painted_rects(&output)
            .into_iter()
            .find(|painted| painted.fill == style.nav_background.to_egui())
            .expect("the nav column is painted")
            .rect;

        // The shared styles sort after the panel pages, and Toolbar is the last
        // shared id; it is the entry that would land under the footer without a
        // scroll viewport.
        let last = painted_clipped_texts(&output)
            .into_iter()
            .find(|run| run.text == "Toolbar")
            .expect("the last shared page is laid out");
        assert!(
            last.rect.top() > nav.bottom(),
            "the last entry is below the column: entry {:?}, nav {nav:?}",
            last.rect
        );
        assert!(
            last.clip.bottom() <= nav.bottom() + 0.01,
            "the list is clipped at the column instead of spilling into the footer"
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

    /// The editor edits its own skin too: it is just another panel in the list.
    #[test]
    fn the_preferences_panel_lists_its_own_page() {
        let harness = Harness::new(vec![page_skin(
            SkinId::Panel(panels::PREFERENCES),
            "Preferences",
            "PreferencesPanel.ron",
            PreferencesPanelStyle::dark(),
        )]);

        let frame = Frame::new(&harness.state, &NoHost, &harness.shared);
        let page = harness
            .panel
            .skin_page(&frame)
            .expect("preferences has a page");
        assert_eq!(page.id, SkinId::Panel(panels::PREFERENCES));
        assert_eq!(page.file, "PreferencesPanel.ron");
        assert_eq!(
            page.title,
            harness.panel.title(&frame).text(),
            "the page is named by the panel's own title, not a second literal"
        );
    }

    // ---------------------------------------------------------------------
    // Write-back: the page hands the owner a whole value
    // ---------------------------------------------------------------------

    /// The generated `replace_skin` takes the page's whole typed copy and
    /// rejects a box of another type: the write path has no paths and no
    /// per-token dispatch.
    #[test]
    fn a_whole_style_replaces_the_panels_live_skin() {
        let mut panel = PreferencesPanel::new();
        let mut style = PreferencesPanelStyle::default();
        style.nav_width += 5.0;

        assert!(panel.replace_skin(Box::new(style.clone())));
        assert_eq!(panel.style, style);
        assert!(
            !panel.replace_skin(Box::new(0u8)),
            "another type is a no-op"
        );
        assert_eq!(panel.style, style);
    }

    /// The typed page path: the style draws itself, a toggle writes into the
    /// page's copy, and the page hands the whole value back once. The file
    /// write stays the separate `Commit` gesture.
    #[test]
    fn a_typed_page_draws_itself_and_hands_back_the_whole_value() {
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, 520.0));
        let panel = panels::CONSOLE;
        harness.pages = vec![page_skin(
            SkinId::Panel(panel),
            "Typed",
            "Typed.ron",
            TypedStyle {
                height: 20.0,
                enabled: true,
            },
        )];

        let _ = harness.frame(Vec::new());
        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);
        assert!(text.contains("Typed"), "{text}");
        assert!(text.contains("Typed.ron"), "{text}");
        assert!(text.contains("Height"), "{text}");
        assert!(text.contains("Enabled"), "{text}");

        let (_, actions) = harness.click(toggle(panel, "enabled"));

        assert_eq!(actions.len(), 2, "one replace, one commit: {actions:?}");
        match &actions[0] {
            Action::Style(StyleAction::Replace { panel: id, skin }) => {
                assert_eq!(*id, panel);
                assert_eq!(
                    *skin,
                    Skin::new(TypedStyle {
                        height: 20.0,
                        enabled: false,
                    }),
                    "the page hands back the value it drew"
                );
            }
            other => panic!("expected a whole-style write, got {other:?}"),
        }
        assert_eq!(actions[1], Action::Style(StyleAction::Commit { panel }));
        assert!(!harness.page_style::<TypedStyle>().enabled);
    }

    /// A click ends the interaction: the copy is handed over, then the save is
    /// asked for — once, not once per frame.
    #[test]
    fn a_toggle_click_writes_the_copy_and_commits_on_release() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (_, actions) = harness.click(toggle(panels::CONSOLE, "enabled"));

        assert_eq!(
            actions,
            vec![
                Action::Style(StyleAction::Replace {
                    panel: panels::CONSOLE,
                    skin: Skin::new(FixtureStyle {
                        enabled: true,
                        ..fixture()
                    }),
                }),
                Action::Style(StyleAction::Commit {
                    panel: panels::CONSOLE,
                }),
            ]
        );
        assert!(
            harness.page_style::<FixtureStyle>().enabled,
            "the page keeps the edited value between frames"
        );
    }

    /// Switching pages saves the page being left, if it has an edit that was
    /// never released: the value goes over before the file write.
    #[test]
    fn switching_pages_hands_over_and_commits_the_edit() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());
        harness.panel.page_state.form.dirty = true;

        let (_, actions) = harness.click(nav_id(panels::PROFILER));

        assert_eq!(
            actions.len(),
            2,
            "a replace and a commit for the page being left: {actions:?}"
        );
        assert!(matches!(
            actions[0],
            Action::Style(StyleAction::Replace {
                panel: panels::CONSOLE,
                ..
            })
        ));
        assert_eq!(
            actions[1],
            Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE,
            })
        );
        assert!(!harness.panel.page_state.form.dirty);
    }

    #[test]
    fn closing_with_a_pending_edit_saves_it() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());
        harness.panel.page_state.form.dirty = true;

        let state = EditorState::new();
        let host = NoHost;
        let frame = Frame::new(&state, &host, &harness.shared);
        let mut out = UiOut::new();

        assert!(
            harness.panel.on_close(&frame, &mut out),
            "close is not vetoed"
        );
        assert_eq!(out.len(), 2, "hand the value over, then write: {out:?}");
        assert!(
            !harness.panel.page_state.form.dirty,
            "the same edit is not committed twice"
        );
    }

    /// Reset replaces the page's live style with its code defaults. Closing is
    /// the dock's job, so the footer has no close action of its own.
    #[test]
    fn the_footer_reset_button_asks_the_owner_for_code_defaults() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());
        harness.panel.page_state.form.dirty = true;

        let (_, actions) = harness.click(reset_id(panels::CONSOLE));

        assert_eq!(
            actions,
            vec![Action::Style(StyleAction::Reset {
                panel: panels::CONSOLE
            })]
        );
        assert!(
            harness.panel.page_state.stale,
            "the next pass re-reads the owner's value"
        );
        assert!(
            !harness.panel.page_state.form.dirty,
            "a reset discards the pending edit instead of committing it"
        );
    }

    /// The footer has one meaningful action. Close belongs to the dock (the
    /// tab's × and the floating window's close control), and Reload is gone
    /// because every edit is already written when its interaction ends.
    #[test]
    fn the_footer_offers_reset_without_close_or_reload() {
        let mut harness = Harness::new(Vec::new());
        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);

        assert!(text.contains("Reset"), "the footer offers Reset: {text}");
        assert!(!text.contains("Close"), "Close is the dock's job: {text}");
        assert!(
            !text.contains("Reload"),
            "there is nothing to reload: {text}"
        );
    }

    /// A docked Preferences tab can be much shorter than the floating default.
    /// The footer is pinned, so the action stays on screen while the page
    /// scrolls behind it.
    #[test]
    fn the_footer_survives_a_short_window() {
        let mut harness = Harness::with_size(skins(), vec2(460.0, 180.0));
        harness.frame(Vec::new());

        let rect = harness.rect_of(reset_id(panels::CONSOLE));
        assert!(
            rect.max.y <= 180.0 && rect.max.x <= 460.0,
            "Reset is outside the panel: {rect:?}"
        );
    }

    // ---------------------------------------------------------------------
    // Shared styles: the shell owns them, the same page protocol edits them
    // ---------------------------------------------------------------------

    /// Shared styles are always available: they are pages even when no panel
    /// is open, which is the point of moving them out of panel instances.
    #[test]
    fn shared_styles_are_editable_without_any_panel_open() {
        let mut harness = Harness::new(Vec::new());

        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);

        assert!(text.contains("Color Picker"), "{text}");
        assert!(text.contains("Toolbar"), "{text}");
        assert!(
            text.contains("Preferences/Styles/ColorPicker.ron"),
            "the first shared page shows its file: {text}"
        );
        assert!(!text.contains("No skins to edit"), "{text}");
    }

    /// Choosing a shared page is view state, like choosing a panel page.
    #[test]
    fn clicking_a_shared_style_shows_its_file() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (output, actions) = harness.click(nav_id(COLOR_PICKER));
        let text = painted_text(&output);

        assert!(
            text.contains("Preferences/Styles/ColorPicker.ron"),
            "{text}"
        );
        assert!(
            actions.is_empty(),
            "choosing a page is view state, not an action: {actions:?}"
        );
    }

    /// The font list is a shared page like the chrome: it exists with no panel
    /// open, and its list rows are ordinary tokens.
    #[test]
    fn the_font_list_has_a_shared_page() {
        let mut harness = Harness::new(Vec::new());
        harness.frame(Vec::new());

        let (output, actions) = harness.click(nav_id(FONTS));
        let text = painted_text(&output);

        assert!(
            text.contains("Preferences/Styles/EditorFonts.ron"),
            "{text}"
        );
        assert!(text.contains("Proportional"), "{text}");
        assert!(
            text.contains("Preferences/Fonts/Inter-Regular.otf"),
            "the row shows the font path: {text}"
        );
        assert!(
            actions.is_empty(),
            "choosing a page emits nothing: {actions:?}"
        );
    }

    /// A shared trait object draws itself into the same rows as a panel page.
    #[test]
    fn a_shared_style_page_draws_itself() {
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, 620.0));

        let _ = harness.frame(Vec::new());
        let _ = harness.click(nav_id(SkinId::Shared(LIST)));
        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);
        assert!(text.contains("List"), "{text}");
        assert!(text.contains("Row height"), "{text}");
    }

    /// A real style carries lists and an enum payload; the typed page draws
    /// all of those without a field table (the synthetic page above is two
    /// scalars).
    #[test]
    fn a_real_style_page_draws_its_lists_and_enum_payloads() {
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, 700.0));
        let panel = PanelId("fonts");
        harness.pages = vec![page_skin(
            SkinId::Panel(panel),
            "Fonts",
            "EditorFonts.ron",
            crate::shared_style::EditorFonts::dark(),
        )];

        let _ = harness.frame(Vec::new());
        let _ = harness.click(nav_id(SkinId::Panel(panel)));
        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);
        assert!(text.contains("Proportional"), "{text}");
        assert!(text.contains("Element 0"), "{text}");
        assert!(text.contains("Priority"), "{text}");
    }

    /// Shared edits use the separate action path: they have no panel instance
    /// to route the page's copy to.
    #[test]
    fn a_shared_commit_hands_the_value_over_and_asks_for_the_file() {
        let mut page = PageState {
            key: Some(SkinId::Shared(COLOR_PICKER)),
            page: Some(Box::new(ColorPickerSkin::dark())),
            ..PageState::default()
        };
        page.form.dirty = true;
        let mut out = UiOut::new();

        page.commit(&mut out);

        let actions = out.take();
        assert_eq!(actions.len(), 2, "{actions:?}");
        match &actions[0] {
            Action::SharedStyle(SharedStyleAction::Replace { style, skin }) => {
                assert_eq!(*style, COLOR_PICKER);
                assert_eq!(*skin, Skin::new(ColorPickerSkin::dark()));
            }
            other => panic!("expected a whole shared-style write, got {other:?}"),
        }
        assert_eq!(
            actions[1],
            Action::SharedStyle(SharedStyleAction::Commit {
                style: COLOR_PICKER
            })
        );
    }

    /// A shared page resets through its own action path: the registry owns the
    /// live value, not a panel instance.
    #[test]
    fn a_shared_page_reset_button_asks_the_registry_for_code_defaults() {
        let mut harness = Harness::new(Vec::new());
        harness.frame(Vec::new());

        let (_, actions) = harness.click(reset_id(COLOR_PICKER));

        assert_eq!(
            actions,
            vec![Action::SharedStyle(SharedStyleAction::Reset {
                style: COLOR_PICKER
            })]
        );
    }

    /// Changing a ListStyle token in the registry must repaint the page's list:
    /// the page reads Frame::list_style(), not a local constant or the panel
    /// skin.
    #[test]
    fn the_list_skin_restyles_the_preferences_list() {
        let mut harness = Harness::with_size(skins(), vec2(700.0, 470.0));
        let ink = Color::from_srgb8([0x12, 0x34, 0x56, 255]);
        let mut list = ListStyle::dark();
        list.band_fill = ink;
        assert!(
            harness.shared.replace(LIST, Box::new(list)),
            "the list skin is registered"
        );
        harness.frame(Vec::new());
        harness.click(nav_id(FONTS));
        let (output, _) = harness.frame(Vec::new());

        assert!(
            painted_rects(&output)
                .iter()
                .any(|painted| painted.fill == ink.to_egui()),
            "the element band uses the registry's fill"
        );
    }

    // ---------------------------------------------------------------------
    // Colour rows: the picker, its popup and the HDR badge
    // ---------------------------------------------------------------------

    #[test]
    fn color_fields_open_our_picker_and_write_the_style() {
        let mut harness = Harness::with_size(color_skins(), vec2(700.0, 800.0));
        harness.frame(Vec::new());

        let field = field(panels::CONSOLE, "background");
        let (_, actions) = harness.click(field);
        assert!(
            actions.is_empty(),
            "opening the picker is not a value change: {actions:?}"
        );
        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);
        assert!(text.contains("Swatches"), "{text}");
        assert!(text.contains("Hexadecimal"), "{text}");

        let swatch = color_swatch_id(field, 4);
        assert!(
            harness.ctx.read_response(swatch).is_some(),
            "the picker paints its palette"
        );
        let (_, actions) = harness.click(swatch);

        let mut expected = color_style();
        expected.background = Color::from_srgb8([0, 0, 0, 255]);
        assert_eq!(actions.len(), 2, "one replace, one commit: {actions:?}");
        assert_eq!(
            actions[0],
            Action::Style(StyleAction::Replace {
                panel: panels::CONSOLE,
                skin: Skin::new(expected.clone()),
            })
        );
        assert_eq!(
            actions[1],
            Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE,
            })
        );
        assert_eq!(
            harness.page_style::<ColorStyle>().background,
            expected.background,
            "the page keeps the picked colour"
        );
        assert_eq!(
            harness.panel.page_state.form.selected.as_deref(),
            Some("background"),
            "working inside the popup keeps the row selected"
        );
    }

    #[test]
    fn an_hdr_field_shows_the_badge_and_the_intensity_picker() {
        let mut harness = Harness::with_size(color_skins(), vec2(700.0, 800.0));
        harness.frame(Vec::new());

        let field = field(panels::CONSOLE, "glow");
        let (_, actions) = harness.click(field);
        assert!(actions.is_empty(), "{actions:?}");
        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);
        assert!(text.contains("HDR"), "{text}");
        assert!(text.contains("HDR Color"), "{text}");
        assert!(text.contains("Intensity"), "{text}");
        assert!(text.contains("+2"), "{text}");
    }

    /// Reset is an ordinary outside click: the picker must close itself even
    /// while one of its own menus is open, with no close call in the panel.
    #[test]
    fn reset_closes_a_picker_whose_mode_menu_is_open() {
        let mut harness = Harness::with_size(color_skins(), vec2(700.0, 800.0));
        harness.frame(Vec::new());
        let field = field(panels::CONSOLE, "background");
        harness.click(field);
        harness.frame(Vec::new());
        let mode = color_mode_id(field);
        harness.click(mode);
        harness.frame(Vec::new());

        let (_, actions) = harness.click(reset_id(panels::CONSOLE));
        assert_eq!(
            actions,
            vec![Action::Style(StyleAction::Reset {
                panel: panels::CONSOLE
            })]
        );
    }

    /// The picker's own text edits must not scroll the page behind it. The
    /// picker owns its scroll-to-cursor requests, so the page ScrollArea never
    /// sees them; the panel does not pin the offset.
    #[test]
    fn typing_in_the_picker_does_not_scroll_the_page() {
        let mut harness =
            Harness::with_screen(long_color_skin(), vec2(537.0, 316.0), vec2(900.0, 900.0));
        harness.frame(Vec::new());

        let field = field(panels::CONSOLE, "background");
        harness.click(field);
        harness.frame(Vec::new());
        let hex = color_hex_id(field);
        harness.click(hex);
        // The click seeds the edit; one more pass lets the TextEdit take focus.
        harness.frame(Vec::new());
        assert!(
            harness.ctx.memory(|memory| memory.has_focus(hex)),
            "hex not focused"
        );

        let probe = label(panels::CONSOLE, "tokens.0");
        let before = harness.rect_of(probe);
        let (_, actions) = harness.frame(vec![egui::Event::Text("#00FF00".to_owned())]);
        assert!(!actions.is_empty(), "typing did not reach the page");
        // A leaked scroll target animates in over the following frames.
        for _ in 0..20 {
            harness.frame(Vec::new());
        }
        let after = harness.rect_of(probe);
        assert_eq!(
            before, after,
            "typing in the picker moved the page under it"
        );
    }

    /// The picker ignores the outside press while its own menu is open, so the
    /// tab click can move the page while the picker is still in temp data. The
    /// picker's own lifecycle must drop it, so it cannot pin the new page or
    /// reopen on the page it left.
    #[test]
    fn switching_pages_closes_a_picker_whose_mode_menu_is_open() {
        let mut harness = Harness::with_size(color_skins(), vec2(700.0, 800.0));
        harness.frame(Vec::new());

        let field = field(panels::CONSOLE, "background");
        harness.click(field);
        harness.frame(Vec::new());
        let mode = color_mode_id(field);
        assert!(harness.ctx.read_response(mode).is_some());
        harness.click(mode);
        harness.frame(Vec::new());

        harness.click(nav_id(FONTS));
        assert_eq!(harness.panel.selected, Some(SkinId::Shared(FONTS)));
    }

    // ---------------------------------------------------------------------
    // Rows: stripes, indentation, folding, selection
    // ---------------------------------------------------------------------

    #[test]
    fn field_rows_alternate_between_two_greys() {
        let mut harness = Harness::new(skins());
        let form = harness
            .shared
            .form_style()
            .expect("the form skin is registered")
            .tokens();

        let (output, _) = harness.frame(Vec::new());

        let odd = form.row_odd;
        let even = form.row_even;
        let mut bands: Vec<PaintedRect> = painted_rects(&output)
            .into_iter()
            .filter(|rect| {
                (rect.fill == odd || rect.fill == even)
                    && approx(rect.rect.height(), form.row_height)
            })
            .collect();
        bands.sort_by(|a, b| a.rect.top().total_cmp(&b.rect.top()));

        assert_eq!(bands.len(), 5, "one band per leaf row: {bands:?}");
        let expected = [odd, even, odd, even, odd];
        let actual: Vec<EguiColor32> = bands.iter().map(|band| band.fill).collect();
        assert_eq!(actual, expected, "the five leaf rows stripe odd/even");
    }

    /// The form rows draw from the shared `Form` skin, not from the panel: a
    /// live replacement restripes them on the next pass, the same way every
    /// other shared style reaches its widget.
    #[test]
    fn the_form_rows_draw_from_the_shared_skin() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let mut skin = EditorFormStyle::dark();
        skin.row_odd = Color::from_srgb8([1, 2, 3, 255]);
        assert!(
            harness.shared.replace(FORM, Box::new(skin.clone())),
            "the form skin is registered"
        );

        let (output, _) = harness.frame(Vec::new());
        assert!(
            painted_rects(&output)
                .iter()
                .any(|painted| painted.fill == skin.row_odd.to_egui()),
            "the edited form skin is what the rows paint"
        );
    }

    #[test]
    fn labels_are_left_aligned_and_indented_by_group_depth() {
        let mut harness = Harness::new(skins());
        let form = harness
            .shared
            .form_style()
            .expect("the form skin is registered")
            .tokens();

        let (output, _) = harness.frame(Vec::new());
        let runs = painted_texts(&output);
        let height = run(&runs, "Height");
        let enabled = run(&runs, "Enabled");
        let warning = run(&runs, "Warning");
        let tint = run(&runs, "Tint");

        assert!(
            approx(height.rect.left(), enabled.rect.left()),
            "left-aligned labels share an edge: Height {:?}, Enabled {:?}",
            height.rect,
            enabled.rect
        );
        assert!(
            approx(warning.rect.left(), height.rect.left() + form.group_indent),
            "a group's leaf is indented one level: Warning {:?} vs Height {:?}",
            warning.rect,
            height.rect
        );
        assert!(
            approx(
                tint.rect.left(),
                height.rect.left() + 2.0 * form.group_indent
            ),
            "a nested group's leaf is indented two levels: Tint {:?}",
            tint.rect
        );

        // The value column does not move with the label's depth.
        let height_box = harness.rect_of(field(panels::CONSOLE, "height"));
        let warning_box = harness.rect_of(field(panels::CONSOLE, "icons.warning"));
        assert!(
            approx(height_box.left(), warning_box.left()),
            "value boxes share one column: {height_box:?} vs {warning_box:?}"
        );
        assert!(
            height.rect.right() < height_box.left(),
            "the name column ends before the value column"
        );
    }

    #[test]
    fn clicking_a_foldout_collapses_its_rows() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (output, actions) = harness.click(group(panels::CONSOLE, "icons"));
        let text = painted_text(&output);
        assert!(text.contains("Icons"), "{text}");
        assert!(!text.contains("Warning"), "{text}");
        assert!(!text.contains("Error"), "{text}");
        assert!(
            !text.contains("Textures"),
            "nested headers fold too: {text}"
        );
        assert!(!text.contains("Tint"), "{text}");
        assert!(
            actions.is_empty(),
            "folding is view state, not an action: {actions:?}"
        );
        assert!(
            harness.panel.page_state.form.collapsed.contains("icons"),
            "the fold is remembered across passes"
        );

        let (output, _) = harness.click(group(panels::CONSOLE, "icons"));
        let text = painted_text(&output);
        assert!(text.contains("Warning"), "{text}");
        assert!(text.contains("Textures"), "{text}");
        assert!(text.contains("Tint"), "{text}");
    }

    #[test]
    fn a_nested_foldout_collapses_only_its_own_rows() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (output, _) = harness.click(group(panels::CONSOLE, "icons.textures"));
        let text = painted_text(&output);

        assert!(text.contains("Warning"), "{text}");
        assert!(!text.contains("Tint"), "{text}");
        assert!(text.contains("Textures"), "{text}");
    }

    #[test]
    fn clicking_a_field_name_selects_it_and_outside_clicks_clear_it() {
        let mut harness = Harness::new(skins());
        let (output, _) = harness.frame(Vec::new());

        let (_, actions) = harness.click(label(panels::CONSOLE, "height"));
        assert!(actions.is_empty(), "selecting is view state: {actions:?}");
        assert_eq!(harness.panel.page_state.form.selected.as_deref(), Some("height"));

        // A press the rows do not consume clears the highlight.
        let heading = painted_texts(&output)
            .into_iter()
            .find(|line| line.text.starts_with("Preferences/Styles/ConsoleStyle.ron"))
            .expect("the page shows its file")
            .rect;
        harness.click_at(heading.center());
        assert_eq!(
            harness.panel.page_state.form.selected, None,
            "an outside click drops the selection"
        );
    }

    // ---------------------------------------------------------------------
    // Value boxes: typing, scrubbing, cursors, fills
    // ---------------------------------------------------------------------

    fn key(key: Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }
    }

    #[test]
    fn clicking_a_number_opens_an_editor_and_enter_commits() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let id = field(panels::CONSOLE, "height");
        harness.click(id);
        harness.frame(Vec::new());

        let (_, actions) = harness.frame(vec![egui::Event::Text("30".to_owned())]);
        assert_eq!(
            harness.page_style::<FixtureStyle>().height,
            30.0,
            "typing writes the value as it goes"
        );
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, Action::Style(StyleAction::Replace { .. }))),
            "the page hands the typed value over: {actions:?}"
        );

        let (_, actions) = harness.frame(vec![key(Key::Enter)]);
        assert!(
            actions.contains(&Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE,
            })),
            "Enter ends the gesture and saves: {actions:?}"
        );
    }

    #[test]
    fn dragging_a_field_name_scrubs_its_value() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let at = harness.rect_of(label(panels::CONSOLE, "height")).center();
        harness.frame(vec![egui::Event::PointerMoved(at)]);
        harness.frame(vec![pointer(at, true)]);
        harness.frame(vec![egui::Event::PointerMoved(at + vec2(10.0, 0.0))]);
        let (_, actions) = harness.frame(vec![pointer(at + vec2(10.0, 0.0), false)]);

        assert_ne!(
            harness.page_style::<FixtureStyle>().height,
            20.0,
            "scrubbing the name changes the value"
        );
        assert!(
            actions
                .iter()
                .any(|action| matches!(action, Action::Style(StyleAction::Commit { .. }))),
            "the release ends the gesture and saves: {actions:?}"
        );
    }

    #[test]
    fn the_name_and_the_value_box_show_their_own_cursors() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let name = harness.rect_of(label(panels::CONSOLE, "height")).center();
        let output = harness.frame(vec![egui::Event::PointerMoved(name)]).0;
        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::ResizeHorizontal,
            "a number's name is a scrub handle"
        );

        let value = harness.rect_of(field(panels::CONSOLE, "height")).center();
        let output = harness.frame(vec![egui::Event::PointerMoved(value)]).0;
        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::Text,
            "the box is a text field"
        );
    }

    #[test]
    fn value_boxes_use_the_unity_fill_and_outline() {
        let mut harness = Harness::new(skins());
        let (output, _) = harness.frame(Vec::new());

        let tokens = crate::shared_style::EditorFieldStyle::dark().tokens();
        let box_rect = harness.rect_of(field(panels::CONSOLE, "height"));
        // The box is two shapes: the fill, then the outline stroke on top.
        let shapes: Vec<PaintedRect> = painted_rects(&output)
            .into_iter()
            .filter(|painted| {
                approx(painted.rect.width(), box_rect.width())
                    && approx(painted.rect.height(), box_rect.height())
            })
            .collect();
        assert!(
            shapes.iter().any(|painted| painted.fill == tokens.fill),
            "the box fill is the field skin: {shapes:?}"
        );
        assert!(
            shapes
                .iter()
                .any(|painted| painted.stroke.color == tokens.edge && painted.stroke.width > 0.0),
            "the box outline is the field skin: {shapes:?}"
        );
    }

    /// The page scrolls, so its rows must stop before the scroll bar: a value
    /// box under the bar would be unclickable at the right edge.
    #[test]
    fn the_value_column_stays_clear_of_the_scroll_bar() {
        let mut harness = Harness::with_size(long_color_skin(), vec2(537.0, 316.0));
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let style = PreferencesPanelStyle::dark();
        let page_right = 537.0 - style.padding;

        let value = harness.rect_of(field(panels::CONSOLE, "background"));
        assert!(
            value.right() <= page_right - style.scrollbar_gap + 0.5,
            "the value column ends before the scroll bar: {value:?}, page right {page_right}"
        );
    }

    /// A narrow page keeps the two columns inside the panel instead of letting
    /// the label column push the value box off the right edge.
    #[test]
    fn a_narrow_page_keeps_two_columns_inside_the_panel() {
        let mut harness = Harness::with_size(skins(), vec2(320.0, 316.0));
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let style = PreferencesPanelStyle::dark();
        let page_right = 320.0 - style.padding;

        let name = harness.rect_of(label(panels::CONSOLE, "icons.warning"));
        let value = harness.rect_of(field(panels::CONSOLE, "icons.warning"));
        assert!(
            name.right() <= value.left(),
            "the name column ends before the value column: {name:?} vs {value:?}"
        );
        assert!(
            value.right() <= page_right,
            "the value box stays inside the panel: {value:?}, page right {page_right}"
        );
    }

    // ---------------------------------------------------------------------
    // Lists: header, count, reorder
    // ---------------------------------------------------------------------

    fn fonts_harness() -> Harness {
        Harness::with_size(Vec::new(), vec2(700.0, 620.0))
    }

    fn open_fonts(harness: &mut Harness) {
        harness.frame(Vec::new());
        harness.click(nav_id(FONTS));
        harness.frame(Vec::new());
    }

    fn font_count(harness: &Harness) -> usize {
        harness
            .page_style::<crate::shared_style::EditorFonts>()
            .proportional
            .len()
    }

    fn font_names(harness: &Harness) -> Vec<String> {
        harness
            .page_style::<crate::shared_style::EditorFonts>()
            .proportional
            .iter()
            .map(|entry| entry.name.clone())
            .collect()
    }

    #[test]
    fn the_list_header_adds_and_removes_entries() {
        let mut harness = fonts_harness();
        open_fonts(&mut harness);
        let key = SkinId::Shared(FONTS);
        let before = font_count(&harness);

        let (_, actions) = harness.click(list_add(key, "proportional"));

        assert_eq!(
            font_count(&harness),
            before + 1,
            "the header adds one entry"
        );
        assert!(
            actions.iter().any(|action| matches!(
                action,
                Action::SharedStyle(SharedStyleAction::Replace { style: FONTS, .. })
            )),
            "the page hands the new list over: {actions:?}"
        );
        assert!(
            actions.iter().any(|action| matches!(
                action,
                Action::SharedStyle(SharedStyleAction::Commit { style: FONTS })
            )),
            "the click also saves: {actions:?}"
        );

        harness.frame(Vec::new());
        // The remove cell acts on the selected element.
        harness.click(label(key, "proportional.1"));
        harness.frame(Vec::new());
        let (_, _) = harness.click(list_remove(key, "proportional"));
        assert_eq!(
            font_count(&harness),
            before,
            "the header removes the selected entry"
        );
    }

    /// The header's count box is a number: dragging it writes the new length.
    #[test]
    fn dragging_the_entry_count_writes_the_new_length() {
        let mut harness = fonts_harness();
        open_fonts(&mut harness);
        let key = SkinId::Shared(FONTS);
        let count = harness.rect_of(field(key, "proportional")).center();

        harness.frame(vec![egui::Event::PointerMoved(count)]);
        harness.frame(vec![pointer(count, true)]);
        let (_, actions) = harness.frame(vec![egui::Event::PointerMoved(count + vec2(30.0, 0.0))]);

        assert_ne!(
            font_count(&harness),
            2,
            "the count drag resized the list: {}",
            font_count(&harness)
        );
        assert!(
            actions.iter().any(|action| matches!(
                action,
                Action::SharedStyle(SharedStyleAction::Replace { .. })
            )),
            "the resized list goes back to the registry: {actions:?}"
        );
        harness.frame(vec![pointer(count + vec2(30.0, 0.0), false)]);
    }

    /// The drag handle is the element's selection cell; clicking it selects the
    /// element band, exactly like clicking the band itself.
    #[test]
    fn clicking_a_drag_handle_selects_the_element() {
        let mut harness = fonts_harness();
        open_fonts(&mut harness);
        let key = SkinId::Shared(FONTS);

        harness.click(list_handle(key, "proportional.0"));

        assert_eq!(
            harness.panel.page_state.form.selected.as_deref(),
            Some("proportional.0")
        );
    }

    /// Dragging an element's handle lifts a ghost and moves the entry when the
    /// pointer is released over the neighbour.
    #[test]
    fn dragging_an_element_handle_reorders_on_release() {
        let mut harness = fonts_harness();
        open_fonts(&mut harness);
        let key = SkinId::Shared(FONTS);
        let handle = harness.rect_of(list_handle(key, "proportional.0")).center();
        let target = harness.rect_of(label(key, "proportional.1")).center();

        harness.frame(vec![egui::Event::PointerMoved(handle)]);
        harness.frame(vec![pointer(handle, true)]);
        // Move past the drag threshold first, then over the neighbour.
        harness.frame(vec![egui::Event::PointerMoved(handle + vec2(0.0, 6.0))]);
        harness.frame(vec![egui::Event::PointerMoved(target + vec2(0.0, 6.0))]);
        let (_, _) = harness.frame(vec![pointer(target + vec2(0.0, 6.0), false)]);

        assert_eq!(
            font_names(&harness)[0],
            "Inter Medium",
            "the dragged entry moved down one slot: {:?}",
            font_names(&harness)
        );
    }

    /// While an element is held, its whole block (band and rows) follows the
    /// pointer on a floating layer; the list keeps its normal layout, so the
    /// neighbour slides into the preview slot and is never painted over.
    #[test]
    fn dragging_an_element_lifts_its_whole_block() {
        let mut harness = fonts_harness();
        open_fonts(&mut harness);
        let key = SkinId::Shared(FONTS);
        let handle = harness.rect_of(list_handle(key, "proportional.0")).center();
        let first = harness.rect_of(label(key, "proportional.0"));
        let second = harness.rect_of(label(key, "proportional.1"));

        // Resting positions of the block band and one of its field rows.
        let (before, _) = harness.frame(Vec::new());
        let before_runs = painted_texts(&before);
        let band_before = run(&before_runs, "Element 0").rect;
        let name_before = run(&before_runs, "Inter").rect;

        harness.frame(vec![egui::Event::PointerMoved(handle)]);
        harness.frame(vec![pointer(handle, true)]);
        harness.frame(vec![egui::Event::PointerMoved(handle + vec2(0.0, 6.0))]);
        let held = second.center() + vec2(0.0, 6.0);
        let (mut output, _) = harness.frame(vec![egui::Event::PointerMoved(held)]);
        // The preview slide eases for 0.16 s; let it settle while still held.
        for _ in 0..12 {
            let (pass, _) = harness.frame(vec![egui::Event::PointerMoved(held)]);
            output = pass;
        }
        let runs = painted_texts(&output);

        // The held block is not painted in place any more: its band appears
        // once, under the pointer.
        let bands: Vec<&PaintedText> = runs.iter().filter(|run| run.text == "Element 0").collect();
        assert_eq!(
            bands.len(),
            1,
            "the floating block replaced the in-place band: {runs:?}"
        );
        let band_after = bands[0].rect;
        assert!(
            approx(band_after.center().y, held.y),
            "the ghost band follows the pointer: {:?}",
            band_after
        );

        // The body lifts with it: the field row moved by the same offset as the
        // band, so the ghost carries the whole block, not just its header.
        let name_after = run(&runs, "Inter").rect;
        assert!(
            approx(
                band_after.min.y - band_before.min.y,
                name_after.min.y - name_before.min.y
            ),
            "the rows lift with the band: band {:?} -> {:?}, name {:?} -> {:?}",
            band_before,
            band_after,
            name_before,
            name_after
        );
        assert!(
            band_after.min.y > band_before.min.y + 1.0,
            "the block moved down to the pointer: {:?} -> {:?}",
            band_before,
            band_after
        );

        // The old slot is not painted over: the neighbour slid up into it and
        // stays visible (the previous approach erased it with the page colour).
        let background = PreferencesPanelStyle::dark().background.to_egui();
        assert!(
            !painted_rects(&output).iter().any(|painted| {
                painted.fill == background
                    && approx(painted.rect.top(), first.top())
                    && painted.rect.height() > first.height()
            }),
            "no background patch hides the neighbour that moved into the old slot"
        );

        // The neighbour slides up into the slot the ghost would drop into.
        let moved = harness.rect_of(label(key, "proportional.1"));
        assert!(
            moved.top() < second.top() - 1.0,
            "the neighbour slides to its preview slot: was {second:?}, now {moved:?}"
        );
    }

    /// The font list's header and element bands, and the fold rules: the list
    /// band folds every entry, an element's caret folds just its fields.
    #[test]
    fn the_font_list_has_one_header_and_folds_each_element() {
        let mut harness = fonts_harness();
        open_fonts(&mut harness);
        let key = SkinId::Shared(FONTS);
        let (output, _) = harness.frame(Vec::new());

        let runs = painted_texts(&output);
        let text = painted_text(&output);
        assert!(
            !text.contains('🔥'),
            "duplicate ids make egui paint its clash warning over the page: {text}"
        );
        assert_eq!(
            runs.iter().filter(|run| run.text == "Proportional").count(),
            1,
            "the list header is painted once: {text}"
        );

        let header = harness.rect_of(group(key, "proportional"));
        let count = harness.rect_of(field(key, "proportional"));
        assert!(
            approx(header.center().y, count.center().y),
            "the count box shares the header row: header {header:?}, count {count:?}"
        );
        assert!(
            count.left() > header.left(),
            "the count box is at the far right: {count:?}"
        );
        let handle = harness.rect_of(list_handle(key, "proportional.0"));
        let band = harness.rect_of(label(key, "proportional.0"));
        assert!(
            handle.right() <= band.left(),
            "the drag handle is the leftmost cell: handle {handle:?}, band {band:?}"
        );
        let add = harness.rect_of(list_add(key, "proportional"));
        let last = harness.rect_of(label(key, "proportional.1"));
        assert!(
            add.top() >= last.bottom(),
            "the add/remove pair sits under the rows: add {add:?}, last band {last:?}"
        );

        // The caret folds the element's fields, not the list.
        let (output, actions) = harness.click(group(key, "proportional.0"));
        let text = painted_text(&output);
        assert!(
            actions.is_empty(),
            "folding is view state, not an action: {actions:?}"
        );
        assert!(
            harness.panel.page_state.form.collapsed.contains("proportional.0"),
            "the element folds under its own dotted path"
        );
        assert!(
            !text.contains("Preferences/Fonts/Inter-Regular.otf"),
            "entry 0's fields fold away: {text}"
        );
        assert!(
            text.contains("Preferences/Fonts/Inter-Medium.otf"),
            "entry 1 keeps its fields: {text}"
        );
        assert!(text.contains("Element 0"), "the band stays: {text}");
    }

    // ---------------------------------------------------------------------
    // Enum tokens: a menu row and its payload
    // ---------------------------------------------------------------------

    fn enum_skin(mode: Mode) -> Vec<PageSkin> {
        vec![page_skin(
            SkinId::Panel(panels::CONSOLE),
            "Console",
            "ConsoleStyle.ron",
            enum_style(mode),
        )]
    }

    /// A payload hangs under the menu button, not at the row's left edge: the
    /// button is the reference, so the payload labels start at its left and the
    /// enum row keeps its own label column. The payload stays on the property
    /// stripes: it has no tint or border of its own.
    #[test]
    fn an_enum_block_hangs_its_payload_under_the_menu_button() {
        let mut harness = Harness::with_size(
            enum_skin(Mode::Soft {
                radius: 4.0,
                samples: 2,
            }),
            vec2(700.0, 470.0),
        );
        let (output, _) = harness.frame(Vec::new());
        let runs = painted_texts(&output);
        let mode = run(&runs, "Mode");
        let radius = run(&runs, "Radius");
        let samples = run(&runs, "Samples");
        let button_left = harness.rect_of(field(panels::CONSOLE, "mode")).left();

        assert!(
            approx(radius.rect.left(), button_left),
            "the payload starts at the menu button: Radius {:?} vs {button_left}",
            radius.rect
        );
        assert!(
            approx(samples.rect.left(), button_left),
            "every payload label starts at the button: Samples {:?}",
            samples.rect
        );
        assert!(
            radius.rect.left() > mode.rect.left() + 1.0,
            "the payload is not back at the row's left edge"
        );

        let radius_value = harness
            .rect_of(field(panels::CONSOLE, "mode.radius"))
            .left();
        let samples_value = harness
            .rect_of(field(panels::CONSOLE, "mode.samples"))
            .left();
        assert!(
            radius_value > button_left,
            "the payload has its own value column under the button: {radius_value}"
        );
        assert!(approx(radius_value, samples_value));

        let form = EditorFormStyle::dark().tokens();
        let striped = painted_rects(&output)
            .into_iter()
            .filter(|painted| {
                (painted.fill == form.row_odd || painted.fill == form.row_even)
                    && approx(painted.rect.height(), form.row_height)
            })
            .count();
        assert_eq!(
            striped, 3,
            "the menu row and its two payload rows stay on the property stripes"
        );
    }

    /// Choosing the unit variant writes the enum in the page's copy; the owner
    /// gets the whole value, and the payload rows disappear because the copy
    /// now holds `Off`.
    #[test]
    fn switching_an_enum_variant_swaps_the_payload_rows() {
        let mut harness = Harness::with_size(
            enum_skin(Mode::Soft {
                radius: 4.0,
                samples: 2,
            }),
            vec2(700.0, 470.0),
        );
        harness.frame(Vec::new());

        let field = field(panels::CONSOLE, "mode");
        harness.click(field);
        // The popup lays its items out on the pass after it opens.
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let item = harness.rect_of(enum_item_id(field, 0));
        let (_, actions) = harness.click_at(item.center());

        assert!(
            actions.iter().any(|action| matches!(
                action,
                Action::Style(StyleAction::Replace {
                    panel: panels::CONSOLE,
                    skin,
                }) if *skin == Skin::new(enum_style(Mode::Off)),
            )),
            "choosing the unit variant writes it: {actions:?}"
        );
        assert_eq!(harness.page_style::<EnumStyle>().mode, Mode::Off);

        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);
        assert!(text.contains("Mode"), "{text}");
        assert!(!text.contains("Radius"), "{text}");
        assert!(!text.contains("Samples"), "{text}");
    }

    /// The font list's priority is an enum token: the menu writes the install
    /// order into the page's copy and the registry gets the whole list back.
    #[test]
    fn the_font_priority_menu_writes_the_install_order() {
        let mut harness = fonts_harness();
        open_fonts(&mut harness);
        let key = SkinId::Shared(FONTS);
        let priority = field(key, "proportional.1.priority");
        assert_eq!(
            harness
                .page_style::<crate::shared_style::EditorFonts>()
                .proportional[1]
                .priority,
            crate::shared_style::EditorFonts::dark().proportional[1].priority,
            "the fixture starts as Push"
        );

        harness.frame(Vec::new());
        harness.click(priority);
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let item = harness.rect_of(enum_item_id(priority, 0));
        let (_, actions) = harness.click_at(item.center());

        assert_eq!(
            harness
                .page_style::<crate::shared_style::EditorFonts>()
                .proportional[1]
                .priority,
            crate::shared_style::EditorFontPriority::First,
            "choosing First writes the token"
        );
        assert!(
            actions.iter().any(|action| matches!(
                action,
                Action::SharedStyle(SharedStyleAction::Replace { style: FONTS, .. })
            )),
            "the whole font list goes back to the registry: {actions:?}"
        );
    }

    // The list chrome's own state is retained across passes: two lists on one
    // page keep separate count edits and drag gestures.
    #[test]
    fn two_lists_on_one_page_keep_separate_chrome_state() {
        assert_ne!(
            list_chrome(SkinId::Shared(FONTS), "proportional"),
            list_chrome(SkinId::Shared(FONTS), "monospace")
        );
    }
}
