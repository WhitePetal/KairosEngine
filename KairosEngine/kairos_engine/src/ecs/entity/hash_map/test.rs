use static_assertions::assert_impl_all;

use crate::ecs::entity::hash_map::EntityHashMap;

// Check that the HashMaps are Clone if the key/values are Clone
assert_impl_all!(EntityHashMap::<usize>: Clone);
// EntityHashMap should implement Reflect
#[cfg(feature = "bevy_reflect")]
assert_impl_all!(EntityHashMap::<i32>: Reflect);
