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
//! the separate SharedStyleAction path, including an explicit Reload.
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
//! or closing this panel. The legacy window instead pushed the whole page back
//! every frame and let the owning window write its TOML, about 60 times a
//! second.

pub mod style;

use kairos_editor_widgets::color::ColorExt;
use std::collections::HashSet;

use egui::{
    Align, Context, CornerRadius, CursorIcon, Id, Key, Margin, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Ui, UiBuilder, WidgetText, WidgetType, pos2, vec2,
};
use kairos_color::ColorUsage;
use kairos_editor_core::{
    Action, LayoutAction, ListEdit, PanelId, SharedStyleAction, SharedStyleId, StyleAction,
    StyleValue,
};
use kairos_editor_widgets::button::Button;
use kairos_editor_widgets::color_picker::{
    ColorField, ColorPickerStyle, ColorProperty, ColorSwatches, any_color_picker_open,
    close_all_color_pickers, eyedropper, is_color_picker_open,
};
use kairos_editor_widgets::divider::Divider;
use kairos_editor_widgets::field::{
    FieldOutcome, FieldStyle, NumberEdit, NumberField, TextField, ToggleField,
};
use kairos_editor_widgets::paint::labelled_widget_info;
use kairos_editor_widgets::style::StyleFile;
use kairos_editor_widgets::text::TextStyle;

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;
use crate::shared_style::SharedSkin;
use crate::skin::{LIST_MAX, StyleField, StyleKind, humanize};

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

fn close_id() -> Id {
    Id::new("kairos.preferences.close")
}

fn reload_id(style: SharedStyleId) -> Id {
    Id::new(("kairos.preferences.reload", style))
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
    /// Shared-style revision this snapshot came from. A reload bumps the
    /// registry revision, and the next pass re-reads the field table.
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
    /// A structural list edit was pushed. The page's snapshot is otherwise
    /// re-read only on a page change or a reload, so a new entry's rows would
    /// not exist until the user left the page: the next pass re-reads it.
    stale: bool,
    /// The list element being dragged for reordering, if any.
    list_drag: Option<ListDrag>,
}

/// The list element a drag started on.
///
/// The target slot is recomputed every pass from the element bands the page
/// just laid out, so only the origin has to survive between passes.
#[derive(Clone, Debug)]
struct ListDrag {
    /// The list token's path (`"proportional"`).
    list: String,
    /// Index of the element the drag started on.
    from: usize,
}

/// One element band this pass laid out, so the drag can resolve the slot the
/// pointer is over without keeping the page's geometry between passes.
struct ElementBand {
    list: String,
    index: usize,
    rect: Rect,
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
                self.list_drag = None;
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
        self.list_drag = None;
        self.stale = false;
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
    /// Shared swatch groups. Loaded from Library/ColorSwatches.ron in init and
    /// saved by the picker whenever a group or swatch changes.
    swatches: ColorSwatches,
}

