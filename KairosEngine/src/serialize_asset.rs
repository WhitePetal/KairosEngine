use serde::{Deserialize, Serialize};

use crate::graphics::texture::Texture;

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta {
    pub source_path: String
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextureAsset {
    pub meta: Meta,
    pub texture: Texture
}

// let texture_bytes = std::fs::read("res/textures/kairos_texture.png").unwrap();
// let texture_image = image::load_from_memory(&texture_bytes).unwrap();
// let texture_data = texture_image.into_rgba8();
// let width = texture_data.width();
// let height = texture_data.height();

// let texture = crate::graphics::texture::Texture { 
//     data: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, texture_data.into_raw()), 
//     width: width, 
//     height: height 
// };
// let texture_asset = crate::serialize_asset::TextureAsset {
//     meta: crate::serialize_asset::Meta {
//         source_path: "res/textures/kairos_texture.texture".into()
//     },
//     texture: texture
// };
// let toml = toml::to_string(&texture_asset).unwrap();
// let _ = std::fs::write("res/textures/kairos_texture.texture", toml);