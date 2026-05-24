use crate::ecs::{
    consts::SPARSE_PAGE_SIZE,
    entity::{Entity, EntityFlag},
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

pub struct EntityStorage {
    dense: Vec<Entity>,
    sparse: Vec<Page<Entity>>,
    head: usize,
}

impl EntityStorage {
    pub fn new(capacity: usize) -> Self {
        Self {
            dense: Vec::with_capacity(capacity),
            sparse: Vec::with_capacity(capacity),
            head: 0,
        }
    }

    pub fn create_entity(&mut self) -> Entity {
        let entity = {
            if self.head == self.dense.len() {
                let entity = Entity::new(self.head as u32, 0, EntityFlag::Default);
                self.dense.push(entity);
                entity
            } else {
                let entity = self.dense[self.head - 1];
                entity.get_next(EntityFlag::Default)
            }
        };
        let sparse_pos = entity.get_sparse_pos();
        if self.sparse.get(sparse_pos.page).is_none() {
            self.sparse.push(Page::new());
        }
        self.sparse[sparse_pos.page].0[sparse_pos.slot] =
            Entity::new(self.head as u32, entity.get_version(), EntityFlag::Default);
        self.head = self.head + 1;
        entity
    }

    pub fn remove_entity(&mut self, entity: &Entity) {
        let sparse_pos = entity.get_sparse_pos();
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when remove entity: {:?}",
            entity
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_alive(),
            "Remove the entity is not alive! entity: {:?}",
            entity
        );

        let index = self.sparse[sparse_pos.page].0[sparse_pos.slot].get_entity() as usize;
        let target = self.head - 1;
        let will_remove = self.dense[index];
        let will_move = self.dense[target];

        let will_remove = will_remove
            .replace_entity(target as u32)
            .get_next(EntityFlag::Dead);
        self.dense[target] = will_remove;
        self.dense[index] = will_move;

        self.sparse[sparse_pos.page].0[sparse_pos.slot] = will_move.replace_entity(index as u32);
        let sparse_pos = will_remove.get_sparse_pos();
        self.sparse[sparse_pos.page].0[sparse_pos.slot] = will_remove
    }
}

pub struct SparseSet<V>
where
    V: Copy,
{
    dense: Vec<V>,
    sparse: Vec<Page<Entity>>,
    head: usize,
}
impl<V> SparseSet<V>
where
    V: Copy,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            dense: Vec::with_capacity(capacity),
            sparse: Vec::with_capacity(capacity),
            head: 0,
        }
    }

    pub fn push_back(&mut self, entity: Entity, value: V) {
        if self.head == self.dense.len() {
            self.dense.push(value);
        } else {
            self.dense[self.head - 1] = value;
        }

        let sparse_pos = entity.get_sparse_pos();
        if self.sparse.get(sparse_pos.page).is_none() {
            self.sparse.push(Page::new());
        }
        self.sparse[sparse_pos.page].0[sparse_pos.slot] =
            Entity::new(self.head as u32, entity.get_version(), EntityFlag::Default);
        self.head = self.head + 1;
    }

    pub fn remove(&mut self, entity: &Entity) {
        let sparse_pos = entity.get_sparse_pos();
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when remove entity: {:?}",
            entity
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_alive(),
            "Remove the entity is not alive! entity: {:?}",
            entity
        );

        let index = self.sparse[sparse_pos.page].0[sparse_pos.slot].get_entity() as usize;
        let target = self.head - 1;
        self.dense[target] = self.dense[index];

        let target_sparse_pos = SparsePos::from_entity(target);
        let target_entity = self.sparse[target_sparse_pos.page].0[target_sparse_pos.slot];
        self.sparse[sparse_pos.page].0[sparse_pos.slot] =
            target_entity.replace_entity(index as u32);
        self.sparse[target_sparse_pos.page].0[target_sparse_pos.slot] =
            entity.replace_flags(EntityFlag::Dead);
    }
}
