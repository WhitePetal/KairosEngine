//! Handles: reference-counted borrows of a stored asset.
//!
//! A [`Handle<A>`] is either [`Handle::Strong`] — keeping the asset alive while
//! any clone exists — or [`Handle::Uuid`], the weak form, which only names the
//! asset. Strong handles share an [`Arc<StrongHandle>`], so the reference count
//! *is* the `Arc` strong count and the last clone to drop sends a [`DropEvent`]
//! down a [`crossbeam_channel`] that the owning asset store drains.

use core::{
    any::TypeId,
    fmt::{self, Debug},
    hash::{Hash, Hasher},
    marker::PhantomData,
};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use uuid::Uuid;

use crate::asset::Asset;
use crate::id::{AssetId, DEFAULT_UUID, UntypedAssetId};
use crate::index::{AssetIndex, AssetIndexAllocator};

/// Announces that the last strong handle to an asset was dropped.
///
/// The event carries the erased index of the freed slot; the asset store uses
/// it to release (and recycle) the stored value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DropEvent {
    pub(crate) index: AssetIndex,
    pub(crate) type_id: TypeId,
}

impl DropEvent {
    /// The slot whose last strong handle was dropped.
    #[inline]
    pub fn index(&self) -> AssetIndex {
        self.index
    }

    /// The asset type the dropped handle named.
    #[inline]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }
}

/// The shared storage behind every [`Handle::Strong`] clone of one asset.
///
/// Its [`Drop`] is the reference count hitting zero: it sends the [`DropEvent`]
/// that tells the asset store the value is no longer needed.
#[derive(Debug)]
pub struct StrongHandle {
    pub(crate) index: AssetIndex,
    pub(crate) type_id: TypeId,
    pub(crate) drop_sender: Sender<DropEvent>,
}

impl Drop for StrongHandle {
    fn drop(&mut self) {
        let _ = self.drop_sender.send(DropEvent {
            index: self.index,
            type_id: self.type_id,
        });
    }
}

/// Hands out strong handles for one asset type and owns the drop channel they
/// report to.
#[derive(Clone)]
pub struct AssetHandleProvider {
    allocator: Arc<AssetIndexAllocator>,
    drop_sender: Sender<DropEvent>,
    drop_receiver: Receiver<DropEvent>,
    type_id: TypeId,
}

impl AssetHandleProvider {
    /// Creates a provider for `type_id`, allocating indices from `allocator`.
    pub fn new(type_id: TypeId, allocator: Arc<AssetIndexAllocator>) -> Self {
        let (drop_sender, drop_receiver) = crossbeam_channel::unbounded();
        Self {
            type_id,
            allocator,
            drop_sender,
            drop_receiver,
        }
    }

    /// Reserves a fresh strong [`UntypedHandle`], allocating a new index.
    pub fn reserve_handle(&self) -> UntypedHandle {
        let index = self.allocator.reserve();
        UntypedHandle::Strong(self.get_handle(index))
    }

    /// Wraps an already-reserved index in a strong handle.
    pub(crate) fn get_handle(&self, index: AssetIndex) -> Arc<StrongHandle> {
        Arc::new(StrongHandle {
            index,
            type_id: self.type_id,
            drop_sender: self.drop_sender.clone(),
        })
    }

    /// The receiving half of the drop channel. The asset store drains this to
    /// learn which assets have lost their last strong handle.
    pub fn drop_receiver(&self) -> Receiver<DropEvent> {
        self.drop_receiver.clone()
    }

    /// The asset type this provider hands out handles for.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }
}

/// A reference to an [`Asset`] of type `A`.
///
/// A [`Handle::Strong`] keeps its asset alive until every clone is dropped; a
/// [`Handle::Uuid`] does not. Cloning a handle clones the reference, so the
/// asset is freed only once the last clone goes away.
pub enum Handle<A: Asset> {
    /// A live (or loading) asset, kept alive while this handle lives.
    Strong(Arc<StrongHandle>),
    /// The weak form: a stable-across-runs identifier that neither keeps the
    /// asset alive nor guarantees it exists.
    Uuid(Uuid, PhantomData<fn() -> A>),
}

impl<A: Asset> Handle<A> {
    /// The [`AssetId`] this handle names.
    #[inline]
    pub fn id(&self) -> AssetId<A> {
        match self {
            Handle::Strong(handle) => AssetId::Index {
                index: handle.index,
                marker: PhantomData,
            },
            Handle::Uuid(uuid, _) => AssetId::Uuid { uuid: *uuid },
        }
    }

    /// Whether this is a [`Handle::Uuid`].
    #[inline]
    pub fn is_uuid(&self) -> bool {
        matches!(self, Handle::Uuid(..))
    }

