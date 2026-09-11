//! The per-type asset store: [`Assets<A>`].
//!
//! One `Assets<A>` resource holds every loaded value of asset type `A`. Values
//! registered by slot ([`AssetId::Index`]) live in a dense, generational
//! `Vec`-like storage; values registered by UUID ([`AssetId::Uuid`]) live in a
//! hash map. Both are addressed through a [`Handle`], an [`AssetId`], or anything
//! else that converts into one.
//!
//! The store owns the [`AssetHandleProvider`] that hands out strong handles for
//! its assets. Strong handles keep their asset alive until the last clone drops;
//! the store then learns about it from the provider's drop channel in
//! [`Assets::track_assets`] and releases the value.

use core::{
    any::TypeId,
    fmt,
    iter::Enumerate,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    slice::IterMut,
};
use std::{collections::HashMap, sync::Arc};

use kairos_ecs::message::MessageWriter;
use kairos_ecs::resource::Resource;
use kairos_ecs::system::{Res, ResMut};
use uuid::Uuid;

use crate::asset::Asset;
use crate::event::AssetEvent;
use crate::handle::{AssetHandleProvider, Handle};
use crate::id::AssetId;
use crate::index::{AssetIndex, AssetIndexAllocator};

/// One slot in a [`DenseAssetStorage`].
///
/// The two states are distinct on purpose:
///
/// - `None` means the slot has no live handle at all. It is free for the
///   allocator to recycle.
/// - `Some { value: None, .. }` means a handle has been reserved for the slot but
///   no value is stored yet (the asset is still loading). The slot must not be
///   recycled while that handle lives.
enum Entry<A: Asset> {
    None,
    Some { value: Option<A>, generation: u32 },
}

/// Stores [`Asset`] values in a `Vec`-like storage identified by [`AssetIndex`].
///
/// The storage is indexed by the dense slot number, so lookup is O(1). Each entry
/// records the generation its index was reserved at; a lookup whose generation
/// does not match misses, which is what stops a handle to an asset that has since
/// been dropped from aliasing whichever asset later took the slot.
struct DenseAssetStorage<A: Asset> {
    storage: Vec<Entry<A>>,
    len: u32,
    allocator: Arc<AssetIndexAllocator>,
}

impl<A: Asset> Default for DenseAssetStorage<A> {
    fn default() -> Self {
        Self {
            storage: Vec::new(),
            len: 0,
            allocator: Arc::new(AssetIndexAllocator::default()),
        }
    }
}

impl<A: Asset> DenseAssetStorage<A> {
    /// Creates an empty storage whose backing vector is preallocated for
    /// `capacity` slots.
    fn with_capacity(capacity: usize) -> Self {
        Self {
            storage: Vec::with_capacity(capacity),
            len: 0,
            allocator: Arc::new(AssetIndexAllocator::default()),
        }
    }

    /// The number of slots that currently hold a value.
    fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether no slot currently holds a value.
    fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Inserts `asset` at `index`, returning `true` if a value was already there
    /// (and was replaced).
    fn insert(
        &mut self,
        index: AssetIndex,
        asset: A,
    ) -> Result<bool, InvalidGenerationError> {
        self.flush();
        let entry = match self.storage.get_mut(index.index() as usize) {
            Some(entry) => entry,
            None => return Err(InvalidGenerationError::Removed { index }),
        };
        if let Entry::Some { value, generation } = entry {
            if *generation == index.generation() {
                let exists = value.is_some();
                if !exists {
                    self.len += 1;
                }
                *value = Some(asset);
                Ok(exists)
            } else {
                Err(InvalidGenerationError::Occupied {
                    index,
                    current_generation: *generation,
                })
            }
        } else {
            Err(InvalidGenerationError::Removed { index })
        }
    }

    /// Removes the value at `index`, recycling the slot so a later
    /// [`AssetIndexAllocator::reserve`] can reuse it.
    fn remove_dropped(&mut self, index: AssetIndex) -> Option<A> {
        self.remove_internal(index, |dense_storage| {
            dense_storage.storage[index.index() as usize] = Entry::None;
            dense_storage.allocator.recycle(index);
        })
    }

