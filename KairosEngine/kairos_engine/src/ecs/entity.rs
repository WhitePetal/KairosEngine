use std::fmt::Display;

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
impl Display for EntityFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl IdFlag for EntityFlag {
    fn get_invalide_flag() -> Self {
        Self::Dead
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity(u64);

impl Ord for Entity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.idx()
            .cmp(&other.idx())
            .then_with(|| self.version().cmp(&other.version()))
    }
}

impl PartialOrd for Entity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

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
    fn idx(&self) -> u32 {
        (self.0 & IDX_MASK) as u32
    }

    #[inline(always)]
    fn version(&self) -> u32 {
        ((self.0 >> IDX_MASK_OFFSET) & VERSION_MASK) as u32
    }

    #[inline(always)]
    fn flags(&self) -> Self::FlagType {
        Self::FlagType::from(((self.0 >> VERSION_MASK_OFFSET) & FLAG_MASK) as u32)
    }

    fn from_other(idx: u32, other: Self) -> Self {
        Self(((other.0 >> IDX_MASK_OFFSET) << IDX_MASK_OFFSET) | (idx as u64))
    }

    #[inline(always)]
    fn replace_idx(&mut self, idx: u32) {
        *self = Self::from_other(idx, *self);
    }

    #[inline(always)]
    fn create_idx_variant(&self, idx: u32) -> Self {
        Self::from_other(idx, *self)
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
        let version = self.version().wrapping_add(1);
        // 0 是全新实体的版本号，回绕时跳过
        let version = if version == 0 { 1 } else { version };
        Self::new(self.idx(), version, flags)
    }
}

impl Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "total_value: {}, idx: {}, version: {}, flags: {}",
            self.0,
            self.idx(),
            self.version(),
            self.flags()
        )
    }
}
