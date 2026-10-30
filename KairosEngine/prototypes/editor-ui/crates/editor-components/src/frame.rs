//! Frame + UiOut: the two types the real frame layer (ADR 3.3) owns.
//!
//! A `Frame` is an immutable view of the state tree (plus, in the real system,
//! host resources such as the engine world). `UiOut` is the frame's action
//! buffer: views only push intents into it, the host drains it after the UI pass
//! and dispatches every action through the one real reducer.
//!
//! Keeping these in the components crate (not in the widgets crate) is the
//! layering point: widgets must not know what a Frame or an Action is.

use editor_core_prototype::{Action, Document, EditorState, ProjectState};

/// Read-only snapshot handed to feature components for one UI pass.
#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    pub state: &'a EditorState,
}

impl<'a> Frame<'a> {
    pub fn new(state: &'a EditorState) -> Self {
        Self { state }
    }

    pub fn active_document(&self) -> Option<&'a Document> {
        self.state.active_document()
    }

    pub fn project(&self) -> &'a ProjectState {
        &self.state.project
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
