// Only `Mesh` and `Texture` still ride the legacy `AssetsSystem` stack: they are
// migrated in later asset slices (S4/S5). The `ShaderAsset`, `Material`, and
// `SerializedMaterial` systems that used to live here are gone — those types now
// ride the next-generation core (`crate::shader`, `crate::material`).
mod mesh;
mod texture;

pub use kairos_asset::assets::asset::*;

pub use mesh::MeshAssetsSystem;
pub use texture::TextureAssetsSystem;
