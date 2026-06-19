use std::{
    any::{Any, TypeId},
    collections::hash_map::Entry,
    marker::PhantomData,
    ops::Range,
    ptr::NonNull,
    sync::{Arc, RwLock},
    usize,
};

use petgraph::graph::{Node, NodeIndex};

use crate::{
    ecs::{
        component::Component,
        entity::Entity,
        sparse_set::SparseSet,
        table::Table,
        table_graph::{TableGraph, TableGraphGeneration},
        world::{EntityData, World},
    },
    types::TypeIdMap,
};

use core::slice::Iter as SliceIter;
use super::tuple_macros::{reverse_apply, smaller_tuples_too};

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

    unsafe fn get<'q>(fetch: &Self::Fetch, n: usize) -> Self::Item<'q> {
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

    unsafe fn get<'q>(fetch: &Self::Fetch, n: usize) -> Self::Item<'q> {
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

    unsafe fn get<'q>(fetch: &Self::Fetch, n: usize) -> Self::Item<'q> {
        unsafe { fetch.0.as_ref().map(|l| L::get(l, n), |r| R::get(r, n)) }
    }
}

/// 按Q进行查找，同时排出掉满足R查找条件的结果
///
// 因为只需要类型信息，因此使用 PhantomData (告诉编译器这里只需要类型信息，不存储实际数据)
pub struct Without<Q, R>(PhantomData<(Q, fn(R))>);

unsafe impl<Q: QueryShared, R> QueryShared for Without<Q, R> {}

// fn(G) 是通过函数指针类型来限制G必须是静态类型
// 并且因为这里只使用G类型本身，因此通过fn(G)，可以忽略G本身的一些类型条件
// 例如：Send、Sync、'static 等，使得 FetchWithout 的类型条件只与F绑定
pub struct FetchWithout<F, G>(F, PhantomData<fn(G)>);

impl<F: Clone, G> Clone for FetchWithout<F, G> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

unsafe impl<F: Fetch, G: Fetch> Fetch for FetchWithout<F, G> {
    type State = F::State;

    fn dangling() -> Self {
        Self(F::dangling(), PhantomData)
    }

    fn access(table: &Table) -> Option<Access> {
        if G::access(table).is_some() {
            None
        } else {
            F::access(table)
        }
    }

    fn borrow(table: &Table, state: Self::State) {
        F::borrow(table, state);
    }

    fn prepare(table: &Table) -> Option<Self::State> {
        if G::access(table).is_some() {
            return None;
        }
        F::prepare(table)
    }

    fn execute(table: &Table, state: Self::State) -> Self {
        Self(F::execute(table, state), PhantomData)
    }

    fn release(table: &Table, state: Self::State) {
        F::release(table, state);
    }

    fn for_each_borrow<FF: FnMut(TypeId, bool)>(f: FF) {
        F::for_each_borrow(f);
    }
}

impl<Q: Query, R: Query> Query for Without<Q, R> {
    type Item<'q> = Q::Item<'q>;
    type Fetch = FetchWithout<Q::Fetch, R::Fetch>;

    unsafe fn get<'q>(fetch: &Self::Fetch, n: usize) -> Self::Item<'q> {
        unsafe { Q::get(&fetch.0, n) }
    }
}

/// 按Q查找，同时还必须满足R的查找条件(但只查找Q的数据，R在查找中只作为类型条件)
pub struct With<Q, R>(PhantomData<(Q, fn(R))>);

unsafe impl<Q: QueryShared, R> QueryShared for With<Q, R> {}

pub struct FetchWith<F, G>(F, PhantomData<fn(G)>);

impl<F: Clone, G> Clone for FetchWith<F, G> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

unsafe impl<F: Fetch, G: Fetch> Fetch for FetchWith<F, G> {
    type State = F::State;

    fn dangling() -> Self {
        Self(F::dangling(), PhantomData)
    }

    fn access(table: &Table) -> Option<Access> {
        if G::access(table).is_some() {
            F::access(table)
        } else {
            None
        }
    }

    fn borrow(table: &Table, state: Self::State) {
        F::borrow(table, state);
    }

    fn prepare(table: &Table) -> Option<Self::State> {
        G::access(table)?;
        F::prepare(table)
    }

    fn execute(table: &Table, state: Self::State) -> Self {
        Self(F::execute(table, state), PhantomData)
    }

    fn release(table: &Table, state: Self::State) {
        F::release(table, state);
    }

    fn for_each_borrow<FF: FnMut(TypeId, bool)>(f: FF) {
        F::for_each_borrow(f);
    }
}

impl<Q: Query, R: Query> Query for With<Q, R> {
    type Item<'q> = Q::Item<'q>;
    type Fetch = FetchWith<Q::Fetch, R::Fetch>;

    unsafe fn get<'q>(fetch: &Self::Fetch, n: usize) -> Self::Item<'q> {
        unsafe { Q::get(&fetch.0, n) }
    }
}

/// 用于检查查询结果是否满足Q的查询条件
///
/// 如果满足则返回true，不满足则返回false
pub struct Satisfies<Q>(PhantomData<Q>);

unsafe impl<Q> QueryShared for Satisfies<Q> {}

pub struct FetchSatisfies<F>(bool, PhantomData<F>);

impl<T> Clone for FetchSatisfies<T> {
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}

unsafe impl<F: Fetch> Fetch for FetchSatisfies<F> {
    type State = bool;

    fn dangling() -> Self {
        Self(false, PhantomData)
    }

    fn access(_table: &Table) -> Option<Access> {
        Some(Access::Iterate)
    }

    fn borrow(_table: &Table, _state: Self::State) {}

    fn prepare(table: &Table) -> Option<Self::State> {
        Some(F::prepare(table).is_some())
    }

    fn execute(_table: &Table, state: Self::State) -> Self {
        Self(state, PhantomData)
    }

    fn release(_table: &Table, _state: Self::State) {}

    fn for_each_borrow<FF: FnMut(TypeId, bool)>(_f: FF) {}
}

impl<Q: Query> Query for Satisfies<Q> {
    type Item<'q> = bool;

    type Fetch = FetchSatisfies<Q::Fetch>;

    unsafe fn get<'q>(fetch: &Self::Fetch, _n: usize) -> Self::Item<'q> {
        fetch.0
    }
}

pub type QueryCache = RwLock<TypeIdMap<Arc<dyn Any + Send + Sync>>>;

struct CachedQueryInner<F: Fetch> {
    states: Box<[(usize, F::State)]>,
    // 当有新建表时，需要更新查找缓存，因为新建的表可能也满足查找条件
    table_graph_generation: TableGraphGeneration,
}

impl<F: Fetch> CachedQueryInner<F> {
    fn new(world: &World) -> Self {
        Self {
            states: world
                .table_graph_iter()
                .enumerate()
                .filter_map(|(idx, x)| F::prepare(&x.weight).map(|state| (idx, state)))
                .collect(),
            table_graph_generation: world.table_graph_generation(),
        }
    }
}

pub struct CachedQuery<F: Fetch> {
    inner: Arc<CachedQueryInner<F>>,
}

impl<F: Fetch> CachedQuery<F> {
    pub fn get(world: &World) -> Self {
        let existing_cache = world
            .query_cache()
            .read()
            .unwrap()
            .get(&TypeId::of::<F>())
            .map(|x| Arc::downcast::<CachedQueryInner<F>>(x.clone()).unwrap())
            .filter(|x| x.table_graph_generation == world.table_graph_generation());

        let inner = existing_cache.unwrap_or_else(
            // 告诉编译器这个闭包属于冷路径代码
            // 这样该闭包不会被内联入调用方，并会将其代码布局到冷路径代码块
            // 这样能够提高热路径代码指令的缓存命中率
            #[cold]
            || {
                // 前面读锁结束后，到这里，中间可能有别的线程已经写入了新的cache
                // 所以我们拿到写锁，然后重新判断一下cache是否存在
                let mut cache = world.query_cache().write().unwrap();
                let entry = cache.entry(TypeId::of::<F>());
                let cached = match entry {
                    Entry::Occupied(mut e) => {
                        let value = Arc::downcast::<CachedQueryInner<F>>(e.get().clone()).unwrap();
                        match value.table_graph_generation == world.table_graph_generation() {
                            true => value,
                            false => {
                                let fresh = Arc::new(CachedQueryInner::<F>::new(world));
                                e.insert(fresh.clone());
                                fresh
                            }
                        }
                    }
                    Entry::Vacant(e) => {
                        let fresh = Arc::new(CachedQueryInner::<F>::new(world));
                        e.insert(fresh.clone());
                        fresh
                    }
                };
                cached
            },
        );
        Self { inner }
    }

    fn table_count(&self) -> usize {
        self.inner.states.len()
    }

    unsafe fn get_state<'a>(
        &self,
        table_graph: &'a TableGraph,
        index: usize,
    ) -> Option<(&'a Table, F::State)> {
        unsafe {
            let &(table_index, state) = self.inner.states.get_unchecked(index);
            let table = &table_graph.get_table_node_unchecked(table_index).weight;
            Some((table, state))
        }
    }

    unsafe fn get_table<'a>(&self, table_graph: &'a TableGraph, index: usize) -> Option<&'a Table> {
        unsafe {
            let &(table_index, _) = self.inner.states.get_unchecked(index);
            let table = &table_graph.get_table_node_unchecked(table_index).weight;
            Some(table)
        }
    }

    fn borrow(&self, table_graph: &TableGraph) {
        for (table, state) in &self.inner.states {
            let table = unsafe { table_graph.get_table_node_unchecked(*table) };
            if table.weight.is_emptry() {
                continue;
            }
            F::borrow(&table.weight, *state);
        }
    }

    fn release_borrow(&self, table_graph: &TableGraph) {
        for (table, state) in &self.inner.states {
            let table = unsafe { table_graph.get_table_node_unchecked(*table) };
            if table.weight.is_emptry() {
                continue;
            }
            F::release(&table.weight, *state);
        }
    }

    fn fetch_all(&self, table_graph: &TableGraph) -> Box<[Option<F>]> {
        let mut fetch = (0..table_graph.node_count())
            .map(|_| None)
            .collect::<Box<[_]>>();
        for (table_index, state) in &self.inner.states {
            let table = unsafe { &table_graph.get_table_node_unchecked(*table_index).weight };
            fetch[*table_index] = Some(F::execute(table, *state))
        }
        fetch
    }
}

