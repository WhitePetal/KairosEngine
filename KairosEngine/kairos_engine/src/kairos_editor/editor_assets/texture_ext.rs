//! The `TextureExt` asset: the editor's runtime composite for a `.texture`.
//!
//! A `.texture` descriptor carries the editable [`SerializedTexture`] settings
//! (source path, size, format, sampler) while the pixel data is produced by the
//! asset processor and loaded as the runtime [`Texture`] asset. The texture
//! inspector edits the settings *and* previews the pixels, so it needs both —
//! plus the source image's original pixels, to resize from when the user picks a
//! new max size.
//!
//! [`TextureExt`] bundles those three. Its loader builds the preview [`Texture`]
//! from the source image with the same processor algorithm the product uses
//! ([`TextureSettings::convert_source`]); the product itself is written only by
//! the processor, never by the editor.

use std::path::PathBuf;

use crate::asset::{
    Asset, AssetLoader, AssetWorldExt, Handle, LoadContext, Reader, UntypedAssetId,
    VisitAssetDependencies,
};
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;
use serde::{Deserialize, Serialize};

use crate::{
    graphics::texture::{
        Texture, TextureSettings,
        sampler::SamplerConfig,
        format::TextureFormat,
    },
    kairos_editor::consts,
};

/// The editor's editable `.texture` descriptor.
///
/// This is the legacy per-type TOML wrapper: it stores the source path and the
/// [`TextureSettings`] the inspector edits. It is no longer a product
/// descriptor — the pixel data lives in the processor's product — and the
/// migration retires it in favour of the source's `.meta` sidecar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedTexture {
    /// Path to the source image file (e.g. PNG).
    pub source_path: PathBuf,
    /// Output width in pixels; `0` keeps the source image's width.
    pub width: u32,
    /// Output height in pixels; `0` keeps the source image's height.
    pub height: u32,
    /// GPU texture format.
    pub format: TextureFormat,
    /// Sampler configuration (filter, wrap, mipmap, etc.).
    pub sampler: SamplerConfig,
}

impl SerializedTexture {
    /// The processor settings this descriptor carries.
    pub fn settings(&self) -> TextureSettings {
        TextureSettings {
            width: self.width,
            height: self.height,
            format: self.format,
            sampler: self.sampler.clone(),
        }
    }

    /// Writes the settings back to the `.texture` TOML descriptor.
    ///
    /// This no longer writes the pixel data: the product is written only by the
    /// processor.
    pub fn save_to_file(&self) -> Result<(), anyhow::Error> {
        let toml_content = toml::to_string(self)?;
        std::fs::write(&self.source_path.with_extension("texture"), toml_content)?;
        Ok(())
    }
}

/// Editor runtime composite for one `.texture`: the editable settings, a handle
/// to the runtime pixel data, and the cached original source image.
#[derive(Debug, Clone)]
pub struct TextureExt {
    /// Canonical settings — modifiable by the inspector.
    pub serialized: SerializedTexture,
    /// Handle to the runtime `Texture` (RGBA pixel data for preview).
    pub texture: Handle<Texture>,
    /// Original image width from the source PNG.
    pub original_width: u32,
    /// Original image height from the source PNG.
    pub original_height: u32,
    /// Cached original RGBA pixel data (for resizing on Apply).
    pub original_rgba: Vec<u8>,
}

impl Asset for TextureExt {}
impl VisitAssetDependencies for TextureExt {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        self.texture.visit_dependencies(visit);
    }
}

/// Reads a `.texture` descriptor, caches the source image's original pixels, and
/// builds the runtime [`Texture`] preview through the processor's algorithm.
#[derive(Debug)]
pub struct TextureExtLoader;

