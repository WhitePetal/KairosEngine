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
//! # Where the fields come from
//!
//! Each panel owns its style (loaded in Panel::init) and derives StyleFields,
//! so it can describe its own tokens (crate::skin). The app snapshots the live
//! panels' field tables into Frame::skins while this panel is open; this panel
//! draws them and never touches another panel directly.
//!
//! Editor-shell styles (the toolbar and the color picker) have no panel
//! instance. They live in the app's crate::shared_style registry and are
//! appended as fixed pages after the panel pages; their writes travel through
//! the separate SharedStyleAction path, including a Reset to code defaults.
//!
//! # The field tree
//!
//! Dotted token paths are a tree (icons.console.info), and the page draws it
//! the way Unity draws an inspector: each nested RON struct is a full-width
//! foldout band with a triangle, leaf rows alternate between two greys, field
//! names are left-aligned, and a selected field gets a blue label and a blue
//! value outline.
//!
//! # Live apply and save
//!
//! A control pushes StyleAction::SetField on every change. The app writes it
//! into the owning panel's live style, so the next pass draws with the new
//! value — that is what makes the edit immediate. The file is written once, on
//! StyleAction::Commit: pointer release, the end of a typed edit, page switch,
//! or closing this panel. A Reset is the other write: it replaces the whole
//! style with the theme's code defaults and saves it in the same click. The
//! legacy window instead pushed the whole page back every frame and let the
//! owning window write its TOML, about 60 times a second.

pub mod style;

use kairos_editor_widgets::color::ColorExt;
use std::collections::HashSet;

use egui::{
    Align, Context, CornerRadius, CursorIcon, Id, Key, Margin, Pos2, Rect, Sense, Shape, Stroke,
    Ui, UiBuilder, WidgetText, WidgetType, pos2, vec2,
};
use kairos_color::{Color, ColorUsage};
use kairos_editor_core::{
    Action, ListEdit, PanelId, SharedStyleAction, SharedStyleId, StyleAction, StyleValue,
};
use kairos_editor_widgets::button::Button;
use kairos_editor_widgets::color_picker::{
    ColorField, ColorPickerStyle, ColorProperty, any_color_picker_open, close_all_color_pickers,
    eyedropper, is_color_picker_open,
};
use kairos_editor_widgets::divider::Divider;
use kairos_editor_widgets::field::{
    FieldLabel, FieldLabelResponse, FieldOutcome, FieldStyle, ListBandIds, ListChrome,
    ListChromeEdit, ListChromeIds, ListChromeState, ListChromeStyle, Number, NumberEdit,
    NumberField, TextField, ToggleField, VariantMenu, enum_popup_id,
};
use kairos_editor_widgets::paint::labelled_widget_info;
use kairos_editor_widgets::style::StyleFile;
use kairos_editor_widgets::text::TextStyle;

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;
use crate::shared_style::{ListStyle, SharedSkin};
use crate::skin::{LIST_MAX, StyleField, humanize};

use self::style::PreferencesPanelStyle;

/// One page in the Preferences editor: a live panel's skin or a shared style.
///
/// Panel skins stay keyed by PanelId; shared styles have their own stable id
/// and their own action path. A local enum keeps the two lists in one nav
/// without teaching core about a second kind of skin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum PageKey {
    Panel(PanelId),
    Shared(SharedStyleId),
}

impl From<PanelId> for PageKey {
    fn from(panel: PanelId) -> Self {
        Self::Panel(panel)
    }
}

impl From<SharedStyleId> for PageKey {
    fn from(style: SharedStyleId) -> Self {
        Self::Shared(style)
    }
}

/// One page's borrowed metadata for this pass.
struct PageInfo<'a> {
    key: PageKey,
    title: &'a str,
    file: &'static str,
    fields: &'a [StyleField],
    /// Shared styles bump this when their file is re-read. A panel page is
    /// always zero: its snapshot is the live style it just edited.
    revision: u64,
}

fn nav_id(page: impl Into<PageKey>) -> Id {
    Id::new(("kairos.preferences.nav", page.into()))
}

/// Stable id of a foldout band, keyed by its dotted group path.
fn group_id(page: impl Into<PageKey>, path: &str) -> Id {
    Id::new(("kairos.preferences.group", page.into(), path))
}

/// Stable id of a field's name, so tests (and egui's click routing) can find
/// the left-hand label without a layout index.
fn label_id(page: impl Into<PageKey>, path: &str) -> Id {
    Id::new(("kairos.preferences.label", page.into(), path))
}

/// Stable id of a field's value box. The box, its drag/click interaction and
/// its inline text editor all share this id: only one of them is live at a
/// time, and keyboard focus must follow the field from one mode to the other.
fn field_id(page: impl Into<PageKey>, path: &str) -> Id {
    Id::new(("kairos.preferences.field", page.into(), path))
}

/// Stable id of a boolean checkbox; see field_id.
fn toggle_id(page: impl Into<PageKey>, path: &str) -> Id {
    Id::new(("kairos.preferences.toggle", page.into(), path))
}

/// Stable id of a list header's add / remove cells.
fn list_add_id(page: impl Into<PageKey>, path: &str) -> Id {
    Id::new(("kairos.preferences.list.add", page.into(), path))
}

fn list_remove_id(page: impl Into<PageKey>, path: &str) -> Id {
    Id::new(("kairos.preferences.list.remove", page.into(), path))
}

/// Stable id of a list element's drag handle. The element band itself uses
/// [`group_id`] (its caret) and [`label_id`] (its selection), like a foldout
/// and a row share a path.
fn list_handle_id(page: impl Into<PageKey>, path: &str) -> Id {
    Id::new(("kairos.preferences.list.handle", page.into(), path))
}

/// Stable key of one list's chrome state. The style page addresses lists by
/// dotted token path; the widget only needs an id it can carry across passes.
fn list_chrome_id(page: impl Into<PageKey>, path: &str) -> Id {
    Id::new(("kairos.preferences.list.chrome", page.into(), path))
}

/// The element index a selected field path belongs to, if any.
fn selected_element_index(selected: Option<&str>, list: &str, len: usize) -> Option<usize> {
    let rest = selected?
        .strip_prefix(list)?
        .strip_prefix('.')?
        .split('.')
        .next()?;
    let index = rest.parse::<usize>().ok()?;
    (index < len).then_some(index)
}

/// The action that writes one field on whichever page owns it.
fn set_field_action(page: PageKey, path: &str, value: &StyleValue) -> Action {
    match page {
        PageKey::Panel(panel) => Action::Style(StyleAction::SetField {
            panel,
            path: path.to_owned(),
            value: value.clone(),
        }),
        PageKey::Shared(style) => Action::SharedStyle(SharedStyleAction::SetField {
            style,
            path: path.to_owned(),
            value: value.clone(),
        }),
    }
}

fn reset_id(page: impl Into<PageKey>) -> Id {
    Id::new(("kairos.preferences.reset", page.into()))
}

/// A number box being typed into: the field it belongs to, and the widget
/// buffer that survives between passes.
struct FieldEdit {
    path: String,
    number: NumberEdit,
}

/// The page being edited: a snapshot of one page's field table, plus the
/// bookkeeping that decides when its file is written.
#[derive(Default)]
struct PageState {
    /// The page the fields belong to.
    key: Option<PageKey>,
    /// Shared-style revision this snapshot came from. A reload or a reset
    /// bumps the registry revision, and the next pass re-reads the field table.
    revision: u64,
    /// Edited in place: a drag follows the pointer even though the owning
    /// panel's next snapshot is a frame behind.
    fields: Vec<StyleField>,
    title: String,
    file: &'static str,
    /// A SetField was pushed since the last save.
    dirty: bool,
    /// The field whose name or value was clicked last. Its name is blue and
    /// its value box is outlined with the accent.
    selected: Option<String>,
    /// Foldout bands folded shut, by dotted group path.
    collapsed: HashSet<String>,
    /// A numeric token being edited as text.
    edit: Option<FieldEdit>,
    /// Last page scroll offset. While a floating color picker is open we keep
    /// this offset fixed so a TextEdit focus inside the picker cannot scroll
    /// the page behind it.
    scroll_offset: egui::Vec2,
    /// A structural list edit or a factory reset landed. The page's snapshot
    /// is otherwise re-read only on a page change or a reload, so a new entry's
    /// rows (or the reset values) would not exist until the user left the page:
    /// the next pass re-reads it.
    stale: bool,
    /// Retained list chrome: the count editor and the reorder gesture.
    list_chrome: ListChromeState,
}

impl PageState {
    /// Show the selected page's field table, committing a pending edit on the
    /// page being left. A shared page is re-read when its file was reloaded
    /// (the registry revision changed); a panel page never is, because its
    /// snapshot is the live value this editor just wrote.
    fn select(&mut self, pages: &[PageInfo<'_>], selected: Option<PageKey>, out: &mut UiOut) {
        let page = selected.and_then(|key| pages.iter().find(|page| page.key == key));

        if self.key == selected {
            let Some(page) = page else {
                return;
            };
            if self.stale {
                // A list edit landed under this page: take the owner's new
                // table (new rows, new count) and keep the foldout state, the
                // selection and an in-flight count edit. The edit is keyed by
                // path, and the count is the only field that can resize the
                // list while being typed into.
                self.fields = page.fields.to_vec();
                self.title = page.title.to_owned();
                self.file = page.file;
                self.revision = page.revision;
                self.stale = false;
                return;
            }
            if self.revision == page.revision {
                return;
            }
            // The file was reloaded under this page: take the new values but
            // keep the selection and foldout state, and drop a text edit whose
            // original value no longer belongs to the new snapshot.
            self.fields = page.fields.to_vec();
            self.title = page.title.to_owned();
            self.file = page.file;
            self.revision = page.revision;
            self.edit = None;
            self.list_chrome.clear();
            // The reloaded file is now the truth; the discarded pre-reload
            // edit must not be written by a later release pass.
            self.dirty = false;
            return;
        }

        self.commit(out);

        match page {
            Some(page) => {
                self.fields = page.fields.to_vec();
                self.title = page.title.to_owned();
                self.file = page.file;
                self.revision = page.revision;
            }
            None => {
                self.fields.clear();
                self.title.clear();
                self.file = "";
                self.revision = 0;
            }
        }
        self.key = selected;
        // A new page starts unfolded and unselected. Carrying either over
        // would highlight a path that only happens to have the same name.
        self.selected = None;
        self.collapsed.clear();
        self.edit = None;
        self.list_chrome.clear();
        self.stale = false;
    }

    /// The owner replaced the whole style. Drop an in-flight edit (its value
    /// no longer belongs to the new table) and re-read the owner's field table
    /// on the next pass.
    fn reset(&mut self) {
        self.edit = None;
        self.dirty = false;
        self.list_chrome.clear();
        self.stale = true;
    }

    /// Ask the app to persist the page being left, if it has an edit that was
    /// never released (typed text, arrow keys).
    fn commit(&mut self, out: &mut UiOut) {
        if !self.dirty {
            return;
        }
        match self.key {
            Some(PageKey::Panel(panel)) => {
                out.push(Action::Style(StyleAction::Commit { panel }));
            }
            Some(PageKey::Shared(style)) => {
                out.push(Action::SharedStyle(SharedStyleAction::Commit { style }));
            }
            None => {}
        }
        self.dirty = false;
    }
}

/// The Preferences panel.
pub struct PreferencesPanel {
    style: PreferencesPanelStyle,
    /// Page list selection; falls back to the first available page.
    selected: Option<PageKey>,
    page: PageState,
    /// A color popup was open when the panel closed; close it on the next
    /// draw so reopening Preferences does not resurrect it.
    close_popups: bool,
}

impl PreferencesPanel {
    /// Build the panel. The catalog factory above is the only caller.
    pub fn new() -> Self {
        Self {
            style: PreferencesPanelStyle::default(),
            selected: None,
            page: PageState::default(),
            close_popups: false,
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
            page,
            close_popups,
        } = self;

        // The dock builds a body Ui from the context style, so the app's
        // "chrome is not selectable" switch does not reach this panel.
        ui.style_mut().interaction.selectable_labels = false;
        if *close_popups {
            egui::Popup::close_all(ui.ctx());
            close_all_color_pickers(ui.ctx());
            *close_popups = false;
        }

        // The dock paints a body colour behind every tab; a floating window uses
        // the panel's own. Painting it here keeps both cases identical.
        ui.painter().rect_filled(
            ui.max_rect(),
            CornerRadius::ZERO,
            style.background.to_egui(),
        );

        // Panel pages first (the existing editor order), then the shared
        // styles, which are always present even when no panel is open.
        let shared_skins: Vec<SharedSkin> = frame.shared.skins();
        let mut pages: Vec<PageInfo<'_>> = frame
            .skins
            .iter()
            .map(|skin| PageInfo {
                key: PageKey::Panel(skin.panel),
                title: &skin.title,
                file: skin.file,
                fields: &skin.fields,
                revision: 0,
            })
            .collect();
        pages.extend(shared_skins.iter().map(|skin| PageInfo {
            key: PageKey::Shared(skin.id),
            title: &skin.title,
            file: skin.file,
            fields: &skin.fields,
            revision: frame.shared.revision(),
        }));

        let picker_style = frame.color_picker_style();
        let field_style = frame.field_style();
        let list_style = frame.list_style();
        let mut current = match *selected {
            Some(page) if pages.iter().any(|entry| entry.key == page) => Some(page),
            _ => pages.first().map(|page| page.key),
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
                    current = page_list(ui, style, &pages, current);
                });

