use kairos_ecs::{
    schedule::{IntoScheduleConfigs, Schedules},
    world::World,
};

use crate::{
    graphics::{
        extract::{
            ResetViewBuffers, extract_game_view, extract_scene_view, mesh::extract_meshes,
            reset_camera_views,
        },
        view_port::{GameView, SceneView},
    },
    kairos_editor::schedule::Extract,
};

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

/// Installs the render rails: the per-view frame buffers, the extract-stage
/// systems that fill them, and the default mesh render service.
///
/// Installed next to [`schedule::install`](crate::kairos_editor::schedule::install)
/// in `Engine::new` — and **after** it, because the rails register into the
/// [`Extract`] sub-stage the schedule skeleton creates. Nothing here touches
/// the wgpu device (buffers are plain data, the systems only read the world),
/// so the whole rail is exercisable on a bare [`World`].
///
/// # Panics
///
/// If the schedule rails are not installed yet: the [`Extract`] stage would not
/// exist, and registering the rails into a stage that is about to be replaced
/// would silently drop them. A missing stage at *run* time is tolerated — the
/// frame driver logs it and moves on — but a missing stage at *install* time is
/// a bootstrap order bug.
pub fn install(world: &mut World) {
    log::debug!("installing the render extract rails");

    world.init_resource::<SceneView>();
    world.init_resource::<GameView>();

    let mut schedules = world.get_resource_or_init::<Schedules>();
    let extract = schedules
        .get_mut(Extract)
        .expect("the `Extract` schedule must exist: install the schedule rails first");

    extract.add_systems((
        // Frame-start reset, so no camera carries an earlier frame's projection
        // into this one.
        reset_camera_views,
        // The view rails clear their buffers next; the mesh service below —
        // and anything else that orders after `ResetViewBuffers` — appends to
        // buffers that are already empty.
        (extract_scene_view, extract_game_view)
            .in_set(ResetViewBuffers)
            .after(reset_camera_views),
        extract_meshes.after(ResetViewBuffers),
    ));
}
