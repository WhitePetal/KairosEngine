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
//!
//! Deliberately absent for now (later tickets): the `AssetServer`, loaders,
//! paths and meta. This module does not import `tokio`.

mod asset;
mod assets;
mod event;
mod handle;
mod id;
mod index;

pub use asset::{Asset, VisitAssetDependencies};
pub use assets::{AssetMut, Assets, AssetsMutIterator, InvalidGenerationError};
pub use event::{AssetEvent, AssetLoadFailedEvent};
pub use handle::{
    AssetHandleProvider, DropEvent, Handle, StrongHandle, UntypedAssetConversionError,
    UntypedHandle,
};
pub use id::{AssetId, UntypedAssetId, UntypedAssetIdConversionError};
pub use index::{AssetIndex, AssetIndexAllocator};

#[cfg(test)]
mod tests;
