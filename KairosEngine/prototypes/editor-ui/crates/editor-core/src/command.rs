//! Command registry: the single place toolbar, menus, command palette and the
//! MCP bridge look up "what can the user do, is it enabled, what does it do".
//!
//! Commands are intents that act on the current selection (e.g. "save active
//! document") rather than carrying runtime ids, because a registry entry is
//! static data.

use crate::action::{Action, AppAction, DocumentAction, LayoutAction, ModalAction};
use crate::state::{EditorState, PanelId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(pub &'static str);

impl CommandId {
    pub const SAVE_DOCUMENT: CommandId = CommandId("document.save");
    pub const UNDO: CommandId = CommandId("document.undo");
    pub const REDO: CommandId = CommandId("document.redo");
    pub const REVERT_DOCUMENT: CommandId = CommandId("document.revert");
    pub const CLOSE_DOCUMENT: CommandId = CommandId("document.close");
    pub const TOGGLE_INSPECTOR: CommandId = CommandId("layout.toggle_inspector");
    pub const TOGGLE_CONSOLE: CommandId = CommandId("layout.toggle_console");
    pub const PERSIST_LAYOUT: CommandId = CommandId("layout.persist");
    pub const QUIT: CommandId = CommandId("app.quit");
    pub const CONFIRM_MODAL: CommandId = CommandId("modal.confirm");
    pub const CANCEL_MODAL: CommandId = CommandId("modal.cancel");
}

#[derive(Clone, Debug)]
pub struct Command {
    pub id: CommandId,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub action: Action,
    pub enabled: fn(&EditorState) -> bool,
}

impl Command {
    pub fn is_enabled(&self, state: &EditorState) -> bool {
        (self.enabled)(state)
    }
}

#[derive(Clone, Debug)]
pub struct CommandRegistry {
    commands: Vec<Command>,
}

impl CommandRegistry {
    pub fn editor_defaults() -> Self {
        Self {
            commands: vec![
                Command {
                    id: CommandId::SAVE_DOCUMENT,
                    label: "Save Document",
                    shortcut: Some("Ctrl+S"),
                    action: Action::Document(DocumentAction::SaveActive),
                    enabled: save_enabled,
                },
                Command {
                    id: CommandId::UNDO,
                    label: "Undo",
                    shortcut: Some("Ctrl+Z"),
                    action: Action::Document(DocumentAction::UndoActive),
                    enabled: undo_enabled,
                },
                Command {
                    id: CommandId::REDO,
                    label: "Redo",
                    shortcut: Some("Ctrl+Shift+Z"),
                    action: Action::Document(DocumentAction::RedoActive),
                    enabled: redo_enabled,
                },
                Command {
                    id: CommandId::REVERT_DOCUMENT,
                    label: "Revert Document",
                    shortcut: None,
                    action: Action::Document(DocumentAction::RevertActive),
                    enabled: revert_enabled,
                },
                Command {
                    id: CommandId::CLOSE_DOCUMENT,
                    label: "Close Document",
                    shortcut: Some("Ctrl+W"),
                    action: Action::Document(DocumentAction::CloseActive),
                    enabled: close_enabled,
                },
                Command {
                    id: CommandId::TOGGLE_INSPECTOR,
                    label: "Toggle Inspector",
                    shortcut: Some("Ctrl+I"),
                    action: Action::Layout(LayoutAction::TogglePanel {
                        panel: PanelId::INSPECTOR,
                    }),
                    enabled: always,
                },
                Command {
                    id: CommandId::TOGGLE_CONSOLE,
                    label: "Toggle Console",
                    shortcut: Some("Ctrl+Shift+C"),
                    action: Action::Layout(LayoutAction::TogglePanel {
                        panel: PanelId::CONSOLE,
                    }),
                    enabled: always,
                },
                Command {
                    id: CommandId::PERSIST_LAYOUT,
                    label: "Persist Layout",
                    shortcut: None,
                    action: Action::Layout(LayoutAction::PersistLayout),
                    enabled: always,
                },
                Command {
                    id: CommandId::QUIT,
                    label: "Quit Editor",
                    shortcut: Some("Ctrl+Q"),
                    action: Action::App(AppAction::QuitRequested),
                    enabled: always,
                },
                Command {
                    id: CommandId::CONFIRM_MODAL,
                    label: "Confirm Dialog",
                    shortcut: Some("Enter"),
                    action: Action::Modal(ModalAction::Confirm),
                    enabled: modal_open,
                },
                Command {
                    id: CommandId::CANCEL_MODAL,
                    label: "Cancel Dialog",
                    shortcut: Some("Escape"),
                    action: Action::Modal(ModalAction::Cancel),
                    enabled: modal_open,
                },
            ],
        }
    }

    pub fn commands(&self) -> impl Iterator<Item = &Command> {
        self.commands.iter()
    }

    pub fn get(&self, id: CommandId) -> Option<&Command> {
        self.commands.iter().find(|command| command.id == id)
    }

    /// Resolve a command to its action, honoring the enabled predicate. Returns
    /// `None` for unknown or disabled commands (toolbar greys them out, palette
    /// filters them, MCP gets an explicit "disabled" answer).
    pub fn action(&self, id: CommandId, state: &EditorState) -> Option<Action> {
        self.get(id)
            .filter(|command| command.is_enabled(state))
            .map(|command| command.action.clone())
    }
}

fn save_enabled(state: &EditorState) -> bool {
    state
        .active_document()
        .is_some_and(|doc| doc.dirty && !doc.is_saving())
}

fn undo_enabled(state: &EditorState) -> bool {
    state.active_document().is_some_and(|doc| doc.can_undo())
}

fn redo_enabled(state: &EditorState) -> bool {
    state.active_document().is_some_and(|doc| doc.can_redo())
}

fn revert_enabled(state: &EditorState) -> bool {
    state.active_document().is_some_and(|doc| doc.dirty)
}

fn close_enabled(state: &EditorState) -> bool {
    state.active_document().is_some()
}

fn modal_open(state: &EditorState) -> bool {
    state.modal.is_open()
}

fn always(_state: &EditorState) -> bool {
    true
}