    /// Removes the value at `index` without recycling the slot. A value with the
    /// same index can still be inserted; the slot is only returned to the
    /// allocator by [`DenseAssetStorage::remove_dropped`].
    fn remove_still_alive(&mut self, index: AssetIndex) -> Option<A> {
        self.remove_internal(index, |_| {})
    }

    fn remove_internal(
        &mut self,
        index: AssetIndex,
        removed_action: impl FnOnce(&mut Self),
    ) -> Option<A> {
        self.flush();
        let value = match self.storage.get_mut(index.index() as usize)? {
            Entry::None => return None,
            Entry::Some { value, generation } => {
                if *generation == index.generation() {
                    value.take().inspect(|_| self.len -= 1)
                } else {
                    return None;
                }
            }
        };
        removed_action(self);
        value
    }

    fn get(&self, index: AssetIndex) -> Option<&A> {
        match self.storage.get(index.index() as usize)? {
            Entry::None => None,
            Entry::Some { value, generation } => {
                if *generation == index.generation() {
                    value.as_ref()
                } else {
                    None
                }
            }
        }
    }

    fn get_mut(&mut self, index: AssetIndex) -> Option<&mut A> {
        match self.storage.get_mut(index.index() as usize)? {
            Entry::None => None,
            Entry::Some { value, generation } => {
                if *generation == index.generation() {
                    value.as_mut()
                } else {
                    None
                }
            }
        }
    }

    /// Grows the backing vector to cover every slot the allocator has handed out,
    /// and re-opens recycled slots at the generation they were last reserved at.
    fn flush(&mut self) {
        // The allocator's index is monotonically increasing, so this only ever
        // grows the vector.
        let new_len = self.allocator.next_index();
        self.storage.resize_with(new_len as usize, || Entry::Some {
            value: None,
            generation: 0,
        });
        while let Some(recycled) = self.allocator.try_recv_recycled() {
            self.storage[recycled.index() as usize] = Entry::Some {
                value: None,
                generation: recycled.generation(),
            };
        }
    }

    fn get_index_allocator(&self) -> Arc<AssetIndexAllocator> {
        self.allocator.clone()
    }

    fn ids(&self) -> impl Iterator<Item = AssetId<A>> + '_ {
        self.storage
            .iter()
            .enumerate()
            .filter_map(|(slot, entry)| match entry {
                Entry::None => None,
                Entry::Some { value, generation } => value.as_ref().map(|_| {
                    AssetId::from(AssetIndex {
                        index: slot as u32,
                        generation: *generation,
                    })
                }),
            })
    }
}

/// The collection of loaded values for one asset type, addressed by handle or id.
///
/// There is exactly one `Assets<A>` per asset type, registered as a world
/// resource. Values registered by slot live in a dense generational storage;
/// values registered by UUID live in a hash map.
///
/// The store tracks changes and queues an [`AssetEvent`] for each one. Events
/// queued here are flushed to `Messages<AssetEvent<A>>` by [`Assets::asset_events`].
#[derive(Resource)]
pub struct Assets<A: Asset> {
    dense_storage: DenseAssetStorage<A>,
    hash_map: HashMap<Uuid, A>,
    handle_provider: AssetHandleProvider,
    /// Events accumulated since the last [`Assets::asset_events`] flush.
    queued_events: Vec<AssetEvent<A>>,
    /// Extra strong handles handed out by [`Assets::get_strong_handle`] for a
    /// slot. While one exists, a drop event for that slot must not release the
    /// value, because the primary handle may already have been consumed.
    duplicate_handles: HashMap<AssetIndex, u16>,
}

impl<A: Asset> Default for Assets<A> {
    fn default() -> Self {
        Self::with_capacity(0)
    }
}

