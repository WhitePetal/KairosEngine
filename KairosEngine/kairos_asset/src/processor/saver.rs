//! The [`AssetSaver`] face: turning an asset value into processed bytes.
//!
//! A saver writes an asset value to a byte [`Writer`] and returns the settings
//! its output loader should be configured with. It is the inverse of an
//! [`AssetLoader`], and is generally used in concert with it through
//! [`LoadTransformAndSave`](super::LoadTransformAndSave).

use core::ops::Deref;
use std::collections::HashMap;

use atomicow::CowArc;
use kairos_ecs::error::KairosError;
use kairos_tasks::ConditionalSendFuture;
use serde::{Deserialize, Serialize};

use crate::asset::Asset;
use crate::handle::{Handle, UntypedHandle};
use crate::id::UntypedAssetId;
use crate::io::{BoxedFuture, Writer};
use crate::loader::{AssetLoader, ErasedLoadedAsset, LabeledAsset};
use crate::meta::Settings;
use crate::path::AssetPath;

use super::transformer::TransformedAsset;

/// Saves an [`Asset`] of type [`AssetSaver::Asset`], producing bytes that
/// [`AssetSaver::OutputLoader`] can load.
///
/// This is the inverse of [`AssetLoader`]. It is currently only used by the
/// asset processor, not as a general-purpose persistence interface.
pub trait AssetSaver: Send + Sync + 'static {
    /// The top-level [`Asset`] this saver writes.
    type Asset: Asset;

    /// The settings this saver is configured with.
    type Settings: Settings + Default + Serialize + for<'a> Deserialize<'a>;

    /// The [`AssetLoader`] that loads what this saver writes.
    type OutputLoader: AssetLoader;

    /// The error this saver can produce.
    type Error: Into<KairosError>;

    /// Writes `asset` to `writer`, returning the settings the output loader
    /// should be configured with.
    fn save(
        &self,
        writer: &mut Writer,
        asset: SavedAsset<'_, Self::Asset>,
        settings: &Self::Settings,
        asset_path: AssetPath<'_>,
    ) -> impl ConditionalSendFuture<
        Output = Result<<Self::OutputLoader as AssetLoader>::Settings, Self::Error>,
    >;
}

/// A type-erased [`AssetSaver`].
pub trait ErasedAssetSaver: Send + Sync + 'static {
    /// Saves an [`ErasedLoadedAsset`] through the typed saver.
    fn save<'a>(
        &'a self,
        writer: &'a mut Writer,
        asset: &'a ErasedLoadedAsset,
        settings: &'a dyn Settings,
        asset_path: AssetPath<'a>,
    ) -> BoxedFuture<'a, Result<(), KairosError>>;

    /// The saver's fully-qualified name.
    fn type_name(&self) -> &'static str;
}

impl<S: AssetSaver> ErasedAssetSaver for S {
    fn save<'a>(
        &'a self,
        writer: &'a mut Writer,
        asset: &'a ErasedLoadedAsset,
        settings: &'a dyn Settings,
        asset_path: AssetPath<'a>,
    ) -> BoxedFuture<'a, Result<(), KairosError>> {
        Box::pin(async move {
            let settings = settings
                .downcast_ref::<S::Settings>()
                .expect("AssetSaver settings should match the saver type");
            let saved = SavedAsset::<S::Asset>::from_loaded(asset)
                .expect("the loaded asset matches the saver's asset type");
            self.save(writer, saved, settings, asset_path)
                .await
                .map_err(Into::into)?;
            Ok(())
        })
    }

    fn type_name(&self) -> &'static str {
        core::any::type_name::<S>()
    }
}

/// An [`Asset`] (and its labeled sub-assets), intended to be saved.
///
/// It is a borrowed, typed view over an [`ErasedLoadedAsset`] or a
/// [`TransformedAsset`]; the labeled assets are reached by label, by id, or by
/// [`UntypedHandle`] without copying the values.
pub struct SavedAsset<'a, A: Asset> {
    value: &'a A,
    labeled_assets: &'a [LabeledAsset],
    label_to_asset_index: &'a HashMap<CowArc<'static, str>, usize>,
    asset_id_to_asset_index: &'a HashMap<UntypedAssetId, usize>,
}

