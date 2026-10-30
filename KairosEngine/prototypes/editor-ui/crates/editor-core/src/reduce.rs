//! Root reducer: routes the aggregate `Action` to per-domain reducers and owns
//! the cross-slice rules that no single slice can decide alone.
//!
//! Reducers are pure functions over the state tree; they may read anything and
//! mutate anything, but they never perform IO, never sleep and never touch the
//! engine or egui. Everything the host must do leaves as `Effect` data.

pub mod document;
pub mod layout;
pub mod modal;
pub mod project;
pub mod selection;

use crate::action::{Action, AppAction, DocumentAction, ModalAction};
use crate::effect::Effect;
use crate::modal::{ConfirmDialog, Modal};
use crate::state::EditorState;

pub fn reduce(state: &mut EditorState, action: Action) -> Vec<Effect> {
    match action {
        // Cross-slice rule #1: closing a dirty document asks first. Intercept it
        // before the document reducer so "close" cannot silently lose edits.
        Action::Document(DocumentAction::Close { document }) => {
            document::close_request(state, document)
        }
        Action::Project(action) => project::reduce(state, action),
        Action::Selection(action) => selection::reduce(state, action),
        Action::Document(action) => document::reduce(state, action),
        Action::Layout(action) => layout::reduce(state, action),
        Action::Modal(action) => modal::reduce(state, action),
        Action::App(action) => reduce_app(state, action),
    }
}

/// App-level cross-slice rules (quit has to inspect every document).
fn reduce_app(state: &mut EditorState, action: AppAction) -> Vec<Effect> {
    match action {
        AppAction::QuitRequested => {
            let dirty = state.documents.dirty_count();
            if dirty == 0 {
                return vec![Effect::Quit];
            }
            // Cross-slice rule #2: unsaved work turns quit into a confirmation
            // whose confirm/cancel are themselves plain actions.
            state.modal.active = Some(Modal::Confirm(ConfirmDialog {
                title: "Quit editor".to_string(),
                message: format!("{dirty} document(s) have unsaved changes. Quit without saving?"),
                confirm: Action::App(AppAction::QuitConfirmed),
                cancel: Action::Modal(ModalAction::Dismiss),
            }));
            Vec::new()
        }
        AppAction::QuitConfirmed => {
            state.modal.active = None;
            vec![Effect::Quit]
        }
    }
}
