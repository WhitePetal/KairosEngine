use crate::{
    asset_loader::assets::AssetsServer,
    ecs::{
        component_tuple::{ComponentQueryMutTuple, ComponentQueryTuple, ComponentsTuple}, consts, entity::{Entity, EntityFlag}, id::Id, sparse_set::{EntityStorage, SparseSet, SparseStroge}, table_graph::TableGraph, world::scene::Scene
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
pub struct World {
    pub assets_server: AssetsServer,
    pub time: Time,
    
    entities: EntityStorage,
    table_graph: TableGraph,
    // components_id_to_table: HashMap<Vec<ComponentId>, NodeIndex>,

    // 后面这里的Scene概念应该会改为Chunk概念
    // 由Game里的各个功能组件/System来做区块划分并通过类似TagComponent进行控制
    pub scene_stroge: SceneStroge,
    scenes: SparseSet<SceneId, Scene>,
}

impl World {
    pub fn new() -> Self {
        let assets_server = AssetsServer::new();
        let time = Time::new();
        
        let entities = EntityStorage::new(consts::WORLD_ENTITIES_CAPACITY);
        let table_graph = TableGraph::new(consts::WORLD_TABLE_GRAPH_CAPACITY);
        
        let scene_stroge = SceneStroge::new(consts::WORLD_SCENE_CAPACITY);
        let scenes = SparseSet::new(consts::WORLD_SCENE_CAPACITY);
        Self {
            assets_server,
            time,
            entities,
            table_graph,
            scene_stroge,
            scenes,
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
        scene_id: &SceneId,
        components_tuple: T,
    ) -> Entity {
        let scene = self.scenes.get_value_mut(scene_id);
        scene.create_entity(&mut self.component_register, components_tuple)
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
