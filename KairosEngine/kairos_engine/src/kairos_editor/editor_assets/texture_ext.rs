//! The `TextureExt` asset: the editor's runtime composite for a `.texture`.
//!
//! A `.texture` descriptor carries the editable [`SerializedTexture`] settings
//! (source path, size, format, sampler) while the pixel data is the separate
//! runtime [`Texture`] asset. The texture inspector edits the settings *and*
//! previews the pixels, so it needs both — plus the source image's original
//! pixels, to resize from when the user picks a new max size.
//!
//! [`TextureExt`] bundles those three. It loads through the next-generation core
//! and declares its [`Texture`] through [`LoadContext::load`], so a live
//! composite keeps the runtime texture loaded.

use kairos_asset::next::{
    Asset, AssetLoader, AssetWorldExt, Handle, LoadContext, Reader, UntypedAssetId,
    VisitAssetDependencies,
};
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;

use crate::{
    graphics::texture::{SerializedTexture, Texture},
    kairos_editor::consts,
};

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

/// Reads a `.texture` descriptor, declares the runtime [`Texture`] as a
/// dependency, and caches the source image's original pixels.
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

            // 2. Declare the runtime `Texture` as a dependency; its loader reads
            //    the `.texture_bin` companion beside this descriptor.
            let texture_path = load_context.path().path().to_path_buf();
            let texture = load_context.load::<Texture>(texture_path);

            // 3. Read the original source image for its dimensions and RGBA
            //    data. A missing or unreadable source is not fatal: the rest of
            //    the composite is still usable, so the cache degrades to empty.
            let source_path = serialized.source_path.clone();
            let (original_width, original_height, original_rgba) =
                match async_fs::read(&source_path).await {
                    Ok(bytes) => match image::load_from_memory(&bytes) {
                        Ok(image) => {
                            let (width, height) = (image.width(), image.height());
                            (width, height, image.into_rgba8().into_vec())
                        }
                        Err(error) => {
                            log::warn!(
                                "TextureExt: failed to decode source image '{}': {error}",
                                source_path.display()
                            );
                            (0, 0, Vec::new())
                        }
                    },
                    Err(error) => {
                        log::warn!(
                            "TextureExt: failed to read source image '{}': {error}",
                            source_path.display()
                        );
                        (0, 0, Vec::new())
                    }
                };

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
    /// extension is claimed by [`TextureLoader`](kairos_graphics::texture::TextureLoader),
    /// and both are always loaded with an explicit asset type.
    fn extensions(&self) -> &[&str] {
        &[]
    }
}

/// Registers the [`TextureExt`] asset and its [`TextureExtLoader`] with the core.
///
/// Must run after [`kairos_asset::next::install`] and after
/// [`kairos_graphics::texture::install`], whose `Texture` store the loader's
/// declared dependency targets.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<TextureExt>(consts::TEXTURE_EXT_ASSETS_CAPACITY);
    world.register_asset_loader(TextureExtLoader);
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use kairos_asset::next::{AssetServer, Assets, install};
    use kairos_ecs::schedule::ScheduleLabel;
    use kairos_ecs::world::World;

    use super::{TextureExt, install as install_texture_ext};
    use crate::graphics::texture::{
        PixelDatas, SerializedTexture, Texture, TextureFormat,
        sampler::{AddressMode, FilterMode, SamplerConfig},
    };

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

    /// A `.texture` load through the core lands the composite in
    /// `Assets<TextureExt>` *and* pulls its `Texture` dependency in, and the
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
            width: 1,
            height: 1,
            format: TextureFormat::Rgba8Unorm,
            sampler: sampler(),
        };
        std::fs::write(&texture_path, toml::to_string(&descriptor).unwrap())
            .expect("write the texture descriptor");
        std::fs::write(
            texture_path.with_extension("texture_bin"),
            SerializedTexture::serialize_pixel_datas(&[PixelDatas::U8(vec![
                10, 20, 30, 255,
            ])]),
        )
        .expect("write the texture binary");

        let cwd = std::env::current_dir().expect("the cwd");
        let rel_path = texture_path
            .strip_prefix(&cwd)
            .expect("the temp dir is under the cwd")
            .to_path_buf();

        let mut world = World::new();
        install(&mut world, Tracking, Events);
        crate::graphics::texture::install(&mut world);
        install_texture_ext(&mut world);

        let handle = world.resource::<AssetServer>().load::<TextureExt>(rel_path);

        // The composite lands once the loader returns; its texture dependency
        // resolves on its own task, so pump until both values are in their stores.
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

        assert_eq!(loaded, Some((2, 2, 1, 1)));
    }
}
