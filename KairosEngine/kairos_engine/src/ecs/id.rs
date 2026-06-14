use std::{fmt::Debug, hash::Hash};

pub trait IdFlag: Debug + Default + Into<u32> {
    fn get_invalide_flag() -> Self;
}

pub trait Id: Debug + Clone + PartialEq + Eq + Hash {
    type FlagType: IdFlag;

    fn new(idx: u32, version: u32, flags: Self::FlagType) -> Self;

    #[inline(always)]
    fn get_invalide_id() -> Self
    where
        Self: Sized,
    {
        Self::new(0, 0, Self::FlagType::get_invalide_flag())
    }

    fn idx(&self) -> u32;

    fn version(&self) -> u32;

    fn flags(&self) -> Self::FlagType;

    fn from_other(idx: u32, other: &Self) -> Self;

    fn replace_idx(&mut self, idx: u32);

    fn create_idx_variant(&self, idx: u32) -> Self;

    fn replace_flags(&mut self, flags: Self::FlagType);

    fn get_next_version(self, flags: Self::FlagType) -> Self;

    fn is_avalide(&self) -> bool {
        (self.flags().into() & (Self::FlagType::get_invalide_flag().into()))
            == Self::FlagType::default().into()
    }
}
