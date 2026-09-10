//! Async asset server and handle system for the Kairos engine.
//!
//! This is the concrete-asset-agnostic half of the engine's asset loader: the
//! [`AssetsServer`](assets::AssetsServer) that owns one handler per asset
//! system, the [`AssetHandle`](assets::AssetHandle) ref-counted handle, and the
//! traits ([`AssetsSystem`](assets::asset::AssetsSystem),
//! [`AssetLoader`](assets::asset::AssetLoader)) an asset system implements.
//!
//! Concrete asset systems live next to their asset types — the graphics ones
//! (`MeshAssetsSystem`, `MaterialAssetsSystem`, …) in `kairos_graphics`, the
//! audio/text/syntax/toml ones in `kairos_engine`. Only the generic machinery
//! lives here, so it is free of any dependency on graphics or engine types.

pub mod assets;
pub mod consts;
