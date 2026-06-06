use crate::ecs::{
    consts::SPARSE_PAGE_SIZE,
    entity::{Entity, EntityFlag}, id::{Id, IdFlag},
};

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
    T: Default + Copy;
impl<T> Page<T>
where
    T: Default + Copy,
{
    pub fn new() -> Self {
        Self(Box::new([T::default(); SPARSE_PAGE_SIZE]))
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

    pub fn remove(&mut self, id: &I) {
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
        self.head = self.head - 1;
        let target = self.head;
        self.dense[target] = self.dense[index];

        let target_sparse_pos = SparsePos::from_entity(target);
        let target_entity = self.sparse[target_sparse_pos.page].0[target_sparse_pos.slot];
        self.sparse[sparse_pos.page].0[sparse_pos.slot] =
            target_entity.replace_idx(index as u32);
        self.sparse[target_sparse_pos.page].0[target_sparse_pos.slot] =
            id.replace_flags(I::FlagType::get_invalide_flag());
    }

    fn get_sparse_pos<T>(id: &T) -> SparsePos where T: Id {
        let idx = id.get_idx() as usize;
        let page = idx / SPARSE_PAGE_SIZE;
        let slot = idx % SPARSE_PAGE_SIZE;
        SparsePos::new(page, slot)
    }
}
