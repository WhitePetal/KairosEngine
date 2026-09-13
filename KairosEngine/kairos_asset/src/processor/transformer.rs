//! The [`AssetTransformer`] face: turning one asset type into another.
//!
//! A transformer is the middle stage of [`LoadTransformAndSave`](super::LoadTransformAndSave):
//! the loader produces `AssetInput`, the transformer maps it to `AssetOutput`,
//! and the saver writes it. Splitting the step out keeps each stage
//! independently useful — a loader can load the source without processing, and
//! a saver can save the transformed asset — but a processor that needs no
//! transformation can use [`IdentityAssetTransformer`] and skip it.

use core::marker::PhantomData;
use std::collections::{HashMap, hash_map::Entry};

use atomicow::CowArc;
use kairos_ecs::error::KairosError;
use kairos_tasks::ConditionalSendFuture;
use serde::{Deserialize, Serialize};

use crate::asset::Asset;
use crate::handle::UntypedHandle;
use crate::id::UntypedAssetId;
use crate::loader::{ErasedLoadedAsset, LabeledAsset};
use crate::meta::Settings;

/// Transforms an [`Asset`] of type [`AssetTransformer::AssetInput`] into one of
/// type [`AssetTransformer::AssetOutput`].
///
/// Commonly used through [`LoadTransformAndSave`](super::LoadTransformAndSave).
pub trait AssetTransformer: Send + Sync + 'static {
    /// The [`Asset`] type this transformer takes as input.
    type AssetInput: Asset;

    /// The [`Asset`] type this transformer produces.
    type AssetOutput: Asset;

    /// The settings this transformer is configured with.
    type Settings: Settings + Default + Serialize + for<'a> Deserialize<'a>;

    /// The error this transformer can produce.
    type Error: Into<KairosError>;

    /// Transforms `asset` according to `settings`.
    ///
    /// The transformed asset's labeled sub-assets may be added to or replaced
    /// as part of the transform.
    fn transform(
        &self,
        asset: TransformedAsset<Self::AssetInput>,
        settings: &Self::Settings,
    ) -> impl ConditionalSendFuture<Output = Result<TransformedAsset<Self::AssetOutput>, Self::Error>>;
}

/// An [`Asset`] (and its labeled sub-assets), intended to be transformed.
///
/// Unlike [`ErasedLoadedAsset`], it owns its value, so the transform step may
/// replace the root asset and hand labeled assets along to the new root.
pub struct TransformedAsset<A: Asset> {
    pub(crate) value: A,
    pub(crate) labeled_assets: Vec<LabeledAsset>,
    pub(crate) label_to_asset_index: HashMap<CowArc<'static, str>, usize>,
    pub(crate) asset_id_to_asset_index: HashMap<UntypedAssetId, usize>,
}

impl<A: Asset> core::ops::Deref for TransformedAsset<A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<A: Asset> core::ops::DerefMut for TransformedAsset<A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<A: Asset> TransformedAsset<A> {
    /// Takes the value out of `asset` if it is of type `A`, along with its
    /// labeled sub-assets.
    pub fn from_loaded(asset: ErasedLoadedAsset) -> Option<Self> {
        let ErasedLoadedAsset {
            value,
            labeled_assets,
            label_to_asset_index,
            asset_id_to_asset_index,
            ..
        } = asset;
        let value = *value.into_any().downcast::<A>().ok()?;
        Some(Self {
            value,
            labeled_assets,
            label_to_asset_index,
            asset_id_to_asset_index,
        })
    }

    /// Replaces the root value, carrying the labeled sub-assets over.
    pub fn replace_asset<B: Asset>(self, asset: B) -> TransformedAsset<B> {
        TransformedAsset {
            value: asset,
            labeled_assets: self.labeled_assets,
            label_to_asset_index: self.label_to_asset_index,
            asset_id_to_asset_index: self.asset_id_to_asset_index,
        }
    }

    /// Moves `labeled_source`'s labeled sub-assets into this asset, replacing
    /// any it already has.
    pub fn take_labeled_assets<B: Asset>(&mut self, labeled_source: TransformedAsset<B>) {
        self.labeled_assets = labeled_source.labeled_assets;
        self.label_to_asset_index = labeled_source.label_to_asset_index;
        self.asset_id_to_asset_index = labeled_source.asset_id_to_asset_index;
    }

    /// The root value.
    #[inline]
    pub fn get(&self) -> &A {
        &self.value
    }

    /// The root value mutably.
    #[inline]
    pub fn get_mut(&mut self) -> &mut A {
        &mut self.value
    }

    /// The type-erased labeled asset named `label`, if one exists.
    pub fn get_erased_labeled(&self, label: impl AsRef<str>) -> Option<&ErasedLoadedAsset> {
        let index = *self.label_to_asset_index.get(label.as_ref())?;
        Some(&self.labeled_assets[index].asset)
    }

    /// Adds `asset` as a labeled sub-asset under `label` and `handle`.
    pub fn insert_labeled(
        &mut self,
        label: impl Into<CowArc<'static, str>>,
        handle: impl Into<UntypedHandle>,
        asset: impl Into<ErasedLoadedAsset>,
    ) {
        let labeled = LabeledAsset {
            asset: asset.into(),
            handle: handle.into(),
        };
        match self.label_to_asset_index.entry(label.into()) {
            Entry::Occupied(entry) => {
                let index = *entry.get();
                let old_id = self.labeled_assets[index].handle.id();
                self.asset_id_to_asset_index.remove(&old_id);
                self.asset_id_to_asset_index
                    .insert(labeled.handle.id(), index);
                self.labeled_assets[index] = labeled;
            }
            Entry::Vacant(entry) => {
                let index = self.labeled_assets.len();
                entry.insert(index);
                self.asset_id_to_asset_index
                    .insert(labeled.handle.id(), index);
                self.labeled_assets.push(labeled);
            }
        }
    }

    /// Every label of a labeled sub-asset.
    pub fn iter_labels(&self) -> impl Iterator<Item = &str> {
        self.label_to_asset_index.keys().map(|label| label.as_ref())
    }
}

/// An identity [`AssetTransformer`] that returns its input unchanged.
///
/// Use it as the transform stage of a
/// [`LoadTransformAndSave`](super::LoadTransformAndSave) that only needs to load
/// and then save — for example, a pure format conversion.
pub struct IdentityAssetTransformer<A: Asset> {
    _marker: PhantomData<fn(A) -> A>,
}

impl<A: Asset> IdentityAssetTransformer<A> {
    /// Creates an identity transformer.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<A: Asset> Default for IdentityAssetTransformer<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Asset> AssetTransformer for IdentityAssetTransformer<A> {
    type AssetInput = A;
    type AssetOutput = A;
    type Settings = ();
    type Error = core::convert::Infallible;

    fn transform(
        &self,
        asset: TransformedAsset<Self::AssetInput>,
        _settings: &Self::Settings,
    ) -> impl ConditionalSendFuture<Output = Result<TransformedAsset<Self::AssetOutput>, Self::Error>>
    {
        async move { Ok(asset) }
    }
}
