use std::{
    any::TypeId,
    fmt::{Debug, Display},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::ecs::{
    component::{Component, MissingComponent},
    component_tuple::{Fetch, Query, QueryOne},
    entity::Entity,
    sparse_set::SparseSet,
    table::{Table, TableColum, TableColumMut},
    world::EntityData,
};

struct ComponentBorrow<'a> {
    table: &'a Table,
    state: usize,
}

impl<'a> ComponentBorrow<'a> {
    unsafe fn for_component<T: Component>(
        table: &'a Table,
        row_index: usize,
    ) -> Result<(NonNull<T>, Self), MissingComponent> {
        let state = table
            .get_state::<T>()
            .ok_or_else(MissingComponent::new::<T>)?;

        let target =
            unsafe { NonNull::new_unchecked(table.get_base::<T>(state).as_ptr().add(row_index)) };

        table.borrow::<T>(state);

        Ok((target, Self { table, state }))
    }
}

impl Clone for ComponentBorrow<'_> {
    fn clone(&self) -> Self {
        unsafe {
            self.table.borrow_raw(self.state);
        }
        Self {
            table: self.table,
            state: self.state,
        }
    }
}

impl Drop for ComponentBorrow<'_> {
    fn drop(&mut self) {
        unsafe {
            self.table.release_raw(self.state);
        }
    }
}

struct ComponentBorrowMut<'a> {
    table: &'a Table,
    state: usize,
}

impl<'a> ComponentBorrowMut<'a> {
    unsafe fn for_component<T: Component>(
        table: &'a Table,
        row_index: usize,
    ) -> Result<(NonNull<T>, Self), MissingComponent> {
        let state = table
            .get_state::<T>()
            .ok_or_else(MissingComponent::new::<T>)?;

        let target =
            unsafe { NonNull::new_unchecked(table.get_base::<T>(state).as_ptr().add(row_index)) };

        table.borrow_mut::<T>(state);

        Ok((target, Self { table, state }))
    }
}

impl Drop for ComponentBorrowMut<'_> {
    fn drop(&mut self) {
        unsafe {
            self.table.release_raw_mut(self.state);
        }
    }
}

pub struct Ref<'a, T: ?Sized> {
    borrow: ComponentBorrow<'a>,
    target: NonNull<T>,
    _phantom: PhantomData<&'a T>,
}

unsafe impl<T: ?Sized + Sync> Send for Ref<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for Ref<'_, T> {}

impl<T: ?Sized> Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.target.as_ref() }
    }
}

impl<'a, T: ?Sized> Ref<'a, T> {
    pub fn map<U: ?Sized, F>(orig: Ref<'a, T>, f: F) -> Ref<'a, U>
    where
        F: FnOnce(&T) -> &U,
    {
        let target = NonNull::from(f(&orig));
        Ref {
            borrow: orig.borrow,
            target,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: Component> Ref<'a, T> {
    pub unsafe fn new(table: &'a Table, row_index: usize) -> Result<Self, MissingComponent> {
        let (target, borrow) = unsafe { ComponentBorrow::for_component::<T>(table, row_index)? };
        Ok(Self {
            borrow,
            target,
            _phantom: PhantomData,
        })
    }
}

impl<T: ?Sized + Debug> Debug for Ref<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.deref(), f)
    }
}

impl<T: ?Sized + Display> Display for Ref<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.deref(), f)
    }
}

impl<T: ?Sized> Clone for Ref<'_, T> {
    fn clone(&self) -> Self {
        Self {
            borrow: self.borrow.clone(),
            target: self.target,
            _phantom: self._phantom,
        }
    }
}

pub struct RefMut<'a, T: ?Sized> {
    borrow: ComponentBorrowMut<'a>,
    target: NonNull<T>,
    _phantom: PhantomData<&'a mut T>,
}

unsafe impl<T: ?Sized + Send> Send for RefMut<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for RefMut<'_, T> {}

impl<T: ?Sized> Deref for RefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.target.as_ref() }
    }
}

impl<T: ?Sized> DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.target.as_mut() }
    }
}

