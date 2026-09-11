//! The next-generation asset core.
//!
//! This module is the landing zone for the `bevy_asset`-style rewrite of the
//! asset system. It is **additive**: it lands beside the untouched legacy stack
//! in [`crate::assets`], and nothing consumes it yet. When the migration lands
//! it is promoted out of `next` to the crate root.
//!
//! The layers that have landed so far are:
//!
//! - Identity and handles: [`AssetIndex`] — a generational slot id, plus
//!   [`AssetIndexAllocator`] to hand them out and recycle them;
//!   [`AssetId`]/[`UntypedAssetId`] — a typed (and type-erased) asset identity;
//!   [`Handle`]/[`UntypedHandle`] — reference-counted borrows backed by
//!   [`StrongHandle`]. Handles are **not** [`Copy`](core::marker::Copy): the
//!   `Arc` inside a strong handle is the reference count, and the last clone to
//!   drop sends a [`DropEvent`]. [`Asset`]/[`VisitAssetDependencies`] are the
//!   trait bound and dependency visitor every asset participates in.
//! - Storage and events: [`Assets<A>`] — the per-type asset store, with
//!   [`AssetMut`] for tracked mutation; [`AssetEvent<A>`] — the five-variant
//!   event face (a `Message`).
//! - Io, paths, and meta: [`AssetPath`]/[`AssetSourceId`] address an asset as
//!   `source://path#label`; [`io::AssetReader`] and [`io::AssetSource`] read its
//!   bytes; [`meta::AssetMeta`] is the RON `.meta` sidecar that names the
//!   loader and its settings.
//! - Loading: [`AssetLoader`] turns bytes into an asset; [`LoadContext`]
//!   declares dependencies and collects labeled sub-assets, and
//!   [`LoadContext::finish`] folds them into a [`LoadedAsset`]/
//!   [`ErasedLoadedAsset`]. [`AssetServer`] registers loaders and asset types,
//!   tracks each asset's [`LoadState`], and runs loads as tasks on the
//!   `IoTaskPool`; [`handle_internal_asset_events`] inserts the finished values
//!   into their stores.
//!
//! Deliberately absent for now (later tickets): untyped and folder loads,
//! `add_async`/guards, `wait_for_asset*`, and the processor. This module does
//! not import `tokio`.

mod asset;
mod assets;
mod event;
mod handle;
mod id;
mod index;
pub mod io;
mod loader;
pub mod meta;
mod path;
mod server;

pub use asset::{Asset, VisitAssetDependencies};
pub use assets::{AssetMut, Assets, AssetsMutIterator, InvalidGenerationError};
pub use event::{AssetEvent, AssetLoadFailedEvent};
pub use handle::{
    AssetHandleProvider, DropEvent, Handle, StrongHandle, UntypedAssetConversionError,
    UntypedHandle,
};
pub use id::{AssetId, UntypedAssetId, UntypedAssetIdConversionError};
pub use index::{AssetIndex, AssetIndexAllocator};
pub use loader::{
    AssetContainer, AssetLoader, ErasedAssetLoader, ErasedLoadedAsset, LoadContext, LoadedAsset,
};
pub use server::{
    AssetLoadError, AssetLoaderError, AssetServer, AssetServerMode, DependencyLoadState,
    LoadState, RecursiveDependencyLoadState, handle_internal_asset_events,
};
pub use io::{
    AssetReader, AssetReaderError, AssetSourceEvent, AssetSourceId, AssetWriter,
    AssetWriterError, ErasedAssetReader, ErasedAssetWriter, Reader, UnapprovedPathMode,
    VecReader, Writer, get_meta_path,
};
pub use meta::{
    AssetAction, AssetActionMinimal, AssetHash, AssetMeta, AssetMetaCheck, AssetMetaDyn,
    AssetMetaMinimal, DeserializeMetaError, MetaTransform, META_FORMAT_VERSION, Settings,
    loader_name, loader_settings_meta_transform, meta_transform_settings,
};
pub use path::{AssetPath, ParseAssetPathError};

#[cfg(test)]
mod tests;
