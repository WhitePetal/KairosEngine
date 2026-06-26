use rapier3d::{
    dynamics::{RigidBodyBuilder, RigidBodyHandle},
    geometry::ColliderBuilder,
};

use crate::{
    ecs::component::Component,
    physics::{PhysicsEngine, collider::Collider},
};

pub struct RigidBody {
    pub handle: RigidBodyHandle,
}
impl Component for RigidBody {}

impl RigidBody {
    pub fn with_sphere_collider(engine: &mut PhysicsEngine, radius: f32) -> (Self, Collider) {
        let rigid_body = RigidBodyBuilder::dynamic().build();
        let rigid_body_handle = engine.rigid_body_set.insert(rigid_body);
        let collider = ColliderBuilder::ball(radius).restitution(0.7).build();
        let collider_handle = engine.collider_set.insert_with_parent(
            collider,
            rigid_body_handle,
            &mut engine.rigid_body_set,
        );
        (
            Self {
                handle: rigid_body_handle,
            },
            Collider {
                handle: collider_handle,
            },
        )
    }
}