impl<'a, T: ?Sized> RefMut<'a, T> {
    pub fn map<U: ?Sized, F>(mut orig: RefMut<'a, T>, f: F) -> RefMut<'a, U>
    where
        F: FnOnce(&mut T) -> &mut U,
    {
        let target = NonNull::from(f(&mut orig));
        RefMut {
            borrow: orig.borrow,
            target,
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: Component> RefMut<'a, T> {
    pub unsafe fn new(table: &'a Table, row_index: usize) -> Result<Self, MissingComponent> {
        let (target, borrow) = unsafe { ComponentBorrowMut::for_component::<T>(table, row_index)? };
        Ok(Self {
            borrow,
            target,
            _phantom: PhantomData,
        })
    }
}

impl<T: ?Sized + Debug> Debug for RefMut<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.deref(), f)
    }
}

impl<T: ?Sized + Display> Display for RefMut<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.deref(), f)
    }
}

pub trait ComponentRef<'a> {
    /// 指向类型组件引用的智能指针
    type Ref;

    /// 指向[`Table`]中列引用的智能指针
    type Colum;

    /// 被 `Ref` 引用的组件类型
    type Component: Component;

    /// 从 `entity` 身上获取组件
    fn get_component(entity: EntityRef<'a>) -> Option<Self::Ref>;

    /// 从原始指针构造
    ///
    /// # Safety
    ///
    /// 在生命周期`a`内对`raw`解引用必须是有效的
    unsafe fn from_raw(raw: *mut Self::Component) -> Self;

    /// 从`table`中借用`colum`
    fn get_colum(table: &'a Table) -> Option<Self::Colum>;
}

impl<'a, T: Component> ComponentRef<'a> for &'a T {
    type Ref = Ref<'a, T>;

    type Colum = TableColum<'a, T>;

    type Component = T;

    fn get_component(entity: EntityRef<'a>) -> Option<Self::Ref> {
        unsafe { Ref::new(entity.table, entity.row_index).ok() }
    }

    unsafe fn from_raw(raw: *mut Self::Component) -> Self {
        unsafe { &*raw }
    }

    fn get_colum(table: &'a Table) -> Option<Self::Colum> {
        TableColum::new(table)
    }
}

impl<'a, T: Component> ComponentRef<'a> for &'a mut T {
    type Ref = RefMut<'a, T>;

    type Colum = TableColumMut<'a, T>;

    type Component = T;

    fn get_component(entity: EntityRef<'a>) -> Option<Self::Ref> {
        unsafe { RefMut::new(entity.table, entity.row_index).ok() }
    }

    unsafe fn from_raw(raw: *mut Self::Component) -> Self {
        unsafe { &mut *raw }
    }

    fn get_colum(table: &'a Table) -> Option<Self::Colum> {
        TableColumMut::new(table)
    }
}

pub trait ComponentRefShared<'a>: ComponentRef<'a> {}

impl<'a, T: Component> ComponentRefShared<'a> for &'a T {}

/// 带组件的实体的句柄
#[derive(Clone, Copy)]
pub struct EntityRef<'a> {
    entity_datas: &'a SparseSet<Entity, EntityData>,
    table: &'a Table,
    row_index: usize,
}

impl<'a> EntityRef<'a> {
    pub unsafe fn new(
        entity_datas: &'a SparseSet<Entity, EntityData>,
        table: &'a Table,
        row_index: usize,
    ) -> Self {
        Self {
            entity_datas,
            table,
            row_index,
        }
    }

    pub fn entity(&self) -> Entity {
        self.table.entity(self.row_index)
    }

    pub fn satisfies<Q: Query>(&self) -> bool {
        Q::Fetch::access(self.table).is_some()
    }

    pub fn has<T: Component>(&self) -> bool {
        self.table.has_component_type::<T>()
    }

    pub fn get<T: ComponentRef<'a>>(&self) -> Option<T::Ref> {
        T::get_component(*self)
    }

    pub fn query<Q: Query>(&self) -> QueryOne<'a, Q> {
        unsafe { QueryOne::new(self.table, self.row_index) }
    }

    pub fn component_types(&self) -> impl Iterator<Item = TypeId> + 'a {
        self.table.types().iter().map(|ty| ty.id())
    }

    pub fn colum_count(&self) -> usize {
        self.table.types().len()
    }

    pub fn is_empty(&self) -> bool {
        self.colum_count() == 0
    }
}

unsafe impl Send for EntityRef<'_> {}
unsafe impl Sync for EntityRef<'_> {}
