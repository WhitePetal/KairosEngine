//! The editor's state tree: which panels are open, which one has focus, and the
//! concrete editor data panels draw from.
//!
//! This is plain data. The dock tree (split fractions, tab order, floating
//! windows) lives in the app's `DockState`; the panel objects live in
//! `kairos_editor_component`'s `PanelHost`. `LayoutState` only records the
//! user's open/focus *intent*, and the app reconciles it with the dock every
//! frame.
//!
//! The only writer is [`crate::reduce`]. The UI reads a read-only frame built by
//! `kairos_editor_component` and pushes actions; host-owned resources (console
//! log, future viewport textures) are not state and do not live here.

use std::collections::BTreeSet;

use crate::panel::{PanelId, PanelInstanceId};
use crate::project::ProjectState;

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

    /// Mark a panel open.
    ///
    /// Runtime opens must go through `LayoutAction` so the layout reducer can
    /// return a placement effect. Only bootstrapping code (which builds the
    /// matching dock tabs itself) should call this directly.
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

/// Concrete editor state: the single tree the UI reads and the reducers write.
///
/// The editor-state slices grow by domain, one field per slice: `layout` records
/// open/focus intent, `project` owns the asset workspace (root, tree, view
/// intent, scan progress). A slice's actions and effects live next to its state
/// in the same crate; the panel UI only ever reads this tree.
#[derive(Debug, Default)]
pub struct EditorState {
    pub layout: LayoutState,
    pub project: ProjectState,
}

impl EditorState {
    pub fn new() -> Self {
        Self::default()
    }
}
