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
        if self.head == self.dense.len() {
            self.dense.push(value);
        } else {
            self.dense[self.head - 1] = value;
        }

        let sparse_pos = Self::get_sparse_pos(&id);
        if self.sparse.get(sparse_pos.page).is_none() {
            self.sparse.push(Page::new());
        }
        self.sparse[sparse_pos.page].0[sparse_pos.slot] =
            I::new(self.head as u32, id.get_version(), I::FlagType::default());
        self.head = self.head + 1;
    }

    pub fn remove(&mut self, id: &I) -> V {
        let sparse_pos = Self::get_sparse_pos(id);
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when remove id: {:?}",
            id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "Remove the id is not alive! id: {:?}",
            id
        );

        let index = self.sparse[sparse_pos.page].0[sparse_pos.slot].get_idx() as usize;
        let value = self.dense[index];
        self.head = self.head - 1;
        let target = self.head;
        self.dense[target] = value;

        let target_sparse_pos = SparsePos::from_entity(target);
        let target_entity = self.sparse[target_sparse_pos.page].0[target_sparse_pos.slot];
        self.sparse[sparse_pos.page].0[sparse_pos.slot] = target_entity.replace_idx(index as u32);
        self.sparse[target_sparse_pos.page].0[target_sparse_pos.slot] =
            id.replace_flags(I::FlagType::get_invalide_flag());

        value
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
        let index = self.sparse[sparse_pos.page].0[sparse_pos.slot].get_idx();
        &self.dense[index as usize]
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
        let index = self.sparse[sparse_pos.page].0[sparse_pos.slot].get_idx();
        &mut self.dense[index as usize]
    }
}
