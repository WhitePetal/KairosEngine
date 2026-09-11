//! The legacy graphics asset stack is gone.
//!
//! `Texture` was the last type still riding the legacy `AssetsSystem` stack; it
//! migrated to the next-generation core in S5, together with the editor's
//! `TextureExt` composite. `Mesh`, `ShaderAsset`, `Material`, and
//! `SerializedMaterial` moved in earlier slices, each into its own module. This
//! module survives only as the historical re-export point for the generic
//! `kairos_asset` machinery.

pub use kairos_asset::assets::asset::*;
