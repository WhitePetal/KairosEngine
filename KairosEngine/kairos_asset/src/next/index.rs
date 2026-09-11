//! Generational asset indices and the allocator that hands them out.
//!
//! An [`AssetIndex`] is the runtime identity of one stored asset: a dense slot
//! number plus a generation that is bumped every time the slot is recycled. The
//! generation is what makes a stale reference (a handle to an asset that has
//! since been dropped) fail to resolve instead of silently aliasing whichever
//! asset later occupied the slot.

use core::sync::atomic::{AtomicU32, Ordering};

use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};

/// A generational runtime-only identifier for a single stored asset.
///
/// Indices are cheap to copy and are not tied to the lifetime of the asset, so
/// an index can outlive the value it named. See [`crate::next::AssetId`] for the
/// typed identifier built on top of it.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct AssetIndex {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl AssetIndex {
    /// The dense slot number of the asset.
    #[inline]
    pub fn index(&self) -> u32 {
        self.index
    }

    /// How many times this slot has been recycled.
    #[inline]
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Packs the index into an opaque `u64`, for transport across boundaries
    /// that cannot carry the strongly typed value.
    ///
    /// The result is only meaningful as input to [`AssetIndex::from_bits`]; do
    /// not read anything into the numeric value itself.
    #[inline]
    pub fn to_bits(self) -> u64 {
        ((self.generation as u64) << 32) | self.index as u64
    }

    /// Unpacks a value produced by [`AssetIndex::to_bits`].
    #[inline]
    pub fn from_bits(bits: u64) -> Self {
        Self {
            index: bits as u32,
            generation: (bits >> 32) as u32,
        }
    }
}

/// Hands out [`AssetIndex`] values and recycles them for reuse.
///
/// Fresh slots come from a monotonically increasing counter; released slots are
/// pushed onto a queue and returned with their generation bumped, so a recycled
/// slot never compares equal to the index that previously named it.
pub struct AssetIndexAllocator {
    next_index: AtomicU32,
    recycled_queue_sender: Sender<AssetIndex>,
    recycled_queue_receiver: Receiver<AssetIndex>,
}

impl Default for AssetIndexAllocator {
    fn default() -> Self {
        let (recycled_queue_sender, recycled_queue_receiver) = crossbeam_channel::unbounded();
        Self {
            next_index: AtomicU32::new(0),
            recycled_queue_sender,
            recycled_queue_receiver,
        }
    }
}

impl AssetIndexAllocator {
    /// Reserves an [`AssetIndex`], reusing a recycled slot when one is
    /// available and allocating a fresh slot otherwise.
    pub fn reserve(&self) -> AssetIndex {
        if let Ok(mut recycled) = self.recycled_queue_receiver.try_recv() {
            recycled.generation += 1;
            recycled
        } else {
            AssetIndex {
                index: self.next_index.fetch_add(1, Ordering::Relaxed),
                generation: 0,
            }
        }
    }

    /// Returns `index` to the pool so a later [`AssetIndexAllocator::reserve`]
    /// can reuse its slot with a bumped generation.
    ///
    /// Only recycle an index once nothing refers to it any more.
    pub fn recycle(&self, index: AssetIndex) {
        // The queue is unbounded, so this only fails if the receiver has been
        // dropped — which cannot happen while the allocator is alive.
        let _ = self.recycled_queue_sender.send(index);
    }
}
