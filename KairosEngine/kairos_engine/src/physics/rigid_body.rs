use rapier3d::{
    dynamics::{RigidBodyBuilder, RigidBodyHandle},
    geometry::ColliderBuilder,
};

use crate::{
    ecs::component::Component,
    math::float3,
    physics::{PhysicsEngine, collider::ColliderMaterial},
};

pub struct RigidBody {
    pub handle: RigidBodyHandle,
}
impl Component for RigidBody {}

impl RigidBody {
    pub fn with_sphere_collider(engine: &mut PhysicsEngine, radius: f32) -> Self {
        let rigid_body = RigidBodyBuilder::dynamic().build();
        let rigid_body_handle = engine.rigid_body_set.insert(rigid_body);
        let collider = ColliderBuilder::ball(radius).build();
        engine.collider_set.insert_with_parent(
            collider,
            rigid_body_handle,
            &mut engine.rigid_body_set,
        );

        Self {
            handle: rigid_body_handle,
        }
    }

    pub fn with_sphere_collider_with_material(
        engine: &mut PhysicsEngine,
        radius: f32,
        material: ColliderMaterial,
    ) -> Self {
        let rigid_body = RigidBodyBuilder::dynamic().build();
        let rigid_body_handle = engine.rigid_body_set.insert(rigid_body);
        let collider = ColliderBuilder::ball(radius)
            .restitution(material.restitution)
            .build();
        engine.collider_set.insert_with_parent(
            collider,
            rigid_body_handle,
            &mut engine.rigid_body_set,
        );

        Self {
            handle: rigid_body_handle,
        }
    }

    pub fn set_position(&self, engine: &mut PhysicsEngine, position: float3) {
        engine.rigid_body_set[self.handle].set_translation(position.into(), false);
    }
}
