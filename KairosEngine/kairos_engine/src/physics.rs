//! The physics subsystem: a [`PhysicsEngine`] World resource plus the
//! `RigidBody` / `Collider` components that point into it.
//!
//! [`install`] is the single bootstrap entry point (bevy-plugin style): it
//! inserts the resource and registers [`physics_step_system`] into
//! `FixedUpdate`, so every [`World`] that went through
//! [`Engine::new`](crate::kairos_editor::Engine::new) is guaranteed to carry
//! physics *and* to advance it once per fixed step. The rapier types themselves
//! never leave this module — engine code talks to the resource through its
//! eager `insert_*` constructors, and entities carry nothing but private
//! handles.

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
    kairos_editor::schedule::FixedUpdate,
    math::{float3, quaternion},
    physics::{
        collider::{Collider, ColliderMaterial},
        rigid_body::RigidBody,
    },
    time::FixedTime,
};
use kairos_ecs::{
    resource::Resource,
    schedule::Schedules,
    system::{Query, Res, ResMut},
    world::World,
};
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

    /// Advances the rapier simulation by one step of `dt` seconds.
    ///
    /// Called by [`physics_step_system`] once per `FixedUpdate` run, always
    /// with the fixed clock's timestep: rapier reads `integration_parameters.dt`
    /// once per `step`, so aligning it here is what keeps simulated physics time
    /// equal to [`FixedTime`] instead of drifting at rapier's own 60 Hz default.
    /// `gravity` and the remaining parameters stay the resource's field
    /// constants — there is deliberately no `Gravity` resource.
    pub(crate) fn step(&mut self, dt: f32) {
        self.integration_parameters.dt = dt;
        self.physics_pipeline.step(
            to_rapier_vec3(self.gravity),
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
    }
}

/// The single `FixedUpdate` system that advances physics one fixed step.
///
/// One run is the whole push → step → pull exchange, in lexical order:
///
/// 1. **push** — every entity's `LocalTransform` position/rotation is written
///    into its rapier body unconditionally, with `wake = true`. An equal-value
///    write is a rapier no-op, so a settled body still goes to sleep; a write
///    that does differ teleports the body and wakes its island on the next step.
/// 2. **step** — [`PhysicsEngine::step`] runs once with [`FixedTime`]'s
///    timestep, so simulated physics time tracks the fixed clock exactly.
/// 3. **pull** — position/rotation are read back from rapier. `scale` is never
///    written: it has no rapier counterpart.
///
/// Only entities carrying a `RigidBody` participate; a collider-only static
/// entity (the ground) is not in the query at all, so it is neither pushed nor
/// pulled. A handle that no longer resolves skips its entity with a `warn!` —
/// the system never panics on a dangling handle, and it never despawns or
/// detaches anything itself.
fn physics_step_system(
    fixed_time: Res<FixedTime>,
    mut physics: ResMut<PhysicsEngine>,
    mut bodies: Query<(&mut LocalTransform, &RigidBody)>,
) {
    for (transform, body) in bodies.iter_mut() {
        let handle = body.handle;
        let Some(rapier_body) = physics.rigid_body_set.get_mut(handle) else {
            log::warn!("skipping dangling rigid body handle {handle:?} on the physics push");
            continue;
        };
        rapier_body.set_position(pose_from(*transform), true);
    }

    physics.step(fixed_time.timestep().as_secs_f32());

    for (mut transform, body) in bodies.iter_mut() {
        let handle = body.handle;
        let Some(rapier_body) = physics.rigid_body_set.get(handle) else {
            log::warn!("skipping dangling rigid body handle {handle:?} on the physics pull");
            continue;
        };
        transform.position = to_float3(rapier_body.translation());
        transform.rotation = quat_from_rapier(rapier_body.rotation());
    }
}

