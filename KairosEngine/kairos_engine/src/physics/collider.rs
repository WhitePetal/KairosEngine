use rapier3d::geometry::{ColliderBuilder, ColliderHandle};

use crate::{ecs::component::Component, physics::PhysicsEngine};

pub struct Collider {
    pub handle: ColliderHandle,
}
impl Component for Collider {}

impl Collider {
    pub fn box_collider(engine: &mut PhysicsEngine, hx: f32, hy: f32, hz: f32) -> Self {
        let collider = ColliderBuilder::cuboid(hx, hy, hz).build();
        let handle = engine.collider_set.insert(collider);

        Self { handle }
    }

    pub fn sphere_collider(engine: &mut PhysicsEngine, radius: f32) -> Self {
        let collider = ColliderBuilder::ball(radius).restitution(0.7).build();
        let handle = engine.collider_set.insert(collider);
        Self { handle }
    }
}
