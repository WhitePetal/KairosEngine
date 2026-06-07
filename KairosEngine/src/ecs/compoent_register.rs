use std::{alloc::Layout, any::TypeId, collections::HashMap};

use crate::ecs::{
    component::{Component, ComponentFlag, ComponentId},
    id::Id,
};

#[derive(Debug, Clone, Copy)]
pub struct ComponentTypeMeta {
    pub layout: Layout,
}
impl ComponentTypeMeta {
    pub fn new(layout: Layout) -> Self {
        Self { layout }
    }
}

pub struct ComponentRegister {
    component_type_to_id: HashMap<TypeId, ComponentId>,
    component_metas: Vec<ComponentTypeMeta>,
}

impl ComponentRegister {
    #[inline(always)]
    pub fn new(capacity: usize) -> Self {
        Self {
            component_type_to_id: HashMap::with_capacity(capacity),
            component_metas: Vec::with_capacity(capacity),
        }
    }

    pub fn get<T: Component>(&mut self) -> (ComponentId, ComponentTypeMeta) {
        let type_id = TypeId::of::<T>();
        if let Some(id) = self.component_type_to_id.get(&type_id) {
            return (id.clone(), self.component_metas[id.get_idx() as usize]);
        } else {
            let len = self.component_type_to_id.len();
            let id = ComponentId::new(len as u32, 0, ComponentFlag::Default);
            let meta = ComponentTypeMeta::new(Layout::new::<T>());
            self.component_metas.push(meta.clone());
            self.component_type_to_id.insert(type_id, id.clone());
            return (id, meta);
        }
    }
}
