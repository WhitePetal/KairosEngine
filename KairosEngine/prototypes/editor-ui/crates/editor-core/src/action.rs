//! Actions are plain, owned data: `Clone + Debug + PartialEq`, no handles, no
//! closures, no receivers. That is what makes every reducer a total function
//! over `(&mut EditorState, Action) -> Vec<Effect>`.

use crate::document::{DocumentKind, EditData, MaterialData, SaveResult};
use crate::effect::EffectTicket;
use crate::modal::ConfirmDialog;
use crate::state::{AssetEntry, AssetId, AssetKind, DocumentId, PanelId};
use crate::viewport::{SceneObjectId, ViewportBindResult, ViewportId};

/// Aggregate action: one routing enum instead of a 57-arm "Message" god-enum.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Project(ProjectAction),
    Selection(SelectionAction),
    Document(DocumentAction),
    Layout(LayoutAction),
    Modal(ModalAction),
    Viewport(ViewportAction),
    App(AppAction),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProjectAction {
    /// Async asset scan finished; rows are plain data.
    AssetsDiscovered { assets: Vec<AssetEntry> },
    /// Host finished loading the asset bytes/preview input.
    AssetLoadCompleted {
        asset: AssetId,
        ok: bool,
        error: Option<String>,
    },
    /// Host produced the preview texture / thumbnail.
    PreviewSpawned { asset: AssetId },
    /// Create an asset row (the prototype's create-folder toolbar action uses
    /// `kind: AssetKind::Folder`). `parent` addresses a real folder *asset*;
    /// folders derived from paths by the view layer have no id, so the view
    /// passes `None` for those (known gap, see merged README).
    CreateAsset {
        parent: Option<AssetId>,
        name: String,
        kind: AssetKind,
    },
    /// Delete an asset row. Clears selection/loading flags for it.
    DeleteAsset { asset: AssetId },
    /// Rename an asset; renaming a folder entry also rewrites descendant paths.
    RenameAsset { asset: AssetId, name: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum SelectionAction {
    /// Selecting an asset is a *cross-slice* rule: it also opens/focuses the
    /// inspector and starts an async load.
    SelectAsset {
        asset: AssetId,
    },
    SelectDocument {
        document: DocumentId,
    },
    ClearAssetSelection,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DocumentAction {
    Open {
        path: String,
        kind: DocumentKind,
        content: EditData,
    },
    /// Request close. Dirty documents are intercepted by the root reducer and
    /// turned into a modal instead of closing.
    Close {
        document: DocumentId,
    },
    CloseActive,
    EditText {
        document: DocumentId,
        insert: String,
    },
    SetText {
        document: DocumentId,
        text: String,
    },
    SetMaterial {
        document: DocumentId,
        material: MaterialData,
    },
    /// Emit a `SaveDocument` effect. Dirty stays true until the host completes.
    Save {
        document: DocumentId,
    },
    SaveActive,
    /// Confirm path of the unsaved-close modal: save, then close on success.
    SaveThenClose {
        document: DocumentId,
    },
    /// Host completion, routed by `App::complete` using its ticket.
    SaveCompleted {
        document: DocumentId,
        ticket: EffectTicket,
        result: SaveResult,
    },
    Revert {
        document: DocumentId,
    },
    RevertActive,
    Undo {
        document: DocumentId,
    },
    UndoActive,
    Redo {
        document: DocumentId,
    },
    RedoActive,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayoutAction {
    /// Show/hide the panel. Closing a viewport panel also destroys its render
    /// target (cross-slice rule owned by the layout reducer).
    TogglePanel {
        panel: PanelId,
    },
    ShowPanel {
        panel: PanelId,
    },
    /// Explicit close, used by panel chrome (tab close button). Unlike
    /// `TogglePanel` it is idempotent and never opens a panel by accident.
    ClosePanel {
        panel: PanelId,
    },
    FocusPanel {
        panel: PanelId,
    },
    /// Ask the host to persist the current layout data.
    PersistLayout,
}

/// Intents produced by a viewport panel.
///
/// The panel only observes egui input and the size it was given; it never sees
/// the camera, the wgpu texture, or the engine world. The host turns these into
/// camera/pick work in the explicit frame phases (see the app shell).
#[derive(Clone, Debug, PartialEq)]
pub enum ViewportAction {
    /// The panel is visible and laid out at `width` x `height` physical pixels.
    /// Idempotent: the reducer only acts when the size differs from the recorded
    /// one, or when the viewport has not been bound yet.
    Ensure {
        viewport: ViewportId,
        width: u32,
        height: u32,
    },
    /// Orbit the camera by a delta in radians, `[yaw, pitch]`.
    Orbit {
        viewport: ViewportId,
        delta: [f32; 2],
    },
    /// Pan the camera by a delta normalized to the viewport height.
    Pan {
        viewport: ViewportId,
        delta: [f32; 2],
    },
    /// Zoom by a unitless delta; positive zooms in.
    Zoom { viewport: ViewportId, delta: f32 },
    /// Click-to-pick at normalized device coordinates in [-1, 1], y up.
    Pick { viewport: ViewportId, ndc: [f32; 2] },
    /// Host completion for an `Effect::CreateViewport` request.
    BindCompleted {
        viewport: ViewportId,
        ticket: EffectTicket,
        result: ViewportBindResult,
    },
    /// Host completion for an `Effect::PickViewport` request.
    PickCompleted {
        viewport: ViewportId,
        ticket: EffectTicket,
        object: Option<SceneObjectId>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ModalAction {
    /// `Box` breaks the `Action -> ModalAction -> ConfirmDialog -> Action`
    /// type cycle; without it the aggregate `Action` enum has infinite size.
    ShowConfirm(Box<ConfirmDialog>),
    /// Run the modal's stored `confirm` action.
    Confirm,
    /// Run the modal's stored `cancel` action.
    Cancel,
    /// Close without running anything (target of cancel-style actions).
    Dismiss,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppAction {
    QuitRequested,
    QuitConfirmed,
}
