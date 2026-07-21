use std::path::Path;

use anyhow::{Error, Ok};

use crate::graphics::texture::{
    SerializedTexture,
    format::TextureFormat,
    sampler::{
        AddressMode, AnisotropyLevel, FilterMode, MipmapConfig, MipmapFilter, SamplerConfig,
    },
};

impl SerializedTexture {
    /// Convert a source image file into a `SerializedTexture` + raw RGBA pixel data.
    pub fn convert_img_to_asset(path: &Path) -> Result<(SerializedTexture, Vec<Vec<u8>>), Error> {
        let texture_bytes = std::fs::read(path)?;
        let texture_image = image::load_from_memory(&texture_bytes)?;
        let texture_data = texture_image.into_rgba8();
        let width = texture_data.width();
        let height = texture_data.height();

        let data = vec![texture_data.into_raw()];

        Ok((
            SerializedTexture {
                source_path: path.to_path_buf(),
                width,
                height,
                format: TextureFormat::Rgba8Unorm,
                sampler: SamplerConfig {
                    filter_mode: FilterMode::Linear,
                    address_mode_u: AddressMode::Repeat,
                    address_mode_v: AddressMode::Repeat,
                    address_mode_w: AddressMode::Repeat,
                    mipmap: Some(MipmapConfig {
                        filter: MipmapFilter::Linear,
                        anisotropy_clamp: AnisotropyLevel::Level2.as_u16(),
                        lod_min_clamp: 0.0,
                        lod_max_clamp: (width.max(height) as f32).log2().floor(),
                    }),
                    compare: None,
                    border_color: None,
                },
            },
            data,
        ))
    }

    /// Write the `.texture` TOML and `.texture_bin` companion files.
    pub fn save_to_file(&self, data: &Vec<Vec<u8>>) -> Result<(), Error> {
        let path = &self.source_path;

        // Write .texture_bin (rkyv)
        let bin_path = path.with_extension("texture_bin");
        let bin_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(data)?;
        std::fs::write(&bin_path, bin_bytes)?;

        // Write .texture TOML (data excluded via SerializedTexture having no data field)
        let toml_content = toml::to_string(self)?;
        let toml_path = path.with_extension("texture");
        std::fs::write(&toml_path, toml_content)?;

        Ok(())
    }
}
