//! The Kairos engine's graphics subsystem, extracted as a standalone crate.
//!
//! Textures, materials, meshes, shaders, the render graph and the wgpu render
//! pipeline live here, together with the per-view extract rail that turns the
//! world's mesh entities into each window's frame buffer. The crate depends
//! only on the engine's foundation crates (`kairos_ecs`, `kairos_transform`,
//! `kairos_math`, `kairos_asset`) — never on `kairos_engine` — so it can be
//! built and tested on its own.

use kairos_ecs::{
    schedule::{IntoScheduleConfigs, ScheduleLabel, Schedules},
    world::World,
};

use crate::{
    extract::{extract_game_view, extract_scene_view, reset_camera_views},
    view_port::{GameView, SceneView},
};

pub mod asset_events;
pub mod assets;
pub mod consts;

pub mod camera;
pub mod material;
pub mod mesh;
pub mod render_pipeline;
pub mod render_state;
pub mod shader;
pub mod texture;
pub mod vertex;

pub mod attachment;
pub mod compare_function;
pub mod egui_texture_handle;
pub mod graphics_graph;

pub mod drawer;
pub mod extract;
pub mod lod_mesh_component;
pub mod material_component;
pub mod view_port;

/// Installs the graphics asset types (`ShaderAsset`, `Texture`, `Material`, and
/// `SerializedMaterial`) and the `Extract`-stage system that collects their
/// change events for cache invalidation.
///
/// Must run after [`kairos_asset::next::install`], which creates the
/// `AssetServer` and the `AssetStages` the per-type registration reads.
pub use asset_events::install_assets;

/// Installs the render rails: the per-view frame buffers, one extract system
/// per view, and the drawers that fill them.
///
/// `extract_stage` is the per-frame schedule the rails register into — the
/// stage that runs after every modifier and before the frame is read. The
/// schedule skeleton (and its label) belongs to the application, not to the
/// graphics crate, so the caller passes it in: `kairos_engine` supplies its
/// `Extract` stage, and a bare [`World`] under test can supply its own.
///
/// Note that the stage is a *precondition*: the caller must have created the
/// schedule before calling this, because registering the rails into a stage
/// that is about to be replaced would silently drop them. A missing stage at
/// *run* time is tolerated — the frame driver logs it and moves on — but a
/// missing stage at *install* time is a bootstrap order bug.
///
/// Nothing here touches the wgpu device (buffers are plain data, the systems
/// only read the world), so the whole rail is exercisable on a bare [`World`].
///
/// # Panics
///
/// If the schedule named by `extract_stage` does not exist yet.
pub fn install(world: &mut World, extract_stage: impl ScheduleLabel) {
    log::debug!("installing the render extract rails");

    world.init_resource::<SceneView>();
    world.init_resource::<GameView>();

    let mut schedules = world.get_resource_or_init::<Schedules>();
    let extract = schedules
        .get_mut(extract_stage)
        .expect("the `Extract` schedule must exist: install the schedule rails first");

    extract.add_systems((
        // Frame-start reset, so no camera carries an earlier frame's derived
        // state into this one.
        reset_camera_views,
        // One rail per view, each owning its view end to end (clear, derive,
        // draw), so no two systems write the same buffer.
        extract_scene_view.after(reset_camera_views),
        extract_game_view.after(reset_camera_views),
    ));
}
