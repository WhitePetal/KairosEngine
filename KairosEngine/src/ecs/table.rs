use std::{
    alloc::{alloc, dealloc, handle_alloc_error, realloc, Layout},
    ptr::{self, NonNull},
};

use crate::ecs::{entity::Entity};


#[derive(Debug, Clone, Copy)]
pub struct ComponentInfo {
    pub offset: usize,
    pub layout: Layout,
}

///
/// 每个类型为一列，每列存储该类型的所有Components
pub struct ComponentTable {
    colums: NonNull<u8>,
    infos: Vec<ComponentInfo>,
    len: usize,
    capacity: usize,
    layout: Layout
}
impl ComponentTable {
    pub fn new(component_layouts: Vec<Layout>, capacity: usize) -> Self {
        let mut infos = Vec::with_capacity(component_layouts.len());
        let mut table_layout = Layout::from_size_align(0, 1).unwrap();
        for layout in component_layouts {
            let colum_layout = Layout::from_size_align(layout.size() * capacity, layout.align()).unwrap();
            let (new_layout, offset ) = table_layout.extend(colum_layout).unwrap();

            infos.push(ComponentInfo {
                offset,
                layout: layout
            });

            table_layout = new_layout;
        }

        table_layout = table_layout.pad_to_align();

        let colums = if table_layout.size() == 0 {
            NonNull::dangling()
        } else {
            let ptr = unsafe {
                alloc(table_layout)
            };
            NonNull::new(ptr).unwrap_or_else(|| handle_alloc_error(table_layout))
        };

        Self { colums, infos, len: 0, capacity, layout: table_layout }
    }

    pub fn write_value<T>(&mut self, colum_index: usize, value: T) {
        debug_assert!(colum_index < self.len);
        
        let info = &self.infos[colum_index];
        debug_assert_eq!(info.layout, Layout::new::<T>());

        unsafe {
            let dst = self.colums.as_ptr().add(info.offset + self.len * info.layout.size()).cast::<T>();
            ptr::write(dst, value);
        }
    }

    pub fn creat_slot<T>(&mut self) -> usize {
        if self.len >= self.capacity {
            self.resize();
        }
        
        let len = self.len;
        self.len = len + 1;
        len
    }

    pub fn pop_slot(&mut self) {
        debug_assert!(self.len > 0);

        self.len = self.len - 1;
    }

    pub fn get_components<T>(&self, colum_index: usize) -> &[T] {
        debug_assert!(colum_index < self.len);
        let info = &self.infos[colum_index];
        debug_assert_eq!(info.layout, Layout::new::<T>());

        unsafe {
            let ptr = self.colums.as_ptr().add(info.offset).cast::<T>();
            std::slice::from_raw_parts(ptr, self.len)
        }
    }

    pub fn get_components_mut<T>(&mut self, colum_index: usize) -> &mut[T] {
        debug_assert!(colum_index < self.len);
        let info = &self.infos[colum_index];
        debug_assert_eq!(info.layout, Layout::new::<T>());

        unsafe {
            let ptr = self.colums.as_ptr().add(info.offset).cast::<T>();
            std::slice::from_raw_parts_mut(ptr, self.len)
        }
    }

    fn resize(&mut self) {
        let mut infos = Vec::clone(&self.infos);
        let mut table_layout = Layout::from_size_align(0, 1).unwrap();

        let capacity = self.capacity << 1;

        for info in &mut infos {
            let colum_layout = Layout::from_size_align(info.layout.size() * capacity, info.layout.align()).unwrap();
            let (new_layout, offset) = table_layout.extend(colum_layout).unwrap();

            *info = ComponentInfo { offset, layout: info.layout };

            table_layout = new_layout
        }

        table_layout = table_layout.pad_to_align();

        let new_ptr = unsafe {
            alloc(table_layout)
        };
        let Some(colums) = NonNull::new(new_ptr) else {
            handle_alloc_error(table_layout);
        };

        for (old_info, new_info) in self.infos.iter().zip(infos.iter()) {
            let bytes = old_info.layout.size() * self.len;

            unsafe {
                let src = self.colums.as_ptr().add(old_info.offset);
                let dst = colums.as_ptr().add(new_info.offset);

                ptr::copy_nonoverlapping(src, dst, bytes);
            }
        };

        self.infos = infos;
        self.colums = colums;
        self.capacity = capacity;
        self.layout = table_layout;
    }
}

pub struct Table {
    entities: Vec<Entity>,
    components_table: ComponentTable,
    /// 通过 entity id 找到 该 entity 身上组件在 ColumeComponents 中的 行下标
    entity_id_to_component_index: Vec<usize>,
}
