use std::{
    alloc::{self, Layout, alloc, dealloc, handle_alloc_error}, any::TypeId, ptr::{self, NonNull}
};

use crate::{ecs::{
    compoent_register::ComponentTypeMeta, component::Component, consts, entity::Entity, id::Id, sparse_set::SparseSet
}, types::OrderedTypeIdMap};

///
/// 每个类型为一列，每列存储该类型的所有Components
#[derive(Debug)]
pub struct ComponentColum {
    data: NonNull<u8>,
}
impl ComponentTable {

    pub fn push_value<T>(&mut self, colum_index: usize, value: T) {
        debug_assert!(colum_index < self.infos.len());

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

    fn creat_row<F>(&mut self, writer: Vec<F>)
    where
        F: FnOnce(*mut u8),
    {
        if self.len >= self.capacity {
            self.resize();
        }

        writer.into_iter().enumerate().for_each(|(colum, writer)| {
            let info = &self.infos[colum];
            unsafe {
                let ptr = self
                    .colums
                    .as_ptr()
                    .add(info.head_offset + self.len * info.layout.size());
                writer(ptr);
            }
        });

        self.len = self.len + 1;
    }

    pub fn get_colum<T>(&self, colum_index: usize) -> &[T] {
        debug_assert!(colum_index < self.infos.len());
        let info = &self.infos[colum_index];
        debug_assert_eq!(info.layout, Layout::new::<T>());

        unsafe {
            let ptr = self.colums.as_ptr().add(info.head_offset).cast::<T>();
            std::slice::from_raw_parts(ptr, self.len)
        }
    }

    pub fn get_colum_mut<T>(&mut self, colum_index: usize) -> &mut [T] {
        debug_assert!(colum_index < self.infos.len());
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

                ((info.drop_fn)(remove_ptr));

                if remove_ptr != ending_ptr {
                    ptr::copy_nonoverlapping::<u8>(ending_ptr, remove_ptr, info.layout.size());
                }
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
                drop_fn: info.drop_fn,
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

        unsafe {
            dealloc(self.colums.as_ptr(), self.layout);
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

fn drop_component<T>(ptr: *mut u8) {
    unsafe { ptr::drop_in_place(ptr.cast::<T>()) }
}

#[derive(Debug, Clone, Copy)]
pub struct ComponentTypeInfo {
    type_id: TypeId,
    layout: Layout,
    drop_fn: unsafe fn(*mut u8),
    #[cfg(debug_assertions)]
    type_name: &'static str
}
impl ComponentTypeInfo {
    pub fn of<T: Component>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            layout: Layout::new::<T>(),
            drop_fn: drop_component::<T>,
            #[cfg(debug_assertions)]
            type_name: core::any::type_name::<T>(),
        }
    }

    pub fn id(&self) -> TypeId {
        self.type_id
    }
}
impl PartialEq for ComponentTypeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}
impl Eq for ComponentTypeInfo {
    
}
impl PartialOrd for ComponentTypeInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ComponentTypeInfo {
    // 主排序为：按照字节对齐降序排序(保证数据在Chunk中按照对齐降序分布，这样可以减少因为数据对齐产生的空位内存)
    // 次排序(字节对齐相同时)：按照TypeId升序排序，这样可以保证每次排序得到的结果是固定的(因为TypeId唯一)
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.layout.align().cmp(&other.layout.align()).reverse().then_with(|| self.type_id.cmp(&other.type_id))
    }
}

#[derive(Debug)]
pub struct Table {
    types: Box<[ComponentTypeInfo]>,
    type_ids: Box<[TypeId]>,
    colum_indexs: OrderedTypeIdMap<usize>,
    // colums 采用懒分配即 row_count, colums_data_row_capacity = 0
    // 那么我们就无法用 entities.capatiy() 作为 colums_data 的 row_capacity
    // 所以我们单独管理 row_count, entities 和 colums 都采用懒分配并保持同步扩容
    // 即 entities.len = row_capacity
    len: usize,
    entities: Box<[Entity]>,
    entitiy_infos: SparseSet<Entity, EntityInfo>,
    // 每列单独分配内存存储，这样可以更好的处理每个类型的借用关系 和 访问时的数据地址计算，且不会引入明显的性能差异
    colums: Box<[ComponentColum]>,
}

impl Table {
    pub fn new(
        types: Box<[ComponentTypeInfo]>,
    ) -> Self {
        let entities = Box::new([]);
        let entitiy_infos = SparseSet::new(consts::TABLE_ROW_CAPACITY);
        let colum_capacity = types.len();
        let type_ids = types.iter().map(|ty| ty.type_id).collect();
        let colum_indexs = OrderedTypeIdMap::new(types.iter().enumerate().map(|(index, ty)| (ty.type_id, index)));
        // NonNull 现在只是创建空的占位值。但Rust要求即使是占位置，也应该基于能够地址对齐来创建
        // 用 max_align 能保证所有 Type 都可以基于此对齐，因此先用这个快速创建NoneNull占位置
        // 在实际分配内存时，再按照各类型具体的对齐进行创建
        let max_align = types.first().map_or(1, |ty| ty.layout.align());
        let colums = (0..colum_capacity).map(|_| ComponentColum {
            // 将 max_align 转为 地址值(指针)，该地址一定基于 max_align 对齐 (addr(max_align) % max_align == 0)
            data: NonNull::new(max_align as *mut u8).unwrap()
        }).collect();

        Self {
            types,
            type_ids,
            colum_indexs,
            len: 0,
            entities,
            entitiy_infos,
            colums,
        }
    }

    pub fn row_count(&self) -> usize {
        self.entities.len()
    }

