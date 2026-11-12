//! The editor's state tree: which panels are open, which one has focus, and the
//! concrete editor data panels draw from.
//!
//! This is plain data. The dock tree (split fractions, tab order, floating
//! windows) lives in the app's `DockState`; the panel objects live in
//! `kairos_editor_component`'s `PanelHost`. `LayoutState` only records the
//! user's open/focus *intent*, and the app reconciles it with the dock every
//! frame.

use std::collections::BTreeSet;

use crate::id::{PanelId, PanelInstanceId};

/// Which panels are open and which panel instance has keyboard/focus.
///
/// This is ordinary app state, not a headless architecture: the dock layout
/// lives in the app/dock crate, while the open/focus intent lives here. The app
/// reconciles the two every frame, so closing a panel or restoring a layout
/// converges without special-case code in the UI.
#[derive(Debug, Default)]
pub struct LayoutState {
    open: BTreeSet<PanelId>,
    focus: Option<PanelInstanceId>,
}

impl LayoutState {
    pub fn is_open(&self, kind: PanelId) -> bool {
        self.open.contains(&kind)
    }

    pub fn open(&mut self, kind: PanelId) {
        self.open.insert(kind);
    }

    pub fn close(&mut self, kind: PanelId) {
        self.open.remove(&kind);
        if self.focus.is_some_and(|focus| focus.kind == kind) {
            self.focus = None;
        }
    }

    pub fn toggle(&mut self, kind: PanelId) {
        if self.is_open(kind) {
            self.close(kind);
        } else {
            self.open(kind);
        }
    }

    pub fn focus(&self) -> Option<PanelInstanceId> {
        self.focus
    }

    pub fn set_focus(&mut self, focus: Option<PanelInstanceId>) {
        self.focus = focus;
    }
}

/// Concrete editor state. Panels get a mutable reference to this through
/// `PanelContext`; this crate is where the state lives, not where reducers live
/// (for now panel bodies mutate state directly).
#[derive(Debug)]
pub struct EditorState {
    pub layout: LayoutState,
    pub project_name: String,
    pub counter: u32,
    pub log: Vec<String>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            layout: LayoutState::default(),
            project_name: String::from("demo-project"),
            counter: 0,
            log: vec![String::from("editor booted")],
        }
    }
}

impl EditorState {
    pub fn new() -> Self {
        Self::default()
    }
}