impl AssetLoader for TextureExtLoader {
    type Asset = TextureExt;
    type Settings = ();
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<TextureExt, KairosError>> {
        async move {
            // 1. Read the `.texture` TOML into its editable form.
            let mut toml_bytes = Vec::new();
            reader.read_to_end(&mut toml_bytes).await?;
            let serialized: SerializedTexture = toml::from_slice(&toml_bytes)?;

            // 2. Read the source image for the original dimensions / RGBA cache
            //    and the preview the processor's algorithm would produce. A
            //    missing or unreadable source is not fatal: the composite
            //    degrades to a 1x1 placeholder.
            let source_path = serialized.source_path.clone();
            let source_bytes = async_fs::read(&source_path).await.ok();
            let decoded = source_bytes
                .as_deref()
                .and_then(|bytes| image::load_from_memory(bytes).ok());
            let (original_width, original_height, original_rgba) = match &decoded {
                Some(image) => (
                    image.width(),
                    image.height(),
                    image.clone().into_rgba8().into_vec(),
                ),
                None => {
                    log::warn!(
                        "TextureExt: failed to read or decode source image '{}'",
                        source_path.display()
                    );
                    (0, 0, Vec::new())
                }
            };

            // 3. Build the preview through the processor's conversion the product
            //    is produced with, so the inspector shows what will be processed.
            let settings = serialized.settings();
            let (resolved, data) = match source_bytes.as_deref() {
                Some(bytes) => TextureSettings::convert_source(bytes, &settings)
                    .map_err(|error| KairosError::error(error))?,
                None => (
                    settings.clone(),
                    vec![crate::graphics::texture::PixelDatas::U8(vec![255; 4])],
                ),
            };
            let texture = Texture {
                width: resolved.width,
                height: resolved.height,
                format: resolved.format,
                data,
                sampler: resolved.sampler,
            };
            let texture = load_context.add_labeled_asset("texture", texture);

            Ok(TextureExt {
                serialized,
                texture,
                original_width,
                original_height,
                original_rgba,
            })
        }
    }

    /// This loader is resolved by asset type, never by extension: the `.texture`
    /// extension is claimed by the editor's descriptor, and the composite is
    /// always loaded with an explicit asset type.
    fn extensions(&self) -> &[&str] {
        &[]
    }
}

/// Registers the [`TextureExt`] asset and its [`TextureExtLoader`] with the core.
///
/// Must run after [`crate::asset::install`] and after
/// [`kairos_graphics::texture::install`], whose `Texture` store the loader's
/// preview targets.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<TextureExt>(consts::TEXTURE_EXT_ASSETS_CAPACITY);
    world.register_asset_loader(TextureExtLoader);
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use crate::asset::{AssetOptions, AssetServer, Assets, install};
    use kairos_ecs::schedule::ScheduleLabel;
    use kairos_ecs::world::World;

    use super::{SerializedTexture, TextureExt, install as install_texture_ext};
    use crate::graphics::texture::{
        Texture, TextureFormat,
        sampler::{AddressMode, FilterMode, SamplerConfig},
    };

    /// The three ad-hoc stages the asset drivers are installed into.
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Boot;

    impl ScheduleLabel for Boot {
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

    /// A `.texture` load through the core lands the composite in
    /// `Assets<TextureExt>` *and* pulls its preview `Texture` in, and the
    /// loader caches the source image's original dimensions.
    #[test]
    fn texture_ext_loads_with_its_texture_dependency() {
        // The default source is rooted at the cwd and `UnapprovedPathMode::Forbid`
        // rejects paths outside it, so the load uses a path relative to the cwd.
        let dir = tempfile::Builder::new()
            .tempdir_in(".")
            .expect("a temp dir in the cwd");

        let source_path = dir.path().join("Probe.png");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]))
            .save(&source_path)
            .expect("write the source png");

        let texture_path = dir.path().join("Probe.texture");
        let descriptor = SerializedTexture {
            source_path: source_path.clone(),
            width: 0,
            height: 0,
            format: TextureFormat::Rgba8Unorm,
            sampler: sampler(),
        };
        std::fs::write(&texture_path, toml::to_string(&descriptor).unwrap())
            .expect("write the texture descriptor");

        let cwd = std::env::current_dir().expect("the cwd");
        let rel_path = texture_path
            .strip_prefix(&cwd)
            .expect("the temp dir is under the cwd")
            .to_path_buf();

        let mut world = World::new();
        install(&mut world, AssetOptions::new(Tracking, Events, Boot));
        crate::graphics::texture::install(&mut world);
        install_texture_ext(&mut world);

        let handle = world.resource::<AssetServer>().load::<TextureExt>(rel_path);

        // The composite lands once the loader returns; its preview texture
        // resolves as a labeled asset, so pump until both are in their stores.
        let mut loaded = None;
        for _ in 0..400 {
            world.run_schedule(Tracking);
            if let Some(ext) = world.resource::<Assets<TextureExt>>().get(handle.id())
                && let Some(texture) = world.resource::<Assets<Texture>>().get(ext.texture.id())
            {
                loaded = Some((
                    ext.original_width,
                    ext.original_height,
                    texture.width,
                    texture.height,
                ));
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(loaded, Some((2, 2, 2, 2)));
    }
}