                let previous_page = page.key;
                page.select(&pages, current, out);
                if previous_page.is_some() && page.key != previous_page {
                    // A color popup belongs to the page that opened it; leaving
                    // the page must not leave it floating over the new one.
                    egui::Popup::close_all(ui.ctx());
                    close_all_color_pickers(ui.ctx());
                }

                ui.scope_builder(UiBuilder::new().max_rect(page_rect), |ui| {
                    page_body(
                        ui,
                        style,
                        &field_style,
                        list_style,
                        &picker_style,
                        page,
                        out,
                    );
                });

                footer(ui, style, footer_rect, page, out);
            });

        // A click ends the interaction: one write per gesture, however many
        // frames the drag took. Keyboard edits have no release; they persist
        // when the page changes or the panel closes, and Enter/Escape ends a
        // color hex or numeric edit immediately.
        let released = ui.input(|input| input.pointer.any_released());
        let keyboard_end =
            ui.input(|input| input.key_pressed(Key::Enter) || input.key_pressed(Key::Escape));
        if page.dirty && (released || keyboard_end) {
            page.commit(out);
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
        self.page.commit(out);
        self.close_popups = true;
        true
    }
}

/// The page list: one entry per panel with an editable skin, then the shared
/// styles (toolbar, color picker), which are always present.
fn page_list(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    pages: &[PageInfo<'_>],
    selected: Option<PageKey>,
) -> Option<PageKey> {
    let mut chosen = selected;
    ui.spacing_mut().item_spacing = vec2(0.0, 1.0);
    let width = ui.available_width();
    let nav = style.nav_style();

    for page in pages {
        let is_current = Some(page.key) == selected;
        let response = ui.add(
            Button::new(page.title, &nav)
                .width(width)
                .selected(is_current)
                .id(nav_id(page.key)),
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
            chosen = Some(page.key);
        }
    }

    chosen
}

/// One page: heading, the file being written, rule, grouped rows.
#[allow(clippy::too_many_arguments)] // four style views plus the page and its buffers
fn page_body(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    field_style: &FieldStyle,
    list_style: &ListStyle,
    picker_style: &ColorPickerStyle,
    page: &mut PageState,
    out: &mut UiOut,
) {
    let picker_before = any_color_picker_open(ui.ctx());
    let page_key = page.key;
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt("kairos-preferences-page")
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
                .label(ui, page.title.clone());
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
                field: field_style,
                chrome: list_style.tokens(field_style),
                key: page_key.expect("a page body is only drawn for a selected page"),
                picker: picker_style,
                page,
                out,
                clicked: false,
                active_shift: None,
                ghost: None,
            };
            writer.rows(ui);
        });
    let picker_after = any_color_picker_open(ui.ctx());
    if picker_before || picker_after {
        // Keep the page under the floating picker still. A TextEdit focus in
        // the picker emits scroll_to_rect, and this ScrollArea would otherwise
        // consume it and slide the Preferences page while the user types.
        scroll.state.offset = page.scroll_offset;
        scroll.state.store(ui.ctx(), scroll.id);
    } else {
        page.scroll_offset = scroll.state.offset;
    }
}

/// Draws one page's field tree.
struct PageWriter<'a> {
    style: &'a PreferencesPanelStyle,
    /// The value-box tokens, shared by every field on any panel.
    field: &'a FieldStyle,
    /// The list chrome's tokens, built once from the page and field styles.
    chrome: ListChromeStyle,
    key: PageKey,
    picker: &'a ColorPickerStyle,
    page: &'a mut PageState,
    out: &'a mut UiOut,
    /// A row, a field name or a foldout consumed this pass's click. Without
    /// one, a click elsewhere on the page ends the current selection.
    clicked: bool,
    /// The element block whose rows are currently offset by a reorder slide,
    /// and the offset that must be removed when the block ends.
    active_shift: Option<(String, f32)>,
    /// The element block being dragged. Its band and rows are drawn on a
    /// floating layer, translated to the pointer, while the normal layout
    /// keeps their height for the gap.
    ghost: Option<GhostBlock>,
}

/// The floating copy of the dragged element block.
#[derive(Clone)]
struct GhostBlock {
    layer: egui::LayerId,
    offset: f32,
    /// Dotted path of the element this block belongs to.
    element: String,
}