impl<A: Asset> Deref for SavedAsset<'_, A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'a, A: Asset> SavedAsset<'a, A> {
    /// Creates a [`SavedAsset`] from `asset` if its stored value is of type
    /// `A`.
    pub fn from_loaded(asset: &'a ErasedLoadedAsset) -> Option<Self> {
        let value = asset.value.as_any().downcast_ref::<A>()?;
        Some(Self {
            value,
            labeled_assets: &asset.labeled_assets,
            label_to_asset_index: &asset.label_to_asset_index,
            asset_id_to_asset_index: &asset.asset_id_to_asset_index,
        })
    }

    /// Creates a [`SavedAsset`] borrowing from a [`TransformedAsset`].
    pub fn from_transformed(asset: &'a TransformedAsset<A>) -> Self {
        Self {
            value: &asset.value,
            labeled_assets: &asset.labeled_assets,
            label_to_asset_index: &asset.label_to_asset_index,
            asset_id_to_asset_index: &asset.asset_id_to_asset_index,
        }
    }

    /// The root value.
    #[inline]
    pub fn get(&self) -> &'a A {
        self.value
    }

    /// The labeled asset named `label`, if one exists and is of type `B`.
    pub fn get_labeled<B: Asset>(&self, label: impl AsRef<str>) -> Option<SavedAsset<'a, B>> {
        let index = *self.label_to_asset_index.get(label.as_ref())?;
        let labeled = &self.labeled_assets[index];
        let value = labeled.asset.value.as_any().downcast_ref::<B>()?;
        Some(SavedAsset {
            value,
            labeled_assets: &labeled.asset.labeled_assets,
            label_to_asset_index: &labeled.asset.label_to_asset_index,
            asset_id_to_asset_index: &labeled.asset.asset_id_to_asset_index,
        })
    }

    /// The type-erased labeled asset named `label`, if one exists.
    pub fn get_erased_labeled(&self, label: impl AsRef<str>) -> Option<&'a ErasedLoadedAsset> {
        let index = *self.label_to_asset_index.get(label.as_ref())?;
        Some(&self.labeled_assets[index].asset)
    }

    /// The labeled asset addressed by `id`, if one exists and is of type `B`.
    pub fn get_labeled_by_id<B: Asset>(
        &self,
        id: impl Into<UntypedAssetId>,
    ) -> Option<SavedAsset<'a, B>> {
        let index = *self.asset_id_to_asset_index.get(&id.into())?;
        let labeled = &self.labeled_assets[index];
        let value = labeled.asset.value.as_any().downcast_ref::<B>()?;
        Some(SavedAsset {
            value,
            labeled_assets: &labeled.asset.labeled_assets,
            label_to_asset_index: &labeled.asset.label_to_asset_index,
            asset_id_to_asset_index: &labeled.asset.asset_id_to_asset_index,
        })
    }

    /// The type-erased labeled asset addressed by `id`, if one exists.
    pub fn get_erased_labeled_by_id(
        &self,
        id: impl Into<UntypedAssetId>,
    ) -> Option<&'a ErasedLoadedAsset> {
        let index = *self.asset_id_to_asset_index.get(&id.into())?;
        Some(&self.labeled_assets[index].asset)
    }

    /// The [`UntypedHandle`] of the labeled asset named `label`, if one exists.
    pub fn get_untyped_handle(&self, label: impl AsRef<str>) -> Option<UntypedHandle> {
        let index = *self.label_to_asset_index.get(label.as_ref())?;
        Some(self.labeled_assets[index].handle.clone())
    }

    /// The typed [`Handle`] of the labeled asset named `label`, if one exists
    /// and is of type `B`.
    pub fn get_handle<B: Asset>(&self, label: impl AsRef<str>) -> Option<Handle<B>> {
        let index = *self.label_to_asset_index.get(label.as_ref())?;
        self.labeled_assets[index]
            .handle
            .clone()
            .try_typed::<B>()
            .ok()
    }

    /// Every label of a labeled asset in this saved asset.
    pub fn iter_labels(&self) -> impl Iterator<Item = &str> {
        self.label_to_asset_index.keys().map(|label| label.as_ref())
    }
}
