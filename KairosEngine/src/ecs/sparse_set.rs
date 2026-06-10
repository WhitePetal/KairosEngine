use std::{
    array, iter::Zip, ops::{Index, IndexMut}
};

use crate::ecs::{consts::SPARSE_PAGE_SIZE, id::Id};

pub mod entity_stroge;
mod test;

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

#[derive(Debug)]
struct Page<T>(Box<[T; SPARSE_PAGE_SIZE]>)
where
    T: Id;
impl<T> Page<T>
where
    T: Id,
{
    pub fn new() -> Self {
        Self(Box::new(array::from_fn(|_| T::get_invalide_id())))
    }
}

#[derive(Debug)]
pub struct SparseSet<I, V>
where
    I: Id,
{
    dense_values: Vec<V>,
    dense_ids: Vec<I>,
    sparse: Vec<Page<I>>,
}
impl<I, V> SparseSet<I, V>
where
    I: Id,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            dense_values: Vec::with_capacity(capacity),
            dense_ids: Vec::with_capacity(capacity),
            sparse: Vec::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, id: &I, value: V) {
        let sparse_pos = Self::get_sparse_pos(id);
        if self.sparse.get(sparse_pos.page).is_none() {
            self.sparse.push(Page::new());
        }
        let sparse_value = &mut self.sparse[sparse_pos.page].0[sparse_pos.slot];
        if sparse_value.is_avalide() {
            debug_assert!(
                sparse_value.get_version() == id.get_version(),
                "Try insert a invalide version id! id: {:?}",
                id
            );
            self.dense_values[sparse_value.get_idx() as usize] = value;
            self.dense_ids[sparse_value.get_idx() as usize] = id.clone();
            self.sparse[sparse_pos.page].0[sparse_pos.slot].replace_flags(id.get_flags());
        } else {
            let end = self.dense_values.len();
            self.dense_values.push(value);
            self.dense_ids.push(id.clone());

            *sparse_value = I::from_other(end as u32, &id);
        }
    }

    pub fn remove(&mut self, id: I) -> V {
        let sparse_pos = Self::get_sparse_pos(&id);
        debug_assert!(
            self.dense_values.len() > 0,
            "The dense array is empty while remove element! id: {:?}",
            id
        );
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

        let sparse_value = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            sparse_value == &id,
            "The id is invalided while remove the id! id: {:?}",
            id
        );

        let end_index = self.dense_values.len() - 1;
        let end_id = &self.dense_ids[end_index];
        let end_sparse_pos = Self::get_sparse_pos(end_id);
        let end_sparse_value = &self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot];

        let index = sparse_value.get_idx() as usize;

        self.dense_values.swap(index, end_index);
        self.dense_ids.swap(index, end_index);

        self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot] =
            I::from_other(sparse_value.get_idx(), &end_sparse_value);
        self.sparse[sparse_pos.page].0[sparse_pos.slot] = I::get_invalide_id();

        self.dense_ids.pop();
        self.dense_values.pop().unwrap()
    }

    #[inline(always)]
    pub fn get_value(&self, id: &I) -> &V {
        &self[id]
    }

    #[inline(always)]
    pub fn get_value_mut(&mut self, id: &I) -> &mut V {
        &mut self[id]
    }

    #[inline(always)]
    pub fn has(&self, id: &I) -> bool {
        let sparse_pos = Self::get_sparse_pos(id);
        if self.sparse.get(sparse_pos.page).is_none() {
            return false;
        }
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
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

impl<I, V> Index<&I> for SparseSet<I, V>
where
    I: Id,
{
    type Output = V;

    fn index(&self, id: &I) -> &Self::Output {
        let sparse_pos = Self::get_sparse_pos(id);
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
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            id.get_version() == index.get_version(),
            "The id's version is invalide! while get value, id: {:?}",
            id
        );
        &self.dense_values[index.get_idx() as usize]
    }
}

impl<I, V> IndexMut<&I> for SparseSet<I, V>
where
    I: Id,
{
    fn index_mut(&mut self, id: &I) -> &mut Self::Output {
        let sparse_pos = Self::get_sparse_pos(id);
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
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            id.get_version() == index.get_version(),
            "The id's version is invalide! while get value, id: {:?}",
            id
        );
        &mut self.dense_values[index.get_idx() as usize]
    }
}

impl<I, V> SparseSet<I, V>
where
    I: Id,
{
    pub fn iter(&self) -> Zip<std::slice::Iter<'_, I>, std::slice::Iter<'_, V>> {
        self.dense_ids.iter().zip(&self.dense_values)
    }
    pub fn iter_mut(&mut self) -> Zip<std::slice::Iter<'_, I>, std::slice::IterMut<'_, V>> {
        self.dense_ids.iter().zip(&mut self.dense_values)
    }
}

#[derive(Debug)]
pub struct SparseStroge<I>
where
    I: Id,
{
    dense: Vec<I>,
    sparse: Vec<Page<I>>,
    head: usize,
}
impl<I> SparseStroge<I>
where
    I: Id,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            dense: Vec::with_capacity(capacity),
            sparse: Vec::with_capacity(capacity),
            head: 0,
        }
    }

    pub fn next(&mut self) -> I {
        if self.head < self.dense.len() {
            let entity = self.dense[self.head].clone();
            let entity = entity.get_next_version(I::FlagType::default());

            let sparse_pos = Self::get_sparse_pos(&entity);
            self.sparse[sparse_pos.page].0[sparse_pos.slot] =
                I::new(self.head as u32, entity.get_version(), entity.get_flags());
            self.dense[self.head] = entity.clone();

            self.head = self.head + 1;

            entity
        } else {
            let entity = I::new(self.head as u32, 0, I::FlagType::default());

            let sparse_pos = Self::get_sparse_pos(&entity);
            if self.sparse.get(sparse_pos.page).is_none() {
                self.sparse.push(Page::new());
            }

            self.sparse[sparse_pos.page].0[sparse_pos.slot] =
                I::new(self.head as u32, entity.get_version(), entity.get_flags());
            self.dense.push(entity.clone());

            self.head = self.head + 1;

            entity
        }
    }

    pub fn remove(&mut self, id: I) {
        let sparse_pos = Self::get_sparse_pos(&id);
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

        let sparse_value = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            sparse_value.get_version() == id.get_version(),
            "The id's version is invalided while remove the id! id: {:?}",
            id
        );

        self.head = self.head - 1;
        let end_index = self.head;
        let end_id = &self.dense[end_index];
        let end_sparse_pos = Self::get_sparse_pos(end_id);
        let end_sparse_value = &self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot];

        let index = sparse_value.get_idx() as usize;

        self.dense.swap(index, end_index);

        self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot] =
            I::from_other(sparse_value.get_idx(), &end_sparse_value);
        self.sparse[sparse_pos.page].0[sparse_pos.slot] = I::get_invalide_id();
    }

    #[inline(always)]
    pub fn has(&self, id: I) -> bool {
        let sparse_pos = Self::get_sparse_pos(&id);
        if self.sparse.get(sparse_pos.page).is_none() {
            return false;
        }
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
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
