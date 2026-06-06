use std::{any::TypeId, collections::HashMap};

use crate::ecs::{component::ComponentId, world::scene::Scene};


pub mod scene;

pub struct World {
    scenes: Vec<Scene>,
    component_type_to_id: HashMap<TypeId, ComponentId>,
}