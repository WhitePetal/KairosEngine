use crate::ecs::{
    entity::{Entity, EntityFlag},
    id::Id,
    sparse_set::SparseSet,
};

pub type EntityStorage = SparseSet<Entity, Entity>;

impl EntityStorage {
    pub fn next(&mut self) -> Entity {
        let head = self.get_head();
        if head < self.dense_values.len() {
            let entity = self.dense_values[head].clone();
            let entity = entity.get_next_version(EntityFlag::Default);
            self.insert(&entity, entity.clone());

            entity
        } else {
            let entity = Entity::new(head as u32, 0, EntityFlag::Default);
            self.insert(&entity, entity.clone());

            entity
        }
    }
}
