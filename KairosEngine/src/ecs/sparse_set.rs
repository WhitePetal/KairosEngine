use std::ops::{Index, IndexMut};

use crate::ecs::{
    consts::SPARSE_PAGE_SIZE,
    id::{Id, IdFlag},
};

pub mod entity_stroge;

pub use entity_stroge::*;

pub struct SparsePos {
    pub page: usize,
    pub slot: usize,
}
impl SparsePos {
    #[inline(always)]
    pub fn new(page: usize, slot: usize) -> Self {
        Self { page, slot }
    }

    #[inline(always)]
    pub fn from_entity(entity: usize) -> Self {
        Self {
            page: entity / SPARSE_PAGE_SIZE,
            slot: entity % SPARSE_PAGE_SIZE,
        }
    }
}

struct Page<T>(Box<[T; SPARSE_PAGE_SIZE]>)
where
    T: Id + Copy;
impl<T> Page<T>
where
    T: Id + Copy,
{
    pub fn new() -> Self {
        Self(Box::new([T::get_invalide_id(); SPARSE_PAGE_SIZE]))
    }
}

pub struct SparseSet<I, V>
where
    I: Id,
    V: Copy,
{
    dense: Vec<V>,
    sparse: Vec<Page<I>>,
    head: usize,
}
impl<I, V> SparseSet<I, V>
where
    I: Id,
    V: Copy,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            dense: Vec::with_capacity(capacity),
            sparse: Vec::with_capacity(capacity),
            head: 0,
        }
    }

    pub fn push_back(&mut self, id: I, value: V) {
        debug_assert!(!self.has(id));

        if self.head == self.dense.len() {
            self.dense.push(value);
        } else {
            self.dense[self.head] = value;
        }

        let sparse_pos = Self::get_sparse_pos(&id);
        if self.sparse.get(sparse_pos.page).is_none() {
            self.sparse.push(Page::new());
        }
        self.sparse[sparse_pos.page].0[sparse_pos.slot] =
            I::new(self.head as u32, id.get_version(), I::FlagType::default());
        self.head = self.head + 1;
    }

    pub fn remove(&mut self, remove_id: &I, end_id: &I) {
        let sparse_pos = Self::get_sparse_pos(remove_id);
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when remove id: {:?}",
            remove_id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "Remove the id is not alive! id: {:?}",
            remove_id
        );

        let index = self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            index.get_version() == remove_id.get_version(),
            "The id's version is invalided while remove the id! id: {:?}",
            remove_id
        );

        self.head = self.head - 1;
        let target = self.dense[self.head];

        let index = index.get_idx() as usize;
        let value = self.dense[index];

        self.dense[index] = target;
        self.dense[self.head] = value;

        let target_sparse_pos = Self::get_sparse_pos(end_id);
        debug_assert!(
            self.sparse.get(target_sparse_pos.page).is_some(),
            "Remove the id failed, the end_id is invalide! id: {:?}, end_id: {:?}",
            remove_id,
            end_id
        );
        debug_assert!(
            self.sparse[target_sparse_pos.page].0[target_sparse_pos.slot].is_avalide(),
            "Remove the id failed, the end_id is invalide! id: {:?}, end_id: {:?}",
            remove_id,
            end_id
        );
        let target_entity = self.sparse[target_sparse_pos.page].0[target_sparse_pos.slot];
        debug_assert!(
            target_entity.get_version() == end_id.get_version(),
            "The end_id's version is invalided while remove the id! id: {:?}, end_id: {:?}",
            remove_id,
            end_id
        );

        self.sparse[sparse_pos.page].0[sparse_pos.slot] = target_entity;
        self.sparse[target_sparse_pos.page].0[target_sparse_pos.slot] =
            remove_id.replace_flags(I::FlagType::get_invalide_flag());
    }

    #[inline(always)]
    pub fn get_head(&self) -> usize {
        self.head
    }

    #[inline(always)]
    pub fn get_value(&self, id: I) -> V {
        self[id]
    }

    #[inline(always)]
    pub fn has(&self, id: I) -> bool {
        let sparse_pos = Self::get_sparse_pos(&id);
        if self.sparse.get(sparse_pos.page).is_none() {
            return false;
        }
        let index = self.sparse[sparse_pos.page].0[sparse_pos.slot];
        if index.get_version() != id.get_version() {
            return false;
        }
        self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide()
    }

    fn get_sparse_pos<T>(id: &T) -> SparsePos
    where
        T: Id,
    {
        let idx = id.get_idx() as usize;
        let page = idx / SPARSE_PAGE_SIZE;
        let slot = idx % SPARSE_PAGE_SIZE;
        SparsePos::new(page, slot)
    }
}

impl<I, V> Index<I> for SparseSet<I, V>
where
    I: Id,
    V: Copy,
{
    type Output = V;

    fn index(&self, id: I) -> &Self::Output {
        let sparse_pos = Self::get_sparse_pos(&id);
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when get value, id: {:?}",
            id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "The id is not alive! while get value, id: {:?}",
            id
        );
        let index = self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            id.get_version() == index.get_version(),
            "The id's version is invalide! while get value, id: {:?}",
            id
        );
        &self.dense[index.get_idx() as usize]
    }
}

impl<I, V> IndexMut<I> for SparseSet<I, V>
where
    I: Id,
    V: Copy,
{
    fn index_mut(&mut self, id: I) -> &mut Self::Output {
        let sparse_pos = Self::get_sparse_pos(&id);
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when get value, id: {:?}",
            id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "The id is not alive! while get value, id: {:?}",
            id
        );
        let index = self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            id.get_version() == index.get_version(),
            "The id's version is invalide! while get value, id: {:?}",
            id
        );
        &mut self.dense[index.get_idx() as usize]
    }
}
