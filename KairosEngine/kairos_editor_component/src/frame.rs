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

use kairos_editor_core::{EditorState, action::Action};
use kairos_log::Level;
use kairos_profile::Profiler;

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
    /// Trimmed stack-trace lines; empty when backtraces are disabled.
    pub stack: Vec<String>,
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
}

impl<'a> Frame<'a> {
    /// Frame carrying host resources, e.g. `Frame::with_host(&state, &self.host)`.
    pub fn with_host(state: &'a EditorState, host: &'a dyn HostView) -> Self {
        Self { state, host }
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
