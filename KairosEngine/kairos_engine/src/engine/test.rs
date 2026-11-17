//! Composition test for the engine bootstrap.
//!
//! `build_world` composes every engine subsystem by calling its `install`. Each
//! module's own tests prove its `install` mounts the right things; this file
//! proves only *composition* — that the bootstrap actually reached each module
//! — so it asserts one representative World resource per install, plus the
//! asset core's stage wiring. It is the engine-side replacement for the deleted
//! `schedule/asset_test.rs`, whose per-type `build_world_registers_*` assertions
//! are covered by each owning module's own install tests.

use crate::asset::{AssetServer, AssetStages, Assets};
use crate::audio::audio::AudioAsset;
use crate::graphics::material::Material;
use crate::graphics::view_port::SceneView;
use crate::kairos_ui::font::Font;
use crate::physics::PhysicsEngine;
use crate::schedule::{PostUpdate, PreUpdate, Startup};
use crate::time::Time;
use kairos_ecs::schedule::ScheduleLabel;

/// Puffin's frame collector is process-wide, so only one live recorder may
/// exist at a time (see `kairos_profile`'s ownership notes). `Engine` owns a
/// recorder, which means tests that create an engine must not run concurrently
/// with each other: two recorders deadlock on puffin's global lock.
static PROFILER_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn lock_profiler_tests() -> std::sync::MutexGuard<'static, ()> {
    PROFILER_TESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
/// Every `build_world` install left its mark: one representative resource each,
/// so a dropped `install` call (or a reordering that skips one) fails here.
///
/// The individual asset types are deliberately not re-listed — that is what the
/// modules' own install tests are for. What is under test here is that the
/// engine's bootstrap reached each module at all.
///
/// The editor's own assets and camera are *not* asserted here: `build_world`
/// does not install them. `kairos_editor::install` owns them, and its own
/// composition test covers them.
#[test]
fn build_world_installs_every_engine_subsystem() {
    let world = super::build_world();

    // The schedule rails and the asset core mounted on them.
    assert!(
        world.get_resource::<Time>().is_some(),
        "`build_world` must install the schedule rails"
    );
    assert!(
        world.get_resource::<AssetServer>().is_some(),
        "`build_world` must install the asset core"
    );

    // The subsystem installs that are not asset stores.
    assert!(
        world.get_resource::<PhysicsEngine>().is_some(),
        "`build_world` must install physics"
    );
    assert!(
        world.get_resource::<SceneView>().is_some(),
        "`build_world` must install the render rails"
    );

    // One representative store per asset install.
    let representative_stores: [(&str, bool); 3] = [
        (
            "the graphics asset types",
            world.get_resource::<Assets<Material>>().is_some(),
        ),
        ("the font", world.get_resource::<Assets<Font>>().is_some()),
        (
            "the runtime audio asset",
            world.get_resource::<Assets<AudioAsset>>().is_some(),
        ),
    ];
    for (subsystem, present) in representative_stores {
        assert!(present, "`build_world` must install {subsystem}");
    }
}

/// The asset core is mounted on the engine's *own* stages, not ad-hoc labels:
/// tracking in `PreUpdate`, events in `PostUpdate`, the processor in `Startup`.
///
/// This is the composition contract the deleted `asset_test.rs` asserted and
/// the one the render-cache ordering relies on.
/// The engine owns the recorder and opens the game frame itself.
///
/// A development build is profiled by its own loop: two ticks produce one
/// flushed frame containing the engine marker and the stage markers, with no
/// editor or UI pass involved. The test builds the schedule rails without the
/// asset/audio subsystems so it stays a profiler-boundary test.
#[test]
fn update_records_a_game_frame_when_recording() {
    let _guard = lock_profiler_tests();
    use crate::inputs::InputEngine;
    use kairos_profile::Profiler;

    let mut world = kairos_ecs::world::World::new();
    crate::schedule::install(&mut world);
    let mut engine = super::Engine {
        world,
        input_engine: InputEngine::new(),
        profiler: Profiler::new(),
    };

    engine.profiler().set_recording(true);
    // The first update enables the macros and opens frame 1; the second
    // update's begin_frame is what flushes it into the store.
    engine.update();
    engine.update();

    let session = engine.profiler().session();
    let mut engine_frame = false;
    let mut stage = false;
    let mut systems = Vec::new();
    for frame in &session.frames {
        for thread in &frame.threads {
            for sample in &thread.samples {
                match session.scope_name(sample.scope) {
                    "Kairos.Engine.Frame" => engine_frame = true,
                    "Kairos.Stage" => stage = true,
                    // Several systems run per frame; collect them all rather
                    // than depending on which one finished last.
                    "Kairos.System" => systems.extend(sample.data.clone()),
                    _ => {}
                }
            }
        }
    }
    assert!(engine_frame, "Engine::update records `Kairos.Engine.Frame`");
    assert!(stage, "the main schedule stages record `Kairos.Stage`");
    // The executors open one scope per scheduled system and put the system's
    // name (with its category) in the data: the panel's per-system breakdown
    // reads it from there.
    assert!(
        systems.iter().any(|system| system.contains("time_system")),
        "a system scope names the system it wrapped: {systems:?}"
    );

    engine.profiler().set_recording(false);
}

/// A capture reserves a timeline row for every task-pool worker, not only the
/// ones that happen to run.
#[test]
fn a_capture_reserves_a_lane_for_every_task_pool_worker() {
    let _guard = lock_profiler_tests();
    let mut engine = super::Engine::headless();
    engine.profiler().set_recording(true);
    engine.update();
    engine.update();

    let session = engine.profiler().session();
    let lanes = &session.thread_lanes;
    assert_eq!(
        lanes.first().map(String::as_str),
        std::thread::current().name(),
        "the frame thread is the first lane: {lanes:?}"
    );
    assert!(
        lanes.iter().any(|lane| lane.starts_with("Compute (")),
        "the compute pool's workers are registered before they run: {lanes:?}"
    );

    engine.profiler().set_recording(false);
}

#[test]
fn build_world_mounts_the_asset_core_on_the_engine_stages() {
    let world = super::build_world();

    let stages = world.resource::<AssetStages>();
    assert_eq!(
        stages.tracking,
        PreUpdate.intern(),
        "asset tracking must run in the engine's `PreUpdate`"
    );
    assert_eq!(
        stages.event,
        PostUpdate.intern(),
        "asset events must flush in the engine's `PostUpdate`"
    );
    assert_eq!(
        stages.startup,
        Startup.intern(),
        "the asset processor must start in the engine's `Startup`"
    );
}
