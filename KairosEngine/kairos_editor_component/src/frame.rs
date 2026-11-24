//! Frame + UiOut: the two types the UI pass owns.
//!
//! A [`Frame`] is an immutable view of the state tree plus the host resources the
//! UI may display but never owns. [`UiOut`] is the pass's action buffer: panels
//! only push intents into it, and the app drains it after the last pass and
//! applies every action through `kairos_editor_core::reduce`.
//!
//! Keeping these here (not in core, not in widgets) is the layering point:
//! core must not know egui, widgets must not know `Action`/`EditorState`.
//!
//! ## Multi-pass
//!
//! egui may run the UI pass more than once per frame (`max_passes = 2` when a
//! layout requests a discard). The app builds a fresh `UiOut` per pass and only
//! dispatches when the pass is not discarded (`!Context::will_discard()`), so a
//! discarded pass cannot commit actions twice. Panels must therefore be
//! side-effect free: read `Frame`, draw, push actions.
//!
//! ## Host resources
//!
//! GPU handles (egui textures), the structured console lines and any other
//! host-owned read-only *dynamic* resource are exposed through [`HostView`]. They
//! are not state: a texture handle is not serializable and log history is
//! host-owned, so the frame borrows a view instead. Hosts with nothing to show
//! use the default [`EmptyHost`] (`Frame::new`).
//!
//! Static editor appearance images (icons) are **not** `HostView` resources:
//! they are file-backed assets under `Preferences/Textures`, preloaded by a
//! panel's `init` through `kairos_editor_widgets::ui_image`, and cached by
//! egui's image loaders. `HostView` is for per-frame resources the host
//! produces (viewport textures) or owns as a stream (log history).

use kairos_editor_core::{EditorState, SharedStyleId, action::Action};
use kairos_editor_widgets::color_picker::ColorPickerStyle;
use kairos_editor_widgets::modal::ModalStyle;
use kairos_log::{Level, StackFrame};
use kairos_profile::Profiler;

use crate::shared_style::{
    COLOR_PICKER, ColorPickerSkin, EDITOR_FIELD, EditorFieldStyle, EditorFormStyle, FORM, LIST,
    ListStyle, MODAL, ModalSkin, SharedStyle, SharedStyles, TOOLBAR,
};
use crate::toolbar::style::ToolBarStyle;

/// One host-owned console entry, rendered by the Console panel.
///
/// The pieces stay separate (timestamp, message, source, stack) so the panel
/// can lay them out like Unity's console: one line `[time] message`, then the
/// grey stack trace below it. The panel never formats the whole entry itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleLine {
    /// Monotonic id; the panel uses it for clear/selection bookkeeping.
    pub id: u64,
    /// Severity, mirrored from the `kairos_log` record.
    pub level: Level,
    /// Time of day, `HH:MM:SS`.
    pub time: String,
    /// Log message without the timestamp prefix.
    pub message: String,
    /// Emitting module; shown when no stack trace is available.
    pub target: String,
    /// Caller location (`file:line`) when the logger knows it.
    pub location: Option<String>,
    /// Trimmed caller frames; empty when backtraces are disabled and the
    /// record has no call site either.
    pub stack: Vec<StackFrame>,
}

impl ConsoleLine {
    /// Minimal entry for tests and headless fixtures.
    pub fn new(level: Level, message: impl Into<String>) -> Self {
        Self {
            id: 0,
            level,
            time: "00:00:00".to_owned(),
            message: message.into(),
            target: String::new(),
            location: None,
            stack: Vec::new(),
        }
    }
}

/// Read-only host resources a frame may display.
///
/// Implemented by the app shell; panels only ever look at the data, never
/// mutate it. All methods default to "nothing available", so a host overrides
/// only what it can actually provide.
pub trait HostView {
    /// Calls `f` with the host-owned console history for this frame, oldest
    /// first.
    ///
    /// A callback rather than a borrowed slice because the history lives behind
    /// the [`ConsoleLog`](crate::ConsoleLog) sink's lock: the panel sees the
    /// lines only while drawing. The panel never consumes history — closing the
    /// console loses nothing and reopening shows the same lines.
    fn console_lines(&self, _f: &mut dyn FnMut(&[ConsoleLine])) {}

