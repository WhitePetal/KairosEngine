//! The texture inspector's loaded edit.
//!
//! After the `.meta` migration (ADR 0002) a texture is a source image (`.png`)
//! plus a `.meta` sidecar whose `AssetAction::Process` carries the editable
//! [`TextureSettings`]; the pixel data is produced by the processor under
//! `imported_assets/Default`. The inspector needs the settings, the source's
//! original pixels (to resize from), and a preview, so [`TextureEdit`] bundles
//! them.
//!
//! It is loaded off the UI thread but is deliberately *not* an asset: nothing
//! else references it, and the preview never belongs in `Assets<Texture>` — the
//! inspector builds its egui texture straight from the pixels. The product
//! itself is written only by the processor, never by the editor.

use std::path::{Path, PathBuf};

use crate::asset::io::get_meta_path;
use crate::asset::{AssetAction, AssetMeta};

use crate::graphics::texture::{PixelDatas, Texture, TextureSettings};

/// The loaded editor view of one texture source: the editable settings, the
/// source image's original pixels, and the preview to show.
#[derive(Debug, Clone)]
pub struct TextureEdit {
    /// Path to the source image (e.g. PNG), relative to the asset root.
    pub source_path: PathBuf,
    /// Editable processor settings, mirrored from the source's `.meta`.
    pub settings: TextureSettings,
    /// The preview: the source converted with `settings`, through the same
    /// algorithm the processor's product uses.
    pub texture: Texture,
    /// Original image width from the source.
    pub original_width: u32,
    /// Original image height from the source.
    pub original_height: u32,
    /// Cached original RGBA pixel data (for resizing on Apply).
    pub original_rgba: Vec<u8>,
}

/// Reads `source_path`'s `.meta` settings and image, and builds the preview.
///
/// Missing or unreadable inputs are not fatal: the settings fall back to
/// [`TextureSettings::default`] and the preview to a 1x1 placeholder.
pub(super) async fn load_texture_edit(source_path: PathBuf) -> TextureEdit {
    let settings = read_settings(&source_path).await;
    let source_bytes = async_fs::read(&source_path).await.ok();

    let (original_width, original_height, original_rgba) = match source_bytes
        .as_deref()
        .and_then(|bytes| image::load_from_memory(bytes).ok())
    {
        Some(image) => (
            image.width(),
            image.height(),
            image.clone().into_rgba8().into_vec(),
        ),
        None => {
            log::warn!(
                "TextureEdit: failed to read or decode source image '{}'",
                source_path.display()
            );
            (0, 0, Vec::new())
        }
    };

    let (resolved, data) = match source_bytes.as_deref() {
        Some(bytes) => match TextureSettings::convert_source(bytes, &settings) {
            Ok(converted) => converted,
            Err(error) => {
                log::warn!(
                    "TextureEdit: failed to convert '{}': {error}",
                    source_path.display()
                );
                (settings.clone(), placeholder())
            }
        },
        None => (settings.clone(), placeholder()),
    };

    TextureEdit {
        source_path,
        settings,
        texture: Texture {
            width: resolved.width,
            height: resolved.height,
            format: resolved.format,
            data,
            sampler: resolved.sampler,
        },
        original_width,
        original_height,
        original_rgba,
    }
}

/// Writes `settings` to the source's `.meta` as an `AssetAction::Process`.
///
/// The editor never writes the product: it records the [`TextureSettings`]
/// beside the source image, and the asset processor — when the host runs in
/// layout ② — regenerates the processed bytes from them.
pub(super) fn write_settings_meta(
    source_path: &Path,
    settings: &TextureSettings,
) -> Result<(), Box<dyn std::error::Error>> {
    let meta = AssetMeta::<(), TextureSettings>::new(AssetAction::Process {
        processor: crate::asset::meta::processor_name::<crate::graphics::texture::TextureProcessor>()
            .to_string(),
        settings: settings.clone(),
    });
    std::fs::write(
        get_meta_path(source_path),
        crate::asset::AssetMetaDyn::serialize(&meta),
    )?;
    Ok(())
}

/// The settings a source's `.meta` carries, or the default when it is missing,
/// not a `Process` action, or unreadable.
async fn read_settings(source_path: &Path) -> TextureSettings {
    let Ok(bytes) = async_fs::read(get_meta_path(source_path)).await else {
        return TextureSettings::default();
    };
    match AssetMeta::<(), TextureSettings>::deserialize(&bytes) {
        Ok(meta) => match meta.asset {
            AssetAction::Process { settings, .. } => settings,
            _ => TextureSettings::default(),
        },
        Err(error) => {
            log::warn!("TextureEdit: unreadable `.meta`, using defaults: {error}");
            TextureSettings::default()
        }
    }
}

/// A 1x1 white placeholder for when the source cannot be read.
fn placeholder() -> Vec<PixelDatas> {
    vec![PixelDatas::U8(vec![255; 4])]
}

#[cfg(test)]
mod test {
    use crate::graphics::texture::{
        TextureFormat,
        sampler::{AddressMode, FilterMode, SamplerConfig},
    };

    use super::{load_texture_edit, write_settings_meta};

    fn nearest_sampler() -> SamplerConfig {
        SamplerConfig {
            filter_mode: FilterMode::Nearest,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mipmap: None,
            compare: None,
            border_color: None,
        }
    }

    /// The edit loads the settings stored in the source's `.meta` and a preview
    /// sized from the source image.
    #[test]
    fn loads_the_settings_and_preview_from_the_source_meta() {
        let dir = tempfile::Builder::new()
            .tempdir_in(".")
            .expect("a temp dir in the cwd");
        let source_path = dir.path().join("Probe.png");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]))
            .save(&source_path)
            .expect("write the source png");

        write_settings_meta(&source_path, &settings()).expect("write the source meta");

        let edit = pollster::block_on(load_texture_edit(source_path));

        assert_eq!((edit.original_width, edit.original_height), (2, 2));
        assert_eq!((edit.texture.width, edit.texture.height), (2, 2));
        assert_eq!(edit.settings.format, TextureFormat::Rgba8Unorm);
        assert_eq!(edit.settings.sampler.filter_mode, FilterMode::Nearest);
    }

    fn settings() -> crate::graphics::texture::TextureSettings {
        crate::graphics::texture::TextureSettings {
            width: 0,
            height: 0,
            format: TextureFormat::Rgba8Unorm,
            sampler: nearest_sampler(),
        }
    }
}