impl<A: Asset> Assets<A> {
    /// Creates an empty store with room for `capacity` assets.
    ///
    /// This is a real preallocation knob (an additive deviation from `bevy_asset`,
    /// where `Assets::with_capacity` is not part of the public surface): the dense
    /// storage, UUID map and queues are all reserved up front.
    pub fn with_capacity(capacity: usize) -> Self {
        let dense_storage = DenseAssetStorage::with_capacity(capacity);
        let handle_provider =
            AssetHandleProvider::new(TypeId::of::<A>(), dense_storage.get_index_allocator());
        Self {
            dense_storage,
            handle_provider,
            hash_map: HashMap::with_capacity(capacity),
            queued_events: Vec::with_capacity(capacity),
            duplicate_handles: HashMap::with_capacity(capacity),
        }
    }

    /// Retrieves a handle provider capable of reserving new [`Handle`] values for
    /// assets stored in this collection.
    pub fn get_handle_provider(&self) -> AssetHandleProvider {
        self.handle_provider.clone()
    }

    /// Reserves a new strong [`Handle`] for an asset that will be stored in this
    /// collection.
    pub fn reserve_handle(&self) -> Handle<A> {
        self.handle_provider.reserve_handle().typed::<A>()
    }

    /// Inserts `asset`, identified by `id`. If a value already exists for `id`,
    /// it is replaced.
    ///
    /// This never returns an error for [`AssetId::Uuid`] ids.
    pub fn insert(
        &mut self,
        id: impl Into<AssetId<A>>,
        asset: A,
    ) -> Result<(), InvalidGenerationError> {
        match id.into() {
            AssetId::Index { index, .. } => self.insert_with_index(index, asset).map(|_| ()),
            AssetId::Uuid { uuid } => {
                self.insert_with_uuid(uuid, asset);
                Ok(())
            }
        }
    }

