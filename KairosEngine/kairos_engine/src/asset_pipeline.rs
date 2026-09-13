//! Generating the processed assets under `imported_assets/Default`.
//!
//! The runtime host runs in [`AssetMode::Unprocessed`] — layout ① — and reads
//! the processed assets by path: the project tree pairs a source (`.glb` /
//! `.png`) with the processed asset under `imported_assets/Default`, and that
//! asset's `.meta` names the loader that decodes it (ADR 0002 / 0004,
//! `ProjectWindow`). What was missing was the entry that *writes* that tree. The
//! graphics [`Process`](kairos_asset::Process) implementations and their
//! registration were already in place — [`kairos_graphics::install_assets`]
//! calls `install_processor` — but no host ever built an [`AssetProcessor`] for
//! them to register against, and nothing outside tests drove
//! [`AssetProcessor::run_initial_processing`].
//!
//! This module is that entry. A *bake* is one offline run of the asset
//! processor: it boots the same asset core the editor boots — the engine's
//! schedule rails, the asset core, the graphics asset types — in layout ②
//! ([`AssetMode::Processed`] with a processor producing the outputs, see
//! [`AssetMode`]'s docs in `kairos_asset`) and drives the processor over every
//! source to completion.
//!
//! The run is headless and one-shot: no window, no audio device, and no
//! watching (once the queue is drained there is nothing left to react to).
//!
//! It processes every source the graphics install claims, so it renews the whole
//! processed tree rather than only the assets checked into the repository: a
//! source with a processor is turned into a processed asset, a source whose
//! loader is used directly is copied verbatim (`AssetAction::Load`), and a
//! source that neither a processor nor a loader claims is ignored and leaves no
//! processed asset at all.

use kairos_asset::io::AssetSourceBuilders;
use kairos_asset::{AssetMode, AssetOptions, AssetProcessor};
use kairos_ecs::world::World;

use crate::kairos_editor::schedule;

#[cfg(test)]
mod test;

/// The bake's asset options: layout ② over the sources, with watching off.
///
/// `use_asset_processor` is forced on rather than read from the `asset_processor`
/// crate feature, because the bake is the host that selects layout ② — the
/// runtime host stays in the default [`AssetMode::Unprocessed`] layout and reads
/// the processed assets as plain files.
fn bake_options() -> AssetOptions {
    AssetOptions::new(schedule::PreUpdate, schedule::PostUpdate, schedule::Startup)
        .with_mode(AssetMode::Processed)
        .with_use_asset_processor(true)
        .with_watch_for_changes_override(false)
}

/// The headless world the bake runs in, over `builders`.
///
/// Passing [`AssetSourceBuilders::default`] leaves the roots at the process
/// working directory as the source root and `imported_assets/Default` as the
/// processed one — the same roots the editor's host uses (ADR 0004). A host
/// whose project root is elsewhere registers its source builder here, and the
/// processed tree then mirrors that source instead.
fn bake_world(builders: AssetSourceBuilders) -> World {
    let mut world = World::new();
    // Registering the sources first keeps `AssetOptions`' default source from
    // taking over: the core only fills in a default that is still missing.
    world.insert_resource(builders);

    // The schedule rails the asset core mounts its stages into, and the
    // `Extract` stage the graphics install registers into.
    schedule::install(&mut world);

    // The asset core in layout ②: an `AssetProcessor` resource exists for the
    // processors to register with, and the server reads the processed side.
    crate::asset::install(&mut world, bake_options());

    crate::graphics::install(&mut world, schedule::Extract);

    // The graphics asset types, and with them the `install_processor` call that
    // registers `MeshProcessor` / `TextureProcessor` against the processor the
    // core just built.
    crate::graphics::install_assets(&mut world, schedule::Extract);

    world
}

/// Drives the world's processor over every source to completion, writing the
/// processed assets (and their `.meta` sidecars) under the processed root.
///
/// # Panics
///
/// If `world` carries no [`AssetProcessor`], which means it was not built by
/// [`bake_world`].
fn run_bake(world: &World) {
    let Some(processor) = world.get_resource::<AssetProcessor>().cloned() else {
        panic!("the world has no `AssetProcessor`: build it with `bake_world`");
    };
    pollster::block_on(processor.run_initial_processing());
}

/// Regenerates `imported_assets/Default` from the project sources.
///
/// This is the product-generation entry point: it is what turns the committed
/// sources under `res/` into the processed assets the runtime host reads.
pub fn bake_assets() {
    let world = bake_world(AssetSourceBuilders::default());
    run_bake(&world);
}
