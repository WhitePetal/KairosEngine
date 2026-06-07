use std::{alloc::Layout, any::TypeId, collections::HashMap, ptr};

use crate::ecs::{
    component::{Component, ComponentFlag, ComponentId},
    id::Id,
};

fn drop_component<T>(ptr: *mut u8) {
    unsafe { ptr::drop_in_place(ptr.cast::<T>()) }
}

#[derive(Debug, Clone, Copy)]
pub struct ComponentTypeMeta {
    pub layout: Layout,
    pub drop_fn: unsafe fn(*mut u8),
}
impl ComponentTypeMeta {
    pub fn new<T: Component>() -> Self {
        Self {
            layout: Layout::new::<T>(),
            drop_fn: drop_component::<T>,
        }
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
            let meta = ComponentTypeMeta::new::<T>();
            self.component_metas.push(meta.clone());
            self.component_type_to_id.insert(type_id, id.clone());
            return (id, meta);
        }
    }
}
