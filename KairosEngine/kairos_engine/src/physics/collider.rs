use rapier3d::geometry::ColliderHandle;

use kairos_ecs::component::Component;

/// Collider parameters passed when building a shape.
#[derive(Debug, Clone, Copy)]
pub struct ColliderMaterial {
    pub restitution: f32,
}

/// An entity's collision shape: a move-only, handle-only [`Component`].
///
/// The handle is the entity's credential of ownership over exactly one rapier
/// collider. It is kept crate-private — engine code talks to colliders through
/// [`PhysicsEngine`](super::PhysicsEngine) instead of reaching into rapier.
///
/// A `Collider` is either standalone (an immovable body-less shape) or attached
/// to a [`RigidBody`](super::rigid_body::RigidBody); the two are told apart
/// structurally by the presence of a `RigidBody`, never by a marker field.
///
/// Deliberately neither `Copy`/`Clone` nor `Default`/serde: a rapier collider
/// has a single owner, so duplicating or default-constructing the credential
/// would alias or orphan the object. See issue #152.
#[derive(Component, Debug, PartialEq)]
pub struct Collider {
    pub(crate) handle: ColliderHandle,
}
