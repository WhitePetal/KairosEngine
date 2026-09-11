//! The graphics asset systems.
//!
//! Every graphics asset type — `Texture`, `Mesh`, `Material`,
//! `SerializedMaterial`, and `ShaderAsset` — now rides the next-generation core
//! in its own module. The legacy per-type `AssetsSystem` stack this module used
//! to expose is gone, so what remains is the historical re-export point for the
//! generic `kairos_asset` machinery.

pub mod asset;

pub use kairos_asset::assets::{
    AssetHandle, AssetsServer, DependencyLoadRequest, DependencyLoadRequestEvent,
    DependencyLoadSetBack,
};