    /// The host-owned profiling capture, when the host runs one.
    ///
    /// The Profiler panel reads the structured session through this handle and
    /// toggles capture here. Recording state is not `EditorState` (it is not
    /// serializable and belongs to the host), so this is the same kind of
    /// host-resource seam as [`HostView::console_lines`], not a state mutation.
    fn profiler(&self) -> Option<&Profiler> {
        None
    }

    /// The host's preview of one project file, keyed by the project root and the
    /// row's relative path.
    ///
    /// A texture row draws the asset itself instead of a family glyph, so the
    /// tree asks this once per visible texture row per frame. The host owns the
    /// decode, the downscale and the GPU texture (the same host-resource seam as
    /// [`Self::console_lines`]: a texture handle is not state), and returns
    /// [`None`] while the image is still being decoded: the row keeps its family
    /// icon and the panel asks again on the next frame. The default implementation
    /// has no host: headless tests and embedders keep the family icon.
    fn project_thumbnail(
        &self,
        _ctx: &egui::Context,
        _root: &str,
        _relative: &str,
    ) -> Option<egui::TextureId> {
        None
    }
}

/// Read-only snapshot handed to feature components for one UI pass.
///
/// Holds the immutable state tree plus the host resources the UI may display.
/// Panels never mutate either: every intent goes into [`UiOut`].
#[derive(Clone, Copy)]
pub struct Frame<'a> {
    pub state: &'a EditorState,
    /// Host-owned display resources (textures, log lines, ...).
    pub host: &'a dyn HostView,
    /// The Preferences pages: one per open panel's live skin, plus the shared
    /// styles. A style already draws itself, so this is only the page list.
    ///
    /// Empty unless Preferences is open: no other panel reads it. The app holds
    /// the snapshot across frames and copies a style only when the page list
    /// changes or an owner is reset; the panel takes its copy of one page when
    /// the user switches to it.
    pub page_skins: &'a [crate::skin::PageSkin],
    /// Appearance owned by the editor shell (toolbar, fields, form rows, lists,
    /// color picker), shared by every panel. `with_host` points this at the pure
    /// code defaults so tests and embedders do not need to own a registry.
    pub shared_styles: &'a SharedStyles,
}

impl<'a> Frame<'a> {
    /// Frame carrying host resources, e.g. `Frame::with_host(&state, &self.host)`.
    pub fn new(
        state: &'a EditorState,
        host: &'a dyn HostView,
        shared_styles: &'a SharedStyles,
    ) -> Self {
        Self {
            state,
            host,
            page_skins: &[],
            shared_styles,
        }
    }

    pub fn with_pages(mut self, page_skins: &'a [crate::skin::PageSkin]) -> Self {
        self.page_skins = page_skins;
        self
    }

    /// The live toolbar skin.
    pub fn toolbar_style(&self) -> &'a ToolBarStyle {
        self.shared_style(TOOLBAR)
    }

    /// The value fields' runtime tokens, projected from the shared skin.
    pub fn field_style(&self) -> kairos_editor_widgets::field::FieldStyle {
        self.shared_style::<EditorFieldStyle>(EDITOR_FIELD).tokens()
    }

    /// The reorderable list's live skin. Lists read this instead of a panel's
    /// style, so every list in the editor shares one skin.
    pub fn list_style(&self) -> &'a ListStyle {
        self.shared_style(LIST)
    }

    /// The form rows' live skin. A property page reads this instead of a panel's
    /// style, so every form in the editor shares one row grid: the Preferences
    /// editor today, an object inspector later.
    pub fn form_style(&self) -> &'a EditorFormStyle {
        self.shared_style(FORM)
    }

    /// The color picker's runtime tokens, projected from the shared skin.
    pub fn color_picker_style(&self) -> ColorPickerStyle {
        self.shared_style::<ColorPickerSkin>(COLOR_PICKER).tokens()
    }

    /// The modal dialog's runtime tokens, projected from the shared skin.
    pub fn modal_style(&self) -> ModalStyle {
        self.shared_style::<ModalSkin>(MODAL).tokens()
    }

    /// One shared style by id.
    ///
    /// A frame always draws from a complete registry: `with_host` points at
    /// the process-wide code defaults and the app attaches the builtin
    /// catalog. A missing style therefore means the caller attached a partial
    /// catalog — a programming error, reported here with the id rather than
    /// silently drawn with some other skin.
    fn shared_style<T: SharedStyle>(&self, id: SharedStyleId) -> &'a T {
        match self.shared_styles.get(id) {
            Some(style) => style,
            None => panic!(
                "the frame's shared registry has no `{id}` skin; build it from StyleCatalog::builtin()"
            ),
        }
    }
}

