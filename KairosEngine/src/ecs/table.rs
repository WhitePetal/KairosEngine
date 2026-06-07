use std::{
    alloc::{Layout, alloc, handle_alloc_error},
    ptr::{self, NonNull},
};

use crate::ecs::{
    compoent_register::ComponentTypeMeta, component::ComponentId, entity::Entity,
    sparse_set::SparseSet,
};

#[derive(Debug, Clone, Copy)]
pub struct ComponentInfo {
    pub head_offset: usize,
    pub layout: Layout,
}

///
/// 每个类型为一列，每列存储该类型的所有Components
pub struct ComponentTable {
    colums: NonNull<u8>,
    infos: Vec<ComponentInfo>,
    len: usize,
    capacity: usize,
    layout: Layout,
}
impl ComponentTable {
    pub fn new(component_metas: Vec<ComponentTypeMeta>, capacity: usize) -> Self {
        let mut infos = Vec::with_capacity(component_metas.len());
        let mut table_layout = Layout::from_size_align(0, 1).unwrap();
        for meta in component_metas {
            let layout = meta.layout;
            let colum_layout =
                Layout::from_size_align(layout.size() * capacity, layout.align()).unwrap();
            let (new_layout, head_offset) = table_layout.extend(colum_layout).unwrap();

            infos.push(ComponentInfo {
                head_offset,
                layout: layout,
            });

            table_layout = new_layout;
        }

        table_layout = table_layout.pad_to_align();

        let colums = if table_layout.size() == 0 {
            NonNull::dangling()
        } else {
            let ptr = unsafe { alloc(table_layout) };
            NonNull::new(ptr).unwrap_or_else(|| handle_alloc_error(table_layout))
        };

        Self {
            colums,
            infos,
            len: 0,
            capacity,
            layout: table_layout,
        }
    }

    pub fn push_value<T>(&mut self, colum_index: usize, value: T) {
        debug_assert!(colum_index < self.len);

        let info = &self.infos[colum_index];
        debug_assert_eq!(info.layout, Layout::new::<T>());

        unsafe {
            let dst = self
                .colums
                .as_ptr()
                .add(info.head_offset + self.len * info.layout.size())
                .cast::<T>();
            ptr::write(dst, value);
        }
    }

    pub fn write_value<T>(&mut self, row_index: usize, colum_index: usize, value: T) {
        let info = self.infos[colum_index];
        unsafe {
            let dst = self
                .colums
                .as_ptr()
                .add(info.head_offset + row_index * info.layout.size())
                .cast::<T>();
            ptr::write::<T>(dst, value);
        }
    }

    pub fn creat_row_slot(&mut self) -> usize {
        if self.len >= self.capacity {
            self.resize();
        }

        let len = self.len;
        self.len = len + 1;
        len
    }

    pub fn pop_row_slot(&mut self) {
        debug_assert!(self.len > 0);

        self.len = self.len - 1;
    }

    pub fn get_components<T>(&self, colum_index: usize) -> &[T] {
        debug_assert!(colum_index < self.len);
        let info = &self.infos[colum_index];
        debug_assert_eq!(info.layout, Layout::new::<T>());

        unsafe {
            let ptr = self.colums.as_ptr().add(info.head_offset).cast::<T>();
            std::slice::from_raw_parts(ptr, self.len)
        }
    }

    pub fn get_components_mut<T>(&mut self, colum_index: usize) -> &mut [T] {
        debug_assert!(colum_index < self.len);
        let info = &self.infos[colum_index];
        debug_assert_eq!(info.layout, Layout::new::<T>());

        unsafe {
            let ptr = self.colums.as_ptr().add(info.head_offset).cast::<T>();
            std::slice::from_raw_parts_mut(ptr, self.len)
        }
    }

