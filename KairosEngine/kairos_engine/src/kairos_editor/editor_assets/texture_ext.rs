//! The `TextureExt` asset: the editor's runtime composite for a texture source.
//!
//! After the `.meta` migration (ADR 0002) a texture is a source image (`.png`)
//! plus a `.meta` sidecar whose `AssetAction::Process` carries the editable
//! [`TextureSettings`]; the pixel data is produced by the processor under
//! `imported_assets/Default`. The editor still needs the settings *and* a
//! preview, so [`TextureExt`] bundles them. Its loader reads the settings from
//! the source's `.meta`, caches the source image's original pixels, and builds
//! the preview [`Texture`] with the same algorithm the product uses
//! ([`TextureSettings::convert_source`]). The product itself is written only by
//! the processor, never by the editor.

use std::path::{Path, PathBuf};

use crate::asset::{
    Asset, AssetAction, AssetLoader, AssetMeta, AssetMetaDyn, AssetWorldExt, Handle, LoadContext,
    Reader, UntypedAssetId, VisitAssetDependencies, io::get_meta_path, meta::processor_name,
};
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;

use crate::{
    graphics::texture::{Texture, TextureSettings, format::TextureFormat, sampler::SamplerConfig},
    kairos_editor::consts,
};

/// The editor's editable view of a texture's `.meta` processor settings.
///
/// Mirrors [`TextureSettings`] plus the source path so the inspector can label
/// and write back the sidecar. It is no longer a product descriptor — the pixel
/// data lives in the processor's product.
#[derive(Debug, Clone)]
pub struct EditableTexture {
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

impl EditableTexture {
    /// The processor settings this descriptor carries.
    pub fn settings(&self) -> TextureSettings {
        TextureSettings {
            width: self.width,
            height: self.height,
            format: self.format,
            sampler: self.sampler.clone(),
        }
    }

    /// The editable view backed by `source_path` and `settings`.
    pub fn from_settings(source_path: PathBuf, settings: TextureSettings) -> Self {
        Self {
            source_path,
            width: settings.width,
            height: settings.height,
            format: settings.format,
            sampler: settings.sampler,
        }
    }

    /// Writes the source's `.meta` processor settings.
    ///
    /// The editor never writes the product: it records the [`TextureSettings`]
    /// as an `AssetAction::Process` beside the source image, and the asset
    /// processor — when the host runs in layout ② — regenerates the processed
    /// bytes from them.
    pub fn write_meta(&self) -> Result<(), Box<dyn std::error::Error>> {
        let meta = AssetMeta::<(), TextureSettings>::new(AssetAction::Process {
            processor: processor_name::<crate::graphics::texture::TextureProcessor>().to_string(),
            settings: self.settings(),
        });
        std::fs::write(
            get_meta_path(&self.source_path),
            AssetMetaDyn::serialize(&meta),
        )?;
        Ok(())
    }
}

/// Editor runtime composite for one texture source: the editable settings, a
/// handle to the runtime pixel data, and the cached original source image.
#[derive(Debug, Clone)]
pub struct TextureExt {
    /// Canonical settings — modifiable by the inspector.
    pub serialized: EditableTexture,
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

/// Reads a source's `.meta` settings, caches the source image's original pixels,
/// and builds the runtime [`Texture`] preview through the processor's algorithm.
///
/// The composite is loaded from the source's `.meta` path (`foo.png.meta`), a
/// path no loader claims, so the server resolves this loader by asset type.
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
            // 1. Read the source's `.meta` into its editable form.
            let mut meta_bytes = Vec::new();
            reader.read_to_end(&mut meta_bytes).await?;
            let source_path = source_from_meta_path(load_context.path().path());
            let settings = read_process_settings(&meta_bytes);

            // 2. Read the source image for the original dimensions / RGBA cache
            //    and the preview the processor's algorithm would produce. A
            //    missing or unreadable source is not fatal: the composite
            //    degrades to a 1x1 placeholder.
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
                serialized: EditableTexture::from_settings(source_path, settings),
                texture,
                original_width,
                original_height,
                original_rgba,
            })
        }
    }

    /// This loader is resolved by asset type, never by extension: the composite
    /// is always loaded with an explicit asset type from the source's `.meta`
    /// path, whose `png.meta` extension no loader claims.
    fn extensions(&self) -> &[&str] {
        &[]
    }
}

/// The source file a `.meta` asset path belongs to (`foo.png.meta` → `foo.png`).
fn source_from_meta_path(meta_path: &Path) -> PathBuf {
    meta_path.with_extension("")
}

/// The settings a source's `.meta` carries, or the default when the sidecar is
/// not a `Process` action or fails to parse.
fn read_process_settings(meta_bytes: &[u8]) -> TextureSettings {
    match AssetMeta::<(), TextureSettings>::deserialize(meta_bytes) {
        Ok(meta) => match meta.asset {
            AssetAction::Process { settings, .. } => settings,
            _ => TextureSettings::default(),
        },
        Err(error) => {
            log::warn!("TextureExt: unreadable texture `.meta`, using defaults: {error}");
            TextureSettings::default()
        }
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

    use super::{EditableTexture, TextureExt, install as install_texture_ext};
    use crate::graphics::texture::{
        Texture, TextureFormat, TextureSettings,
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

    /// A source `.meta` load through the core lands the composite in
    /// `Assets<TextureExt>` *and* pulls its preview `Texture` in, and the
    /// loader caches the source image's original dimensions.
    #[test]
    fn texture_ext_loads_from_the_source_meta() {
        // The default source is rooted at the cwd and `UnapprovedPathMode::Forbid`
        // rejects paths outside it, so the load uses a path relative to the cwd.
        let dir = tempfile::Builder::new()
            .tempdir_in(".")
            .expect("a temp dir in the cwd");

        let source_path = dir.path().join("Probe.png");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]))
            .save(&source_path)
            .expect("write the source png");

        let settings = TextureSettings {
            width: 0,
            height: 0,
            format: TextureFormat::Rgba8Unorm,
            sampler: sampler(),
        };
        EditableTexture::from_settings(source_path.clone(), settings)
            .write_meta()
            .expect("write the source meta");

        let cwd = std::env::current_dir().expect("the cwd");
        let meta_path = source_path.with_extension("png.meta");
        let rel_path = meta_path
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
        // Also carry the settings back out, so a defaults-only load is caught.
        let mut loaded = None;
        for _ in 0..400 {
            world.run_schedule(Tracking);
            if let Some(ext) = world.resource::<Assets<TextureExt>>().get(handle.id())
                && let Some(texture) = world.resource::<Assets<Texture>>().get(ext.texture.id())
            {
                let settings = ext.serialized.settings();
                loaded = Some((
                    ext.original_width,
                    ext.original_height,
                    texture.width,
                    texture.height,
                    settings.format,
                    settings.sampler.filter_mode,
                ));
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(
            loaded,
            Some((2, 2, 2, 2, TextureFormat::Rgba8Unorm, FilterMode::Nearest))
        );
    }
}
