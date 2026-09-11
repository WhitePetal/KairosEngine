// Only `Texture` still rides the legacy `AssetsSystem` stack: it is migrated in
// a later asset slice (S5). The `Mesh`, `ShaderAsset`, `Material`, and
// `SerializedMaterial` systems that used to live here are gone — those types now
// ride the next-generation core (`crate::mesh`, `crate::shader`,
// `crate::material`).
mod texture;

pub use kairos_asset::assets::asset::*;

pub use texture::TextureAssetsSystem;
