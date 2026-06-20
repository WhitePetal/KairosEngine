use std::{
    alloc::{self, Layout, alloc, dealloc},
    any::{TypeId, type_name},
    fmt::Debug,
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
};

use crate::{
    ecs::{
        borrow::AtomicBorrow, component::Component, consts, entity::Entity, id::Id,
        sparse_set::SparseSet,
    },
    types::OrderedTypeIdMap,
};

///
/// 每个类型为一列，每列存储该类型的所有Components
#[derive(Debug)]
pub struct ComponentColum {
    data: NonNull<u8>,
    state: AtomicBorrow,
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
    _type_name: &'static str,
}
impl ComponentTypeInfo {
    pub fn of<T: Component>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            layout: Layout::new::<T>(),
            drop_fn: drop_component::<T>,
            #[cfg(debug_assertions)]
            _type_name: core::any::type_name::<T>(),
        }
    }

    #[inline(always)]
    pub fn id(&self) -> TypeId {
        self.type_id
    }

    #[inline(always)]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn drop(&self, ptr: *mut u8) {
        unsafe {
            (self.drop_fn)(ptr);
        }
    }
}
impl PartialEq for ComponentTypeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}
impl Eq for ComponentTypeInfo {}
impl PartialOrd for ComponentTypeInfo {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ComponentTypeInfo {
    // 主排序为：按照字节对齐降序排序(保证数据在Chunk中按照对齐降序分布，这样可以减少因为数据对齐产生的空位内存)
    // 次排序(字节对齐相同时)：按照TypeId升序排序，这样可以保证每次排序得到的结果是固定的(因为TypeId唯一)
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.layout
            .align()
            .cmp(&other.layout.align())
            .reverse()
            .then_with(|| self.type_id.cmp(&other.type_id))
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
    pub fn new(types: Box<[ComponentTypeInfo]>) -> Self {
        let entities = Box::new([]);
        let entitiy_infos = SparseSet::new(consts::TABLE_ROW_CAPACITY);
        let colum_capacity = types.len();
        let type_ids = types.iter().map(|ty| ty.type_id).collect();
        let colum_indexs = OrderedTypeIdMap::new(
            types
                .iter()
                .enumerate()
                .map(|(index, ty)| (ty.type_id, index)),
        );
        // NonNull 现在只是创建空的占位值。但Rust要求即使是占位置，也应该基于能够地址对齐来创建
        // 用 max_align 能保证所有 Type 都可以基于此对齐，因此先用这个快速创建NoneNull占位置
        // 在实际分配内存时，再按照各类型具体的对齐进行创建
        let max_align = types.first().map_or(1, |ty| ty.layout.align());
        let colums = (0..colum_capacity)
            .map(|_| ComponentColum {
                // 将 max_align 转为 地址值(指针)，该地址一定基于 max_align 对齐 (addr(max_align) % max_align == 0)
                data: NonNull::new(max_align as *mut u8).unwrap(),
                state: AtomicBorrow::new(),
            })
            .collect();

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
        self.len
    }

    pub fn colum_count(&self) -> usize {
        self.types.len()
    }

    pub fn clear(&mut self) {
        for (ty, colum) in self.types.iter().zip(&self.colums) {
            for row in 0..self.len {
                unsafe {
                    let removed = colum.data.as_ptr().add(row * ty.layout.size());
                    (ty.drop_fn)(removed);
                }
            }
        }
        self.len = 0;
    }

    /// 创建一个实体行
    /// Return: row_index
    pub fn allocate_entity(&mut self, entity: &Entity) -> usize {
        if self.len == self.entities.len() {
            self.grow(consts::TABLE_ROW_CAPACITY);
        }

        let row_index = self.len;
        self.entitiy_infos.insert(entity, EntityInfo { row_index });
        self.entities[row_index] = entity.clone();
        self.len = row_index + 1;
        row_index
    }

    fn grow(&mut self, min_incement: usize) {
        self.grow_exact(self.row_capacity().max(min_incement));
    }
    fn grow_exact(&mut self, increment: usize) {
        let old_count = self.len;
        let old_cap = self.entities.len();
        let new_cap = old_cap + increment;

        let mut new_entities = vec![Entity::get_invalide_id(); new_cap].into_boxed_slice();
        new_entities[0..old_count].clone_from_slice(&self.entities[0..old_count]);
        self.entities = new_entities;

        let new_colums = self
            .types
            .iter()
            .zip(&self.colums)
            .map(|(info, old_colum)| {
                let storage = {
                    if info.layout.size() == 0 {
                        NonNull::new(info.layout.align() as *mut u8).unwrap()
                    } else {
                        let layout = Layout::from_size_align(
                            info.layout.size() * new_cap,
                            info.layout.align(),
                        )
                        .unwrap();
                        unsafe {
                            let mem = alloc(layout);
                            let mem = NonNull::new(mem)
                                .unwrap_or_else(|| alloc::handle_alloc_error(layout));
                            ptr::copy_nonoverlapping(
                                old_colum.data.as_ptr(),
                                mem.as_ptr(),
                                info.layout.size() * old_count,
                            );
                            mem
                        }
                    }
                };
                ComponentColum {
                    data: storage,
                    state: AtomicBorrow::new(),
                }
            })
            .collect::<Box<[_]>>();

        if old_cap > 0 {
            for (info, colum) in self.types.iter().zip(&self.colums) {
                if info.layout.size() == 0 {
                    continue;
                }

                unsafe {
                    dealloc(
                        colum.data.as_ptr(),
                        Layout::from_size_align(info.layout.size() * old_cap, info.layout.align())
                            .unwrap(),
                    );
                }
            }
        }

        self.colums = new_colums;
    }