impl<F: Fetch> Clone for CachedQuery<F> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

pub struct View<'q, Q: Query> {
    entity_datas: &'q SparseSet<Entity, EntityData>,
    tables: &'q TableGraph,
    fetches: Box<[Option<Q::Fetch>]>,
}

unsafe impl<Q: Query> Send for View<'_, Q> where for<'a> Q::Item<'a>: Send {}
unsafe impl<Q: Query> Sync for View<'_, Q> where for<'a> Q::Item<'a>: Sync {}

pub struct ViewIter<'a, Q: Query> {
    entity_datas: &'a SparseSet<Entity, EntityData>,
    tables: SliceIter<'a, Node<Table>>,
    fetches: SliceIter<'a, Option<Q::Fetch>>,
    iter: ChunkIter<Q>,
}

impl<'a, Q: Query> Iterator for ViewIter<'a, Q> {
    type Item = Q::Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match unsafe { self.iter.next() } {
                Some(components) => {
                    return Some(components);
                }
                None => {
                    let table = self.tables.next()?;
                    let fetch = self.fetches.next()?;
                    self.iter = fetch.clone().map_or(ChunkIter::empty(), |fetch| {
                        ChunkIter::new(&table.weight, fetch)
                    });
                    continue;
                }
            }
        }
    }
}