    /// Whether this is a [`Handle::Strong`].
    #[inline]
    pub fn is_strong(&self) -> bool {
        matches!(self, Handle::Strong(_))
    }

    /// Erases the asset type, recording it in the resulting [`UntypedHandle`].
    #[inline]
    pub fn untyped(self) -> UntypedHandle {
        self.into()
    }
}

impl<A: Asset> Clone for Handle<A> {
    fn clone(&self) -> Self {
        match self {
            Handle::Strong(handle) => Handle::Strong(handle.clone()),
            Handle::Uuid(uuid, _) => Handle::Uuid(*uuid, PhantomData),
        }
    }
}

impl<A: Asset> Default for Handle<A> {
    fn default() -> Self {
        Handle::Uuid(DEFAULT_UUID, PhantomData)
    }
}

impl<A: Asset> Debug for Handle<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = core::any::type_name::<A>();
        match self {
            Handle::Strong(handle) => write!(
                f,
                "StrongHandle<{name}>{{ index: {:?}, type_id: {:?} }}",
                handle.index, handle.type_id
            ),
            Handle::Uuid(uuid, _) => write!(f, "UuidHandle<{name}>({uuid})"),
        }
    }
}

// Handles compare, order and hash purely by their id, so a `HashMap<Handle, _>`
// behaves like a `HashMap<AssetId, _>`.
impl<A: Asset> Hash for Handle<A> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

impl<A: Asset> PartialEq for Handle<A> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl<A: Asset> Eq for Handle<A> {}

impl<A: Asset> PartialOrd for Handle<A> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<A: Asset> Ord for Handle<A> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id().cmp(&other.id())
    }
}

impl<A: Asset> From<&Handle<A>> for AssetId<A> {
    #[inline]
    fn from(value: &Handle<A>) -> Self {
        value.id()
    }
}

impl<A: Asset> From<&Handle<A>> for UntypedAssetId {
    #[inline]
    fn from(value: &Handle<A>) -> Self {
        value.id().into()
    }
}

impl<A: Asset> From<&mut Handle<A>> for AssetId<A> {
    #[inline]
    fn from(value: &mut Handle<A>) -> Self {
        value.id()
    }
}

impl<A: Asset> From<&mut Handle<A>> for UntypedAssetId {
    #[inline]
    fn from(value: &mut Handle<A>) -> Self {
        value.id().into()
    }
}

impl<A: Asset> From<Uuid> for Handle<A> {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Handle::Uuid(uuid, PhantomData)
    }
}

/// A [`Handle`] with its asset type erased into runtime [`TypeId`]
/// information, so handles of different asset types can be stored and compared
/// together.
#[derive(Clone)]
pub enum UntypedHandle {
    /// A strong handle, keeping the asset alive while any clone lives.
    Strong(Arc<StrongHandle>),
    /// A UUID handle, which does not keep the asset alive.
    Uuid {
        /// The asset type this handle names.
        type_id: TypeId,
        /// The registered UUID.
        uuid: Uuid,
    },
}

impl UntypedHandle {
    /// The default handle for an asset type, mirroring [`Handle::default`].
    pub fn default_for_type(type_id: TypeId) -> Self {
        Self::Uuid {
            type_id,
            uuid: DEFAULT_UUID,
        }
    }

    /// The [`UntypedAssetId`] this handle names.
    #[inline]
    pub fn id(&self) -> UntypedAssetId {
        match self {
            UntypedHandle::Strong(handle) => UntypedAssetId::Index {
                type_id: handle.type_id,
                index: handle.index,
            },
            UntypedHandle::Uuid { type_id, uuid } => UntypedAssetId::Uuid {
                type_id: *type_id,
                uuid: *uuid,
            },
        }
    }

    /// The asset type this handle names.
    #[inline]
    pub fn type_id(&self) -> TypeId {
        match self {
            UntypedHandle::Strong(handle) => handle.type_id,
            UntypedHandle::Uuid { type_id, .. } => *type_id,
        }
    }

    /// Converts to a typed [`Handle`] without checking the recorded type.
    #[inline]
    pub fn typed_unchecked<A: Asset>(self) -> Handle<A> {
        match self {
            UntypedHandle::Strong(handle) => Handle::Strong(handle),
            UntypedHandle::Uuid { uuid, .. } => Handle::Uuid(uuid, PhantomData),
        }
    }

