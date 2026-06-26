use std::{
    any::TypeId,
    collections::{HashMap, hash_map::Entry},
    fmt::Debug,
    hash::{BuildHasher, BuildHasherDefault, Hasher},
    ops::Add,
    ptr,
    sync::Mutex,
};

use petgraph::graph::{Node, NodeIndex};

use crate::ecs::{
    batch::ColumBatch,
    component::{Component, ComponentError, MissingComponent},
    component_tuple::{
        CachedQuery, ComponentTuple, ComponentTupleKey, DynamicComponentTuple, Fetch, Query,
        QueryBorrow, QueryCache, QueryMut, QueryOne, QueryOneError, View, ViewBorrow,
        assert_borrow, assert_distinct,
    },
    consts,
    entity::{Entity, EntityFlag},
    entity_ref::{ComponentRef, EntityRef},
    id::Id,
    sparse_set::{self, AllocManyState, EntityStorage, NoSuchId, SparseSet},
    table::Table,
    table_graph::{InsertTarget, TableGraph, TableGraphGeneration},
    take::TakeEntity,
};

#[derive(Debug)]
pub struct EntityData {
    table_index: NodeIndex,
    row_index: usize,
}
impl Default for EntityData {
    fn default() -> Self {
        Self {
            table_index: NodeIndex::new(0),
            row_index: usize::MAX,
        }
    }
}
impl EntityData {
    #[inline(always)]
    pub fn table_index(&self) -> NodeIndex {
        self.table_index
    }

    #[inline(always)]
    pub fn row_index(&self) -> usize {
        self.row_index
    }

    #[inline(always)]
    pub fn set_row_index(&mut self, row_index: usize) {
        self.row_index = row_index
    }
}

#[derive(Debug, Default)]
struct NodeIndexTupleIdHasher(u64);
impl Hasher for NodeIndexTupleIdHasher {
    fn write_u32(&mut self, node_index: u32) {
        self.0 ^= u64::from(node_index);
    }

    fn write_u64(&mut self, type_id: u64) {
        self.0 ^= type_id;
    }

    fn finish(&self) -> u64 {
        todo!()
    }

    fn write(&mut self, _bytes: &[u8]) {
        unreachable!()
    }
}