impl<'q, Q: Query> View<'q, Q> {
    pub unsafe fn new(
        entity_datas: &'q SparseSet<Entity, EntityData>,
        tables: &'q TableGraph,
        cache: CachedQuery<Q::Fetch>,
    ) -> Self {
        Self {
            entity_datas,
            tables,
            fetches: cache.fetch_all(tables),
        }
    }

    pub fn get(&self, entity: &Entity) -> Option<Q::Item<'_>>
    where
        Q: QueryShared,
    {
        unsafe { self.get_unchecked(entity) }
    }

    pub fn get_mut(&mut self, entity: &Entity) -> Option<Q::Item<'_>> {
        unsafe { self.get_unchecked(entity) }
    }

    pub unsafe fn get_unchecked(&self, entity: &Entity) -> Option<Q::Item<'_>> {
        match self.entity_datas.get(entity) {
            Some(data) => self.fetches[data.table_index().index()]
                .as_ref()
                .map(|fetch| unsafe { Q::get(fetch, data.row_index()) }),
            None => None,
        }
    }

    pub fn contains(&self, entity: &Entity) -> bool {
        let Some(data) = self.entity_datas.get(entity) else {
            return false;
        };
        self.fetches[data.table_index().index()].is_some()
    }

    pub fn get_disjoint_mut<const N: usize>(
        &mut self,
        entities: [Entity; N],
    ) -> [Option<Q::Item<'_>>; N] {
        assert_distinct(&entities);

        let mut items = [(); N].map(|()| None);

        for (item, entity) in items.iter_mut().zip(entities) {
            unsafe {
                *item = self.get_unchecked(&entity);
            }
        }

        items
    }

    pub fn iter_mut(&mut self) -> ViewIter<'_, Q> {
        ViewIter {
            entity_datas: self.entity_datas,
            tables: self.tables.iter(),
            fetches: self.fetches.iter(),
            iter: ChunkIter::empty(),
        }
    }
}

impl<'a, Q: Query> IntoIterator for &'a mut View<'_, Q> {
    type IntoIter = ViewIter<'a, Q>;
    type Item = Q::Item<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

pub struct ViewBorrow<'w, Q: Query> {
    view: View<'w, Q>,
    cache: CachedQuery<Q::Fetch>,
}

impl<'w, Q: Query> ViewBorrow<'w, Q> {
    pub fn new(world: &'w World) -> Self {
        let cache = CachedQuery::get(world);
        cache.borrow(world.table_graph());
        let view =
            unsafe { View::<Q>::new(world.entity_datas(), world.table_graph(), cache.clone()) };

        Self { view, cache }
    }