/// Installs the physics resource and its single fixed-step system onto `world`.
///
/// Registered in `Engine::new` right after
/// [`schedule::install`](crate::kairos_editor::schedule::install), so physics is
/// present for the whole lifetime of an `Engine`. This is the one place physics
/// is wired into the schedule: `KairosGame` never registers a physics system.
///
/// # Panics
///
/// If the schedule rails are not installed yet: the [`FixedUpdate`] stage would
/// not exist, and registering into a stage that does not exist is a bootstrap
/// order bug.
pub(crate) fn install(world: &mut World) {
    log::debug!("installing the physics resource and fixed-step system");
    world.insert_resource(PhysicsEngine::new());

    let mut schedules = world.get_resource_or_init::<Schedules>();
    schedules
        .get_mut(FixedUpdate)
        .expect("the `FixedUpdate` schedule must exist: install the schedule rails first")
        .add_systems(physics_step_system);
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
    use kairos_ecs::{entity::Entity, world::World};
    use rapier3d::dynamics::RigidBodyHandle;

    use crate::kairos_editor::schedule;

    fn initial_transform() -> LocalTransform {
        LocalTransform::new(
            float3::new(1.0, 2.0, 3.0),
            quaternion::from_euler(float3::new(0.3, -0.5, 0.7)),
            float3::ONE,
        )
    }

    #[test]
    fn install_inserts_the_physics_resource() {
        let world = boot();
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

    // -----------------------------------------------------------------------
    // The fixed-step system: push → step → pull through `FixedUpdate`
    // -----------------------------------------------------------------------

    /// The number of fixed steps the falling tests run: one simulated second at
    /// the default 64 Hz clock, far more than free fall needs to cross the
    /// one-metre margin below.
    const STEPS: usize = 64;

    /// The ball must end at least this far below its start, so a system that
    /// never steps cannot pass by accident.
    const FALL_MARGIN: f32 = 1.0;

    /// Boots the schedule rails and installs physics — the same order
    /// `Engine::new` uses, so the `FixedUpdate` content schedule exists by the
    /// time `install` registers into it.
    fn boot() -> World {
        let mut world = World::new();
        schedule::install(&mut world);
        install(&mut world);
        world
    }

    /// Runs the `FixedUpdate` schedule once — one physics step. The per-frame
    /// driver is bypassed deliberately: its step count depends on accumulated
    /// virtual time, while this exercises the step system deterministically.
    fn run_fixed_step(world: &mut World) {
        world.run_schedule(FixedUpdate);
        world.clear_trackers();
    }

    fn transform_of(world: &World, entity: Entity) -> LocalTransform {
        *world
            .get::<LocalTransform>(entity)
            .expect("the entity carries a transform")
    }

    /// Builds a standalone immovable box through the resource — no `RigidBody`,
    /// like the game ground — and spawns it at `transform`.
    fn spawn_ground(world: &mut World, transform: LocalTransform) -> Entity {
        let collider = world
            .resource_mut::<PhysicsEngine>()
            .insert_immovable_box(float3::new(100.0, 1.0, 100.0), transform);
        world.spawn((transform, collider)).id()
    }

    /// Builds a dynamic sphere through the resource and spawns it with both
    /// physics components at `transform`.
    fn spawn_ball(world: &mut World, transform: LocalTransform) -> Entity {
        let (body, collider) = {
            let mut physics = world.resource_mut::<PhysicsEngine>();
            physics.insert_movable_sphere(0.5, ColliderMaterial { restitution: 0.2 }, transform)
        };
        world.spawn((transform, body, collider)).id()
    }

    /// The acceptance capstone: a dynamic ball spawned over an immovable ground
    /// falls under gravity across fixed steps, while the ground — a
    /// collider-only static entity — is never written.
    #[test]
    fn a_dynamic_ball_falls_while_the_ground_stays_put() {
        let mut world = boot();

        let ground_pose = LocalTransform::new(float3::ZERO, quaternion::IDENTITY, float3::ONE);
        let ground = spawn_ground(&mut world, ground_pose);

        let ball_start = LocalTransform::new(
            float3::new(0.0, 5.0, 0.0),
            quaternion::IDENTITY,
            float3::ONE,
        );
        let ball = spawn_ball(&mut world, ball_start);

        for _ in 0..STEPS {
            run_fixed_step(&mut world);
        }

        let ball_end = transform_of(&world, ball);
        assert!(
            ball_end.position.y() < ball_start.position.y() - FALL_MARGIN,
            "the ball must fall significantly: y {} -> {}",
            ball_start.position.y(),
            ball_end.position.y()
        );
        assert_eq!(
            transform_of(&world, ground),
            ground_pose,
            "a collider-only entity has no `RigidBody`, so physics must not touch its transform"
        );
    }

    /// Push and pull move position/rotation only: `scale` is engine-side state
    /// with no rapier counterpart, so no fixed step may rewrite it.
    #[test]
    fn synchronisation_never_touches_scale() {
        let mut world = boot();

        let scale = float3::new(2.0, 3.0, 4.0);
        let ball_start =
            LocalTransform::new(float3::new(0.0, 5.0, 0.0), quaternion::IDENTITY, scale);
        let ball = spawn_ball(&mut world, ball_start);

        for _ in 0..STEPS {
            run_fixed_step(&mut world);
        }

        let ball_end = transform_of(&world, ball);
        assert_eq!(ball_end.scale, scale, "scale must survive every push and pull");
        assert!(
            ball_end.position.y() < ball_start.position.y(),
            "the same steps moved the ball, so the run did reach physics"
        );
    }

    /// The step's `dt` is the fixed clock's timestep, not rapier's 1/60 default:
    /// after 64 steps the free-fall drop matches semi-implicit Euler at
    /// `1/64 s` (≈4.98 m) and is far from the `1/60 s` value (≈5.67 m).
    #[test]
    fn a_fixed_step_uses_the_fixed_timestep() {
        let mut world = boot();

        let start = LocalTransform::new(
            float3::new(0.0, 100.0, 0.0),
            quaternion::IDENTITY,
            float3::ONE,
        );
        let ball = spawn_ball(&mut world, start);

        for _ in 0..STEPS {
            run_fixed_step(&mut world);
        }

        // Semi-implicit Euler under constant gravity g for n steps of dt:
        // drop = g * dt^2 * n * (n + 1) / 2. The expected value is derived from
        // kinematics and the fixed clock's timestep, independent of the step
        // code — so a regression to rapier's 1/60 default fails this.
        let dt = world
            .get_resource::<FixedTime>()
            .expect("install registers FixedTime")
            .timestep()
            .as_secs_f32();
        let expected_drop = 9.81 * dt * dt * (STEPS * (STEPS + 1) / 2) as f32;
        let drop = start.position.y() - transform_of(&world, ball).position.y();
        assert!(
            (drop - expected_drop).abs() < 0.2,
            "64 steps must drop ≈{expected_drop} m at the fixed timestep, got {drop} m"
        );
    }

    /// A `RigidBody` whose handle no longer resolves is skipped with a warning,
    /// never a panic, and its transform stays untouched.
    #[test]
    fn a_dangling_rigid_body_handle_is_skipped() {
        let mut world = boot();

        let start = LocalTransform::new(
            float3::new(0.0, 5.0, 0.0),
            quaternion::IDENTITY,
            float3::ONE,
        );
        let entity = world
            .spawn((
                start,
                RigidBody {
                    handle: RigidBodyHandle::invalid(),
                },
            ))
            .id();

        for _ in 0..3 {
            run_fixed_step(&mut world);
        }

        assert_eq!(
            transform_of(&world, entity),
            start,
            "a dangling handle must be skipped, leaving its transform alone"
        );
    }
}
