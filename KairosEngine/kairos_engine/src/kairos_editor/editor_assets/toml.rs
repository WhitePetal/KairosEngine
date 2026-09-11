//! The `Toml` asset: a parsed TOML table loaded through the
//! asset core.
//!
//! The editor's TOML inspector edits a [`toml::Table`] in place, so the table
//! itself is the asset. It is a local newtype: `Asset` is a foreign trait and
//! `toml::Table` a foreign type, so the impl must live on a type this crate owns.

use kairos_asset::{
    Asset, AssetLoader, AssetWorldExt, LoadContext, Reader, VisitAssetDependencies,
};
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;

/// A parsed TOML document.
#[derive(Debug, Clone)]
pub struct Toml(pub toml::Table);

impl Asset for Toml {}
impl VisitAssetDependencies for Toml {}

/// How many TOML slots [`Assets<Toml>`](kairos_asset::Assets) preallocates.
///
/// Carried over from the legacy stack; the count lands here with the type it
/// belongs to.
pub const TOML_TABLE_ASSETS_CAPACITY: usize = 64;

/// Reads a `.toml` file and parses it into a [`Toml`] table.
#[derive(Debug)]
pub struct TomlLoader;

impl AssetLoader for TomlLoader {
    type Asset = Toml;
    type Settings = ();
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Toml, KairosError>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let content = String::from_utf8(bytes)?;
            let table: toml::Table = toml::from_str(&content)?;
            Ok(Toml(table))
        }
    }

    fn extensions(&self) -> &[&str] {
        &["toml"]
    }
}

/// Registers the [`Toml`] asset and its [`TomlLoader`] with the asset core.
///
/// Must run after [`kairos_asset::install`], which creates the
/// `AssetServer` and the `AssetStages` this reads.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<Toml>(TOML_TABLE_ASSETS_CAPACITY);
    world.register_asset_loader(TomlLoader);
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use kairos_asset::{AssetServer, Assets, install};
    use kairos_ecs::schedule::ScheduleLabel;
    use kairos_ecs::world::World;

    use super::{Toml, install as install_toml};

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

    /// A `.toml` load through the new core lands its table in `Assets<Toml>`.
    #[test]
    fn toml_loads_through_the_core() {
        // The default source is rooted at the cwd and `UnapprovedPathMode::Forbid`
        // rejects paths outside it, so the load uses a path relative to the cwd.
        let dir = tempfile::Builder::new()
            .tempdir_in(".")
            .expect("a temp dir in the cwd");
        let full_path = dir.path().join("Probe.toml");
        std::fs::write(&full_path, b"answer = 42\n").expect("write the toml");
        let cwd = std::env::current_dir().expect("the cwd");
        let rel_path = full_path
            .strip_prefix(&cwd)
            .expect("the temp dir is under the cwd")
            .to_path_buf();

        let mut world = World::new();
        install(&mut world, Tracking, Events);
        install_toml(&mut world);

        let handle = world.resource::<AssetServer>().load::<Toml>(rel_path);

        // The loader runs on the io task pool, so pump the tracking stage until
        // its result reaches the store.
        let mut answer = None;
        for _ in 0..200 {
            world.run_schedule(Tracking);
            if let Some(toml) = world.resource::<Assets<Toml>>().get(handle.id()) {
                answer = toml.0.get("answer").and_then(|value| value.as_integer());
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(answer, Some(42));
    }
}
