//! Composition test for the editor's install entry point.
//!
//! [`install`](super::install) composes the editor's five self-contained
//! installs. Each module's own tests prove its `install` mounts the right
//! things; this file proves only *composition* — that one `install` call reaches
//! all five — so it asserts one representative World resource per install.
//!
//! It is the editor-side counterpart to the engine's `engine::test`: after the
//! split, `build_world` no longer installs the editor's assets, so this is where
//! the five are proven reachable from a single entry point.

use crate::camera::SceneViewInput;
use crate::editor_assets::{Text, Toml};
use crate::syntax::SyntaxHighlightSettings;
use crate::ui::inspector::texture::TextureEdit;
use kairos_ecs::world::World;
use kairos_engine::asset::{AssetOptions, AssetServer, AssetStages, Assets};
use kairos_engine::schedule::{PostUpdate, PreUpdate, Startup};

/// The engine bootstrap [`install`](super::install) requires: the schedule rails
/// (its `PostUpdate` stage) and the asset core (its `AssetServer` and
/// `AssetStages`). It mirrors `build_world`'s first two steps without the
/// subsystems the editor install does not touch.
fn bootstrapped_world() -> World {
    let mut world = World::new();
    kairos_engine::schedule::install(&mut world);
    kairos_engine::asset::install(
        &mut world,
        AssetOptions::new(PreUpdate, PostUpdate, Startup),
    );
    world
}

/// One `install` call mounts all five editor installs: the four asset stores and
/// the camera controller.
#[test]
fn install_mounts_every_editor_subsystem() {
    let mut world = bootstrapped_world();

    // The precondition is real, not incidental: the engine bootstrap put the
    // asset core on the world before the editor install reads it.
    assert!(
        world.get_resource::<AssetServer>().is_some(),
        "the harness must install the asset core first"
    );
    assert!(
        world.get_resource::<AssetStages>().is_some(),
        "the harness must install the asset core first"
    );

    crate::install(&mut world);

    let representative_stores: [(&str, bool); 4] = [
        (
            "the editor text",
            world.get_resource::<Assets<Text>>().is_some(),
        ),
        (
            "the editor toml",
            world.get_resource::<Assets<Toml>>().is_some(),
        ),
        (
            "the syntax highlighting settings",
            world
                .get_resource::<Assets<SyntaxHighlightSettings>>()
                .is_some(),
        ),
        (
            "the texture edit",
            world.get_resource::<Assets<TextureEdit>>().is_some(),
        ),
    ];
    for (subsystem, present) in representative_stores {
        assert!(present, "`kairos_editor::install` must install {subsystem}");
    }

    assert!(
        world.get_resource::<SceneViewInput>().is_some(),
        "`kairos_editor::install` must install the editor camera controller"
    );
}