    /// Retrieves the [`AssetMut`] stored for `id`, inserting it with `insert_fn`
    /// first if it is absent.
    ///
    /// This never returns an error for [`AssetId::Uuid`] ids.
    pub fn get_or_insert_with(
        &mut self,
        id: impl Into<AssetId<A>>,
        insert_fn: impl FnOnce() -> A,
    ) -> Result<AssetMut<'_, A>, InvalidGenerationError> {
        let id: AssetId<A> = id.into();
        if self.get(id).is_none() {
            self.insert(id, insert_fn())?;
        }
        // Unreachable as an error: either `get` was `Some`, or we just inserted.
        Ok(self
            .get_mut(id)
            .expect("the asset was none even though we checked or inserted"))
    }

    /// Whether `id` exists in this collection.
    pub fn contains(&self, id: impl Into<AssetId<A>>) -> bool {
        match id.into() {
            AssetId::Index { index, .. } => self.dense_storage.get(index).is_some(),
            AssetId::Uuid { uuid } => self.hash_map.contains_key(&uuid),
        }
    }

    pub(crate) fn insert_with_uuid(&mut self, uuid: Uuid, asset: A) -> Option<A> {
        let result = self.hash_map.insert(uuid, asset);
        if result.is_some() {
            self.queued_events
                .push(AssetEvent::Modified { id: uuid.into() });
        } else {
            self.queued_events
                .push(AssetEvent::Added { id: uuid.into() });
        }
        result
    }

    pub(crate) fn insert_with_index(
        &mut self,
        index: AssetIndex,
        asset: A,
    ) -> Result<bool, InvalidGenerationError> {
        let replaced = self.dense_storage.insert(index, asset)?;
        if replaced {
            self.queued_events
                .push(AssetEvent::Modified { id: index.into() });
        } else {
            self.queued_events
                .push(AssetEvent::Added { id: index.into() });
        }
        Ok(replaced)
    }

    /// Adds `asset` and allocates a new strong [`Handle`] for it.
    pub fn add(&mut self, asset: impl Into<A>) -> Handle<A> {
        let index = self.dense_storage.allocator.reserve();
        self.insert_with_index(index, asset.into())
            .expect("a freshly reserved index is always insertable");
        Handle::Strong(self.handle_provider.get_handle(index))
    }

    /// Upgrades an [`AssetId`] into a strong [`Handle`] that keeps the asset
    /// alive.
    ///
    /// Returns `None` if `id` is not part of this collection (for example, the
    /// asset has already been dropped), or if it is a [`AssetId::Uuid`] (strong
    /// handles are only handed out for slot-based assets).
    pub fn get_strong_handle(&mut self, id: AssetId<A>) -> Option<Handle<A>> {
        if !self.contains(id) {
            return None;
        }
        let index = match id {
            AssetId::Index { index, .. } => index,
            AssetId::Uuid { .. } => return None,
        };
        *self.duplicate_handles.entry(index).or_insert(0) += 1;
        Some(Handle::Strong(self.handle_provider.get_handle(index)))
    }

    /// Retrieves a reference to the asset with `id`, if it exists.
    pub fn get(&self, id: impl Into<AssetId<A>>) -> Option<&A> {
        match id.into() {
            AssetId::Index { index, .. } => self.dense_storage.get(index),
            AssetId::Uuid { uuid } => self.hash_map.get(&uuid),
        }
    }

    /// Retrieves a mutable reference to the asset with `id`, if it exists.
    ///
    /// The returned [`AssetMut`] queues an [`AssetEvent::Modified`] when it is
    /// mutably dereferenced, so mutating through it notifies downstream systems
    /// even if the value ends up unchanged. Use [`Assets::get_mut_untracked`] to
    /// mutate without notifying.
    pub fn get_mut(&mut self, id: impl Into<AssetId<A>>) -> Option<AssetMut<'_, A>> {
        let id: AssetId<A> = id.into();
        let result = match id {
            AssetId::Index { index, .. } => self.dense_storage.get_mut(index),
            AssetId::Uuid { uuid } => self.hash_map.get_mut(&uuid),
        };
        Some(AssetMut {
            asset: result?,
            guard: AssetMutChangeNotifier {
                changed: false,
                asset_id: id,
                queued_events: &mut self.queued_events,
            },
        })
    }

    /// Retrieves a mutable reference to the asset with `id`, if it exists.
    ///
    /// The same as [`Assets::get_mut`], except it never queues an
    /// [`AssetEvent::Modified`].
    pub fn get_mut_untracked(&mut self, id: impl Into<AssetId<A>>) -> Option<&mut A> {
        match id.into() {
            AssetId::Index { index, .. } => self.dense_storage.get_mut(index),
            AssetId::Uuid { uuid } => self.hash_map.get_mut(&uuid),
        }
    }

    /// Removes (and returns) the asset with `id`, if it exists, queuing
    /// [`AssetEvent::Removed`].
    pub fn remove(&mut self, id: impl Into<AssetId<A>>) -> Option<A> {
        let id: AssetId<A> = id.into();
        let result = self.remove_untracked(id);
        if result.is_some() {
            self.queued_events.push(AssetEvent::Removed { id });
        }
        result
    }

    /// Removes (and returns) the asset with `id`, if it exists, without queuing
    /// [`AssetEvent::Removed`].
    pub fn remove_untracked(&mut self, id: impl Into<AssetId<A>>) -> Option<A> {
        match id.into() {
            AssetId::Index { index, .. } => {
                self.duplicate_handles.remove(&index);
                self.dense_storage.remove_still_alive(index)
            }
            AssetId::Uuid { uuid } => self.hash_map.remove(&uuid),
        }
    }

    /// Releases the value whose last strong handle was dropped.
    ///
    /// Always queues [`AssetEvent::Unused`], and [`AssetEvent::Removed`] as well
    /// when a value was actually present — in that order.
    pub(crate) fn remove_dropped(&mut self, index: AssetIndex) {
        match self.duplicate_handles.get_mut(&index) {
            None => {}
            Some(0) => {
                self.duplicate_handles.remove(&index);
            }
            Some(count) => {
                *count -= 1;
                return;
            }
        }

        let existed = self.dense_storage.remove_dropped(index).is_some();

        self.queued_events
            .push(AssetEvent::Unused { id: index.into() });
        if existed {
            self.queued_events
                .push(AssetEvent::Removed { id: index.into() });
        }
    }

    /// Drains the handle provider's drop channel, releasing every asset whose last
    /// strong handle has been dropped since the last call.
    ///
    /// Always queues [`AssetEvent::Unused`] and, when a value was actually present,
    /// [`AssetEvent::Removed`] as well — in that order.
    pub(crate) fn drain_dropped_assets(&mut self) {
        let drop_receiver = self.handle_provider.drop_receiver();
        while let Ok(drop_event) = drop_receiver.try_recv() {
            self.remove_dropped(drop_event.index());
        }
    }

    /// The per-type driver system that releases assets whose last strong handle
    /// was dropped since it last ran.
    pub fn track_assets(mut assets: ResMut<Self>) {
        assets.drain_dropped_assets();
    }

    /// Flushes queued [`AssetEvent`]s to `Messages<AssetEvent<A>>`.
    pub fn asset_events(mut assets: ResMut<Self>, mut messages: MessageWriter<AssetEvent<A>>) {
        messages.write_batch(assets.queued_events.drain(..));
    }

    /// A run condition for [`Assets::asset_events`]: it returns `false` when
    /// there is nothing to flush.
    pub fn asset_events_condition(assets: Res<Self>) -> bool {
        !assets.queued_events.is_empty()
    }

    /// Whether the collection holds no values.
    pub fn is_empty(&self) -> bool {
        self.dense_storage.is_empty() && self.hash_map.is_empty()
    }

    /// The number of values currently stored.
    pub fn len(&self) -> usize {
        self.dense_storage.len() + self.hash_map.len()
    }

    /// Iterates over the [`AssetId`] of every stored value.
    pub fn ids(&self) -> impl Iterator<Item = AssetId<A>> + '_ {
        self.dense_storage
            .ids()
            .chain(self.hash_map.keys().map(|uuid| AssetId::from(*uuid)))
    }

    /// Iterates over the [`AssetId`] and value of every stored asset.
    pub fn iter(&self) -> impl Iterator<Item = (AssetId<A>, &A)> {
        self.dense_storage
            .storage
            .iter()
            .enumerate()
            .filter_map(|(slot, entry)| match entry {
                Entry::None => None,
                Entry::Some { value, generation } => value.as_ref().map(|value| {
                    let id = AssetId::Index {
                        index: AssetIndex {
                            index: slot as u32,
                            generation: *generation,
                        },
                        marker: PhantomData,
                    };
                    (id, value)
                }),
            })
            .chain(
                self.hash_map
                    .iter()
                    .map(|(uuid, value)| (AssetId::Uuid { uuid: *uuid }, value)),
            )
    }

    /// Iterates over the [`AssetId`] and mutable value of every stored asset,
    /// queuing [`AssetEvent::Modified`] for each one visited.
    pub fn iter_mut(&mut self) -> AssetsMutIterator<'_, A> {
        AssetsMutIterator {
            dense_storage: self.dense_storage.storage.iter_mut().enumerate(),
            hash_map: self.hash_map.iter_mut(),
            queued_events: &mut self.queued_events,
        }
    }

    /// The events queued but not yet flushed to `Messages`.
    #[cfg(test)]
    pub(crate) fn queued_events(&self) -> &[AssetEvent<A>] {
        &self.queued_events
    }
}

