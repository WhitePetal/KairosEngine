//! Frame + UiOut: the two types the real frame layer (ADR 3.3) owns.
//!
//! A `Frame` is an immutable view of the state tree plus the host resources the
//! UI may display but never owns. `UiOut` is the frame's action buffer: views
//! only push intents into it, the host drains it after the UI pass and dispatches
//! every action through the one real reducer.
//!
//! Keeping these in the components crate (not in the widgets crate) is the
//! layering point: widgets must not know what a Frame or an Action is.
//!
//! ## Host resources
//!
//! GPU handles (egui textures), the structured log ring buffer and any other
//! host-owned read-only resource are exposed through [`HostView`]. The domain
//! store must not copy them (a texture handle is not serializable, and the log
//! history is host-owned), so the frame carries a borrowed view instead. Hosts
//! that have nothing to show use [`EmptyHost`] (the default for
//! [`Frame::new`]), which keeps headless tests free of host wiring.

use editor_core_prototype::{Action, Document, EditorState, ProjectState, ViewportId};

/// Read-only host resources a frame may display.
///
/// Implemented by the host (the app shell in the real editor); panels only ever
/// look at the data, never mutate it. Both methods default to "nothing
/// available" so a host only overrides what it can actually provide.
pub trait HostView {
    /// egui texture of a viewport render target, if the host has rendered and
    /// registered one for this viewport.
    fn viewport_texture(&self, viewport: ViewportId) -> Option<egui::TextureId> {
        let _ = viewport;
        None
    }

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
/// Panels never mutate either: every intent goes into [`UiOut`] as an [`Action`].
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

    pub fn active_document(&self) -> Option<&'a Document> {
        self.state.active_document()
    }

    pub fn project(&self) -> &'a ProjectState {
        &self.state.project
    }
}

impl std::fmt::Debug for Frame<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("state", self.state)
            .finish_non_exhaustive()
    }
}

/// Frame-level action buffer: the only channel from the view layer back to the
/// reducer. Hosts drain it with `take()` once per UI pass (last pass wins when
/// egui runs multiple passes -- see merged README).
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
