use std::{any::TypeId, collections::HashMap};

use crate::ecs::{
    compoent_register::ComponentRegister, component::ComponentId, component_tuple::ComponentsTuple,
    consts, entity::Entity, world::scene::Scene,
};

pub mod scene;

pub struct World {
    scenes: Vec<Scene>,
    component_register: ComponentRegister,
}

impl World {
    pub fn new() -> Self {
        let component_register = ComponentRegister::new(consts::COMPONENT_TYPE_CAPACITY);
        Self {
            scenes: Vec::with_capacity(consts::WORLD_SCENE_CAPACITY),
            component_register,
        }
    }
}
