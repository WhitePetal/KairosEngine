mod maybe_location;
mod debug_info;

pub use maybe_location::MaybeLocation;

/// A debug checked version of [`Option::unwrap_unchecked`]. Will panic in
/// debug modes if unwrapping a `None` or `Err` value in debug mode, but is
/// equivalent to `Option::unwrap_unchecked` or `Result::unwrap_unchecked`
/// in release mode.
#[doc(hidden)]
pub trait DebugCheckedUnwrap {
    type Item;
    /// # Panics
    /// Panics if the value is `None` or `Err`, only in debug mode.
    ///
    /// # Safety
    /// This must never be called on a `None` or `Err` value. This can
    /// only be called on `Some` or `Ok` values.
    unsafe fn debug_checked_unwrap(self) -> Self::Item;
}

// These two impls are explicitly split to ensure that the unreachable! macro
// does not cause inlining to fail when compiling in release mode.
#[cfg(debug_assertions)]
impl<T> DebugCheckedUnwrap for Option<T> {
    type Item = T;

    #[inline(always)]
    #[track_caller]
    unsafe fn debug_checked_unwrap(self) -> Self::Item {
        if let Some(inner) = self {
            inner
        } else {
            unreachable!()
        }
    }
}

// These two impls are explicitly split to ensure that the unreachable! macro
// does not cause inlining to fail when compiling in release mode.
#[cfg(debug_assertions)]
impl<T, U> DebugCheckedUnwrap for Result<T, U> {
    type Item = T;

    #[inline(always)]
    #[track_caller]
    unsafe fn debug_checked_unwrap(self) -> Self::Item {
        if let Ok(inner) = self {
            inner
        } else {
            unreachable!()
        }
    }
}

// These two impls are explicitly split to ensure that the unreachable! macro
// does not cause inlining to fail when compiling in release mode.
#[cfg(not(debug_assertions))]
impl<T, U> DebugCheckedUnwrap for Result<T, U> {
    type Item = T;

    #[inline(always)]
    #[track_caller]
    unsafe fn debug_checked_unwrap(self) -> Self::Item {
        if let Ok(inner) = self {
            inner
        } else {
            core::hint::unreachable_unchecked()
        }
    }
}

#[cfg(not(debug_assertions))]
impl<T> DebugCheckedUnwrap for Option<T> {
    type Item = T;

    #[inline(always)]
    unsafe fn debug_checked_unwrap(self) -> Self::Item {
        if let Some(inner) = self {
            inner
        } else {
            core::hint::unreachable_unchecked()
        }
    }
}
