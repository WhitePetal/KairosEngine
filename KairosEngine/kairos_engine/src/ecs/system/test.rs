use super::*;
use crate::ecs::component_tuple::Changed;
use crate::ecs::world::World;
use crate::math::float3;
use crate::math::quaternion;
use crate::spatial::Transform;

fn make_transform() -> Transform {
    Transform::new(float3::ZERO, quaternion::IDENTITY, float3::ONE)
}

/// System 首次运行时检测所有现有组件。
#[test]
fn first_run_detects_all_components() {
    let mut world = World::new();
    world.spawn((make_transform(),));
    world.spawn((make_transform(),));

    let count = std::cell::Cell::new(0u32);
    let count_ref = &count;
    let mut system = FunctionSystem::new(move |w: &mut World| {
        count_ref.set(w.query_mut::<Changed<Transform>>().into_iter().count() as u32);
    });

    system.initialize(&mut world);
    system.run(&mut world);

    assert_eq!(count.get(), 2, "首次运行应检测到所有现有组件");
}

/// System A 在同一帧修改组件后，System B 能通过 Changed 检测到。
#[test]
fn system_a_writes_system_b_detects_change() {
    let mut world = World::new();
    let _e = world.spawn((make_transform(),));

    let mut system_a = FunctionSystem::new(|w: &mut World| {
        let mut view = w.query_mut::<&mut Transform>().into_iter();
        if let Some(mut t) = view.next() {
            t.position = float3::new(1.0, 0.0, 0.0);
        }
    });

    let changed_count = std::cell::Cell::new(0u32);
    let changed_ref = &changed_count;
    let mut system_b = FunctionSystem::new(move |w: &mut World| {
        changed_ref.set(w.query_mut::<Changed<Transform>>().into_iter().count() as u32);
    });

    system_a.initialize(&mut world);
    system_b.initialize(&mut world);

    // 首次运行：两个 system 都能看到所有组件
    system_a.run(&mut world);
    system_b.run(&mut world);

    // 第二帧：system_a 修改组件，system_b 应检测到变更
    system_a.run(&mut world);
    system_b.run(&mut world);

    assert!(
        changed_count.get() >= 1,
        "System B 应检测到 System A 的修改（got {}）",
        changed_count.get()
    );
}

/// 多个 System 各自有独立的 last_run。
#[test]
fn systems_have_independent_last_run() {
    let mut world = World::new();
    world.spawn((make_transform(),));

    let a_count = std::cell::Cell::new(0u32);
    let a_ref = &a_count;
    let mut system_a = FunctionSystem::new(move |w: &mut World| {
        a_ref.set(w.query_mut::<Changed<Transform>>().into_iter().count() as u32);
    });

    let b_count = std::cell::Cell::new(0u32);
    let b_ref = &b_count;
    let mut system_b = FunctionSystem::new(move |w: &mut World| {
        b_ref.set(w.query_mut::<Changed<Transform>>().into_iter().count() as u32);
    });

    system_a.initialize(&mut world);
    system_b.initialize(&mut world);

    // 第一帧：两个 system 都首次运行，应看到 1 个实体
    system_a.run(&mut world);
    system_b.run(&mut world);
    assert_eq!(b_count.get(), 1, "B 首次运行应看到 1 个实体");

    // 第二帧：只跑 system_a，再跑 system_b
    // A 不修改组件，B 应看不到变更
    system_a.run(&mut world);
    system_b.run(&mut world);
    assert_eq!(b_count.get(), 0, "B 第二帧不应检测到变更（无修改）");
}

/// System 在不修改组件时，下次 run 不触发 Changed。
#[test]
fn unchanged_system_does_not_trigger_changed() {
    let mut world = World::new();
    world.spawn((make_transform(),));

    let mut system = FunctionSystem::new(|_w: &mut World| {
        // 不修改任何组件
    });

    let changed_count = std::cell::Cell::new(0u32);
    let changed_ref = &changed_count;
    let mut query_system = FunctionSystem::new(move |w: &mut World| {
        changed_ref.set(w.query_mut::<Changed<Transform>>().into_iter().count() as u32);
    });

    system.initialize(&mut world);
    query_system.initialize(&mut world);

    // 首次 run：所有组件都是新的
    system.run(&mut world);
    query_system.run(&mut world);
    assert_eq!(changed_count.get(), 1, "首次 run 应检测到实体");

    // 第二帧：system 什么都没做，查询不应检测到变更
    system.run(&mut world);
    query_system.run(&mut world);
    assert_eq!(
        changed_count.get(),
        0,
        "未修改时不应检测到 Changed"
    );
}

/// 初始化后首次 run，Changed<T> 检测到所有组件。
#[test]
fn initialize_sets_last_run_to_negative_infinity() {
    let mut world = World::new();
    world.spawn((make_transform(),));

    let mut system = FunctionSystem::new(|w: &mut World| {
        let count = w.query_mut::<Changed<Transform>>().into_iter().count();
        assert_eq!(count, 1, "首次 run 应检测到 1 个实体");
    });

    system.initialize(&mut world);
    system.run(&mut world);
}

/// System 多次 initialize 是幂等的。
#[test]
fn initialize_is_idempotent() {
    let mut world = World::new();
    world.spawn((make_transform(),));

    let mut system = FunctionSystem::new(|w: &mut World| {
        let _ = w.query_mut::<&Transform>();
    });

    system.initialize(&mut world);
    let last_run = system.meta().last_run;

    system.initialize(&mut world);
    assert_eq!(
        system.meta().last_run,
        last_run,
        "再次 initialize 不应改变 last_run"
    );
}
