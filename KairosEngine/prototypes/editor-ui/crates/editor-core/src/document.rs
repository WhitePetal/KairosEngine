//! Document model + edit history. A document owns its content, its last saved
//! snapshot and its undo history; nothing here touches the filesystem.

use crate::effect::EffectTicket;
use crate::state::DocumentId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
    Text,
    Toml,
    Material,
    Binary,
}

/// Material data stub: enough to show a non-text edit path that is still plain
/// data (`f32` fields, no GPU/engine handles).
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialData {
    pub base_color: [f32; 3],
    pub roughness: f32,
    pub metallic: f32,
}

impl Default for MaterialData {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0],
            roughness: 0.5,
            metallic: 0.0,
        }
    }
}

/// Editable content of a document.
#[derive(Clone, Debug, PartialEq)]
pub enum EditData {
    Text(String),
    Toml(String),
    Material(MaterialData),
}

impl EditData {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            EditData::Text(text) | EditData::Toml(text) => text.clone().into_bytes(),
            EditData::Material(data) => format!(
                "base_color={:?}\nroughness={}\nmetallic={}\n",
                data.base_color, data.roughness, data.metallic
            )
            .into_bytes(),
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            EditData::Text(text) | EditData::Toml(text) => Some(text),
            EditData::Material(_) => None,
        }
    }
}

/// One undoable step. Snapshot based: simple, obviously correct, memory hungry.
#[derive(Clone, Debug, PartialEq)]
pub struct EditTransaction {
    pub label: String,
    pub before: EditData,
    pub after: EditData,
}

/// A save that the host has been asked to perform. Keeping the ticket *and* the
/// exact content snapshot makes completions safe even if the user keeps typing.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingSave {
    pub ticket: EffectTicket,
    pub content: EditData,
}

/// Result reported by the host when a save request completes.
#[derive(Clone, Debug, PartialEq)]
pub enum SaveResult {
    Ok,
    Failed { error: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub id: DocumentId,
    pub path: String,
    pub kind: DocumentKind,
    pub edit: EditData,
    /// Last content known to be on disk (or the content the document opened with).
    pub saved: EditData,
    pub dirty: bool,
    pub history: Vec<EditTransaction>,
    /// Number of applied history entries; `history[cursor..]` is the redo tail.
    pub cursor: usize,
    pub pending_save: Option<PendingSave>,
    /// Set by the "save then close" confirmation flow.
    pub close_after_save: bool,
}

impl Document {
    pub fn new(id: DocumentId, path: String, kind: DocumentKind, content: EditData) -> Self {
        Self {
            id,
            path,
            kind,
            saved: content.clone(),
            edit: content,
            dirty: false,
            history: Vec::new(),
            cursor: 0,
            pending_save: None,
            close_after_save: false,
        }
    }

    fn recompute_dirty(&mut self) {
        self.dirty = self.edit != self.saved;
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.history.len()
    }

    pub fn is_saving(&self) -> bool {
        self.pending_save.is_some()
    }

    /// Apply a new content snapshot as one undoable transaction.
    pub fn apply_edit(&mut self, label: impl Into<String>, after: EditData) {
        if self.edit == after {
            return;
        }
        let before = std::mem::replace(&mut self.edit, after.clone());
        self.history.truncate(self.cursor);
        self.history.push(EditTransaction {
            label: label.into(),
            before,
            after,
        });
        self.cursor = self.history.len();
        self.recompute_dirty();
    }

    pub fn append_text(&mut self, insert: &str) -> bool {
        let after = match &self.edit {
            EditData::Text(text) => EditData::Text(format!("{text}{insert}")),
            EditData::Toml(text) => EditData::Toml(format!("{text}{insert}")),
            EditData::Material(_) => return false,
        };
        self.apply_edit("Edit text", after);
        true
    }

    pub fn set_text(&mut self, text: String) -> bool {
        let after = match &self.edit {
            EditData::Text(_) => EditData::Text(text),
            EditData::Toml(_) => EditData::Toml(text),
            EditData::Material(_) => return false,
        };
        self.apply_edit("Set text", after);
        true
    }

    pub fn set_material(&mut self, material: MaterialData) -> bool {
        if !matches!(self.edit, EditData::Material(_)) {
            return false;
        }
        self.apply_edit("Edit material", EditData::Material(material));
        true
    }

    pub fn undo(&mut self) -> bool {
        if !self.can_undo() {
            return false;
        }
        self.cursor -= 1;
        self.edit = self.history[self.cursor].before.clone();
        self.recompute_dirty();
        true
    }

    pub fn redo(&mut self) -> bool {
        if !self.can_redo() {
            return false;
        }
        self.edit = self.history[self.cursor].after.clone();
        self.cursor += 1;
        self.recompute_dirty();
        true
    }

    /// Restore the last saved content as one (undoable) transaction.
    pub fn revert(&mut self) -> bool {
        if self.edit == self.saved {
            return false;
        }
        let before = std::mem::replace(&mut self.edit, self.saved.clone());
        self.history.truncate(self.cursor);
        self.history.push(EditTransaction {
            label: "Revert to saved".to_string(),
            before,
            after: self.saved.clone(),
        });
        self.cursor = self.history.len();
        self.recompute_dirty();
        true
    }

    /// Host reported that `content` was written successfully.
    pub fn mark_saved(&mut self, content: EditData) {
        self.saved = content;
        self.pending_save = None;
        self.recompute_dirty();
    }

    /// Host reported that the save request failed; keep the document dirty.
    pub fn mark_save_failed(&mut self) {
        self.pending_save = None;
        self.close_after_save = false;
    }
}
