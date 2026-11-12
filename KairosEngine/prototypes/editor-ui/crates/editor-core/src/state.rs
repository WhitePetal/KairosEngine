//! The single owned editor state tree.
//!
//! Everything in here is plain data: no `egui` types, no engine handles, no
//! `Arc<Mutex<_>>`, no receivers. Views may only read `&EditorState`, and
//! reducers are the only code allowed to take `&mut EditorState`.

use std::collections::{BTreeMap, BTreeSet};

use crate::document::{Document, DocumentKind, EditData, MaterialData};
use crate::effect::{EffectTicket, RequestKind};
use crate::modal::ModalState;
use crate::viewport::{SceneObjectId, ViewportState};

/// Stable id of an asset entry. Plain data, copyable, hashable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(pub u64);

/// Stable id of an opened document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(pub u64);

/// Stable identity of a logical panel *kind*.
///
/// Deliberately a string newtype and **not** a closed enum: panel kinds are
/// contributed by feature modules (project, inspector, viewport, console,
/// gallery, game, preferences, plugin panels, ...). Adding one must not edit
/// `editor-core` or `EditorApp`; the reducer treats the id as opaque data
/// (open/focus sets) and the UI registers a factory under it. Re-exported by
/// `editor-components` so panels share the same id vocabulary as the store.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PanelId(pub &'static str);

impl PanelId {
    pub const PROJECT: Self = Self("project");
    pub const INSPECTOR: Self = Self("inspector");
    pub const VIEWPORT: Self = Self("viewport");
    pub const CONSOLE: Self = Self("console");
    pub const ASSET_BROWSER: Self = Self("asset-browser");
    pub const PREFERENCES: Self = Self("preferences");
}

/// Asset kind as understood by the editor core (no platform file handles).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssetKind {
    Texture,
    Scene,
    Material,
    Toml,
    Audio,
    Binary,
    /// Folder entries exist only so the create-folder project action has an
    /// explicit kind to persist. The *view* still derives the folder tree from
    /// asset paths; most folders have no asset entry at all.
    Folder,
}

/// One row of the project/asset browser. Deliberately "dumb" data so it can be
/// produced by an async scanner on any thread.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetEntry {
    pub id: AssetId,
    pub path: String,
    pub kind: AssetKind,
}

/// Project / asset-browser slice.
#[derive(Clone, Debug, Default)]
pub struct ProjectState {
    pub assets: Vec<AssetEntry>,
    /// Assets whose async load is in flight; drives the "loading" UI.
    pub loading_assets: BTreeSet<AssetId>,
    /// Assets whose preview has been produced by the host.
    pub preview_ready: BTreeSet<AssetId>,
}

impl ProjectState {
    pub fn asset(&self, id: AssetId) -> Option<&AssetEntry> {
        self.assets.iter().find(|asset| asset.id == id)
    }

    /// Next free asset id. The real scanner owns ids; reducers in this prototype
    /// allocate deterministically so create-asset stays a pure function.
    pub fn next_asset_id(&self) -> AssetId {
        AssetId(
            self.assets
                .iter()
                .map(|asset| asset.id.0)
                .max()
                .unwrap_or(0)
                + 1,
        )
    }

    /// Demo project for the merged view prototype: nested-ish asset paths so the
    /// project panel can derive a folder hierarchy from paths alone (no parent
    /// pointers and no folder entries in the core model).
    pub fn demo() -> Self {
        let assets = [
            ("assets/models/hero.scene", AssetKind::Scene),
            ("assets/models/crate.material", AssetKind::Material),
            ("assets/models/broken.bin", AssetKind::Binary),
            ("assets/textures/noise.texture", AssetKind::Texture),
            ("assets/textures/hero.png", AssetKind::Texture),
            ("assets/scenes/level01.kairos", AssetKind::Scene),
            ("assets/config/settings.toml", AssetKind::Toml),
            ("assets/audio/theme.ogg", AssetKind::Audio),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (path, kind))| AssetEntry {
            id: AssetId(index as u64 + 1),
            path: path.to_string(),
            kind,
        })
        .collect();
        Self {
            assets,
            ..Self::default()
        }
    }
}

/// What is selected and what document is active. Commands resolve their target
/// through this slice instead of carrying ids around.
#[derive(Clone, Debug, Default)]
pub struct SelectionState {
    pub selected_asset: Option<AssetId>,
    pub active_document: Option<DocumentId>,
    /// Scene object picked in the viewport (prototype stand-in for an entity).
    pub selected_scene_object: Option<SceneObjectId>,
}

