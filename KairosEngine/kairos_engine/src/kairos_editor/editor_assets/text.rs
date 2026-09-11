//! The `Text` asset: a UTF-8 document loaded through the asset
//! core.
//!
//! Scripts (`.rs`), documents (`.md`/`.txt`), and shader sources (`.wgsl`) are
//! all the same asset — a string the editor reads and writes — so one [`Text`]
//! type carries them. It is a local newtype over [`String`]: `Asset` is a
//! foreign trait and `String` a foreign type, so the impl must live on a type
//! this crate owns.

use crate::asset::{
    Asset, AssetLoader, AssetWorldExt, LoadContext, Reader, VisitAssetDependencies,
};
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;

/// A UTF-8 text document: a script, markdown file, or shader source.
#[derive(Debug, Clone)]
pub struct Text(pub String);

impl Asset for Text {}
impl VisitAssetDependencies for Text {}

/// How many text slots [`Assets<Text>`](crate::asset::Assets) preallocates.
///
/// Carried over from the legacy stack, where the text store borrowed the
/// material capacity; text files are numerous, so the store is sized up front.
pub const TEXT_ASSETS_CAPACITY: usize = 512;

/// Reads a source file to a UTF-8 [`Text`] document.
#[derive(Debug)]
pub struct TextLoader;

impl AssetLoader for TextLoader {
    type Asset = Text;
    type Settings = ();
    type Error = std::io::Error;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Text, std::io::Error>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let content = String::from_utf8(bytes).map_err(|error| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, error)
            })?;
            Ok(Text(content))
        }
    }

    fn extensions(&self) -> &[&str] {
        &["rs", "md", "txt", "wgsl"]
    }
}

/// Registers the [`Text`] asset and its [`TextLoader`] with the asset core.
///
/// Must run after [`crate::asset::install`], which creates the
/// `AssetServer` and the `AssetStages` this reads.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<Text>(TEXT_ASSETS_CAPACITY);
    world.register_asset_loader(TextLoader);
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use crate::asset::{AssetServer, Assets, install};
    use kairos_ecs::schedule::ScheduleLabel;
    use kairos_ecs::world::World;

    use super::{Text, install as install_text};

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

    /// A `.md` load through the new core lands its content in `Assets<Text>`.
    #[test]
    fn text_loads_through_the_core() {
        // The default source is rooted at the cwd and `UnapprovedPathMode::Forbid`
        // rejects paths outside it, so the load uses a path relative to the cwd.
        let dir = tempfile::Builder::new()
            .tempdir_in(".")
            .expect("a temp dir in the cwd");
        let full_path = dir.path().join("Probe.md");
        std::fs::write(&full_path, b"# hello").expect("write the document");
        let cwd = std::env::current_dir().expect("the cwd");
        let rel_path = full_path
            .strip_prefix(&cwd)
            .expect("the temp dir is under the cwd")
            .to_path_buf();

        let mut world = World::new();
        install(&mut world, Tracking, Events);
        install_text(&mut world);

        let handle = world.resource::<AssetServer>().load::<Text>(rel_path);

        // The loader runs on the io task pool, so pump the tracking stage until
        // its result reaches the store.
        let mut content = None;
        for _ in 0..200 {
            world.run_schedule(Tracking);
            if let Some(text) = world.resource::<Assets<Text>>().get(handle.id()) {
                content = Some(text.0.clone());
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(content.as_deref(), Some("# hello"));
    }
}
