//! The builder-pattern surface for loading nested assets inside a
//! [`LoadContext`], mirroring `bevy_asset`'s `loader_builders.rs`.
//!
//! [`NestedLoadBuilder`] splits the nested-loading surface in two:
//!
//! - the **deferred** `load`/`load_erased`/`load_untyped` family, which hands
//!   back a [`Handle`] and records a handle dependency, exactly like
//!   [`LoadContext::load`]; and
//! - the **immediate** (direct) `load_value*` family, which returns the loaded
//!   value now and records only a
//!   [`loader_dependency`](LoadContext::read_asset_bytes) — a processing input,
//!   not a readiness edge.

use core::any::{type_name, TypeId};
use std::path::Path;

use crate::asset::Asset;
use crate::assets::LoadedUntypedAsset;
use crate::handle::{Handle, UntypedHandle};
use crate::id::UntypedAssetId;
use crate::io::Reader;
use crate::loader::{
    ErasedAssetLoader, ErasedLoadedAsset, LoadContext, LoadDirectError, LoadedAsset,
};
use crate::meta::{MetaTransform, Settings, loader_settings_meta_transform};
use crate::path::AssetPath;
use crate::server::AssetLoadError;

/// A builder for loading nested assets inside a [`LoadContext`].
///
/// Created by [`LoadContext::load_builder`].
#[must_use = "the nested load doesn't start until NestedLoadBuilder has been consumed"]
pub struct NestedLoadBuilder<'ctx, 'builder> {
    load_context: &'builder mut LoadContext<'ctx>,
    /// A function to modify the loader settings of the nested load.
    meta_transform: Option<MetaTransform>,
    /// Whether this load may read a path the server would otherwise deny.
    override_unapproved: bool,
}

impl<'ctx, 'builder> NestedLoadBuilder<'ctx, 'builder> {
    pub(crate) fn new(load_context: &'builder mut LoadContext<'ctx>) -> Self {
        Self {
            load_context,
            meta_transform: None,
            override_unapproved: false,
        }
    }

    /// Overrides the nested asset's [`AssetLoader`] settings for this load.
    ///
    /// The settings type must match the loader's `Settings`; a mismatch is
    /// silently ignored (see [`loader_settings_meta_transform`]).
    ///
    /// [`AssetLoader`]: crate::AssetLoader
    pub fn with_settings<S: Settings>(
        mut self,
        settings: impl Fn(&mut S) + Send + Sync + 'static,
    ) -> Self {
        let new_transform = loader_settings_meta_transform(settings);
        if let Some(prev_transform) = self.meta_transform.take() {
            self.meta_transform = Some(Box::new(move |meta| {
                prev_transform(meta);
                new_transform(meta);
            }));
        } else {
            self.meta_transform = Some(new_transform);
        }
        self
    }

    /// Allows this nested load to read a path that escapes its source even when
    /// the server is in [`UnapprovedPathMode::Deny`].
    ///
    /// [`UnapprovedPathMode::Forbid`] is unaffected: it rejects unapproved paths
    /// whatever the builder asks for.
    ///
    /// [`UnapprovedPathMode::Deny`]: crate::UnapprovedPathMode::Deny
    /// [`UnapprovedPathMode::Forbid`]: crate::UnapprovedPathMode::Forbid
    pub fn override_unapproved(mut self) -> Self {
        self.override_unapproved = true;
        self
    }

