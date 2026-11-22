//! Generating the processed assets under `Library/imported_assets/Default`.
//!
//! The runtime host runs in [`AssetMode::Unprocessed`] — layout ① — and reads
//! the processed assets by path: the project tree pairs a source (`.glb` /
//! `.png`) with the processed asset under `Library/imported_assets/Default`, and that
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

use std::path::{Path, PathBuf};

use kairos_asset::io::{AssetSourceBuilder, AssetSourceBuilders, AssetSourceId};
use kairos_asset::{AssetMode, AssetOptions, AssetProcessor};
use kairos_ecs::world::World;
use kairos_ignore::IgnoreRules;

use crate::schedule;

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
/// working directory as the source root and `Library/imported_assets/Default` as the
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

/// Regenerates `Library/imported_assets/Default` from the project sources.
///
/// This is the product-generation entry point: it is what turns the committed
/// sources under `res/` into the processed assets the runtime host reads. The
/// source is the project's *visible* content: the same `.kairosignore` rules the
/// project browser applies decide which directories the bake never enters, so a
/// directory the browser cannot show never gets a product (ADR 0018).
pub fn bake_assets() {
    let root = std::env::current_dir().unwrap_or_default();
    let world = bake_world(bake_source_builders(&root));
    run_bake(&world);
}

/// The default source for the project rooted at `root`, minus the directories
/// its ignore rules exclude.
///
/// This is `AssetOptions`' own default source — the project root as the
/// unprocessed root, `Library/imported_assets/Default` as the processed one
/// (ADR 0004 / 0017) — with one addition: every directory `.kairosignore` rules
/// out is passed as an unprocessed exclusion, so the processor's walk and the
/// project browser agree about what the project owns.
fn bake_source_builders(root: &Path) -> AssetSourceBuilders {
    let rules = ignore_rules(root);
    let excluded = excluded_directories(root, &rules);

    let processed = root.join(AssetOptions::DEFAULT_PROCESSED_FILE_PATH);
    let builder = match (root.to_str(), processed.to_str()) {
        (Some(unprocessed), Some(processed)) => {
            AssetSourceBuilder::platform_default(unprocessed, Some(processed))
        }
        _ => {
            // A non-UTF-8 project root cannot be spelled as the asset core's
            // relative source root; fall back to the working directory's own
            // defaults rather than refusing to bake anything.
            log::warn!(
                "bake: the project root {} is not valid UTF-8; using the working directory's source roots",
                root.display()
            );
            AssetSourceBuilder::platform_default(
                "",
                Some(AssetOptions::DEFAULT_PROCESSED_FILE_PATH),
            )
        }
    };

    if !excluded.is_empty() {
        log::info!(
            "bake: {} directories ignored by {}: {}",
            excluded.len(),
            ignore_file(root).display(),
            summarize(&excluded)
        );
    }
    let builder = builder.with_unprocessed_excludes(excluded);

    let mut builders = AssetSourceBuilders::default();
    builders.insert(AssetSourceId::Default, builder);
    builders
}

/// The project's ignore file, at the project root.
fn ignore_file(root: &Path) -> PathBuf {
    root.join(".kairosignore")
}

/// Read the project's ignore rules, falling back to the built-in defaults.
///
/// A missing file is the normal case (the editor seeds it on first open; a
/// headless bake may run before that), so it is not worth a warning. An
/// unreadable file and an unsupported rule are worth one each: silently baking
/// content the browser hides is exactly the divergence the shared rules exist to
/// prevent.
fn ignore_rules(root: &Path) -> IgnoreRules {
    let path = ignore_file(root);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!(
                    "bake: cannot read {}: {error}; using the built-in ignore defaults",
                    path.display()
                );
            }
            return IgnoreRules::defaults();
        }
    };
    let (rules, rejected) = IgnoreRules::parse(text.lines().map(str::to_owned));
    for line in rejected {
        log::warn!("bake: unsupported rule {line:?} in {} (skipped)", path.display());
    }
    rules
}

/// Every directory under `root` that the rules exclude, relative to `root`.
///
/// The walk stops at an ignored directory instead of descending into it — the
/// same shape the project browser's scan has — so the result is the topmost
/// ignored directory of each subtree, which is what an unprocessed exclusion
/// wants.
fn excluded_directories(root: &Path, rules: &IgnoreRules) -> Vec<PathBuf> {
    let mut excluded = Vec::new();
    let mut stack = vec![String::new()];
    while let Some(relative) = stack.pop() {
        let directory = if relative.is_empty() {
            root.to_path_buf()
        } else {
            root.join(&relative)
        };
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            // Symlinked directories are not followed, matching the browser's walk:
            // an unprocessed exclusion would recurse through them.
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            if rules.is_ignored(&child, &name) {
                excluded.push(PathBuf::from(&child));
            } else {
                stack.push(child);
            }
        }
    }
    excluded
}

/// The first few excluded directories, for the log line.
fn summarize(paths: &[PathBuf]) -> String {
    const SHOWN: usize = 6;
    let mut text = paths
        .iter()
        .take(SHOWN)
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if paths.len() > SHOWN {
        text.push_str(&format!(", ... ({} more)", paths.len() - SHOWN));
    }
    text
}
