//! The engine's asset layer.
//!
//! The generic machinery — [`AssetsServer`](kairos_asset::assets::AssetsServer),
//! [`AssetHandle`](kairos_asset::assets::AssetHandle), the `AssetsSystem`
//! traits — lives in `kairos_asset`; the graphics asset systems (mesh,
//! material, shader, texture, serialized material) live in `kairos_graphics`.
//! This module re-exports both alongside the engine's own systems (audio, text,
//! font, syntax, toml) so `crate::asset_loader::assets::…` remains the single
//! import path inside the engine.

pub mod asset;

pub use kairos_asset::assets::asset::*;
pub use kairos_asset::assets::{
    AssetsServer, DependencyLoadRequest, DependencyLoadRequestEvent, DependencyLoadSetBack,
};

pub use kairos_graphics::assets::{MeshAssetsSystem, TextureAssetsSystem};

pub use asset::{
    AudioAssetHandle, AudioAssetsSystem, AudioExtAssetsSystem, FontAssetsSystem, PcmAssetsSystem,
    SyntaxAssetsSystem, TextAssetsSystem, TomlTableAssetsSystem,
};