impl PageWriter<'_> {
    fn rows(&mut self, ui: &mut Ui) {
        let style = self.style;
        // The scroll bar floats over the content in egui; reserving its lane
        // keeps the rightmost value box (an icon path, typically) clear of it.
        let row_width = (ui.available_width() - style.scrollbar_gap).max(1.0);
        // The groups opened so far, one segment name per level. Comparing
        // segments (not joined paths) is what keeps a nested run open across
        // all of its fields: a stack holding `"proportional.0"` never matches
        // the `"0"` segment, so the entry would be re-opened — and its band's
        // widget id re-created — for every field inside it.
        let mut open: Vec<String> = Vec::new();
        let mut row_number = 0usize;
        // The element whose field rows are being drawn, and how many of them
        // have been drawn: the stripe and the block's bottom edge are per
        // element, not per page.
        let mut current_element: Option<String> = None;
        let mut element_row = 0usize;
        // The list bands are collected by the chrome widget; it resolves a
        // reorder only after every list on the page has laid out its rows.
        ListChrome::begin(&mut self.page.list_chrome);
        // Lists whose element rows have started but whose block has not closed
        // yet. A list ends when the next field leaves its dotted path.
        let mut list_stack: Vec<String> = Vec::new();

        // A `Vec<T>` token is a list, empty or not: the list node is the only
        // field an empty list has, and it is what gives the empty list a
        // header to add the first entry from.
        let lists: Vec<(String, usize)> = self
            .page
            .fields
            .iter()
            .filter(|field| field.value.is_list())
            .map(|field| {
                let len = match field.value {
                    StyleValue::ListLen(len) => len,
                    _ => 0,
                };
                (field.path.clone(), len)
            })
            .collect();
        let list_paths: Vec<String> = lists.iter().map(|(path, _)| path.clone()).collect();

        // Enum tokens, and the ones whose active variant has payload rows.
        // Payload rows sit at the enum's own depth: the block tint, not an
        // indent, is what ties them to the menu button.
        let enums: Vec<String> = self
            .page
            .fields
            .iter()
            .filter(|field| field.value.is_variant())
            .map(|field| field.path.clone())
            .collect();
        let enum_blocks: Vec<String> = enums
            .iter()
            .filter(|path| {
                self.page
                    .fields
                    .iter()
                    .any(|field| field.path.starts_with(&format!("{path}.")))
            })
            .cloned()
            .collect();

        for index in 0..self.page.fields.len() {
            let (path, is_list) = {
                let field = &self.page.fields[index];
                (field.path.clone(), field.value.is_list())
            };

            // Restore the layout offset of the element block we just left.
            if let Some((element, shift)) = self.active_shift.take() {
                if path == element || path.starts_with(&format!("{element}.")) {
                    self.active_shift = Some((element, shift));
                } else {
                    ui.add_space(-shift);
                }
            }

            // Close every list block this field is no longer inside: its
            // footer (add/remove under the container) belongs between the last
            // element row and this field.
            while let Some(list) = list_stack.last() {
                if path == *list || path.starts_with(&format!("{list}.")) {
                    break;
                }
                let list = list_stack.pop().expect("checked just above");
                self.list_footer(ui, &list, row_width);
            }

            // A list node draws its own header (name and inline count) instead
            // of a row in the value column.
            if is_list {
                let segments: Vec<&str> = path.split('.').filter(|part| !part.is_empty()).collect();
                let parents = &segments[..segments.len().saturating_sub(1)];
                let depth = self.open_groups(
                    ui,
                    parents,
                    &mut open,
                    &enums,
                    row_width,
                    &mut row_number,
                    &lists,
                );

                // A folded group hides the list's entries, not its header.
                let hidden = collapsed_above(parents, &self.page.collapsed);
                if let Some(last) = segments.last() {
                    open.push((*last).to_owned());
                }
                if !hidden {
                    self.list_header(ui, index, depth, row_width);
                    if style.row_gap > 0.0 {
                        ui.add_space(style.row_gap);
                    }
                    // Re-read the collapse state: clicking the header above
                    // may have just toggled it. A folded list has no container
                    // and no footer.
                    if !self.page.collapsed.contains(&path) {
                        ListChrome::open(
                            &mut self.page.list_chrome,
                            list_chrome_id(self.key, &path),
                            ui.cursor().top(),
                        );
                        list_stack.push(path.clone());
                    }
                }
                continue;
            }

            let group = group_path(&path);
            let segments: Vec<&str> = group.split('.').filter(|part| !part.is_empty()).collect();
            let depth = self.open_groups(
                ui,
                &segments,
                &mut open,
                &enums,
                row_width,
                &mut row_number,
                &lists,
            );

            // Which element block this row belongs to, and where inside it.
            // The path, not the visual depth, distinguishes two entries of the
            // same list.
            let element = element_path(&group, &list_paths);
            if element.is_some() && element != current_element {
                current_element = element.clone();
                element_row = 0;
            } else if element.is_none() {
                current_element = None;
                element_row = 0;
            }
            let next_element = self
                .page
                .fields
                .get(index + 1)
                .and_then(|field| element_path(&group_path(&field.path), &list_paths));
            let last_in_element = element.is_some() && element != next_element;

            // A folded group hides its rows, not its own band.
            if !collapsed_above(&segments, &self.page.collapsed) {
                // A row inside a list element keeps that element's gutter and
                // stripes on the element's band background; an enum payload
                // row keeps the menu button's label column instead of
                // indenting, and has no tint or border of its own.
                let slot = RowSlot {
                    depth,
                    element: element
                        .as_ref()
                        .map(|element| visual_depth(element, &enums)),
                    element_row,
                    last_in_element,
                };
                let block = enum_block(&path, &enum_blocks, &enums);
                self.row(ui, index, slot, block, row_width, row_number);
                row_number += 1;
                if element.is_some() {
                    element_row += 1;
                }
                if style.row_gap > 0.0 {
                    ui.add_space(style.row_gap);
                }
            }
        }

        if let Some((_, shift)) = self.active_shift.take() {
            ui.add_space(-shift);
        }
        while let Some(list) = list_stack.pop() {
            self.list_footer(ui, &list, row_width);
        }

        self.finish_list_drag(ui);

        // A click in the empty page (or on its heading) ends the selection;
        // rows, names and foldouts record their own clicks.
        if ui.input(|input| input.pointer.any_click()) && !self.clicked {
            self.page.selected = None;
            self.page.edit = None;
        }
    }

    /// Close the groups this row is no longer in, open the ones it is, and
    /// return the row's visual depth.
    ///
    /// Fields are in declaration order, so the group runs stay contiguous. An
    /// enum path opens no band — its own menu row already names it — and adds
    /// no depth: its payload rows hang under the menu button. A nested struct
    /// inside the payload still opens its band and adds a level.
    #[allow(clippy::too_many_arguments)]
    fn open_groups(
        &mut self,
        ui: &mut Ui,
        segments: &[&str],
        open: &mut Vec<String>,
        enums: &[String],
        row_width: f32,
        row_number: &mut usize,
        lists: &[(String, usize)],
    ) -> usize {
        let common = open
            .iter()
            .zip(segments.iter())
            .take_while(|(opened, segment)| opened.as_str() == **segment)
            .count();
        open.truncate(common);

        let mut visual = 0usize;
        let mut prefix = String::new();
        for (level, segment) in segments.iter().enumerate() {
            if !prefix.is_empty() {
                prefix.push('.');
            }
            prefix.push_str(segment);
            let is_enum = enums.iter().any(|path| path == &prefix);
            if level >= common {
                // Open a group only when nothing above it is folded; the stack
                // is this band's ancestors, so ask before pushing it.
                let hidden = collapsed_above(&segments[..level], &self.page.collapsed);
                open.push((*segment).to_owned());
                if !hidden && !is_enum {
                    self.band(
                        ui,
                        &prefix,
                        visual,
                        humanize(segment),
                        row_width,
                        row_number,
                        lists,
                    );
                }
            }
            if !is_enum {
                visual += 1;
            }
        }
        visual
    }

    /// One group band: a foldout, or — when its parent is a list — one of the
    /// list's element rows (drag handle, caret, `Element N`).
    #[allow(clippy::too_many_arguments)]
    fn band(
        &mut self,
        ui: &mut Ui,
        path: &str,
        depth: usize,
        label: String,
        row_width: f32,
        row_number: &mut usize,
        lists: &[(String, usize)],
    ) {
        let parent = path.rsplit_once('.').map_or("", |(parent, _)| parent);
        if lists.iter().any(|(list, _)| list == parent) {
            let index = path
                .rsplit('.')
                .next()
                .and_then(|part| part.parse::<usize>().ok())
                .unwrap_or(0);
            let now = ui.input(|input| input.time);
            let list_id = list_chrome_id(self.key, parent);
            let dragging = self.page.list_chrome.drag_index(list_id) == Some(index);
            if dragging
                && let Some(pointer) = ui.ctx().pointer_interact_pos()
                && let Some(offset) =
                    self.page
                        .list_chrome
                        .drag_offset(list_id, ui.cursor().top(), pointer)
            {
                // The whole element block follows the pointer: its band and
                // rows are drawn on a floating layer, translated by the grab
                // offset, while the parent reserves the normal height.
                let layer = egui::LayerId::new(
                    egui::Order::Tooltip,
                    Id::new(("kairos.list.block", list_id)),
                );
                self.ghost = Some(GhostBlock {
                    layer,
                    offset,
                    element: path.to_owned(),
                });
                let rect =
                    Rect::from_min_size(ui.cursor().min, vec2(row_width, self.chrome.row_height));
                let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layer_id(layer));
                // A child inherits its parent's clip, which is the scroll
                // viewport. The ghost must escape that viewport: clip it to
                // its own row instead, so a half-scrolled element drags whole.
                child.set_clip_rect(rect);
                child.disable();
                child.with_visual_transform(
                    egui::emath::TSTransform::from_translation(vec2(0.0, offset)),
                    |ui| self.element_band(ui, path, index, depth, row_width, 0.0),
                );
                ui.add_space(self.chrome.row_height);
                *row_number += 1;
                return;
            }
            self.ghost = None;
            let shift = self.page.list_chrome.shift(list_id, index, now);
            if shift != 0.0 {
                ui.add_space(shift);
                self.active_shift = Some((path.to_owned(), shift));
            }
            self.element_band(ui, path, index, depth, row_width, shift);
            *row_number += 1;
        } else {
            self.ghost = None;
            self.foldout(ui, path, depth, label, row_width);
        }
    }

    /// A list's header: the foldout band and the inline count box. The
    /// add/remove pair is drawn by [Self::list_footer] at the bottom-right of
    /// the list block, like Unity's reorderable list.
    fn list_header(&mut self, ui: &mut Ui, index: usize, depth: usize, row_width: f32) {
        let (path, name, len) = {
            let field = &self.page.fields[index];
            let len = match field.value {
                StyleValue::ListLen(len) => len,
                _ => 0,
            };
            (field.path.clone(), field.name.clone(), len)
        };
        let folded = self.page.collapsed.contains(&path);
        let outcome = ListChrome::header(
            ui,
            &mut self.page.list_chrome,
            &self.chrome,
            list_chrome_id(self.key, &path),
            ListChromeIds {
                header: group_id(self.key, &path),
                count: field_id(self.key, &path),
                add: list_add_id(self.key, &path),
                remove: list_remove_id(self.key, &path),
            },
            &name,
            len,
            folded,
            depth,
            row_width,
            LIST_MAX as usize,
        );
        if outcome.clicked {
            self.page.selected = Some(path.clone());
            self.clicked = true;
        }
        if let Some(folded) = outcome.folded {
            if folded {
                self.page.collapsed.insert(path.clone());
            } else {
                self.page.collapsed.remove(&path);
            }
            self.clicked = true;
        }
        if let Some(ListChromeEdit::SetLen { len, .. }) = outcome.edit {
            self.page.list_chrome.clear_shifts();
            let value = StyleValue::ListLen(len);
            self.out.push(set_field_action(self.key, &path, &value));
            self.page.fields[index].value = value;
            self.page.dirty = true;
            self.page.stale = true;
        }
        if outcome.finished && self.page.dirty {
            self.page.commit(self.out);
        }
    }

    /// Close one list block: the container frame ends at the current cursor,
    /// and the add/remove pair hangs outside it, flush under the bottom-right
    /// corner.
    fn list_footer(&mut self, ui: &mut Ui, path: &str, row_width: f32) {
        let Some((len, selected)) = self.page.fields.iter().find_map(|field| {
            if field.path != path || !field.value.is_list() {
                return None;
            }
            let len = match field.value {
                StyleValue::ListLen(len) => len,
                _ => 0,
            };
            let selected = selected_element_index(self.page.selected.as_deref(), path, len);
            Some((len, selected))
        }) else {
            return;
        };
        let outcome = ListChrome::footer(
            ui,
            &mut self.page.list_chrome,
            &self.chrome,
            list_chrome_id(self.key, path),
            ListChromeIds {
                header: group_id(self.key, path),
                count: field_id(self.key, path),
                add: list_add_id(self.key, path),
                remove: list_remove_id(self.key, path),
            },
            len,
            selected,
            row_width,
            LIST_MAX as usize,
        );
        if outcome.clicked {
            self.clicked = true;
        }
        match outcome.edit {
            Some(ListChromeEdit::Insert { index, .. }) => {
                // The new entry becomes the selection, so the next click of the
                // minus cell removes what the user just added.
                self.page.selected = Some(format!("{path}.{index}"));
                self.push_list_edit(path, ListEdit::Insert { index });
            }
            Some(ListChromeEdit::Remove { index, .. }) => {
                self.page.selected = None;
                self.push_list_edit(path, ListEdit::Remove { index });
            }
            Some(ListChromeEdit::SetLen { .. } | ListChromeEdit::Move { .. }) | None => {}
        }
    }

    /// One list element: drag handle, foldout caret, `Element N`. The band's
    /// fields follow as rows in the same block; the chrome only draws and
    /// reports the band interaction.
    #[allow(clippy::too_many_arguments)]
    fn element_band(
        &mut self,
        ui: &mut Ui,
        path: &str,
        index: usize,
        depth: usize,
        row_width: f32,
        shift: f32,
    ) {
        let list_path = path.rsplit_once('.').map_or("", |(list, _)| list);
        let selected = self.page.selected.as_deref() == Some(path);
        let folded = self.page.collapsed.contains(path);
        let outcome = ListChrome::band(
            ui,
            &mut self.page.list_chrome,
            &self.chrome,
            list_chrome_id(self.key, list_path),
            ListBandIds {
                fold: group_id(self.key, path),
                select: label_id(self.key, path),
                handle: list_handle_id(self.key, path),
            },
            index,
            &format!("Element {index}"),
            shift,
            depth,
            row_width,
            self.chrome.band_fill,
            selected,
            folded,
        );
        if outcome.pressed || outcome.drag_started {
            self.page.selected = Some(path.to_owned());
            self.clicked = true;
        }
        if let Some(folded) = outcome.folded {
            if folded {
                self.page.collapsed.insert(path.to_owned());
            } else {
                self.page.collapsed.remove(path);
            }
            self.clicked = true;
        }
        if outcome.selected {
            self.page.selected = Some(path.to_owned());
            self.clicked = true;
        }
    }

    /// Ask the owner to apply a structural list edit, then persist it: a list
    /// edit is a discrete click, not a drag stream, so live value and file go
    /// together.
    fn push_list_edit(&mut self, path: &str, edit: ListEdit) {
        self.page.list_chrome.clear_shifts();
        match self.key {
            PageKey::Panel(panel) => {
                self.out.push(Action::Style(StyleAction::EditList {
                    panel,
                    path: path.to_owned(),
                    edit,
                }));
                self.out.push(Action::Style(StyleAction::Commit { panel }));
            }
            PageKey::Shared(style) => {
                self.out
                    .push(Action::SharedStyle(SharedStyleAction::EditList {
                        style,
                        path: path.to_owned(),
                        edit,
                    }));
                self.out
                    .push(Action::SharedStyle(SharedStyleAction::Commit { style }));
            }
        }
        // The new entry's rows come from the owner's next field table, and a
        // text edit addressed into the old indices no longer means the same
        // entry.
        self.page.stale = true;
        self.page.dirty = false;
        self.page.edit = None;
        self.clicked = true;
    }

    /// Move one element when its drag is released. The owner applies the move
    /// at once, so the next pass paints the new order and the drop animation
    /// uses the slots the blocks land in.
    fn push_list_move(&mut self, path: &str, from: usize, to: usize) {
        let edit = ListEdit::Move { from, to };
        match self.key {
            PageKey::Panel(panel) => self.out.push(Action::Style(StyleAction::EditList {
                panel,
                path: path.to_owned(),
                edit,
            })),
            PageKey::Shared(style) => {
                self.out
                    .push(Action::SharedStyle(SharedStyleAction::EditList {
                        style,
                        path: path.to_owned(),
                        edit,
                    }));
            }
        }
        // The rows are re-read from the owner's new order, but the drag stays
        // alive: its rebound index addresses the same element.
        self.page.stale = true;
        self.page.dirty = true;
        self.page.edit = None;
        self.clicked = true;
    }

    /// Resolve the active drag from this pass's element bands. Holding the
    /// pointer previews the gap: the held block is drawn on the floating layer
    /// at the pointer, and the neighbours slide to their preview slots. The
    /// release emits the move once and commits the file once.
    fn finish_list_drag(&mut self, ui: &mut Ui) {
        let outcome = ListChrome::finish(ui, &mut self.page.list_chrome);
        if let Some(ListChromeEdit::Move { list, from, to }) = outcome.edit
            && let Some(path) = self.page.fields.iter().find_map(|field| {
                (field.value.is_list() && list_chrome_id(self.key, &field.path) == list)
                    .then(|| field.path.clone())
            })
        {
            self.page.selected = Some(format!("{path}.{to}"));
            self.push_list_move(&path, from, to);
        }
        if outcome.finished {
            // A drag release is still a click inside the list: it must not be
            // treated as the empty-page click that clears the selection.
            self.clicked = true;
            self.page.commit(self.out);
        }
    }

    /// One foldout band: triangle, left-aligned group name, full-width hit
    /// area. Unity calls this a foldout; the RON calls it a nested struct.
    fn foldout(&mut self, ui: &mut Ui, path: &str, depth: usize, label: String, row_width: f32) {
        let style = self.style;
        let (rect, _) = ui.allocate_exact_size(vec2(row_width, style.group_height), Sense::hover());
        let ghost = self.ghost.as_ref().and_then(|ghost| {
            (path == ghost.element || path.starts_with(&format!("{}.", ghost.element)))
                .then_some((ghost.layer, ghost.offset))
        });
        if let Some((layer, offset)) = ghost {
            // A nested group inside the dragged block: reserve its band and
            // draw it on the floating layer with the rest of the block.
            let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layer_id(layer));
            // Clip the floating row to itself, not to the scroll viewport it
            // may have been cut by before the drag.
            child.set_clip_rect(rect);
            child.disable();
            child.with_visual_transform(
                egui::emath::TSTransform::from_translation(vec2(0.0, offset)),
                |ui| self.foldout_at(ui, rect, path, depth, &label),
            );
            return;
        }
        self.foldout_at(ui, rect, path, depth, &label);
    }

    fn foldout_at(&mut self, ui: &mut Ui, rect: Rect, path: &str, depth: usize, label: &str) {
        let style = self.style;
        let response = ui
            .interact(rect, group_id(self.key, path), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);

        let collapsed = self.page.collapsed.contains(path);
        let fill = if response.hovered() {
            style.group_hover
        } else {
            style.group_background
        };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::ZERO, fill.to_egui());

        let arrow = pos2(
            rect.left() + style.row_inset + depth as f32 * style.group_indent + 4.0,
            rect.center().y,
        );
        paint_foldout_arrow(painter, arrow, collapsed, style.group.to_egui());

        let text_left = rect.left() + style.row_inset + 16.0 + depth as f32 * style.group_indent;
        let text = TextStyle::sized(style.group.to_egui(), style.group_size);
        let galley = text.layout(ui, label);
        let text_pos = pos2(text_left, rect.center().y - galley.size().y * 0.5);
        let color = style.group.to_egui();
        painter.with_clip_rect(rect).galley(text_pos, galley, color);

        labelled_widget_info(
            &response,
            WidgetType::SelectableLabel,
            label,
            Some(!collapsed),
        );

        if response.clicked() {
            if collapsed {
                self.page.collapsed.remove(path);
            } else {
                self.page.collapsed.insert(path.to_owned());
            }
            self.clicked = true;
        }

        let _ = response.on_hover_text(path.to_owned());
    }

    /// One leaf row: its band, left name, value box.
    ///
    /// `slot` says where the name starts (a list element's fields keep the
    /// element's gutter) and `block` says whether this row hangs under an
    /// enum's menu button. A payload row starts at the menu button, which is
    /// that block's value column; a row with neither is an ordinary alternating
    /// band.
    #[allow(clippy::too_many_arguments)]
    fn row(
        &mut self,
        ui: &mut Ui,
        index: usize,
        slot: RowSlot,
        block: EnumBlock,
        row_width: f32,
        row_number: usize,
    ) {
        let style = self.style;
        // Element rows are list chrome; ordinary property rows keep the
        // panel's own height.
        let row_height = if slot.element.is_some() {
            self.chrome.row_height
        } else {
            style.row_height
        };
        let (rect, _) = ui.allocate_exact_size(vec2(row_width, row_height), Sense::hover());
        let path = self.page.fields[index].path.clone();
        let ghost = self.ghost.as_ref().and_then(|ghost| {
            (path == ghost.element || path.starts_with(&format!("{}.", ghost.element)))
                .then_some((ghost.layer, ghost.offset))
        });
        if let Some((layer, offset)) = ghost {
            // The dragged block's row: keep the parent slot, paint the real
            // row on the floating layer at the pointer.
            let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layer_id(layer));
            // Clip the floating row to itself, not to the scroll viewport it
            // may have been cut by before the drag.
            child.set_clip_rect(rect);
            child.disable();
            child.with_visual_transform(
                egui::emath::TSTransform::from_translation(vec2(0.0, offset)),
                |ui| self.row_at(ui, rect, index, slot, block, row_number),
            );
            return;
        }
        self.row_at(ui, rect, index, slot, block, row_number);
    }

    fn row_at(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        index: usize,
        slot: RowSlot,
        block: EnumBlock,
        row_number: usize,
    ) {
        let style = self.style;
        let key = self.key;
        let path = self.page.fields[index].path.clone();
        let in_element = slot.element.is_some();
        // A row inside a list element uses the list skin's inset and indent;
        // a page-level row keeps the panel's inspector metrics.
        let (row_inset, group_indent) = if in_element {
            (self.chrome.row_inset, self.chrome.group_indent)
        } else {
            (style.row_inset, style.group_indent)
        };
        let gutter_width = self.chrome.gutter_width;
        // Inside an element the fields stripe on the band background itself,
        // so the block and its header read as one raised control. Outside,
        // the page keeps its own two-grey property stripes.
        let fill = if in_element {
            if slot.element_row.is_multiple_of(2) {
                self.chrome.band_fill
            } else {
                self.chrome.band_alt
            }
        } else {
            style.row_fill(row_number).to_egui()
        };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::ZERO, fill);
        if in_element && slot.last_in_element {
            // The last field closes the block with the same dark edge the band
            // has under its header.
            painter.rect_filled(
                Rect::from_min_size(
                    pos2(rect.left(), rect.bottom() - 1.0),
                    vec2(rect.width(), 1.0),
                ),
                CornerRadius::ZERO,
                self.chrome.container_border,
            );
        }

        let name = self.page.fields[index].name.clone();
        let is_number = self.page.fields[index].value.is_number();

        // Unity's layout: one label column, then a value column that fills the
        // rest of the row. The label column shrinks before the value box would
        // run under the scroll bar.
        let content_left = rect.left() + row_inset;
        let content_right = (rect.right() - row_inset).max(content_left + 1.0);
        let content_width = content_right - content_left;
        let max_label = (content_width - style.label_gap - style.field_width).max(48.0);
        let label_width = style
            .label_width
            .min(max_label)
            .min((content_width - style.label_gap - 1.0).max(0.0));
        let value_left = content_left + label_width + style.label_gap;

        // A payload row hangs under the menu button: the button's left edge is
        // the row's value column, so the payload gets a nested label/value
        // split inside that column. It is *not* back at the row's left edge;
        // the enum row keeps that column for its own name.
        let (label_left, value_left, value_width) = if block.payload {
            let base_width = (content_right - value_left).max(1.0);
            let sub_label = (base_width * 0.42).clamp(48.0, style.label_width);
            let sub_value = value_left + sub_label + style.label_gap;
            (value_left, sub_value, (content_right - sub_value).max(1.0))
        } else {
            (
                content_left,
                value_left,
                (content_right - value_left).max(1.0),
            )
        };
        let value_rect = Rect::from_min_size(
            pos2(value_left, rect.center().y - self.field.height * 0.5),
            vec2(value_width, self.field.height),
        );
        let label_rect = Rect::from_min_max(
            pos2(label_left, rect.top()),
            pos2(value_left - style.label_gap, rect.bottom()),
        );
        let label_x = if block.payload {
            let relative = slot.depth.saturating_sub(block.base_depth);
            label_left + relative as f32 * style.group_indent
        } else if let Some(element) = slot.element {
            let x0 = rect.left() + row_inset + element.saturating_sub(1) as f32 * group_indent;
            x0 + gutter_width
                + 4.0
                + 16.0
                + slot.depth.saturating_sub(element) as f32 * group_indent
        } else {
            content_left + 16.0 + slot.depth as f32 * style.group_indent
        };

        // The name is an interaction, not just paint: clicking it selects the
        // field, a text field takes keyboard focus so typing starts, and a
        // number field is scrubbed by dragging it (Unity's left/right cursor).
        let label = FieldLabel::new(label_id(key, &path), &name, self.field).draggable(is_number);
        let label_response = label.interact(ui, label_rect);
        if label_response.clicked() {
            self.page.selected = Some(path.clone());
            self.clicked = true;
            if matches!(self.page.fields[index].value, StyleValue::Text(_)) {
                ui.memory_mut(|memory| memory.request_focus(field_id(key, &path)));
            }
        }

        let PageState {
            fields,
            selected,
            edit,
            ..
        } = &mut *self.page;
        let field = &mut fields[index];
        let outcome = draw_value(
            ui,
            self.field,
            key,
            self.picker,
            field,
            selected,
            edit,
            &mut self.clicked,
            value_rect,
            Some(&label_response),
        );

        if outcome.popup_open {
            // A click in the picker is still an interaction with this row;
            // without this the page's outside-click sweep would clear the
            // selection while the user is editing the color.
            self.clicked = true;
        }

        if outcome.changed {
            let field = &self.page.fields[index];
            self.out
                .push(set_field_action(key, &field.path, &field.value));
            self.page.dirty = true;
            if field.value.is_variant() {
                // The new variant's payload is a different set of rows; the
                // page re-reads the owner's table on the next pass.
                self.page.stale = true;
            }
        }
        if outcome.finished_edit && self.page.dirty {
            self.page.commit(self.out);
        }

        // Paint the name after the value: a click on the value updates the
        // selection, and the blue label should appear in the same pass.
        let focused = ui.memory(|memory| memory.has_focus(field_id(key, &path)));
        let editing = self
            .page
            .edit
            .as_ref()
            .is_some_and(|edit| edit.path == path);
        let active = self.page.selected.as_deref() == Some(path.as_str()) || focused || editing;
        label.paint(
            ui,
            Rect::from_min_max(
                pos2(label_x, rect.top()),
                pos2(label_rect.right(), rect.bottom()),
            ),
            active,
        );

        let _ = label_response.response.on_hover_text(path);
    }
}

