//! Add methods on [`World`] so asset work can be reached when all a caller has
//! is a world, without taking an [`AssetServer`] or [`Assets<A>`] system
//! parameter first.

use kairos_ecs::world::World;

use crate::{Asset, AssetPath, AssetServer, Assets, Handle, LoadBuilder};

/// Methods for working with assets directly from a [`World`].
pub trait DirectAssetAccessExt {
    /// Inserts an asset, similarly to [`Assets::add`].
    ///
    /// # Panics
    ///
    /// If `self` has no [`Assets<A>`] resource.
    fn add_asset<A: Asset>(&mut self, asset: impl Into<A>) -> Handle<A>;

    /// Loads an asset, similarly to [`AssetServer::load`].
    ///
    /// # Panics
    ///
    /// If `self` has no [`AssetServer`] resource.
    fn load_asset<'a, A: Asset>(&self, path: impl Into<AssetPath<'a>>) -> Handle<A>;

    /// Creates a [`LoadBuilder`], similarly to [`AssetServer::load_builder`].
    ///
    /// # Panics
    ///
    /// If `self` has no [`AssetServer`] resource.
    fn load_builder(&self) -> LoadBuilder<'_>;
}

impl DirectAssetAccessExt for World {
    fn add_asset<A: Asset>(&mut self, asset: impl Into<A>) -> Handle<A> {
        self.resource_mut::<Assets<A>>().add(asset)
    }

    fn load_asset<'a, A: Asset>(&self, path: impl Into<AssetPath<'a>>) -> Handle<A> {
        self.resource::<AssetServer>().load(path)
    }

    fn load_builder(&self) -> LoadBuilder<'_> {
        self.resource::<AssetServer>().load_builder()
    }
}
