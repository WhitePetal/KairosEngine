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

#[derive(Debug)]
struct CountedAsset {
    value: u32,
}

impl Asset for CountedAsset {}
impl VisitAssetDependencies for CountedAsset {}

fn modified_count<A: Asset>(assets: &Assets<A>) -> usize {
    assets
        .queued_events()
        .iter()
        .filter(|event| matches!(event, AssetEvent::Modified { .. }))
        .count()
}

#[test]
fn assets_insert_is_immediately_visible() {
    let mut assets = Assets::<TestAsset>::default();
    let uuid = Uuid::from_u128(1);
    let uuid_id = AssetId::<TestAsset>::Uuid { uuid };

    assets.insert(uuid_id, TestAsset).unwrap();
    assert!(assets.contains(uuid_id));
    assert!(assets.get(uuid_id).is_some());
    assert_eq!(assets.len(), 1);
    assert!(!assets.is_empty());

    // A slot id reserved from the store is insertable as soon as it is reserved.
    let handle = assets.reserve_handle();
    let slot_id = handle.id();
    assets.insert(slot_id, TestAsset).unwrap();
    assert!(assets.contains(slot_id));
    assert!(assets.get(slot_id).is_some());
    assert_eq!(assets.len(), 2);
}

#[test]
fn assets_strong_handle_lifecycle_removes_on_last_drop() {
    let mut assets = Assets::<TestAsset>::default();
    let handle = assets.add(TestAsset);
    let id = handle.id();

    assert!(assets.contains(id));
    assert_eq!(assets.len(), 1);
    assert_eq!(assets.queued_events().len(), 1, "add queues Added");

    let strong_count = |handle: &Handle<TestAsset>| match handle {
        Handle::Strong(strong) => Arc::strong_count(strong),
        Handle::Uuid(..) => panic!("expected a strong handle"),
    };
    assert_eq!(strong_count(&handle), 1);

    let clone = handle.clone();
    assert_eq!(strong_count(&handle), 2);
    drop(clone);
    assert_eq!(strong_count(&handle), 1);

    // A clone dropping is not the last handle: nothing is released yet.
    assets.drain_dropped_assets();
    assert!(assets.contains(id));
    assert_eq!(assets.len(), 1);

    drop(handle);
    assets.drain_dropped_assets();

    assert!(!assets.contains(id));
    assert!(assets.get(id).is_none());
    assert!(assets.is_empty());
    assert_eq!(assets.len(), 0);

    // `Unused` is queued before `Removed`.
    let tail: Vec<_> = assets.queued_events().iter().rev().take(2).collect();
    assert!(matches!(tail[0], AssetEvent::Removed { .. }));
    assert!(matches!(tail[1], AssetEvent::Unused { .. }));
}

#[test]
fn assets_stores_are_independent_per_type() {
    let mut first = Assets::<TestAsset>::default();
    let mut second = Assets::<OtherAsset>::default();

    let first_handle = first.add(TestAsset);
    let second_handle = second.add(OtherAsset);

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert!(first.get(first_handle.id()).is_some());
    assert!(second.get(second_handle.id()).is_some());

    // Dropping and tracking the first store leaves the second untouched.
    drop(first_handle);
    first.drain_dropped_assets();
    assert!(first.is_empty());
    assert_eq!(second.len(), 1);
    assert!(second.get(second_handle.id()).is_some());
}

#[test]
fn assets_recycles_slot_and_bumps_generation() {
    let mut assets = Assets::<TestAsset>::default();
    let first = assets.add(TestAsset);
    let first_id = first.id();
    drop(first);
    assets.drain_dropped_assets();
    assert!(assets.get(first_id).is_none());

    let second = assets.add(TestAsset);
    let second_id = second.id();
    assert!(assets.get(second_id).is_some());
    assert_ne!(first_id, second_id, "a recycled slot must not alias its old id");

    let first_slot = slot_of_id(first_id);
    let second_slot = slot_of_id(second_id);
    assert_eq!(first_slot.index(), second_slot.index());
    assert_ne!(first_slot.generation(), second_slot.generation());
}

#[test]
fn assets_get_mut_tracks_deref_mut_only() {
    let mut assets = Assets::<CountedAsset>::default();
    let handle = assets.add(CountedAsset { value: 0 });
    let id = handle.id();
    assert_eq!(modified_count(&assets), 0);

    {
        let mut asset = assets.get_mut(id).unwrap();
        asset.value += 1;
    }
    assert_eq!(modified_count(&assets), 1, "deref_mut queues Modified");
    assert_eq!(assets.get(id).unwrap().value, 1);

    {
        let asset = assets.get_mut(id).unwrap();
        let _ = asset.value;
    }
    assert_eq!(
        modified_count(&assets),
        1,
        "reading through the guard does not queue Modified"
    );
}

#[test]
fn assets_asset_mut_untracked_paths_do_not_queue_modified() {
    let mut assets = Assets::<CountedAsset>::default();
    let handle = assets.add(CountedAsset { value: 0 });
    let id = handle.id();

    {
        let asset = assets.get_mut(id).unwrap();
        let _ = asset.into_inner_untracked();
    }
    assert_eq!(modified_count(&assets), 0);

    {
        let mut asset = assets.get_mut(id).unwrap();
        let _ = asset.bypass_change_detection();
    }
    assert_eq!(modified_count(&assets), 0);

    {
        let asset = assets.get_mut(id).unwrap();
        let _ = asset.into_inner();
    }
    assert_eq!(modified_count(&assets), 1, "into_inner queues Modified");
}

