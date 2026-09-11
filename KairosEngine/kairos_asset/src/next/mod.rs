//! The next-generation asset core.
//!
//! This module is the landing zone for the `bevy_asset`-style rewrite of the
//! asset system. It is **additive**: it lands beside the untouched legacy stack
//! in [`crate::assets`], and nothing consumes it yet. When the migration lands
//! it is promoted out of `next` to the crate root.
//!
//! This layer is identity and handles only:
//!
//! - [`AssetIndex`] — a generational slot id, plus [`AssetIndexAllocator`] to
//!   hand them out and recycle them.
//! - [`AssetId`]/[`UntypedAssetId`] — a typed (and type-erased) asset identity.
//! - [`Handle`]/[`UntypedHandle`] — reference-counted borrows backed by
//!   [`StrongHandle`]. Handles are **not** [`Copy`](core::marker::Copy): the
//!   `Arc` inside a strong handle is the reference count, and the last clone to
//!   drop sends a [`DropEvent`].
//! - [`Asset`]/[`VisitAssetDependencies`] — the trait bound and dependency
//!   visitor every asset participates in.
//!
//! Deliberately absent for now (later tickets): the asset store (`Assets<A>`),
//! `AssetServer`, loaders, paths and meta. This module does not import `tokio`.

mod asset;
mod handle;
mod id;
mod index;

pub use asset::{Asset, VisitAssetDependencies};
pub use handle::{
    AssetHandleProvider, DropEvent, Handle, StrongHandle, UntypedAssetConversionError,
    UntypedHandle,
};
pub use id::{AssetId, UntypedAssetId, UntypedAssetIdConversionError};
pub use index::{AssetIndex, AssetIndexAllocator};

#[cfg(test)]
mod tests;
