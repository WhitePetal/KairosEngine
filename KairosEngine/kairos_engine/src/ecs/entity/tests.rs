use super::*;

// #[test]
// fn entity_niche_optimization() {
//     assert_eq!(size_of::<Entity>(), size_of::<Option<Entity>>());
// }

// #[test]
// fn entity_bits_roundtrip() {
//     let r = EntityIndex::from_raw_u32(0xDEADBEEF).unwrap();
//     assert_eq!(EntityIndex::from_bits(r.to_bits()), r);

//     let e = Entity::from_index_and_generation(
//         EntityIndex::from_raw_u32(0xDEADBEEF).unwrap(),
//         EntityGeneration::from_bits(0x5AADF00D),
//     );
//     assert_eq!(Entity::from_bits(e.to_bits()), e);
// }

// #[test]
// fn entity_const() {
//     const C1: Entity = Entity::from_index(EntityIndex::from_raw_u32(42).unwrap());
//     assert_eq!(42, C1.index_u32());
//     assert_eq!(0, C1.generation().to_bits());

//     const C2: Entity = Entity::from_bits(0x0000_00ff_0000_00cc);
//     assert_eq!(!0x0000_00cc, C2.index_u32());
//     assert_eq!(0x0000_00ff, C2.generation().to_bits());

//     const C3: u32 = Entity::from_index(EntityIndex::from_raw_u32(33).unwrap()).index_u32();
//     assert_eq!(33, C3);

//     const C4: u32 = Entity::from_bits(0x00dd_00ff_1111_1111)
//         .generation()
//         .to_bits();
//     assert_eq!(0x00dd_00ff, C4);
// }

// #[test]
// #[expect(
//     clippy::nonminimal_bool,
//     reason = "This intentionally tests all possible comparison operators as separate functions; thus, we don't want to rewrite these comparisons to use different operators."
// )]
// fn entity_comparison() {
//     assert_eq!(
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         ),
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         )
//     );
//     assert_ne!(
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(789)
//         ),
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         )
//     );
//     assert_ne!(
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         ),
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(789)
//         )
//     );
//     assert_ne!(
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         ),
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(456).unwrap(),
//             EntityGeneration::from_bits(123)
//         )
//     );

//     // ordering is by generation then by index

//     assert!(
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         ) >= Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         )
//     );
//     assert!(
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         ) <= Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         )
//     );
//     assert!(
//         !(Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         ) < Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         ))
//     );
//     assert!(
//         !(Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         ) > Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(123).unwrap(),
//             EntityGeneration::from_bits(456)
//         ))
//     );

//     assert!(
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(9).unwrap(),
//             EntityGeneration::from_bits(1)
//         ) < Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(1).unwrap(),
//             EntityGeneration::from_bits(9)
//         )
//     );
//     assert!(
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(1).unwrap(),
//             EntityGeneration::from_bits(9)
//         ) > Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(9).unwrap(),
//             EntityGeneration::from_bits(1)
//         )
//     );

//     assert!(
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(1).unwrap(),
//             EntityGeneration::from_bits(1)
//         ) > Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(2).unwrap(),
//             EntityGeneration::from_bits(1)
//         )
//     );
//     assert!(
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(1).unwrap(),
//             EntityGeneration::from_bits(1)
//         ) >= Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(2).unwrap(),
//             EntityGeneration::from_bits(1)
//         )
//     );
//     assert!(
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(2).unwrap(),
//             EntityGeneration::from_bits(2)
//         ) < Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(1).unwrap(),
//             EntityGeneration::from_bits(2)
//         )
//     );
//     assert!(
//         Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(2).unwrap(),
//             EntityGeneration::from_bits(2)
//         ) <= Entity::from_index_and_generation(
//             EntityIndex::from_raw_u32(1).unwrap(),
//             EntityGeneration::from_bits(2)
//         )
//     );
// }

// // Feel free to change this test if needed, but it seemed like an important
// // part of the best-case performance changes in PR#9903.
// #[test]
// fn entity_hash_keeps_similar_ids_together() {
//     use core::hash::BuildHasher;
//     let hash = EntityHash;

//     let first_id = 0xC0FFEE << 8;
//     let first_hash = hash.hash_one(Entity::from_index(
//         EntityIndex::from_raw_u32(first_id).unwrap(),
//     ));

//     for i in 1..=255 {
//         let id = first_id + i;
//         let hash = hash.hash_one(Entity::from_index(EntityIndex::from_raw_u32(id).unwrap()));
//         assert_eq!(first_hash.wrapping_sub(hash) as u32, i);
//     }
// }

// #[test]
// fn entity_hash_id_bitflip_affects_high_7_bits() {
//     use core::hash::BuildHasher;

//     let hash = EntityHash;

//     let first_id = 0xC0FFEE;
//     let first_hash = hash.hash_one(Entity::from_index(
//         EntityIndex::from_raw_u32(first_id).unwrap(),
//     )) >> 57;

//     for bit in 0..u32::BITS {
//         let id = first_id ^ (1 << bit);
//         let hash = hash.hash_one(Entity::from_index(EntityIndex::from_raw_u32(id).unwrap())) >> 57;
//         assert_ne!(hash, first_hash);
//     }
// }

// #[test]
// fn entity_generation_is_approximately_ordered() {
//     use core::cmp::Ordering;

//     let old = EntityGeneration::FIRST;
//     let middle = old.after_versions(1);
//     let younger_before_ord_wrap = middle.after_versions(EntityGeneration::DIFF_MAX);
//     let younger_after_ord_wrap = younger_before_ord_wrap.after_versions(1);

//     assert_eq!(middle.cmp_approx(&old), Ordering::Greater);
//     assert_eq!(middle.cmp_approx(&middle), Ordering::Equal);
//     assert_eq!(middle.cmp_approx(&younger_before_ord_wrap), Ordering::Less);
//     assert_eq!(
//         middle.cmp_approx(&younger_after_ord_wrap),
//         Ordering::Greater
//     );
// }

// #[test]
// fn entity_debug() {
//     let entity = Entity::from_index(EntityIndex::from_raw_u32(42).unwrap());
//     let string = format!("{entity:?}");
//     assert_eq!(string, "42v0");

//     let entity = Entity::PLACEHOLDER;
//     let string = format!("{entity:?}");
//     assert_eq!(string, "PLACEHOLDER");
// }

// #[test]
// fn entity_display() {
//     let entity = Entity::from_index(EntityIndex::from_raw_u32(42).unwrap());
//     let string = format!("{entity}");
//     assert_eq!(string, "42v0");

//     let padded_left = format!("{entity:<5}");
//     assert_eq!(padded_left, "42v0 ");

//     let padded_right = format!("{entity:>6}");
//     assert_eq!(padded_right, "  42v0");

//     let entity = Entity::PLACEHOLDER;
//     let string = format!("{entity}");
//     assert_eq!(string, "PLACEHOLDER");
// }

// #[test]
// fn allocator() {
//     let mut allocator = EntityAllocator::default();
//     let mut entities = allocator.alloc_many(2048).collect::<Vec<_>>();
//     for _ in 0..2048 {
//         entities.push(allocator.alloc());
//     }

//     let pre_len = entities.len();
//     entities.sort();
//     entities.dedup();
//     assert_eq!(pre_len, entities.len());

//     for e in entities.drain(..) {
//         allocator.free(e);
//     }

//     entities.extend(allocator.alloc_many(5000));
//     let pre_len = entities.len();
//     entities.sort();
//     entities.dedup();
//     assert_eq!(pre_len, entities.len());
// }
