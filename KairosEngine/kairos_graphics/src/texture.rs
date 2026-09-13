use futures_lite::AsyncWriteExt;
use kairos_asset::{
    Asset, AssetLoader, AssetProcessor, AssetWorldExt, LoadContext, Process, ProcessContext,
    ProcessError, Reader, VisitAssetDependencies, Writer,
};
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;
use serde::{Deserialize, Serialize};

use crate::consts::TEXTURE_ASSETS_CAPACITY;

pub mod format;
pub mod sampler;

mod serialize;

#[cfg(test)]
mod test;

pub use format::{PixelDatas, TextureFormat};
use sampler::SamplerConfig;

/// Power-of-two size presets for texture dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumIter)]
pub enum TextureMaxSize {
    Size2 = 2,
    Size4 = 4,
    Size8 = 8,
    Size16 = 16,
    Size32 = 32,
    Size64 = 64,
    Size128 = 128,
    Size256 = 256,
    Size512 = 512,
    Size1024 = 1024,
    Size2048 = 2048,
    Size4096 = 4096,
}

impl TextureMaxSize {
    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

/// Pick the smallest `TextureMaxSize` >= `max(width, height)`.
pub fn find_texture_max_size(width: u32, height: u32) -> TextureMaxSize {
    use strum::IntoEnumIterator;
    let max_side = width.max(height);
    for size in TextureMaxSize::iter() {
        if size.as_u32() >= max_side {
            return size;
        }
    }
    TextureMaxSize::Size4096
}

/// The settings of a texture's processing and loading.
///
/// The same value is both [`TextureProcessor::Settings`] — the per-asset
/// configuration carried in the source `.meta` and editable by the inspector —
/// and [`TextureLoader::Settings`] — the resolved description of the product,
/// written into the processed `.meta`. A zero `width`/`height` means "keep the
/// source image's dimension"; the processor resolves it before writing the
/// product, so the loader always sees concrete dimensions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextureSettings {
    /// Output width in pixels; `0` keeps the source image's width.
    pub width: u32,
    /// Output height in pixels; `0` keeps the source image's height.
    pub height: u32,
    /// GPU texture format the product is encoded in.
    pub format: TextureFormat,
    /// Sampler configuration (filter, wrap, mipmap, etc.).
    pub sampler: SamplerConfig,
}

impl Default for TextureSettings {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            format: TextureFormat::Rgba8Unorm,
            sampler: SamplerConfig::default(),
        }
    }
}

/// Runtime form held by [`Assets<Texture>`](kairos_asset::Assets).
///
/// Contains the resolved dimensions, the pixel data loaded from the product,
/// and the sampler configuration. `data` is a mip-chain: `data[0]` = base
/// level, `data[1..]` = coarser levels.
#[derive(Debug, Clone)]
pub struct Texture {
    /// Texture width in pixels.
    pub width: u32,
    /// Texture height in pixels.
    pub height: u32,
    /// GPU texture format.
    pub format: TextureFormat,
    /// Pixel data per mip level. `data[0]` = base level.
    pub data: Vec<PixelDatas>,
    /// Sampler configuration (filter, wrap, mipmap, etc.).
    pub sampler: SamplerConfig,
}

impl Asset for Texture {}
impl VisitAssetDependencies for Texture {}

/// Loads the **processed** [`Texture`] produced by [`TextureProcessor`].
///
/// This is the processor's [`OutputLoader`](Process::OutputLoader): the product
/// is the concatenated mip chain, and the dimensions, format, and sampler live
/// in the settings carried by the product's `.meta`; the loader needs no
/// descriptor file and no companion.
#[derive(Debug)]
pub struct TextureLoader;

impl AssetLoader for TextureLoader {
    type Asset = Texture;
    type Settings = TextureSettings;
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &TextureSettings,
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Texture, KairosError>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;

            let lod_max_clamp = settings
                .sampler
                .mipmap
                .as_ref()
                .map(|mipmap| mipmap.lod_max_clamp)
                .unwrap_or(0.0);
            let data = TextureSettings::deserialize_pixel_datas(
                &bytes,
                settings.width,
                settings.height,
                lod_max_clamp,
                settings.format,
            )?;

            Ok(Texture {
                width: settings.width,
                height: settings.height,
                format: settings.format,
                data,
                sampler: settings.sampler.clone(),
            })
        }
    }

    fn extensions(&self) -> &[&str] {
        &["texture_bin"]
    }
}

/// Turns a source image into the processed [`Texture`] product.
///
/// The low-level [`Process`] shell around the retained conversion algorithm
/// ([`TextureSettings::convert_source`]): decode, resize, mip, encode. It reads
/// the source bytes through the [`ProcessContext`] and writes only the encoded
/// mip chain, returning the resolved settings the product `.meta` records.
pub struct TextureProcessor;

impl Process for TextureProcessor {
    type Settings = TextureSettings;
    type OutputLoader = TextureLoader;

    fn process(
        &self,
        context: &mut ProcessContext,
        settings: &Self::Settings,
        writer: &mut Writer,
    ) -> impl ConditionalSendFuture<Output = Result<TextureSettings, ProcessError>> {
        async move {
            let mut bytes = Vec::new();
            context.asset_reader().read_to_end(&mut bytes).await.map_err(
                |error| ProcessError::AssetReaderError {
                    path: context.path().clone(),
                    err: error.into(),
                },
            )?;

            let (resolved, data) = TextureSettings::convert_source(&bytes, settings)
                .map_err(|error| ProcessError::AssetTransformError(error.into()))?;

            let encoded = TextureSettings::serialize_pixel_datas(&data);
            writer.write_all(&encoded).await.map_err(|error| {
                ProcessError::AssetWriterError {
                    path: context.path().clone(),
                    err: error.into(),
                }
            })?;

            Ok(resolved)
        }
    }
}

/// Registers the [`Texture`] asset and its [`TextureLoader`] with the core.
///
/// Must run after [`kairos_asset::install`], which creates the
/// `AssetServer` and the `AssetStages` this reads.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<Texture>(TEXTURE_ASSETS_CAPACITY);
    world.register_asset_loader(TextureLoader);
}

/// Registers [`TextureProcessor`] with the processor and makes it the default
/// for `.png` sources.
///
/// Called by the graphics install when the host runs in layout ②; a host with no
/// processor has nothing to register against.
pub fn install_processor(processor: &AssetProcessor) {
    processor.register_processor(TextureProcessor);
    processor.set_default_processor::<TextureProcessor>("png");
}
