//! The asset event face.
//!
//! [`AssetEvent<A>`] is the per-type notification a store emits when one of its
//! values is added, modified, removed, released (unused), or finishes loading
//! together with all of its dependencies. It is a [`Message`], so consumers read
//! it with a `MessageReader<AssetEvent<A>>` and the engine advances it once per
//! frame.

use core::fmt::{self, Debug};

use kairos_ecs::message::Message;

use crate::next::asset::Asset;
use crate::next::id::AssetId;

/// A notification that something happened to an [`Asset`] of type `A`.
///
/// `Added`/`Modified`/`Removed`/`Unused` are produced by the store itself; the
/// loader side emits `LoadedWithDependencies` once an asset and everything it
/// depends on are ready.
#[derive(Message)]
pub enum AssetEvent<A: Asset> {
    /// Emitted whenever an [`Asset`] is added.
    Added { id: AssetId<A> },
    /// Emitted whenever an [`Asset`] value is modified.
    Modified { id: AssetId<A> },
    /// Emitted whenever an [`Asset`] is removed.
    Removed { id: AssetId<A> },
    /// Emitted when the last strong [`Handle`](crate::next::Handle) of an
    /// [`Asset`] is dropped.
    Unused { id: AssetId<A> },
    /// Emitted whenever an [`Asset`] has been fully loaded, including its
    /// dependencies and all recursive dependencies.
    LoadedWithDependencies { id: AssetId<A> },
}

impl<A: Asset> AssetEvent<A> {
    /// Returns `true` if this is [`AssetEvent::Added`] and matches `asset_id`.
    pub fn is_added(&self, asset_id: impl Into<AssetId<A>>) -> bool {
        matches!(self, AssetEvent::Added { id } if *id == asset_id.into())
    }

    /// Returns `true` if this is [`AssetEvent::Modified`] and matches `asset_id`.
    pub fn is_modified(&self, asset_id: impl Into<AssetId<A>>) -> bool {
        matches!(self, AssetEvent::Modified { id } if *id == asset_id.into())
    }

    /// Returns `true` if this is [`AssetEvent::Removed`] and matches `asset_id`.
    pub fn is_removed(&self, asset_id: impl Into<AssetId<A>>) -> bool {
        matches!(self, AssetEvent::Removed { id } if *id == asset_id.into())
    }

    /// Returns `true` if this is [`AssetEvent::Unused`] and matches `asset_id`.
    pub fn is_unused(&self, asset_id: impl Into<AssetId<A>>) -> bool {
        matches!(self, AssetEvent::Unused { id } if *id == asset_id.into())
    }

    /// Returns `true` if this is [`AssetEvent::LoadedWithDependencies`] and
    /// matches `asset_id`.
    pub fn is_loaded_with_dependencies(&self, asset_id: impl Into<AssetId<A>>) -> bool {
        matches!(self, AssetEvent::LoadedWithDependencies { id } if *id == asset_id.into())
    }
}

// `AssetId<A>` is `Copy`/`Debug`/`PartialEq` regardless of `A`, so these are
// implemented by hand rather than derived: a derive would wrongly require
// `A: Copy`/`A: Debug`/`A: PartialEq`.
impl<A: Asset> Clone for AssetEvent<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Asset> Copy for AssetEvent<A> {}

impl<A: Asset> Debug for AssetEvent<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Added { id } => f.debug_struct("Added").field("id", id).finish(),
            Self::Modified { id } => f.debug_struct("Modified").field("id", id).finish(),
            Self::Removed { id } => f.debug_struct("Removed").field("id", id).finish(),
            Self::Unused { id } => f.debug_struct("Unused").field("id", id).finish(),
            Self::LoadedWithDependencies { id } => f
                .debug_struct("LoadedWithDependencies")
                .field("id", id)
                .finish(),
        }
    }
}

impl<A: Asset> PartialEq for AssetEvent<A> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Added { id: left }, Self::Added { id: right })
            | (Self::Modified { id: left }, Self::Modified { id: right })
            | (Self::Removed { id: left }, Self::Removed { id: right })
            | (Self::Unused { id: left }, Self::Unused { id: right })
            | (
                Self::LoadedWithDependencies { id: left },
                Self::LoadedWithDependencies { id: right },
            ) => left == right,
            _ => false,
        }
    }
}

impl<A: Asset> Eq for AssetEvent<A> {}

/// A [`Message`] emitted when a specific [`Asset`] fails to load.
///
/// The loader that produced the failure is added by the loader ticket: this
/// carries only the identity of the failed asset for now, because the asset path
/// and load-error types land with the IO and loader layers.
#[derive(Message)]
pub struct AssetLoadFailedEvent<A: Asset> {
    /// The stable identifier of the asset that failed to load.
    pub id: AssetId<A>,
}

impl<A: Asset> AssetLoadFailedEvent<A> {
    /// Creates a failure event for the asset identified by `id`.
    pub fn new(id: impl Into<AssetId<A>>) -> Self {
        Self { id: id.into() }
    }
}

// `AssetId<A>` is `Copy`/`Debug` regardless of `A`; derive would add `A` bounds.
impl<A: Asset> Clone for AssetLoadFailedEvent<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Asset> Copy for AssetLoadFailedEvent<A> {}

impl<A: Asset> Debug for AssetLoadFailedEvent<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetLoadFailedEvent")
            .field("id", &self.id)
            .finish()
    }
}