    /// Converts to a typed [`Handle`], checking the recorded type in debug
    /// builds only.
    #[inline]
    pub fn typed_debug_checked<A: Asset>(self) -> Handle<A> {
        debug_assert_eq!(
            self.type_id(),
            TypeId::of::<A>(),
            "the target Handle<{}>'s TypeId does not match the TypeId of this UntypedHandle",
            core::any::type_name::<A>()
        );
        self.typed_unchecked()
    }

    /// Converts to a typed [`Handle`].
    ///
    /// # Panics
    ///
    /// Panics if the recorded type does not match `A`.
    #[inline]
    pub fn typed<A: Asset>(self) -> Handle<A> {
        let Ok(handle) = self.try_typed() else {
            panic!(
                "the target Handle<{}>'s TypeId does not match the TypeId of this UntypedHandle",
                core::any::type_name::<A>()
            )
        };
        handle
    }

    /// Converts to a typed [`Handle`] if the recorded type matches `A`.
    #[inline]
    pub fn try_typed<A: Asset>(self) -> Result<Handle<A>, UntypedAssetConversionError> {
        Handle::try_from(self)
    }
}

impl Debug for UntypedHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UntypedHandle::Strong(handle) => write!(
                f,
                "StrongHandle{{ type_id: {:?}, index: {:?} }}",
                handle.type_id, handle.index
            ),
            UntypedHandle::Uuid { type_id, uuid } => {
                write!(f, "UuidHandle{{ type_id: {type_id:?}, uuid: {uuid} }}")
            }
        }
    }
}

impl PartialEq for UntypedHandle {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id() && self.type_id() == other.type_id()
    }
}

impl Eq for UntypedHandle {}

impl Hash for UntypedHandle {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

impl PartialOrd for UntypedHandle {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        if self.type_id() == other.type_id() {
            self.id().partial_cmp(&other.id())
        } else {
            None
        }
    }
}

impl From<&UntypedHandle> for UntypedAssetId {
    #[inline]
    fn from(value: &UntypedHandle) -> Self {
        value.id()
    }
}

// Cross operations between typed and untyped handles.

impl<A: Asset> PartialEq<UntypedHandle> for Handle<A> {
    #[inline]
    fn eq(&self, other: &UntypedHandle) -> bool {
        TypeId::of::<A>() == other.type_id() && self.id() == other.id()
    }
}

impl<A: Asset> PartialEq<Handle<A>> for UntypedHandle {
    #[inline]
    fn eq(&self, other: &Handle<A>) -> bool {
        other == self
    }
}

impl<A: Asset> PartialOrd<UntypedHandle> for Handle<A> {
    fn partial_cmp(&self, other: &UntypedHandle) -> Option<core::cmp::Ordering> {
        if TypeId::of::<A>() != other.type_id() {
            None
        } else {
            self.id().partial_cmp(&other.id())
        }
    }
}

impl<A: Asset> PartialOrd<Handle<A>> for UntypedHandle {
    fn partial_cmp(&self, other: &Handle<A>) -> Option<core::cmp::Ordering> {
        Some(other.partial_cmp(self)?.reverse())
    }
}

impl<A: Asset> From<Handle<A>> for UntypedHandle {
    fn from(value: Handle<A>) -> Self {
        match value {
            Handle::Strong(handle) => UntypedHandle::Strong(handle),
            Handle::Uuid(uuid, _) => UntypedHandle::Uuid {
                type_id: TypeId::of::<A>(),
                uuid,
            },
        }
    }
}

impl<A: Asset> TryFrom<UntypedHandle> for Handle<A> {
    type Error = UntypedAssetConversionError;

    fn try_from(value: UntypedHandle) -> Result<Self, Self::Error> {
        let found = value.type_id();
        let expected = TypeId::of::<A>();

        if found != expected {
            return Err(UntypedAssetConversionError::TypeIdMismatch { expected, found });
        }

        Ok(match value {
            UntypedHandle::Strong(handle) => Handle::Strong(handle),
            UntypedHandle::Uuid { uuid, .. } => Handle::Uuid(uuid, PhantomData),
        })
    }
}

/// The error returned when an [`UntypedHandle`] is converted to the wrong typed
/// [`Handle`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UntypedAssetConversionError {
    /// The recorded [`TypeId`] does not match the target asset type.
    TypeIdMismatch {
        /// The [`TypeId`] of the asset type being converted to.
        expected: TypeId,
        /// The [`TypeId`] recorded in the untyped handle.
        found: TypeId,
    },
}

impl fmt::Display for UntypedAssetConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypeIdMismatch { expected, found } => write!(
                f,
                "this UntypedHandle is for {found:?} and cannot be converted into a Handle<{expected:?}>"
            ),
        }
    }
}

impl std::error::Error for UntypedAssetConversionError {}
