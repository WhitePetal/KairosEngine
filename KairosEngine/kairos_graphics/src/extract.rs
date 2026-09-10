//! The extract rail: the systems that read the frame's final state and refill
//! each view's buffer.
//!
//! They register into the application's extract stage (the `Extract`
//! sub-stage between `PostUpdate` and `Last` in the engine's schedule), passed
//! to [`install`](crate::install). That placement is the contract: every modifier
//! has already run, so nothing here needs to order itself against gameplay,
//! physics or the editor camera controller.
//!
//! One system per view — [`extract_scene_view`], [`extract_game_view`] — and
//! each one owns its view end to end: clear the buffer, derive the camera's
//! projection, then call the drawers that fill it. Nothing writes two views at
//! once, so clearing and filling need no cross-system ordering, each view lists
//! the drawers it wants on its own, and the views stay free to diverge
//! (per-camera culling will make them).

use kairos_ecs::system::{Query, ResMut};
use kairos_transform::LocalTransform;

use crate::{
    camera::{Camera, CameraView},
    drawer,
    lod_mesh_component::LODMesh,
    material_component::MaterialComponent,
    view_port::{GameView, SceneView, ViewResource},
};

/// Clears every camera's derived view state at the top of the stage.
///
/// This is what makes "no stale matrix survives a frame" unconditional. Each
/// view below derives the data of *its* camera, and a camera no view renders
/// from — unbound, or abandoned when a view was pointed elsewhere — would
/// otherwise keep what an earlier frame derived for it, readable by anyone
/// still holding the entity. Back to [`CameraView::default`], i.e. "no view
/// size known yet", exactly as a freshly spawned camera reads.
pub fn reset_camera_views(mut cameras: Query<&mut CameraView>) {
    for mut derived in cameras.iter_mut() {
        *derived = CameraView::default();
    }
}

/// The Scene view's rail: refill the editor camera's frame buffer.
pub fn extract_scene_view(
    mut view: ResMut<SceneView>,
    meshes: Query<(&LocalTransform, &LODMesh, &MaterialComponent)>,
    mut cameras: Query<(&LocalTransform, &Camera, &mut CameraView)>,
) {
    if reset_and_derive(&mut *view, &mut cameras) {
        drawer::draw_meshes(view.draws_mut(), &meshes);
    }
}

/// The Game view's rail: refill the game camera's frame buffer. Same work as
/// [`extract_scene_view`], separate system because the two views own separate
/// buffers — each names the drawers it wants, so they can diverge later.
pub fn extract_game_view(
    mut view: ResMut<GameView>,
    meshes: Query<(&LocalTransform, &LODMesh, &MaterialComponent)>,
    mut cameras: Query<(&LocalTransform, &Camera, &mut CameraView)>,
) {
    if reset_and_derive(&mut *view, &mut cameras) {
        drawer::draw_meshes(view.draws_mut(), &meshes);
    }
}

/// The one implementation of "prepare a view for this frame", instantiated per
/// view: empty the buffer, then write this frame's derived size, aspect and
/// projection onto the camera the view is bound to.
///
/// Returns whether the view renders this frame — `false` for a closed window
/// (`size == 0`), which is also what keeps `width / height` away from a
/// division by zero, and is why the caller's drawers are skipped rather than
/// filling a buffer nothing will read.
///
/// [`reset_camera_views`] has already cleared every camera's derived state, so
/// a view that derives nothing (closed window, no camera) leaves "no view size
/// known yet" behind rather than an older frame's numbers. Every invalid state
/// is a no-op rather than a crash: no camera bound, a binding that outlives its
/// entity, or a camera without a [`CameraView`].
///
/// The `cameras` query is an *index*, not a sweep: the only camera this writes
/// is the one `view.camera()` points at (`get_mut` by entity), so a view can
/// never derive another view's camera — and a camera no view is bound to is
/// written by nobody, staying at its cleared default.
fn reset_and_derive(
    view: &mut impl ViewResource,
    cameras: &mut Query<(&LocalTransform, &Camera, &mut CameraView)>,
) -> bool {
    view.draws_mut().clear();

    let size = view.size();
    if !size.is_renderable() {
        return false;
    }

    if let Some(entity) = view.camera()
        && let Ok((transform, intrinsics, mut derived)) = cameras.get_mut(entity)
    {
        let aspect = size.aspect();
        derived.physical_size = (size.width, size.height);
        derived.aspect = aspect;
        derived.view_projection = Some(intrinsics.get_view_projection_matrix(*transform, aspect));
    }

    true
}

#[cfg(test)]
mod test;
