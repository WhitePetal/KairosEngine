use std::{any::TypeId, hash::Hash};

use crate::ecs::{
    entity::Entity,
    sparse_set::EntityStorage,
    table::{ComponentTypeInfo, Table},
};

type ComponentWriter = Box<dyn FnOnce(*mut u8)>;

#[derive(Debug, PartialEq, Eq)]
pub struct ComponentTupleKey(TypeId);
impl Hash for ComponentTupleKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let data =
        // SAFETY: The `offset` stays in-bounds, it just moves the pointer to the 2nd half of the `TypeId`.
        // Only the first ptr-sized chunk ever has provenance, so that second half is always
        // fine to read at integer type.
            unsafe { std::mem::transmute_copy(&self.0) };
        state.write_u64(data);
    }
}
impl From<TypeId> for ComponentTupleKey {
    fn from(value: TypeId) -> Self {
        Self(value)
    }
}

pub trait ComponentTuple {
    fn key(&self) -> Option<ComponentTupleKey>;

    fn type_infos(&self) -> Box<[ComponentTypeInfo]>;

    fn put<F: FnMut(*mut u8, ComponentTypeInfo)>(self, f: F);

    fn with_ids<T, F: FnOnce(&[TypeId]) -> T>(&self, f: F) -> T;

    fn create_entity(
        self,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity;
}