    pub fn remove_row(&mut self, row_index: usize) {
        for info in &self.infos {
            unsafe {
                let remove_ptr = self
                    .colums
                    .as_ptr()
                    .add(info.head_offset + row_index * info.layout.size());
                let ending_ptr = self
                    .colums
                    .as_ptr()
                    .add(info.head_offset + (self.len - 1) * info.layout.size());
                ptr::copy::<u8>(ending_ptr, remove_ptr, info.layout.size());
            }
        }
        self.len = self.len - 1;
    }

    fn resize(&mut self) {
        let mut infos = Vec::clone(&self.infos);
        let mut table_layout = Layout::from_size_align(0, 1).unwrap();

        let capacity = self.capacity << 1;

        for info in &mut infos {
            let colum_layout =
                Layout::from_size_align(info.layout.size() * capacity, info.layout.align())
                    .unwrap();
            let (new_layout, head_offset) = table_layout.extend(colum_layout).unwrap();

            *info = ComponentInfo {
                head_offset,
                layout: info.layout,
            };

            table_layout = new_layout
        }

        table_layout = table_layout.pad_to_align();

        let new_ptr = unsafe { alloc(table_layout) };
        let Some(colums) = NonNull::new(new_ptr) else {
            handle_alloc_error(table_layout);
        };

        for (old_info, new_info) in self.infos.iter().zip(infos.iter()) {
            let bytes = old_info.layout.size() * self.len;

            unsafe {
                let src = self.colums.as_ptr().add(old_info.head_offset);
                let dst = colums.as_ptr().add(new_info.head_offset);

                ptr::copy_nonoverlapping(src, dst, bytes);
            }
        }

        self.infos = infos;
        self.colums = colums;
        self.capacity = capacity;
        self.layout = table_layout;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EntityInfo {
    pub row_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ComponentTypeInfo {
    pub colum_index: usize,
}

pub struct Table {
    types: SparseSet<ComponentId, ComponentTypeInfo>,
    entities: Vec<Entity>,
    entitiy_infos: SparseSet<Entity, EntityInfo>,
    components_table: ComponentTable,
}

impl Table {
    pub fn new(
        row_capacity: usize,
        component_ids: Vec<ComponentId>,
        component_metas: Vec<ComponentTypeMeta>,
    ) -> Self {
        let entities = Vec::with_capacity(row_capacity);
        let entitiy_infos = SparseSet::new(row_capacity);
        let colum_capacity = component_ids.len();
        let mut types = SparseSet::new(colum_capacity);
        component_ids
            .iter()
            .enumerate()
            .for_each(|(index, component_id)| {
                types.insert(component_id, ComponentTypeInfo { colum_index: index });
            });
        let components_table = ComponentTable::new(component_metas, colum_capacity);

        Self {
            types,
            entities,
            entitiy_infos,
            components_table,
        }
    }

    pub fn write_value<T>(&mut self, entity: Entity, component_id: ComponentId, component: T) {
        debug_assert!(self.entitiy_infos.has(entity.clone()));
        debug_assert!(self.types.has(component_id.clone()));
        let entity_info = self.entitiy_infos[entity];
        let component_type_info = self.types[component_id];
        self.components_table.write_value(
            entity_info.row_index,
            component_type_info.colum_index,
            component,
        );
    }

    pub fn push_row(&mut self, entity: Entity) {
        debug_assert!(!self.entitiy_infos.has(entity.clone()));
        self.entitiy_infos.insert(
            &entity,
            EntityInfo {
                row_index: self.entitiy_infos.get_head(),
            },
        );
        self.entities.push(entity);
        self.components_table.creat_row_slot();
    }

    pub fn remove_row(&mut self, entity: Entity) {
        debug_assert!(self.entities.len() > 0);
        debug_assert!(self.entitiy_infos.has(entity.clone()));
        let end_entity = self.entities.pop().unwrap();
        let entity_info = self.entitiy_infos.remove(entity);

        self.components_table.remove_row(entity_info.row_index);
        self.entities[entity_info.row_index] = end_entity;
    }
}
