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

/// Serialize `Vec<PixelDatas>` into a custom binary buffer.
///
/// Format:
/// [mip_count: u32 LE]
/// For each mip level:
///   [variant_tag: 1 byte]  — 0=U8, 1=F16, 2=F32
///   [data_len: u32 LE]  — byte length of the raw pixel data
///   [data: data_len bytes] — raw bytes
pub fn serialize_pixel_datas(data: &[PixelDatas]) -> Vec<u8> {
    let mip_count = data.len() as u32;
    let mut buf = Vec::new();
    buf.extend_from_slice(&mip_count.to_le_bytes());
    for level in data {
        match level {
            PixelDatas::U8(bytes) => {
                buf.push(0); // variant tag: U8
                let len = bytes.len() as u32;
                buf.extend_from_slice(&len.to_le_bytes());
                buf.extend_from_slice(bytes);
            }
            PixelDatas::F16(values) => {
                buf.push(1); // variant tag: F16
                let bytes: &[u8] = bytemuck::cast_slice(values);
                let len = bytes.len() as u32;
                buf.extend_from_slice(&len.to_le_bytes());
                buf.extend_from_slice(bytes);
            }
            PixelDatas::F32(values) => {
                buf.push(2); // variant tag: F32
                let bytes: &[u8] = bytemuck::cast_slice(values);
                let len = bytes.len() as u32;
                buf.extend_from_slice(&len.to_le_bytes());
                buf.extend_from_slice(bytes);
            }
        }
    }
    buf
}

/// Deserialize a custom binary buffer into `Vec<PixelDatas>`.
///
/// Format is self-describing via per-level variant tags.
/// `format` parameter is a fallback for legacy data without tags.
pub fn deserialize_pixel_datas(bytes: &[u8], format: TextureFormat) -> Result<Vec<PixelDatas>, Error> {
    if bytes.len() < 4 {
        return Err(Error::msg("texture_bin file too short"));
    }
    let mip_count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let mut pos = 4;
    let mut levels = Vec::with_capacity(mip_count);

    // Determine fallback variant (used for legacy data without tags).
    let fallback_is_f16 = is_half_float_format(format);
    let fallback_is_f32 = is_full_float_format(format);

    for _ in 0..mip_count {
        if pos >= bytes.len() {
            return Err(Error::msg("texture_bin: unexpected end of data"));
        }
        // Peek: if next byte looks like a variant tag (0-2), consume it.
        // Legacy format starts with data_len (u32 LE > 2), so this is
        // backward-compatible.
        let has_tag = matches!(bytes[pos], 0 | 1 | 2);
        let variant: u8 = if has_tag {
            pos += 1;
            bytes[pos - 1]
        } else {
            // Legacy: infer from TextureFormat.
            if fallback_is_f16 { 1 } else if fallback_is_f32 { 2 } else { 0 }
        };

        if pos + 4 > bytes.len() {
            return Err(Error::msg("texture_bin: unexpected end of data"));
        }
        let data_len = u32::from_le_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]) as usize;
        pos += 4;
        if pos + data_len > bytes.len() {
            return Err(Error::msg("texture_bin: mip level data truncated"));
        }
        let raw = &bytes[pos..pos + data_len];
        let level = match variant {
            1 => {
                let values: Vec<half::f16> = bytemuck::cast_slice(raw).to_vec();
                PixelDatas::F16(values)
            }
            2 => {
                let values: Vec<f32> = bytemuck::cast_slice(raw).to_vec();
                PixelDatas::F32(values)
            }
            _ => {
                PixelDatas::U8(raw.to_vec())
            }
        };
        levels.push(level);
        pos += data_len;
    }
    Ok(levels)
}

fn is_half_float_format(format: TextureFormat) -> bool {
    matches!(
        format,
        TextureFormat::R16Float
            | TextureFormat::Rg16Float
            | TextureFormat::Rgba16Float
            | TextureFormat::Bc6hRgbUfloat
            | TextureFormat::Bc6hRgbFloat
    )
}

fn is_full_float_format(format: TextureFormat) -> bool {
    matches!(
        format,
        TextureFormat::R32Float
            | TextureFormat::Rg32Float
            | TextureFormat::Rgba32Float
    )
}
