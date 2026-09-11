//! Async asset server and handle system for the Kairos engine.
//!
//! This is the concrete-asset-agnostic half of the engine's asset loader: the
//! [`AssetsServer`](assets::AssetsServer) that owns one handler per asset
//! system, the [`AssetHandle`](assets::AssetHandle) ref-counted handle, and the
//! traits ([`AssetsSystem`](assets::asset::AssetsSystem),
//! [`AssetLoader`](assets::asset::AssetLoader)) an asset system implements.
//!
//! Concrete asset systems live next to their asset types — the legacy graphics
//! ones (`MeshAssetsSystem`, `TextureAssetsSystem`) in `kairos_graphics`, the
//! audio/text/syntax/toml ones in `kairos_engine`. Only the generic machinery
//! lives here, so it is free of any dependency on graphics or engine types.
//!
//! The [`next`] module is the landing zone for the `bevy_asset`-style rewrite of
//! this system. It is additive and unconsumed for now; the stack above it is the
//! legacy implementation that still drives the engine.

pub mod assets;
pub mod consts;
pub mod next;
