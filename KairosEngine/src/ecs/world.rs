use std::{
    any::TypeId,
    collections::{HashMap, hash_map::Entry},
    hash::{BuildHasher, BuildHasherDefault, Hasher},
    ops::Add,
    ptr,
    sync::Mutex,
};

use petgraph::graph::NodeIndex;

use crate::{
    asset_loader::assets::AssetsServer,
    ecs::{
        component::{Component, ComponentError},
        component_tuple::{ComponentTuple, ComponentTupleKey, StaticTypedComponentTuple},
        consts,
        entity::{Entity, EntityFlag},
        id::Id,
        sparse_set::{self, EntityStorage, SparseSet},
        table_graph::{InsertTarget, TableGraph},
    },
    timer::Time,
};

#[derive(Debug)]
pub struct EntityData {
    table_index: NodeIndex,
    row_index: usize,
}
impl Default for EntityData {
    fn default() -> Self {
        Self { table_index: NodeIndex::new(0), row_index: usize::MAX }
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

pub type NodeIndexTupleIdMap<V> =
    HashMap<(NodeIndex, ComponentTupleKey), V, BuildHasherDefault<NodeIndexTupleIdHasher>>;
pub type TupleIdMap<V> = HashMap<ComponentTupleKey, V, BuildHasherDefault<TupleIdHasher>>;

#[derive(Debug)]
pub struct World {
    pub assets_server: AssetsServer,
    pub time: Time,

    entities: EntityStorage,
    entity_datas: SparseSet<Entity, EntityData>,
    table_graph: TableGraph,
    // components_id_to_table: HashMap<Vec<ComponentId>, NodeIndex>,
    tuple_to_table: TupleIdMap<NodeIndex>,
    insert_edges: NodeIndexTupleIdMap<InsertTarget>,
    remove_edges: NodeIndexTupleIdMap<NodeIndex>,

    // 后面这里的Scene概念应该会改为Chunk概念
    // 由Game里的各个功能组件/System来做区块划分并通过类似TagComponent进行控制
    // pub scene_stroge: SceneStroge,
    // scenes: SparseSet<SceneId, Scene>,
    id: u64,
}

impl World {
    pub fn new() -> Self {
        static ID: Mutex<u64> = Mutex::new(1);
        let id = {
            let mut id = ID.lock().unwrap();
            let next = id.add(1);
            *id = next;
            next
        };

        let assets_server = AssetsServer::new();
        let time = Time::new();

        let entities = EntityStorage::new(consts::WORLD_ENTITIES_CAPACITY);
        let entity_datas = SparseSet::new(consts::WORLD_ENTITIES_CAPACITY);
        let table_graph = TableGraph::new(consts::WORLD_TABLE_GRAPH_CAPACITY);

        let tuple_to_table = TupleIdMap::default();
        let insert_edges = NodeIndexTupleIdMap::default();
        let remove_edges = NodeIndexTupleIdMap::default();

        Self {
            assets_server,
            time,
            entities,
            entity_datas,
            tuple_to_table,
            insert_edges,
            remove_edges,
            table_graph,
            id,
        }
    }

    /// 给一个实体添加组件
    pub fn insert<T: ComponentTuple>(&mut self, entity: &Entity, components: T) {
        self.flush();

        let data = self.entity_datas.get_value(entity);
        let src_table = data.table_index;
        let row_index = data.row_index;
        self.insert_inner(entity, components, src_table, row_index);
    }

    fn insert_inner<T: ComponentTuple>(
        &mut self,
        entity: &Entity,
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
            components.put(|ptr, info| {
                source_table.put_dynamic(ptr, &info, row_index);
            });
            return;
        }

        let (source_table, target_table) = self
            .table_graph
            .index2(src_table, target_ref.get_node_index());

        let target_row_index = target_table.allocate_entity(entity);
        let entity_data = self.entity_datas.get_value_mut(entity);
        entity_data.table_index = target_ref.get_node_index();
        entity_data.row_index = target_row_index;

        // 写入components到新表
        components.put(|ptr, info| {
            target_table.put_dynamic(ptr, &info, target_row_index);
        });

        // 转移需要转移的老表中的数据
        for info in target_ref.get_need_moves() {
            let src = source_table.get_dynamice(info, row_index).unwrap();
            target_table.put_dynamic(src.as_ptr(), info, target_row_index);
        }

        // remove 老表的entity，并更新这里entity data 的 row_index
        // 会被覆盖更新的，我们在前面drop了老数据。其他数据相当于是移动到新表(转移所有权而非被销毁)，因此不需要drop
        if let Some(moved) = source_table.remove_entity(entity, false) {
            let moved_data = self.entity_datas.get_value_mut(&moved);
            moved_data.row_index = row_index;
        }
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
    pub fn spawn<T: ComponentTuple>(&mut self, components: T) -> Entity {
        self.flush();

        let entity = self.entities.alloc();
        self.spawn_inner(&entity, components);
        entity
    }

    fn spawn_inner<T: ComponentTuple>(&mut self, entity: &Entity, components: T) {
        let tables = &mut self.table_graph;
        let table_index = match components.key() {
            Some(key) => *self.tuple_to_table.entry(key).or_insert(
                components.with_ids(|types| tables.get(types, || components.type_infos())),
            ),
            None => components.with_ids(|types| tables.get(types, || components.type_infos())),
        };

        let table = &mut tables[table_index];
        let row_index = table.allocate_entity(entity);
        components.put(|ptr, info| {
            table.put_dynamic(ptr, &info, row_index);
        });
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
    pub fn spawn_at<T: ComponentTuple>(&mut self, entity: Entity, components: T) {
        self.flush();

        match self.entities.alloc_at(&entity) {
            sparse_set::AllocAt::New(range) => {
                for idx in range {
                    self.entity_datas.insert(
                        &Entity::new(idx as u32, 0, EntityFlag::Dead),
                        EntityData::default(),
                    );
                }
                self.entity_datas.insert(&entity, EntityData::default());
            }
            sparse_set::AllocAt::BeUsed => {},
            sparse_set::AllocAt::Using => {
                let entity_data = self.entity_datas.get_value(&entity);
                if let Some(moved) = self.table_graph[entity_data.table_index].remove_entity(&entity, true) {
                    self.entity_datas[&moved].row_index = entity_data.row_index
                }
            },
        }
        self.spawn_inner(&entity, components);
    }

    /// 从实体身上移除'T'组件
    pub fn remove<T: StaticTypedComponentTuple + 'static>(
        &mut self,
        entity: &Entity,
    ) -> Result<T, ComponentError> {
        self.flush();

        let entity_data = self.entity_datas.get_value_mut(&entity);
        let old_row_index = entity_data.row_index;
        let source_table = &self.table_graph[entity_data.table_index];

        let tuple = T::get(|info| source_table.get_dynamice(&info, old_row_index))?;

        let target = Self::remove_target::<T>(
            &mut self.table_graph,
            &mut self.remove_edges,
            entity_data.table_index,
        );

        if entity_data.table_index != target {
            let (source_table, target_table) =
                self.table_graph.index2(entity_data.table_index, target);
            let target_row_index = target_table.allocate_entity(&entity);
            entity_data.table_index = target;
            entity_data.row_index = target_row_index;
            if let Some(moved) = unsafe {
                source_table.move_to(old_row_index, |src, info| {
                    if let Some(dst) = target_table.get_dynamice(info, target_row_index) {
                        ptr::copy_nonoverlapping(src, dst.as_ptr(), info.layout().size());
                    }
                })
            } {
                self.entity_datas[&moved].row_index = old_row_index
            }
        }

        Ok(tuple)
    }

    fn remove_target<T: StaticTypedComponentTuple + 'static>(
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
    pub fn remove_one<T: Component>(&mut self, entity: &Entity) -> Result<T, ComponentError> {
        self.remove::<(T,)>(entity).map(|(x,)| x)
    }

    /// 从实体身上移除 'S' 组件, 然后添加 'T' 组件
    pub fn exchange<S: StaticTypedComponentTuple + 'static, T: ComponentTuple>(
        &mut self,
        entity: &Entity,
        components: T,
    ) -> Result<S, ComponentError> {
        self.flush();

        let entity_data = self.entity_datas.get_value(entity);

        let source_table = &self.table_graph[entity_data.table_index];

        let tuple = S::get(|info| source_table.get_dynamice(&info, entity_data.row_index))?;

        let intermediate = Self::remove_target::<S>(
            &mut self.table_graph,
            &mut self.remove_edges,
            entity_data.table_index,
        );
        self.insert_inner(entity, components, intermediate, entity_data.row_index);

        Ok(tuple)
    }

    /// 从实体身上移除单个 'S' 组件，然后添加单个 'T' 组件
    pub fn exchange_one<S: Component, T: Component>(
        &mut self,
        entity: &Entity,
        component: T,
    ) -> Result<S, ComponentError> {
        self.exchange::<(S,), (T,)>(entity, (component,))
            .map(|(x,)| x)
    }
}
