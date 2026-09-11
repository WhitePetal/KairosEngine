use core::{any::TypeId, cmp::Ordering};
use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::Arc,
};

use uuid::Uuid;

use super::*;

#[derive(Debug)]
struct TestAsset;

impl Asset for TestAsset {}
impl VisitAssetDependencies for TestAsset {}

#[derive(Debug)]
struct OtherAsset;

impl Asset for OtherAsset {}
impl VisitAssetDependencies for OtherAsset {}

fn provider<A: Asset>() -> AssetHandleProvider {
    AssetHandleProvider::new(TypeId::of::<A>(), Arc::new(AssetIndexAllocator::default()))
}

fn hash_of<T: Hash>(value: &T) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn index_of<A: Asset>(handle: &Handle<A>) -> AssetIndex {
    slot_of_id(handle.id())
}

#[test]
fn asset_index_round_trip() {
    let index = AssetIndex {
        index: 1337,
        generation: 42,
    };
    assert_eq!(index.index(), 1337);
    assert_eq!(index.generation(), 42);
    assert_eq!(AssetIndex::from_bits(index.to_bits()), index);
}

#[test]
fn allocator_reserves_distinct_indices_and_recycles() {
    let allocator = AssetIndexAllocator::default();

    let a = allocator.reserve();
    let b = allocator.reserve();
    assert_eq!(a.index(), 0);
    assert_eq!(b.index(), 1);
    assert_eq!(a.generation(), 0);
    assert_ne!(a, b);

    allocator.recycle(a);
    let recycled = allocator.reserve();
    assert_eq!(recycled.index(), a.index());
    assert_eq!(recycled.generation(), 1);
    assert_ne!(recycled, a, "a recycled slot must not alias its old index");
}

#[test]
fn strong_handle_refcount_is_arc_strong_count_and_drop_sends_event() {
    let provider = provider::<TestAsset>();
    let drop_receiver = provider.drop_receiver();

    let handle: Handle<TestAsset> = provider.reserve_handle().typed();
    let strong_count = |handle: &Handle<TestAsset>| match handle {
        Handle::Strong(strong) => Arc::strong_count(strong),
        Handle::Uuid(..) => panic!("expected a strong handle"),
    };
    assert_eq!(strong_count(&handle), 1);

    let clone = handle.clone();
    assert_eq!(strong_count(&handle), 2);

    drop(clone);
    assert_eq!(strong_count(&handle), 1);
    assert!(
        drop_receiver.try_recv().is_err(),
        "no drop event while a clone is alive"
    );

    let index = index_of(&handle);
    drop(handle);

    let event = drop_receiver
        .try_recv()
        .expect("last clone must send a drop event");
    assert_eq!(event.index(), index);
    assert_eq!(event.type_id(), TypeId::of::<TestAsset>());
}

#[test]
fn uuid_handle_defaults_and_reports_itself() {
    let handle = Handle::<TestAsset>::default();
    assert!(handle.is_uuid());
    assert!(!handle.is_strong());
    assert_eq!(handle.id(), AssetId::<TestAsset>::default());
    assert_eq!(
        handle.id(),
        AssetId::Uuid {
            uuid: AssetId::<TestAsset>::DEFAULT_UUID
        }
    );
}

#[test]
fn handle_identity_is_by_id() {
    let provider = provider::<TestAsset>();
    let first: Handle<TestAsset> = provider.reserve_handle().typed();
    let index = index_of(&first);
    let second = Handle::<TestAsset>::Strong(provider.get_handle(index));

    assert_eq!(first, second);
    assert_eq!(first.cmp(&second), Ordering::Equal);
    assert_eq!(hash_of(&first), hash_of(&second));
}

#[test]
fn untyped_handle_round_trips_and_rejects_wrong_type() {
    let provider = provider::<TestAsset>();
    let untyped = provider.reserve_handle();
    let typed: Handle<TestAsset> = untyped.clone().typed();

    assert_eq!(untyped, typed, "typed and untyped handles compare by id");
    assert_eq!(hash_of(&untyped), hash_of(&typed));
    assert_eq!(typed.untyped(), untyped);

    let error = untyped.try_typed::<OtherAsset>().unwrap_err();
    assert!(matches!(
        error,
        UntypedAssetConversionError::TypeIdMismatch { .. }
    ));
}

