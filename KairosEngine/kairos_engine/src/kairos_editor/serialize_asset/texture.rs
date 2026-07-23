use std::path::Path;

use anyhow::{Error, Ok};

use crate::graphics::texture::{
    PixelDatas, SerializedTexture,
    format::TextureFormat,
    sampler::{
        AddressMode, AnisotropyLevel, FilterMode, MipmapConfig, MipmapFilter, SamplerConfig,
    },
};

impl SerializedTexture {
    /// Convert a source image file into a `SerializedTexture` + raw RGBA pixel data.
    pub fn convert_img_to_asset(path: &Path) -> Result<(SerializedTexture, Vec<PixelDatas>), Error> {
        let texture_bytes = std::fs::read(path)?;
        let texture_image = image::load_from_memory(&texture_bytes)?;
        let texture_data = texture_image.into_rgba8();
        let width = texture_data.width();
        let height = texture_data.height();

        let data = vec![PixelDatas::U8(texture_data.into_raw())];

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
    ///
    /// `.texture_bin` uses a custom binary format (no rkyv):
    /// [mip_count: u32 LE]
    /// [mip0_data_len: u32 LE]
    /// [mip0_data: data_len bytes]
    /// [mip1_data_len: u32 LE]
    /// [mip1_data: ...]
    pub fn save_to_file(&self, data: &Vec<PixelDatas>) -> Result<(), Error> {
        let path = &self.source_path;

        // Write .texture_bin (custom format)
        let bin_path = path.with_extension("texture_bin");
        let bin_bytes = serialize_pixel_datas(data);
        std::fs::write(&bin_path, bin_bytes)?;

        // Write .texture TOML (data excluded via SerializedTexture having no data field)
        let toml_content = toml::to_string(self)?;
        let toml_path = path.with_extension("texture");
        std::fs::write(&toml_path, toml_content)?;

        Ok(())
    }
}

/// Serialize `Vec<PixelDatas>` into raw binary.
///
/// Format:
///   [mip_count: u32 LE]
///   For each mip: [byte_len: u32 LE] [data: byte_len bytes]
///
/// No type tags — the `.texture` TOML's `format` field determines the variant.
pub fn serialize_pixel_datas(data: &[PixelDatas]) -> Vec<u8> {
    let mip_count = data.len() as u32;
    let mut buf = Vec::new();
    buf.extend_from_slice(&mip_count.to_le_bytes());
    for level in data {
        let bytes = level.as_bytes();
        let len = bytes.len() as u32;
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(bytes);
    }
    buf
}

/// Deserialize raw binary into `Vec<PixelDatas>`.
///
/// The `format` from `.texture` TOML determines which variant to construct:
///   - U8  → all SDR / compressed formats
///   - F16 → R16F, Rg16F, Rgba16F, BC6h, ASTC HDR
///   - F32 → R32F, Rg32F, Rgba32F
pub fn deserialize_pixel_datas(bytes: &[u8], format: TextureFormat) -> Result<Vec<PixelDatas>, Error> {
    if bytes.len() < 4 {
        return Err(Error::msg("texture_bin too short"));
    }
    let mip_count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if mip_count == 0 || mip_count > 16 {
        return Err(Error::msg(format!("implausible mip_count {mip_count}")));
    }

    let use_f16 = matches!(format,
        TextureFormat::R16Float | TextureFormat::Rg16Float | TextureFormat::Rgba16Float
        | TextureFormat::Bc6hRgbUfloat | TextureFormat::Bc6hRgbFloat
    );
    let use_f32 = matches!(format,
        TextureFormat::R32Float | TextureFormat::Rg32Float | TextureFormat::Rgba32Float
    );

    let mut pos = 4;
    let mut levels = Vec::with_capacity(mip_count);
    for _ in 0..mip_count {
        if pos + 4 > bytes.len() {
            return Err(Error::msg("texture_bin: truncated"));
        }
        let data_len = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]) as usize;
        pos += 4;
        if pos + data_len > bytes.len() {
            return Err(Error::msg("texture_bin: mip data truncated"));
        }
        let raw = &bytes[pos..pos + data_len];
        let level = if use_f16 {
            PixelDatas::F16(bytemuck::cast_slice(raw).to_vec())
        } else if use_f32 {
            PixelDatas::F32(bytemuck::cast_slice(raw).to_vec())
        } else {
            PixelDatas::U8(raw.to_vec())
        };
        levels.push(level);
        pos += data_len;
    }
    Ok(levels)
}
