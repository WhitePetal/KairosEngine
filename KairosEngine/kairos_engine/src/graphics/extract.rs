//! The extract rail: the systems that read the frame's final state and refill
//! each view's buffer.
//!
//! They register into [`Extract`](crate::kairos_editor::schedule::Extract), the
//! sub-stage between `PostUpdate` and `Last`. That placement is the contract:
//! every modifier has already run, so nothing here needs to order itself
//! against gameplay, physics or the editor camera controller.
//!
//! Two layers, per the render-rails decision:
//!
//! - the **view rails** in this module ([`reset_camera_views`],
//!   [`extract_scene_view`], [`extract_game_view`]) clear the cameras' derived
//!   state and each view's buffer, then derive the projections. They own the
//!   "who clears what" question;
//! - the **draw producers** ([`mesh`], registered after
//!   [`ResetViewBuffers`]) append entries. Swapping what is drawn is a matter
//!   of which producer systems `graphics::install` registers.

use kairos_ecs::{
    schedule::SystemSet,
    system::{Query, ResMut},
};
use kairos_transform::LocalTransform;

use crate::graphics::{
    camera::{Camera, CameraView},
    view_port::{GameView, SceneView, ViewResource},
};

pub mod mesh;

/// The view-rail systems — reset a view's buffer, then derive its projection.
///
/// Draw producers order themselves **after** this set rather than before a
/// specific rail system, so a new view or a new producer can be added without
/// naming the others. The order is load-bearing, not cosmetic: a producer that
/// ran first would have its entries erased by the reset.
///
/// Hand-rolled like the schedule labels (see
/// [`schedule`](crate::kairos_editor::schedule)): `kairos_ecs` re-exports the
/// `Component` / `Resource` derives but not `SystemSet`, and a one-method trait
/// is not worth a `kairos_ecs_macros` dependency of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResetViewBuffers;

impl SystemSet for ResetViewBuffers {
    fn dyn_clone(&self) -> Box<dyn SystemSet> {
        Box::new(*self)
    }
}

/// Clears every camera's derived view state at the top of the stage.
///
/// This is what makes "no stale matrix survives a frame" unconditional: each
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

/// The Scene view's rail: empty its buffer and derive this frame's projection
/// onto the editor camera entity.
pub fn extract_scene_view(
    mut view: ResMut<SceneView>,
    mut cameras: Query<(&LocalTransform, &Camera, &mut CameraView)>,
) {
    extract_view(&mut *view, &mut cameras);
}

/// The Game view's rail. Same work as [`extract_scene_view`], separate system
/// because the two views own separate buffers.
pub fn extract_game_view(
    mut view: ResMut<GameView>,
    mut cameras: Query<(&LocalTransform, &Camera, &mut CameraView)>,
) {
    extract_view(&mut *view, &mut cameras);
}

/// The one implementation of "extract a view", instantiated per view.
///
/// Clears the view's buffer and writes this frame's derived size, aspect and
/// projection onto the camera it is bound to. [`reset_camera_views`] has
/// already cleared every camera's derived state, so a view that derives nothing
/// (closed window, no camera) leaves "no view size known yet" behind rather
/// than an older frame's numbers. Every invalid state is a no-op rather than a
/// crash: no camera bound, a binding that outlives its entity, a camera without
/// a [`CameraView`], or a closed window (`size == 0`, which is also what keeps
/// `width / height` away from a division by zero).
fn extract_view(
    view: &mut impl ViewResource,
    cameras: &mut Query<(&LocalTransform, &Camera, &mut CameraView)>,
) {
    let camera = view.camera();
    view.draws_mut().clear();

    let Some(entity) = camera else { return };
    let Ok((transform, intrinsics, mut derived)) = cameras.get_mut(entity) else {
        return;
    };

    let size = view.size();
    if !size.is_renderable() {
        return;
    }

    let aspect = size.aspect();
    derived.physical_size = (size.width, size.height);
    derived.aspect = aspect;
    derived.view_projection = Some(intrinsics.get_view_projection_matrix(*transform, aspect));
}

#[cfg(test)]
mod test;
