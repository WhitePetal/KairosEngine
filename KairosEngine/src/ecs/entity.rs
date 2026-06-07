use crate::ecs::{
    consts::{
        FLAG_MASK, FLAG_MASK_LEN, IDX_MASK, IDX_MASK_OFFSET, VERSION_MASK, VERSION_MASK_OFFSET,
    },
    id::{Id, IdFlag},
};
use num_enum::{FromPrimitive, IntoPrimitive};

#[repr(u32)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, IntoPrimitive, FromPrimitive)]
pub enum EntityFlag {
    #[default]
    Default = 0x0,
    Dead = 0x1,
}

impl IdFlag for EntityFlag {
    fn get_invalide_flag() -> Self {
        Self::Dead
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Entity(u64);

impl Id for Entity {
    type FlagType = EntityFlag;

    #[inline(always)]
    fn new(idx: u32, version: u32, flags: Self::FlagType) -> Self {
        Self(
            ((flags as u64) << VERSION_MASK_OFFSET)
                | ((version as u64) << IDX_MASK_OFFSET)
                | (idx as u64),
        )
    }

    #[inline(always)]
    fn get_idx(&self) -> u32 {
        (self.0 & IDX_MASK) as u32
    }

    #[inline(always)]
    fn get_version(&self) -> u32 {
        ((self.0 >> IDX_MASK_OFFSET) & VERSION_MASK) as u32
    }

    #[inline(always)]
    fn get_flags(&self) -> Self::FlagType {
        Self::FlagType::from(((self.0 >> VERSION_MASK_OFFSET) & FLAG_MASK) as u32)
    }

    fn from_other(idx: u32, other: &Self) -> Self {
        Self(((other.0 >> IDX_MASK_OFFSET) << IDX_MASK_OFFSET) | (idx as u64))
    }

    #[inline(always)]
    fn replace_idx(&mut self, idx: u32) {
        *self = Self::from_other(idx, &self);
    }

    #[inline(always)]
    fn create_idx_variant(&self, idx: u32) -> Self {
        Self::from_other(idx, &self)
    }

    #[inline(always)]
    fn replace_flags(&mut self, flags: Self::FlagType) {
        *self = Self(
            ((flags as u64 & FLAG_MASK) << VERSION_MASK_OFFSET)
                | ((self.0 << FLAG_MASK_LEN) >> FLAG_MASK_LEN),
        );
    }

    #[inline(always)]
    fn get_next_version(self, flags: Self::FlagType) -> Self {
        let version = self.get_version() + 1;
        Self::new(self.get_idx(), version, flags)
    }
}
