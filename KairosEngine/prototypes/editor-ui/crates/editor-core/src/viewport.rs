//! Viewport domain slice: the plain data the reducer owns about rendered views.
//!
//! This module is the prototype stand-in for `core::ViewportState` in the target
//! architecture (`docs/plan/editor-ui-architecture/README.md` §3.5/§4.3).
//!
//! ## Ownership
//!
//! | State | Owner |
//! |---|---|
//! | open viewports, pixel size, bind/pick request slots | this slice (reducer-owned) |
//! | wgpu texture, egui `TextureId`, camera pose, depth buffer | host (`ViewportHost`; the real editor keeps the camera in the ECS `World`) |
//! | panel visibility / focus | [`crate::state::LayoutState`] |
//!
//! **No camera pose here.** In the real editor the orbit state is the
//! `EditorCameraController` component in the `World` and the render pipeline owns
//! GPU resources; duplicating either in the store would create two sources of
//! truth. Camera intents therefore travel as data ([CameraInput]) inside
//! `Effect::ViewportCameraInput`, are applied by the host before the engine
//! update, and never echo back into this slice.
//!
//! ## Async contract
//!
//! A panel that becomes visible emits `ViewportAction::Ensure` with its current
//! pixel size. The first `Ensure` opens a `RequestKind::BindViewport` ticket and
//! emits `Effect::CreateViewport`; the host answers with
//! `EffectOutcome::ViewportBound` / `ViewportBindFailed`, and this reducer (not the
//! host) validates the ticket. Picking works the same way with
//! `RequestKind::PickSceneObject` / `EffectOutcome::SceneObjectPicked`.

use std::collections::BTreeMap;

use crate::effect::EffectTicket;

/// Stable identity of a rendered viewport.
///
/// One id per render target: scene, game, material preview, ... The prototype
/// only uses [`ViewportId::SCENE`], but the reducer is keyed by id from day one so
/// adding the game/preview views is data, not a structural change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewportId(pub u32);

impl ViewportId {
    /// The editor scene viewport (the editor camera's view).
    pub const SCENE: Self = Self(1);
}

/// Stable id of a scene object that can be selected by picking.
///
/// Prototype stand-in for `Entity`: the demo viewport reports object ids 1
/// (cube) and 2 (grid plane). In the real editor picking returns an `Entity`
/// from the engine, which the editor maps to its own stable selection id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SceneObjectId(pub u64);

/// Editor-side description of one viewport.
///
/// This is deliberately small: what to render, how big, whether the host bound
/// it, and which request tickets are still in flight. Everything GPU- or
/// camera-shaped lives on the host.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewportInfo {
    /// Requested pixel size (already clamped to at least 1x1).
    pub size: (u32, u32),
    /// The size the host was asked to create; lets a bind completion replay a
    /// resize that happened while the texture was being created.
    pub bind_size: (u32, u32),
    /// Whether the host reported a live render target for this viewport.
    pub bound: bool,
    /// Ticket of the in-flight `Effect::CreateViewport`, if any.
    pub pending_bind: Option<EffectTicket>,
    /// Ticket of the in-flight `Effect::PickViewport`, if any.
    pub pending_pick: Option<EffectTicket>,
    /// Last host bind error; the panel renders it instead of retrying in a loop.
    pub last_error: Option<String>,
}

impl ViewportInfo {
    /// A viewport that is waiting for its bind completion.
    pub fn creating(size: (u32, u32), ticket: EffectTicket) -> Self {
        Self {
            size,
            bind_size: size,
            bound: false,
            pending_bind: Some(ticket),
            pending_pick: None,
            last_error: None,
        }
    }
}

/// Host result of a bind request, kept in the action vocabulary (not a
/// `Result` in `Action`) so it derives `PartialEq` like every other action.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewportBindResult {
    Ok,
    Failed { error: String },
}

/// Camera intent produced by the viewport panel. Plain data: no egui, no wgpu.
///
/// The panel converts raw pointer state into **camera-ready units**; the host
/// applies them without knowing about egui. This keeps the reducer and the host
/// camera contract identical between the prototype and the real editor (the
/// real host maps them onto `kairos_editor::camera::EditorCameraController`).
/// The UI never reads camera state back; it only draws the texture the host
/// rendered.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraInput {
    /// Orbit delta in radians, `[yaw, pitch]`.
    pub orbit: [f32; 2],
    /// Pan delta normalized to the viewport height, `[x, y]` (so the gesture
    /// feels the same at any panel size).
    pub pan: [f32; 2],
    /// Zoom delta; **positive zooms in**, negative zooms out.
    pub zoom: f32,
}

impl CameraInput {
    /// Whether the intent would change anything.
    pub fn is_zero(&self) -> bool {
        self.orbit == [0.0, 0.0] && self.pan == [0.0, 0.0] && self.zoom == 0.0
    }

    /// Merge a second intent into this one (a single frame can carry several
    /// gestures; the host applies one input per viewport per frame at most).
    pub fn merge(&mut self, other: Self) {
        self.orbit[0] += other.orbit[0];
        self.orbit[1] += other.orbit[1];
        self.pan[0] += other.pan[0];
        self.pan[1] += other.pan[1];
        self.zoom += other.zoom;
    }
}

/// All viewports the editor currently knows about, keyed by stable id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewportState {
    views: BTreeMap<ViewportId, ViewportInfo>,
}

impl ViewportState {
    /// Empty store: no viewport has been opened yet.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, viewport: ViewportId) -> Option<&ViewportInfo> {
        self.views.get(&viewport)
    }

    pub fn get_mut(&mut self, viewport: ViewportId) -> Option<&mut ViewportInfo> {
        self.views.get_mut(&viewport)
    }

    pub fn contains(&self, viewport: ViewportId) -> bool {
        self.views.contains_key(&viewport)
    }

    pub fn insert(&mut self, viewport: ViewportId, info: ViewportInfo) -> Option<ViewportInfo> {
        self.views.insert(viewport, info)
    }

    pub fn remove(&mut self, viewport: ViewportId) -> Option<ViewportInfo> {
        self.views.remove(&viewport)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ViewportId, &ViewportInfo)> {
        self.views.iter()
    }

    pub fn len(&self) -> usize {
        self.views.len()
    }

    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }
}
