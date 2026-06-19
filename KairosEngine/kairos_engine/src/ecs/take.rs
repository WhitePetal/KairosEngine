use std::any::Any;

use crate::ecs::{component_tuple::DynamicComponentTuple, entity::Entity, sparse_set::{SparseSet, SparseStroge}, table::Table, world::EntityData};



pub struct TakeEntity<'a> {
    entities: &'a mut SparseStroge<Entity>,
    entity_datas: &'a mut SparseSet<Entity, EntityData>,
    table: &'a mut Table,
    entity: Entity,
    drop: bool,
}

impl<'a> TakeEntity<'a> {
    pub unsafe fn new(entities: &'a mut SparseStroge<Entity>, entity_datas: &'a mut SparseSet<Entity, EntityData>, entity: Entity, table: &'a mut Table) -> Self {
        Self { 
            entities, 
            entity_datas, 
            table, 
            entity, 
            drop: true
        }
    }
}

impl Drop for TakeEntity<'_> {
    fn drop(&mut self) {
        if let Some(moved) = self.entities.free(self.entity.clone()).ok() {
            if let Some(entity_data) = self.entity_datas.remove(self.entity.clone(), moved) {
                    if let Some(moved) = self.table.remove_entity(&self.entity, self.drop) {
                        self.entity_datas[&moved].set_row_index(entity_data.row_index());
                    }
            }
        }
    }
}

unsafe impl DynamicComponentTuple for TakeEntity<'_> {
    fn type_infos(&self) -> Box<[super::table::ComponentTypeInfo]> {
        self.table.types().iter().copied().collect::<Box<[_]>>()
    }

    unsafe fn put<F: FnMut(*mut u8, super::table::ComponentTypeInfo)>(mut self, mut f: F) {
        self.drop = false;
        let loc = self.entity_datas.get(&self.entity).unwrap();
        for ty in self.table.types() {
            let ptr = self
                .table
                .get_dynamice(ty, loc.row_index())
                .unwrap();
            f(ptr.as_ptr(), *ty)
        }
    }

    fn with_ids<T, F: FnOnce(&[std::any::TypeId]) -> T>(&self, f: F) -> T {
        f(self.table.type_ids())
    }
}