#[derive(Debug, Default)]
struct TupleIdHasher(u64);
impl Hasher for TupleIdHasher {
    fn write_u64(&mut self, i: u64) {
        // 每个类型只能被Hash一次，即此时self.hash应该为0
        debug_assert_eq!(self.0, 0);

        self.0 = i;
    }
    fn write_u128(&mut self, i: u128) {
        debug_assert_eq!(self.0, 0);

        // u64位数足够，直接downcast到u64
        self.0 = i as u64;
    }
    fn write(&mut self, bytes: &[u8]) {
        debug_assert_eq!(self.0, 0);

        // 只有在 TypeId 既不是 u64, 也不是 u128 时才会发生，这通常不会出现
        let mut hasher = foldhash::fast::FixedState::with_seed(0xb334867b740a29a5).build_hasher();
        hasher.write(bytes);
        self.0 = hasher.finish();
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

type NodeIndexTupleIdMap<V> =
    HashMap<(NodeIndex, ComponentTupleKey), V, BuildHasherDefault<NodeIndexTupleIdHasher>>;
type TupleIdMap<V> = HashMap<ComponentTupleKey, V, BuildHasherDefault<TupleIdHasher>>;

#[derive(Debug)]
pub struct World {
    entities: EntityStorage,
    entity_datas: SparseSet<Entity, EntityData>,
    table_graph: TableGraph,
    // components_id_to_table: HashMap<Vec<ComponentId>, NodeIndex>,
    tuple_to_table: TupleIdMap<NodeIndex>,
    insert_edges: NodeIndexTupleIdMap<InsertTarget>,
    remove_edges: NodeIndexTupleIdMap<NodeIndex>,

    query_cache: QueryCache,

    // 后面这里的Scene概念应该会改为Chunk概念
    // 由Game里的各个功能组件/System来做区块划分并通过类似TagComponent进行控制
    // pub scene_stroge: SceneStroge,
    // scenes: SparseSet<SceneId, Scene>,
    _id: u64,
}

impl World {
    pub fn new() -> Self {
        static ID: Mutex<u64> = Mutex::new(1);
        let _id = {
            let mut id = ID.lock().unwrap();
            let next = id.add(1);
            *id = next;
            next
        };

        let entities = EntityStorage::new(consts::WORLD_ENTITIES_CAPACITY);
        let entity_datas = SparseSet::new(consts::WORLD_ENTITIES_CAPACITY);
        let table_graph = TableGraph::new(consts::WORLD_TABLE_GRAPH_CAPACITY);

        let tuple_to_table = TupleIdMap::default();
        let insert_edges = NodeIndexTupleIdMap::default();
        let remove_edges = NodeIndexTupleIdMap::default();

        Self {
            entities,
            entity_datas,
            tuple_to_table,
            insert_edges,
            remove_edges,
            table_graph,
            query_cache: QueryCache::default(),
            _id,
        }
    }

    /// 给一个实体添加组件
    pub fn insert<T: DynamicComponentTuple>(
        &mut self,
        entity: Entity,
        components: T,
    ) -> Result<(), NoSuchId> {
        self.flush();

        match self.entity_datas.get(entity) {
            Some(data) => {
                let src_table = data.table_index;
                let row_index = data.row_index;
                self.insert_inner(entity, components, src_table, row_index);
                Ok(())
            }
            None => Err(NoSuchId),
        }
    }

    fn insert_inner<T: DynamicComponentTuple>(
        &mut self,
        entity: Entity,
        components: T,
        src_table: NodeIndex,
        row_index: usize,
    ) {
        let target;
        let target_ref = match components.key() {
            Some(key) => match self.insert_edges.entry((src_table, key)) {
                Entry::Occupied(occupied_entry) => occupied_entry.into_mut(),
                Entry::Vacant(vacant_entry) => {
                    let target = self.table_graph.get_insert_target(src_table, &components);
                    vacant_entry.insert(target)
                }
            },
            None => {
                target = self.table_graph.get_insert_target(src_table, &components);
                &target
            }
        };

        let source_table = &mut self.table_graph[src_table];

        // drop老表中会被覆盖更新的行
        for ty in target_ref.get_need_updates() {
            let ptr = source_table.get_dynamice(ty, row_index).unwrap();
            ty.drop(ptr.as_ptr());
        }

        // 表没变，直接把components的最新值写入老表
        if target_ref.get_node_index() == src_table {
            unsafe {
                components.put(|ptr, info| {
                    source_table.put_dynamic(ptr, &info, row_index);
                });
            }
            return;
        }

        let (source_table, target_table) = self
            .table_graph
            .index2(src_table, target_ref.get_node_index());

        let target_row_index = target_table.allocate_entity(entity);
        let entity_data = &mut self.entity_datas[entity];
        entity_data.table_index = target_ref.get_node_index();
        entity_data.row_index = target_row_index;

        // 写入components到新表
        unsafe {
            components.put(|ptr, info| {
                target_table.put_dynamic(ptr, &info, target_row_index);
            });
        }

        // 转移需要转移的老表中的数据
        for info in target_ref.get_need_moves() {
            let src = source_table.get_dynamice(info, row_index).unwrap();
            target_table.put_dynamic(src.as_ptr(), info, target_row_index);
        }

        // remove 老表的entity，并更新这里entity data 的 row_index
        // 会被覆盖更新的，我们在前面drop了老数据。其他数据相当于是移动到新表(转移所有权而非被销毁)，因此不需要drop
        if let Some(moved) = source_table.remove_entity(entity, false) {
            let moved_data = &mut self.entity_datas[moved];
            moved_data.row_index = row_index;
        }
    }

    pub fn insert_one<T: Component>(
        &mut self,
        entity: Entity,
        component: T,
    ) -> Result<(), NoSuchId> {
        self.insert(entity, (component,))
    }

    /// 把预留但未挂在任何组件的entity放入根表中
    /// 在添加或删除组件或实体时，必须先调用该方法
    /// 例如 spawn, despawn, insert and remove
    pub fn flush(&mut self) {
        let root_node = NodeIndex::new(0);
        let root_table = &mut self.table_graph[root_node];
        self.entities.flush(|entity| {
            self.entity_datas.insert(
                entity,
                EntityData {
                    table_index: root_node,
                    row_index: root_table.allocate_entity(entity),
                },
            );
        });
    }

    /// 创建带有组件的实体
    pub fn spawn<T: DynamicComponentTuple>(&mut self, components: T) -> Entity {
        self.flush();

        let entity = self.entities.alloc();
        self.spawn_inner(entity, components);
        entity
    }

    fn spawn_inner<T: DynamicComponentTuple>(&mut self, entity: Entity, components: T) {
        let tables = &mut self.table_graph;
        let table_index = match components.key() {
            Some(key) => *self.tuple_to_table.entry(key).or_insert(
                components.with_ids(|types| tables.get(types, || components.type_infos())),
            ),
            None => components.with_ids(|types| tables.get(types, || components.type_infos())),
        };

        let table = &mut tables[table_index];
        let row_index = table.allocate_entity(entity);
        unsafe {
            components.put(|ptr, info| {
                table.put_dynamic(ptr, &info, row_index);
            });
        }
        self.entity_datas.insert(
            entity,
            EntityData {
                table_index,
                row_index,
            },
        );
    }

    /// 给指定的实体重新分配组件
    /// 如果Entity已存在，则会用传入的Entity覆盖， 并删除原本Entity的组件，然后用输入的新组件重新创建
    /// 如果Entity不存在，则会创建这个指定的Entity，并添加输入的组件
    pub fn spawn_at<T: DynamicComponentTuple>(&mut self, entity: Entity, components: T) {
        self.flush();

        let entity = self.alloc_at_inner(entity);

        self.spawn_inner(entity, components);
    }

    fn alloc_at_inner(&mut self, entity: Entity) -> Entity {
        match self.entities.alloc_at(entity) {
            sparse_set::AllocAt::New(range) => {
                for idx in range {
                    self.entity_datas.insert(
                        Entity::new(idx as u32, 0, EntityFlag::Dead),
                        EntityData::default(),
                    );
                }
                self.entity_datas.insert(entity, EntityData::default());
                entity
            }
            sparse_set::AllocAt::BeUsed(entity) => entity,
            sparse_set::AllocAt::Using => {
                let entity_data = &self.entity_datas[entity];
                if let Some(moved) =
                    self.table_graph[entity_data.table_index].remove_entity(entity, true)
                {
                    self.entity_datas[moved].row_index = entity_data.row_index;
                }
                entity
            }
        }
    }

    /// 通过组件迭代器批量创建带这些组件的实体
    /// 调用后只会为这些实体预留分配内存，而不会创建实际的数据
    /// 实际数据会在返回的迭代器中做懒创建
    pub fn spawn_batch<I>(&mut self, iter: I) -> SpawnBatchIter<'_, I::IntoIter>
    where
        I: IntoIterator,
        I::Item: ComponentTuple + 'static,
    {
        self.flush();

        let iter = iter.into_iter();
        let (lower, upper) = iter.size_hint();
        let table_index = self.reserve_inner::<I::Item>(
            usize::try_from(upper.unwrap_or(lower)).expect("iterator too larget"),
        );

        SpawnBatchIter {
            inner: iter,
            entities: &mut self.entities,
            entity_datas: &mut self.entity_datas,
            table_index,
            table: &mut self.table_graph[table_index],
        }
    }

    /// 高效的生成 [`ColumBatch`] 中的内容
    pub fn spawn_colum_batch(&mut self, batch: ColumBatch) -> SpawnColumnBatchIter<'_> {
        self.flush();

        let table = batch.0;
        let entity_count = table.row_count();
        let (table_index, mut start_row_index) = self.table_graph.insert_batch(table);

        let table = &mut self.table_graph[table_index];
        let entity_alloc_many = self.entities.alloc_many(entity_count);

        for &id in &self.entities.iter().as_slice()[entity_alloc_many.pending_range.clone()] {
            self.entity_datas[id] = EntityData {
                table_index,
                row_index: start_row_index,
            };

            table.set_entity_id(start_row_index, id.idx());
            start_row_index = start_row_index + 1;
        }

        for index in entity_alloc_many.fresh.clone() {
            self.entity_datas.insert(
                Entity::new(index as u32, 0, EntityFlag::Default),
                EntityData {
                    table_index,
                    row_index: start_row_index,
                },
            );
            table.set_entity_id(start_row_index, index as u32);
            start_row_index = start_row_index + 1;
        }

        SpawnColumnBatchIter {
            entity_alloc: entity_alloc_many,
            entities: &mut self.entities,
        }
    }

    /// 生成 [`ColumBatch`] 的数据，并将其中的实体替换为输入的实体
    /// 这要求输入的实体切片长度等于[`ColumBatch`]中的实体数量
    pub fn spawn_colum_batch_at(&mut self, handles: &[Entity], batch: ColumBatch) {
        let table = batch.0;
        debug_assert_eq!(
            handles.len(),
            table.row_count(),
            "number of entity {} must match number of entities {}",
            handles.len(),
            table.row_count()
        );

        for handle in handles {
            let _ = self.alloc_at_inner(*handle);
        }

        let (table_index, start_row_index) = self.table_graph.insert_batch(table);

        let table = &mut self.table_graph[table_index];
        for (handle, index) in handles.iter().zip(start_row_index as usize..) {
            table.set_entity_id(index, handle.idx());
            self.entity_datas.insert(
                *handle,
                EntityData {
                    table_index,
                    row_index: index,
                },
            );
        }
    }

    /// 销毁一个实体和它身上的所有组件
    pub fn despawn(&mut self, entity: Entity) -> Result<(), NoSuchId> {
        self.flush();

        let moved = self.entities.free(entity)?;
        match self.entity_datas.remove(entity, moved) {
            Some(entity_data) => {
                if let Some(moved) =
                    self.table_graph[entity_data.table_index].remove_entity(entity, true)
                {
                    self.entity_datas[moved].row_index = entity_data.row_index;
                }
                Ok(())
            }
            None => Err(NoSuchId),
        }
    }

    /// despawn 所有实体
    ///
    /// 但会保留所有已分配的内存，以便后续可能的重用
    pub fn clear(&mut self) {
        for x in self.table_graph.get_tables_mut() {
            x.clear();
        }
        self.entities.clear();
        self.entity_datas.clear();
    }

    /// 预留分配出 additional 个 带 'T' 组件的实体
    /// 这会分配出数据的内存，但并不会创建有效数据，该方法主要用于需要批量spawn时，避免多个单次spawn导致的内存频繁realloc
    pub fn reserve<T: ComponentTuple + 'static>(&mut self, additional: usize) {
        self.reserve_inner::<T>(additional);
    }