/// What one value control did this pass.
#[derive(Clone, Copy, Debug, Default)]
struct ValueOutcome {
    changed: bool,
    /// A typed numeric edit ended (Enter, Escape, or focus loss); the caller
    /// writes the file now instead of waiting for a pointer release.
    finished_edit: bool,
    /// The color picker popup is open. Clicks inside it must not clear the
    /// row selection, so the page treats them as a row interaction.
    popup_open: bool,
}

/// One token's control: the page's field table mapped onto the reusable field
/// widgets (kairos_editor_widgets::field). The widgets know boxes, drags and
/// checkmarks; the mapping between a StyleValue and the plain value a widget
/// edits lives here, next to the action that follows from it.
#[allow(clippy::too_many_arguments)]
fn draw_value(
    ui: &mut Ui,
    field_style: &FieldStyle,
    key: PageKey,
    picker: &ColorPickerStyle,
    field: &mut StyleField,
    selected: &mut Option<String>,
    edit: &mut Option<FieldEdit>,
    clicked: &mut bool,
    rect: Rect,
    label: Option<&FieldLabelResponse>,
) -> ValueOutcome {
    let path = field.path.clone();
    let name = field.name.clone();
    let variants = field.variants.unwrap_or(&[]);
    let range = field.range;
    let id = field_id(key, &path);
    let editing = edit.as_ref().is_some_and(|current| current.path == path);
    let active = selected.as_deref() == Some(path.as_str())
        || editing
        || ui.memory(|memory| memory.has_focus(id));

    macro_rules! number_row {
        ($value:expr) => {
            draw_number(
                ui,
                field_style,
                key,
                &path,
                &name,
                $value,
                selected,
                edit,
                clicked,
                rect,
                label,
                range,
            )
        };
    }

    match &mut field.value {
        StyleValue::F32(value) => number_row!(value),
        StyleValue::F64(value) => number_row!(value),
        StyleValue::I8(value) => number_row!(value),
        StyleValue::I16(value) => number_row!(value),
        StyleValue::I32(value) => number_row!(value),
        StyleValue::I64(value) => number_row!(value),
        StyleValue::Isize(value) => number_row!(value),
        StyleValue::U8(value) => number_row!(value),
        StyleValue::U16(value) => number_row!(value),
        StyleValue::U32(value) => number_row!(value),
        StyleValue::U64(value) => number_row!(value),
        StyleValue::Usize(value) => number_row!(value),
        StyleValue::Text(value) => {
            let outcome = TextField::new(id, value, field_style)
                .selected(active)
                .show(ui, rect);
            if outcome.clicked() || outcome.response.has_focus() {
                *selected = Some(path);
                if outcome.clicked() {
                    *clicked = true;
                }
            }
            finished(outcome)
        }
        StyleValue::Bool(on) => {
            let outcome = ToggleField::new(toggle_id(key, &path), on, field_style)
                .selected(active)
                .label(&name)
                .show(ui, rect);
            select_row(outcome.clicked(), &path, selected, clicked);
            finished(outcome)
        }
        StyleValue::Color(color) => draw_color(
            ui,
            picker,
            color,
            ColorUsage::LDR,
            selected,
            clicked,
            rect,
            id,
            &path,
            active,
        ),
        StyleValue::HdrColor(color) => draw_color(
            ui,
            picker,
            color,
            ColorUsage::HDR,
            selected,
            clicked,
            rect,
            id,
            &path,
            active,
        ),
        StyleValue::Variant(current) => {
            if variants.is_empty() {
                return ValueOutcome::default();
            }
            // The page has the variant names and the active id, not the
            // owner's typed enum: this control edits that id in place.
            let outcome = VariantMenu::new(id, current, variants, field_style)
                .selected(active)
                .label(&name)
                .show(ui, rect);
            select_row(
                outcome.clicked() || outcome.changed(),
                &path,
                selected,
                clicked,
            );
            let popup_open = egui::Popup::is_id_open(ui.ctx(), enum_popup_id(id));
            ValueOutcome {
                changed: outcome.changed(),
                finished_edit: false,
                popup_open,
            }
        }
        // A list draws its own header: the count box is a number field the
        // header builds, so a list node never reaches a row's value column.
        StyleValue::ListLen(_) => ValueOutcome::default(),
    }
}

/// Draw one number token with its concrete type. The `N` bounds in
/// `NumberField` are the type safety; `range` is only an optional UI clamp
/// (the list count's growth cap), never the type's own range.
#[allow(clippy::too_many_arguments)]
fn draw_number<N: Number>(
    ui: &mut Ui,
    field_style: &FieldStyle,
    key: PageKey,
    path: &str,
    name: &str,
    value: &mut N,
    selected: &mut Option<String>,
    edit: &mut Option<FieldEdit>,
    clicked: &mut bool,
    rect: Rect,
    label: Option<&FieldLabelResponse>,
    range: Option<(f64, f64)>,
) -> ValueOutcome {
    let id = field_id(key, path);
    let editing = edit.as_ref().is_some_and(|current| current.path == path);
    let active =
        selected.as_deref() == Some(path) || editing || ui.memory(|memory| memory.has_focus(id));
    let mut number_field = NumberField::new(id, value, field_style)
        .selected(active)
        .label(name);
    if let Some((min, max)) = range {
        number_field = number_field.range(min, max);
    }
    if let Some(label) = label {
        number_field = number_field.dragged_by(label);
    }
    let mut number = take_number(edit, path);
    let outcome = number_field.show(ui, rect, &mut number);
    keep_number(edit, path, number);
    select_row(selected_row(&outcome), path, selected, clicked);
    finished(outcome)
}

/// The number editor buffer for this field, taken out of the page so the widget
/// can borrow it. A buffer that belongs to another row stays where it is.
fn take_number(edit: &mut Option<FieldEdit>, path: &str) -> Option<NumberEdit> {
    if edit.as_ref().is_some_and(|current| current.path == path) {
        return edit.take().map(|current| current.number);
    }
    None
}

