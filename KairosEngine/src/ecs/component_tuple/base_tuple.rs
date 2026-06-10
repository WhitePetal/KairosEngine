use std::{any::TypeId, hash::Hash, ptr};

use crate::ecs::{
    component::{Component},
    entity::Entity,
    id::Id,
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

pub trait ComponentsTuple {
    fn key(&self) -> Option<ComponentTupleKey>;

    fn type_infos(&self) -> Box<[ComponentTypeInfo]>;

    fn put<F: FnMut(*mut u8, ComponentTypeInfo)>(self, f: F);

    fn create_entity(
        self,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity;
}

impl<A: Component> ComponentsTuple for A {
    fn type_infos(&self) -> Box<[ComponentTypeInfo]> {
        let a = ComponentTypeInfo::of::<A>();
        Box::new([a])
    }

    fn create_entity(
        self,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity {
        let a = self;
        let entity = entity_stroge.next();
        components_table.push_row(
            &entity,
            vec![|dest: *mut u8| unsafe {
                ptr::write::<A>(dest.cast::<A>(), a);
            }],
        );

        entity
    }
}
impl<A: Component, B: Component> ComponentsTuple for (A, B) {
    fn type_infos(&self) -> Box<[ComponentTypeInfo]> {
        let a = ComponentTypeInfo::of::<A>();
        let b = ComponentTypeInfo::of::<B>();
        let mut components = [a, b];
        components.sort();
        Box::new(components)
    }

    fn create_entity(
        self,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity {
        let (a, b) = self;
        let (a_id, _) = register.get::<A>();
        let (b_id, _) = register.get::<B>();

        let mut writers: [(ComponentId, ComponentWriter); 2] = [
            (
                a_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<A>(dest.cast::<A>(), a);
                }),
            ),
            (
                b_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<B>(dest.cast::<B>(), b);
                }),
            ),
        ];

        writers.sort_by_key(|(id, _)| id.get_idx());
        let writers = writers
            .into_iter()
            .map(|(_, writer)| writer)
            .collect::<Vec<_>>();

        let entity = entity_stroge.next();
        components_table.push_row(&entity, writers);

        entity
    }
}
impl<A: Component, B: Component, C: Component> ComponentsTuple for (A, B, C) {
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>) {
        let a = register.get::<A>();
        let b = register.get::<B>();
        let c = register.get::<C>();
        let mut components = [a, b, c];
        components.sort_by_key(|(id, _)| id.get_idx());
        components.into_iter().unzip()
    }

    fn create_entity(
        self,
        register: &mut ComponentRegister,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity {
        let (a, b, c) = self;
        let (a_id, _) = register.get::<A>();
        let (b_id, _) = register.get::<B>();
        let (c_id, _) = register.get::<C>();

        let mut writers: [(ComponentId, ComponentWriter); 3] = [
            (
                a_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<A>(dest.cast::<A>(), a);
                }),
            ),
            (
                b_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<B>(dest.cast::<B>(), b);
                }),
            ),
            (
                c_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<C>(dest.cast::<C>(), c);
                }),
            ),
        ];

        writers.sort_by_key(|(id, _)| id.get_idx());
        let writers = writers
            .into_iter()
            .map(|(_, writer)| writer)
            .collect::<Vec<_>>();

        let entity = entity_stroge.next();
        components_table.push_row(&entity, writers);

        entity
    }
}
impl<A: Component, B: Component, C: Component, D: Component> ComponentsTuple for (A, B, C, D) {
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>) {
        let a = register.get::<A>();
        let b = register.get::<B>();
        let c = register.get::<C>();
        let d = register.get::<D>();
        let mut components = [a, b, c, d];
        components.sort_by_key(|(id, _)| id.get_idx());
        components.into_iter().unzip()
    }

    fn create_entity(
        self,
        register: &mut ComponentRegister,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity {
        let (a, b, c, d) = self;
        let (a_id, _) = register.get::<A>();
        let (b_id, _) = register.get::<B>();
        let (c_id, _) = register.get::<C>();
        let (d_id, _) = register.get::<D>();

        let mut writers: [(ComponentId, ComponentWriter); 4] = [
            (
                a_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<A>(dest.cast::<A>(), a);
                }),
            ),
            (
                b_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<B>(dest.cast::<B>(), b);
                }),
            ),
            (
                c_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<C>(dest.cast::<C>(), c);
                }),
            ),
            (
                d_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<D>(dest.cast::<D>(), d);
                }),
            ),
        ];

        writers.sort_by_key(|(id, _)| id.get_idx());
        let writers = writers
            .into_iter()
            .map(|(_, writer)| writer)
            .collect::<Vec<_>>();

        let entity = entity_stroge.next();
        components_table.push_row(&entity, writers);

        entity
    }
}
impl<A: Component, B: Component, C: Component, D: Component, E: Component> ComponentsTuple
    for (A, B, C, D, E)
{
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>) {
        let a = register.get::<A>();
        let b = register.get::<B>();
        let c = register.get::<C>();
        let d = register.get::<D>();
        let e = register.get::<E>();
        let mut components = [a, b, c, d, e];
        components.sort_by_key(|(id, _)| id.get_idx());
        components.into_iter().unzip()
    }

    fn create_entity(
        self,
        register: &mut ComponentRegister,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity {
        let (a, b, c, d, e) = self;
        let (a_id, _) = register.get::<A>();
        let (b_id, _) = register.get::<B>();
        let (c_id, _) = register.get::<C>();
        let (d_id, _) = register.get::<D>();
        let (e_id, _) = register.get::<E>();

        let mut writers: [(ComponentId, ComponentWriter); 5] = [
            (
                a_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<A>(dest.cast::<A>(), a);
                }),
            ),
            (
                b_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<B>(dest.cast::<B>(), b);
                }),
            ),
            (
                c_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<C>(dest.cast::<C>(), c);
                }),
            ),
            (
                d_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<D>(dest.cast::<D>(), d);
                }),
            ),
            (
                e_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<E>(dest.cast::<E>(), e);
                }),
            ),
        ];

        writers.sort_by_key(|(id, _)| id.get_idx());
        let writers = writers
            .into_iter()
            .map(|(_, writer)| writer)
            .collect::<Vec<_>>();

        let entity = entity_stroge.next();
        components_table.push_row(&entity, writers);

        entity
    }
}
impl<A: Component, B: Component, C: Component, D: Component, E: Component, F: Component>
    ComponentsTuple for (A, B, C, D, E, F)
{
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>) {
        let a = register.get::<A>();
        let b = register.get::<B>();
        let c = register.get::<C>();
        let d = register.get::<D>();
        let e = register.get::<E>();
        let f = register.get::<F>();
        let mut components = [a, b, c, d, e, f];
        components.sort_by_key(|(id, _)| id.get_idx());
        components.into_iter().unzip()
    }

    fn create_entity(
        self,
        register: &mut ComponentRegister,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity {
        let (a, b, c, d, e, f) = self;
        let (a_id, _) = register.get::<A>();
        let (b_id, _) = register.get::<B>();
        let (c_id, _) = register.get::<C>();
        let (d_id, _) = register.get::<D>();
        let (e_id, _) = register.get::<E>();
        let (f_id, _) = register.get::<F>();

        let mut writers: [(ComponentId, ComponentWriter); 6] = [
            (
                a_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<A>(dest.cast::<A>(), a);
                }),
            ),
            (
                b_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<B>(dest.cast::<B>(), b);
                }),
            ),
            (
                c_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<C>(dest.cast::<C>(), c);
                }),
            ),
            (
                d_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<D>(dest.cast::<D>(), d);
                }),
            ),
            (
                e_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<E>(dest.cast::<E>(), e);
                }),
            ),
            (
                f_id,
                Box::new(move |dest: *mut u8| unsafe {
                    ptr::write::<F>(dest.cast::<F>(), f);
                }),
            ),
        ];

        writers.sort_by_key(|(id, _)| id.get_idx());
        let writers = writers
            .into_iter()
            .map(|(_, writer)| writer)
            .collect::<Vec<_>>();

        let entity = entity_stroge.next();
        components_table.push_row(&entity, writers);

        entity
    }
}
