use std::ptr;

use crate::ecs::{
    compoent_register::{ComponentRegister, ComponentTypeMeta},
    component::{Component, ComponentId},
    entity::Entity,
    sparse_set::EntityStorage,
    table::Table,
};

pub trait ComponentsTuple {
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>);

    fn create_entity(
        self,
        register: &mut ComponentRegister,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity;
}

impl<A: Component> ComponentsTuple for A {
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>) {
        let a = register.get::<A>();
        (vec![a.0], vec![a.1])
    }

    fn create_entity(
        self,
        _register: &mut ComponentRegister,
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
// impl<A: Component, B: Component> ComponentsTuple for (A, B) {
//     fn to_ids(register: &mut ComponentRegister) -> Vec<(ComponentId, ComponentTypeMeta)> {
//         vec![register.get_id_meta::<A>(), register.get_id_meta::<B>()]
//     }
// }
// impl<A: Component, B: Component, C: Component> ComponentsTuple for (A, B, C) {
//     fn to_ids(register: &mut ComponentRegister) -> Vec<(ComponentId, ComponentTypeMeta)> {
//         vec![
//             register.get_id_meta::<A>(),
//             register.get_id_meta::<B>(),
//             register.get_id_meta::<C>(),
//         ]
//     }
// }
// impl<A: Component, B: Component, C: Component, D: Component> ComponentsTuple for (A, B, C, D) {
//     fn to_ids(register: &mut ComponentRegister) -> Vec<(ComponentId, ComponentTypeMeta)> {
//         vec![
//             register.get_id_meta::<A>(),
//             register.get_id_meta::<B>(),
//             register.get_id_meta::<C>(),
//             register.get_id_meta::<D>(),
//         ]
//     }
// }
// impl<A: Component, B: Component, C: Component, D: Component, E: Component> ComponentsTuple
//     for (A, B, C, D, E)
// {
//     fn to_ids(register: &mut ComponentRegister) -> Vec<(ComponentId, ComponentTypeMeta)> {
//         vec![
//             register.get_id_meta::<A>(),
//             register.get_id_meta::<B>(),
//             register.get_id_meta::<C>(),
//             register.get_id_meta::<D>(),
//             register.get_id_meta::<E>(),
//         ]
//     }
// }
// impl<A: Component, B: Component, C: Component, D: Component, E: Component, F: Component>
//     ComponentsTuple for (A, B, C, D, E, F)
// {
//     fn to_ids(register: &mut ComponentRegister) -> Vec<(ComponentId, ComponentTypeMeta)> {
//         vec![
//             register.get_id_meta::<A>(),
//             register.get_id_meta::<B>(),
//             register.get_id_meta::<C>(),
//             register.get_id_meta::<D>(),
//             register.get_id_meta::<E>(),
//             register.get_id_meta::<F>(),
//         ]
//     }
// }