/// Put the buffer back unless the edit ended (Enter, Escape, focus loss).
fn keep_number(edit: &mut Option<FieldEdit>, path: &str, number: Option<NumberEdit>) {
    if let Some(number) = number {
        *edit = Some(FieldEdit {
            path: path.to_owned(),
            number,
        });
    }
}

/// A click, a scrub or a typed value on a box selects its row, so the name turns
/// blue and the box keeps the accent edge.
fn selected_row(outcome: &FieldOutcome) -> bool {
    outcome.clicked() || outcome.changed() || outcome.response.dragged()
}

fn select_row(clicked: bool, path: &str, selected: &mut Option<String>, out: &mut bool) {
    if clicked {
        *selected = Some(path.to_owned());
        *out = true;
    }
}

fn finished(outcome: FieldOutcome) -> ValueOutcome {
    ValueOutcome {
        changed: outcome.changed(),
        finished_edit: outcome.finished,
        popup_open: false,
    }
}

/// A color token: Unity's inspector field, a full-width swatch with the HDR
/// badge when the field's ColorProperty has HDR usage and the eyedropper slot.
/// Clicking the swatch opens the shared color picker rather than egui's
/// built-in one; ColorField is already a widget, so this only hands it the
/// exact `Color` inside the `StyleValue::Color`/`StyleValue::HdrColor` row.
#[allow(clippy::too_many_arguments)]
fn draw_color(
    ui: &mut Ui,
    picker_style: &ColorPickerStyle,
    color: &mut Color,
    usage: ColorUsage,
    selected: &mut Option<String>,
    clicked: &mut bool,
    rect: Rect,
    id: Id,
    path: &str,
    active: bool,
) -> ValueOutcome {
    let mut property = ColorProperty::new(*color, usage);
    let response = ui.put(
        rect,
        ColorField::new(id, &mut property, picker_style)
            .selected(active)
            .height(rect.height()),
    );
    // A pick's confirming click never reaches egui (the host masks it), so
    // the eyedropper reports its own end of gesture; without that the page
    // would keep the committed colour only until the next pointer release.
    let committed = eyedropper::take_committed(ui.ctx());

    let mut changed = false;
    if response.changed() {
        *color = property.color;
        changed = true;
    }
    if response.clicked() {
        *selected = Some(path.to_owned());
        *clicked = true;
    }

    let popup_open = is_color_picker_open(ui.ctx(), id);

    ValueOutcome {
        changed,
        finished_edit: committed,
        popup_open,
    }
}

