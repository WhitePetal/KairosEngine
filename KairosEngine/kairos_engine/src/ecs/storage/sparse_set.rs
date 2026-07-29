use std::hash::Hash;

/// Represents something that can be stored in a [`SparseSet`] as an integer.
///
/// Ideally, the `usize` values should be very small (ie: incremented starting from
/// zero), as the number of bits needed to represent a `SparseSetIndex` in a `FixedBitSet`
/// is proportional to the **value** of those `usize`.
pub trait SparseSetIndex: Clone + PartialEq + Eq + Hash {
    /// Gets the sparse set index corresponding to this instance.
    fn sparse_set_index(&self) -> usize;
    /// Creates a new instance of this type with the specified index.
    fn get_sparse_set_index(value: usize) -> Self;
}

macro_rules! impl_sparse_set_index {
    ($($ty:ty),+) => {
        $(impl SparseSetIndex for $ty {
            #[inline]
            fn sparse_set_index(&self) -> usize {
                *self as usize
            }

            #[inline]
            fn get_sparse_set_index(value: usize) -> Self {
                value as $ty
            }
        })*
    };
}

impl_sparse_set_index!(u8, u16, u32, u64, usize);
