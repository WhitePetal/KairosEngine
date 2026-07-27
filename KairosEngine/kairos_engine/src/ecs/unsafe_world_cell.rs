use std::marker::PhantomData;

use crate::ecs::{
    change_detection::Tick,
    component_tuple::{Query, QueryBorrow, QueryMut},
    world::World,
};

/// 用于从只读引用获取可变 World 访问的原始指针包装。
///
/// `UnsafeWorldCell` 提供了原子 `change_tick` 的读取路径，是 System 调度器
/// 让多个 System 并行读取 World 的基础。
///
/// # Safety
///
/// - `UnsafeWorldCell` 不提供任何内在的别名保护。调用者（调度器）必须保证
///   **同时只有一个 Writer**，可以有多个 Reader（遵循 Rust 的别名规则）。
/// - `query_mut::<Q>()` 会创建临时 `&mut World` 引用，调用者必须确保没有
///   其他 `&World` 或 `&mut World` 引用同时存在。
/// - `increment_change_tick()` 通过原子操作安全递增，但仍需调用者确保
///   语义上的互斥（如多线程并发递增可能违反期望的 tick 顺序）。
pub struct UnsafeWorldCell<'w> {
    world: *const World,
    _marker: PhantomData<&'w ()>,
}

// SAFETY: UnsafeWorldCell 通过 raw pointer 访问 World。Send/Sync 的正确性
// 由调用者（调度器）保证：只要 World 的所有访问遵循别名规则（唯一写/共享读），
// 跨线程传递 UnsafeWorldCell 就是安全的。
unsafe impl Send for UnsafeWorldCell<'_> {}
unsafe impl Sync for UnsafeWorldCell<'_> {}

impl<'w> UnsafeWorldCell<'w> {
    /// 从 `&World` 创建一个新的 `UnsafeWorldCell`。
    ///
    /// # Safety
    ///
    /// 调用者必须确保通过此 `UnsafeWorldCell` 的所有访问遵循 Rust 的别名规则。
    #[inline]
    pub unsafe fn new(world: &'w World) -> Self {
        Self {
            world: world as *const World,
            _marker: PhantomData,
        }
    }

    /// 返回 `&World` 引用（安全读访问）。
    #[inline]
    fn world(&self) -> &World {
        unsafe { &*self.world }
    }

    /// 返回 `&mut World` 引用（可变访问）。
    ///
    /// # Safety
    ///
    /// 调用者必须确保没有其他 `&World` 或 `&mut World` 引用同时存在。
    #[inline]
    unsafe fn world_mut(&self) -> &mut World {
        unsafe { &mut *(self.world as *mut World) }
    }

    /// 返回当前变更检测 tick。
    #[inline]
    pub fn change_tick(&self) -> Tick {
        self.world().change_tick()
    }

    /// 返回上次调用 `clear_trackers()` 时的 tick。
    #[inline]
    pub fn last_change_tick(&self) -> Tick {
        self.world().last_change_tick()
    }

    /// 原子递增变更检测 tick 并返回 **递增前的值**（Bevy 语义一致）。
    ///
    /// 返回的是递增前的旧值，与 `World::increment_change_tick()` 语义一致。
    ///
    /// # Safety
    ///
    /// 调用者必须确保没有其他 `&mut World` 引用同时存在（即语义上的互斥）。
    #[inline]
    pub unsafe fn increment_change_tick(&self) -> Tick {
        // SAFETY: 通过 world_mut 获取 &mut World 引用，调用其公共方法。
        // 调用者（调度器）必须保证没有其他并发访问。
        unsafe { self.world_mut().increment_change_tick() }
    }

    /// 执行只读查询（同 `World::query()`）。
    #[inline]
    pub fn query<Q: Query>(&self) -> QueryBorrow<'_, Q> {
        QueryBorrow::new(self.world())
    }

    /// 执行可变查询（同 `World::query_mut()`）。
    ///
    /// # Safety
    ///
    /// 调用者必须确保没有其他 `query_mut` 或 `&mut World` 引用同时存在，
    /// 且当前没有任何进行中的只读查询会与此次可变查询冲突。
    #[inline]
    pub unsafe fn query_mut<Q: Query>(&self) -> QueryMut<'_, Q> {
        let world = unsafe { self.world_mut() };
        QueryMut::new(world)
    }
}

#[cfg(test)]
mod tests {
    use crate::ecs::{
        component::Component,
        entity::Entity,
        unsafe_world_cell::UnsafeWorldCell,
        world::World,
    };