fn paint_foldout_arrow(
    painter: &egui::Painter,
    center: Pos2,
    collapsed: bool,
    color: egui::Color32,
) {
    let points = if collapsed {
        vec![
            center + vec2(-2.5, -4.0),
            center + vec2(-2.5, 4.0),
            center + vec2(2.5, 0.0),
        ]
    } else {
        vec![
            center + vec2(-4.0, -2.5),
            center + vec2(4.0, -2.5),
            center + vec2(0.0, 2.5),
        ]
    };
    painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

/// Where one row sits horizontally.
///
/// `depth` is the *visual* depth: an enum token itself adds no label level.
/// A payload row is laid out under the menu button (see [EnumBlock]), not in
/// the enum row's label column. `element` is the visual depth of the list
/// element block the row is inside, if any; every row of that block keeps its
/// gutter and stripes on the element's raised background.
#[derive(Clone, Copy)]
struct RowSlot {
    depth: usize,
    element: Option<usize>,
    /// Zero-based field row inside the element, for the two-grey stripe.
    element_row: usize,
    /// Last row of the element block: it paints the block's bottom edge.
    last_in_element: bool,
}

/// How a row relates to an enum's payload.
#[derive(Clone, Copy, Default)]
struct EnumBlock {
    /// The row is a payload row, not the enum row itself: it is laid out under
    /// the menu button (whose column starts at the row's value column) rather
    /// than in the row's left label column.
    payload: bool,
    /// Visual depth of the enum row that owns the payload, so a nested struct
    /// inside the payload indents from the menu button rather than the page.
    base_depth: usize,
}

/// The visual depth of a dotted group path: one level per segment that is not
/// an enum token.
///
/// An enum segment contributes no depth of its own: its menu button is the
/// reference for its payload. The payload rows are indented from that button
/// by [EnumBlock::base_depth], not from the page's left edge. A nested struct
/// inside the payload still adds one level.
fn visual_depth(group: &str, enums: &[String]) -> usize {
    let mut depth = 0;
    let mut prefix = String::new();
    for segment in group.split('.').filter(|segment| !segment.is_empty()) {
        if !prefix.is_empty() {
            prefix.push('.');
        }
        prefix.push_str(segment);
        if !enums.iter().any(|path| path == &prefix) {
            depth += 1;
        }
    }
    depth
}

/// The deepest list element dotted path (`list.N`) containing `group`.
///
/// The depth is not enough to tell elements apart: two entries of one list sit
/// at the same depth. The path is what identifies the element block whose
/// stripes and edges a row belongs to.
fn element_path(group: &str, lists: &[String]) -> Option<String> {
    let mut best: Option<String> = None;
    for list in lists {
        let Some(rest) = group
            .strip_prefix(list.as_str())
            .and_then(|rest| rest.strip_prefix('.'))
        else {
            continue;
        };
        let Some(index) = rest.split('.').next() else {
            continue;
        };
        if index.parse::<usize>().is_err() {
            continue;
        }
        let element = format!("{list}.{index}");
        if best
            .as_ref()
            .is_none_or(|current| current.len() < element.len())
        {
            best = Some(element);
        }
    }
    best
}

/// How a row relates to the innermost enum that owns it.
///
/// `blocks` are the enum paths whose active variant has at least one payload
/// row. A payload hangs under the menu button; it gets no tint and no border,
/// so it stays part of the same element block as the field rows above it.
fn enum_block(path: &str, blocks: &[String], enums: &[String]) -> EnumBlock {
    // The payload is laid out under the menu button of the *innermost* enum
    // that strictly contains this row: the row's own enum is not a payload,
    // but a nested enum row is a payload of its parent.
    let mut parent: Option<&String> = None;
    for block in blocks {
        if path.starts_with(&format!("{block}."))
            && parent.is_none_or(|current| block.len() > current.len())
        {
            parent = Some(block);
        }
    }
    EnumBlock {
        payload: parent.is_some(),
        base_depth: parent.map_or(0, |block| visual_depth(block, enums)),
    }
}

/// The dotted group path of a token (icons.console), or an empty string for a
/// root token.
fn group_path(path: &str) -> String {
    path.rsplit_once('.')
        .map_or_else(String::new, |(group, _)| group.to_owned())
}

/// Whether one of `segments`' prefixes — a group this row sits in, at any depth
/// — is folded. `segments` are segment names, so the dotted paths the collapse
/// set stores are rebuilt here.
fn collapsed_above(segments: &[&str], collapsed: &HashSet<String>) -> bool {
    let mut path = String::new();
    for segment in segments {
        if !path.is_empty() {
            path.push('.');
        }
        path.push_str(segment);
        if collapsed.contains(&path) {
            return true;
        }
    }
    false
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
                // A picker or menu opened from the old values would be stale
                // over the reset page; close it before asking for the defaults.
                egui::Popup::close_all(ui.ctx());
                close_all_color_pickers(ui.ctx());
                page.reset();
                match key {
                    PageKey::Panel(panel) => {
                        out.push(Action::Style(StyleAction::Reset { panel }));
                    }
                    PageKey::Shared(shared) => {
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
    use egui::{Color32 as EguiColor32, Key, Modifiers};
    use kairos_color::Color;
    use kairos_editor_core::EditorState;
    use kairos_editor_widgets::color_picker::color_swatch_id;
    use kairos_editor_widgets::field::enum_item_id;
    use kairos_editor_widgets::style::Style;

    use crate::shared_style::{COLOR_PICKER, FONTS, LIST, SharedStyles};
    use crate::skin::PanelSkin;

    struct NoHost;

    impl HostView for NoHost {}

    fn fields() -> Vec<StyleField> {
        vec![
            StyleField::new("height", "Height", StyleValue::F32(20.0)),
            StyleField::new("enabled", "Enabled", StyleValue::Bool(false)),
            StyleField::new(
                "icons.warning",
                "Warning",
                StyleValue::Text("warning.png".to_owned()),
            ),
            StyleField::new(
                "icons.error",
                "Error",
                StyleValue::Text("error.png".to_owned()),
            ),
            StyleField::new(
                "icons.textures.tint",
                "Tint",
                StyleValue::Text("tint.png".to_owned()),
            ),
        ]
    }

    fn skins() -> Vec<PanelSkin> {
        vec![
            PanelSkin {
                panel: panels::CONSOLE,
                title: "Console".to_owned(),
                file: "ConsoleStyle.ron",
                fields: fields(),
            },
            PanelSkin {
                panel: panels::PROFILER,
                title: "Profiler".to_owned(),
                file: "Profiler.ron",
                fields: fields(),
            },
        ]
    }

    fn color_fields() -> Vec<StyleField> {
        vec![
            StyleField::new(
                "background",
                "Background",
                StyleValue::Color(Color::from_srgb8([20, 140, 220, 255])),
            ),
            StyleField::new(
                "glow",
                "Glow",
                StyleValue::HdrColor(Color::new(2.0, 1.0, 0.5, 1.0)),
            ),
        ]
    }

    fn color_skins() -> Vec<PanelSkin> {
        vec![PanelSkin {
            panel: panels::CONSOLE,
            title: "Console".to_owned(),
            file: "ConsoleStyle.ron",
            fields: color_fields(),
        }]
    }

    /// Enough rows to force the page's scroll bar, so the right-hand gutter
    /// can be measured.
    fn long_skin() -> Vec<PanelSkin> {
        let mut fields = fields();
        for index in 0..40 {
            fields.push(StyleField::new(
                format!("token_{index}"),
                format!("Token {index}"),
                StyleValue::F32(index as f32),
            ));
        }
        vec![PanelSkin {
            panel: panels::CONSOLE,
            title: "Console".to_owned(),
            file: "ConsoleStyle.ron",
            fields,
        }]
    }

    /// A style enum with a payload: the menu row plus the active variant's
    /// fields, all at one label column.
    const MODE_VARIANTS: &[&str] = &["Off", "Soft"];

    fn enum_fields(mode: &str) -> Vec<StyleField> {
        let mut fields = vec![
            StyleField::new("mode", "Mode", StyleValue::Variant(mode.to_owned()))
                .with_variants(MODE_VARIANTS),
        ];
        if mode == "Soft" {
            fields.push(StyleField::new(
                "mode.radius",
                "Radius",
                StyleValue::F32(4.0),
            ));
            fields.push(StyleField::new(
                "mode.samples",
                "Samples",
                StyleValue::U32(2),
            ));
        }
        fields
    }

    fn enum_skin(mode: &str) -> Vec<PanelSkin> {
        vec![PanelSkin {
            panel: panels::CONSOLE,
            title: "Console".to_owned(),
            file: "ConsoleStyle.ron",
            fields: enum_fields(mode),
        }]
    }

    /// Headless panel harness: one egui context, one frame's worth of skins and
    /// one panel. Actions are collected, not applied: they are the panel's only
    /// output.
    struct Harness {
        ctx: Context,
        size: egui::Vec2,
        state: EditorState,
        skins: Vec<PanelSkin>,
        /// The live shared skins this pass reads; tests may edit one to prove
        /// the page is wired to the registry and not to local constants.
        shared: SharedStyles,
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
                shared: SharedStyles::default(),
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
                shared,
                panel,
                ..
            } = self;
            let frame = Frame::with_host(state, &host)
                .with_skins(skins)
                .with_shared(shared);
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
            self.click_at(rect.center())
        }

        fn click_at(&mut self, pos: egui::Pos2) -> (egui::FullOutput, Vec<Action>) {
            self.frame(vec![egui::Event::PointerMoved(pos)]);
            self.frame(vec![pointer(pos, true)]);
            self.frame(vec![pointer(pos, false)])
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

    #[derive(Clone, Debug)]
    struct PaintedText {
        text: String,
        rect: egui::Rect,
        color: EguiColor32,
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
                    color: text.override_text_color.unwrap_or(text.fallback_color),
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

    #[test]
    fn the_page_list_shows_every_styled_panel_and_its_field_tree() {
        let mut harness = Harness::new(skins());

        let (output, actions) = harness.frame(Vec::new());
        let text = painted_text(&output);

        assert!(text.contains("Console"), "{text}");
        assert!(text.contains("Profiler"), "{text}");
        // Shared styles are always listed after the live panel skins.
        assert!(text.contains("Toolbar"), "{text}");
        assert!(text.contains("Color Picker"), "{text}");
        // The first page's heading, file and rows.
        assert!(
            text.contains("Preferences/Styles/ConsoleStyle.ron"),
            "{text}"
        );
        assert!(text.contains("Height"), "{text}");
        assert!(text.contains("Enabled"), "{text}");
        // Nested tokens are foldout bands, not dim labels.
        assert!(text.contains("Icons"), "{text}");
        assert!(text.contains("Warning"), "{text}");
        assert!(text.contains("Textures"), "{text}");
        assert!(text.contains("Tint"), "{text}");
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
                    value: StyleValue::Bool(true),
                }),
                Action::Style(StyleAction::Commit {
                    panel: panels::CONSOLE,
                }),
            ]
        );
        assert!(
            matches!(harness.panel.page.fields[1].value, StyleValue::Bool(true)),
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

    /// Reset replaces the page's live style with its code defaults. Closing is
    /// the dock's job, so the footer has no close action of its own.
    #[test]
    fn the_footer_reset_button_asks_the_owner_for_code_defaults() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());
        harness.panel.page.dirty = true;

        let (_, actions) = harness.click(reset_id(panels::CONSOLE));

        assert_eq!(
            actions,
            vec![Action::Style(StyleAction::Reset {
                panel: panels::CONSOLE
            })]
        );
        assert!(
            harness.panel.page.stale,
            "the next pass re-reads the reset field table"
        );
        assert!(
            !harness.panel.page.dirty,
            "a reset discards the pending edit instead of committing it"
        );
    }

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
    /// open, and its indexed rows are ordinary tokens.
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

    /// Shared edits use the separate action path: they have no panel instance
    /// to route a SetField or Commit to.
    #[test]
    fn a_dirty_shared_page_commits_through_the_shared_action() {
        let mut out = UiOut::new();
        let mut page = PageState {
            key: Some(PageKey::Shared(COLOR_PICKER)),
            dirty: true,
            ..PageState::default()
        };

        page.commit(&mut out);

        assert_eq!(
            out.take(),
            vec![Action::SharedStyle(SharedStyleAction::Commit {
                style: COLOR_PICKER
            })]
        );

        let value = StyleValue::F32(0.9);
        assert_eq!(
            set_field_action(PageKey::Shared(COLOR_PICKER), "border_width", &value),
            Action::SharedStyle(SharedStyleAction::SetField {
                style: COLOR_PICKER,
                path: "border_width".to_owned(),
                value,
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

    /// The editor edits its own skin too: it is just another panel in the list.
    #[test]
    fn the_preferences_panel_lists_its_own_skin() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let state = EditorState::new();
        let host = NoHost;
        let frame = Frame::with_host(&state, &host);
        let own = harness.panel.skin(&frame).expect("preferences has a skin");
        assert_eq!(own.panel, PREFERENCES);
        assert_eq!(own.file, "PreferencesPanel.ron");
        assert_eq!(
            own.title,
            harness.panel.title(&frame).text(),
            "the page is named by the panel's own title, not a second literal"
        );
        assert!(own.fields.iter().any(|field| field.path == "nav_width"));
        assert!(
            own.fields.iter().any(|field| field.path == "row_odd"),
            "the new zebra tokens are editable too"
        );
    }

    #[test]
    fn color_fields_open_our_picker_and_write_the_style() {
        let mut harness = Harness::with_size(color_skins(), vec2(700.0, 800.0));
        harness.frame(Vec::new());

        let field = field_id(panels::CONSOLE, "background");
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
        assert_eq!(
            actions,
            vec![
                Action::Style(StyleAction::SetField {
                    panel: panels::CONSOLE,
                    path: "background".to_owned(),
                    value: StyleValue::Color(Color::from_srgb8([0, 0, 0, 255])),
                }),
                Action::Style(StyleAction::Commit {
                    panel: panels::CONSOLE,
                }),
            ]
        );
        assert_eq!(
            harness.panel.page.fields[0].value,
            StyleValue::Color(Color::from_srgb8([0, 0, 0, 255]))
        );
        assert_eq!(
            harness.panel.page.selected.as_deref(),
            Some("background"),
            "working inside the popup keeps the row selected"
        );
    }

    #[test]
    fn an_hdr_field_shows_the_badge_and_the_intensity_picker() {
        let mut harness = Harness::with_size(color_skins(), vec2(700.0, 800.0));
        harness.frame(Vec::new());

        let field = field_id(panels::CONSOLE, "glow");
        let (_, actions) = harness.click(field);
        assert!(actions.is_empty(), "{actions:?}");
        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);
        assert!(text.contains("HDR"), "{text}");
        assert!(text.contains("HDR Color"), "{text}");
        assert!(text.contains("Intensity"), "{text}");
        assert!(text.contains("+2"), "{text}");
    }

    #[test]
    fn field_rows_alternate_between_two_greys() {
        let style = PreferencesPanelStyle::dark();
        let mut harness = Harness::new(skins());

        let (output, _) = harness.frame(Vec::new());

        let odd = style.row_odd.to_egui();
        let even = style.row_even.to_egui();
        let mut bands: Vec<PaintedRect> = painted_rects(&output)
            .into_iter()
            .filter(|rect| {
                (rect.fill == odd || rect.fill == even)
                    && approx(rect.rect.height(), style.row_height)
            })
            .collect();
        bands.sort_by(|a, b| a.rect.top().total_cmp(&b.rect.top()));

        assert_eq!(bands.len(), 5, "one band per leaf row: {bands:?}");
        let expected = [odd, even, odd, even, odd];
        let actual: Vec<EguiColor32> = bands.iter().map(|band| band.fill).collect();
        assert_eq!(
            actual, expected,
            "the five leaf rows should stripe odd/even/odd/even/odd"
        );
    }

    #[test]
    fn labels_are_left_aligned_and_indented_by_group_depth() {
        let style = PreferencesPanelStyle::dark();
        let mut harness = Harness::new(skins());

        let (output, _) = harness.frame(Vec::new());
        let runs = painted_texts(&output);
        let height = run(&runs, "Height");
        let enabled = run(&runs, "Enabled");
        let warning = run(&runs, "Warning");
        let tint = run(&runs, "Tint");

        assert!(
            approx(height.rect.left(), enabled.rect.left()),
            "left-aligned labels share an edge regardless of text width: Height {:?}, Enabled {:?}",
            height.rect,
            enabled.rect
        );
        assert!(
            approx(warning.rect.left(), height.rect.left() + style.group_indent),
            "a group's leaf is indented one level: Warning {:?} vs Height {:?}",
            warning.rect,
            height.rect
        );
        assert!(
            approx(
                tint.rect.left(),
                height.rect.left() + 2.0 * style.group_indent
            ),
            "a nested group's leaf is indented two levels: Tint {:?} vs Height {:?}",
            tint.rect,
            height.rect
        );

        // The value column does not move with the label's depth.
        let height_box = harness.rect_of(field_id(panels::CONSOLE, "height"));
        let warning_box = harness.rect_of(field_id(panels::CONSOLE, "icons.warning"));
        assert!(
            approx(height_box.left(), warning_box.left()),
            "value boxes share one column: {height_box:?} vs {warning_box:?}"
        );
        assert!(
            height.rect.right() < height_box.left(),
            "the name column ends before the value column: {:?} vs {:?}",
            height.rect,
            height_box
        );
    }

    #[test]
    fn clicking_a_foldout_collapses_its_rows() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (output, actions) = harness.click(group_id(panels::CONSOLE, "icons"));
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

        let (output, _) = harness.click(group_id(panels::CONSOLE, "icons"));
        let text = painted_text(&output);
        assert!(text.contains("Warning"), "{text}");
        assert!(text.contains("Textures"), "{text}");
        assert!(text.contains("Tint"), "{text}");
    }

    #[test]
    fn a_nested_foldout_collapses_only_its_own_rows() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (output, _) = harness.click(group_id(panels::CONSOLE, "icons.textures"));
        let text = painted_text(&output);

        assert!(text.contains("Warning"), "{text}");
        assert!(!text.contains("Tint"), "{text}");
        assert!(text.contains("Textures"), "{text}");
    }

    /// The font list's header and element bands, and the fold rules: the list
    /// band folds every entry, an element's caret folds just its fields.
    #[test]
    fn the_font_list_has_one_header_and_folds_each_element() {
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, 470.0));
        harness.frame(Vec::new());
        harness.click(nav_id(FONTS));
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
        assert_eq!(
            runs.iter().filter(|run| run.text == "2").count(),
            1,
            "the header shows the entry count: {text}"
        );

        // Unity's list header carries the count inline; the add/remove pair
        // lives under the rows, at the bottom-right of the block.
        let header = harness.rect_of(group_id(PageKey::Shared(FONTS), "proportional"));
        let count = harness.rect_of(field_id(PageKey::Shared(FONTS), "proportional"));
        assert!(
            approx(header.center().y, count.center().y),
            "the count box shares the header row: header {header:?}, count {count:?}"
        );
        assert!(
            count.left() > header.left(),
            "the count box is at the far right of the header: {count:?}"
        );
        let handle = harness.rect_of(list_handle_id(PageKey::Shared(FONTS), "proportional.0"));
        let band = harness.rect_of(label_id(PageKey::Shared(FONTS), "proportional.0"));
        assert!(
            handle.right() <= band.left(),
            "the drag handle is the leftmost cell: handle {handle:?}, band {band:?}"
        );
        let add = harness.rect_of(list_add_id(PageKey::Shared(FONTS), "proportional"));
        let last = harness.rect_of(label_id(PageKey::Shared(FONTS), "proportional.1"));
        assert!(
            add.top() >= last.bottom(),
            "the add/remove pair sits under the rows: add {add:?}, last band {last:?}"
        );
        let style = PreferencesPanelStyle::dark();
        let frame = painted_rects(&output)
            .into_iter()
            .find(|painted| {
                painted.stroke.color == style.divider.to_egui()
                    && painted.stroke.width >= 1.0
                    && painted.rect.height() > last.height()
                    && painted.rect.width() > last.width() * 0.8
            })
            .expect("the list container frame is painted around the rows");
        assert!(
            frame.rect.top() >= header.bottom() && approx(frame.rect.bottom(), add.top()),
            "the frame ends at the last row, and the pair hangs outside it flush underneath: frame {frame:?}, add {add:?}"
        );
        assert!(
            add.bottom() > frame.rect.bottom(),
            "the pair is below the frame, not inside it: add {add:?}, frame {frame:?}"
        );
        assert!(
            handle.left() <= frame.rect.left() + 3.0,
            "the grip is at the container's left edge: handle {handle:?}, frame {frame:?}"
        );
        let field = crate::shared_style::EditorFieldStyle::dark().tokens();
        let chrome = ListStyle::dark().tokens(&field);
        let stripes: Vec<PaintedRect> = painted_rects(&output)
            .into_iter()
            .filter(|painted| {
                (painted.fill == chrome.band_fill || painted.fill == chrome.band_alt)
                    && approx(painted.rect.height(), chrome.row_height)
                    && painted.rect.width() > 400.0
            })
            .collect();
        assert!(
            stripes
                .iter()
                .any(|painted| painted.fill == chrome.band_alt),
            "the fields stripe on the element's raised band: {stripes:?}"
        );
        assert!(
            !painted_rects(&output).iter().any(|painted| {
                painted.fill == style.nav_background.to_egui()
                    && approx(painted.rect.height(), 20.0)
                    && painted.rect.width() > 400.0
            }),
            "the element body is not a second dark interior"
        );
        assert!(
            !painted_rects(&output).iter().any(|painted| {
                painted.fill == field.edge_hover
                    && approx(painted.rect.height(), 1.0)
                    && painted.rect.width() > 40.0
                    && painted.rect.width() < 500.0
            }),
            "the enum payload has no horizontal border rules"
        );
        assert!(
            !painted_rects(&output).iter().any(|painted| {
                painted.fill == field.edge_hover
                    && painted.rect.width() <= 1.5
                    && painted.rect.height() >= 10.0
            }),
            "the enum payload has no vertical border edge"
        );

        for index in 0..2 {
            assert_eq!(
                runs.iter()
                    .filter(|run| run.text == format!("Element {index}"))
                    .count(),
                1,
                "element {index} is one band: {text}"
            );
        }

        // Clicking the band selects the element; the next pass paints the blue.
        harness.click(label_id(PageKey::Shared(FONTS), "proportional.0"));
        let (output, _) = harness.frame(Vec::new());
        assert_eq!(
            harness.panel.page.selected.as_deref(),
            Some("proportional.0"),
            "the element band is selectable"
        );
        let blue = ListStyle::dark().selection.to_egui();
        assert!(
            painted_rects(&output)
                .iter()
                .any(|painted| painted.fill == blue),
            "the selected element paints the selection band"
        );

        // The caret folds the element's fields, not the list.
        let (output, actions) = harness.click(group_id(PageKey::Shared(FONTS), "proportional.0"));
        let text = painted_text(&output);
        assert!(
            actions.is_empty(),
            "folding is view state, not an action: {actions:?}"
        );
        assert!(
            harness.panel.page.collapsed.contains("proportional.0"),
            "the element folds under its own dotted path: {:?}",
            harness.panel.page.collapsed
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

    /// The list chrome is a shared page, not a group inside another panel's
    /// skin: it must be selectable in the nav and expose its own tokens.
    #[test]
    fn the_list_page_is_the_shared_list_skin() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());
        harness.click(nav_id(LIST));

        assert_eq!(harness.panel.page.key, Some(PageKey::Shared(LIST)));
        let paths: Vec<&str> = harness
            .panel
            .page
            .fields
            .iter()
            .map(|field| field.path.as_str())
            .collect();
        assert!(paths.contains(&"band_fill"), "{paths:?}");
        assert!(paths.contains(&"button_fill"), "{paths:?}");
        assert!(
            !paths.contains(&"field"),
            "the value boxes stay on their own page: {paths:?}"
        );
    }

    /// Changing a ListStyle token in the registry must repaint the page's list:
    /// the page reads Frame::list_style(), not a local constant or the panel
    /// skin.
    #[test]
    fn the_list_skin_restyles_the_preferences_list() {
        let mut harness = Harness::with_size(skins(), vec2(700.0, 470.0));
        let ink = Color::from_srgb8([0x12, 0x34, 0x56, 255]);
        assert!(
            harness
                .shared
                .set_field(LIST, "band_fill", &StyleValue::Color(ink)),
            "the list skin is registered"
        );
        harness.frame(Vec::new());
        harness.click(nav_id(FONTS));
        let (output, _) = harness.frame(Vec::new());

        assert!(
            painted_rects(&output).iter().any(|painted| {
                painted.fill == ink.to_egui()
                    && painted.rect.width() > 400.0
                    && approx(painted.rect.height(), ListStyle::dark().row_height)
            }),
            "the element band reads the shared ListStyle: {output_shapes}",
            output_shapes = painted_rects(&output).len()
        );
    }

    /// The header's two controls: `+` appends a default entry, `-` removes the
    /// selected one (or the last when nothing is selected). Both persist at
    /// once, because a list edit is a click, not a drag stream.
    #[test]
    fn the_list_header_adds_and_removes_entries() {
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, 470.0));
        harness.frame(Vec::new());
        harness.click(nav_id(FONTS));

        let (_, actions) = harness.click(list_add_id(PageKey::Shared(FONTS), "proportional"));
        assert_eq!(
            actions,
            vec![
                Action::SharedStyle(SharedStyleAction::EditList {
                    style: FONTS,
                    path: "proportional".to_owned(),
                    edit: ListEdit::Insert { index: 2 },
                }),
                Action::SharedStyle(SharedStyleAction::Commit { style: FONTS }),
            ]
        );
        assert!(
            harness.panel.page.stale,
            "the page re-reads the owner's table for the new entry"
        );

        let (_, actions) = harness.click(list_remove_id(PageKey::Shared(FONTS), "proportional"));
        assert_eq!(
            actions.first(),
            Some(&Action::SharedStyle(SharedStyleAction::EditList {
                style: FONTS,
                path: "proportional".to_owned(),
                edit: ListEdit::Remove { index: 1 },
            })),
            "with no selection, minus removes the last entry: {actions:?}"
        );
    }

    /// The count box is an integer field on the list node: scrubbing it writes
    /// the new length, which resizes the list (the same gesture as any number).
    #[test]
    fn dragging_the_entry_count_writes_the_new_length() {
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, 470.0));
        harness.frame(Vec::new());
        harness.click(nav_id(FONTS));

        let rect = harness.rect_of(field_id(PageKey::Shared(FONTS), "proportional"));
        let start = rect.center();
        let end = egui::pos2(start.x + 20.0, start.y);
        harness.frame(vec![egui::Event::PointerMoved(start)]);
        harness.frame(vec![pointer(start, true)]);
        let (_, actions) = harness.frame(vec![egui::Event::PointerMoved(end)]);

        assert_eq!(
            actions.first(),
            Some(&Action::SharedStyle(SharedStyleAction::SetField {
                style: FONTS,
                path: "proportional".to_owned(),
                value: StyleValue::ListLen(22),
            })),
            "a 20 px scrub adds 20 entries: {actions:?}"
        );
        assert!(
            harness.panel.page.stale,
            "the page re-reads the resized list"
        );

        // Releasing the scrub writes the file once, like every other drag.
        let (_, actions) = harness.frame(vec![pointer(end, false)]);
        assert_eq!(
            actions,
            vec![Action::SharedStyle(SharedStyleAction::Commit {
                style: FONTS
            })],
            "the release persists the resize: {actions:?}"
        );
    }

    /// A plain click on the handle selects the row, like Unity's mouse-down
    /// selection, and the release is not mistaken for an empty-page click.
    #[test]
    fn clicking_a_drag_handle_selects_the_element() {
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, 470.0));
        harness.frame(Vec::new());
        harness.click(nav_id(FONTS));
        harness.click(list_handle_id(PageKey::Shared(FONTS), "proportional.1"));

        assert_eq!(
            harness.panel.page.selected.as_deref(),
            Some("proportional.1"),
            "the handle selects its element on mouse-down"
        );
    }

    /// Dragging an element's handle lifts a ghost that follows the pointer; the
    /// list does not reorder until the release, which then emits one move and
    /// one commit.
    #[test]
    fn dragging_an_element_handle_lifts_a_ghost_and_moves_on_release() {
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, 470.0));
        harness.frame(Vec::new());
        harness.click(nav_id(FONTS));

        let handle = harness.rect_of(list_handle_id(PageKey::Shared(FONTS), "proportional.0"));
        let start = handle.center();
        let target = harness.rect_of(label_id(PageKey::Shared(FONTS), "proportional.1"));
        let neighbour_top = target.top();
        let end = egui::pos2(start.x, target.bottom() - 2.0);
        harness.frame(vec![egui::Event::PointerMoved(start)]);
        harness.frame(vec![pointer(start, true)]);
        let (output, actions) = harness.frame(vec![egui::Event::PointerMoved(end)]);

        assert!(
            actions.is_empty(),
            "the list stands still while the pointer is down: {actions:?}"
        );
        let runs = painted_texts(&output);
        assert_eq!(
            runs.iter().filter(|run| run.text == "Element 0").count(),
            1,
            "the held block is painted once"
        );
        assert_eq!(
            harness.panel.page.selected.as_deref(),
            Some("proportional.0"),
            "the held element is selected without moving"
        );

        // The next pass hides the held block and applies the preview slide:
        // only the ghost is painted, and the neighbour moves up into the gap.
        let (output, actions) = harness.frame(Vec::new());
        assert!(
            actions.is_empty(),
            "the preview still does not reorder: {actions:?}"
        );
        let runs = painted_texts(&output);
        assert_eq!(
            runs.iter().filter(|run| run.text == "Element 0").count(),
            1,
            "the held block is painted once, on the floating layer"
        );
        let ghost_path = runs
            .iter()
            .find(|run| run.text == "Preferences/Fonts/Inter-Regular.otf")
            .unwrap_or_else(|| {
                panic!(
                    "the ghost carries the element's body, not just its header: {}",
                    runs.iter()
                        .map(|run| run.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" | ")
                )
            });
        assert!(
            ghost_path.rect.top() > neighbour_top,
            "the ghost block's fields follow the pointer, not their old slot: {:?}",
            ghost_path.rect
        );
        let ghost_band = runs
            .iter()
            .find(|run| run.text == "Element 0")
            .expect("the ghost band label");
        assert!(
            ghost_band.rect.top() > 100.0,
            "the ghost band sits under the pointer, not at its old top: {:?}",
            ghost_band.rect
        );
        // The neighbour eases over 160 ms and `read_response` is one pass
        // behind; let the slide finish before measuring its band.
        for _ in 0..12 {
            let (_, actions) = harness.frame(Vec::new());
            assert!(actions.is_empty(), "the preview still holds: {actions:?}");
        }
        let moved = harness.rect_of(label_id(PageKey::Shared(FONTS), "proportional.1"));
        assert!(
            moved.top() < neighbour_top - 20.0,
            "the neighbour slides into the preview slot while the pointer is held: {neighbour_top} -> {}",
            moved.top()
        );

        let (_, actions) = harness.frame(vec![pointer(end, false)]);
        assert_eq!(
            actions,
            vec![
                Action::SharedStyle(SharedStyleAction::EditList {
                    style: FONTS,
                    path: "proportional".to_owned(),
                    edit: ListEdit::Move { from: 0, to: 1 },
                }),
                Action::SharedStyle(SharedStyleAction::Commit { style: FONTS }),
            ],
            "the release is the one move and the one save: {actions:?}"
        );
        assert_eq!(
            harness.panel.page.selected.as_deref(),
            Some("proportional.1"),
            "the released row stays selected in its new slot"
        );
    }

    /// Dragging upward does not swap the list under the pointer: the release
    /// moves it once, and a resting pointer afterwards emits nothing.
    #[test]
    fn dragging_up_moves_once_on_release_and_does_not_bounce() {
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, 470.0));
        harness.frame(Vec::new());
        harness.click(nav_id(FONTS));

        let handle = harness.rect_of(list_handle_id(PageKey::Shared(FONTS), "proportional.1"));
        let start = handle.center();
        let band = harness.rect_of(label_id(PageKey::Shared(FONTS), "proportional.0"));
        let end = egui::pos2(start.x, band.top() + 2.0);
        harness.frame(vec![egui::Event::PointerMoved(start)]);
        harness.frame(vec![pointer(start, true)]);
        let (_, actions) = harness.frame(vec![egui::Event::PointerMoved(end)]);
        assert!(
            actions.is_empty(),
            "the row stays in its slot until the release: {actions:?}"
        );

        let (_, actions) = harness.frame(Vec::new());
        assert!(
            actions.is_empty(),
            "resting the pointer over another slot does not reorder: {actions:?}"
        );

        let (_, actions) = harness.frame(vec![pointer(end, false)]);
        assert_eq!(
            actions,
            vec![
                Action::SharedStyle(SharedStyleAction::EditList {
                    style: FONTS,
                    path: "proportional".to_owned(),
                    edit: ListEdit::Move { from: 1, to: 0 },
                }),
                Action::SharedStyle(SharedStyleAction::Commit { style: FONTS }),
            ],
            "the release persists the one move: {actions:?}"
        );
        assert_eq!(
            harness.panel.page.selected.as_deref(),
            Some("proportional.0"),
            "the released row stays selected"
        );
    }

    /// An element that is only half inside the scroll viewport must still be
    /// dragged as one whole block. The ghost is clipped to its own rows, not to
    /// the viewport that hid the rest of the element before the drag.
    #[test]
    fn a_half_visible_element_drags_as_a_whole_block() {
        let height = 230.0;
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, height));
        harness.frame(Vec::new());
        let (before, _) = harness.click(nav_id(FONTS));

        let handle = harness.rect_of(list_handle_id(PageKey::Shared(FONTS), "proportional.1"));
        assert!(
            handle.top() < height - 20.0,
            "the handle must be reachable before the drag: {handle:?}"
        );
        assert!(
            !painted_text(&before).contains("Preferences/Fonts/Inter-Medium.otf"),
            "the half of the element below the page is not painted before the drag"
        );

        let start = handle.center();
        harness.frame(vec![egui::Event::PointerMoved(start)]);
        harness.frame(vec![pointer(start, true)]);
        let (_, _) = harness.frame(vec![egui::Event::PointerMoved(start + vec2(0.0, 10.0))]);
        let (output, _) = harness.frame(Vec::new());

        let clips = painted_clipped_texts(&output);
        let body = clips
            .iter()
            .find(|run| run.text == "Preferences/Fonts/Inter-Medium.otf")
            .unwrap_or_else(|| {
                panic!(
                    "the ghost carries the hidden half of the element: {:?}",
                    clips.iter().map(|run| &run.text).collect::<Vec<_>>()
                )
            });
        assert!(
            body.clip.contains_rect(body.rect),
            "the ghost row is still clipped to the scroll viewport: clip {:?}, row {:?}",
            body.clip,
            body.rect
        );
    }

    #[test]
    fn clicking_a_field_name_selects_it_and_outside_clicks_clear_it() {
        let field = crate::shared_style::EditorFieldStyle::dark().tokens();
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (output, _) = harness.click(label_id(panels::CONSOLE, "height"));

        assert_eq!(harness.panel.page.selected.as_deref(), Some("height"));
        let runs = painted_texts(&output);
        assert_eq!(
            run(&runs, "Height").color,
            field.label_active,
            "the selected field's name turns blue"
        );
        let box_rect = harness.rect_of(field_id(panels::CONSOLE, "height"));
        let accent = field.accent;
        assert!(
            painted_rects(&output).iter().any(|painted| {
                painted.stroke.color == accent
                    && approx(painted.rect.left(), box_rect.left())
                    && approx(painted.rect.top(), box_rect.top())
                    && approx(painted.rect.width(), box_rect.width())
            }),
            "the selected value box is outlined with the accent: {box_rect:?}"
        );

        // Clicking the page's heading (outside every row) ends the selection.
        let (_, _) = harness.click_at(egui::pos2(200.0, 40.0));
        assert_eq!(harness.panel.page.selected, None);
        // The click's release pass painted the still-selected colour; the next
        // pass is the one that shows the cleared state.
        let (output, _) = harness.frame(Vec::new());
        let runs = painted_texts(&output);
        assert_eq!(
            run(&runs, "Height").color,
            field.label,
            "the name returns to its normal colour"
        );
    }

    #[test]
    fn clicking_a_number_opens_an_editor_and_enter_commits() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        harness.click(field_id(panels::CONSOLE, "height"));
        assert!(
            harness.panel.page.edit.is_some(),
            "clicking a numeric box starts an inline edit"
        );

        // One pass draws the editor and focuses it; the next pass accepts text.
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let (_, actions) = harness.frame(vec![egui::Event::Text("30".to_owned())]);

        assert_eq!(
            harness.panel.page.fields[0].value,
            StyleValue::F32(30.0),
            "typing replaces the selected number"
        );
        assert!(
            actions.contains(&Action::Style(StyleAction::SetField {
                panel: panels::CONSOLE,
                path: "height".to_owned(),
                value: StyleValue::F32(30.0),
            })),
            "the live value follows the text: {actions:?}"
        );

        let (_, actions) = harness.frame(vec![egui::Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        assert!(harness.panel.page.edit.is_none(), "Enter ends the edit");
        assert!(
            actions.contains(&Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE,
            })),
            "the end of a typed edit writes the file: {actions:?}"
        );
    }

    /// Unity's inspector scrubs a number by dragging its name; the box itself
    /// only clicks into the inline editor.
    #[test]
    fn dragging_a_field_name_scrubs_its_value() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let rect = harness.rect_of(label_id(panels::CONSOLE, "height"));
        let start = rect.center();
        let end = egui::pos2(start.x + 20.0, start.y);
        harness.frame(vec![egui::Event::PointerMoved(start)]);
        harness.frame(vec![pointer(start, true)]);
        let (_, actions) = harness.frame(vec![egui::Event::PointerMoved(end)]);

        assert_eq!(
            actions.first(),
            Some(&Action::Style(StyleAction::SetField {
                panel: panels::CONSOLE,
                path: "height".to_owned(),
                value: StyleValue::F32(28.0),
            })),
            "20 px right on a 20 px height adds 20 * 0.4: {actions:?}"
        );
        assert_eq!(
            harness.panel.page.selected.as_deref(),
            Some("height"),
            "the scrubbed row is the selected one"
        );

        let (_, actions) = harness.frame(vec![pointer(end, false)]);
        assert!(
            actions.contains(&Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE,
            })),
            "the release persists the scrub: {actions:?}"
        );
    }

    /// The pointer says what each half of a row does before the click lands:
    /// the name is the scrub handle, the box is an editable value.
    #[test]
    fn the_name_and_the_value_box_show_their_own_cursors() {
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let value = harness.rect_of(field_id(panels::CONSOLE, "height"));
        let (output, _) = harness.frame(vec![egui::Event::PointerMoved(value.center())]);
        assert_eq!(output.platform_output.cursor_icon, egui::CursorIcon::Text);

        let name = harness.rect_of(label_id(panels::CONSOLE, "height"));
        let (output, _) = harness.frame(vec![egui::Event::PointerMoved(name.center())]);
        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::ResizeHorizontal
        );
    }

    /// The value box is Unity's inset rail: dark fill, one quiet outline, and
    /// a left-aligned value.
    #[test]
    fn value_boxes_use_the_unity_fill_and_outline() {
        let field = crate::shared_style::EditorFieldStyle::dark().tokens();
        let mut harness = Harness::new(skins());

        let (output, _) = harness.frame(Vec::new());
        let rect = harness.rect_of(field_id(panels::CONSOLE, "icons.warning"));
        let painted = painted_rects(&output);

        assert!(
            painted.iter().any(|shape| {
                shape.fill == field.fill
                    && approx(shape.rect.left(), rect.left())
                    && approx(shape.rect.top(), rect.top())
                    && approx(shape.rect.width(), rect.width())
                    && approx(shape.rect.height(), rect.height())
            }),
            "the icon-path box uses the control rail: {rect:?}\nnear: {:?}",
            painted
                .iter()
                .filter(|shape| approx(shape.rect.top(), rect.top()))
                .map(|shape| (shape.rect, shape.fill, shape.stroke))
                .collect::<Vec<_>>()
        );
        assert!(
            painted.iter().any(|shape| {
                shape.stroke.color == field.edge
                    && approx(shape.rect.width(), rect.width())
                    && approx(shape.rect.height(), rect.height())
            }),
            "the icon-path box has one quiet outline"
        );
    }

    /// The icon-path box used to run under the scroll bar. The page reserves
    /// its gutter before laying the value column out.
    #[test]
    fn the_value_column_stays_clear_of_the_scroll_bar() {
        let mut harness = Harness::new(long_skin());
        harness.frame(Vec::new());
        let (_, _) = harness.frame(Vec::new());

        let rect = harness.rect_of(field_id(panels::CONSOLE, "icons.warning"));
        let panel_right = 537.0 - 14.0;
        assert!(
            rect.right() <= panel_right - 8.0 + 0.01,
            "the value box leaves the scroll-bar gutter clear: {rect:?}"
        );
        assert!(
            rect.right() > 400.0,
            "the box is still the wide value column, not a squeezed stump: {rect:?}"
        );
    }

    /// A narrow dock still keeps the Unity two-column row; it does not stack
    /// the value under the name.
    #[test]
    fn a_narrow_page_keeps_two_columns_inside_the_panel() {
        let mut harness = Harness::with_size(skins(), vec2(460.0, 316.0));
        let (output, _) = harness.frame(Vec::new());
        let runs = painted_texts(&output);
        let height = run(&runs, "Height");
        let rect = harness.rect_of(field_id(panels::CONSOLE, "height"));

        assert!(
            height.rect.left() < rect.left(),
            "the name stays to the left of its value: {:?} vs {rect:?}",
            height.rect
        );
        assert!(
            rect.right() <= 460.0 - 14.0 - 8.0 + 0.01,
            "the value box stays inside the dock: {rect:?}"
        );
        assert!(
            rect.width() >= 100.0,
            "the label column shrank instead of the value box: {rect:?}"
        );
    }

    /// Token paths are a tree: everything before the last dot is the group.
    /// The font list's install order is the enum field's first real use: the
    /// menu shows both priorities and choosing one writes the variant.
    #[test]
    fn the_font_priority_menu_writes_the_install_order() {
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, 470.0));
        harness.frame(Vec::new());
        harness.click(nav_id(FONTS));
        let (output, _) = harness.frame(Vec::new());
        assert!(
            painted_text(&output).contains("Priority"),
            "the install order is a token now"
        );

        let field = field_id(PageKey::Shared(FONTS), "proportional.0.priority");
        harness.click(field);
        // The popup learns its size on the pass it opens; let it land.
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let item = harness.rect_of(enum_item_id(field, 1));
        let (_, actions) = harness.click_at(item.center());

        assert_eq!(
            actions,
            vec![
                Action::SharedStyle(SharedStyleAction::SetField {
                    style: FONTS,
                    path: "proportional.0.priority".to_owned(),
                    value: StyleValue::Variant("Push".to_owned()),
                }),
                Action::SharedStyle(SharedStyleAction::Commit { style: FONTS }),
            ],
            "choosing a variant is one edit and one write: {actions:?}"
        );
        let field = harness
            .panel
            .page
            .fields
            .iter()
            .find(|field| field.path == "proportional.0.priority")
            .expect("the priority row");
        assert_eq!(field.value, StyleValue::Variant("Push".to_owned()));
        assert!(
            harness.panel.page.stale,
            "the payload table is re-read: Push has rows and First has none"
        );
    }

    /// A tuple payload is transparent: the default second font entry is Push,
    /// so the page draws all four struct fields directly under the variant
    /// row, with no Test header for the unnamed field. This is the case a
    /// hand-written StyleNode used to drop.
    #[test]
    fn an_enum_payload_struct_shows_its_fields_without_a_header() {
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, 700.0));
        harness.frame(Vec::new());
        harness.click(nav_id(FONTS));
        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);

        assert!(
            !text.contains("Test"),
            "an unnamed payload has no header: {text}"
        );
        assert!(text.contains("Fvalue"), "the struct's f32 field: {text}");
        assert!(
            text.contains("Ivalue"),
            "the struct's integer field: {text}"
        );
        assert!(text.contains("Svalue"), "the struct's string field: {text}");
        assert!(text.contains("Cvalue"), "the struct's color field: {text}");

        // The tuple payload has no header and hangs under the Priority menu
        // button, exactly like the synthetic enum block below.
        let runs = painted_texts(&output);
        let fvalue = run(&runs, "Fvalue");
        let button_left = harness
            .rect_of(field_id(PageKey::Shared(FONTS), "proportional.1.priority"))
            .left();
        assert!(
            approx(fvalue.rect.left(), button_left),
            "the payload starts at the Priority button: Fvalue {:?} vs {button_left}",
            fvalue.rect
        );
        assert!(
            harness
                .rect_of(field_id(
                    PageKey::Shared(FONTS),
                    "proportional.1.priority.fvalue"
                ))
                .left()
                > button_left,
            "the payload value column is inside the button column"
        );
    }

    /// A payload hangs under the menu button, not at the row's left edge: the
    /// button is the reference, so the payload labels start at its left and
    /// the enum row keeps its own label column. It has no tint and no border of
    /// its own: it stays part of the surrounding property stripes.
    #[test]
    fn an_enum_block_hangs_its_payload_under_the_menu_button() {
        let mut harness = Harness::with_size(enum_skin("Soft"), vec2(700.0, 470.0));
        let (output, _) = harness.frame(Vec::new());
        let runs = painted_texts(&output);
        let mode = run(&runs, "Mode");
        let radius = run(&runs, "Radius");
        let samples = run(&runs, "Samples");
        let button_left = harness.rect_of(field_id(panels::CONSOLE, "mode")).left();

        assert!(
            approx(radius.rect.left(), button_left),
            "the payload starts at the menu button: Radius {:?} vs button {button_left}",
            radius.rect
        );
        assert!(
            approx(samples.rect.left(), button_left),
            "every payload label starts at the button: Samples {:?}",
            samples.rect
        );
        assert!(
            radius.rect.left() > mode.rect.left() + 1.0,
            "the payload is not back at the row's left edge: Radius {:?} vs Mode {:?}",
            radius.rect,
            mode.rect
        );

        let radius_value = harness
            .rect_of(field_id(panels::CONSOLE, "mode.radius"))
            .left();
        let samples_value = harness
            .rect_of(field_id(panels::CONSOLE, "mode.samples"))
            .left();
        assert!(
            radius_value > button_left,
            "the payload has its own value column under the button: {radius_value}"
        );
        assert!(approx(radius_value, samples_value));

        let style = PreferencesPanelStyle::dark();
        let striped = painted_rects(&output)
            .into_iter()
            .filter(|painted| {
                (painted.fill == style.row_odd.to_egui()
                    || painted.fill == style.row_even.to_egui())
                    && approx(painted.rect.height(), style.row_height)
            })
            .count();
        assert_eq!(
            striped, 3,
            "the menu row and its two payload rows stay on the property stripes"
        );
    }

    #[test]
    fn switching_an_enum_variant_re_reads_the_payload_rows() {
        let mut harness = Harness::with_size(enum_skin("Soft"), vec2(700.0, 470.0));
        harness.frame(Vec::new());

        let field = field_id(panels::CONSOLE, "mode");
        harness.click(field);
        harness.frame(Vec::new());
        harness.frame(Vec::new());
        let item = harness.rect_of(enum_item_id(field, 0));
        let (_, actions) = harness.click_at(item.center());
        assert_eq!(
            actions.first(),
            Some(&Action::Style(StyleAction::SetField {
                panel: panels::CONSOLE,
                path: "mode".to_owned(),
                value: StyleValue::Variant("Off".to_owned()),
            })),
            "choosing the unit variant writes it: {actions:?}"
        );
        assert!(harness.panel.page.stale);

        // The owner's next table has the unit variant's rows (none), and the
        // page takes it because the switch marked the snapshot stale.
        harness.skins = enum_skin("Off");
        let (output, _) = harness.frame(Vec::new());
        let text = painted_text(&output);
        assert!(text.contains("Mode"), "{text}");
        assert!(!text.contains("Radius"), "{text}");
        assert!(!text.contains("Samples"), "{text}");
    }

    #[test]
    fn token_paths_split_into_groups() {
        assert_eq!(group_path("icons.console.info"), "icons.console");
        assert_eq!(group_path("height"), "");
    }
}
