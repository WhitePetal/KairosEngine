//! The physics subsystem: a [`PhysicsEngine`] World resource plus the
//! `RigidBody` / `Collider` components that point into it.
//!
//! [`install`] is the single bootstrap entry point (bevy-plugin style): it
//! inserts the resource, so every [`World`] that went through
//! [`Engine::new`](crate::kairos_editor::Engine::new) is guaranteed to carry
//! physics. The rapier types themselves never leave this module — engine code
//! talks to the resource through its eager `insert_*` constructors, and
//! entities carry nothing but private handles.

use rapier3d::{
    dynamics::{
        CCDSolver, CoefficientCombineRule, ImpulseJointSet, IntegrationParameters, IslandManager,
        MultibodyJointSet, RigidBodyBuilder, RigidBodySet,
    },
    geometry::{ColliderBuilder, ColliderSet, DefaultBroadPhase, NarrowPhase},
    math::{Pose, Rotation, Vector3},
    pipeline::PhysicsPipeline,
};

use crate::{
    math::{float3, quaternion},
    physics::{
        collider::{Collider, ColliderMaterial},
        rigid_body::RigidBody,
    },
};
use kairos_ecs::{resource::Resource, world::World};
use kairos_transform::LocalTransform;

pub mod collider;
pub mod rigid_body;

/// The engine's rapier world, stored as a World resource.
///
/// Holds the rapier sets plus the pipeline state needed to step them. The sets
/// and pipeline are private: callers build and destroy rapier objects through
/// the eager `insert_*` methods below, so rapier types stay inside the module.
#[derive(Resource)]
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
        }
    }

    /// Eagerly inserts a dynamic sphere body together with its child ball
    /// collider, placing the body's pose at `initial` once, at construction.
    ///
    /// The returned pair belongs on the same entity: the collider hangs off the
    /// body at the origin, so the body's pose is also the sphere's pose. The
    /// sphere's `material.restitution` uses a `Max` combine rule, so the
    /// restitution stays `material.restitution` against a default (zero
    /// restitution) surface instead of being averaged down.
    pub fn insert_movable_sphere(
        &mut self,
        radius: f32,
        material: ColliderMaterial,
        initial: LocalTransform,
    ) -> (RigidBody, Collider) {
        let rigid_body = RigidBodyBuilder::dynamic()
            .pose(pose_from(initial))
            .build();
        let rigid_body_handle = self.rigid_body_set.insert(rigid_body);

        let collider = ColliderBuilder::ball(radius)
            .restitution(material.restitution)
            .restitution_combine_rule(CoefficientCombineRule::Max)
            .build();
        let collider_handle = self.collider_set.insert_with_parent(
            collider,
            rigid_body_handle,
            &mut self.rigid_body_set,
        );

        (
            RigidBody {
                handle: rigid_body_handle,
            },
            Collider {
                handle: collider_handle,
            },
        )
    }

    /// Eagerly inserts a standalone immovable box collider — no body — placed
    /// at `initial` once, at construction.
    ///
    /// A collider without a parent body is a static object: rapier never moves
    /// it, which is exactly the ground/plane shape.
    pub fn insert_immovable_box(
        &mut self,
        half_extents: float3,
        initial: LocalTransform,
    ) -> Collider {
        let collider = ColliderBuilder::cuboid(
            half_extents.x(),
            half_extents.y(),
            half_extents.z(),
        )
        .position(pose_from(initial))
        .build();

        Collider {
            handle: self.collider_set.insert(collider),
        }
    }
}

/// Installs the physics resource onto `world`.
///
/// Registered in `Engine::new` right after
/// [`schedule::install`](crate::kairos_editor::schedule::install), so physics is
/// present for the whole lifetime of an `Engine`. System and hook registration
/// will join it here as the corresponding tickets land.
pub(crate) fn install(world: &mut World) {
    log::debug!("installing the physics resource");
    world.insert_resource(PhysicsEngine::new());
}

/// Builds a rapier pose from an entity's initial transform.
///
/// Only position and rotation cross over — scale has no rapier counterpart, so
/// the collider's world-space size is passed in pre-scaled by the caller.
fn pose_from(initial: LocalTransform) -> Pose {
    Pose::from_parts(
        to_rapier_vec3(initial.position),
        to_rapier_rotation(initial.rotation),
    )
}

// Explicit engine ↔ rapier conversions (the old `From` impls were removed
// when math moved to `kairos_math`; orphan rules forbid them there, #139).
fn to_rapier_vec3(v: float3) -> Vector3 {
    Vector3::new(v.x(), v.y(), v.z())
}

fn to_rapier_rotation(q: quaternion) -> Rotation {
    Rotation::from_xyzw(q.0.x(), q.0.y(), q.0.z(), q.0.w())
}

fn to_float3(v: Vector3) -> float3 {
    float3::new(v.x, v.y, v.z)
}

fn quat_from_rapier(r: &Rotation) -> quaternion {
    quaternion::new(r.x, r.y, r.z, r.w)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_ecs::world::World;

    fn initial_transform() -> LocalTransform {
        LocalTransform::new(
            float3::new(1.0, 2.0, 3.0),
            quaternion::from_euler(float3::new(0.3, -0.5, 0.7)),
            float3::ONE,
        )
    }

    #[test]
    fn install_inserts_the_physics_resource() {
        let mut world = World::new();
        install(&mut world);
        assert!(
            world.get_resource::<PhysicsEngine>().is_some(),
            "install must leave a PhysicsEngine in the world"
        );
    }

    #[test]
    fn insert_movable_sphere_resolves_and_places_the_body() {
        let mut physics = PhysicsEngine::new();
        let initial = initial_transform();

        let (body, collider) = physics.insert_movable_sphere(
            0.5,
            ColliderMaterial { restitution: 0.8 },
            initial,
        );

        let rapier_body = physics
            .rigid_body_set
            .get(body.handle)
            .expect("the body handle must resolve in the rigid-body set");
        assert_eq!(to_float3(rapier_body.translation()), initial.position);
        assert_eq!(quat_from_rapier(rapier_body.rotation()), initial.rotation);

        let rapier_collider = physics
            .collider_set
            .get(collider.handle)
            .expect("the collider handle must resolve in the collider set");
        assert_eq!(
            rapier_collider.parent(),
            Some(body.handle),
            "the sphere collider must hang off the dynamic body"
        );
    }

    #[test]
    fn insert_immovable_box_resolves_without_a_body() {
        let mut physics = PhysicsEngine::new();
        let initial = initial_transform();

        let collider =
            physics.insert_immovable_box(float3::new(100.0, 1.0, 100.0), initial);

        let rapier_collider = physics
            .collider_set
            .get(collider.handle)
            .expect("the collider handle must resolve in the collider set");
        assert_eq!(rapier_collider.parent(), None, "a box must be standalone");
        assert_eq!(to_float3(rapier_collider.translation()), initial.position);
        assert_eq!(quat_from_rapier(&rapier_collider.rotation()), initial.rotation);
    }

    #[test]
    fn components_spawn_on_an_entity() {
        let mut physics = PhysicsEngine::new();
        let (body, collider) = physics.insert_movable_sphere(
            0.5,
            ColliderMaterial { restitution: 0.8 },
            initial_transform(),
        );

        let mut world = World::new();
        let entity = world.spawn((initial_transform(), body, collider)).id();

        assert!(world.get::<RigidBody>(entity).is_some());
        assert!(world.get::<Collider>(entity).is_some());
    }
}
