use rapier3d::geometry::{ColliderBuilder, ColliderHandle};

use crate::{ecs::component::Component, math::float3, physics::PhysicsEngine};

#[derive(Debug, Clone, Copy)]
pub struct ColliderMaterial {
    pub restitution: f32,
}

pub struct Collider {
    pub handle: ColliderHandle,
}
impl Component for Collider {}

impl Collider {
    pub fn box_collider_with_material(
        engine: &mut PhysicsEngine,
        hx: f32,
        hy: f32,
        hz: f32,
        material: ColliderMaterial,
    ) -> Self {
        let collider = ColliderBuilder::cuboid(hx, hy, hz)
            .restitution(material.restitution)
            .build();
        let handle = engine.collider_set.insert(collider);

        Self { handle }
    }

    pub fn sphere_collider_with_material(
        engine: &mut PhysicsEngine,
        radius: f32,
        material: ColliderMaterial,
    ) -> Self {
        let collider = ColliderBuilder::ball(radius)
            .restitution(material.restitution)
            .build();
        let handle = engine.collider_set.insert(collider);
        Self { handle }
    }

    pub fn box_collider(engine: &mut PhysicsEngine, hx: f32, hy: f32, hz: f32) -> Self {
        let collider = ColliderBuilder::cuboid(hx, hy, hz).build();
        let handle = engine.collider_set.insert(collider);

        Self { handle }
    }

    pub fn sphere_collider(engine: &mut PhysicsEngine, radius: f32) -> Self {
        let collider = ColliderBuilder::ball(radius).build();
        let handle = engine.collider_set.insert(collider);
        Self { handle }
    }

    pub fn set_position(&self, engine: &mut PhysicsEngine, position: float3) {
        engine.collider_set[self.handle].set_translation(position.into());
    }
}
