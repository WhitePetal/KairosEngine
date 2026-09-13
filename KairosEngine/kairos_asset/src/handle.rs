//! Handles: reference-counted borrows of a stored asset.
//!
//! A [`Handle<A>`] is either [`Handle::Strong`] — keeping the asset alive while
//! any clone exists — or [`Handle::Uuid`], the weak form, which only names the
//! asset. Strong handles share an [`Arc<StrongHandle>`], so the reference count
//! *is* the `Arc` strong count and the last clone to drop sends a `DropEvent`
//! down a [`crossbeam_channel`] that the owning asset store drains.

use core::{
    any::TypeId,
    fmt::{self, Debug},
    hash::{Hash, Hasher},
    marker::PhantomData,
};
use std::sync::{Arc, Mutex};

use crossbeam_channel::{Receiver, Sender};
use kairos_ecs::error::Result;
use kairos_ecs::template::{FromTemplate, SpecializeFromTemplate, Template, TemplateContext};
use thiserror::Error;
use uuid::Uuid;

use crate::asset::Asset;
use crate::assets::Assets;
use crate::id::{AssetId, DEFAULT_UUID, UntypedAssetId};
use crate::index::{AssetIndex, AssetIndexAllocator};
use crate::meta::MetaTransform;
use crate::path::AssetPath;
use crate::server::AssetServer;

/// Announces that the last strong handle to an asset was dropped.
///
/// The event carries the erased index of the freed slot; the asset store uses
/// it to release (and recycle) the stored value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DropEvent {
    pub(crate) index: AssetIndex,
    pub(crate) type_id: TypeId,
    /// Whether the asset is managed by the [`AssetServer`].
    ///
    /// A handle the server handed out reports `true`, so the drop also releases
    /// the server's [`AssetInfo`]; a handle from [`Assets::add`] reports
    /// `false`, so there is nothing server-side to release.
    ///
    /// [`AssetInfo`]: crate::server::AssetInfos
    pub(crate) asset_server_managed: bool,
}

impl DropEvent {
    /// The slot whose last strong handle was dropped.
    #[inline]
    pub(crate) fn index(&self) -> AssetIndex {
        self.index
    }

    /// The erased id of the asset whose last strong handle was dropped.
    #[inline]
    pub(crate) fn id(&self) -> UntypedAssetId {
        UntypedAssetId::Index {
            type_id: self.type_id,
            index: self.index,
        }
    }
}

/// The shared storage behind every [`Handle::Strong`] clone of one asset.
///
/// Its [`Drop`] is the reference count hitting zero: it sends the `DropEvent`
/// that tells the asset store the value is no longer needed.
///
/// It also carries the [`MetaTransform`] the asset was loaded with. It is stored
/// on the handle because it is configuration tied to the lifetime of one load
/// and it must be repeatable when the asset is hot-reloaded.
pub struct StrongHandle {
    pub(crate) index: AssetIndex,
    pub(crate) type_id: TypeId,
    /// Whether the asset is managed by the [`AssetServer`]: its drops are
    /// reported back so the server can clear its bookkeeping too.
    pub(crate) asset_server_managed: bool,
    /// The path the asset was loaded from, if it was loaded by path rather than
    /// added directly. Lets [`Handle::path`] recover it without the server.
    pub(crate) path: Option<AssetPath<'static>>,
    /// The settings override applied when the asset was loaded, replayed on
    /// reload.
    pub(crate) meta_transform: Option<MetaTransform>,
    pub(crate) drop_sender: Sender<DropEvent>,
}

// Hand-written because [`MetaTransform`] is a boxed closure and therefore not
// [`Debug`].
impl Debug for StrongHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StrongHandle")
            .field("index", &self.index)
            .field("type_id", &self.type_id)
            .field("asset_server_managed", &self.asset_server_managed)
            .field("path", &self.path)
            .field("drop_sender", &self.drop_sender)
            .finish_non_exhaustive()
    }
}

