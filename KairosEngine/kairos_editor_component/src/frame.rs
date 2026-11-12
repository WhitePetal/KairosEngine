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
//! GPU handles (egui textures), the console log snapshot and any other
//! host-owned read-only resource are exposed through [`HostView`]. They are not
//! state: a texture handle is not serializable and log history is host-owned, so
//! the frame borrows a view instead. Hosts with nothing to show use the default
//! [`EmptyHost`] (`Frame::new`).

use kairos_editor_core::{EditorState, action::Action};

/// Read-only host resources a frame may display.
///
/// Implemented by the app shell; panels only ever look at the data, never
/// mutate it. All methods default to "nothing available", so a host overrides
/// only what it can actually provide.
pub trait HostView {
    /// Snapshot of the host-owned console/log lines for this frame.
    fn console_lines(&self) -> &[String] {
        &[]
    }
}

/// A host view with no resources: headless tests, panels that need no host data.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmptyHost;

impl HostView for EmptyHost {}

/// The one shared empty host. A `Frame` is `Copy`, so the default host has to
/// live in static storage rather than in a temporary.
static EMPTY_HOST: EmptyHost = EmptyHost;

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
    /// Frame for a host with no extra resources (headless tests, tools).
    pub fn new(state: &'a EditorState) -> Self {
        Self {
            state,
            host: &EMPTY_HOST,
        }
    }

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
