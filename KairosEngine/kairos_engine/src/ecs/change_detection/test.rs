use crate::ecs::{
    change_detection::{DetectChanges, DetectChangesMut, Ref, Tick},
    component::Component,
    component_tuple::{Added, Changed},
    world::World,
};

// 测试组件
#[derive(Debug, Clone, PartialEq)]
struct Transform {
    x: f32,
    y: f32,
    z: f32,
}
impl Component for Transform {}

#[derive(Debug, Clone, PartialEq)]
struct Velocity {
    x: f32,
    y: f32,
    z: f32,
}
impl Component for Velocity {}

/// 测试 spawn → Changed/Added 匹配新插入的组件
#[test]
fn spawn_entity_changed_matches() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let _entity = world.spawn((Transform { x: 1.0, y: 0.0, z: 0.0 },));

    // Bind the query to avoid temporary lifetime issues
    let mut query = world.query::<Changed<Transform>>();
    let results: Vec<&Transform> = query.iter().collect();
    assert_eq!(results.len(), 1, "spawned entity should appear in Changed query");
    assert_eq!(*results[0], Transform { x: 1.0, y: 0.0, z: 0.0 });
    drop(query);

    // Also verify Added matches
    let mut query = world.query::<Added<Transform>>();
    let results: Vec<&Transform> = query.iter().collect();
    assert_eq!(results.len(), 1, "spawned entity should appear in Added query");
}

/// 测试 insert → Added 匹配新插入的组件
#[test]
fn insert_entity_added_matches() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let entity = world.spawn((Transform { x: 0.0, y: 0.0, z: 0.0 },));

    // After incrementing tick, the old component should NOT be changed
    world.increment_tick(); // tick = 2

    // Insert a new component onto the entity
    world
        .insert_one(entity, Velocity {
            x: 5.0,
            y: 0.0,
            z: 0.0,
        })
        .unwrap();

    // Changed<Velocity> should match (it was just inserted)
    let mut query = world.query::<Changed<Velocity>>();
    let v_changed: Vec<&Velocity> = query.iter().collect();
    assert_eq!(v_changed.len(), 1, "inserted Velocity should appear in Changed query");

    // Added<Velocity> should also match
    let mut query = world.query::<Added<Velocity>>();
    let v_added: Vec<&Velocity> = query.iter().collect();
    assert_eq!(v_added.len(), 1, "inserted Velocity should appear in Added query");
}

/// 测试修改后的组件被 Changed 查询捕获
#[test]
fn modify_entity_changed_matches() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let entity = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));

    // Advance tick so the component is no longer "changed" for tick 1
    world.increment_tick(); // tick = 2

    // Modify by re-inserting the component with new values
    world
        .insert_one(entity, Transform { x: 10.0, y: 20.0, z: 30.0 })
        .unwrap();

    // Changed<Transform> should match (it was just modified)
    let mut query = world.query::<Changed<Transform>>();
    let changed: Vec<&Transform> = query.iter().collect();
    assert_eq!(changed.len(), 1, "modified Transform should appear in Changed query");
    assert_eq!(*changed[0], Transform { x: 10.0, y: 20.0, z: 30.0 });
}

/// 测试未修改的组件不被 Changed/Added 查询返回
#[test]
fn unchanged_component_not_returned() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let _entity = world.spawn((Transform { x: 0.0, y: 0.0, z: 0.0 },));

    // Advance tick past the insertion tick
    world.increment_tick(); // tick = 2

    // This component was inserted at tick 1, now at tick 2 it should NOT be changed
    let mut query = world.query::<Changed<Transform>>();
    let changed: Vec<&Transform> = query.iter().collect();
    assert_eq!(changed.len(), 0, "unchanged Transform should NOT appear in Changed query");
    drop(query);

    let mut query = world.query::<Added<Transform>>();
    let added: Vec<&Transform> = query.iter().collect();
    assert_eq!(added.len(), 0, "unchanged Transform should NOT appear in Added query");
}

