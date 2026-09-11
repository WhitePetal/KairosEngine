//! The `Font` asset: raw font bytes, loaded through the asset core.
//!
//! This is a leaf asset type on the `bevy_asset`-style core
//! (`AssetLoader` + `Assets<Font>` + `Handle<Font>`), so the loader, its
//! registration, and the capacity knob all live next to the type they belong to.

use crate::asset::{
    Asset, AssetLoader, AssetWorldExt, LoadContext, Reader, VisitAssetDependencies,
};
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;

/// A font, kept as the raw bytes of its file until egui registers them.
#[derive(Debug, Clone)]
pub struct Font {
    pub bytes: Vec<u8>,
}

impl Asset for Font {}
impl VisitAssetDependencies for Font {}

/// How many font slots [`Assets<Font>`](crate::asset::Assets) preallocates.
///
/// The capacity is a real knob carried over from the legacy stack; fonts are few
/// and long-lived, so the store is sized up front.
pub const FONT_ASSETS_CAPACITY: usize = 8;

/// Reads font bytes from a `.ttf` file.
#[derive(Debug)]
pub struct FontLoader;

impl AssetLoader for FontLoader {
    type Asset = Font;
    type Settings = ();
    type Error = std::io::Error;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Font, std::io::Error>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            Ok(Font { bytes })
        }
    }

    fn extensions(&self) -> &[&str] {
        &["ttf"]
    }
}

/// Registers the [`Font`] asset and its [`FontLoader`] with the asset core.
///
/// Must run after [`crate::asset::install`], which creates the
/// `AssetServer` and the `AssetStages` this reads.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<Font>(FONT_ASSETS_CAPACITY);
    world.register_asset_loader(FontLoader);
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use crate::asset::{AssetServer, Assets, install};
    use kairos_ecs::schedule::ScheduleLabel;
    use kairos_ecs::world::World;

    use super::{Font, install as install_font};

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

    /// A `.ttf` load through the new core lands its bytes in `Assets<Font>`.
    #[test]
    fn font_loads_through_the_core() {
        // The default source is rooted at the cwd and `UnapprovedPathMode::Forbid`
        // rejects paths outside it, so the load uses a path relative to the cwd.
        let dir = tempfile::Builder::new()
            .tempdir_in(".")
            .expect("a temp dir in the cwd");
        let full_path = dir.path().join("Probe.ttf");
        std::fs::write(&full_path, b"font-bytes").expect("write the font bytes");
        let cwd = std::env::current_dir().expect("the cwd");
        let rel_path = full_path
            .strip_prefix(&cwd)
            .expect("the temp dir is under the cwd")
            .to_path_buf();

        let mut world = World::new();
        install(&mut world, Tracking, Events);
        install_font(&mut world);

        let handle = world.resource::<AssetServer>().load::<Font>(rel_path);

        // The loader runs on the io task pool, so pump the tracking stage until
        // its result reaches the store.
        let mut bytes = None;
        for _ in 0..200 {
            world.run_schedule(Tracking);
            if let Some(font) = world.resource::<Assets<Font>>().get(handle.id()) {
                bytes = Some(font.bytes.clone());
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(bytes.as_deref(), Some(&b"font-bytes"[..]));
    }
}
