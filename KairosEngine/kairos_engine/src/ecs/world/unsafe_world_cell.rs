use super::World;
use crate::ecs::{
    change_detection::Tick,
    component_tuple::{Query, QueryBorrow, QueryMut},
};
use core::{cell::UnsafeCell, marker::PhantomData, ptr};

/// 用于从只读引用获取可变 World 访问的原始指针包装。
///
/// 与 Bevy 一致：resource/component 可以通过 `&self` 访问，
/// 调用者负责保证别名规则不被违反。
#[derive(Copy, Clone)]
pub struct UnsafeWorldCell<'w> {
    ptr: *mut World,
    #[cfg(debug_assertions)]
    allows_mutable_access: bool,
    _marker: PhantomData<(&'w World, &'w UnsafeCell<World>)>,
}

// SAFETY: `&World` 和 `&mut World` 都是 Send
unsafe impl Send for UnsafeWorldCell<'_> {}
// SAFETY: `&World` 和 `&mut World` 都是 Sync
unsafe impl Sync for UnsafeWorldCell<'_> {}

impl<'w> From<&'w mut World> for UnsafeWorldCell<'w> {
    #[inline]
    fn from(value: &'w mut World) -> Self {
        value.as_unsafe_world_cell()
    }
}

impl<'w> From<&'w World> for UnsafeWorldCell<'w> {
    #[inline]
    fn from(value: &'w World) -> Self {
        value.as_unsafe_world_cell_readonly()
    }
}

impl<'w> UnsafeWorldCell<'w> {
    /// 创建只读访问的 UnsafeWorldCell。
    #[inline]
    pub(crate) fn new_readonly(world: &'w World) -> Self {
        Self {
            ptr: ptr::from_ref(world).cast_mut(),
            #[cfg(debug_assertions)]
            allows_mutable_access: false,
            _marker: PhantomData,
        }
    }

    /// 创建完全读写访问的 UnsafeWorldCell。
    #[inline]
    pub(crate) fn new_mutable(world: &'w mut World) -> Self {
        Self {
            ptr: ptr::from_mut(world),
            #[cfg(debug_assertions)]
            allows_mutable_access: true,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub(crate) fn assert_allows_mutable_access(self) {
        #[cfg(debug_assertions)]
        debug_assert!(
            self.allows_mutable_access,
            "mutating world data via a read-only UnsafeWorldCell is forbidden"
        );
    }

    /// 获取 `&'w mut World`。
    ///
    /// # Safety
    ///
    /// - `self` 必须通过 `World::as_unsafe_world_cell`（而非 `as_unsafe_world_cell_readonly`）创建。
    /// - 返回的 `&mut World` 必须唯一：不能与任何其他 World 借用同时存在。
    #[inline]
    pub unsafe fn world_mut(self) -> &'w mut World {
        self.assert_allows_mutable_access();
        unsafe { &mut *self.ptr }
    }

    /// 获取 `&'w World`（只读访问）。
    ///
    /// # Safety
    ///
    /// - 必须有权对整个 World 进行不可变访问。
    /// - 不能存在活跃的独占借用。
    #[inline]
    pub unsafe fn world(self) -> &'w World {
        unsafe { self.unsafe_world() }
    }

    /// 获取 `&'w World`，仅用于访问元数据。
    ///
    /// # Safety
    ///
    /// - 返回的引用仅用于访问元数据。
    #[inline]
    pub unsafe fn world_metadata(self) -> &'w World {
        unsafe { self.unsafe_world() }
    }

    /// `unsafe_world` 是私有的内部方法，返回 `&World` 即使存在可变借用。
    #[inline]
    unsafe fn unsafe_world(self) -> &'w World {
        unsafe { &*self.ptr }
    }

    /// 返回当前变更检测 tick。
    #[inline]
    pub fn change_tick(self) -> Tick {
        unsafe { self.world_metadata() }.change_tick()
    }

    /// 返回上次调用 `clear_trackers()` 时的 tick。
    #[inline]
    pub fn last_change_tick(self) -> Tick {
        unsafe { self.world_metadata() }.last_change_tick()
    }

    /// 原子递增变更检测 tick 并返回递增前的值。
    ///
    /// 这是 safe 的，因为 `AtomicU32::fetch_add` 只需要共享引用。
    #[inline]
    pub fn increment_change_tick(self) -> Tick {
        let prev = unsafe { &self.world_metadata().change_tick }
            .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        Tick(prev)
    }

    /// 执行只读查询。
    ///
    /// # Safety
    ///
    /// 调用者必须确保没有其他 `query_mut` 或 `&mut World` 引用同时存在。
    #[inline]
    pub unsafe fn query<Q: Query>(self) -> QueryBorrow<'w, Q> {
        QueryBorrow::new(unsafe { self.unsafe_world() })
    }

    /// 执行可变查询。
    ///
    /// # Safety
    ///
    /// 调用者必须确保没有其他 `query`/`query_mut` 或 `&mut World` 引用同时存在。
    #[inline]
    pub unsafe fn query_mut<Q: Query>(self) -> QueryMut<'w, Q> {
        self.assert_allows_mutable_access();
        QueryMut::new(unsafe { &mut *self.ptr })
    }
}

