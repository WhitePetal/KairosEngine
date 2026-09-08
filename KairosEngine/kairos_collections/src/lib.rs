//! Hashing utilities and deterministic collections shared by `kairos_ecs` and `kairos_engine`.
//!
//! Collection types live flat at the crate root, hashing utilities live in
//! the [`hash`] submodule (mirroring the split of `bevy_utils`/`bevy_platform`
//! that this crate was extracted from).

mod fixed_hashmap;
mod fixed_hashset;
mod pre_hash_map;
mod type_id_map;

pub mod hash;

pub use fixed_hashmap::FixedHashMap;
pub use fixed_hashset::FixedHashSet;
pub use pre_hash_map::{PreHashMap, PreHashMapExt};
pub use type_id_map::{TypeIdMap, TypeIdMapEntry, TypeIdMapExt};