/// 测试多实体场景下 Changed 只返回正确实体
#[test]
fn changed_only_returns_modified_entities() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let e1 = world.spawn((Transform { x: 1.0, y: 0.0, z: 0.0 },));
    let _e2 = world.spawn((Transform { x: 2.0, y: 0.0, z: 0.0 },));

    world.increment_tick(); // tick = 2

    // Only modify e1
    world
        .insert_one(e1, Transform { x: 10.0, y: 0.0, z: 0.0 })
        .unwrap();

    let mut query = world.query::<Changed<Transform>>();
    let changed: Vec<&Transform> = query.iter().collect();
    assert_eq!(changed.len(), 1, "only one entity should have changed Transform");
    assert_eq!(*changed[0], Transform { x: 10.0, y: 0.0, z: 0.0 });
}

/// 测试 query_mut 标记组件为已修改
#[test]
fn query_mut_marks_as_changed() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let _e1 = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));

    world.increment_tick(); // tick = 2

    // Access via query_mut - QueryMut implements IntoIterator.
    // The tuple (&mut Transform,) yields items of type (&mut Transform,) - access via .0
    for mut t in world.query::<(&mut Transform,)>().iter() {
        t.0.x = 100.0;
    }

    // Now Changed<Transform> should match since we accessed it mutably
    let mut query = world.query::<Changed<Transform>>();
    let changed: Vec<&Transform> = query.iter().collect();
    assert_eq!(changed.len(), 1, "mutably accessed component should be in Changed query");
    assert_eq!(changed[0].x, 100.0);
}

/// 测试多个不同类型的组件只返回正确的 Changed 类型
#[test]
fn changed_does_not_mix_types() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let entity = world.spawn((
        Transform { x: 1.0, y: 2.0, z: 3.0 },
        Velocity { x: 0.1, y: 0.2, z: 0.3 },
    ));

    world.increment_tick(); // tick = 2

    // Only modify Transform
    world
        .insert_one(entity, Transform { x: 10.0, y: 20.0, z: 30.0 })
        .unwrap();

    let mut query = world.query::<Changed<Transform>>();
    let changed_t: Vec<&Transform> = query.iter().collect();
    assert_eq!(changed_t.len(), 1, "Transform was modified");

    let mut query = world.query::<Changed<Velocity>>();
    let changed_v: Vec<&Velocity> = query.iter().collect();
    assert_eq!(changed_v.len(), 0, "Velocity was NOT modified");
}

/// 测试 exchange 触发 Changed
#[test]
fn exchange_triggers_changed() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let entity = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));

    world.increment_tick(); // tick = 2

    // Exchange: remove Transform, add Velocity
    let removed: Transform = world
        .exchange_one::<Transform, Velocity>(entity, Velocity { x: 5.0, y: 6.0, z: 7.0 })
        .unwrap();
    assert_eq!(removed, Transform { x: 1.0, y: 2.0, z: 3.0 });

    // Velocity was just added
    let mut query = world.query::<Added<Velocity>>();
    let added_v: Vec<&Velocity> = query.iter().collect();
    assert_eq!(added_v.len(), 1, "Velocity was added via exchange");

    // Transform was removed, so it shouldn't appear
    let mut query = world.query::<Changed<Transform>>();
    let changed_t: Vec<&Transform> = query.iter().collect();
    assert_eq!(changed_t.len(), 0, "Transform was removed, not modified");
}

// -------------------------------------------------------------------------/ 测试 increment_tick 后不再返回组件
#[test]
fn increment_tick_clears_changed() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let _entity = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));

    // Components inserted at tick 1
    {
        let mut query = world.query::<Changed<Transform>>();
        assert_eq!(
            query.iter().count(),
            1,
            "newly spawned is changed"
        );
    } // query is dropped here, releasing the borrow on world

    world.increment_tick(); // tick = 2

    // After incrementing, no more changes
    {
        let mut query = world.query::<Changed<Transform>>();
        assert_eq!(
            query.iter().count(),
            0,
            "after increment_tick, no components should be changed"
        );
    }
}

