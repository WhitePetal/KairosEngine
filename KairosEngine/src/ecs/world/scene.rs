use std::{collections::HashMap, fmt::Debug};

use crate::ecs::{
    component_tuple::{ComponentQueryMutTuple, ComponentQueryTuple, ComponentsTuple},
    entity::Entity,
    sparse_set::EntityStorage,
    table::Table,
    table_graph::TableGraph, world::{SceneId, World},
};


#[derive(Debug)]
pub struct Scene {

}


impl Scene {
    pub fn new<F: FnMut(&SceneId, &mut World) -> () + 'static>(world: &'a mut World, update: F) -> Self {
        Self { world, update_inner: Box::new(update) }
    }

    pub fn create_entity<T: ComponentsTuple>(
        &mut self,
        component_register: &mut ComponentRegister,
        components_tuple: T,
    ) -> Entity {
        let component_id_metas = T::to_ids(component_register);

        let table_node_index = {
            if let Some(table_node_index) = self.components_id_to_table.get(&component_id_metas.0) {
                *table_node_index
            } else {
                // TODO: Build node edges?
                self.table_graph.graph.add_node(Table::new(
                    self.default_table_capacity,
                    component_id_metas.0,
                    component_id_metas.1,
                ))
            }
        };

        let table = &mut self.table_graph.graph[table_node_index];
        components_tuple.create_entity(component_register, &mut self.entities, table)
    }

    pub fn query<'a, Q: ComponentQueryTuple + 'a, F: FnMut(Q::Item<'a>)>(
        &'a self,
        register: &mut ComponentRegister,
        f: F,
    ) {
        Q::foreach(register, &self.table_graph, f);
    }

    pub fn query_mut<'a, Q: ComponentQueryMutTuple + 'a, F: FnMut(Q::Item<'a>)>(
        &'a mut self,
        register: &mut ComponentRegister,
        f: F,
    ) {
        Q::foreach(register, &mut self.table_graph, f);
    }

    pub fn add_components_for_entity<T: ComponentsTuple>(entity: Entity, component_tuple: T) {
        todo!()
    }
}
