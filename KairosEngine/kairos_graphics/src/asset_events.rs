//! Graphics-side consumption of the asset event face.
//!
//! The render caches (pipelines, texture bind groups) used to be validated by
//! comparing per-asset version and `modify_count` numbers each frame. The
//! next-generation core replaces those counters with [`AssetEvent`]s, so this
//! module collects the `Modified`/`Removed` events of the graphics asset types
//! into [`GraphicsAssetEvents`] — a World resource the render pipeline drains
//! before it draws, evicting the stale cache entries.
//!
//! Collection runs in the graphics crate's `Extract` stage, after every modifier
//! has run and before the frame is presented, so the invalidation the pipeline
//! reads is always this frame's.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};

use kairos_asset::next::{AssetEvent, AssetId};
use kairos_ecs::message::MessageReader;
use kairos_ecs::resource::Resource;
use kairos_ecs::schedule::{ScheduleLabel, Schedules};
use kairos_ecs::system::ResMut;
use kairos_ecs::world::World;

use crate::{material::Material, mesh::Mesh, shader::ShaderAsset, texture::Texture};

/// The graphics assets whose change events invalidate the render caches.
///
/// Shared behind an [`Arc`] so the `Extract`-stage collector can write it while
/// the render pipeline holds it by value; [`GraphicsAssetEvents::lock`] gives
/// access to the underlying sets.
#[derive(Resource, Clone, Default)]
pub struct GraphicsAssetEvents {
    inner: Arc<Mutex<GraphicsAssetEventSets>>,
}

/// The accumulated change signals, split by asset type and by kind.
#[derive(Default)]
pub(crate) struct GraphicsAssetEventSets {
    pub(crate) modified_shaders: HashSet<AssetId<ShaderAsset>>,
    pub(crate) removed_shaders: HashSet<AssetId<ShaderAsset>>,
    pub(crate) modified_textures: HashSet<AssetId<Texture>>,
    pub(crate) removed_textures: HashSet<AssetId<Texture>>,
    pub(crate) modified_materials: HashSet<AssetId<Material>>,
    pub(crate) removed_materials: HashSet<AssetId<Material>>,
    pub(crate) modified_meshes: HashSet<AssetId<Mesh>>,
    pub(crate) removed_meshes: HashSet<AssetId<Mesh>>,
}

impl GraphicsAssetEvents {
    /// Locks the accumulated sets for reading and draining.
    pub(crate) fn lock(&self) -> MutexGuard<'_, GraphicsAssetEventSets> {
        self.inner
            .lock()
            .expect("graphics asset event lock poisoned")
    }
}

/// Collects this frame's graphics asset events into [`GraphicsAssetEvents`].
///
/// `Added`/`Unused`/`LoadedWithDependencies` are ignored: a newly added asset
/// has no cache entry to evict, and `Unused` is always followed by `Removed`
/// when a value actually went away.
pub fn collect_graphics_asset_events(
    events: ResMut<GraphicsAssetEvents>,
    mut shaders: MessageReader<AssetEvent<ShaderAsset>>,
    mut textures: MessageReader<AssetEvent<Texture>>,
    mut materials: MessageReader<AssetEvent<Material>>,
    mut meshes: MessageReader<AssetEvent<Mesh>>,
) {
    let mut sets = events.lock();

    for event in shaders.read() {
        match event {
            AssetEvent::Modified { id } => {
                sets.modified_shaders.insert(*id);
            }
            AssetEvent::Removed { id } => {
                sets.removed_shaders.insert(*id);
            }
            _ => {}
        }
    }
    for event in textures.read() {
        match event {
            AssetEvent::Modified { id } => {
                sets.modified_textures.insert(*id);
            }
            AssetEvent::Removed { id } => {
                sets.removed_textures.insert(*id);
            }
            _ => {}
        }
    }
    for event in materials.read() {
        match event {
            AssetEvent::Modified { id } => {
                sets.modified_materials.insert(*id);
            }
            AssetEvent::Removed { id } => {
                sets.removed_materials.insert(*id);
            }
            _ => {}
        }
    }
    for event in meshes.read() {
        match event {
            AssetEvent::Modified { id } => {
                sets.modified_meshes.insert(*id);
            }
            AssetEvent::Removed { id } => {
                sets.removed_meshes.insert(*id);
            }
            _ => {}
        }
    }
}

/// Installs the graphics asset types (`ShaderAsset`, `Texture`, `Mesh`,
/// `Material`, and `SerializedMaterial`) into `world`, plus the `Extract`-stage
/// event collector their caches are invalidated by.
///
/// Must run after [`kairos_asset::next::install`], which creates the
/// `AssetServer` and the `AssetStages` the per-type registration reads, and
/// after the schedule holding `extract_stage` exists.
///
/// # Panics
///
/// If the schedule named by `extract_stage` does not exist yet.
pub fn install_assets(world: &mut World, extract_stage: impl ScheduleLabel) {
    crate::shader::install(world);
    crate::texture::install(world);
    crate::mesh::install(world);
    crate::material::install(world);

    world.init_resource::<GraphicsAssetEvents>();

    let mut schedules = world.get_resource_or_init::<Schedules>();
    schedules
        .get_mut(extract_stage)
        .expect("the extract schedule must exist: install the schedule rails first")
        .add_systems(collect_graphics_asset_events);
}
