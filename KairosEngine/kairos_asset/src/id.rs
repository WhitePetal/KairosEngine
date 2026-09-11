//! Asset identifiers, typed ([`AssetId<A>`]) and untyped ([`UntypedAssetId`]).
//!
//! An id names an asset independently of who holds it. It is cheap to copy and
//! can point at an asset that no longer exists, unlike a
//! [`Handle`](crate::Handle), which keeps its asset alive.

use core::{
    any::TypeId,
    fmt::{self, Debug},
    hash::{Hash, Hasher},
    marker::PhantomData,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::asset::Asset;
use crate::index::AssetIndex;

/// The UUID that [`AssetId::default`] resolves to.
pub(crate) const DEFAULT_UUID: Uuid = Uuid::from_u128(200809721996911295814598172825939264631);

/// A UUID that is never valid to assign to an asset.
pub(crate) const INVALID_UUID: Uuid = Uuid::from_u128(108428345662029828789348721013522787528);

/// A runtime-only identifier for an [`Asset`] of type `A`.
///
/// This is cheap to [`Copy`]/[`Clone`] and is not tied to the lifetime of the
/// asset, so it can name an asset that no longer exists. For an identifier tied
/// to the asset's lifetime, see [`Handle`](crate::Handle).
///
/// For a type-erased id, see [`UntypedAssetId`].
#[derive(Serialize, Deserialize)]
#[serde(bound = "")]
pub enum AssetId<A: Asset> {
    /// The default, slot-based identifier. This is what every asset gets unless
    /// it is explicitly registered under a [`AssetId::Uuid`].
    Index {
        /// The opaque slot (and generation) the asset lives in.
        index: AssetIndex,
        /// Carries the asset type without storing a value.
        #[serde(skip)]
        marker: PhantomData<fn() -> A>,
    },
    /// A stable-across-runs identifier, used only for assets explicitly
    /// registered under one.
    Uuid {
        /// The registered UUID.
        uuid: Uuid,
    },
}

impl<A: Asset> AssetId<A> {
    /// The UUID [`AssetId::default`] resolves to. Assigning a value to it in an
    /// asset store is valid and, where appropriate, conventional.
    pub const DEFAULT_UUID: Uuid = DEFAULT_UUID;

    /// A UUID that should never be assigned to. Use [`AssetId::invalid`] to get
    /// an [`AssetId`] carrying it.
    pub const INVALID_UUID: Uuid = INVALID_UUID;

    /// Returns an id carrying [`AssetId::INVALID_UUID`], which should never be
    /// assigned to.
    #[inline]
    pub const fn invalid() -> Self {
        Self::Uuid {
            uuid: Self::INVALID_UUID,
        }
    }

    /// Converts this into a type-erased [`UntypedAssetId`], recording the asset
    /// type alongside the id.
    #[inline]
    pub fn untyped(self) -> UntypedAssetId {
        self.into()
    }

    #[inline]
    fn internal(self) -> InternalAssetId {
        match self {
            AssetId::Index { index, .. } => InternalAssetId::Index(index),
            AssetId::Uuid { uuid } => InternalAssetId::Uuid(uuid),
        }
    }
}

impl<A: Asset> Default for AssetId<A> {
    fn default() -> Self {
        AssetId::Uuid {
            uuid: Self::DEFAULT_UUID,
        }
    }
}

impl<A: Asset> Clone for AssetId<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Asset> Copy for AssetId<A> {}

impl<A: Asset> Debug for AssetId<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetId::Index { index, .. } => write!(
                f,
                "AssetId<{}>{{ index: {}, generation: {} }}",
                core::any::type_name::<A>(),
                index.index(),
                index.generation()
            ),
            AssetId::Uuid { uuid } => {
                write!(f, "AssetId<{}>{{ uuid: {} }}", core::any::type_name::<A>(), uuid)
            }
        }
    }
}

impl<A: Asset> Hash for AssetId<A> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.internal().hash(state);
        TypeId::of::<A>().hash(state);
    }
}

impl<A: Asset> PartialEq for AssetId<A> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.internal() == other.internal()
    }
}

impl<A: Asset> Eq for AssetId<A> {}

impl<A: Asset> PartialOrd for AssetId<A> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<A: Asset> Ord for AssetId<A> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.internal().cmp(&other.internal())
    }
}

impl<A: Asset> From<AssetIndex> for AssetId<A> {
    #[inline]
    fn from(index: AssetIndex) -> Self {
        AssetId::Index {
            index,
            marker: PhantomData,
        }
    }
}

impl<A: Asset> From<Uuid> for AssetId<A> {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        AssetId::Uuid { uuid }
    }
}

/// An id with the [`Asset`] type erased into runtime [`TypeId`] information,
/// so ids across asset types can be stored and compared together.
///
/// Unlike [`AssetId`], this is not [`Serialize`]/[`Deserialize`]: a [`TypeId`]
/// has no stable serialized form.
#[derive(Debug, Copy, Clone)]
pub enum UntypedAssetId {
    /// A slot-based id, as for [`AssetId::Index`].
    Index {
        /// The asset type this id names.
        type_id: TypeId,
        /// The opaque slot (and generation) the asset lives in.
        index: AssetIndex,
    },
    /// A UUID-based id, as for [`AssetId::Uuid`].
    Uuid {
        /// The asset type this id names.
        type_id: TypeId,
        /// The registered UUID.
        uuid: Uuid,
    },
}

