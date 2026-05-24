
use crate::ecs::{consts::{ENTITY_MASK, ENTITY_MASK_OFFSET, FLAG_MASK, FLAG_MASK_LEN, SPARSE_PAGE_SIZE, VERSION_MASK, VERSION_MASK_OFFSET}, sparse_set::SparsePos};

pub enum EntityFlag {
    Default = 0x0,
    Dead = 0x1,
}

#[derive(Debug, Clone, Copy)]
pub struct Entity(u64);

impl Default for Entity {
    fn default() -> Self {
        Entity::new(0, 0, EntityFlag::Dead)
    }
}

impl Entity {
    #[inline(always)]
    pub fn new(idx: u32, version: u32, flags: EntityFlag) -> Self {
        Self(
            ((flags as u64) << VERSION_MASK_OFFSET) | ((version as u64) << ENTITY_MASK_OFFSET) | (idx as u64)
        )
    }

    #[inline(always)]
    pub fn combine(idx: u32, entity: &Entity) -> Self {
        Self (
            ((entity.0 >> ENTITY_MASK_OFFSET) << ENTITY_MASK_OFFSET) | (idx as u64)
        )
    }

    #[inline(always)]
    pub fn get_entity(&self) -> u32 {
        (self.0 & ENTITY_MASK) as u32
    }

    #[inline(always)]
    pub fn get_version(&self) -> u32 {
        ((self.0 >> ENTITY_MASK_OFFSET) & VERSION_MASK) as u32
    }

    #[inline(always)]
    pub fn get_flags(&self) -> u32 {
        ((self.0 >> VERSION_MASK_OFFSET) & FLAG_MASK) as u32
    }

    #[inline(always)]
    pub fn replace_entity(&self, entity: u32) -> Self {
        Self::combine(entity, self)
    }

    #[inline(always)]
    pub fn replace_flags(&self, flags: EntityFlag) -> Self {
        Self(((flags as u64 & FLAG_MASK) << VERSION_MASK_OFFSET) | ((self.0 << FLAG_MASK_LEN) >> FLAG_MASK_LEN))
    }

    #[inline(always)]
    pub fn is_alive(&self) -> bool {
        (self.get_flags() & (EntityFlag::Dead as u32)) == 0u32
    }

    pub fn get_next(&self, flags: EntityFlag) -> Self {
        let version = self.get_version() + 1;
        Self::new(self.get_entity(), version, flags)
    }

    pub fn get_page_index(&self) -> usize {
        let entity = self.get_entity();
        let index = entity as usize / SPARSE_PAGE_SIZE;
        index
    }

    pub fn get_slot_index(&self) -> usize {
        let entity = self.get_entity();
        let index = entity as usize % SPARSE_PAGE_SIZE;
        index
    }
    
    pub fn get_sparse_pos(&self) -> SparsePos {
        let entity = self.get_entity() as usize;
        let page = entity / SPARSE_PAGE_SIZE;
        let slot = entity % SPARSE_PAGE_SIZE;
        SparsePos::new(page, slot)
    }
}
