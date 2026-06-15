use std::{any::TypeId, ptr::NonNull};

use crate::ecs::{component::Component, entity::Entity, table::Table};

/// [`Query`] 对 [`Table`] 的访问类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Access {
    /// 只读取实体Id，不读取组件
    Iterate,
    /// 读取组件
    Read,
    /// 读写组件
    Write,
}

/// 组件列上的流式迭代器
#[allow(clippy::missing_safety_doc)]
pub unsafe trait Fetch: Clone + Sized + 'static {
    /// Table 中 Fetch 需要访问的特定列的运行时关联状态
    type State: Copy + Send + Sync;

    /// 对于不需要 `get` 的 query，返回的占位值
    fn dangling() -> Self;

    /// 定义Query如何访问Table
    fn access(table: &Table) -> Option<Access>;

    /// 从`table`获取动态借用
    fn borrow(table: &Table, state: Self::State);
    /// 遍历`table`前，先查询它的状态
    fn prepare(table: &Table) -> Option<Self::State>;
    /// 基于关联状态给`table`构造`Fetch`
    fn execute(table: &Table, state: Self::State) -> Self;
    /// 释放从`borrow`获取的动态借用
    fn release(table: &Table, state: Self::State);

    /// 用于编译期检查，检查Fetch中的所有借用关系
    fn for_each_borrow<F: FnMut(TypeId, bool)>(f: F);
}

/// 从[`World`](crate::ecs::world::World)获取的组件类型集合
pub trait Query {
    /// 查询的结果类型
    type Item<'a>;

    type Fetch: Fetch;

    /// 获取Table中第n行的数据
    unsafe fn get<'a>(fetch: &Self::Fetch, n: usize) -> Self::Item<'a>;
}

/// 用于标记 Item 不会产出 &mut T 的 Query
#[allow(clippy::missing_safety_doc)]
pub unsafe trait QueryShared {}

unsafe impl QueryShared for Entity {}

#[derive(Clone)]
pub struct FetchEntity(NonNull<Entity>);
unsafe impl Fetch for FetchEntity {
    type State = ();

    fn dangling() -> Self {
        Self(NonNull::dangling())
    }

    fn access(_table: &Table) -> Option<Access> {
        Some(Access::Iterate)
    }

    /// Query Entity 返回的是拷贝值(无引用)，不需要借用检查
    fn borrow(_table: &Table, _state: Self::State) {}

    fn prepare(_table: &Table) -> Option<Self::State> {
        Some(())
    }

    fn execute(table: &Table, _state: Self::State) -> Self {
        Self(table.entities())
    }

    fn release(_table: &Table, _state: Self::State) {}

    fn for_each_borrow<F: FnMut(TypeId, bool)>(_f: F) {}
}

impl Query for Entity {
    type Item<'a> = Entity;
    type Fetch = FetchEntity;

    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn get<'a>(fetch: &Self::Fetch, n: usize) -> Self::Item<'a> {
        let entity = fetch.0.as_ptr().add(n).read();
        entity
    }
}

unsafe impl<T> QueryShared for &'_ T {}

pub struct FetchRead<T>(NonNull<T>);

impl<T> Clone for FetchRead<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

unsafe impl<T: Component> Fetch for FetchRead<T> {
    /// table colum index
    type State = usize;

    fn dangling() -> Self {
        Self(NonNull::dangling())
    }

    fn access(table: &Table) -> Option<Access> {
        if table.has_component_type::<T>() {
            Some(Access::Read)
        } else {
            None
        }
    }

    fn borrow(table: &Table, state: Self::State) {
        table.borrow::<T>(state);
    }

    fn prepare(table: &Table) -> Option<Self::State> {
        table.get_state::<T>()
    }

    fn execute(table: &Table, state: Self::State) -> Self {
        unsafe { Self(table.get_base(state)) }
    }

    fn release(table: &Table, state: Self::State) {
        table.release::<T>(state);
    }

    fn for_each_borrow<F: FnMut(TypeId, bool)>(mut f: F) {
        // 不可变引用可存在多个，因此传 false 表示非唯一(un_unique)
        f(TypeId::of::<T>(), false);
    }
}

impl<T: Component> Query for &'_ T {
    type Item<'q> = &'q T;

    type Fetch = FetchRead<T>;

    unsafe fn get<'q>(fetch: &Self::Fetch, n: usize) -> Self::Item<'q> {
        unsafe { &*fetch.0.as_ptr().add(n) }
    }
}

pub struct FetchWrite<T>(NonNull<T>);

impl<T> Clone for FetchWrite<T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

unsafe impl<T: Component> Fetch for FetchWrite<T> {
    type State = usize;

    fn dangling() -> Self {
        Self(NonNull::dangling())
    }

    fn access(table: &Table) -> Option<Access> {
        if table.has_component_type::<T>() {
            Some(Access::Write)
        } else {
            None
        }
    }

    fn borrow(table: &Table, state: Self::State) {
        table.borrow_mut::<T>(state);
    }

    fn prepare(table: &Table) -> Option<Self::State> {
        table.get_state::<T>()
    }

    fn execute(table: &Table, state: Self::State) -> Self {
        unsafe { Self(table.get_base::<T>(state)) }
    }

    fn release(table: &Table, state: Self::State) {
        table.release_mut::<T>(state);
    }

    fn for_each_borrow<F: FnMut(TypeId, bool)>(mut f: F) {
        // 可变借用是唯一的，因此传true
        f(TypeId::of::<T>(), true);
    }
}