    pub fn get(&self, entity: &Entity) -> Option<Q::Item<'_>>
    where
        Q: QueryShared,
    {
        self.view.get(entity)
    }

    pub fn get_mut(&mut self, entity: Entity) -> Option<Q::Item<'_>> {
        self.view.get_mut(&entity)
    }

    pub fn contains(&self, entity: &Entity) -> bool {
        self.view.contains(entity)
    }

    pub unsafe fn get_unchecked(&self, entity: &Entity) -> Option<Q::Item<'_>> {
        unsafe { self.view.get_unchecked(entity) }
    }

    pub fn get_disjoint_mut<const N: usize>(
        &mut self,
        entities: [Entity; N],
    ) -> [Option<Q::Item<'_>>; N] {
        self.view.get_disjoint_mut(entities)
    }

    pub fn iter_mut(&mut self) -> ViewIter<'_, Q> {
        self.view.iter_mut()
    }
}

impl<Q: Query> Drop for ViewBorrow<'_, Q> {
    fn drop(&mut self) {
        self.cache.release_borrow(self.view.tables);
    }
}

impl<'a, Q: Query> IntoIterator for &'a mut ViewBorrow<'_, Q> {
    type IntoIter = ViewIter<'a, Q>;
    type Item = Q::Item<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

struct ChunkIter<Q: Query> {
    fetch: Q::Fetch,
    position: usize,
    len: usize,
}

impl<Q: Query> ChunkIter<Q> {
    fn new(table: &Table, fetch: Q::Fetch) -> Self {
        Self {
            fetch,
            position: 0,
            len: table.row_count(),
        }
    }

    fn empty() -> Self {
        Self {
            fetch: Q::Fetch::dangling(),
            position: 0,
            len: 0,
        }
    }

    unsafe fn next<'a>(&mut self) -> Option<Q::Item<'a>> {
        if self.position == self.len {
            return None;
        }
        let item = unsafe { Q::get(&self.fetch, self.position) };
        self.position = self.position + 1;
        Some(item)
    }

    fn remaining(&self) -> usize {
        self.len - self.position
    }
}

pub struct TableIter<Q: Query> {
    tables: Range<usize>,
    cache: CachedQuery<Q::Fetch>,
}

impl<Q: Query> TableIter<Q> {
    fn new(cache: CachedQuery<Q::Fetch>) -> Self {
        Self {
            tables: 0..cache.table_count(),
            cache,
        }
    }

    unsafe fn next(&mut self, world: &World) -> Option<ChunkIter<Q>> {
        loop {
            let Some((table, state)) = (unsafe {
                self.cache
                    .get_state(world.table_graph(), self.tables.next()?)
            }) else {
                continue;
            };

            let fetch = Q::Fetch::execute(table, state);
            return Some(ChunkIter::new(table, fetch));
        }
    }

    fn entity_len(&self, world: &World) -> usize {
        self.tables
            .clone()
            .filter_map(|x| unsafe { self.cache.get_table(world.table_graph(), x) })
            .map(|x| x.row_count())
            .sum()
    }
}

/// 遍历`Q`查询到的组件集合迭代器
pub struct QueryIter<'q, Q: Query> {
    world: &'q World,
    tables: TableIter<Q>,
    iter: ChunkIter<Q>,
}

impl<'q, Q: Query> QueryIter<'q, Q> {
    unsafe fn new(world: &'q World, cache: CachedQuery<Q::Fetch>) -> Self {
        Self {
            world,
            tables: TableIter::new(cache),
            iter: ChunkIter::empty(),
        }
    }
}

unsafe impl<Q: Query> Send for QueryIter<'_, Q> where for<'a> Q::Item<'a>: Send {}
unsafe impl<Q: Query> Sync for QueryIter<'_, Q> where for<'a> Q::Item<'a>: Sync {}

impl<Q: Query> ExactSizeIterator for QueryIter<'_, Q> {
    fn len(&self) -> usize {
        self.tables.entity_len(self.world) + self.iter.remaining()
    }
}

impl<'q, Q: Query> Iterator for QueryIter<'q, Q> {
    type Item = Q::Item<'q>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match unsafe { self.iter.next() } {
                Some(components) => {
                    return Some(components);
                }
                None => unsafe { self.iter = self.tables.next(self.world)? },
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }
}

/// [`QueryIter`] 的 批处理版本
pub struct BatchedIter<'q, Q: Query> {
    _marker: PhantomData<&'q Q>,
    entity_datas: &'q SparseSet<Entity, EntityData>,
    tables: &'q TableGraph,
    index_iter: Range<usize>,
    batch_size: usize,
    cache: CachedQuery<Q::Fetch>,
    batch: usize,
}

pub struct Batch<'q, Q: Query> {
    entity_datas: &'q SparseSet<Entity, EntityData>,
    states: ChunkIter<Q>,
}