    pub fn remove_entity(&mut self, entity: &Entity, drop: bool) -> Option<Entity> {
        if self.len == 0 {
            return None;
        }
        let end = self.len - 1;
        let moved = self.entities[end].clone();
        match self.entitiy_infos.remove(entity.clone(), moved.clone()) {
            Some(entity_info) => {
                let row_index = entity_info.row_index;
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
                    self.entities[row_index] = moved.clone();
                    Some(moved)
                } else {
                    None
                }
            }
            None => None,
        }
    }

    fn row_capacity(&self) -> usize {
        self.entities.len()
    }

    pub fn types(&self) -> &[ComponentTypeInfo] {
        &self.types
    }

    pub fn type_ids(&self) -> &Box<[TypeId]> {
        &self.type_ids
    }

    pub fn get_dynamice(&self, info: &ComponentTypeInfo, row_index: usize) -> Option<NonNull<u8>> {
        debug_assert!(row_index <= self.len);
        unsafe {
            Some(NonNull::new_unchecked(
                self.colums
                    .get_unchecked(*self.colum_indexs.get(&info.type_id)?)
                    .data
                    .as_ptr()
                    .add(info.layout.size() * row_index)
                    .cast::<u8>(),
            ))
        }
    }

    pub fn put_dynamic(&mut self, component: *mut u8, info: &ComponentTypeInfo, row_index: usize) {
        unsafe {
            let ptr = self
                .get_dynamice(info, row_index)
                .unwrap()
                .as_ptr()
                .cast::<u8>();
            ptr::copy_nonoverlapping(component, ptr, info.layout.size());
        }
    }

    /// 把 row_index 行数据通过 f 写入目标
    pub fn move_to<F: FnMut(*mut u8, &ComponentTypeInfo) -> ()>(
        &mut self,
        row_index: usize,
        mut f: F,
    ) -> Option<Entity> {
        let last = self.len - 1;
        for (info, colum) in self.types.iter().zip(&self.colums) {
            unsafe {
                let moved_out = colum.data.as_ptr().add(row_index * info.layout.size());
                (f)(moved_out, info);
                if row_index != last {
                    let moved = colum.data.as_ptr().add(last * info.layout.size());
                    ptr::copy_nonoverlapping(moved, moved_out, info.layout.size());
                }
            }
        }
        self.len = self.len - 1;
        if row_index != last {
            self.entities[row_index] = self.entities[last].clone();
            Some(self.entities[last].clone())
        } else {
            None
        }
    }

    pub fn has_component_type_id(&self, component_type: TypeId) -> bool {
        self.type_ids.contains(&component_type)
    }

    pub fn has_component_type<T: Component>(&self) -> bool {
        self.has_component_type_id(TypeId::of::<T>())
    }

    pub fn reserve(&mut self, additional: usize) {
        let free = self.capacity() - self.len;
        if additional > free {
            let increment = additional - free;
            self.grow(increment.max(consts::TABLE_ROW_CAPACITY));
        }
    }

    pub fn capacity(&self) -> usize {
        self.entities.len()
    }

    /// 把 'other' Table 合并入当前Table (other 的数据 push 到当前table后面)
    pub fn merge(&mut self, mut other: Table) {
        self.reserve(other.row_count());
        for ((info, dst), src) in self.types.iter().zip(&self.colums).zip(&other.colums) {
            unsafe {
                dst.data
                    .as_ptr()
                    .add(info.layout.size() * self.len)
                    .copy_from_nonoverlapping(src.data.as_ptr(), other.len * info.layout.size());
            }
        }
        self.len += other.len;
        other.len = 0;
    }

    pub fn entity(&self, row_index: usize) -> Entity {
        self.entities[row_index].clone()
    }

    pub fn set_entity_id(&mut self, row_index: usize, id: u32) {
        self.entities[row_index] = self.entities[row_index].create_idx_variant(id);
    }