    /// Loads `path` as `A`, returning its handle.
    ///
    /// This is a deferred load: the caller does not get the value, only a handle
    /// that becomes ready once the load (and its dependencies) settle. Use
    /// [`load_value`](Self::load_value) to read the value immediately.
    #[must_use = "not using the returned strong handle may result in the unexpected release of the asset"]
    pub fn load<'a, A: Asset>(self, path: impl Into<AssetPath<'a>>) -> Handle<A> {
        self.load_internal(TypeId::of::<A>(), Some(type_name::<A>()), path.into())
            .typed_debug_checked()
    }

    /// Loads `path` as the runtime asset type `type_id`, returning its handle.
    ///
    /// The type-erased counterpart of [`load`](Self::load).
    #[must_use = "not using the returned strong handle may result in the unexpected release of the asset"]
    pub fn load_erased<'a>(self, type_id: TypeId, path: impl Into<AssetPath<'a>>) -> UntypedHandle {
        self.load_internal(type_id, None, path.into())
    }

    /// Loads `path` without knowing its type, returning a handle to a
    /// [`LoadedUntypedAsset`] wrapper.
    ///
    /// This is a deferred load; use
    /// [`load_untyped_value`](Self::load_untyped_value) to read the value
    /// immediately.
    #[must_use = "not using the returned strong handle may result in the unexpected release of the asset"]
    pub fn load_untyped<'a>(self, path: impl Into<AssetPath<'a>>) -> Handle<LoadedUntypedAsset> {
        let path = path.into().into_owned();
        if path.path() == Path::new("") {
            return Handle::default();
        }
        let handle = if self.load_context.should_load_dependencies {
            self.load_context
                .asset_server
                .load_unknown_type_with_meta_transform(
                    path,
                    self.meta_transform,
                    (),
                    self.override_unapproved,
                )
        } else {
            self.load_context
                .asset_server
                .get_or_create_path_handle_erased(
                    path,
                    TypeId::of::<LoadedUntypedAsset>(),
                    Some(type_name::<LoadedUntypedAsset>()),
                    self.meta_transform,
                )
                .typed_debug_checked()
        };
        record_dependency(self.load_context, handle.id().untyped());
        handle
    }

    /// Loads `path` as `A` and returns the loaded value.
    ///
    /// This is an immediate (direct) load: the value is available now, and the
    /// path is recorded as a loader dependency rather than a handle dependency.
    ///
    /// # Errors
    ///
    /// Returns [`LoadDirectError::EmptyPath`] for a path with no file component,
    /// [`LoadDirectError::RequestedSubasset`] for a path carrying a label, or
    /// [`LoadDirectError::LoadError`] if the load itself fails.
    pub async fn load_value<'a, A: Asset>(
        self,
        path: impl Into<AssetPath<'a>>,
    ) -> Result<LoadedAsset<A>, LoadDirectError> {
        self.load_typed_value_internal(path.into().into_owned(), None)
            .await
    }

    /// Loads `path` as the runtime asset type `type_id` and returns the loaded
    /// value.
    ///
    /// The type-erased counterpart of [`load_value`](Self::load_value).
    pub async fn load_erased_value<'a>(
        self,
        type_id: TypeId,
        path: impl Into<AssetPath<'a>>,
    ) -> Result<ErasedLoadedAsset, LoadDirectError> {
        self.load_value_internal(Some(type_id), &path.into().into_owned(), None)
            .await
            .map(|(_, asset)| asset)
    }

    /// Loads `path` without knowing its type and returns the loaded value.
    pub async fn load_untyped_value<'a>(
        self,
        path: impl Into<AssetPath<'a>>,
    ) -> Result<ErasedLoadedAsset, LoadDirectError> {
        self.load_value_internal(None, &path.into().into_owned(), None)
            .await
            .map(|(_, asset)| asset)
    }

    /// Loads `A` from `reader` instead of from the source, returning the loaded
    /// value.
    ///
    /// `path` names the asset for handle purposes (labeled assets hang off it,
    /// and relative paths inside the nested loader resolve against it). The
    /// loader is resolved by asset type.
    pub async fn load_value_from_reader<'a, A: Asset>(
        self,
        path: impl Into<AssetPath<'a>>,
        reader: &'builder mut dyn Reader,
    ) -> Result<LoadedAsset<A>, LoadDirectError> {
        self.load_typed_value_internal(path.into().into_owned(), Some(reader))
            .await
    }

    /// Loads the runtime asset type `type_id` from `reader`, returning the
    /// loaded value.
    pub async fn load_erased_value_from_reader<'a>(
        self,
        type_id: TypeId,
        path: impl Into<AssetPath<'a>>,
        reader: &'builder mut dyn Reader,
    ) -> Result<ErasedLoadedAsset, LoadDirectError> {
        self.load_value_internal(Some(type_id), &path.into().into_owned(), Some(reader))
            .await
            .map(|(_, asset)| asset)
    }

    /// Loads an asset of unknown type from `reader`, resolving the loader from
    /// `path`, and returns the loaded value.
    pub async fn load_untyped_value_from_reader<'a>(
        self,
        path: impl Into<AssetPath<'a>>,
        reader: &'builder mut dyn Reader,
    ) -> Result<ErasedLoadedAsset, LoadDirectError> {
        self.load_value_internal(None, &path.into().into_owned(), Some(reader))
            .await
            .map(|(_, asset)| asset)
    }

    /// Acquires the handle for `(type_id, path)` and, when this context loads
    /// dependencies, kicks off the deferred load.
    fn load_internal<'a>(
        self,
        type_id: TypeId,
        type_name: Option<&str>,
        path: AssetPath<'a>,
    ) -> UntypedHandle {
        let path = path.into_owned();
        if path.path() == Path::new("") {
            return UntypedHandle::default_for_type(type_id);
        }
        let handle = if self.load_context.should_load_dependencies {
            self.load_context.asset_server.load_with_meta_transform(
                path,
                type_id,
                type_name,
                self.meta_transform,
                (),
                self.override_unapproved,
            )
        } else {
            self.load_context
                .asset_server
                .get_or_create_path_handle_erased(path, type_id, type_name, self.meta_transform)
        };
        record_dependency(self.load_context, handle.id());
        handle
    }

    /// Creates the future for one immediate (direct) load.
    ///
    /// The type is either given, or deduced from `path` (via its loader or
    /// `.meta`). When `reader` is [`Some`], the load reads from it; otherwise it
    /// goes through the server's source.
    async fn load_value_internal(
        self,
        type_id: Option<TypeId>,
        path: &AssetPath<'static>,
        reader: Option<&'builder mut dyn Reader>,
    ) -> Result<(std::sync::Arc<dyn ErasedAssetLoader>, ErasedLoadedAsset), LoadDirectError> {
        if path.path() == Path::new("") {
            return Err(LoadDirectError::EmptyPath(path.clone()));
        }
        if path.label().is_some() {
            return Err(LoadDirectError::RequestedSubasset(path.clone()));
        }

        let (mut meta, loader, mut reader) = if let Some(reader) = reader {
            let loader = if let Some(type_id) = type_id {
                self.load_context
                    .asset_server
                    .get_asset_loader_with_asset_type_id(type_id)
                    .await
                    .map_err(AssetLoadError::from)
            } else {
                self.load_context
                    .asset_server
                    .get_path_asset_loader(path)
                    .await
                    .map_err(AssetLoadError::from)
            }
            .map_err(|error| LoadDirectError::LoadError {
                dependency: path.clone(),
                error,
            })?;
            let meta = loader.default_meta();
            (meta, loader, ReaderRef::Borrowed(reader))
        } else {
            let (meta, loader, reader) = self
                .load_context
                .asset_server
                .get_meta_loader_and_reader(path, type_id)
                .await
                .map_err(|error| LoadDirectError::LoadError {
                    dependency: path.clone(),
                    error,
                })?;
            (meta, loader, ReaderRef::Boxed(reader))
        };

        if let Some(meta_transform) = self.meta_transform {
            meta_transform(&mut *meta);
        }

        let asset = self
            .load_context
            .load_direct_internal(
                path.clone(),
                meta.loader_settings().expect("meta corresponds to a load"),
                &*loader,
                reader.as_mut(),
                meta.processed_info().as_ref(),
            )
            .await?;
        Ok((loader, asset))
    }

    /// [`load_value_internal`](Self::load_value_internal), with a generic to
    /// check the returned value's type.
    async fn load_typed_value_internal<A: Asset>(
        self,
        path: AssetPath<'static>,
        reader: Option<&'builder mut dyn Reader>,
    ) -> Result<LoadedAsset<A>, LoadDirectError> {
        let (loader, untyped_asset) = self
            .load_value_internal(Some(TypeId::of::<A>()), &path, reader)
            .await?;
        untyped_asset
            .downcast::<A>()
            .map_err(|_| LoadDirectError::LoadError {
                dependency: path.clone(),
                error: AssetLoadError::RequestedHandleTypeMismatch {
                    path,
                    requested: TypeId::of::<A>(),
                    actual_asset_name: loader.asset_type_name(),
                    loader_name: loader.type_name(),
                },
            })
    }
}

/// The source of the reader a nested load reads from: either the caller's own
/// reader or one the server handed back from its source.
enum ReaderRef<'a> {
    Borrowed(&'a mut dyn Reader),
    Boxed(Box<dyn Reader + 'a>),
}

impl ReaderRef<'_> {
    fn as_mut(&mut self) -> &mut dyn Reader {
        match self {
            ReaderRef::Borrowed(reader) => &mut **reader,
            ReaderRef::Boxed(reader) => &mut **reader,
        }
    }
}

/// Records a deferred handle as a dependency of the outer load.
///
/// A refused load (an empty or unapproved path) yields the default `Uuid`
/// handle, which is not a handle edge, so only strong (`Index`) handles are
/// recorded.
///
/// `bevy_asset` 0.19.1 instead unwraps the handle-to-index conversion here and
/// panics on a refused unapproved path; this matches the fix `bevy` later
/// adopted for `bevy` #21584 (PR #25435).
fn record_dependency(load_context: &mut LoadContext, id: UntypedAssetId) {
    if matches!(id, UntypedAssetId::Index { .. }) {
        load_context.dependencies.insert(id);
    }
}
