use crate::ecs::{
    entity::{Entity, EntityFlag},
    id::Id,
    sparse_set::SparseSet,
};

pub type EntityStorage = SparseSet<Entity, Entity>;

impl EntityStorage {
    pub fn next(&mut self) -> Entity {
        let head = self.get_head();
        if head < self.dense.len() {
            let index_entity = Entity::new(head as u32, 0, EntityFlag::Default);
            let sparse_pos = Self::get_sparse_pos(&index_entity);
            self.sparse[sparse_pos.page].0[sparse_pos.slot] = index_entity;
            let entity = self.dense[head];
            let entity = entity.get_next_version(EntityFlag::Default);
            self.dense[head] = entity;
            self.head = self.head + 1;

            entity
        } else {
            let entity = Entity::new(head as u32, 0, EntityFlag::Default);
            self.push_back(entity, entity);

            entity
        }
    }
}