impl<A: Asset> fmt::Debug for Assets<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Assets")
            .field("len", &self.len())
            .field("queued_events", &self.queued_events.len())
            .finish_non_exhaustive()
    }
}

/// A unique mutable borrow of an asset.
///
/// Mutably dereferencing it queues an [`AssetEvent::Modified`]. Methods that
/// explicitly opt out — [`AssetMut::into_inner_untracked`] and
/// [`AssetMut::bypass_change_detection`] — do not.
pub struct AssetMut<'a, A: Asset> {
    asset: &'a mut A,
    guard: AssetMutChangeNotifier<'a, A>,
}

impl<'a, A: Asset> AssetMut<'a, A> {
    /// Marks the asset as modified and returns a reference to it.
    pub fn into_inner(mut self) -> &'a mut A {
        self.guard.changed = true;
        self.asset
    }

    /// Returns a reference to the asset without marking it as modified.
    pub fn into_inner_untracked(self) -> &'a mut A {
        self.asset
    }

    /// Manually bypasses change detection, allowing the underlying value to be
    /// mutated without queuing [`AssetEvent::Modified`].
    ///
    /// # Warning
    ///
    /// This can have unexpected consequences for anything relying on change
    /// detection. Prefer [`Assets::get_mut_untracked`] when the intent is simply
    /// "do not track this mutation".
    pub fn bypass_change_detection(&mut self) -> &mut A {
        self.asset
    }
}