impl<T: Component> Query for &'_ mut T {
    type Item<'q> = &'q mut T;

    type Fetch = FetchWrite<T>;

    unsafe fn get<'a>(fetch: &Self::Fetch, n: usize) -> Self::Item<'a> {
        unsafe { &mut *fetch.0.as_ptr().add(n) }
    }
}

unsafe impl<T: QueryShared> QueryShared for Option<T> {}

#[derive(Clone)]
pub struct TryFetch<T>(Option<T>);

unsafe impl<T: Fetch> Fetch for TryFetch<T> {
    type State = Option<T::State>;

    fn dangling() -> Self {
        Self(None)
    }

    fn access(table: &Table) -> Option<Access> {
        Some(T::access(table).unwrap_or(Access::Iterate))
    }

    fn borrow(table: &Table, state: Self::State) {
        if let Some(state) = state {
            T::borrow(table, state);
        }
    }

    fn prepare(table: &Table) -> Option<Self::State> {
        Some(T::prepare(table))
    }

    fn execute(table: &Table, state: Self::State) -> Self {
        Self(state.map(|state| T::execute(table, state)))
    }

    fn release(table: &Table, state: Self::State) {
        if let Some(state) = state {
            T::release(table, state);
        }
    }

    fn for_each_borrow<F: FnMut(TypeId, bool)>(f: F) {
        T::for_each_borrow(f);
    }
}

impl<T: Query> Query for Option<T> {
    type Item<'q> = Option<T::Item<'q>>;
    type Fetch = TryFetch<T::Fetch>;

    unsafe fn get<'a>(fetch: &Self::Fetch, n: usize) -> Self::Item<'a> {
        unsafe { Some(T::get(fetch.0.as_ref()?, n)) }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Or<L, R> {
    /// Just an `L`
    Left(L),
    /// Just an `R`
    Right(R),
    /// Both an `L` and an `R`
    Both(L, R),
}

impl<L, R> Or<L, R> {
    pub fn new(l: Option<L>, r: Option<R>) -> Option<Self> {
        match (l, r) {
            (None, None) => None,
            (None, Some(r)) => Some(Self::Right(r)),
            (Some(l), None) => Some(Self::Left(l)),
            (Some(l), Some(r)) => Some(Self::Both(l, r)),
        }
    }

    pub fn split(self) -> (Option<L>, Option<R>) {
        match self {
            Or::Left(l) => (Some(l), None),
            Or::Right(r) => (None, Some(r)),
            Or::Both(l, r) => (Some(l), Some(r)),
        }
    }

    pub fn left(self) -> Option<L> {
        match self {
            Or::Left(l) => Some(l),
            Or::Both(l, _) => Some(l),
            _ => None,
        }
    }

    pub fn right(self) -> Option<R> {
        match self {
            Or::Right(r) => Some(r),
            Or::Both(_, r) => Some(r),
            _ => None,
        }
    }

    pub fn map<L1, R1, ToL1, ToR1>(self, f: ToL1, g: ToR1) -> Or<L1, R1>
    where
        ToL1: FnOnce(L) -> L1,
        ToR1: FnOnce(R) -> R1,
    {
        match self {
            Or::Left(l) => Or::Left(f(l)),
            Or::Right(r) => Or::Right(g(r)),
            Or::Both(l, r) => Or::Both(f(l), g(r)),
        }
    }

    pub fn as_ref(&self) -> Or<&L, &R> {
        match self {
            Or::Left(l) => Or::Left(l),
            Or::Right(r) => Or::Right(r),
            Or::Both(l, r) => Or::Both(l, r),
        }
    }

    pub fn as_mut(&mut self) -> Or<&mut L, &mut R> {
        match self {
            Or::Left(l) => Or::Left(l),
            Or::Right(r) => Or::Right(r),
            Or::Both(l, r) => Or::Both(l, r),
        }
    }
}

impl<L, R> Or<&'_ L, &'_ R>
where
    L: Clone,
    R: Clone,
{
    pub fn cloned(self) -> Or<L, R> {
        self.map(Clone::clone, Clone::clone)
    }
}

unsafe impl<L: QueryShared, R: QueryShared> QueryShared for Or<L, R> {}

#[derive(Clone)]
pub struct FetchOr<L, R>(Or<L, R>);

unsafe impl<L: Fetch, R: Fetch> Fetch for FetchOr<L, R> {
    type State = Or<L::State, R::State>;

    fn dangling() -> Self {
        Self(Or::Left(L::dangling()))
    }

    fn access(table: &Table) -> Option<Access> {
        L::access(table).max(R::access(table))
    }

    fn borrow(table: &Table, state: Self::State) {
        state.map(|l| L::borrow(table, l), |r| R::borrow(table, r));
    }

    fn prepare(table: &Table) -> Option<Self::State> {
        Or::new(L::prepare(table), R::prepare(table))
    }

    fn execute(table: &Table, state: Self::State) -> Self {
        Self(state.map(|l| L::execute(table, l), |r| R::execute(table, r)))
    }

    fn release(table: &Table, state: Self::State) {
        state.map(|l| L::release(table, l), |r| R::release(table, r));
    }

    fn for_each_borrow<F: FnMut(TypeId, bool)>(mut f: F) {
        L::for_each_borrow(&mut f);
        R::for_each_borrow(&mut f);
    }
}

impl<L: Query, R: Query> Query for Or<L, R> {
    type Item<'q> = Or<L::Item<'q>, R::Item<'q>>;

    type Fetch = FetchOr<L::Fetch, R::Fetch>;

    unsafe fn get<'a>(fetch: &Self::Fetch, n: usize) -> Self::Item<'a> {
        unsafe { fetch.0.as_ref().map(|l| L::get(l, n), |r| R::get(r, n)) }
    }
}