impl<'q, Q: Query> Iterator for Batch<'q, Q> {
    type Item = Q::Item<'q>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe { self.states.next() }
    }
}

unsafe impl<Q: Query> Send for Batch<'_, Q> where for<'a> Q::Item<'a>: Send {}
unsafe impl<Q: Query> Sync for Batch<'_, Q> where for<'a> Q::Item<'a>: Sync {}

impl<'q, Q: Query> BatchedIter<'q, Q> {
    unsafe fn new(
        entity_datas: &'q SparseSet<Entity, EntityData>,
        tables: &'q TableGraph,
        batch_size: usize,
        cache: CachedQuery<Q::Fetch>,
    ) -> Self {
        Self {
            _marker: PhantomData,
            entity_datas,
            tables,
            index_iter: (0..cache.table_count()),
            batch_size,
            cache,
            batch: 0,
        }
    }
}

unsafe impl<Q: Query> Send for BatchedIter<'_, Q> where for<'a> Q::Item<'a>: Send {}
unsafe impl<Q: Query> Sync for BatchedIter<'_, Q> where for<'a> Q::Item<'a>: Sync {}

impl<'q, Q: Query> Iterator for BatchedIter<'q, Q> {
    type Item = Batch<'q, Q>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut indices = self.index_iter.clone();
            let index = indices.next()?;
            let Some((table, state)) = (unsafe { self.cache.get_state(self.tables, index) }) else {
                self.index_iter = indices;
                continue;
            };
            let offset = self.batch_size * self.batch;
            if offset >= table.row_count() {
                self.index_iter = indices;
                self.batch = 0;
                continue;
            }
            let fetch = Q::Fetch::execute(table, state);
            self.batch += 1;
            let mut states = ChunkIter::new(table, fetch);
            states.position = offset as usize;
            states.len = offset + self.batch_size.min(table.row_count() - offset);
            return Some(Batch {
                entity_datas: self.entity_datas,
                states,
            });
        }
    }
}

/// 借用 [`World`] 来执行查询`Q`
///
/// 该对象被Drop时会释放借用
pub struct QueryBorrow<'w, Q: Query> {
    world: &'w World,
    cache: Option<CachedQuery<Q::Fetch>>,
}

unsafe impl<Q: Query> Send for QueryBorrow<'_, Q> where for<'a> Q::Item<'a>: Send {}
unsafe impl<Q: Query> Sync for QueryBorrow<'_, Q> where for<'a> Q::Item<'a>: Sync {}

impl<'w, Q: Query> QueryBorrow<'w, Q> {
    pub fn new(world: &'w World) -> Self {
        Self { world, cache: None }
    }

    pub fn iter(&mut self) -> QueryIter<'_, Q> {
        let cache = self.borrow().clone();
        unsafe { QueryIter::new(self.world, cache) }
    }

    pub fn view(&mut self) -> View<'_, Q> {
        let cache = self.borrow().clone();
        unsafe { View::new(self.world.entity_datas(), self.world.table_graph(), cache) }
    }

    fn borrow(&mut self) -> &CachedQuery<Q::Fetch> {
        self.cache.get_or_insert_with(|| {
            let cache = CachedQuery::get(self.world);
            cache.borrow(self.world.table_graph());
            cache
        })
    }

    pub fn iter_batched(&mut self, batch_size: usize) -> BatchedIter<'_, Q> {
        let cache = self.borrow().clone();
        unsafe {
            BatchedIter::new(
                self.world.entity_datas(),
                self.world.table_graph(),
                batch_size,
                cache,
            )
        }
    }

    fn transform<R: Query>(self) -> QueryBorrow<'w, R> {
        QueryBorrow {
            world: self.world,
            cache: None
        }
    }

    pub fn with<R: Query>(self) -> QueryBorrow<'w, With<Q, R>> {
        self.transform()
    }

    pub fn without<R: Query>(self) -> QueryBorrow<'w, Without<Q, R>> {
        self.transform()
    }
}

impl<Q: Query> Drop for QueryBorrow<'_, Q> {
    fn drop(&mut self) {
        if let Some(cache) = &self.cache {
            cache.release_borrow(self.world.table_graph());
        }
    }
}

impl<'q, Q: Query> IntoIterator for &'q mut QueryBorrow<'_, Q> {
    type Item = Q::Item<'q>;
    type IntoIter = QueryIter<'q, Q>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct PreparedQuery<Q: Query> {
    tables_generation: TableGraphGeneration,
    states: Box<[(NodeIndex, <Q::Fetch as Fetch>::State)]>,
    fetches: Box<[Option<Q::Fetch>]>,
}

pub struct PreparedQueryBorrow<'q, Q: Query> {
    entity_datas: &'q SparseSet<Entity, EntityData>,
    tables: &'q TableGraph,
    states: &'q [(NodeIndex, <Q::Fetch as Fetch>::State)],
    fetches: &'q mut [Option<Q::Fetch>],
}

