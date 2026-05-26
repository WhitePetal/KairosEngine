use std::path::Path;

use anyhow::{Error, Ok};

use crate::graphics::texture::{self, TextureAsset};

impl TextureAsset {
    pub fn conver_img_to_asset(path: &Path) -> Result<TextureAsset, Error> {
        let texture_bytes = std::fs::read(path)?;
        let texture_image = image::load_from_memory(&texture_bytes)?;
        let texture_data = texture_image.into_rgba8();
        let width = texture_data.width();
        let height = texture_data.height();

        let data = texture_data.into_raw();

        let texture = crate::graphics::texture::Texture {
            width: width,
            height: height,
            data,
        };
        let meta = texture::Meta {
            source_path: path.to_path_buf(),
        };
        let texture_asset = TextureAsset { meta, texture };
        Ok(texture_asset)
    }

    pub fn save_to_file(&self) -> Result<(), Error> {
        let mut asset = self.clone();

        let path = &asset.meta.source_path;
        let bin_path = path.with_extension("texture_bin");
        let data = &asset.texture.data;
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(data)?;
        let _ = std::fs::write(bin_path, bytes);

        asset.texture.data = vec![];

        let toml = toml::to_string(self)?;
        let path = path.with_extension("texture");

        let _ = std::fs::write(&path, toml);
        Ok(())
    }
}