#[test]
fn assets_get_mut_untracked_never_queues_modified() {
    let mut assets = Assets::<CountedAsset>::default();
    let handle = assets.add(CountedAsset { value: 0 });
    let id = handle.id();

    assets.get_mut_untracked(id).unwrap().value = 5;
    assert_eq!(assets.get(id).unwrap().value, 5);
    assert_eq!(modified_count(&assets), 0);
}

#[test]
fn assets_insert_distinguishes_occupied_and_removed() {
    let mut assets = Assets::<TestAsset>::default();
    let handle = assets.reserve_handle();
    let index = slot_of_id(handle.id());
    assets.insert(handle.id(), TestAsset).unwrap();

    let stale = AssetIndex {
        index: index.index(),
        generation: index.generation() + 1,
    };
    let error = assets
        .insert(AssetId::<TestAsset>::from(stale), TestAsset)
        .unwrap_err();
    assert!(matches!(error, InvalidGenerationError::Occupied { .. }));

    // Dropping the handle recycles the slot, after which the old index is gone.
    drop(handle);
    assets.drain_dropped_assets();
    let error = assets
        .insert(AssetId::<TestAsset>::from(index), TestAsset)
        .unwrap_err();
    assert!(matches!(error, InvalidGenerationError::Removed { .. }));
}

#[test]
fn assets_get_strong_handle_keeps_the_value_alive() {
    let mut assets = Assets::<TestAsset>::default();
    let handle = assets.add(TestAsset);
    let id = handle.id();

    let extra = assets.get_strong_handle(id).expect("a stored asset can be upgraded");
    drop(handle);
    assets.drain_dropped_assets();

    // The duplicate handle still keeps the value alive, so the drop is a no-op.
    assert!(assets.contains(id));
    assert!(assets.get(id).is_some());
    assert_ne!(assets.queued_events().last(), Some(&AssetEvent::Removed { id }));

    drop(extra);
    assets.drain_dropped_assets();
    assert!(!assets.contains(id));
    assert!(assets.get(id).is_none());

    // Once gone, it can no longer be upgraded.
    assert!(assets.get_strong_handle(id).is_none());
}

#[test]
fn assets_uuid_insert_replace_and_remove() {
    let mut assets = Assets::<TestAsset>::default();
    let id = AssetId::<TestAsset>::Uuid {
        uuid: Uuid::from_u128(7),
    };

    assets.insert(id, TestAsset).unwrap();
    assert!(assets.contains(id));
    assert!(matches!(assets.queued_events().last(), Some(AssetEvent::Added { .. })));

    assets.insert(id, TestAsset).unwrap();
    assert!(matches!(
        assets.queued_events().last(),
        Some(AssetEvent::Modified { .. })
    ));
    assert_eq!(assets.len(), 1);

    assert!(assets.remove(id).is_some());
    assert!(!assets.contains(id));
    assert!(assets.remove(id).is_none());
}

#[test]
fn assets_iter_ids_and_iter_mut() {
    let mut assets = Assets::<CountedAsset>::default();
    let handle = assets.add(CountedAsset { value: 1 });
    let slot_id = handle.id();
    let uuid_id = AssetId::<CountedAsset>::Uuid {
        uuid: Uuid::from_u128(9),
    };
    assets.insert(uuid_id, CountedAsset { value: 10 }).unwrap();

    let ids: Vec<_> = assets.ids().collect();
    assert!(ids.contains(&slot_id));
    assert!(ids.contains(&uuid_id));
    assert_eq!(assets.iter().count(), 2);

    for (_id, asset) in assets.iter_mut() {
        asset.value += 1;
    }
    assert_eq!(assets.get(slot_id).unwrap().value, 2);
    assert_eq!(assets.get(uuid_id).unwrap().value, 11);
    assert_eq!(modified_count(&assets), 2, "every visited asset is Modified");
}

#[test]
fn assets_with_capacity_is_an_empty_store_that_works() {
    let mut assets = Assets::<TestAsset>::with_capacity(16);
    assert!(assets.is_empty());
    assert_eq!(assets.len(), 0);
    assert!(assets.ids().next().is_none());

    let handle = assets.add(TestAsset);
    assert!(assets.get(handle.id()).is_some());
    assert_eq!(assets.len(), 1);
}

#[test]
fn asset_event_variants_report_themselves() {
    let id = AssetId::<TestAsset>::Uuid {
        uuid: Uuid::from_u128(3),
    };

    assert!(AssetEvent::Added { id }.is_added(id));
    assert!(AssetEvent::Modified { id }.is_modified(id));
    assert!(AssetEvent::Removed { id }.is_removed(id));
    assert!(AssetEvent::Unused { id }.is_unused(id));
    assert!(AssetEvent::LoadedWithDependencies { id }.is_loaded_with_dependencies(id));

    assert!(!AssetEvent::Added { id }.is_removed(id));
    assert_ne!(AssetEvent::Added { id }, AssetEvent::Removed { id });
    assert_eq!(AssetEvent::Added { id }, AssetEvent::Added { id });

    let failed = AssetLoadFailedEvent::<TestAsset>::new(id);
    assert_eq!(failed.id, id);
}

mod io;
mod loader;
mod meta;
mod path;
