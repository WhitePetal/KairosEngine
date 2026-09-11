//! The engine's asset layer.
//!
//! The generic machinery — [`AssetsServer`](kairos_asset::assets::AssetsServer),
//! [`AssetHandle`](kairos_asset::assets::AssetHandle), the `AssetsSystem`
//! traits — lives in `kairos_asset`; the remaining legacy per-type systems
//! (audio, text, font, syntax, toml) live in `kairos_engine`. The migrated
//! graphics and editor types (`Mesh`, material, shader, serialized material,
//! `Texture`, and the editor's `TextureExt`) ride the next-generation core
//! instead. This module re-exports the legacy machinery alongside the engine's
//! own systems so `crate::asset_loader::assets::…` remains the single import
//! path inside the engine.

pub mod asset;

pub use kairos_asset::assets::asset::*;
pub use kairos_asset::assets::{
    AssetsServer, DependencyLoadRequest, DependencyLoadRequestEvent, DependencyLoadSetBack,
};

pub use asset::{
    AudioAssetHandle, AudioAssetsSystem, AudioExtAssetsSystem, FontAssetsSystem, PcmAssetsSystem,
    SyntaxAssetsSystem, TextAssetsSystem, TomlTableAssetsSystem,
};
