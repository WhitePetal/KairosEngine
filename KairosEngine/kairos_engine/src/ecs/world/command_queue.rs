use std::{mem::MaybeUninit, ptr::NonNull};



/// Wraps pointers to a [`CommandQueue`], used internally to avoid stacked borrow rules when
/// partially applying the world's command queue recursively
#[derive(Clone)]
pub(crate) struct RawCommandQueue {
    pub(crate) bytes: NonNull<Vec<MaybeUninit<u8>>>,
    pub(crate) cursor: NonNull<usize>,
    pub(crate) panic_recovery: NonNull<Vec<MaybeUninit<u8>>>,
}