/// Open documents, keyed by id.
#[derive(Clone, Debug, Default)]
pub struct DocumentsState {
    docs: BTreeMap<DocumentId, Document>,
    next_id: u64,
}

impl DocumentsState {
    /// Open a document. Returns the fresh id; the caller (reducer) decides what
    /// else to update, which keeps this slice free of cross-slice rules.
    pub fn create(&mut self, path: String, kind: DocumentKind, content: EditData) -> DocumentId {
        self.next_id += 1;
        let id = DocumentId(self.next_id);
        self.docs.insert(id, Document::new(id, path, kind, content));
        id
    }

    pub fn get(&self, id: DocumentId) -> Option<&Document> {
        self.docs.get(&id)
    }

    pub fn get_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
        self.docs.get_mut(&id)
    }

    pub fn remove(&mut self, id: DocumentId) -> Option<Document> {
        self.docs.remove(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Document> {
        self.docs.values()
    }

    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    pub fn dirty_count(&self) -> usize {
        self.docs.values().filter(|doc| doc.dirty).count()
    }

    /// Demo helper (tests/tools only): create a material document without going
    /// through `Action::Document(Open)`. The real flow is always an Action.
    pub fn demo_material(&mut self) -> DocumentId {
        self.create(
            "assets/models/crate.material".to_string(),
            DocumentKind::Material,
            EditData::Material(MaterialData::default()),
        )
    }

    /// Demo helper (tests/tools only): create a Toml document.
    pub fn demo_text(&mut self) -> DocumentId {
        self.create(
            "assets/config/settings.toml".to_string(),
            DocumentKind::Toml,
            EditData::Toml("# demo document\n".to_string()),
        )
    }
}

/// Docking layout as plain data (see `DockState` in kairos_editor today).
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutState {
    pub open_panels: BTreeSet<PanelId>,
    pub focus: PanelId,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            open_panels: [
                PanelId::PROJECT,
                PanelId::INSPECTOR,
                PanelId::VIEWPORT,
                PanelId::CONSOLE,
            ]
            .into_iter()
            .collect(),
            focus: PanelId::VIEWPORT,
        }
    }
}

impl LayoutState {
    pub fn is_open(&self, panel: PanelId) -> bool {
        self.open_panels.contains(&panel)
    }

    pub fn open(&mut self, panel: PanelId) {
        self.open_panels.insert(panel);
    }

    pub fn close(&mut self, panel: PanelId) {
        self.open_panels.remove(&panel);
        if self.focus == panel {
            self.focus = self
                .open_panels
                .iter()
                .next()
                .copied()
                .unwrap_or(PanelId::VIEWPORT);
        }
    }
}

/// The one and only state tree the editor core owns.
#[derive(Clone, Debug)]
pub struct EditorState {
    pub project: ProjectState,
    pub selection: SelectionState,
    pub documents: DocumentsState,
    pub layout: LayoutState,
    pub modal: ModalState,
    /// Viewports the editor knows about; GPU state and camera pose stay on the
    /// host (see [`crate::viewport`]).
    pub viewports: ViewportState,
    /// Host requests that have been emitted but not completed yet. This is what
    /// lets `App::complete` route an async completion back to an `Action`.
    pub inflight: BTreeMap<EffectTicket, RequestKind>,
    next_ticket: EffectTicket,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            project: ProjectState::default(),
            selection: SelectionState::default(),
            documents: DocumentsState::default(),
            layout: LayoutState::default(),
            modal: ModalState::default(),
            viewports: ViewportState::default(),
            inflight: BTreeMap::new(),
            next_ticket: EffectTicket(0),
        }
    }
}

impl EditorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a new request ticket. Deterministic: called only from reducers.
    pub fn alloc_ticket(&mut self) -> EffectTicket {
        self.next_ticket = EffectTicket(self.next_ticket.0 + 1);
        self.next_ticket
    }

    /// Allocate a ticket *and* remember what kind of completion it expects.
    pub fn begin_request(&mut self, kind: RequestKind) -> EffectTicket {
        let ticket = self.alloc_ticket();
        self.inflight.insert(ticket, kind);
        ticket
    }

    pub fn is_inflight(&self, ticket: EffectTicket) -> bool {
        self.inflight.contains_key(&ticket)
    }

    pub fn active_document(&self) -> Option<&Document> {
        self.selection
            .active_document
            .and_then(|id| self.documents.get(id))
    }
}
