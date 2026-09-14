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
