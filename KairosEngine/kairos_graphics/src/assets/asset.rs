mod material;
mod mesh;
mod serialized_material;
mod shader;
mod texture;

pub use kairos_asset::assets::asset::*;

pub use material::MaterialAssetsSystem;
pub use mesh::MeshAssetsSystem;
pub use serialized_material::SerializedMaterialAssetsSystem;
pub use shader::ShaderAssetsSystem;
pub use texture::TextureAssetsSystem;