    fn reserve_inner<T: ComponentTuple + 'static>(&mut self, additional: usize) -> NodeIndex {
        self.flush();

        if let Some(shortfall) = self.entities.reserve(additional) {
            self.entity_datas.reserve(shortfall);
        }

        let tables = &mut self.table_graph;
        let table_id = *self
            .tuple_to_table
            .entry(ComponentTupleKey::from(TypeId::of::<T>()))
            .or_insert_with(|| {
                T::with_static_ids(|ids| {
                    tables.get(ids, || {
                        T::with_static_type_info(|info| info.iter().copied().collect::<Box<[_]>>())
                    })
                })
            });

        tables[table_id].reserve(additional);
        table_id
    }

    pub fn contains(&self, entity: Entity) -> bool {
        self.entities.has(entity)
    }

    /// 从实体身上移除'T'组件
    pub fn remove<T: ComponentTuple + 'static>(
        &mut self,
        entity: Entity,
    ) -> Result<T, ComponentError> {
        self.flush();

        match self.entity_datas.get_mut(entity) {
            Some(entity_data) => {
                let old_row_index = entity_data.row_index;
                let source_table = &self.table_graph[entity_data.table_index];

                let tuple =
                    unsafe { T::get(|info| source_table.get_dynamice(&info, old_row_index))? };

                let target = Self::remove_target::<T>(
                    &mut self.table_graph,
                    &mut self.remove_edges,
                    entity_data.table_index,
                );

                if entity_data.table_index != target {
                    let (source_table, target_table) =
                        self.table_graph.index2(entity_data.table_index, target);
                    let target_row_index = target_table.allocate_entity(entity);
                    entity_data.table_index = target;
                    entity_data.row_index = target_row_index;
                    if let Some(moved) = unsafe {
                        source_table.move_to(old_row_index, |src, info| {
                            if let Some(dst) = target_table.get_dynamice(info, target_row_index) {
                                ptr::copy_nonoverlapping(src, dst.as_ptr(), info.layout().size());
                            }
                        })
                    } {
                        self.entity_datas[moved].row_index = old_row_index
                    }
                }

                Ok(tuple)
            }
            None => Err(ComponentError::NoSuchEntity),
        }
    }

    fn remove_target<T: ComponentTuple + 'static>(
        tables: &mut TableGraph,
        remove_edges: &mut NodeIndexTupleIdMap<NodeIndex>,
        old_table: NodeIndex,
    ) -> NodeIndex {
        match remove_edges.entry((old_table, ComponentTupleKey::from(TypeId::of::<T>()))) {
            Entry::Occupied(occupied_entry) => *occupied_entry.into_mut(),
            Entry::Vacant(vacant_entry) => {
                let infos = T::with_static_type_info(|removed| {
                    tables[old_table]
                        .types()
                        .iter()
                        .filter(|x| removed.binary_search(x).is_err())
                        .cloned()
                        .collect::<Box<_>>()
                });
                let elements = infos.iter().map(|x| x.id()).collect::<Box<_>>();
                let row_index = tables.get(elements, move || infos);
                *vacant_entry.insert(row_index)
            }
        }
    }

    /// 从实体身上移除单个'T'组件
    pub fn remove_one<T: Component>(&mut self, entity: Entity) -> Result<T, ComponentError> {
        self.remove::<(T,)>(entity).map(|(x,)| x)
    }

    /// 从实体身上移除 'S' 组件, 然后添加 'T' 组件
    pub fn exchange<S: ComponentTuple + 'static, T: DynamicComponentTuple>(
        &mut self,
        entity: Entity,
        components: T,
    ) -> Result<S, ComponentError> {
        self.flush();

        match self.entity_datas.get(entity) {
            Some(entity_data) => {
                let source_table = &self.table_graph[entity_data.table_index];

                let tuple = unsafe {
                    S::get(|info| source_table.get_dynamice(&info, entity_data.row_index))?
                };

                let intermediate = Self::remove_target::<S>(
                    &mut self.table_graph,
                    &mut self.remove_edges,
                    entity_data.table_index,
                );
                self.insert_inner(entity, components, intermediate, entity_data.row_index);

                Ok(tuple)
            }
            None => Err(ComponentError::NoSuchEntity),
        }
    }

    /// 从实体身上移除单个 'S' 组件，然后添加单个 'T' 组件
    pub fn exchange_one<S: Component, T: Component>(
        &mut self,
        entity: Entity,
        component: T,
    ) -> Result<S, ComponentError> {
        self.exchange::<(S,), (T,)>(entity, (component,))
            .map(|(x,)| x)
    }

    pub fn table_graph_generation(&self) -> TableGraphGeneration {
        self.table_graph.gneeration()
    }

    pub fn table_graph(&self) -> &TableGraph {
        &self.table_graph
    }

    pub fn table_graph_iter(
        &self,
    ) -> impl ExactSizeIterator<Item = &'_ petgraph::graph::Node<Table>> + '_ {
        self.table_graph.get_tables().iter()
    }

    pub fn query_cache(&self) -> &QueryCache {
        &self.query_cache
    }

    pub fn entity_datas(&self) -> &SparseSet<Entity, EntityData> {
        &self.entity_datas
    }

    pub fn query<Q: Query>(&self) -> QueryBorrow<'_, Q> {
        QueryBorrow::new(self)
    }

    pub fn view<Q: Query>(&self) -> ViewBorrow<'_, Q> {
        ViewBorrow::new(self)
    }

    pub fn query_mut<Q: Query>(&mut self) -> QueryMut<'_, Q> {
        QueryMut::new(self)
    }

    pub fn view_mut<Q: Query>(&mut self) -> View<'_, Q> {
        assert_borrow::<Q>();

        let cache = CachedQuery::get(self);
        unsafe { View::<Q>::new(&self.entity_datas, &self.table_graph, cache) }
    }

    pub fn query_one<Q: Query>(&self, entity: Entity) -> QueryOne<'_, Q> {
        let Some(loc) = self.entity_datas.get(entity) else {
            return QueryOne::default();
        };

        unsafe { QueryOne::new(&self.table_graph[loc.table_index], loc.row_index) }
    }

    pub fn query_one_mut<Q: Query>(
        &mut self,
        entity: Entity,
    ) -> Result<Q::Item<'_>, QueryOneError> {
        assert_borrow::<Q>();

        let loc = self.entity_datas.get(entity).ok_or(NoSuchId)?;
        let table = &self.table_graph[loc.table_index];
        let state = Q::Fetch::prepare(table).ok_or(QueryOneError::Unsatisfield)?;
        let fetch = Q::Fetch::execute(table, state);
        unsafe { Ok(Q::get(&fetch, loc.row_index)) }
    }

    pub fn query_disjoint_mut<Q: Query, const N: usize>(
        &mut self,
        entities: [Entity; N],
    ) -> [Result<Q::Item<'_>, QueryOneError>; N] {
        assert_borrow::<Q>();
        assert_distinct(&entities);

        entities.map(|entity| {
            let loc = self.entity_datas.get(entity).ok_or(NoSuchId)?;
            let table = &self.table_graph[loc.table_index];
            let state = Q::Fetch::prepare(table).ok_or(QueryOneError::Unsatisfield)?;
            let fetch = Q::Fetch::execute(table, state);
            unsafe { Ok(Q::get(&fetch, loc.row_index)) }
        })
    }

    pub fn entity_ref(&self, entity: Entity) -> Result<EntityRef<'_>, NoSuchId> {
        let loc = self.entity_datas.get(entity).ok_or(NoSuchId)?;
        unsafe {
            Ok(EntityRef::new(
                &self.table_graph[loc.table_index],
                loc.row_index,
            ))
        }
    }

    pub fn get<'a, T: ComponentRef<'a>>(
        &'a self,
        entity: Entity,
    ) -> Result<T::Ref, ComponentError> {
        Ok(self
            .entity_ref(entity)?
            .get::<T>()
            .ok_or_else(MissingComponent::new::<T::Component>)?)
    }

    pub unsafe fn get_unchecked<'a, T: ComponentRef<'a>>(&'a self, entity: Entity) -> T {
        let loc = unsafe { self.entity_datas.get_unchecked(entity) };
        let table = &self.table_graph[loc.table_index];
        unsafe {
            let state = table.get_state::<T::Component>().unwrap();
            T::from_raw(
                table
                    .get_base::<T::Component>(state)
                    .as_ptr()
                    .add(loc.row_index),
            )
        }
    }

    pub fn satisfies<Q: Query>(&self, entity: Entity) -> bool {
        self.entity_ref(entity)
            .map_or(false, |e| e.satisfies::<Q>())
    }

    pub unsafe fn find_entity_from_id(&self, id: u32) -> Entity {
        unsafe { self.entities.resolve_unknown_version(id) }
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter::new(&self.table_graph, &self.entity_datas)
    }

    pub fn take(&mut self, entity: Entity) -> Result<TakeEntity<'_>, NoSuchId> {
        self.flush();

        let loc = self.entity_datas.get(entity).ok_or(NoSuchId)?;
        let table = &mut self.table_graph[loc.table_index];
        unsafe {
            Ok(TakeEntity::new(
                &mut self.entities,
                &mut self.entity_datas,
                entity,
                table,
            ))
        }
    }
}

