use std::any::TypeId;

use indexmap::IndexMap;

use crate::hash::NoOpHash;

/// A specialized map type with Key of [`TypeId`]
/// Iteration order only depends on the order of insertions and deletions.
pub type TypeIdMap<V> = IndexMap<TypeId, V, NoOpHash>;