impl Drop for StrongHandle {
    fn drop(&mut self) {
        let _ = self.drop_sender.send(DropEvent {
            index: self.index,
            type_id: self.type_id,
            asset_server_managed: self.asset_server_managed,
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
        self.reserve_handle_internal(false, None, None)
    }

    /// [`AssetHandleProvider::reserve_handle`] with the server-managed flag, a
    /// path, and a [`MetaTransform`] attached so a hot-reload replays the same
    /// loader settings.
    pub(crate) fn reserve_handle_internal(
        &self,
        asset_server_managed: bool,
        path: Option<AssetPath<'static>>,
        meta_transform: Option<MetaTransform>,
    ) -> UntypedHandle {
        let index = self.allocator.reserve();
        UntypedHandle::Strong(self.get_handle(index, asset_server_managed, path, meta_transform))
    }

    /// Wraps an already-reserved index in a strong handle, recording whether the
    /// server manages it, the path it was loaded from, and the
    /// [`MetaTransform`] a hot-reload replays.
    pub(crate) fn get_handle(
        &self,
        index: AssetIndex,
        asset_server_managed: bool,
        path: Option<AssetPath<'static>>,
        meta_transform: Option<MetaTransform>,
    ) -> Arc<StrongHandle> {
        Arc::new(StrongHandle {
            index,
            type_id: self.type_id,
            asset_server_managed,
            path,
            meta_transform,
            drop_sender: self.drop_sender.clone(),
        })
    }

    /// The receiving half of the drop channel. The asset store drains this to
    /// learn which assets have lost their last strong handle.
    pub(crate) fn drop_receiver(&self) -> Receiver<DropEvent> {
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

    /// The path the asset was loaded from, if this is a [`Handle::Strong`] that
    /// has one.
    ///
    /// A [`Handle::Uuid`] never has a path, and neither does an asset that was
    /// added directly rather than loaded by path.
    #[inline]
    pub fn path(&self) -> Option<&AssetPath<'static>> {
        match self {
            Handle::Strong(handle) => handle.path.as_ref(),
            Handle::Uuid(..) => None,
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

// This enables `FromTemplate` specialization for [`Handle<A>`] using the
// ["auto trait specialization" trick](https://github.com/coolcatcoder/rust_techniques/issues/1).
//
// [`Handle<A>`] is [`Clone`] + [`Default`], so it would otherwise be covered by
// the blanket `impl<T: Clone + Default + Unpin> FromTemplate for T`. Writing a
// manual `Unpin` impl with an unsatisfiable where-clause makes `Handle<A>`
// `!Unpin` in practice, escaping the blanket impl so the manual one below wins.
impl<A: Asset> Unpin for Handle<A> where for<'a> [()]: SpecializeFromTemplate {}

impl<A: Asset> FromTemplate for Handle<A> {
    type Template = HandleTemplate<A>;
}

/// A [`Template`] that produces a [`Handle`].
///
/// When a type with a [`Handle<A>`] field derives [`FromTemplate`], that field's
/// template type becomes [`HandleTemplate<A>`], so template / scene definitions
/// can name assets without holding a runtime handle. A string literal assigned to
/// such a field converts through [`From`] into [`HandleTemplate::Path`].
///
/// Templates live at the template / scene layer: the asset's documentation layer
/// keeps storing paths, while runtime values keep storing handles.
pub enum HandleTemplate<A: Asset> {
    /// Creates a [`Handle`] by calling [`AssetServer::load`] on the given
    /// [`AssetPath`].
    Path(AssetPath<'static>),
    /// Creates a [`Handle`] by cloning the given [`Handle`] value.
    Handle(Handle<A>),
    /// Creates a [`Handle`] by adding an inline value to [`Assets<A>`]. The
    /// resulting handle is cached on the template and reused by future builds.
    ///
    /// This should generally be constructed using [`HandleTemplate::value`] or
    /// [`asset_value`].
    Value(ArcMutexValue<A>),
}

impl<A: Asset> HandleTemplate<A> {
    /// Creates a new [`HandleTemplate`] for the given `value`, so an asset can be
    /// defined "inline" in a template / scene.
    ///
    /// The `value` is added to [`Assets<A>`] on the first build; later builds
    /// reuse the resulting handle.
    pub fn value(value: impl Into<A>) -> Self {
        Self::Value(ArcMutexValue(Arc::new(Mutex::new(AssetOrHandle::Value(
            Some(value.into()),
        )))))
    }
}

/// Stores an `Arc<Mutex<AssetOrHandle<A>>>`.
///
/// Shared between clones of a [`HandleTemplate::Value`] so the handle produced on
/// the first build is cached for all of them.
pub struct ArcMutexValue<A: Asset>(Arc<Mutex<AssetOrHandle<A>>>);

impl<A: Asset> Clone for ArcMutexValue<A> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

enum AssetOrHandle<A: Asset> {
    Value(Option<A>),
    Handle(Handle<A>),
}

impl<A: Asset> Default for HandleTemplate<A> {
    fn default() -> Self {
        Self::Handle(Handle::default())
    }
}

impl<A: Asset> From<Handle<A>> for HandleTemplate<A> {
    fn from(handle: Handle<A>) -> Self {
        Self::Handle(handle)
    }
}

impl<I: Into<AssetPath<'static>>, A: Asset> From<I> for HandleTemplate<A> {
    fn from(path: I) -> Self {
        Self::Path(path.into())
    }
}

impl<A: Asset> Template for HandleTemplate<A> {
    type Output = Handle<A>;

    fn build_template(&self, context: &mut TemplateContext) -> Result<Self::Output> {
        match self {
            HandleTemplate::Path(path) => {
                let server = context.resource::<AssetServer>();
                Ok(server.load(path))
            }
            HandleTemplate::Handle(handle) => Ok(handle.clone()),
            HandleTemplate::Value(value) => {
                // This unwrap is ok. If another caller panicked while holding this
                // mutex, then the program is in an invalid state and this should
                // panic too.
                let mut value_or_handle = value.0.lock().unwrap();
                match &mut *value_or_handle {
                    AssetOrHandle::Value(value) => {
                        // This unwrap is ok because the private `AssetOrHandle`
                        // only holds `None` briefly, between `take` and the line
                        // below that replaces it with the cached handle.
                        let handle = context.resource_mut::<Assets<A>>().add(value.take().unwrap());
                        *value_or_handle = AssetOrHandle::Handle(handle.clone());
                        Ok(handle)
                    }
                    AssetOrHandle::Handle(handle) => Ok(handle.clone()),
                }
            }
        }
    }

    fn clone_template(&self) -> Self {
        match self {
            HandleTemplate::Path(path) => Self::Path(path.clone()),
            HandleTemplate::Handle(handle) => Self::Handle(handle.clone()),
            HandleTemplate::Value(value) => Self::Value(value.clone()),
        }
    }
}

/// Creates a new [`HandleTemplate`] for the given `asset` value, so an asset can
/// be defined "inline" in a template / scene.
///
/// This supports [`Into`] to automatically convert values that can become `A`.
pub fn asset_value<I: Into<A>, A: Asset>(asset: I) -> HandleTemplate<A> {
    HandleTemplate::value(asset)
}

impl<A: Asset> Debug for Handle<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = core::any::type_name::<A>();
        match self {
            Handle::Strong(handle) => write!(
                f,
                "StrongHandle<{name}>{{ index: {:?}, type_id: {:?}, path: {:?} }}",
                handle.index, handle.type_id, handle.path
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

/// Creates a [`Handle`] from a string literal containing a UUID.
///
/// The result is a `const`, so a UUID-named asset can be referenced without a
/// loader:
///
/// ```
/// # use kairos_asset::{Asset, Handle, VisitAssetDependencies, uuid_handle};
/// # struct Image;
/// # impl Asset for Image {}
/// # impl VisitAssetDependencies for Image {}
/// const IMAGE: Handle<Image> = uuid_handle!("1347c9b7-c46a-48e7-b7b8-023a354b7cac");
/// ```
#[macro_export]
macro_rules! uuid_handle {
    ($uuid:expr) => {{
        $crate::Handle::Uuid($crate::uuid::uuid!($uuid), core::marker::PhantomData)
    }};
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

    /// The path the asset was loaded from, if this is an
    /// [`UntypedHandle::Strong`] that has one.
    #[inline]
    pub fn path(&self) -> Option<&AssetPath<'static>> {
        match self {
            UntypedHandle::Strong(handle) => handle.path.as_ref(),
            UntypedHandle::Uuid { .. } => None,
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

    /// The [`MetaTransform`] stored on the handle, if it is strong and was
    /// loaded with one.
    #[inline]
    pub(crate) fn meta_transform(&self) -> Option<&MetaTransform> {
        match self {
            UntypedHandle::Strong(handle) => handle.meta_transform.as_ref(),
            UntypedHandle::Uuid { .. } => None,
        }
    }
}

impl Debug for UntypedHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UntypedHandle::Strong(handle) => write!(
                f,
                "StrongHandle{{ type_id: {:?}, index: {:?}, path: {:?} }}",
                handle.type_id, handle.index, handle.path
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
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UntypedAssetConversionError {
    /// The recorded [`TypeId`] does not match the target asset type.
    #[error(
        "this UntypedHandle is for {found:?} and cannot be converted into a Handle<{expected:?}>"
    )]
    TypeIdMismatch {
        /// The [`TypeId`] of the asset type being converted to.
        expected: TypeId,
        /// The [`TypeId`] recorded in the untyped handle.
        found: TypeId,
    },
}
