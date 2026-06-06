use std::alloc::Layout;

use num_enum::{FromPrimitive, IntoPrimitive};

use crate::ecs::{consts::{FLAG_MASK, FLAG_MASK_LEN, IDX_MASK, IDX_MASK_OFFSET, VERSION_MASK, VERSION_MASK_OFFSET}, id::{Id, IdFlag}};


#[repr(u32)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, IntoPrimitive, FromPrimitive)]
pub enum ComponentFlag {
    #[default]
    Default,
    Invalide
}
impl IdFlag for ComponentFlag {
    fn get_invalide_flag() -> Self {
        Self::Invalide
    }
}



#[derive(Default, Debug, Clone, Copy)]
pub struct ComponentId(u64);

impl Id for ComponentId {
    type FlagType = ComponentFlag;
    
    fn new(idx: u32, version: u32, flags: Self::FlagType) -> Self {
        Self(
            ((flags as u64) << VERSION_MASK_OFFSET)
                | ((version as u64) << IDX_MASK_OFFSET)
                | (idx as u64),
        )
    }
    
    fn get_idx(&self) -> u32 {
        (self.0 & IDX_MASK) as u32
    }
    
    fn get_version(&self) -> u32 {
        ((self.0 >> IDX_MASK_OFFSET) & VERSION_MASK) as u32
    }
    
    fn get_flags(&self) -> Self::FlagType {
        Self::FlagType::from(((self.0 >> VERSION_MASK_OFFSET) & FLAG_MASK) as u32)
    }
    
    fn from_other(idx: u32, other: &Self) -> Self {
        Self(((other.0 >> IDX_MASK_OFFSET) << IDX_MASK_OFFSET) | (idx as u64))
    }
    
    fn replace_idx(self, entity: u32) -> Self {
        Self::from_other(entity, &self)
    }
    
    fn replace_flags(self, flags: Self::FlagType) -> Self {
        Self(
            ((flags as u64 & FLAG_MASK) << VERSION_MASK_OFFSET)
                | ((self.0 << FLAG_MASK_LEN) >> FLAG_MASK_LEN),
        )
    }
    
    fn get_next_version(self) -> Self {
        let flags = self.get_flags();
        let version = self.get_version() + 1;
        Self::new(self.get_idx(), version, flags.into())
    }
}