pub struct PreparedQueryIter<'q, Q: Query> {
    entity_datas: &'q SparseSet<Entity, EntityData>,
    tables: &'q TableGraph,
    states: SliceIter<'q, (NodeIndex, <Q::Fetch as Fetch>::State)>,
    iter: ChunkIter<Q>,
}

pub struct PreparedView<'q, Q: Query> {
    entity_datas: &'q SparseSet<Entity, EntityData>,
    tables: &'q TableGraph,
    fetches: &'q mut [Option<Q::Fetch>],
}

unsafe impl<Q: Query> Send for PreparedView<'_, Q> where for<'a> Q::Item<'a>: Send {}
unsafe impl<Q: Query> Sync for PreparedView<'_, Q> where for<'a> Q::Item<'a>: Sync {}

impl<'q, Q: Query> PreparedView<'q, Q> {
    unsafe fn new(
        entity_datas: &'q SparseSet<Entity, EntityData>,
        tables: &'q TableGraph,
        states: SliceIter<'q, (NodeIndex, <Q::Fetch as Fetch>::State)>,
        fetches: &'q mut [Option<Q::Fetch>],
    ) -> Self {
        fetches.iter_mut().for_each(|fetch| *fetch = None);

        for (idx, state) in states {
            let table = &tables[*idx];
            fetches[idx.index()] = Some(Q::Fetch::execute(table, *state))
        }

        Self {
            entity_datas,
            tables,
            fetches,
        }
    }

    pub unsafe fn get_unchecked(&self, entity: &Entity) -> Option<Q::Item<'_>> {
        let entity_data = self.entity_datas.get(entity)?;

        self.fetches[entity_data.table_index().index()]
            .as_ref()
            .map(|fetch| unsafe { Q::get(fetch, entity_data.row_index()) })
    }

    pub fn get(&self, entity: &Entity) -> Option<Q::Item<'_>>
    where
        Q: QueryShared,
    {
        unsafe { self.get_unchecked(entity) }
    }

    pub fn get_mut(&mut self, entity: &Entity) -> Option<Q::Item<'_>> {
        unsafe { self.get_unchecked(entity) }
    }

    pub fn get_disjoint_mut<const N: usize>(
        &mut self,
        entities: [Entity; N],
    ) -> [Option<Q::Item<'_>>; N] {
        assert_distinct(&entities);

        let mut items = [(); N].map(|()| None);

        for (item, entity) in items.iter_mut().zip(entities) {
            unsafe {
                *item = self.get_unchecked(&entity);
            }
        }

        items
    }

    pub fn iter_mut(&mut self) -> ViewIter<'_, Q> {
        ViewIter {
            entity_datas: self.entity_datas,
            tables: self.tables.iter(),
            fetches: self.fetches.iter(),
            iter: ChunkIter::empty(),
        }
    }
}

impl<'a, Q: Query> IntoIterator for &'a mut PreparedView<'_, Q> {
    type IntoIter = ViewIter<'a, Q>;
    type Item = Q::Item<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

unsafe impl<Q: Query> Send for PreparedQueryIter<'_, Q> where for<'a> Q::Item<'a>: Send {}
unsafe impl<Q: Query> Sync for PreparedQueryIter<'_, Q> where for<'a> Q::Item<'a>: Sync {}

impl<'q, Q: Query> PreparedQueryIter<'q, Q> {
    unsafe fn new(
        entity_datas: &'q SparseSet<Entity, EntityData>,
        tables: &'q TableGraph,
        states: SliceIter<'q, (NodeIndex, <Q::Fetch as Fetch>::State)>,
    ) -> Self {
        Self {
            entity_datas,
            tables: tables,
            states,
            iter: ChunkIter::empty(),
        }
    }
}

impl<Q: Query> ExactSizeIterator for PreparedQueryIter<'_, Q> {
    fn len(&self) -> usize {
        self.states
            .clone()
            .map(|(idx, _)| self.tables[*idx].row_count())
            .sum::<usize>()
            + self.iter.remaining()
    }
}

impl<'q, Q: Query> Iterator for PreparedQueryIter<'q, Q> {
    type Item = Q::Item<'q>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match unsafe { self.iter.next() } {
                Some(components) => {
                    return Some(components);
                }
                None => {
                    let (idx, state) = self.states.next()?;
                    let table = &self.tables[*idx];
                    self.iter = ChunkIter::new(table, Q::Fetch::execute(table, *state));
                    continue;
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }
}

impl<'q, Q: Query> PreparedQueryBorrow<'q, Q> {
    fn new(
        entity_datas: &'q SparseSet<Entity, EntityData>,
        tables: &'q TableGraph,
        states: &'q [(NodeIndex, <Q::Fetch as Fetch>::State)],
        fetches: &'q mut [Option<Q::Fetch>],
    ) -> Self {
        for (idx, state) in states {
            if tables[*idx].is_emptry() {
                continue;
            }
            Q::Fetch::borrow(&tables[*idx], *state);
        }

        Self {
            entity_datas,
            tables,
            states,
            fetches,
        }
    }