impl<A: Asset> Deref for AssetMut<'_, A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        self.asset
    }
}

impl<A: Asset> DerefMut for AssetMut<'_, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.changed = true;
        self.asset
    }
}

/// Queues [`AssetEvent::Modified`] on drop when the value was mutably
/// dereferenced. Split out of [`AssetMut`] so the reference can be moved out of
/// it while change tracking stays sound.
struct AssetMutChangeNotifier<'a, A: Asset> {
    changed: bool,
    asset_id: AssetId<A>,
    queued_events: &'a mut Vec<AssetEvent<A>>,
}

impl<A: Asset> Drop for AssetMutChangeNotifier<'_, A> {
    fn drop(&mut self) {
        if self.changed {
            self.queued_events
                .push(AssetEvent::Modified { id: self.asset_id });
        }
    }
}

/// A mutable iterator over [`Assets`], as returned by [`Assets::iter_mut`].
pub struct AssetsMutIterator<'a, A: Asset> {
    queued_events: &'a mut Vec<AssetEvent<A>>,
    dense_storage: Enumerate<IterMut<'a, Entry<A>>>,
    hash_map: std::collections::hash_map::IterMut<'a, Uuid, A>,
}

impl<'a, A: Asset> Iterator for AssetsMutIterator<'a, A> {
    type Item = (AssetId<A>, &'a mut A);

    fn next(&mut self) -> Option<Self::Item> {
        for (slot, entry) in &mut self.dense_storage {
            match entry {
                Entry::None => continue,
                Entry::Some { value, generation } => {
                    let id = AssetId::Index {
                        index: AssetIndex {
                            index: slot as u32,
                            generation: *generation,
                        },
                        marker: PhantomData,
                    };
                    self.queued_events.push(AssetEvent::Modified { id });
                    if let Some(value) = value {
                        return Some((id, value));
                    }
                }
            }
        }
        if let Some((uuid, value)) = self.hash_map.next() {
            let id = AssetId::Uuid { uuid: *uuid };
            self.queued_events.push(AssetEvent::Modified { id });
            Some((id, value))
        } else {
            None
        }
    }
}

/// The error returned when an [`AssetIndex`] cannot be inserted at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidGenerationError {
    /// A value with a different generation currently occupies the slot.
    Occupied {
        /// The index that was requested.
        index: AssetIndex,
        /// The generation currently stored in the slot.
        current_generation: u32,
    },
    /// The slot has been removed and is no longer insertable.
    Removed {
        /// The index that was requested.
        index: AssetIndex,
    },
}

impl fmt::Display for InvalidGenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvalidGenerationError::Occupied {
                index,
                current_generation,
            } => write!(
                f,
                "AssetIndex {index:?} has an invalid generation. The current generation is '{current_generation}'."
            ),
            InvalidGenerationError::Removed { index } => {
                write!(f, "AssetIndex {index:?} has been removed")
            }
        }
    }
}

impl std::error::Error for InvalidGenerationError {}