    pub fn colum_count(&self) -> usize {
        self.types.len()
    }

    pub fn clear(&mut self) {
        for (ty, colum) in self.types.iter().zip(&self.colums) {
            for row in 0..self.entities.len() {
                unsafe {
                    let removed = colum.data.as_ptr().add(row * ty.layout.size());
                    (ty.drop_fn)(removed);
                }
            }
        }
        self.len = 0;
    }

    pub fn allocate_entity(&mut self, entity: &Entity) {
        if self.len == self.entities.len() {
            self.grow(consts::TABLE_ROW_CAPACITY);
        }

        self.entitiy_infos.insert(entity, EntityInfo { 
            row_index: self.len
        });
        self.entities[self.len] = entity.clone();
        self.len = self.len + 1;
    }

    fn grow(&mut self, min_incement: usize) {
        self.grow_exact(self.row_capacity().max(min_incement));
    }
    fn grow_exact(&mut self, increment: usize) {
        let old_count = self.len;
        let old_cap = self.entities.len();
        let new_cap = old_cap + increment;

        let mut new_entities = vec![Entity::get_invalide_id(); new_cap].into_boxed_slice();
        new_entities[0..old_count].copy_from_slice(&self.entities[0..old_count]);
        self.entities = new_entities;

        let new_colums = self
            .types
            .iter()
            .zip(&self.colums)
            .map(|(info, old_colum)| {
                let storage = 
                {
                    if info.layout.size() == 0 {
                        NonNull::new(info.layout.align() as *mut u8).unwrap()
                    } else {
                        let layout = Layout::from_size_align(info.layout.size() * new_cap, info.layout.align()).unwrap();
                        unsafe {
                            let mem = alloc(layout);
                            let mem = NonNull::new(mem).unwrap_or_else(|| alloc::handle_alloc_error(layout));
                            ptr::copy_nonoverlapping(old_colum.data.as_ptr(), mem.as_ptr(), info.layout.size() * old_count);
                            mem
                        }
                    }
                };
                ComponentColum {
                    data: storage
                }
            }).collect::<Box<[_]>>();

        if old_cap > 0 {
            for (info, colum) in self.types.iter().zip(&self.colums) {
                if info.layout.size() == 0 {
                    continue;;
                }

                unsafe {
                    dealloc(colum.data.as_ptr(), Layout::from_size_align(info.layout.size() * old_cap, info.layout.align()).unwrap());
                }
            }
        }
    }

    pub fn remove_entity(&mut self, entity: &Entity, drop: bool) {
        let end = self.len - 1;
        let row_index = self.entitiy_infos.remove(entity.clone()).row_index;
        let swap = row_index != end;
        for (info, colum) in self.types.iter().zip(&self.colums) {
            unsafe {
                let layout_size = info.layout.size();
                let removed = colum.data.as_ptr().add(row_index * layout_size);
                if drop {
                    (info.drop_fn)(removed);
                }
                if swap {
                    let moved = colum.data.as_ptr().add(end * layout_size);
                    ptr::copy_nonoverlapping(moved, removed, layout_size);
                }
            }
        }
        self.len = end;
        if swap {
            self.entities[row_index] = self.entities[end].clone();
        }
    }

    fn row_capacity(&self) -> usize {
        self.entities.len()
    }

    pub fn push_row<F>(&mut self, entity: &Entity, component_writes: Vec<F>)
    where
        F: FnOnce(*mut u8),
    {
        debug_assert!(!self.entitiy_infos.has(entity));
        self.entitiy_infos.insert(
            entity,
            EntityInfo {
                row_index: self.entities.len(),
            },
        );
        self.entities.push(entity.clone());
        self.components_table.creat_row(component_writes);
    }

    pub fn remove_row(&mut self, entity: Entity) {
        debug_assert!(self.entities.len() > 0);
        debug_assert!(self.entitiy_infos.has(&entity));
        let end_entity = self.entities.pop().unwrap();
        let entity_info = self.entitiy_infos.remove(entity);

        self.components_table.remove_row(entity_info.row_index);
        self.entities[entity_info.row_index] = end_entity;
    }


    pub fn has_component(&self, component_type: &TypeId) -> bool {
        self.type_ids.contains(component_type)
    }

    pub fn component_colum_index(&self, component_type: &TypeId) -> usize {
        debug_assert!(
            self.type_ids.contains(component_type),
            "No component id in the table! component_id: {:?}",
            component_type
        );

        self.types.get_value(component_id).colum_index
    }

    pub fn contains_all_components(&self, component_ids: &[&ComponentId]) -> bool {
        component_ids
            .iter()
            .all(|component_id| self.has_component(*component_id))
    }

    pub fn component_slice<T: Component>(&self, component_id: &ComponentId) -> &[T] {
        let colum_index = self.component_colum_index(component_id);
        self.components_table.get_colum::<T>(colum_index)
    }

    pub fn component_slice_mut<T: Component>(&mut self, component_id: &ComponentId) -> &mut [T] {
        let colum_index = self.component_colum_index(component_id);
        self.components_table.get_colum_mut::<T>(colum_index)
    }
}


impl Drop for Table {
    fn drop(&mut self) {
        let row_count = self.len;
        self.clear();
        // 没有添加过Entity/Component
        if row_count <= 0 {
            return;
        }
        for (info, colum) in self.types.iter().zip(&self.colums) {
            if info.layout.size() != 0 {
                unsafe {
                    dealloc(
                        colum.data.as_ptr(), 
                        // colum构造时使用的checked方法，因此这里可以unchecked
                        Layout::from_size_align_unchecked(info.layout.size() * row_count, info.layout.align())
                    );
                }
            }
        }
    }
}