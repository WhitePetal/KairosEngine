use rapier3d::{
    dynamics::{
        CCDSolver, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet,
        RigidBodySet,
    },
    geometry::{ColliderSet, DefaultBroadPhase, NarrowPhase},
    pipeline::PhysicsPipeline,
};

use crate::{ecs::world::World, math::float3, physics::rigid_body::RigidBody, spatial::Transform};

pub mod collider;
pub mod rigid_body;

pub struct PhysicsEngine {
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    gravity: float3,
    integration_parameters: IntegrationParameters,
    island_manager: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
    physics_hooks: (),
    event_handler: (),
    physics_pipeline: PhysicsPipeline,
    accumulator: f32,
}

impl PhysicsEngine {
    pub fn new() -> Self {
        let rigid_body_set = RigidBodySet::new();
        let collider_set = ColliderSet::new();

        let gravity = float3::new(0.0, -9.81, 0.0);
        let integration_parameters = IntegrationParameters::default();
        let island_manager = IslandManager::new();
        let broad_phase = DefaultBroadPhase::new();
        let narrow_phase = NarrowPhase::new();
        let impulse_joint_set = ImpulseJointSet::new();
        let multibody_joint_set = MultibodyJointSet::new();
        let ccd_solver = CCDSolver::new();
        let physics_hooks = ();
        let event_handler = ();

        let physics_pipeline = PhysicsPipeline::new();

        Self {
            rigid_body_set,
            collider_set,
            gravity,
            integration_parameters,
            island_manager,
            broad_phase,
            narrow_phase,
            impulse_joint_set,
            multibody_joint_set,
            ccd_solver,
            physics_hooks,
            event_handler,
            physics_pipeline,
            accumulator: 0.0,
        }
    }

    pub fn update(&mut self, word: &mut World, delta_time: f32) {
        const FIXED_DT: f32 = 1.0 / 60.0;
        const MAX_ACCUMULATOR: f32 = 0.25;

        self.accumulator += delta_time;
        if self.accumulator > MAX_ACCUMULATOR {
            self.accumulator = MAX_ACCUMULATOR;
        }

        while self.accumulator >= FIXED_DT {
            self.physics_pipeline.step(
                self.gravity.into(),
                &self.integration_parameters,
                &mut self.island_manager,
                &mut self.broad_phase,
                &mut self.narrow_phase,
                &mut self.rigid_body_set,
                &mut self.collider_set,
                &mut self.impulse_joint_set,
                &mut self.multibody_joint_set,
                &mut self.ccd_solver,
                &self.physics_hooks,
                &self.event_handler,
            );

            self.accumulator -= FIXED_DT;
        }

        for (mut transform, rigid_body) in
            word.query_mut::<(&mut Transform, &RigidBody)>().into_iter()
        {
            let rigid_body = &self.rigid_body_set[rigid_body.handle];
            transform.position = rigid_body.translation().into();
            transform.rotation = (*rigid_body.rotation()).into();
        }
    }
}