    pub fn iter(&mut self) -> PreparedQueryIter<'_, Q> {
        unsafe { PreparedQueryIter::new(self.entity_datas, self.tables, self.states.iter()) }
    }

    pub fn view(&mut self) -> PreparedView<'_, Q> {
        unsafe {
            PreparedView::new(
                self.entity_datas,
                self.tables,
                self.states.iter(),
                self.fetches,
            )
        }
    }
}

impl<Q: Query> Drop for PreparedQueryBorrow<'_, Q> {
    fn drop(&mut self) {
        for (idx, state) in self.states {
            if self.tables[*idx].is_emptry() {
                continue;
            }

            Q::Fetch::release(&self.tables[*idx], *state);
        }
    }
}

impl<Q: Query> PreparedQuery<Q> {
    pub fn new() -> Self {
        Self {
            tables_generation: Default::default(),
            states: Default::default(),
            fetches: Default::default(),
        }
    }

    fn prepare(world: &World) -> Self {
        let tables_generation = world.table_graph().gneeration();

        let states = world
            .table_graph_iter()
            .enumerate()
            .filter_map(|(idx, x)| {
                Q::Fetch::prepare(&x.weight).map(|state| (NodeIndex::new(idx), state))
            })
            .collect();

        let fetches = world.table_graph_iter().map(|_| None).collect();

        Self {
            tables_generation,
            states,
            fetches,
        }
    }

    pub fn query<'q>(&'q mut self, world: &'q World) -> PreparedQueryBorrow<'q, Q> {
        if self.tables_generation != world.table_graph().gneeration() {
            *self = Self::prepare(world);
        }

        let entity_datas = world.entity_datas();
        let tables = world.table_graph();

        PreparedQueryBorrow::new(entity_datas, tables, &self.states, &mut self.fetches)
    }

    pub fn query_mut<'q>(&'q mut self, world: &'q mut World) -> PreparedQueryIter<'q, Q> {
        assert_borrow::<Q>();

        if self.tables_generation != world.table_graph().gneeration() {
            *self = Self::prepare(world)
        }

        let entity_datas = world.entity_datas();
        let tables = world.table_graph();

        unsafe { PreparedQueryIter::new(entity_datas, tables, self.states.iter()) }
    }

    pub fn view_mut<'q>(&'q mut self, world: &'q World) -> PreparedView<'q, Q> {
        assert_borrow::<Q>();

        if self.tables_generation != world.table_graph().gneeration() {
            *self = Self::prepare(world)
        }

        let entity_datas = world.entity_datas();
        let tables = world.table_graph();

        unsafe { PreparedView::new(entity_datas, tables, self.states.iter(), &mut self.fetches) }
    }
}

impl<Q: Query> Default for PreparedQuery<Q> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct QueryMut<'q, Q: Query> {
    world: &'q mut World,
    _marker: PhantomData<fn() -> Q>,
}

impl<'q, Q: Query> QueryMut<'q, Q> {
    pub fn new(world: &'q mut World) -> Self {
        assert_borrow::<Q>();

        Self {
            world,
            _marker: PhantomData,
        }
    }

    pub fn view(&mut self) -> View<'_, Q> {
        let cache = CachedQuery::get(self.world);
        unsafe { View::new(self.world.entity_datas(), self.world.table_graph(), cache) }
    }

    fn transform<R: Query>(self) -> QueryMut<'q, R> {
        QueryMut {
            world: self.world,
            _marker: PhantomData,
        }
    }

    pub fn with<R: Query>(self) -> QueryMut<'q, With<Q, R>> {
        self.transform()
    }

    pub fn without<R: Query>(self) -> QueryMut<'q, Without<Q, R>> {
        self.transform()
    }

    pub fn into_iter_batched(self, batch_size: usize) -> BatchedIter<'q, Q> {
        let cache = CachedQuery::get(self.world);
        unsafe {
            BatchedIter::new(self.world.entity_datas(), self.world.table_graph(), batch_size, cache)
        }
    }
}

impl<'q, Q: Query> IntoIterator for QueryMut<'q, Q> {
    type Item = <QueryIter<'q, Q> as Iterator >::Item;
    type IntoIter = QueryIter<'q, Q>;

    fn into_iter(self) -> Self::IntoIter {
        let cache = CachedQuery::get(self.world);
        unsafe { QueryIter::new(self.world, cache) }
    }
}