pub struct Iter<'a> {
    tables: core::slice::Iter<'a, Node<Table>>,
    entity_datas: &'a SparseSet<Entity, EntityData>,
    current: Option<&'a Node<Table>>,
    row_index: usize,
}

unsafe impl Send for Iter<'_> {}
unsafe impl Sync for Iter<'_> {}

impl<'a> Iter<'a> {
    fn new(tables: &'a TableGraph, entity_datas: &'a SparseSet<Entity, EntityData>) -> Self {
        Self {
            tables: tables.iter(),
            entity_datas,
            current: None,
            row_index: 0,
        }
    }
}

impl ExactSizeIterator for Iter<'_> {
    fn len(&self) -> usize {
        self.entity_datas.len()
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = EntityRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.current {
                Some(current) => {
                    if self.row_index == current.weight.row_count() {
                        self.current = None;
                        continue;
                    }
                    let row_index = self.row_index;
                    self.row_index = self.row_index + 1;
                    return Some(unsafe { EntityRef::new(&current.weight, row_index) });
                }
                None => {
                    self.current = Some(self.tables.next()?);
                    self.row_index = 0;
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}

/// [`World::spawn_batch`] 创建出来的迭代器
/// spawn_batch的实体和其组件数据在该迭代器中实现懒创建
pub struct SpawnBatchIter<'a, I>
where
    I: Iterator,
    I::Item: ComponentTuple,
{
    inner: I,
    entities: &'a mut EntityStorage,
    entity_datas: &'a mut SparseSet<Entity, EntityData>,
    table_index: NodeIndex,
    table: &'a mut Table,
}

impl<I> Iterator for SpawnBatchIter<'_, I>
where
    I: Iterator,
    I::Item: ComponentTuple,
{
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        let components = self.inner.next()?;
        let entity = self.entities.alloc();
        let row_index = self.table.allocate_entity(entity);
        unsafe {
            components.put(|ptr, info| {
                self.table.put_dynamic(ptr, &info, row_index);
            });
        }
        self.entity_datas.insert(
            entity,
            EntityData {
                table_index: self.table_index,
                row_index,
            },
        );
        Some(entity)
    }
}
impl<I, T> ExactSizeIterator for SpawnBatchIter<'_, I>
where
    I: ExactSizeIterator<Item = T>,
    T: ComponentTuple,
{
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<I> Drop for SpawnBatchIter<'_, I>
where
    I: Iterator,
    I::Item: ComponentTuple,
{
    fn drop(&mut self) {
        for _ in self {}
    }
}

pub struct SpawnColumnBatchIter<'a> {
    entity_alloc: AllocManyState,
    entities: &'a mut EntityStorage,
}

impl ExactSizeIterator for SpawnColumnBatchIter<'_> {
    fn len(&self) -> usize {
        self.entity_alloc.len()
    }
}

impl Iterator for SpawnColumnBatchIter<'_> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.entity_alloc.next()?;
        Some(unsafe { self.entities.flush_alloc_many(index) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}
impl Drop for SpawnColumnBatchIter<'_> {
    fn drop(&mut self) {
        self.entities.finish_alloc_many(&mut self.entity_alloc);
    }
}
