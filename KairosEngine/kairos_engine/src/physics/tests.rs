use super::*;
use kairos_ecs::{entity::Entity, world::World};
use rapier3d::{dynamics::RigidBodyHandle, geometry::ColliderHandle};

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
// Despawn cleanup: the `on_discard` hooks registered by `install`
// -----------------------------------------------------------------------

/// Despawning a movable entity releases both rapier objects it owned: the
/// body hook removes the body (detaching its child collider rather than
/// cascading) and the collider hook then removes that child collider.
#[test]
fn despawning_a_movable_entity_releases_its_body_and_child_collider() {
    let mut world = boot();

    let (body, collider) = world.resource_mut::<PhysicsEngine>().insert_movable_sphere(
        0.5,
        ColliderMaterial { restitution: 0.2 },
        initial_transform(),
    );
    let body_handle = body.handle;
    let collider_handle = collider.handle;

    let entity = world.spawn((initial_transform(), body, collider)).id();

    {
        let physics = world.get_resource::<PhysicsEngine>().unwrap();
        assert!(
            physics.rigid_body_set.get(body_handle).is_some(),
            "the body must resolve before the despawn"
        );
        assert!(
            physics.collider_set.get(collider_handle).is_some(),
            "the child collider must resolve before the despawn"
        );
    }

    world.despawn(entity);

    let physics = world.get_resource::<PhysicsEngine>().unwrap();
    assert!(
        physics.rigid_body_set.get(body_handle).is_none(),
        "the discarded RigidBody hook must remove the rapier body"
    );
    assert!(
        physics.collider_set.get(collider_handle).is_none(),
        "the discarded Collider hook must remove the child collider"
    );
}

/// Pins the body hook's `remove_attached_colliders = false`: deleting a body
/// detaches its child collider instead of cascading, so the collider is left
/// for its own hook — this is what makes the two hooks order-insensitive.
#[test]
fn removing_a_body_detaches_its_child_collider_instead_of_deleting_it() {
    let mut physics = PhysicsEngine::new();
    let (body, collider) = physics.insert_movable_sphere(
        0.5,
        ColliderMaterial { restitution: 0.2 },
        initial_transform(),
    );

    assert!(
        physics.remove_rigid_body(body.handle),
        "the body must have existed"
    );

    assert!(
        physics.rigid_body_set.get(body.handle).is_none(),
        "the body must be gone"
    );
    let rapier_collider = physics
        .collider_set
        .get(collider.handle)
        .expect("a body must not delete its child collider — that is the Collider hook's job");
    assert_eq!(
        rapier_collider.parent(),
        None,
        "the surviving child collider must be detached from the removed body"
    );
}

/// Despawning an immovable entity releases its standalone collider: no body
/// is involved, so the collider hook alone owns the cleanup.
#[test]
fn despawning_an_immovable_entity_releases_its_standalone_collider() {
    let mut world = boot();

    let collider = world
        .resource_mut::<PhysicsEngine>()
        .insert_immovable_box(float3::new(100.0, 1.0, 100.0), initial_transform());
    let collider_handle = collider.handle;

    let entity = world.spawn((initial_transform(), collider)).id();
    world.despawn(entity);

    let physics = world.get_resource::<PhysicsEngine>().unwrap();
    assert!(
        physics.collider_set.get(collider_handle).is_none(),
        "the discarded Collider hook must remove a standalone collider"
    );
}

/// A discarded handle that no longer resolves is reported with a `warn!`,
/// never a panic — and reaching the later cleanup proves the hook did not
/// abort the despawn.
#[test]
fn discarding_a_dangling_handle_warns_instead_of_panicking() {
    let mut world = boot();

    let (body, collider) = world.resource_mut::<PhysicsEngine>().insert_movable_sphere(
        0.5,
        ColliderMaterial { restitution: 0.2 },
        initial_transform(),
    );
    let survivor_body = body.handle;
    let survivor_collider = collider.handle;
    let survivor = world.spawn((initial_transform(), body, collider)).id();

    let dangling = world
        .spawn((
            initial_transform(),
            RigidBody {
                handle: RigidBodyHandle::invalid(),
            },
            Collider {
                handle: ColliderHandle::invalid(),
            },
        ))
        .id();

    world.despawn(dangling);

    {
        let physics = world.get_resource::<PhysicsEngine>().unwrap();
        assert!(
            physics.rigid_body_set.get(survivor_body).is_some()
                && physics.collider_set.get(survivor_collider).is_some(),
            "a dangling handle must not disturb the live rapier objects"
        );
    }

    world.despawn(survivor);
    let physics = world.get_resource::<PhysicsEngine>().unwrap();
    assert!(
        physics.rigid_body_set.get(survivor_body).is_none()
            && physics.collider_set.get(survivor_collider).is_none(),
        "cleanup after a dangling handle must still release live objects"
    );
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