impl UntypedAssetId {
    /// Converts to a typed [`AssetId`] without checking the recorded type.
    ///
    /// Only use this when the type is genuinely known; prefer
    /// [`UntypedAssetId::typed`] or [`UntypedAssetId::try_typed`] otherwise.
    #[inline]
    pub fn typed_unchecked<A: Asset>(self) -> AssetId<A> {
        match self {
            UntypedAssetId::Index { index, .. } => AssetId::Index {
                index,
                marker: PhantomData,
            },
            UntypedAssetId::Uuid { uuid, .. } => AssetId::Uuid { uuid },
        }
    }

    /// Converts to a typed [`AssetId`], checking the recorded type in debug
    /// builds only.
    #[inline]
    pub fn typed_debug_checked<A: Asset>(self) -> AssetId<A> {
        debug_assert_eq!(
            self.type_id(),
            TypeId::of::<A>(),
            "the target AssetId<{}>'s TypeId does not match the TypeId of this UntypedAssetId",
            core::any::type_name::<A>()
        );
        self.typed_unchecked()
    }

    /// Converts to a typed [`AssetId`].
    ///
    /// # Panics
    ///
    /// Panics if the recorded type does not match `A`.
    #[inline]
    pub fn typed<A: Asset>(self) -> AssetId<A> {
        let Ok(id) = self.try_typed() else {
            panic!(
                "the target AssetId<{}>'s TypeId does not match the TypeId of this UntypedAssetId",
                core::any::type_name::<A>()
            )
        };
        id
    }

    /// Converts to a typed [`AssetId`] if the recorded type matches `A`.
    #[inline]
    pub fn try_typed<A: Asset>(self) -> Result<AssetId<A>, UntypedAssetIdConversionError> {
        AssetId::try_from(self)
    }

    /// The asset type this id was recorded for.
    #[inline]
    pub fn type_id(&self) -> TypeId {
        match self {
            UntypedAssetId::Index { type_id, .. } | UntypedAssetId::Uuid { type_id, .. } => *type_id,
        }
    }

    #[inline]
    fn internal(self) -> InternalAssetId {
        match self {
            UntypedAssetId::Index { index, .. } => InternalAssetId::Index(index),
            UntypedAssetId::Uuid { uuid, .. } => InternalAssetId::Uuid(uuid),
        }
    }
}

impl PartialEq for UntypedAssetId {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.type_id() == other.type_id() && self.internal() == other.internal()
    }
}

impl Eq for UntypedAssetId {}

impl Hash for UntypedAssetId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.internal().hash(state);
        self.type_id().hash(state);
    }
}

impl PartialOrd for UntypedAssetId {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for UntypedAssetId {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.type_id()
            .cmp(&other.type_id())
            .then_with(|| self.internal().cmp(&other.internal()))
    }
}

/// The id shape shared by the typed and untyped wrappers, with no type
/// information at all.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
enum InternalAssetId {
    Index(AssetIndex),
    Uuid(Uuid),
}

// Cross operations between typed and untyped ids.

impl<A: Asset> PartialEq<UntypedAssetId> for AssetId<A> {
    #[inline]
    fn eq(&self, other: &UntypedAssetId) -> bool {
        TypeId::of::<A>() == other.type_id() && self.internal() == other.internal()
    }
}

impl<A: Asset> PartialEq<AssetId<A>> for UntypedAssetId {
    #[inline]
    fn eq(&self, other: &AssetId<A>) -> bool {
        other == self
    }
}

impl<A: Asset> PartialOrd<UntypedAssetId> for AssetId<A> {
    fn partial_cmp(&self, other: &UntypedAssetId) -> Option<core::cmp::Ordering> {
        if TypeId::of::<A>() != other.type_id() {
            None
        } else {
            Some(self.internal().cmp(&other.internal()))
        }
    }
}

impl<A: Asset> PartialOrd<AssetId<A>> for UntypedAssetId {
    fn partial_cmp(&self, other: &AssetId<A>) -> Option<core::cmp::Ordering> {
        Some(other.partial_cmp(self)?.reverse())
    }
}

impl<A: Asset> From<AssetId<A>> for UntypedAssetId {
    #[inline]
    fn from(value: AssetId<A>) -> Self {
        let type_id = TypeId::of::<A>();
        match value {
            AssetId::Index { index, .. } => UntypedAssetId::Index { type_id, index },
            AssetId::Uuid { uuid } => UntypedAssetId::Uuid { type_id, uuid },
        }
    }
}

impl<A: Asset> TryFrom<UntypedAssetId> for AssetId<A> {
    type Error = UntypedAssetIdConversionError;

    #[inline]
    fn try_from(value: UntypedAssetId) -> Result<Self, Self::Error> {
        let found = value.type_id();
        let expected = TypeId::of::<A>();

        match value {
            UntypedAssetId::Index { index, type_id } if type_id == expected => Ok(AssetId::Index {
                index,
                marker: PhantomData,
            }),
            UntypedAssetId::Uuid { uuid, type_id } if type_id == expected => {
                Ok(AssetId::Uuid { uuid })
            }
            _ => Err(UntypedAssetIdConversionError::TypeIdMismatch { expected, found }),
        }
    }
}

/// The error returned when an [`UntypedAssetId`] is converted to the wrong
/// typed [`AssetId`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UntypedAssetIdConversionError {
    /// The recorded [`TypeId`] does not match the target asset type.
    TypeIdMismatch {
        /// The [`TypeId`] of the asset type being converted to.
        expected: TypeId,
        /// The [`TypeId`] recorded in the untyped id.
        found: TypeId,
    },
}

impl fmt::Display for UntypedAssetIdConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypeIdMismatch { expected, found } => write!(
                f,
                "this UntypedAssetId is for {found:?} and cannot be converted into an AssetId<{expected:?}>"
            ),
        }
    }
}

impl std::error::Error for UntypedAssetIdConversionError {}
