use std::{any::TypeId, collections::{HashMap, hash_map::Entry}, hash::{BuildHasher, BuildHasherDefault, Hasher}, ops::Add, sync::Mutex};

use petgraph::graph::NodeIndex;

use crate::{
    asset_loader::assets::AssetsServer,
    ecs::{
        component_tuple::{ComponentQueryMutTuple, ComponentQueryTuple, ComponentTupleKey, ComponentsTuple}, consts, entity::{Entity, EntityFlag}, id::Id, sparse_set::{EntityStorage, SparseSet, SparseStroge}, table::ComponentTypeInfo, table_graph::{InsertTarget, TableGraph}, world::scene::Scene
    },
    timer::Time,
};

pub mod scene;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SceneId(Entity);

impl Id for SceneId {
    type FlagType = EntityFlag;

    #[inline(always)]
    fn new(idx: u32, version: u32, flags: Self::FlagType) -> Self {
        Self(Entity::new(idx, version, flags))
    }

    #[inline(always)]
    fn get_idx(&self) -> u32 {
        self.0.get_idx()
    }

    #[inline(always)]
    fn get_version(&self) -> u32 {
        self.0.get_version()
    }

    #[inline(always)]
    fn get_flags(&self) -> Self::FlagType {
        self.0.get_flags()
    }

    #[inline(always)]
    fn from_other(idx: u32, other: &Self) -> Self {
        Self(Entity::from_other(idx, &other.0))
    }

    #[inline(always)]
    fn replace_idx(&mut self, idx: u32) {
        self.0.replace_idx(idx);
    }

    #[inline(always)]
    fn create_idx_variant(&self, idx: u32) -> Self {
        Self(self.0.create_idx_variant(idx))
    }

    #[inline(always)]
    fn replace_flags(&mut self, flags: Self::FlagType) {
        self.0.replace_flags(flags);
    }

    #[inline(always)]
    fn get_next_version(self, flags: Self::FlagType) -> Self {
        Self(self.0.get_next_version(flags))
    }
}

type SceneStroge = SparseStroge<SceneId>;

#[derive(Debug)]
pub struct EntityData {
    table_index: NodeIndex,
    row_index: usize
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

    fn write(&mut self, bytes: &[u8]) {
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


pub type NodeIndexTupleIdMap<V> = HashMap<(NodeIndex, ComponentTupleKey), V, BuildHasherDefault<NodeIndexTupleIdHasher>>;
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
    pub scene_stroge: SceneStroge,
    scenes: SparseSet<SceneId, Scene>,

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

        let scene_stroge = SceneStroge::new(consts::WORLD_SCENE_CAPACITY);
        let scenes = SparseSet::new(consts::WORLD_SCENE_CAPACITY);

        Self {
            assets_server,
            time,
            entities,
            entity_datas,
            tuple_to_table,
            insert_edges,
            remove_edges,
            table_graph,
            scene_stroge,
            scenes,
            id,
        }
    }

    #[inline(always)]
    pub fn push_scene(&mut self, scene: Scene) -> SceneId {
        let scene_id = self.scene_stroge.next();
        self.scenes.insert(&scene_id, scene);
        scene_id
    }

    #[inline(always)]
    pub fn get_scene(&self, scene_id: &SceneId) -> &Scene {
        self.scenes.get_value(scene_id)
    }

    #[inline(always)]
    pub fn get_scene_mut(&mut self, scene_id: &SceneId) -> &mut Scene {
        self.scenes.get_value_mut(scene_id)
    }

    pub fn create_entity<T: ComponentsTuple>(
        &mut self,
        components_tuple: T,
    ) -> Entity {
        let entity = self.entities.next();
        todo!()
        // scene.create_entity(&mut self.component_register, components_tuple)
    }

    pub fn insert<T: ComponentsTuple>(&mut self, entity: &Entity, components: T) {
        self.flush();

        let data = self.entity_datas.get_value(entity);
        let src_table = data.table_index;
        let row_index = data.row_index;
        let target;
        let target_ref = match components.key() {
            Some(key) => {
                match self.insert_edges.entry((src_table, key)) {
                    Entry::Occupied(occupied_entry) => {
                        occupied_entry.into_mut()
                    },
                    Entry::Vacant(vacant_entry) => {
                        let target = self.table_graph.get_insert_target(src_table, &components);
                        vacant_entry.insert(target)
                    },
                }
            },
            None => {
                target = self.table_graph.get_insert_target(src_table, &components);
                &target
            },
        };

        let source_table = &mut self.table_graph[src_table];

        // drop老表的行
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

        let (source_table, target_table) = self.table_graph.index2(src_table, target_ref.get_node_index());

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
        // 最开始我们以及drop了，因此这里 drop = false
        if let Some(moved) = source_table.remove_entity(entity, false) {
            let moved_data = self.entity_datas.get_value_mut(&moved);
            moved_data.row_index = row_index;
        }
    }

    ///
    /// 把预留但未挂在任何组件的entity放入根表中
    /// 在添加或删除组件或实体时，必须先调用该方法
    /// 例如 spawn, despawn, insert and remove
    pub fn flush(&mut self) {
        let root_node = NodeIndex::new(0);
        let root_table = &mut self.table_graph[root_node];
        self.entities.flush(|entity| {
            self.entity_datas.insert(entity, EntityData { 
                table_index: root_node,
                row_index: root_table.allocate_entity(entity)
            });
        });
    }

    pub fn spawn<T: ComponentsTuple>(&mut self, components: T) -> Entity {
        self.flush();
        
        let entity = self.entities.next();
        self.spawn_inner(&entity, components);
        entity
    }

    fn spawn_inner<T: ComponentsTuple>(&mut self, entity: &Entity, components: T) {
        let tables = &mut self.table_graph;
        let table_index = match components.key() {
            Some(key) => {
                *self.tuple_to_table.entry(key).or_insert(
                    components.with_ids(|types| tables.get(types, || components.type_infos()))
                )
            },
            None => components.with_ids(|types| tables.get(types, || components.type_infos())),
        };

        let table = &mut tables[table_index];
        let row_index = table.allocate_entity(entity);
        components.put(|ptr, info| {
            table.put_dynamic(ptr, &info, row_index);
        });
        self.entity_datas.insert(entity, EntityData { 
            table_index, 
            row_index 
        });
    }


    pub fn query<'a, Q: ComponentQueryTuple + 'a, F: FnMut(Q::Item<'a>)>(
        &'a mut self,
        scene_id: &SceneId,
        f: F,
    ) {
        let scene = self.scenes.get_value(scene_id);
        scene.query::<Q, F>(&mut self.component_register, f);
    }

    pub fn query_mut<'a, Q: ComponentQueryMutTuple + 'a, F: FnMut(Q::Item<'a>)>(
        &'a mut self,
        scene_id: &SceneId,
        f: F,
    ) {
        let scene = self.scenes.get_value_mut(scene_id);
        scene.query_mut::<Q, F>(&mut self.component_register, f);
    }
}