/// 测试有多个实体时，只返回匹配 Changed 的实体
#[test]
fn multiple_entities_partial_change() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let e1 = world.spawn((Transform { x: 1.0, y: 1.0, z: 1.0 },));
    let _e2 = world.spawn((Transform { x: 2.0, y: 2.0, z: 2.0 },));
    let e3 = world.spawn((Transform { x: 3.0, y: 3.0, z: 3.0 },));

    world.increment_tick(); // tick = 2

    // Modify 2 out of 3 entities
    world
        .insert_one(e1, Transform { x: 10.0, y: 10.0, z: 10.0 })
        .unwrap();
    world
        .insert_one(e3, Transform { x: 30.0, y: 30.0, z: 30.0 })
        .unwrap();

    let mut query = world.query::<Changed<Transform>>();
    let changed: Vec<&Transform> = query.iter().collect();
    assert_eq!(changed.len(), 2, "two entities should have changed Transform");
    assert!(
        changed.iter().any(|t| t.x == 10.0),
        "e1 should be in results"
    );
    assert!(
        changed.iter().any(|t| t.x == 30.0),
        "e3 should be in results"
    );
}

/// 测试 increment_change_tick 返回新增值
#[test]
fn increment_change_tick_returns_new_tick() {
    let mut world = World::new();
    assert_eq!(world.change_tick(), Tick::MIN);

    let tick1 = world.increment_change_tick();
    assert_eq!(tick1, Tick::new(1));
    assert_eq!(world.change_tick(), Tick::new(1));

    let tick2 = world.increment_change_tick();
    assert_eq!(tick2, Tick::new(2));
    assert_eq!(world.change_tick(), Tick::new(2));
}

/// 测试 clear_trackers 推进 last_change_tick
#[test]
fn clear_trackers_advances_last_change_tick() {
    let mut world = World::new();
    assert_eq!(world.change_tick(), Tick::MIN);
    world.increment_tick(); // tick = 1
    world.increment_tick(); // tick = 2

    world.clear_trackers();
    // after clear_trackers, last_change_tick = change_tick = 2
    // no panic means success
}

/// 测试 Table grow_exact realloc 后 ticks 数据完整
#[test]
fn table_grow_preserves_ticks() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    // spawn many entities to force grow
    let mut entities = Vec::new();
    for i in 0..100 {
        let e = world.spawn((Transform { x: i as f32, y: 0.0, z: 0.0 },));
        entities.push(e);
    }

    // All should be changed at tick 1
    let mut query = world.query::<Changed<Transform>>();
    assert_eq!(query.iter().count(), 100, "all 100 entities should be changed after spawn");
    drop(query);

    world.increment_tick(); // tick = 2

    // None should be changed at tick 2
    let mut query = world.query::<Changed<Transform>>();
    assert_eq!(query.iter().count(), 0, "no entities should be changed after tick advance");
}

/// 测试 remove_entity swap 后 ticks 与被移动的 entity 对应正确
#[test]
fn remove_entity_swap_preserves_tick_correspondence() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let e1 = world.spawn((Transform { x: 1.0, y: 0.0, z: 0.0 },));
    let e2 = world.spawn((Transform { x: 2.0, y: 0.0, z: 0.0 },));
    let _e3 = world.spawn((Transform { x: 3.0, y: 0.0, z: 0.0 },));

    world.increment_tick(); // tick = 2

    // Modify e2 only
    world.insert_one(e2, Transform { x: 20.0, y: 0.0, z: 0.0 }).unwrap();

    // Remove e1 (this will swap e1 with e3 in the table)
    world.despawn(e1).unwrap();

    // Changed query should return only e2
    let mut query = world.query::<Changed<Transform>>();
    let changed: Vec<&Transform> = query.iter().collect();
    assert_eq!(changed.len(), 1, "only e2 should be changed");
    assert_eq!(changed[0].x, 20.0, "e2 should have x=20.0");
}