#[test]
fn asset_id_default_and_invalid() {
    assert_eq!(
        AssetId::<TestAsset>::default(),
        AssetId::Uuid {
            uuid: AssetId::<TestAsset>::DEFAULT_UUID
        }
    );
    assert_eq!(
        AssetId::<TestAsset>::invalid(),
        AssetId::Uuid {
            uuid: AssetId::<TestAsset>::INVALID_UUID
        }
    );
}

#[test]
fn typed_and_untyped_ids_are_interchangeable() {
    let typed = AssetId::<TestAsset>::Uuid {
        uuid: Uuid::from_u128(123),
    };
    let untyped = UntypedAssetId::Uuid {
        type_id: TypeId::of::<TestAsset>(),
        uuid: Uuid::from_u128(123),
    };

    assert_eq!(typed, untyped);
    assert_eq!(untyped, typed);
    assert_eq!(typed.untyped(), untyped);
    assert_eq!(UntypedAssetId::from(typed), untyped);
    assert_eq!(AssetId::<TestAsset>::try_from(untyped).unwrap(), typed);
    assert_eq!(hash_of(&typed), hash_of(&untyped));

    assert!(untyped.try_typed::<OtherAsset>().is_err());
}

#[test]
fn slot_ids_convert_between_typed_and_untyped() {
    let index = AssetIndexAllocator::default().reserve();
    let typed = AssetId::<TestAsset>::from(index);
    let untyped = UntypedAssetId::Index {
        type_id: TypeId::of::<TestAsset>(),
        index,
    };

    assert_eq!(typed.untyped(), untyped);
    assert_eq!(untyped.typed::<TestAsset>(), typed);
    assert_eq!(slot_of_id(typed), index);
}

fn slot_of_id<A: Asset>(id: AssetId<A>) -> AssetIndex {
    match id {
        AssetId::Index { index, .. } => index,
        AssetId::Uuid { .. } => panic!("expected a slot-based id"),
    }
}

#[test]
fn untyped_ids_order_by_type_then_id() {
    let small = AssetId::<TestAsset>::Uuid {
        uuid: Uuid::from_u128(1),
    };
    let large = AssetId::<TestAsset>::Uuid {
        uuid: Uuid::from_u128(2),
    };
    assert!(small < large);
    assert!(small.untyped() < large.untyped());
    assert!(small < large.untyped());
}

#[test]
fn visit_dependencies_walks_builtin_containers() {
    let provider = provider::<TestAsset>();
    let a: Handle<TestAsset> = provider.reserve_handle().typed();
    let b: Handle<TestAsset> = provider.reserve_handle().typed();
    let expected = vec![a.id().untyped(), b.id().untyped()];

    let mut collected = Vec::new();
    Some(vec![a.clone(), b.clone()]).visit_dependencies(&mut |id| collected.push(id));
    assert_eq!(collected, expected);

    let mut collected = Vec::new();
    [a.clone(), b.clone()].visit_dependencies(&mut |id| collected.push(id));
    assert_eq!(collected, expected);

    let map: HashMap<String, Handle<TestAsset>> =
        HashMap::from([("a".to_string(), a.clone()), ("b".to_string(), b.clone())]);
    let mut count = 0;
    map.visit_dependencies(&mut |_| count += 1);
    assert_eq!(count, 2);

    let set: HashSet<Handle<TestAsset>> = HashSet::from([a.clone(), b.clone()]);
    let mut count = 0;
    set.visit_dependencies(&mut |_| count += 1);
    assert_eq!(count, 2);

    // The default implementation is a leaf: it visits nothing.
    let mut count = 0;
    TestAsset.visit_dependencies(&mut |_| count += 1);
    assert_eq!(count, 0);
}
