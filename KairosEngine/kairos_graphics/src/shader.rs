use std::path::PathBuf;

use kairos_asset::next::{
    Asset, AssetLoader, AssetWorldExt, LoadContext, Reader, VisitAssetDependencies,
};
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;
use serde::{Deserialize, Serialize};

use crate::consts::SHADER_ASSETS_CAPACITY;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub source_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderAsset {
    pub meta: Meta,
    pub shader_string: String,
}

impl Asset for ShaderAsset {}
impl VisitAssetDependencies for ShaderAsset {}

/// Reads a WGSL source file into a [`ShaderAsset`].
#[derive(Debug)]
pub struct ShaderLoader;

impl AssetLoader for ShaderLoader {
    type Asset = ShaderAsset;
    type Settings = ();
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<ShaderAsset, KairosError>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let shader_string = String::from_utf8(bytes)?;
            Ok(ShaderAsset {
                meta: Meta {
                    source_path: load_context.path().path().to_path_buf(),
                },
                shader_string,
            })
        }
    }

    fn extensions(&self) -> &[&str] {
        &["wgsl"]
    }
}

/// Registers the [`ShaderAsset`] asset and its [`ShaderLoader`] with the core.
///
/// Must run after [`kairos_asset::next::install`], which creates the
/// `AssetServer` and the `AssetStages` this reads.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<ShaderAsset>(SHADER_ASSETS_CAPACITY);
    world.register_asset_loader(ShaderLoader);
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use kairos_asset::next::{AssetServer, Assets, install};
    use kairos_ecs::schedule::ScheduleLabel;
    use kairos_ecs::world::World;

    use super::{ShaderAsset, install as install_shader};

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

    /// A `.wgsl` load through the new core lands its source in `Assets<ShaderAsset>`.
    #[test]
    fn shader_loads_through_the_core() {
        // The default source is rooted at the cwd and `UnapprovedPathMode::Forbid`
        // rejects paths outside it, so the load uses a path relative to the cwd.
        let dir = tempfile::Builder::new()
            .tempdir_in(".")
            .expect("a temp dir in the cwd");
        let full_path = dir.path().join("Probe.wgsl");
        std::fs::write(&full_path, b"@vertex fn vs_main() {}").expect("write the shader");
        let cwd = std::env::current_dir().expect("the cwd");
        let rel_path = full_path
            .strip_prefix(&cwd)
            .expect("the temp dir is under the cwd")
            .to_path_buf();

        let mut world = World::new();
        install(&mut world, Tracking, Events);
        install_shader(&mut world);

        let handle = world
            .resource::<AssetServer>()
            .load::<ShaderAsset>(rel_path);

        // The loader runs on the io task pool, so pump the tracking stage until
        // its result reaches the store.
        let mut source = None;
        for _ in 0..200 {
            world.run_schedule(Tracking);
            if let Some(shader) = world.resource::<Assets<ShaderAsset>>().get(handle.id()) {
                source = Some(shader.shader_string.clone());
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(source.as_deref(), Some("@vertex fn vs_main() {}"));
    }
}
