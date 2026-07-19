//! Texture compression encoding — converts RGBA8 pixel data into
//! GPU texture formats for `.texture_bin` persistence.
//!
//! ## TODO
//! - BC6h, BC7 encoding
//! - ETC2 / EAC encoding
//! - ASTC encoding
//! - Non-RGBA uncompressed format conversion (channel swizzling)

use crate::graphics::texture_format::TextureFormat;

/// Encode RGBA8 pixel data into `format`.
///
/// Returns `None` when the format is not yet supported for encoding.
pub fn encode_rgba(rgba: &[u8], width: u32, height: u32, format: TextureFormat) -> Option<Vec<u8>> {
    let w = width as usize;
    let h = height as usize;

    match format {
        // Pass-through uncompressed (RGBA8 byte layout identical)
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => Some(rgba.to_vec()),

        // BC1-5 via texpresso
        TextureFormat::Bc1RgbaUnorm
        | TextureFormat::Bc1RgbaUnormSrgb
        | TextureFormat::Bc2RgbaUnorm
        | TextureFormat::Bc2RgbaUnormSrgb
        | TextureFormat::Bc3RgbaUnorm
        | TextureFormat::Bc3RgbaUnormSrgb
        | TextureFormat::Bc4RUnorm
        | TextureFormat::Bc4RSnorm
        | TextureFormat::Bc5RgUnorm
        | TextureFormat::Bc5RgSnorm => {
            let tf = match format {
                TextureFormat::Bc1RgbaUnorm | TextureFormat::Bc1RgbaUnormSrgb => {
                    texpresso::Format::Bc1
                }
                TextureFormat::Bc2RgbaUnorm | TextureFormat::Bc2RgbaUnormSrgb => {
                    texpresso::Format::Bc2
                }
                TextureFormat::Bc3RgbaUnorm | TextureFormat::Bc3RgbaUnormSrgb => {
                    texpresso::Format::Bc3
                }
                TextureFormat::Bc4RUnorm | TextureFormat::Bc4RSnorm => texpresso::Format::Bc4,
                _ => texpresso::Format::Bc5,
            };

            let mut output = vec![0u8; tf.compressed_size(w, h)];
            tf.compress(rgba, w, h, texpresso::Params::default(), &mut output);
            Some(output)
        }

        // TODO: BC6h, BC7, ETC2, EAC, ASTC, non-RGBA uncompressed
        _ => None,
    }
}
