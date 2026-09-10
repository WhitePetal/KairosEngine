//! The physics subsystem: a [`PhysicsEngine`] World resource plus the
//! `RigidBody` / `Collider` components that point into it.
//!
//! [`install`] is the single bootstrap entry point (bevy-plugin style): it
//! inserts the resource, registers [`physics_step_system`] into `FixedUpdate`,
//! and attaches the despawn-cleanup `on_discard` hooks to both components, so
//! every [`World`] that went through
//! [`Engine::new`](crate::kairos_editor::Engine::new) is guaranteed to carry
//! physics, to advance it once per fixed step, and to release a discarded
//! entity's rapier objects. The rapier types themselves never leave this module
//! — engine code talks to the resource through its eager `insert_*` constructors,
//! and entities carry nothing but private handles.

use rapier3d::{
    dynamics::{
        CCDSolver, CoefficientCombineRule, ImpulseJointSet, IntegrationParameters, IslandManager,
        MultibodyJointSet, RigidBodyBuilder, RigidBodyHandle, RigidBodySet,
    },
    geometry::{ColliderBuilder, ColliderHandle, ColliderSet, DefaultBroadPhase, NarrowPhase},
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
    lifecycle::HookContext,
    resource::Resource,
    schedule::Schedules,
    system::{Query, Res, ResMut},
    world::{DeferredWorld, World},
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

    /// Removes the rigid body behind `handle`, returning whether it existed.
    ///
    /// `remove_attached_colliders` is `false`: a body never owns the colliders
    /// that hang off it — each [`Collider`] component owns its own rapier object,
    /// so attached colliders are detached (`set_parent(None)`) and left for the
    /// collider hook to delete. That keeps the two cleanup paths independent and
    /// order-insensitive. Module-private: rapier types never leave `physics`.
    fn remove_rigid_body(&mut self, handle: RigidBodyHandle) -> bool {
        self.rigid_body_set
            .remove(
                handle,
                &mut self.island_manager,
                &mut self.collider_set,
                &mut self.impulse_joint_set,
                &mut self.multibody_joint_set,
                false,
            )
            .is_some()
    }

    /// Removes the collider behind `handle`, returning whether it existed.
    ///
    /// `wake_up` is `false`: tearing down an object never needs to wake an island.
    /// Removing a collider detaches it from its parent body first, so this is
    /// equally correct before or after the body hook has run. Module-private:
    /// rapier types never leave `physics`.
    fn remove_collider(&mut self, handle: ColliderHandle) -> bool {
        self.collider_set
            .remove(handle, &mut self.island_manager, &mut self.rigid_body_set, false)
            .is_some()
    }
}

/// [`ComponentHook`](kairos_ecs::lifecycle::ComponentHook) that releases the
/// rapier body owned by a discarded [`RigidBody`].
///
/// Runs on every removal path — despawn, `remove`, and replace — so a discarded
/// credential never leaks its rapier object. The copyable handle is pulled out
/// first to end the shared borrow of the world, then the resource is borrowed
/// mutably. A handle that no longer resolves is a real bug under
/// `remove_attached_colliders = false`, so it is reported with a `warn!` rather
/// than a panic.
fn on_discard_rigid_body(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    let Some(handle) = world.entity(entity).get::<RigidBody>().map(|body| body.handle) else {
        return;
    };
    let mut physics = world.resource_mut::<PhysicsEngine>();
    if !physics.remove_rigid_body(handle) {
        log::warn!("dangling rigid body handle on discard: {handle:?}");
    }
}

/// [`ComponentHook`](kairos_ecs::lifecycle::ComponentHook) that releases the
/// rapier collider owned by a discarded [`Collider`].
///
/// Independent of, and order-insensitive with, [`on_discard_rigid_body`]: each
/// hook deletes exactly the object its own component holds. A handle that no
/// longer resolves is reported with a `warn!`, never a panic.
fn on_discard_collider(mut world: DeferredWorld, HookContext { entity, .. }: HookContext) {
    let Some(handle) = world.entity(entity).get::<Collider>().map(|collider| collider.handle)
    else {
        return;
    };
    let mut physics = world.resource_mut::<PhysicsEngine>();
    if !physics.remove_collider(handle) {
        log::warn!("dangling collider handle on discard: {handle:?}");
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

/// Installs the physics resource, its single fixed-step system, and the
/// despawn-cleanup hooks onto `world`.
///
/// Registered in `Engine::new` right after
/// [`schedule::install`](crate::kairos_editor::schedule::install), so physics is
/// present for the whole lifetime of an `Engine`. This is the one place physics
/// is wired into the schedule: `KairosGame` never registers a physics system.
///
/// The same call also registers an `on_discard` hook on both physics components,
/// so every path that drops a `RigidBody`/`Collider` — despawn, `remove`, or
/// replace — releases the rapier object it owned. The hook and the resource are
/// installed together, so "there is a hook" and "there is a `PhysicsEngine`"
/// always coincide.
///
/// # Panics
///
/// If the schedule rails are not installed yet: the [`FixedUpdate`] stage would
/// not exist, and registering into a stage that does not exist is a bootstrap
/// order bug.
///
/// If the physics components are already in use — for example, installing
/// physics twice — hook registration fails: a `ComponentHooks` can only be
/// edited before the component exists in an archetype, and a `RigidBody` or
/// `Collider` may only carry one `on_discard` hook.
pub(crate) fn install(world: &mut World) {
    log::debug!("installing the physics resource and fixed-step system");
    world.insert_resource(PhysicsEngine::new());

    world
        .register_component_hooks::<RigidBody>()
        .on_discard(on_discard_rigid_body);
    world
        .register_component_hooks::<Collider>()
        .on_discard(on_discard_collider);

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
mod tests;
