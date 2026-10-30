//! Throwaway prototype of the proposed `kairos_editor` UI core architecture.
//!
//! The data flow, deliberately one direction:
//!
//! ```text
//! view / MCP --- dispatch(Action) --> App --> reduce(&mut EditorState) --> Vec<Effect> --> host
//!                                                                                        |
//! App::complete(ticket, EffectOutcome) <--------------------------------------------------+
//! ```
//!
//! * one owned `EditorState` (plain data, no egui/engine types),
//! * pure reducers `(&mut state, Action) -> Vec<Effect>`,
//! * effects as data executed by the host, completions routed by `EffectTicket`,
//! * a `CommandRegistry` shared by toolbar/menu/palette/MCP,
//! * document model with snapshot undo/redo.
//!
//! This crate has zero dependencies on the main workspace; it is a design probe,
//! not production code. Run `cargo test` inside `prototypes/editor-ui`.

#![forbid(unsafe_code)]

pub mod action;
pub mod command;
pub mod document;
pub mod effect;
pub mod modal;
pub mod reduce;
pub mod state;

pub use action::{
    Action, AppAction, DocumentAction, LayoutAction, ModalAction, ProjectAction, SelectionAction,
};
pub use command::{Command, CommandId, CommandRegistry};
pub use document::{
    Document, DocumentKind, EditData, EditTransaction, MaterialData, PendingSave, SaveResult,
};
pub use effect::{Effect, EffectOutcome, EffectTicket, RequestId, RequestKind};
pub use modal::{ConfirmDialog, Modal, ModalState};
pub use state::{
    AssetEntry, AssetId, AssetKind, DocumentId, DocumentsState, EditorState, LayoutState, PanelId,
    ProjectState, SelectionState,
};

/// Thin host around the core: owns the state tree, the command registry and the
/// outbox of effects that the host must execute.
#[derive(Debug)]
pub struct App {
    pub state: EditorState,
    pub commands: CommandRegistry,
    outbox: Vec<Effect>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            state: EditorState::new(),
            commands: CommandRegistry::editor_defaults(),
            outbox: Vec::new(),
        }
    }

    /// Reduce one action, collect its effects into the outbox and return them.
    /// The returned vector is the immediate result of this dispatch (handy for
    /// tests); the host normally drains `take_effects` once per frame.
    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        let effects = reduce::reduce(&mut self.state, action);
        self.outbox.extend(effects.iter().cloned());
        effects
    }

    /// Dispatch a registry command if it exists and is enabled for the current
    /// state. Shared by toolbar, menu, command palette and the MCP bridge.
    pub fn dispatch_command(&mut self, id: CommandId) -> Vec<Effect> {
        let action = self.commands.action(id, &self.state);
        match action {
            Some(action) => self.dispatch(action),
            None => Vec::new(),
        }
    }

    /// Route an async host completion back into an action and reduce it.
    /// Completions for unknown/stale tickets are ignored.
    pub fn complete(&mut self, ticket: EffectTicket, outcome: EffectOutcome) -> Vec<Effect> {
        let Some(kind) = self.state.inflight.get(&ticket).copied() else {
            return Vec::new();
        };
        let action = match (kind, outcome) {
            (RequestKind::SaveDocument { document }, EffectOutcome::DocumentSaved) => {
                Action::Document(DocumentAction::SaveCompleted {
                    document,
                    ticket,
                    result: SaveResult::Ok,
                })
            }
            (
                RequestKind::SaveDocument { document },
                EffectOutcome::DocumentSaveFailed { error },
            ) => Action::Document(DocumentAction::SaveCompleted {
                document,
                ticket,
                result: SaveResult::Failed { error },
            }),
            (RequestKind::LoadAsset { asset }, EffectOutcome::AssetLoaded) => {
                Action::Project(ProjectAction::AssetLoadCompleted {
                    asset,
                    ok: true,
                    error: None,
                })
            }
            (RequestKind::LoadAsset { asset }, EffectOutcome::AssetLoadFailed { error }) => {
                Action::Project(ProjectAction::AssetLoadCompleted {
                    asset,
                    ok: false,
                    error: Some(error),
                })
            }
            (RequestKind::SpawnPreview { asset }, EffectOutcome::PreviewSpawned) => {
                Action::Project(ProjectAction::PreviewSpawned { asset })
            }
            // Ticket/outcome mismatch: host contract violation, ignore it.
            _ => return Vec::new(),
        };
        self.state.inflight.remove(&ticket);
        self.dispatch(action)
    }

    /// Drain effects accumulated since the last frame.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.outbox)
    }

    pub fn pending_effects(&self) -> &[Effect] {
        &self.outbox
    }
}

#[cfg(test)]
mod lib_tests {
    use super::*;

    #[test]
    fn dispatch_collects_into_outbox() {
        let mut app = App::new();
        assert!(app.take_effects().is_empty());
        let effects = app.dispatch(Action::Layout(LayoutAction::PersistLayout));
        assert_eq!(effects.len(), 1);
        assert_eq!(app.pending_effects().len(), 1);
        assert_eq!(app.take_effects(), effects);
        assert!(app.pending_effects().is_empty());
    }
}