    pub fn entities(&self) -> NonNull<Entity> {
        unsafe { NonNull::new_unchecked(self.entities.as_ptr() as *mut _) }
    }

    pub fn borrow<T: Component>(&self, state: usize) {
        debug_assert_eq!(self.types[state].type_id, TypeId::of::<T>());

        if !self.colums[state].state.borrow() {
            panic!("{} already borrowed uniquely", type_name::<T>());
        }
    }
    pub unsafe fn borrow_raw(&self, state: usize) {
        if !self.colums[state].state.borrow() {
            panic!("state index {} already borrowed uniquely", state);
        }
    }
    pub fn borrow_mut<T: Component>(&self, state: usize) {
        assert_eq!(self.types[state].type_id, TypeId::of::<T>());

        if !self.colums[state].state.borrow_mut() {
            panic!("{} already borrowed", type_name::<T>());
        }
    }
    pub fn release<T: Component>(&self, state: usize) {
        assert_eq!(self.types[state].type_id, TypeId::of::<T>());
        self.colums[state].state.release();
    }
    pub fn release_mut<T: Component>(&self, state: usize) {
        assert_eq!(self.types[state].type_id, TypeId::of::<T>());
        self.colums[state].state.release_mut();
    }
    pub unsafe fn release_raw(&self, state: usize) {
        self.colums[state].state.release();
    }
    pub unsafe fn release_raw_mut(&self, state: usize) {
        self.colums[state].state.release_mut();
    }

    pub fn get_state<T: Component>(&self) -> Option<usize> {
        self.colum_indexs.get(&TypeId::of::<T>()).copied()
    }

    pub unsafe fn get_base<T: Component>(&self, state: usize) -> NonNull<T> {
        debug_assert_eq!(self.types[state].type_id, TypeId::of::<T>());

        unsafe {
            NonNull::new_unchecked(self.colums.get_unchecked(state).data.as_ptr().cast::<T>())
        }
    }

    pub fn is_emptry(&self) -> bool {
        self.len == 0
    }
}

impl Drop for Table {
    fn drop(&mut self) {
        let row_capacity = self.entities.len();
        self.clear();
        // 没有添加过Entity/Component
        if row_capacity <= 0 {
            return;
        }
        for (info, colum) in self.types.iter().zip(&self.colums) {
            if info.layout.size() != 0 {
                unsafe {
                    dealloc(
                        colum.data.as_ptr(),
                        // colum构造时使用的checked方法，因此这里可以unchecked
                        // 内存是按照 capacity (entities.len()) 分配的，必须使用相同大小释放
                        Layout::from_size_align_unchecked(
                            info.layout.size() * row_capacity,
                            info.layout.align(),
                        ),
                    );
                }
            }
        }
    }
}

pub struct TableColum<'a, T: Component> {
    table: &'a Table,
    colum: &'a [T],
}

impl<'a, T: Component> TableColum<'a, T> {
    pub fn new(table: &'a Table) -> Option<Self> {
        let state = table.get_state::<T>()?;
        let ptr = unsafe { table.get_base::<T>(state) };
        let colum = unsafe { core::slice::from_raw_parts(ptr.as_ptr(), table.row_count()) };
        table.borrow::<T>(state);
        Some(Self { table, colum })
    }
}

impl<T: Component> Deref for TableColum<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.colum
    }
}

impl<T: Component> Drop for TableColum<'_, T> {
    fn drop(&mut self) {
        let state = self.table.get_state::<T>().unwrap();
        self.table.release::<T>(state);
    }
}

impl<T: Component> Clone for TableColum<'_, T> {
    fn clone(&self) -> Self {
        let state = self.table.get_state::<T>().unwrap();
        self.table.borrow::<T>(state);
        Self {
            table: self.table,
            colum: self.colum,
        }
    }
}

impl<T: Component + Debug> Debug for TableColum<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.colum.fmt(f)
    }
}

pub struct TableColumMut<'a, T: Component> {
    table: &'a Table,
    colum: &'a mut [T],
}

impl<'a, T: Component> TableColumMut<'a, T> {
    pub fn new(table: &'a Table) -> Option<Self> {
        let state = table.get_state::<T>()?;
        let ptr = unsafe { table.get_base::<T>(state) };
        let colum = unsafe { core::slice::from_raw_parts_mut(ptr.as_ptr(), table.row_count()) };
        table.borrow_mut::<T>(state);
        Some(Self { table, colum })
    }
}

impl<T: Component> Deref for TableColumMut<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.colum
    }
}

impl<T: Component> DerefMut for TableColumMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.colum
    }
}

impl<T: Component> Drop for TableColumMut<'_, T> {
    fn drop(&mut self) {
        let state = self.table.get_state::<T>().unwrap();
        self.table.release_mut::<T>(state);
    }
}

impl<T: Component + Debug> Debug for TableColumMut<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.colum.fmt(f)
    }
}