pub fn assert_distinct<const N: usize>(entities: &[Entity; N]) {
    match N {
        1 => (),
        2 => assert_ne!(entities[0], entities[1]),
        3 => {
            assert_ne!(entities[0], entities[1]);
            assert_ne!(entities[1], entities[2]);
            assert_ne!(entities[2], entities[0]);
        }
        _ => {
            let mut entities = entities.clone();
            entities.sort_unstable();
            for index in 0..N - 1 {
                assert_ne!(entities[index], entities[index + 1]);
            }
        }
    }
}

pub fn assert_borrow<Q: Query>() {
    let mut i = 0;
    Q::Fetch::for_each_borrow(|a, unique| {
        if unique {
            let mut j = 0;
            Q::Fetch::for_each_borrow(|b, _| {
                if i != j {
                    core::assert!(a != b, "query violates a unique borrow");
                }
                j += 1;
            });
        }
        i += 1;
    });
}






// unsafe impl<A: Fetch, B: Fetch> Fetch for (A, B) {
//     type State = (A::State, B::State);

//     fn dangling() -> Self {
//         (A::dangling(), B::dangling())
//     }

//     fn access(table: &Table) -> Option<Access> {
//         let mut acess = Access::Iterate;
//         acess = acess.max(A::access(table)?);
//         acess = acess.max(B::access(table)?);
//         Some(acess)
//     }

//     fn borrow(table: &Table, state: Self::State) {
//         let (a, b) = state;
//         A::borrow(table, a);
//         B::borrow(table, b);
//     }

//     fn prepare(table: &Table) -> Option<Self::State> {
//         Some((A::prepare(table)?, B::prepare(table)?))
//     }

//     fn execute(table: &Table, state: Self::State) -> Self {
//         let (a, b) = state;
//         (A::execute(table, a), B::execute(table, b))
//     }

//     fn release(table: &Table, state: Self::State) {
//         let (a, b) = state;
//         A::release(table, a);
//         B::release(table, b);
//     }

//     fn for_each_borrow<F: FnMut(TypeId, bool)>(mut f: F) {
//         A::for_each_borrow(&mut f);
//         B::for_each_borrow(&mut f);
//     }
// }

// impl<A: Query, B: Query> Query for (A, B) {
//     type Item<'a> = (A::Item<'a>, B::Item<'a>);

//     type Fetch = (A::Fetch, B::Fetch);

//     unsafe fn get<'a>(fetch: &Self::Fetch, n: usize) -> Self::Item<'a> {
//         let (a, b) = fetch;
//         (A::get(a, n), B::get(b, n))
//     }
// }


macro_rules! tuple_impl {
    ($($name: ident),*) => {
        unsafe impl<$($name: Fetch),*> Fetch for ($($name,)*) {
            type State = ($($name::State,)*);

            #[allow(clippy::unused_unit)]
            fn dangling() -> Self {
                ($($name::dangling(),)*)
            }

            #[allow(unused_variables, unused_mut)]
fn access(table: &Table) -> Option<Access> {
                let mut access = Access::Iterate;
                $(
                    access = access.max($name::access(table)?);
                )*
                Some(access)
            }

            #[allow(unused_variables, non_snake_case, clippy::unused_unit)]
            fn borrow(table: &Table, state: Self::State) {
                let ($($name,)*) = state;
                $($name::borrow(table, $name);)*
            }
            #[allow(unused_variables)]
            #[cold]
            fn prepare(table: &Table) -> Option<Self::State> {
                Some(($($name::prepare(table)?,)*))
            }
            #[allow(unused_variables, non_snake_case, clippy::unused_unit)]
            #[inline(always)]
            fn execute(table: &Table, state: Self::State) -> Self {
                let ($($name,)*) = state;
                ($($name::execute(table, $name),)*)
            }
            #[allow(unused_variables, non_snake_case, clippy::unused_unit)]
            fn release(table: &Table, state: Self::State) {
                let ($($name,)*) = state;
                $($name::release(table, $name);)*
            }

            #[allow(unused_variables, unused_mut, clippy::unused_unit)]
            fn for_each_borrow<__F: FnMut(TypeId, bool)>(mut f: __F) {
                $($name::for_each_borrow(&mut f);)*
            }
        }

        impl<$($name: Query),*> Query for ($($name,)*) {
            type Item<'q> = ($($name::Item<'q>,)*);

            type Fetch = ($($name::Fetch,)*);

            #[allow(unused_variables, clippy::unused_unit, unsafe_op_in_unsafe_fn)]
            unsafe fn get<'q>(fetch: &Self::Fetch, n: usize) -> Self::Item<'q> {
                #[allow(non_snake_case)]
                let ($(ref $name,)*) = *fetch;
                ($(unsafe { $name::get($name, n) },)*)
            }
        }

        unsafe impl<$($name: QueryShared),*> QueryShared for ($($name,)*) {}
    };
}

smaller_tuples_too!(tuple_impl, O, N, M, L, K, J, I, H, G, F, E, D, C, B, A);