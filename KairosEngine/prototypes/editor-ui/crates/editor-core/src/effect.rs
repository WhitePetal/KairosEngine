//! Effects are *data*: reducers describe host work, they never perform it.
//!
//! The host drains `Effect`s, does IO/GPU/thread work, and calls
//! `App::complete(ticket, outcome)` when an async request finishes.

use crate::state::{AssetId, DocumentId, LayoutState, PanelId};
use crate::viewport::{CameraInput, SceneObjectId, ViewportId};

/// Identifies one in-flight host request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectTicket(pub u64);

/// Alias kept for the "request id" vocabulary used in the design discussion.
pub type RequestId = EffectTicket;

/// What kind of completion a ticket expects. Stored in `EditorState::inflight`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    SaveDocument {
        document: DocumentId,
    },
    LoadAsset {
        asset: AssetId,
    },
    SpawnPreview {
        asset: AssetId,
    },
    /// A viewport render target is being created by the host.
    BindViewport {
        viewport: ViewportId,
    },
    /// A pick request is in flight (CPU ray cast in the prototype, GPU id
    /// buffer / engine query in production).
    PickSceneObject {
        viewport: ViewportId,
    },
}

/// Host work described as plain data.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    SaveDocument {
        ticket: EffectTicket,
        document: DocumentId,
        bytes: Vec<u8>,
    },
    LoadAsset {
        ticket: EffectTicket,
        asset: AssetId,
        path: String,
    },
    SpawnPreview {
        ticket: EffectTicket,
        asset: AssetId,
    },
    /// Ask the host to move egui keyboard/scroll focus to a panel.
    FocusPanel {
        panel: PanelId,
    },
    /// Native OS dialog (error, reveal-in-file-manager, ...).
    ShowNativeDialog {
        title: String,
        message: String,
    },
    Quit,
    /// Persist the dock layout; the data is owned here, the host owns the file.
    PersistLayout {
        layout: LayoutState,
    },
    /// Persist the project/asset list after a plain project mutation. The host
    /// owns the file; the data stays in `ProjectState` (same philosophy as
    /// `PersistLayout`).
    PersistProject,

    // --- viewport / GPU ---------------------------------------------------
    /// Create the host render target for a viewport. In the real editor this is
    /// the "open scene tab -> create view + bind its attachment to egui" path.
    CreateViewport {
        ticket: EffectTicket,
        viewport: ViewportId,
        width: u32,
        height: u32,
    },
    /// Resize an existing host render target (panel/dock/DPI size change).
    SyncViewportSize {
        viewport: ViewportId,
        width: u32,
        height: u32,
    },
    /// Drop the render target (panel closed). The host frees the egui texture.
    DestroyViewport {
        viewport: ViewportId,
    },
    /// Apply a camera intent to the host camera controller. Must run before the
    /// engine update / scene render in the real frame loop (one-frame latency is
    /// accepted and explicit; see the design doc).
    ViewportCameraInput {
        viewport: ViewportId,
        input: CameraInput,
    },
    /// Ray-pick a scene object for a viewport.
    PickViewport {
        ticket: EffectTicket,
        viewport: ViewportId,
        ndc: [f32; 2],
    },
}

impl Effect {
    /// Ticket of an async effect, if it has one.
    pub fn ticket(&self) -> Option<EffectTicket> {
        match self {
            Effect::SaveDocument { ticket, .. }
            | Effect::LoadAsset { ticket, .. }
            | Effect::SpawnPreview { ticket, .. }
            | Effect::CreateViewport { ticket, .. }
            | Effect::PickViewport { ticket, .. } => Some(*ticket),
            _ => None,
        }
    }
}

/// Host-reported outcome for a previously emitted async effect.
#[derive(Clone, Debug, PartialEq)]
pub enum EffectOutcome {
    DocumentSaved,
    DocumentSaveFailed {
        error: String,
    },
    AssetLoaded,
    AssetLoadFailed {
        error: String,
    },
    PreviewSpawned,
    /// Host finished creating + binding the viewport render target.
    ViewportBound,
    ViewportBindFailed {
        error: String,
    },
    /// Host finished a pick request (`None` = clicked empty space).
    SceneObjectPicked {
        object: Option<SceneObjectId>,
    },
}
