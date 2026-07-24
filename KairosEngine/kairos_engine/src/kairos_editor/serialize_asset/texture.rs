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
    /// Convert a source image file into a `SerializedTexture` + raw pixel data.
    ///
    /// Auto-detects HDR / high-bit-depth images and routes them to the f16
    /// half-float pipeline. SDR 8-bit images go through the existing u8 path.
    pub fn convert_img_to_asset(
        path: &Path,
    ) -> Result<(SerializedTexture, Vec<PixelDatas>), Error> {
        let texture_bytes = std::fs::read(path)?;
        let texture_image = image::load_from_memory(&texture_bytes)?;

        match &texture_image {
            // HDR / high-bit-depth → f16 half-float pipeline
            image::DynamicImage::ImageRgb32F(_)
            | image::DynamicImage::ImageRgba32F(_)
            | image::DynamicImage::ImageLuma16(_)
            | image::DynamicImage::ImageLumaA16(_)
            | image::DynamicImage::ImageRgb16(_)
            | image::DynamicImage::ImageRgba16(_) => Self::convert_hdr(texture_image, path),

            // SDR 8-bit → u8 pipeline
            _ => Self::convert_sdr(texture_image, path),
        }
    }

    /// SDR path: 8-bit images → u8 mip chain → `PixelDatas::U8`.
    fn convert_sdr(
        img: image::DynamicImage,
        path: &Path,
    ) -> Result<(SerializedTexture, Vec<PixelDatas>), Error> {
        let texture_data = img.into_rgba8();
        let width = texture_data.width();
        let height = texture_data.height();

        let mip_count = (width.max(height) as f32).log2().floor() as u32;
        let raw = texture_data.into_raw();
        let mut data: Vec<PixelDatas> = Vec::with_capacity(mip_count as usize);
        let mut current_rgba = raw;
        let mut current_w = width;
        let mut current_h = height;

        for _ in 0..mip_count {
            data.push(PixelDatas::U8(current_rgba.clone()));
            let (pw, ph) = (current_w, current_h);
            current_w = (current_w / 2).max(1);
            current_h = (current_h / 2).max(1);
            if let Some(source) = image::RgbaImage::from_raw(pw, ph, current_rgba) {
                current_rgba = image::imageops::resize(
                    &source,
                    current_w,
                    current_h,
                    image::imageops::FilterType::Lanczos3,
                )
                .into_vec();
            } else {
                break;
            }
        }

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
                        lod_max_clamp: mip_count as f32,
                    }),
                    compare: None,
                    border_color: None,
                },
            },
            data,
        ))
    }

    /// HDR path: f32 / 16-bit images → f32 mip chain → `PixelDatas::F16`.
    ///
    /// F32 sources (EXR, Radiance HDR) preserve their original float range.
    /// 16-bit integer sources (PNG, TIFF) are normalized to 0–1 before f16
    /// conversion to avoid precision loss near f16's max (~65504).
    fn convert_hdr(
        img: image::DynamicImage,
        path: &Path,
    ) -> Result<(SerializedTexture, Vec<PixelDatas>), Error> {
        // Detect u16 source so we can normalize before f16 conversion.
        let needs_normalize = matches!(
            &img,
            image::DynamicImage::ImageLuma16(_)
                | image::DynamicImage::ImageLumaA16(_)
                | image::DynamicImage::ImageRgb16(_)
                | image::DynamicImage::ImageRgba16(_)
        );

        let texture_data = img.into_rgba32f();
        let width = texture_data.width();
        let height = texture_data.height();

        let mip_count = (width.max(height) as f32).log2().floor() as u32;
        let raw = texture_data.into_raw();
        let mut data: Vec<PixelDatas> = Vec::with_capacity(mip_count as usize);
        let mut current = raw;
        let mut current_w = width;
        let mut current_h = height;

        for _ in 0..mip_count {
            // f32 → f16, normalizing u16-sourced values to 0–1
            let f16: Vec<half::f16> = current
                .iter()
                .map(|&v| {
                    let v = if needs_normalize {
                        (v / 65535.0).clamp(0.0, 1.0)
                    } else {
                        v
                    };
                    half::f16::from_f32(v)
                })
                .collect();
            data.push(PixelDatas::F16(f16));

            let (pw, ph) = (current_w, current_h);
            current_w = (current_w / 2).max(1);
            current_h = (current_h / 2).max(1);

            if let Some(source) = image::Rgba32FImage::from_raw(pw, ph, current) {
                current = image::imageops::resize(
                    &source,
                    current_w,
                    current_h,
                    image::imageops::FilterType::Lanczos3,
                )
                .into_raw();
            } else {
                break;
            }
        }

        Ok((
            SerializedTexture {
                source_path: path.to_path_buf(),
                width,
                height,
                format: TextureFormat::Rgba16Float,
                sampler: SamplerConfig {
                    filter_mode: FilterMode::Linear,
                    address_mode_u: AddressMode::Repeat,
                    address_mode_v: AddressMode::Repeat,
                    address_mode_w: AddressMode::Repeat,
                    mipmap: Some(MipmapConfig {
                        filter: MipmapFilter::Linear,
                        anisotropy_clamp: AnisotropyLevel::Level2.as_u16(),
                        lod_min_clamp: 0.0,
                        lod_max_clamp: mip_count as f32,
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
    /// `.texture_bin` uses a headerless format:
    /// each mip level's raw encoded bytes are concatenated back-to-back.
    /// The number of levels and per-level byte boundaries are computed at
    /// load time from the `.texture` TOML's width, height, format,
    /// lod_min_clamp and lod_max_clamp.
    pub fn save_to_file(&self, data: &Vec<PixelDatas>) -> Result<(), Error> {
        let path = &self.source_path;

        // Write .texture_bin (custom format)
        let bin_path = path.with_extension("texture_bin");
        let bin_bytes = Self::serialize_pixel_datas(data);
        std::fs::write(&bin_path, bin_bytes)?;

        // Write .texture TOML (data excluded via SerializedTexture having no data field)
        let toml_content = toml::to_string(self)?;
        let toml_path = path.with_extension("texture");
        std::fs::write(&toml_path, toml_content)?;

        Ok(())
    }

    /// Serialize `Vec<PixelDatas>` into raw binary.
    ///
    /// Format:
    ///   Raw bytes of each mip level are concatenated — **no headers**.
    ///   The deserializer recomputes per-level byte boundaries from
    ///   the `.texture` TOML metadata (width, height, format,
    ///   lod_min_clamp, lod_max_clamp).
    ///
    /// No type tags — the `.texture` TOML's `format` field determines the variant.
    pub fn serialize_pixel_datas(data: &[PixelDatas]) -> Vec<u8> {
        let mut buf = Vec::new();
        for level in data {
            buf.extend_from_slice(level.as_bytes());
        }
        buf
    }

    /// Deserialize raw binary into `Vec<PixelDatas>`.
    ///
    /// The mip count and per-level byte boundaries are computed from the
    /// `.texture` TOML metadata — the binary itself has no headers.
    ///
    /// The `format` determines which variant to construct:
    ///   - U8  → all SDR / compressed formats
    ///   - F16 → R16F, Rg16F, Rgba16F, BC6h, ASTC HDR
    ///   - F32 → R32F, Rg32F, Rgba32F
    pub fn deserialize_pixel_datas(
        bytes: &[u8],
        width: u32,
        height: u32,
        lod_max_clamp: f32,
        format: TextureFormat,
    ) -> Result<Vec<PixelDatas>, Error> {
        let mip_count = format.stored_mip_count(width, height, lod_max_clamp);

        let mut pos = 0;
        let mut levels = Vec::with_capacity(mip_count);
        for level_idx in 0..mip_count {
            let expected_len = format.mip_level_byte_count(width, height, level_idx as u32);
            if pos + expected_len > bytes.len() {
                return Err(Error::msg(format!(
                    "texture_bin: mip level {level_idx} truncated (need {expected_len} bytes, have {})",
                    bytes.len() - pos
                )));
            }
            let raw = &bytes[pos..pos + expected_len];
            levels.push(format.pixel_datas_from_raw(raw));
            pos += expected_len;
        }
        if pos != bytes.len() {
            return Err(Error::msg(format!(
                "texture_bin: {} trailing bytes after last mip level",
                bytes.len() - pos
            )));
        }
        Ok(levels)
    }
}