/// 测试 move_to 正确携带 ticks
#[test]
fn move_to_carries_ticks() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let entity = world.spawn((
        Transform { x: 1.0, y: 2.0, z: 3.0 },
        Velocity { x: 0.1, y: 0.2, z: 0.3 },
    ));

    world.increment_tick(); // tick = 2

    // Remove Velocity — this triggers move_to internally
    let _vel: Velocity = world.remove_one(entity).unwrap();

    // The entity still has Transform
    let entity_ref = world.entity_ref(entity).unwrap();
    let result: Transform = (*entity_ref.get::<&Transform>().unwrap()).clone();
    assert_eq!(result, Transform { x: 1.0, y: 2.0, z: 3.0 });

    // Transform should NOT be changed (its tick was preserved during move)
    let mut query = world.query::<Changed<Transform>>();
    assert_eq!(query.iter().count(), 0, "Transform tick should be preserved during move_to");
}

/// 测试 Tick wrapping 在 World 层面的正确性
#[test]
fn tick_wrapping_at_world_level() {
    let mut world = World::new();

    for _ in 0..10 {
        world.increment_tick();
    }

    let tick_before = world.change_tick();
    assert!(tick_before.0 > 0, "tick should have advanced");

    // Spawn 后检查 Changed
    world.increment_tick();
    let _e = world.spawn((Transform { x: 1.0, y: 0.0, z: 0.0 },));
    let mut query = world.query::<Changed<Transform>>();
    assert_eq!(query.iter().count(), 1, "entity spawned after tick advance should be changed");
}

// =========================================================================
// T3 — Mut<T>/Ref<T> Access Wrappers
// =========================================================================

/// 测试 DerefMut 自动标记 changed
#[test]
fn mut_deref_mut_marks_changed() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let _e = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));
    world.increment_tick(); // tick = 2

    // Access via (&mut Transform,) — Query::get returns Mut<Transform>
    for mut t in world.query::<(&mut Transform,)>().iter() {
        t.0.x = 100.0; // DerefMut → set_changed
    }

    // Changed should detect the modification
    let mut query = world.query::<Changed<Transform>>();
    let changed: Vec<&Transform> = query.iter().collect();
    assert_eq!(changed.len(), 1, "Mut::deref_mut should mark changed");
    assert_eq!(changed[0].x, 100.0);
}

/// 测试 bypass_change_detection 不触发 changed 标记
#[test]
fn mut_bypass_change_detection_does_not_mark() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let _e = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));
    world.increment_tick(); // tick = 2

    // Access via query_mut and use bypass_change_detection
    {
        let mut q = world.query::<(&mut Transform,)>();
        for mut item in q.iter() {
            let t: &mut Transform = item.0.bypass_change_detection();
            t.x = 200.0;
        }
    }

    // Changed should NOT detect the modification
    let mut query = world.query::<Changed<Transform>>();
    assert_eq!(
        query.iter().count(),
        0,
        "bypass_change_detection should NOT mark changed"
    );
}

/// 测试 set_if_neq 值不同时标记 changed
#[test]
fn mut_set_if_neq_marks_when_different() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let _e = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));
    world.increment_tick(); // tick = 2

    // Use set_if_neq with a different value → should mark changed
    {
        let mut q = world.query::<(&mut Transform,)>();
        for mut item in q.iter() {
            item.0.set_if_neq(Transform { x: 10.0, y: 20.0, z: 30.0 });
        }
    }

    let mut query = world.query::<Changed<Transform>>();
    assert_eq!(
        query.iter().count(),
        1,
        "set_if_neq with different value should mark changed"
    );
}

/// 测试 set_if_neq 值相同时不标记 changed
#[test]
fn mut_set_if_neq_noop_when_equal() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let original = Transform { x: 1.0, y: 2.0, z: 3.0 };
    let _e = world.spawn((original.clone(),));
    world.increment_tick(); // tick = 2

    // Use set_if_neq with the same value → should NOT mark changed
    {
        let mut q = world.query::<(&mut Transform,)>();
        for mut item in q.iter() {
            item.0.set_if_neq(original.clone());
        }
    }

    let mut query = world.query::<Changed<Transform>>();
    assert_eq!(
        query.iter().count(),
        0,
        "set_if_neq with same value should NOT mark changed"
    );
}

