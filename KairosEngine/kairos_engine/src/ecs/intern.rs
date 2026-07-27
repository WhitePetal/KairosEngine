use crate::collections::fixed_hashset::FixedHashSet;
use crate::hash::FixedHasher;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{PoisonError, RwLock};

pub trait Internable: Hash + Eq {
    fn leak(&self) -> &'static Self;

    fn ref_eq(&self, other: &Self) -> bool;

    fn ref_hash<H: Hasher>(&self, other: &mut H);
}
impl Internable for str {
    fn leak(&self) -> &'static Self {
        let str = self.to_owned().into_boxed_str();
        Box::leak(str)
    }

    fn ref_eq(&self, other: &Self) -> bool {
        self.as_ptr() == other.as_ptr() && self.len() == other.len()
    }

    fn ref_hash<H: Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        self.as_ptr().hash(state);
    }
}

pub struct Interned<T: ?Sized + Internable + 'static>(pub &'static T);

impl<T: ?Sized + Internable> Deref for Interned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<T: ?Sized + Internable> Clone for Interned<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized + Internable> Copy for Interned<T> {}

impl<T: ?Sized + Internable> PartialEq for Interned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.ref_eq(other.0)
    }
}
impl<T: ?Sized + Internable> Eq for Interned<T> {}

impl<T: ?Sized + Internable> Hash for Interned<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.ref_hash(state);
    }
}

impl<T: ?Sized + Internable + Debug> Debug for Interned<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: ?Sized + Internable> From<&Interned<T>> for Interned<T> {
    fn from(value: &Interned<T>) -> Self {
        *value
    }
}

/// A thread-safe interner which can be used to create [`Interned<T>`] from `&T`
///
/// For details on interning, see [the module level docs](self).
///
/// The implementation ensures that two equal values return two equal [`Interned<T>`] values.
///
/// To use an [`Interner<T>`], `T` must implement [`Internable`].
pub struct Interner<T: ?Sized + 'static>(RwLock<FixedHashSet<&'static T>>);

impl<T: ?Sized> Interner<T> {
    pub const fn new() -> Self {
        Self(RwLock::new(FixedHashSet::with_hasher(FixedHasher)))
    }
}

impl<T: Internable + ?Sized> Interner<T> {
    pub fn intern(&self, value: &T) -> Interned<T> {
        {
            let set = self.0.read().unwrap_or_else(PoisonError::into_inner);

            if let Some(value) = set.get(value) {
                return Interned(*value);
            }
        }

        {
            let mut set = self.0.write().unwrap_or_else(PoisonError::into_inner);

            if let Some(value) = set.get(value) {
                Interned(*value)
            } else {
                let leaked = value.leak();
                set.insert(leaked);
                Interned(leaked)
            }
        }
    }
}

impl<T: ?Sized> Default for Interner<T> {
    fn default() -> Self {
        Self::new()
    }
}
