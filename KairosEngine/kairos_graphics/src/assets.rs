//! The graphics asset systems.
//!
//! Together with `kairos_asset`'s generic [`AssetsServer`](kairos_asset::assets::AssetsServer),
//! these load the graphics asset types: [`Mesh`](crate::mesh::Mesh),
//! [`Material`](crate::material::Material),
//! [`SerializedMaterial`](crate::material::SerializedMaterial),
//! [`Texture`](crate::texture::Texture) and
//! [`ShaderAsset`](crate::shader::ShaderAsset).
//!
//! The module re-exports the generic machinery (`AssetHandle`, `AssetsSystem`,
//! `AssetsServer`, …) alongside the graphics systems, so `crate::assets::…`
//! is the one import path for both the handle types components store and the
//! systems that resolve them.

pub mod asset;

pub use asset::{
    MaterialAssetsSystem, MeshAssetsSystem, SerializedMaterialAssetsSystem, ShaderAssetsSystem,
    TextureAssetsSystem,
};

pub use kairos_asset::assets::{
    AssetHandle, AssetsServer, DependencyLoadRequest, DependencyLoadRequestEvent,
    DependencyLoadSetBack,
};