impl std::fmt::Debug for Frame<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("state", self.state)
            .finish_non_exhaustive()
    }
}

/// Frame-level action buffer: the only channel from the view layer back into the
/// state tree. The app drains it with `take()` after the last UI pass.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiOut {
    actions: Vec<Action>,
}

impl UiOut {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, action: Action) {
        self.actions.push(action);
    }

    pub fn extend(&mut self, actions: impl IntoIterator<Item = Action>) {
        self.actions.extend(actions);
    }

    pub fn take(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.actions)
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn as_slice(&self) -> &[Action] {
        &self.actions
    }
}

#[cfg(test)]
mod tests {
    use egui::{Color32, Theme};
    use kairos_color::Color;
    use kairos_editor_core::EditorState;
    use kairos_editor_widgets::style::Style;

    use super::*;
    use crate::shared_style::{MODAL, ModalSkin, SharedStyleDef, StyleCatalog};

    struct EmptyHost;

    impl HostView for EmptyHost {}

    /// A registry built from a catalog that registers nothing: the shape of a
    /// misconfigured embedder. The frame's accessors require a complete
    /// registry, so this is a setup bug, not a runtime state.
    fn empty_registry() -> SharedStyles {
        static NO_STYLES: [SharedStyleDef; 0] = [];
        SharedStyles::defaults(&StyleCatalog::new(&NO_STYLES), Theme::Dark)
    }

    /// Without `.with_shared` the frame draws from the process-wide code
    /// defaults, so a test or embedder can draw without owning a registry.
    #[test]
    fn a_frame_without_a_live_registry_draws_the_code_defaults() {
        let state = EditorState::new();
        let host = EmptyHost;
        let shared_styles = SharedStyles::default();
        let frame = Frame::new(&state, &host, &shared_styles);

        assert_eq!(frame.toolbar_style().height, ToolBarStyle::dark().height);
        assert_eq!(frame.list_style(), &ListStyle::dark());
        assert_eq!(frame.modal_style().backdrop, ModalStyle::dark().backdrop);
    }

    /// Attaching a catalog that omits a style is a setup bug, not a state to
    /// render around: the first access names the missing skin.
    #[test]
    #[should_panic(expected = "has no `list` skin")]
    fn a_registry_without_a_style_is_a_programming_error() {
        let state = EditorState::new();
        let host = EmptyHost;
        let shared = empty_registry();
        let _ = Frame::new(&state, &host, &shared).list_style();
    }

    /// The frame reads the live registry value, so a Preferences edit is what
    /// the next dialog draws.
    #[test]
    fn a_registered_modal_skin_is_what_the_frame_draws_with() {
        let state = EditorState::new();
        let host = EmptyHost;
        let mut shared = SharedStyles::default();
        let backdrop = Color::from_srgb8([0x12, 0x34, 0x56, 0xFF]);
        let mut skin = ModalSkin::dark();
        skin.backdrop = backdrop;
        assert!(shared.replace(MODAL, Box::new(skin)));

        let style = Frame::new(&state, &host, &shared).modal_style();

        assert_eq!(style.backdrop, Color32::from_rgb(0x12, 0x34, 0x56));
        assert_eq!(
            style.fill,
            ModalStyle::dark().fill,
            "the rest of the dialog still comes from the registered skin"
        );
    }
}
