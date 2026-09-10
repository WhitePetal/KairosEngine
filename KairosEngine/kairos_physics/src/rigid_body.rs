use rapier3d::dynamics::RigidBodyHandle;

use kairos_ecs::component::Component;

/// An entity's physics body: a move-only, handle-only [`Component`].
///
/// The handle is the entity's credential of ownership over exactly one rapier
/// rigid body. It is kept crate-private — engine code talks to bodies through
/// [`PhysicsEngine`](super::PhysicsEngine) instead of reaching into rapier.
///
/// Deliberately neither `Copy`/`Clone` nor `Default`/serde: a rapier body has a
/// single owner, so duplicating or default-constructing the credential would
/// alias or orphan the object. See issue #152.
#[derive(Component, Debug, PartialEq)]
pub struct RigidBody {
    pub(crate) handle: RigidBodyHandle,
}