impl PreferencesPanel {
    /// Build the panel. The catalog factory above is the only caller.
    pub fn new() -> Self {
        Self {
            style: PreferencesPanelStyle::default(),
            selected: None,
            page: PageState::default(),
            close_popups: false,
            swatches: ColorSwatches::default(),
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
        match ColorSwatches::load("Library/ColorSwatches.ron") {
            Ok(book) => self.swatches = book,
            Err(error) => {
                kairos_log::warn!(target: "kairos_editor", "color swatches: {error}");
                self.swatches.set_path("Library/ColorSwatches.ron");
            }
        }
    }

    fn ui(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut) {
        let Self {
            style,
            selected,
            page,
            close_popups,
            swatches,
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
                    page_body(ui, style, &picker_style, page, swatches, out);
                });

                footer(ui, style, footer_rect, page.key, out);
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
        if let Err(error) = self.swatches.save_if_dirty() {
            kairos_log::warn!(target: "kairos_editor", "color swatches: {error}");
        }
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
fn page_body(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    picker_style: &ColorPickerStyle,
    page: &mut PageState,
    swatches: &mut ColorSwatches,
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
                field: style.field_style(),
                key: page_key.expect("a page body is only drawn for a selected page"),
                picker: picker_style,
                page,
                swatches,
                out,
                clicked: false,
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
    /// The value-box tokens, projected once for the whole page.
    field: FieldStyle,
    key: PageKey,
    picker: &'a ColorPickerStyle,
    page: &'a mut PageState,
    swatches: &'a mut ColorSwatches,
    out: &'a mut UiOut,
    /// A row, a field name or a foldout consumed this pass's click. Without
    /// one, a click elsewhere on the page ends the current selection.
    clicked: bool,
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
        // Every element band this pass, in draw order: the reorder target is a
        // slot between them, so it can only be resolved once they all exist.
        let mut bands: Vec<ElementBand> = Vec::new();

        // A `Vec<T>` token is a list, empty or not: the list node is the only
        // field an empty list has, and it is what gives the empty list a
        // header to add the first entry from.
        let lists: Vec<(String, u32)> = self
            .page
            .fields
            .iter()
            .filter(|field| field.kind == StyleKind::List)
            .map(|field| {
                let len = match field.value {
                    StyleValue::Integer(len) => len,
                    _ => 0,
                };
                (field.path.clone(), len)
            })
            .collect();

        for index in 0..self.page.fields.len() {
            let (path, kind) = {
                let field = &self.page.fields[index];
                (field.path.clone(), field.kind)
            };

            // A list node draws its own two-row header (name, then count and
            // add/remove) instead of a row in the value column.
            if kind == StyleKind::List {
                let segments: Vec<&str> = path.split('.').filter(|part| !part.is_empty()).collect();
                let depth = segments.len().saturating_sub(1);
                let common = open
                    .iter()
                    .zip(segments.iter())
                    .take_while(|(opened, segment)| opened.as_str() == **segment)
                    .count();
                open.truncate(common);

                for level in common..depth {
                    // Open a group only when nothing above it is folded; the
                    // stack is this band's ancestors, so ask before pushing it.
                    let hidden = collapsed_above(&segments[..level], &self.page.collapsed);
                    open.push(segments[level].to_owned());
                    if !hidden {
                        let band = open.join(".");
                        self.band(
                            ui,
                            &band,
                            level,
                            humanize(segments[level]),
                            row_width,
                            &mut row_number,
                            &lists,
                            &mut bands,
                        );
                    }
                }

                // A folded group hides the list's entries, not its header.
                let hidden = collapsed_above(&segments[..depth], &self.page.collapsed);
                open.push(segments[depth].to_owned());
                if !hidden {
                    self.list_header(ui, index, depth, row_width);
                }
                if style.row_gap > 0.0 {
                    ui.add_space(style.row_gap);
                }
                continue;
            }

            let group = group_path(&path);
            let segments: Vec<&str> = group.split('.').filter(|part| !part.is_empty()).collect();

            // Close the groups this field is no longer in, then open the ones
            // it is. Fields are in declaration order, so the group runs stay
            // contiguous.
            let common = open
                .iter()
                .zip(segments.iter())
                .take_while(|(opened, segment)| opened.as_str() == **segment)
                .count();
            open.truncate(common);

            for depth in common..segments.len() {
                // Open a group only when nothing above it is folded; the stack
                // is this row's ancestors, so ask before pushing the group.
                let hidden = collapsed_above(&segments[..depth], &self.page.collapsed);
                open.push(segments[depth].to_owned());
                if !hidden {
                    let band = open.join(".");
                    self.band(
                        ui,
                        &band,
                        depth,
                        humanize(segments[depth]),
                        row_width,
                        &mut row_number,
                        &lists,
                        &mut bands,
                    );
                }
            }

            // A folded group hides its rows, not its own band.
            if !collapsed_above(&segments, &self.page.collapsed) {
                // A field whose group path is exactly a list element sits in
                // that element's block: it keeps the handle gutter's divider
                // and is not indented a second time.
                let element = segments.len() >= 2
                    && lists
                        .iter()
                        .any(|(list, _)| list == &segments[..segments.len() - 1].join("."));
                self.row(ui, index, segments.len(), row_width, row_number, element);
                row_number += 1;
                if style.row_gap > 0.0 {
                    ui.add_space(style.row_gap);
                }
            }
        }

        self.finish_list_drag(ui, &bands);

        // A click in the empty page (or on its heading) ends the selection;
        // rows, names and foldouts record their own clicks.
        if ui.input(|input| input.pointer.any_click()) && !self.clicked {
            self.page.selected = None;
            self.page.edit = None;
        }
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
        lists: &[(String, u32)],
        bands: &mut Vec<ElementBand>,
    ) {
        let parent = path.rsplit_once('.').map_or("", |(parent, _)| parent);
        if lists.iter().any(|(list, _)| list == parent) {
            let index = path
                .rsplit('.')
                .next()
                .and_then(|part| part.parse::<usize>().ok())
                .unwrap_or(0);
            self.element_band(ui, path, index, depth, row_width, *row_number, bands);
            *row_number += 1;
        } else {
            self.foldout(ui, path, depth, label, row_width);
        }
    }

    /// A list's two-row header: the foldout band, then the count and the
    /// add/remove pair in the block's bottom-right corner.
    fn list_header(&mut self, ui: &mut Ui, index: usize, depth: usize, row_width: f32) {
        let style = self.style;
        let (path, name, len) = {
            let field = &self.page.fields[index];
            let len = match field.value {
                StyleValue::Integer(len) => len,
                _ => 0,
            };
            (field.path.clone(), field.name.clone(), len)
        };
        let collapsed = self.page.collapsed.contains(&path);

        // Row 1: the band. Clicking it folds the list, like any group band.
        let (rect, _) = ui.allocate_exact_size(vec2(row_width, style.group_height), Sense::hover());
        let response = ui
            .interact(rect, group_id(self.key, &path), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
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
        let galley = text.layout(ui, &name);
        painter.with_clip_rect(rect).galley(
            pos2(text_left, rect.center().y - galley.size().y * 0.5),
            galley,
            style.group.to_egui(),
        );
        labelled_widget_info(
            &response,
            WidgetType::SelectableLabel,
            &name,
            Some(!collapsed),
        );
        if response.clicked() {
            if collapsed {
                self.page.collapsed.remove(&path);
            } else {
                self.page.collapsed.insert(path.clone());
            }
            self.clicked = true;
        }

        // Row 2: the size and its controls.
        let (rect, _) = ui.allocate_exact_size(vec2(row_width, style.row_height), Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, style.group_background.to_egui());
        let cell = style.list_button_cell_width;
        let buttons = Rect::from_min_size(
            pos2(
                rect.right() - style.row_inset - cell * 2.0,
                rect.center().y - style.list_button_height * 0.5,
            ),
            vec2(cell * 2.0, style.list_button_height),
        );
        let count_rect = Rect::from_min_size(
            pos2(
                buttons.left() - 4.0 - style.list_count_width,
                rect.center().y - style.field_height * 0.5,
            ),
            vec2(style.list_count_width, style.field_height),
        );

        // The count is a normal integer field: drag to scrub, click to type.
        // Writing it resizes the list (growing appends default entries).
        let mut count = StyleField {
            path: path.clone(),
            name: name.clone(),
            value: StyleValue::Integer(len),
            kind: StyleKind::Integer { max: LIST_MAX },
        };
        let outcome = draw_value(
            ui,
            &self.field,
            self.key,
            self.picker,
            &mut count,
            &mut self.page.selected,
            &mut self.page.edit,
            self.swatches,
            &mut self.clicked,
            count_rect,
        );
        if outcome.changed {
            self.out
                .push(set_field_action(self.key, &path, &count.value));
            self.page.fields[index].value = count.value.clone();
            self.page.dirty = true;
            self.page.stale = true;
        }
        if outcome.finished_edit && self.page.dirty {
            self.page.commit(self.out);
        }

        if let Some(edit) = self.list_buttons(ui, &path, len, buttons) {
            match edit {
                // The new entry becomes the selection, so the next click of
                // ~~-~~ removes what the user just added, not the old last row.
                ListEdit::Insert { index } => {
                    self.page.selected = Some(format!("{path}.{index}"));
                }
                ListEdit::Remove { .. } => self.page.selected = None,
                ListEdit::Move { .. } => {}
            }
            self.push_list_edit(&path, edit);
        }
    }

    /// The add/remove pair, drawn as one two-cell control. Returns the edit a
    /// click asked for; the caller applies and persists it.
    fn list_buttons(&mut self, ui: &mut Ui, path: &str, len: u32, rect: Rect) -> Option<ListEdit> {
        let style = self.style;
        let cell = rect.width() * 0.5;
        let add_rect = Rect::from_min_size(rect.min, vec2(cell, rect.height()));
        let remove_rect = Rect::from_min_size(
            pos2(rect.left() + cell, rect.top()),
            vec2(cell, rect.height()),
        );
        let remove_enabled = len > 0;

        let add = ui
            .interact(add_rect, list_add_id(self.key, path), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        let remove = ui.interact(
            remove_rect,
            list_remove_id(self.key, path),
            if remove_enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );

        let painter = ui.painter();
        let corner = CornerRadius::same(style.button_corner_radius.round() as u8);
        painter.rect_filled(rect, corner, style.button_fill.to_egui());
        painter.rect_stroke(
            rect,
            corner,
            Stroke::new(style.button_border_width, style.button_border.to_egui()),
            StrokeKind::Inside,
        );
        painter.rect_filled(
            Rect::from_min_size(
                pos2(rect.center().x, rect.top() + 1.0),
                vec2(1.0, rect.height() - 2.0),
            ),
            CornerRadius::ZERO,
            style.button_border.to_egui(),
        );
        for (response, cell_rect, enabled) in [
            (&add, add_rect, true),
            (&remove, remove_rect, remove_enabled),
        ] {
            if enabled && response.hovered() {
                painter.rect_filled(cell_rect.shrink(1.0), corner, style.button_hover.to_egui());
            }
        }

        let ink = style.button_text.to_egui();
        let dim = style.path.to_egui();
        let half = 4.0;
        let plus = add_rect.center();
        painter.line_segment(
            [pos2(plus.x - half, plus.y), pos2(plus.x + half, plus.y)],
            Stroke::new(1.0, ink),
        );
        painter.line_segment(
            [pos2(plus.x, plus.y - half), pos2(plus.x, plus.y + half)],
            Stroke::new(1.0, ink),
        );
        let minus = remove_rect.center();
        painter.line_segment(
            [pos2(minus.x - half, minus.y), pos2(minus.x + half, minus.y)],
            Stroke::new(1.0, if remove_enabled { ink } else { dim }),
        );
        labelled_widget_info(&add, WidgetType::Button, "Add list entry", None);
        labelled_widget_info(&remove, WidgetType::Button, "Remove list entry", None);

        if add.clicked() {
            self.clicked = true;
            return Some(ListEdit::Insert {
                index: len as usize,
            });
        }
        if remove.clicked() && remove_enabled {
            self.clicked = true;
            if let Some(index) = self.remove_index(path, len) {
                return Some(ListEdit::Remove { index });
            }
        }
        None
    }

    /// One list element: drag handle, foldout caret, `Element N`. Its fields
    /// follow as rows in the same block.
    #[allow(clippy::too_many_arguments)]
    fn element_band(
        &mut self,
        ui: &mut Ui,
        path: &str,
        index: usize,
        depth: usize,
        row_width: f32,
        row_number: usize,
        bands: &mut Vec<ElementBand>,
    ) {
        let style = self.style;
        let (rect, _) = ui.allocate_exact_size(vec2(row_width, style.row_height), Sense::hover());
        let selected = self.page.selected.as_deref() == Some(path);
        let fill = if selected {
            style.list_selection
        } else {
            style.row_fill(row_number)
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, fill.to_egui());

        let x0 = rect.left() + style.row_inset + depth as f32 * style.group_indent;
        let divider_x = x0 + style.list_gutter_width;
        let content_left = divider_x + 4.0;
        let label_x = content_left + 16.0;
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(divider_x, rect.top()), vec2(1.0, rect.height())),
            CornerRadius::ZERO,
            style.divider.to_egui(),
        );

        // The handle: two bars, with the whole row height as its target so a
        // drag can start on the row and not only on the 10x6 glyph.
        let handle = Rect::from_center_size(
            pos2(x0 + style.list_gutter_width * 0.5, rect.center().y),
            vec2(style.list_gutter_width, rect.height()),
        );
        let drag = ui
            .interact(handle, list_handle_id(self.key, path), Sense::drag())
            .on_hover_cursor(CursorIcon::Grab);
        let ink = if drag.hovered() || selected {
            style.label.to_egui()
        } else {
            style.path.to_egui()
        };
        for offset in [-2.0, 2.0] {
            ui.painter().rect_filled(
                Rect::from_min_size(
                    pos2(handle.center().x - 5.0, handle.center().y + offset - 0.5),
                    vec2(10.0, 1.0),
                ),
                CornerRadius::ZERO,
                ink,
            );
        }
        let list = path
            .rsplit_once('.')
            .map_or(String::new(), |(list, _)| list.to_owned());
        if drag.drag_started() {
            self.page.list_drag = Some(ListDrag {
                list: list.clone(),
                from: index,
            });
        }
        bands.push(ElementBand { list, index, rect });

        // Caret: folds this element's fields, not the list.
        let collapsed = self.page.collapsed.contains(path);
        let caret_rect =
            Rect::from_min_size(pos2(content_left, rect.top()), vec2(16.0, rect.height()));
        let caret = ui
            .interact(caret_rect, group_id(self.key, path), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        paint_foldout_arrow(
            ui.painter(),
            pos2(content_left + 4.0, rect.center().y),
            collapsed,
            style.group.to_egui(),
        );
        if caret.clicked() {
            if collapsed {
                self.page.collapsed.remove(path);
            } else {
                self.page.collapsed.insert(path.to_owned());
            }
            self.clicked = true;
        }

        // The rest of the row selects the element.
        let label_rect =
            Rect::from_min_max(pos2(label_x, rect.top()), pos2(rect.right(), rect.bottom()));
        let select = ui.interact(label_rect, label_id(self.key, path), Sense::click());
        let color = if selected { style.title } else { style.label }.to_egui();
        let text = TextStyle::sized(color, style.body_size);
        let galley = text.layout(ui, format!("Element {index}"));
        ui.painter().with_clip_rect(label_rect).galley(
            pos2(label_x, rect.center().y - galley.size().y * 0.5),
            galley,
            color,
        );
        if select.clicked() {
            self.page.selected = Some(path.to_owned());
            self.clicked = true;
        }
    }

    /// Ask the owner to apply a structural list edit, then persist it: a list
    /// edit is a discrete click, not a drag stream, so live value and file go
    /// together.
    fn push_list_edit(&mut self, path: &str, edit: ListEdit) {
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

    /// The selected element of `list`, or its last one.
    fn remove_index(&self, list: &str, len: u32) -> Option<usize> {
        if len == 0 {
            return None;
        }
        let last = len as usize - 1;
        let Some(selected) = self.page.selected.as_deref() else {
            return Some(last);
        };
        let Some(rest) = selected
            .strip_prefix(list)
            .and_then(|rest| rest.strip_prefix('.'))
        else {
            return Some(last);
        };
        match rest
            .split('.')
            .next()
            .and_then(|part| part.parse::<usize>().ok())
        {
            Some(index) if index <= last => Some(index),
            _ => Some(last),
        }
    }

    /// Resolve the reorder target from this pass's element bands and, on
    /// release, move the element. The insertion line is painted while dragging.
    fn finish_list_drag(&mut self, ui: &mut Ui, bands: &[ElementBand]) {
        let Some(drag) = self.page.list_drag.clone() else {
            return;
        };
        // A drag whose button is already up but never produced a release (the
        // pointer left the window) has no slot to drop on: drop the drag.
        if !ui.input(|input| input.pointer.any_released())
            && !ui.input(|input| input.pointer.primary_down())
        {
            self.page.list_drag = None;
            return;
        }
        let mut list_bands: Vec<&ElementBand> =
            bands.iter().filter(|band| band.list == drag.list).collect();
        list_bands.sort_by_key(|band| band.index);
        let Some(first) = list_bands.first().copied() else {
            self.page.list_drag = None;
            return;
        };
        let Some(pointer) = ui.ctx().pointer_interact_pos() else {
            return;
        };
        let slot = list_bands
            .iter()
            .take_while(|band| pointer.y > band.rect.center().y)
            .count();
        let y = if slot == 0 {
            first.rect.top()
        } else {
            list_bands[slot - 1].rect.bottom()
        };
        let style = self.style;
        let left = first.rect.left() + style.row_inset;
        let right = first.rect.right() - style.row_inset;
        ui.painter().rect_filled(
            Rect::from_min_size(pos2(left, y - 1.0), vec2(right - left, 2.0)),
            CornerRadius::ZERO,
            style.accent.to_egui(),
        );

        if ui.input(|input| input.pointer.any_released()) {
            let to = if slot > drag.from { slot - 1 } else { slot };
            self.page.list_drag = None;
            if to != drag.from && to < list_bands.len() {
                self.push_list_edit(
                    &drag.list,
                    ListEdit::Move {
                        from: drag.from,
                        to,
                    },
                );
                self.page.selected = Some(format!("{}.{}", drag.list, to));
            }
        }
    }

    /// One foldout band: triangle, left-aligned group name, full-width hit
    /// area. Unity calls this a foldout; the RON calls it a nested struct.
    fn foldout(&mut self, ui: &mut Ui, path: &str, depth: usize, label: String, row_width: f32) {
        let style = self.style;
        let (rect, _) = ui.allocate_exact_size(vec2(row_width, style.group_height), Sense::hover());
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
        let galley = text.layout(ui, &label);
        let text_pos = pos2(text_left, rect.center().y - galley.size().y * 0.5);
        let color = style.group.to_egui();
        painter.with_clip_rect(rect).galley(text_pos, galley, color);

        labelled_widget_info(
            &response,
            WidgetType::SelectableLabel,
            &label,
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

    /// One leaf row: alternating band, left name, value box.
    ///
    /// `in_element` marks a row that sits directly inside a list element: it
    /// keeps the element block's handle gutter (the divider runs through it)
    /// and starts its name where the element's own label starts, so the block
    /// does not indent its fields twice.
    #[allow(clippy::too_many_arguments)]
    fn row(
        &mut self,
        ui: &mut Ui,
        index: usize,
        depth: usize,
        row_width: f32,
        row_number: usize,
        in_element: bool,
    ) {
        let style = self.style;
        let key = self.key;
        let (rect, _) = ui.allocate_exact_size(vec2(row_width, style.row_height), Sense::hover());
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            style.row_fill(row_number).to_egui(),
        );

        let path = self.page.fields[index].path.clone();
        let name = self.page.fields[index].name.clone();
        let kind = self.page.fields[index].kind;

        // Unity's layout: one label column, then a value column that fills the
        // rest of the row. The label column shrinks before the value box would
        // run under the scroll bar.
        let content_left = rect.left() + style.row_inset;
        let content_right = (rect.right() - style.row_inset).max(content_left + 1.0);
        let content_width = content_right - content_left;
        let max_label = (content_width - style.label_gap - style.field_width).max(48.0);
        let label_width = style
            .label_width
            .min(max_label)
            .min((content_width - style.label_gap - 1.0).max(0.0));
        let value_left = content_left + label_width + style.label_gap;
        let value_width = (content_right - value_left).max(1.0);
        let value_rect = Rect::from_min_size(
            pos2(value_left, rect.center().y - style.field_height * 0.5),
            vec2(value_width, style.field_height),
        );
        let label_rect = Rect::from_min_max(
            pos2(content_left, rect.top()),
            pos2(value_left - style.label_gap, rect.bottom()),
        );
        let (label_x, divider_x) = if in_element {
            let x0 =
                rect.left() + style.row_inset + depth.saturating_sub(1) as f32 * style.group_indent;
            let divider = x0 + style.list_gutter_width;
            (divider + 4.0 + 16.0, Some(divider))
        } else {
            (
                content_left + 16.0 + depth as f32 * style.group_indent,
                None,
            )
        };
        if let Some(x) = divider_x {
            ui.painter().rect_filled(
                Rect::from_min_size(pos2(x, rect.top()), vec2(1.0, rect.height())),
                CornerRadius::ZERO,
                style.divider.to_egui(),
            );
        }

        // The name is an interaction, not just paint: clicking it selects the
        // field, and a text field takes keyboard focus so typing starts.
        let label_response = ui.interact(label_rect, label_id(key, &path), Sense::click());
        if label_response.clicked() {
            self.page.selected = Some(path.clone());
            self.clicked = true;
            if matches!(kind, StyleKind::Text) {
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
            &self.field,
            key,
            self.picker,
            field,
            selected,
            edit,
            self.swatches,
            &mut self.clicked,
            value_rect,
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
        let color = style.label_color(active).to_egui();
        let text = TextStyle::sized(color, style.body_size);
        let galley = text.layout(ui, &name);
        let text_pos = pos2(label_x, rect.center().y - galley.size().y * 0.5);
        ui.painter()
            .with_clip_rect(label_rect)
            .galley(text_pos, galley, color);

        let _ = label_response.on_hover_text(path);
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
    swatches: &mut ColorSwatches,
    clicked: &mut bool,
    rect: Rect,
) -> ValueOutcome {
    let path = field.path.clone();
    let id = field_id(key, &path);
    let editing = edit.as_ref().is_some_and(|current| current.path == path);
    let active = selected.as_deref() == Some(path.as_str())
        || editing
        || ui.memory(|memory| memory.has_focus(id));

    match field.kind {
        StyleKind::Float => {
            let StyleValue::Float(value) = &mut field.value else {
                return ValueOutcome::default();
            };
            let mut number = take_number(edit, &path);
            let outcome = NumberField::float(id, value, field_style)
                .selected(active)
                .label(&field.name)
                .show(ui, rect, &mut number);
            keep_number(edit, &path, number);
            select_row(
                outcome.clicked() || outcome.response.dragged(),
                &path,
                selected,
                clicked,
            );
            finished(outcome)
        }
        StyleKind::Integer { max } => {
            let StyleValue::Integer(value) = &mut field.value else {
                return ValueOutcome::default();
            };
            let mut number = take_number(edit, &path);
            let outcome = NumberField::integer(id, value, max, field_style)
                .selected(active)
                .label(&field.name)
                .show(ui, rect, &mut number);
            keep_number(edit, &path, number);
            select_row(
                outcome.clicked() || outcome.response.dragged(),
                &path,
                selected,
                clicked,
            );
            finished(outcome)
        }
        StyleKind::Text => {
            let StyleValue::Text(value) = &mut field.value else {
                return ValueOutcome::default();
            };
            let outcome = TextField::new(id, value, field_style)
                .selected(active)
                .label(&field.name)
                .show(ui, rect);
            if outcome.clicked() || outcome.response.has_focus() {
                *selected = Some(path);
                if outcome.clicked() {
                    *clicked = true;
                }
            }
            finished(outcome)
        }
        StyleKind::Toggle => {
            let StyleValue::Toggle(on) = &mut field.value else {
                return ValueOutcome::default();
            };
            let outcome = ToggleField::new(toggle_id(key, &path), on, field_style)
                .selected(active)
                .label(&field.name)
                .show(ui, rect);
            select_row(outcome.clicked(), &path, selected, clicked);
            finished(outcome)
        }
        StyleKind::Color | StyleKind::HdrColor => draw_color(
            ui, picker, field, selected, clicked, swatches, rect, id, &path, active,
        ),
        // A list draws its own header: the count box is an integer field the
        // header builds, so a list node never reaches a row's value column.
        StyleKind::List => ValueOutcome::default(),
    }
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

/// A click (or a scrub) on a value selects its row, so the name turns blue and
/// the box keeps the accent edge.
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
/// built-in one; ColorField is already a widget, so this is only the mapping
/// between a StyleValue and a ColorProperty.
#[allow(clippy::too_many_arguments)]
fn draw_color(
    ui: &mut Ui,
    picker_style: &ColorPickerStyle,
    field: &mut StyleField,
    selected: &mut Option<String>,
    clicked: &mut bool,
    swatches: &mut ColorSwatches,
    rect: Rect,
    id: Id,
    path: &str,
    active: bool,
) -> ValueOutcome {
    let usage = match field.kind {
        StyleKind::HdrColor => ColorUsage::HDR,
        _ => ColorUsage::LDR,
    };
    let Some(mut color) = color_property(&field.value, usage) else {
        return ValueOutcome::default();
    };

    let response = ui.put(
        rect,
        ColorField::new(id, &mut color, picker_style)
            .selected(active)
            .height(rect.height())
            .swatches(swatches),
    );
    // A pick's confirming click never reaches egui (the host masks it), so
    // the eyedropper reports its own end of gesture; without that the page
    // would keep the committed colour only until the next pointer release.
    let committed = eyedropper::take_committed(ui.ctx());

    let mut changed = false;
    if response.changed() {
        field.value = StyleValue::Color(color.color);
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

fn color_property(value: &StyleValue, usage: ColorUsage) -> Option<ColorProperty> {
    match value {
        StyleValue::Color(color) => Some(ColorProperty::new(*color, usage)),
        _ => None,
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

/// Close, right-aligned, plus Reload on a shared page. Every edit is already
/// written when the interaction ends, so there is nothing to apply or cancel
/// here; Reload is the explicit "the file changed, read it again" path.
fn footer(
    ui: &mut Ui,
    style: &PreferencesPanelStyle,
    rect: Rect,
    page: Option<PageKey>,
    out: &mut UiOut,
) {
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(Button::new("Close", &style.button_style()).id(close_id()))
                .clicked()
            {
                out.push(Action::Layout(LayoutAction::ClosePanel {
                    panel: PREFERENCES,
                }));
            }

            if let Some(PageKey::Shared(shared)) = page
                && ui
                    .add(Button::new("Reload", &style.button_style()).id(reload_id(shared)))
                    .clicked()
            {
                // The open picker was drawn from the old file; close it
                // rather than leave a stale window over the refreshed page.
                egui::Popup::close_all(ui.ctx());
                close_all_color_pickers(ui.ctx());
                out.push(Action::SharedStyle(SharedStyleAction::Reload {
                    style: shared,
                }));
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
    use kairos_editor_widgets::style::Style;

    use crate::shared_style::{COLOR_PICKER, FONTS};
    use crate::skin::PanelSkin;

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
            StyleField {
                path: "icons.error".to_owned(),
                name: "Error".to_owned(),
                value: StyleValue::Text("error.png".to_owned()),
                kind: StyleKind::Text,
            },
            StyleField {
                path: "icons.textures.tint".to_owned(),
                name: "Tint".to_owned(),
                value: StyleValue::Text("tint.png".to_owned()),
                kind: StyleKind::Text,
            },
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
            StyleField {
                path: "background".to_owned(),
                name: "Background".to_owned(),
                value: StyleValue::Color(Color::from_srgb8([20, 140, 220, 255])),
                kind: StyleKind::Color,
            },
            StyleField {
                path: "glow".to_owned(),
                name: "Glow".to_owned(),
                value: StyleValue::Color(Color::new(2.0, 1.0, 0.5, 1.0)),
                kind: StyleKind::HdrColor,
            },
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
            fields.push(StyleField {
                path: format!("token_{index}"),
                name: format!("Token {index}"),
                value: StyleValue::Float(index as f32),
                kind: StyleKind::Float,
            });
        }
        vec![PanelSkin {
            panel: panels::CONSOLE,
            title: "Console".to_owned(),
            file: "ConsoleStyle.ron",
            fields,
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

        let value = StyleValue::Float(0.9);
        assert_eq!(
            set_field_action(PageKey::Shared(COLOR_PICKER), "border_width", &value),
            Action::SharedStyle(SharedStyleAction::SetField {
                style: COLOR_PICKER,
                path: "border_width".to_owned(),
                value,
            })
        );
    }

    /// Reload is the explicit refresh path for a shared page whose file was
    /// edited outside the editor.
    #[test]
    fn a_shared_page_reload_button_asks_the_registry_to_re_read_the_file() {
        let mut harness = Harness::new(Vec::new());
        harness.frame(Vec::new());

        let (_, actions) = harness.click(reload_id(COLOR_PICKER));

        assert_eq!(
            actions,
            vec![Action::SharedStyle(SharedStyleAction::Reload {
                style: COLOR_PICKER
            })]
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
        let blue = PreferencesPanelStyle::dark().list_selection.to_egui();
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
                value: StyleValue::Integer(22),
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

    /// Dragging an element's handle reorders it: the drop slot is resolved
    /// from the element bands the page laid out, and the move is persisted at
    /// once (a drop is a click, not a stream).
    #[test]
    fn dragging_an_element_handle_moves_it() {
        let mut harness = Harness::with_size(Vec::new(), vec2(700.0, 470.0));
        harness.frame(Vec::new());
        harness.click(nav_id(FONTS));

        let handle = harness.rect_of(list_handle_id(PageKey::Shared(FONTS), "proportional.0"));
        let start = handle.center();
        let end = egui::pos2(start.x, start.y + 70.0);
        harness.frame(vec![egui::Event::PointerMoved(start)]);
        harness.frame(vec![pointer(start, true)]);
        harness.frame(vec![egui::Event::PointerMoved(end)]);
        let (_, actions) = harness.frame(vec![pointer(end, false)]);

        assert_eq!(
            actions.first(),
            Some(&Action::SharedStyle(SharedStyleAction::EditList {
                style: FONTS,
                path: "proportional".to_owned(),
                edit: ListEdit::Move { from: 0, to: 1 },
            })),
            "the drag drops element 0 below element 1: {actions:?}"
        );
        assert!(
            actions.contains(&Action::SharedStyle(SharedStyleAction::Commit {
                style: FONTS
            })),
            "a drop is persisted with the move: {actions:?}"
        );
    }

    #[test]
    fn clicking_a_field_name_selects_it_and_outside_clicks_clear_it() {
        let style = PreferencesPanelStyle::dark();
        let mut harness = Harness::new(skins());
        harness.frame(Vec::new());

        let (output, _) = harness.click(label_id(panels::CONSOLE, "height"));

        assert_eq!(harness.panel.page.selected.as_deref(), Some("height"));
        let runs = painted_texts(&output);
        assert_eq!(
            run(&runs, "Height").color,
            style.label_active.to_egui(),
            "the selected field's name turns blue"
        );
        let box_rect = harness.rect_of(field_id(panels::CONSOLE, "height"));
        let accent = style.accent.to_egui();
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
            style.label.to_egui(),
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
            StyleValue::Float(30.0),
            "typing replaces the selected number"
        );
        assert!(
            actions.contains(&Action::Style(StyleAction::SetField {
                panel: panels::CONSOLE,
                path: "height".to_owned(),
                value: StyleValue::Float(30.0),
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

    /// The value box is Unity's inset rail: dark fill, one quiet outline, and
    /// a left-aligned value.
    #[test]
    fn value_boxes_use_the_unity_fill_and_outline() {
        let style = PreferencesPanelStyle::dark();
        let mut harness = Harness::new(skins());

        let (output, _) = harness.frame(Vec::new());
        let rect = harness.rect_of(field_id(panels::CONSOLE, "icons.warning"));
        let painted = painted_rects(&output);

        assert!(
            painted.iter().any(|shape| {
                shape.fill == style.control_rail.to_egui()
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
                shape.stroke.color == style.control_edge.to_egui()
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
    #[test]
    fn token_paths_split_into_groups() {
        assert_eq!(group_path("icons.console.info"), "icons.console");
        assert_eq!(group_path("height"), "");
    }
}