    struct Transform {
        x: f32,
        y: f32,
        z: f32,
    }
    impl Component for Transform {}

    #[allow(dead_code)]
    struct Velocity {
        x: f32,
        y: f32,
        z: f32,
    }
    impl Component for Velocity {}

    /// UnsafeWorldCell 可以正确读取 World.change_tick
    #[test]
    fn read_change_tick() {
        let mut world = World::new();
        world.increment_tick();
        world.increment_tick();

        let cell = unsafe { UnsafeWorldCell::new(&world) };
        assert_eq!(cell.change_tick(), world.change_tick());
    }

    /// UnsafeWorldCell 可以正确读取 World.last_change_tick
    #[test]
    fn read_last_change_tick() {
        let mut world = World::new();
        let initial = world.change_tick();

        // clear_trackers advances last_change_tick
        world.clear_trackers();
        let cell = unsafe { UnsafeWorldCell::new(&world) };
        assert_eq!(cell.last_change_tick(), world.last_change_tick());
        // last_change_tick should be the old change_tick value
        // (increment_change_tick returns old, clear_trackers stores it)
        assert_eq!(cell.last_change_tick(), initial);
    }

    /// UnsafeWorldCell 可以原子递增 change_tick
    #[test]
    fn increment_change_tick_atomic() {
        let world = World::new();
        let initial = world.change_tick();

        let cell = unsafe { UnsafeWorldCell::new(&world) };
        let old = unsafe { cell.increment_change_tick() };

        // 返回旧值
        assert_eq!(old, initial);
        // change_tick 已递增
        assert_eq!(cell.change_tick(), initial + 1u32);
        assert_eq!(world.change_tick(), initial + 1u32);
    }

    /// 多次递增 change_tick
    #[test]
    fn increment_change_tick_multiple() {
        let world = World::new();
        let initial = world.change_tick();

        let cell = unsafe { UnsafeWorldCell::new(&world) };

        let t1 = unsafe { cell.increment_change_tick() };
        assert_eq!(t1, initial);
        assert_eq!(cell.change_tick(), initial + 1u32);

        let t2 = unsafe { cell.increment_change_tick() };
        assert_eq!(t2, initial + 1u32);
        assert_eq!(cell.change_tick(), initial + 2u32);

        let t3 = unsafe { cell.increment_change_tick() };
        assert_eq!(t3, initial + 2u32);
        assert_eq!(cell.change_tick(), initial + 3u32);
    }

    /// UnsafeWorldCell 可以执行只读查询
    #[test]
    fn read_only_query() {
        let mut world = World::new();
        let entity = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));

        let cell = unsafe { UnsafeWorldCell::new(&world) };
        let mut query = cell.query::<(Entity, &Transform)>();
        let results: Vec<(Entity, f32, f32, f32)> = query
            .iter()
            .map(|(e, t)| (e, t.x, t.y, t.z))
            .collect();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, entity);
        assert_eq!(results[0].1, 1.0);
        assert_eq!(results[0].2, 2.0);
        assert_eq!(results[0].3, 3.0);
    }

    /// UnsafeWorldCell 可以执行可变查询（通过 unsafe）
    #[test]
    fn mutable_query() {
        let mut world = World::new();
        world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));

        let cell = unsafe { UnsafeWorldCell::new(&world) };
        let query = unsafe { cell.query_mut::<(Entity, &mut Transform)>() };
        for (_e, mut t) in query {
            t.x = 10.0;
            t.y = 20.0;
        }
        drop(cell);

        // Verify the change took effect
        let mut query = world.query::<(Entity, &Transform)>();
        for (_e, t) in query.iter() {
            assert_eq!(t.x, 10.0);
            assert_eq!(t.y, 20.0);
            assert_eq!(t.z, 3.0);
        }
    }

    /// UnsafeWorldCell 是 Send + Sync（编译期验证）
    #[test]
    fn is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<UnsafeWorldCell<'_>>();
        assert_sync::<UnsafeWorldCell<'_>>();
    }

    /// 多个只读查询可以共存
    #[test]
    fn multiple_read_only_queries() {
        let mut world = World::new();
        world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));
        world.spawn((Velocity { x: 4.0, y: 5.0, z: 6.0 },));

        let cell = unsafe { UnsafeWorldCell::new(&world) };

        let mut q1 = cell.query::<&Transform>();
        let mut q2 = cell.query::<&Velocity>();

        let transform_count = q1.iter().count();
        let velocity_count = q2.iter().count();

        assert_eq!(transform_count, 1);
        assert_eq!(velocity_count, 1);
    }
}
