use anyhow::{Error, Ok};
use half::f16;

use crate::texture::{PixelDatas, TextureSettings, format};

/// The decode/mip/encode algorithm shared by the texture processor.
///
/// This is the retained manual pipeline — the former `convert_img_to_asset` plus
/// the inspector's encode loop — with no file IO: it takes the source image
/// bytes the processor read and returns the encoded mip chain plus the resolved
/// [`TextureSettings`] its output loader should be configured with.
impl TextureSettings {
    /// Decodes `bytes` and produces the encoded mip chain `format` prescribes.
    ///
    /// A zero `width`/`height` keeps the source image's dimension. SDR 8-bit
    /// sources take the u8 path; HDR / high-bit-depth sources take the f16 path,
    /// matching the loader's [`deserialize_pixel_datas`](Self::deserialize_pixel_datas).
    pub fn convert_source(
        bytes: &[u8],
        settings: &TextureSettings,
    ) -> Result<(TextureSettings, Vec<PixelDatas>), Error> {
        let image = image::load_from_memory(bytes)?;
        let width = if settings.width == 0 {
            image.width()
        } else {
            settings.width
        };
        let height = if settings.height == 0 {
            image.height()
        } else {
            settings.height
        };

        let data = if is_hdr_source(&image) {
            encode_f16_chain(
                image,
                width,
                height,
                settings.format,
                settings.sampler.mipmap.as_ref(),
            )
        } else {
            encode_u8_chain(
                image,
                width,
                height,
                settings.format,
                settings.sampler.mipmap.as_ref(),
            )
        };

        Ok((
            TextureSettings {
                width,
                height,
                format: settings.format,
                sampler: settings.sampler.clone(),
            },
            data,
        ))
    }

    /// Serializes a mip chain into the headerless product bytes.
    ///
    /// Raw bytes of each mip level are concatenated with **no headers**; the
    /// loader recomputes the per-level boundaries from its settings. No type tags
    /// — the format determines the variant.
    pub fn serialize_pixel_datas(data: &[PixelDatas]) -> Vec<u8> {
        let mut buf = Vec::new();
        for level in data {
            buf.extend_from_slice(level.as_bytes());
        }
        buf
    }

    /// Deserializes raw product bytes back into a mip chain.
    ///
    /// The mip count and per-level byte boundaries are computed from
    /// `width`/`height`/`lod_max_clamp`/`format` — the binary itself has no
    /// headers.
    pub fn deserialize_pixel_datas(
        bytes: &[u8],
        width: u32,
        height: u32,
        lod_max_clamp: f32,
        format: format::TextureFormat,
    ) -> Result<Vec<PixelDatas>, Error> {
        let mip_count = format.stored_mip_count(width, height, lod_max_clamp);

        let mut pos = 0;
        let mut levels = Vec::with_capacity(mip_count);
        for level_idx in 0..mip_count {
            let expected_len = format.mip_level_byte_count(width, height, level_idx as u32);
            if pos + expected_len > bytes.len() {
                return Err(Error::msg(format!(
                    "texture product: mip level {level_idx} truncated (need {expected_len} bytes, have {})",
                    bytes.len() - pos
                )));
            }
            let raw = &bytes[pos..pos + expected_len];
            levels.push(format.pixel_datas_from_raw(raw));
            pos += expected_len;
        }
        if pos != bytes.len() {
            return Err(Error::msg(format!(
                "texture product: {} trailing bytes after last mip level",
                bytes.len() - pos
            )));
        }
        Ok(levels)
    }
}

/// Whether the decoded image is a float / 16-bit source that takes the f16 path.
fn is_hdr_source(image: &image::DynamicImage) -> bool {
    matches!(
        image,
        image::DynamicImage::ImageRgb32F(_)
            | image::DynamicImage::ImageRgba32F(_)
            | image::DynamicImage::ImageLuma16(_)
            | image::DynamicImage::ImageLumaA16(_)
            | image::DynamicImage::ImageRgb16(_)
            | image::DynamicImage::ImageRgba16(_)
    )
}

/// The last mip level to store, per the inspector's loop: `lod_max_clamp`
/// bounded by the deepest level the dimensions allow.
fn end_level(
    width: u32,
    height: u32,
    mipmap: Option<&crate::texture::sampler::MipmapConfig>,
) -> u32 {
    let Some(mipmap) = mipmap else {
        return 0;
    };
    let max_possible = (width.max(height) as f32).log2().floor() as u32;
    (mipmap.lod_max_clamp.floor() as u32).min(max_possible)
}

/// SDR path: 8-bit RGBA intermediate, encoded per level.
fn encode_u8_chain(
    image: image::DynamicImage,
    width: u32,
    height: u32,
    format: format::TextureFormat,
    mipmap: Option<&crate::texture::sampler::MipmapConfig>,
) -> Vec<PixelDatas> {
    let mut rgba = image.into_rgba8();
    if (width, height) != (rgba.width(), rgba.height()) {
        rgba = image::imageops::resize(&rgba, width, height, image::imageops::FilterType::Lanczos3);
    }

    let (block_w, block_h) = format.block_dimensions();
    let mut levels = Vec::new();
    let mut current = rgba.into_raw();
    let mut current_w = width;
    let mut current_h = height;

    for _ in 0..=end_level(width, height, mipmap) {
        if current_w < block_w || current_h < block_h {
            break;
        }
        let pixels = PixelDatas::U8(current.clone());
        levels.push(format::encode(&pixels, current_w, current_h, format));

        let (previous_w, previous_h) = (current_w, current_h);
        current_w = (current_w / 2).max(1);
        current_h = (current_h / 2).max(1);
        if let Some(source) = image::RgbaImage::from_raw(previous_w, previous_h, current) {
            current = image::imageops::resize(
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

    levels
}

/// HDR path: f32 RGBA intermediate, normalized when the source was 16-bit
/// integer, encoded per level.
fn encode_f16_chain(
    image: image::DynamicImage,
    width: u32,
    height: u32,
    format: format::TextureFormat,
    mipmap: Option<&crate::texture::sampler::MipmapConfig>,
) -> Vec<PixelDatas> {
    let needs_normalize = matches!(
        &image,
        image::DynamicImage::ImageLuma16(_)
            | image::DynamicImage::ImageLumaA16(_)
            | image::DynamicImage::ImageRgb16(_)
            | image::DynamicImage::ImageRgba16(_)
    );

    let mut rgba = image.into_rgba32f();
    if (width, height) != (rgba.width(), rgba.height()) {
        rgba = image::imageops::resize(&rgba, width, height, image::imageops::FilterType::Lanczos3);
    }

    let (block_w, block_h) = format.block_dimensions();
    let mut levels = Vec::new();
    let mut current: Vec<f32> = rgba
        .into_raw()
        .into_iter()
        .map(|value| {
            if needs_normalize {
                (value / 65535.0).clamp(0.0, 1.0)
            } else {
                value
            }
        })
        .collect();
    let mut current_w = width;
    let mut current_h = height;

    for _ in 0..=end_level(width, height, mipmap) {
        if current_w < block_w || current_h < block_h {
            break;
        }
        let halves: Vec<f16> = current.iter().map(|&value| f16::from_f32(value)).collect();
        let pixels = PixelDatas::F16(halves);
        levels.push(format::encode(&pixels, current_w, current_h, format));

        let (previous_w, previous_h) = (current_w, current_h);
        current_w = (current_w / 2).max(1);
        current_h = (current_h / 2).max(1);
        if let Some(source) = image::Rgba32FImage::from_raw(previous_w, previous_h, current) {
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

    levels
}
