//! Viewport reducers: keep the store's view of render targets coherent.

use crate::action::ViewportAction;
use crate::effect::{Effect, RequestKind};
use crate::state::EditorState;
use crate::viewport::{CameraInput, ViewportBindResult, ViewportId, ViewportInfo};

/// A zero-sized render target is invalid; panels are clamped before they reach
/// the host.
const MIN_EXTENT: u32 = 1;

pub fn reduce(state: &mut EditorState, action: ViewportAction) -> Vec<Effect> {
    match action {
        ViewportAction::Ensure {
            viewport,
            width,
            height,
        } => ensure(
            state,
            viewport,
            (width.max(MIN_EXTENT), height.max(MIN_EXTENT)),
        ),
        ViewportAction::Orbit { viewport, delta } => camera(
            state,
            viewport,
            CameraInput {
                orbit: delta,
                ..CameraInput::default()
            },
        ),
        ViewportAction::Pan { viewport, delta } => camera(
            state,
            viewport,
            CameraInput {
                pan: delta,
                ..CameraInput::default()
            },
        ),
        ViewportAction::Zoom { viewport, delta } => camera(
            state,
            viewport,
            CameraInput {
                zoom: delta,
                ..CameraInput::default()
            },
        ),
        ViewportAction::Pick { viewport, ndc } => pick(state, viewport, ndc),
        ViewportAction::BindCompleted {
            viewport,
            ticket,
            result,
        } => bind_completed(state, viewport, ticket, result),
        ViewportAction::PickCompleted {
            viewport,
            ticket,
            object,
        } => pick_completed(state, viewport, ticket, object),
    }
}

/// Idempotent "this viewport is visible at this size" intent.
///
/// * not known yet -> allocate a bind ticket and emit [`Effect::CreateViewport`]
/// * known + same size -> no-op (the panel emits this every frame; must be cheap)
/// * known + different size + bound -> [`Effect::SyncViewportSize`]
/// * known + different size + bind in flight -> remember the size; the bind
///   completion replays it as a resize (target was created at the old size)
fn ensure(state: &mut EditorState, viewport: ViewportId, size: (u32, u32)) -> Vec<Effect> {
    let Some(info) = state.viewports.get(viewport) else {
        let ticket = state.begin_request(RequestKind::BindViewport { viewport });
        state
            .viewports
            .insert(viewport, ViewportInfo::creating(size, ticket));
        return vec![Effect::CreateViewport {
            ticket,
            viewport,
            width: size.0,
            height: size.1,
        }];
    };
    if info.size == size {
        return Vec::new();
    }
    let bound = info.bound;
    if let Some(info) = state.viewports.get_mut(viewport) {
        info.size = size;
        info.last_error = None;
    }
    if bound {
        vec![Effect::SyncViewportSize {
            viewport,
            width: size.0,
            height: size.1,
        }]
    } else {
        Vec::new()
    }
}

/// Camera intents are only forwarded once the render target exists; before that
/// there is nothing to orbit and the host camera should stay untouched.
fn camera(state: &EditorState, viewport: ViewportId, input: CameraInput) -> Vec<Effect> {
    if input.is_zero() {
        return Vec::new();
    }
    match state.viewports.get(viewport) {
        Some(info) if info.bound => vec![Effect::ViewportCameraInput { viewport, input }],
        _ => Vec::new(),
    }
}

/// One pick request per viewport at a time; a second click while the first is
/// in flight is dropped rather than queued (interaction latency matters more
/// than completeness, and the host answers in the same frame in the prototype).
fn pick(state: &mut EditorState, viewport: ViewportId, ndc: [f32; 2]) -> Vec<Effect> {
    let Some(info) = state.viewports.get(viewport) else {
        return Vec::new();
    };
    if !info.bound || info.pending_pick.is_some() {
        return Vec::new();
    }
    let ticket = state.begin_request(RequestKind::PickSceneObject { viewport });
    if let Some(info) = state.viewports.get_mut(viewport) {
        info.pending_pick = Some(ticket);
    }
    vec![Effect::PickViewport {
        ticket,
        viewport,
        ndc,
    }]
}

fn bind_completed(
    state: &mut EditorState,
    viewport: ViewportId,
    ticket: crate::effect::EffectTicket,
    result: ViewportBindResult,
) -> Vec<Effect> {
    let Some(info) = state.viewports.get_mut(viewport) else {
        return Vec::new();
    };
    if info.pending_bind != Some(ticket) {
        // Stale/duplicate completion: the host contract was violated, ignore.
        return Vec::new();
    }
    info.pending_bind = None;
    match result {
        ViewportBindResult::Ok => {
            info.bound = true;
            info.last_error = None;
            // The panel may have resized while the target was being created;
            // the host built it at bind_size, so replay the current size.
            if info.size != info.bind_size {
                let (width, height) = info.size;
                return vec![Effect::SyncViewportSize {
                    viewport,
                    width,
                    height,
                }];
            }
            Vec::new()
        }
        ViewportBindResult::Failed { error } => {
            info.last_error = Some(error);
            Vec::new()
        }
    }
}

fn pick_completed(
    state: &mut EditorState,
    viewport: ViewportId,
    ticket: crate::effect::EffectTicket,
    object: Option<crate::viewport::SceneObjectId>,
) -> Vec<Effect> {
    let Some(info) = state.viewports.get_mut(viewport) else {
        return Vec::new();
    };
    if info.pending_pick != Some(ticket) {
        return Vec::new();
    }
    info.pending_pick = None;
    // A pick that hits nothing clears the scene selection, like every 3D editor.
    state.selection.selected_scene_object = object;
    Vec::new()
}
