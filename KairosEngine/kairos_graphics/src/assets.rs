//! The graphics asset systems.
//!
//! Together with `kairos_asset`'s generic [`AssetsServer`](kairos_asset::assets::AssetsServer),
//! these load the remaining legacy graphics asset type,
//! [`Texture`](crate::texture::Texture). The `Mesh`, `Material`,
//! [`SerializedMaterial`](crate::material::SerializedMaterial), and
//! [`ShaderAsset`](crate::shader::ShaderAsset) types now ride the
//! next-generation core in their own modules.
//!
//! The module re-exports the generic machinery (`AssetHandle`, `AssetsSystem`,
//! `AssetsServer`, …) alongside the graphics systems, so `crate::assets::…`
//! is the one import path for both the handle types components store and the
//! systems that resolve them.

pub mod asset;

pub use asset::TextureAssetsSystem;

pub use kairos_asset::assets::{
    AssetHandle, AssetsServer, DependencyLoadRequest, DependencyLoadRequestEvent,
    DependencyLoadSetBack,
};