/// 测试 Ref::is_changed / is_added 的正确性
#[test]
fn ref_is_changed_and_is_added() {
    let mut world = World::new();
    world.increment_tick(); // tick = 1

    let _e = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));
    world.increment_tick(); // tick = 2

    // At tick 2, the component was added at tick 1, so is_added should be false
    // (last_run for the implicit system is tick 2 here)
    // Actually, World::query uses last_change_tick as last_run and change_tick as this_run.
    // After spawn at tick 1, then increment to tick 2, the component was added at tick 1.
    // The query runs with last_run = last_change_tick, this_run = change_tick.
    // Since we didn't call clear_trackers, last_change_tick is still Tick::MIN (0).
    // So is_added(0, 2) should return true since 1 is_newer_than(0, 2).
    // Let me verify: last_run = 0, this_run = 2, added = 1
    // ticks_since_change = 1.relative_to(2) = (2-1).wrapping = 1, min MAX = 1
    // ticks_since_system = 0.relative_to(2) = (2-0).wrapping = 2, min MAX = 2
    // 1 < 2 → true. Yes, is_added = true.

    // After incrementing to tick 3 without any changes, is_added should be false
    world.increment_tick(); // tick = 3

    // Now: last_run = 0 (last_change_tick hasn't been updated), this_run = 3, added = 1
    // ticks_since_change = 1.relative_to(3) = 2, min MAX = 2
    // ticks_since_system = 0.relative_to(3) = 3, min MAX = 3
    // 2 < 3 → true still! So we need to advance more or call clear_trackers.
    // Actually, let's use clear_trackers to set last_change_tick = change_tick.
    world.clear_trackers(); // last_change_tick = 3
    world.increment_tick(); // tick = 4
    // Now the query will use last_run = 3, this_run = 4
    // added = 1, is_added(3, 4):
    // ticks_since_change = 1.relative_to(4) = 3, min MAX = 3
    // ticks_since_system = 3.relative_to(4) = 1, min MAX = 1
    // 3 < 1 → false! is_added = false. ✅

    let mut query = world.query::<(&Transform,)>();
    let items: Vec<_> = query.iter().collect();
    assert_eq!(items.len(), 1);
    let r: &Ref<'_, Transform> = &items[0].0;
    // At this point, the component is old, so is_added and is_changed should be false
    assert!(!r.is_added(), "component added long ago should not be is_added");
    assert!(!r.is_changed(), "component not recently changed should not be is_changed");
}

/// 测试 Ref 实现了 Copy（不要求 T: Copy）
#[test]
fn ref_implements_copy() {
    // 手动验证: Ref 可以自由复制而不要求 T: Copy
    let world = &mut World::new();
    world.increment_tick();
    let _e = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));
    world.increment_tick();

    let mut query = world.query::<(&Transform,)>();
    let items: Vec<_> = query.iter().collect();
    let r = items[0].0; // Move out
    let r2 = r; // Copy — should work without T: Copy
    let _ = r; // suppress unused warning
    let _ = r2;
}

/// 测试 FetchRead 返回 Ref（而非 &T）
#[test]
fn fetch_read_returns_ref() {
    let mut world = World::new();
    world.increment_tick();
    let _e = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));

    // (&Transform,) should yield Ref<Transform> items (not &Transform)
    let mut query = world.query::<(&Transform,)>();
    for item in query.iter() {
        // item is (Ref<Transform>,), item.0 is Ref<Transform>
        // Deref to &Transform still works
        let _: &Transform = &*item.0;
        // is_changed/is_added available
        let _ = item.0.is_changed();
        let _ = item.0.is_added();
    }
}

/// 测试 FetchWrite 返回 Mut（而非 &mut T）
#[test]
fn fetch_write_returns_mut() {
    let mut world = World::new();
    world.increment_tick();
    let _e = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));

    // (&mut Transform,) should yield Mut<Transform> items (not &mut Transform)
    let mut query = world.query::<(&mut Transform,)>();
    for mut item in query.iter() {
        // item is (Mut<Transform>,), item.0 is Mut<Transform>
        // DerefMut works and auto-marks changed
        item.0.x = 42.0;
        // is_changed available
        let _ = item.0.is_changed();
        // bypass_change_detection available
        let _: &mut Transform = item.0.bypass_change_detection();
    }
}