#[cfg(test)]
mod tests {
    use super::World;
    use super::UnsafeWorldCell;
    use crate::ecs::{component::Component, entity::Entity};

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

        let cell = world.as_unsafe_world_cell_readonly();
        assert_eq!(cell.change_tick(), world.change_tick());
    }

    /// UnsafeWorldCell 可以正确读取 World.last_change_tick
    #[test]
    fn read_last_change_tick() {
        let mut world = World::new();
        let initial = world.change_tick();

        world.clear_trackers();
        let cell = world.as_unsafe_world_cell_readonly();
        assert_eq!(cell.last_change_tick(), world.last_change_tick());
        assert_eq!(cell.last_change_tick(), initial);
    }

    /// UnsafeWorldCell 可以原子递增 change_tick
    #[test]
    fn increment_change_tick_atomic() {
        let world = World::new();
        let initial = world.change_tick();

        // 需要 &mut World 创建 mutable UnsafeWorldCell
        let mut world_mut = World::new();
        let cell = world_mut.as_unsafe_world_cell();
        let old = cell.increment_change_tick();

        assert_eq!(old, initial);
        assert_eq!(cell.change_tick(), initial + 1u32);
        assert_eq!(world_mut.change_tick(), initial + 1u32);
    }

    /// 多次递增 change_tick
    #[test]
    fn increment_change_tick_multiple() {
        let mut world = World::new();
        let initial = world.change_tick();

        let cell = world.as_unsafe_world_cell();

        let t1 = cell.increment_change_tick();
        assert_eq!(t1, initial);
        assert_eq!(cell.change_tick(), initial + 1u32);

        let t2 = cell.increment_change_tick();
        assert_eq!(t2, initial + 1u32);
        assert_eq!(cell.change_tick(), initial + 2u32);

        let t3 = cell.increment_change_tick();
        assert_eq!(t3, initial + 2u32);
        assert_eq!(cell.change_tick(), initial + 3u32);
    }

    /// UnsafeWorldCell 可以执行只读查询
    #[test]
    fn read_only_query() {
        let mut world = World::new();
        let entity = world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));
        world.flush();

        let cell = world.as_unsafe_world_cell_readonly();
        let mut query = unsafe { cell.query::<(Entity, &Transform)>() };
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

    /// UnsafeWorldCell 可以执行可变查询
    #[test]
    fn mutable_query() {
        let mut world = World::new();
        world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));
        world.flush();

        let cell = world.as_unsafe_world_cell();
        let query = unsafe { cell.query_mut::<(Entity, &mut Transform)>() };
        for (_e, mut t) in query {
            t.x = 10.0;
            t.y = 20.0;
        }

        let mut q = world.query::<(Entity, &Transform)>();
        for (_e, t) in q.iter() {
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

    /// 可以通过 From<&mut World> 创建 UnsafeWorldCell
    #[test]
    fn from_mut_world() {
        let mut world = World::new();
        let cell = UnsafeWorldCell::from(&mut world);
        let tick = cell.change_tick();
        assert_eq!(tick, world.change_tick());
    }

    /// 可以通过 From<&World> 创建 UnsafeWorldCell
    #[test]
    fn from_shared_world() {
        let world = World::new();
        let cell = UnsafeWorldCell::from(&world);
        let tick = cell.change_tick();
        assert_eq!(tick, world.change_tick());
    }

    /// 多个只读查询可以共存
    #[test]
    fn multiple_read_only_queries() {
        let mut world = World::new();
        world.spawn((Transform { x: 1.0, y: 2.0, z: 3.0 },));
        world.spawn((Velocity { x: 4.0, y: 5.0, z: 6.0 },));
        world.flush();

        let cell = world.as_unsafe_world_cell_readonly();

        let mut q1 = unsafe { cell.query::<&Transform>() };
        let mut q2 = unsafe { cell.query::<&Velocity>() };

        assert_eq!(q1.iter().count(), 1);
        assert_eq!(q2.iter().count(), 1);
    }
}
