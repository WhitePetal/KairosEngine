use std::path::PathBuf;

use kairos_asset::next::{
    Asset, AssetLoader, AssetWorldExt, LoadContext, Reader, VisitAssetDependencies,
};
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;
use serde::{Deserialize, Serialize};

use crate::consts::TEXTURE_ASSETS_CAPACITY;

pub mod format;
pub mod sampler;

mod serialize;

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

/// TOML-serializable form stored in `.texture` files.
///
/// Contains the source image path, texture dimensions, format,
/// and sampler configuration.
/// The pixel data is stored separately in the companion `.texture_bin` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedTexture {
    /// Path to the source image file (e.g. PNG).
    pub source_path: PathBuf,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// GPU texture format.
    pub format: TextureFormat,
    /// Sampler configuration (filter, wrap, mipmap, etc.).
    pub sampler: SamplerConfig,
}

/// Runtime form held by [`Assets<Texture>`](kairos_asset::next::Assets).
///
/// Contains the resolved dimensions, the pixel data loaded
/// from `.texture_bin`, and the sampler configuration.
/// `data` is a mip-chain: `data[0]` = base level, `data[1..]` = coarser levels.
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

/// Reads a `.texture` TOML descriptor and its companion `.texture_bin` payload.
///
/// The pixel data lives beside the descriptor rather than in an asset of its
/// own, so the loader reads it directly with `async-fs`: the default source is
/// rooted at the process working directory, which is also what the descriptor's
/// path is relative to.
#[derive(Debug)]
pub struct TextureLoader;

impl AssetLoader for TextureLoader {
    type Asset = Texture;
    type Settings = ();
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Texture, KairosError>> {
        async move {
            let mut toml_bytes = Vec::new();
            reader.read_to_end(&mut toml_bytes).await?;
            let serialized: SerializedTexture = toml::from_slice(&toml_bytes)?;

            let lod_max_clamp = serialized
                .sampler
                .mipmap
                .as_ref()
                .map(|m| m.lod_max_clamp)
                .unwrap_or(0.0);
            let bin_path = load_context.path().path().with_extension("texture_bin");
            let bytes = async_fs::read(&bin_path).await?;
            let data = SerializedTexture::deserialize_pixel_datas(
                &bytes,
                serialized.width,
                serialized.height,
                lod_max_clamp,
                serialized.format,
            )?;

            Ok(Texture {
                width: serialized.width,
                height: serialized.height,
                format: serialized.format,
                data,
                sampler: serialized.sampler,
            })
        }
    }

    fn extensions(&self) -> &[&str] {
        &["texture"]
    }
}

/// Registers the [`Texture`] asset and its [`TextureLoader`] with the core.
///
/// Must run after [`kairos_asset::next::install`], which creates the
/// `AssetServer` and the `AssetStages` this reads.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<Texture>(TEXTURE_ASSETS_CAPACITY);
    world.register_asset_loader(TextureLoader);
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use kairos_asset::next::{AssetServer, Assets, install};
    use kairos_ecs::schedule::ScheduleLabel;
    use kairos_ecs::world::World;

    use super::{PixelDatas, Texture, TextureFormat, install as install_texture};
    use crate::texture::sampler::{AddressMode, FilterMode, SamplerConfig};
    use crate::texture::SerializedTexture;

    /// The two ad-hoc stages the asset drivers are installed into.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Tracking;

    impl ScheduleLabel for Tracking {
        fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
            Box::new(*self)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Events;

    impl ScheduleLabel for Events {
        fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
            Box::new(*self)
        }
    }

    fn sampler() -> SamplerConfig {
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

    /// A `.texture` + `.texture_bin` pair loads through the new core.
    #[test]
    fn texture_loads_through_the_core() {
        let dir = tempfile::Builder::new()
            .tempdir_in(".")
            .expect("a temp dir in the cwd");
        let full_path = dir.path().join("Probe.texture");
        let descriptor = SerializedTexture {
            source_path: full_path.clone(),
            width: 1,
            height: 1,
            format: TextureFormat::Rgba8Unorm,
            sampler: sampler(),
        };
        std::fs::write(&full_path, toml::to_string(&descriptor).unwrap())
            .expect("write the texture descriptor");
        std::fs::write(
            full_path.with_extension("texture_bin"),
            SerializedTexture::serialize_pixel_datas(&[PixelDatas::U8(vec![
                10, 20, 30, 255,
            ])]),
        )
        .expect("write the texture binary");
        let cwd = std::env::current_dir().expect("the cwd");
        let rel_path = full_path
            .strip_prefix(&cwd)
            .expect("the temp dir is under the cwd")
            .to_path_buf();

        let mut world = World::new();
        install(&mut world, Tracking, Events);
        install_texture(&mut world);

        let handle = world.resource::<AssetServer>().load::<Texture>(rel_path);

        let mut loaded = None;
        for _ in 0..200 {
            world.run_schedule(Tracking);
            if let Some(texture) = world.resource::<Assets<Texture>>().get(handle.id()) {
                loaded = Some((texture.width, texture.height));
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(loaded, Some((1, 1)));
    }
}
