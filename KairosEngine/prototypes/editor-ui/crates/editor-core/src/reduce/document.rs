//! Document reducers: open/edit/save/save-complete/revert/undo/redo and the
//! unsaved-close confirmation flow.

use crate::action::{Action, DocumentAction, ModalAction};
use crate::document::{Document, DocumentKind, EditData, PendingSave, SaveResult};
use crate::effect::{Effect, RequestKind};
use crate::modal::{ConfirmDialog, Modal};
use crate::state::{DocumentId, EditorState};

pub fn reduce(state: &mut EditorState, action: DocumentAction) -> Vec<Effect> {
    match action {
        DocumentAction::Open {
            path,
            kind,
            content,
        } => open(state, path, kind, content),
        DocumentAction::Close { document } => close_request(state, document),
        DocumentAction::CloseActive => match state.selection.active_document {
            Some(document) => close_request(state, document),
            None => Vec::new(),
        },
        DocumentAction::EditText { document, insert } => with_document(state, document, |doc| {
            doc.append_text(&insert);
        }),
        DocumentAction::SetText { document, text } => with_document(state, document, |doc| {
            doc.set_text(text);
        }),
        DocumentAction::SetMaterial { document, material } => {
            with_document(state, document, |doc| {
                doc.set_material(material);
            })
        }
        DocumentAction::Save { document } => save(state, document),
        DocumentAction::SaveActive => match state.selection.active_document {
            Some(document) => save(state, document),
            None => Vec::new(),
        },
        DocumentAction::SaveThenClose { document } => save_then_close(state, document),
        DocumentAction::SaveCompleted {
            document,
            ticket,
            result,
        } => save_completed(state, document, ticket, result),
        DocumentAction::Revert { document } => with_document(state, document, |doc| {
            doc.revert();
        }),
        DocumentAction::RevertActive => match state.selection.active_document {
            Some(document) => with_document(state, document, |doc| {
                doc.revert();
            }),
            None => Vec::new(),
        },
        DocumentAction::Undo { document } => with_document(state, document, |doc| {
            doc.undo();
        }),
        DocumentAction::UndoActive => match state.selection.active_document {
            Some(document) => with_document(state, document, |doc| {
                doc.undo();
            }),
            None => Vec::new(),
        },
        DocumentAction::Redo { document } => with_document(state, document, |doc| {
            doc.redo();
        }),
        DocumentAction::RedoActive => match state.selection.active_document {
            Some(document) => with_document(state, document, |doc| {
                doc.redo();
            }),
            None => Vec::new(),
        },
    }
}

/// Called by the root reducer for every close request.
pub fn close_request(state: &mut EditorState, document: DocumentId) -> Vec<Effect> {
    let Some(doc) = state.documents.get(document) else {
        return Vec::new();
    };
    if !doc.dirty {
        return force_close(state, document);
    }
    let dialog = ConfirmDialog {
        title: "Unsaved changes".to_string(),
        message: format!("{} has unsaved changes. Save before closing?", doc.path),
        confirm: Action::Document(DocumentAction::SaveThenClose { document }),
        cancel: Action::Modal(ModalAction::Dismiss),
    };
    state.modal.active = Some(Modal::Confirm(dialog));
    Vec::new()
}

fn open(
    state: &mut EditorState,
    path: String,
    kind: DocumentKind,
    content: EditData,
) -> Vec<Effect> {
    let document = state.documents.create(path, kind, content);
    state.selection.active_document = Some(document);
    Vec::new()
}

fn force_close(state: &mut EditorState, document: DocumentId) -> Vec<Effect> {
    state.documents.remove(document);
    if state.selection.active_document == Some(document) {
        state.selection.active_document = None;
    }
    Vec::new()
}

fn save(state: &mut EditorState, document: DocumentId) -> Vec<Effect> {
    let Some(doc) = state.documents.get(document) else {
        return Vec::new();
    };
    if !doc.dirty || doc.is_saving() {
        return Vec::new();
    }
    let content = doc.edit.clone();
    let bytes = content.to_bytes();
    let ticket = state.begin_request(RequestKind::SaveDocument { document });
    if let Some(doc) = state.documents.get_mut(document) {
        doc.pending_save = Some(PendingSave { ticket, content });
    }
    vec![Effect::SaveDocument {
        ticket,
        document,
        bytes,
    }]
}

fn save_then_close(state: &mut EditorState, document: DocumentId) -> Vec<Effect> {
    let Some(doc) = state.documents.get(document) else {
        return Vec::new();
    };
    if !doc.dirty {
        return force_close(state, document);
    }
    let effects = save(state, document);
    // Marked after save() so the flag survives a completion that races the click.
    if let Some(doc) = state.documents.get_mut(document) {
        doc.close_after_save = true;
    }
    effects
}

fn save_completed(
    state: &mut EditorState,
    document: DocumentId,
    ticket: crate::effect::EffectTicket,
    result: SaveResult,
) -> Vec<Effect> {
    let mut effects = Vec::new();
    let mut close_now = false;
    {
        let Some(doc) = state.documents.get_mut(document) else {
            return Vec::new();
        };
        let Some(pending) = doc.pending_save.take() else {
            return Vec::new();
        };
        if pending.ticket != ticket {
            // Stale completion (should not happen: tickets are unique), keep state.
            doc.pending_save = Some(pending);
            return Vec::new();
        }
        match result {
            SaveResult::Ok => {
                doc.mark_saved(pending.content);
                close_now = doc.close_after_save;
                doc.close_after_save = false;
            }
            SaveResult::Failed { error } => {
                doc.mark_save_failed();
                effects.push(Effect::ShowNativeDialog {
                    title: "Save failed".to_string(),
                    message: error,
                });
            }
        }
    }
    if close_now {
        force_close(state, document);
    }
    effects
}

fn with_document(
    state: &mut EditorState,
    document: DocumentId,
    edit: impl FnOnce(&mut Document),
) -> Vec<Effect> {
    if let Some(doc) = state.documents.get_mut(document) {
        edit(doc);
    }
    Vec::new()
}
