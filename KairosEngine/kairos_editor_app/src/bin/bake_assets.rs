//! The editor's offline bake: regenerate `Library/imported_assets/Default`.
//!
//! Baking is an authoring-time **editor** feature, not a runtime engine one. The
//! engine provides the instruments —
//! [`bake_world`](kairos_engine::asset_pipeline::bake_world) boots the asset core
//! in layout ② and
//! [`run_bake`](kairos_engine::asset_pipeline::run_bake) drives its processor to
//! completion — but *what the project is* is the editor's question. This binary
//! answers it: the project root is the working directory, and the directories the
//! project browser cannot show (the same `.kairosignore` rules the panel applies)
//! are the directories the bake never walks (ADR 0018).
//!
//! That is why this binary lives here and not in `kairos_engine`: the engine must
//! not read editor configuration, and the ignore rules are the project browser's
//! own (`kairos_editor_core::project`). The engine exports an instrument, the
//! editor owns the policy.
//!
//! Run it from the project root:
//!
//! ```text
//! cargo run --bin bake_assets
//! ```

use std::path::{Path, PathBuf};

use kairos_editor_core::project::{IgnoreRules, RelPath};
use kairos_engine::asset::AssetOptions;
use kairos_engine::asset::io::{AssetSourceBuilder, AssetSourceBuilders, AssetSourceId};
use kairos_engine::asset_pipeline::{bake_world, run_bake};

/// The project's ignore file, at the project root — the same name and the same
/// grammar the project browser reads.
const IGNORE_FILE: &str = ".kairosignore";

fn main() {
    // The process logger, without the run-log file: a bake is not an editor
    // session, and truncating `Library/Logs/KairosEngine.log` mid-run would hide
    // whatever the editor had logged.
    let _log = kairos_log::init_with(kairos_log::Config {
        file: None,
        ..kairos_log::Config::default()
    });

    if let Err(error) = bake() {
        kairos_log::error!(target: "kairos_editor", "bake: {error}");
        std::process::exit(1);
    }
}

/// Bake the project rooted at the working directory.
fn bake() -> Result<(), String> {
    let root = std::env::current_dir()
        .map_err(|error| format!("cannot resolve the project root: {error}"))?;
    kairos_log::info!(target: "kairos_editor", "bake: project {}", root.display());

    let rules = ignore_rules(&root);
    let excluded = excluded_directories(&root, &rules);
    if excluded.is_empty() {
        kairos_log::info!(
            target: "kairos_editor",
            "bake: every directory is visible to the project"
        );
    } else {
        kairos_log::info!(
            target: "kairos_editor",
            "bake: {} directories ignored by {}: {}",
            excluded.len(),
            root.join(IGNORE_FILE).display(),
            summarize(&excluded)
        );
    }

    let world = bake_world(source_builders(&root, excluded));
    run_bake(&world);
    kairos_log::info!(target: "kairos_editor", "bake: done");
    Ok(())
}

/// The project's ignore rules, falling back to the built-in defaults.
///
/// A missing file is the normal case for a project the editor has not opened yet,
/// so it is not worth a warning; an unreadable file and an unsupported rule are
/// worth one each, because silently baking content the browser hides is exactly
/// the divergence the shared rules exist to prevent.
fn ignore_rules(root: &Path) -> IgnoreRules {
    let path = root.join(IGNORE_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                kairos_log::warn!(
                    target: "kairos_editor",
                    "bake: cannot read {}: {error}; using the built-in ignore defaults",
                    path.display()
                );
            }
            return IgnoreRules::defaults();
        }
    };
    let (rules, rejected) = IgnoreRules::parse(text.lines().map(str::to_owned));
    for line in rejected {
        kairos_log::warn!(
            target: "kairos_editor",
            "bake: unsupported rule {line:?} in {} (skipped)",
            path.display()
        );
    }
    rules
}

/// Every directory under `root` the rules exclude, relative to `root`.
///
/// The walk stops at an ignored directory instead of descending into it — the same
/// shape the project browser's scan has — so the result is the topmost ignored
/// directory of each subtree, which is what an unprocessed exclusion wants.
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
            // Symlinked directories are not followed, matching the browser's walk.
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            if rules.is_ignored(&RelPath::new(child.clone()), &name) {
                excluded.push(PathBuf::from(&child));
            } else {
                stack.push(child);
            }
        }
    }
    excluded
}

/// The default source for the project rooted at `root`, minus `excluded`.
///
/// This is `AssetOptions`' own default source — the project root as the unprocessed
/// root, `Library/imported_assets/Default` as the processed one (ADR 0004 / 0017) —
/// with the project's ignored directories passed as unprocessed exclusions, so the
/// processor's walk and the project browser agree about what the project owns.
fn source_builders(root: &Path, excluded: Vec<PathBuf>) -> AssetSourceBuilders {
    let processed = root.join(AssetOptions::DEFAULT_PROCESSED_FILE_PATH);
    let builder = match (root.to_str(), processed.to_str()) {
        (Some(unprocessed), Some(processed)) => {
            AssetSourceBuilder::platform_default(unprocessed, Some(processed))
        }
        _ => {
            // A non-UTF-8 project root cannot be spelled as the asset core's
            // source root; the working directory's own defaults are the same place.
            kairos_log::warn!(
                target: "kairos_editor",
                "bake: the project root {} is not valid UTF-8; using the working directory's source roots",
                root.display()
            );
            AssetSourceBuilder::platform_default(
                "",
                Some(AssetOptions::DEFAULT_PROCESSED_FILE_PATH),
            )
        }
    };
    let mut builders = AssetSourceBuilders::default();
    builders.insert(
        AssetSourceId::Default,
        builder.with_unprocessed_excludes(excluded),
    );
    builders
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

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_engine::asset::{AssetProcessor, FileTransactionLogFactory};

    /// The workspace root: the crate lives one level under it, and it holds the
    /// committed sources these cases stage.
    fn workspace_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("kairos_editor_app sits under the workspace root")
    }

    /// A project root with one visible and one ignored copy of the same source.
    fn project() -> tempfile::TempDir {
        let root = tempfile::TempDir::new().expect("a temp dir");
        // A realistic project file spells the built-in defaults out: the file
        // replaces them, it does not add to them.
        std::fs::write(
            root.path().join(IGNORE_FILE),
            ".*\ntarget\nignored\n",
        )
        .expect("the ignore file");
        for directory in ["res", "ignored"] {
            let target = root.path().join(directory);
            std::fs::create_dir_all(&target).expect("the source tree");
            for suffix in [".png", ".png.meta"] {
                let staged = workspace_root().join(format!("res/textures/white{suffix}"));
                std::fs::copy(&staged, target.join(format!("white{suffix}")))
                    .unwrap_or_else(|error| panic!("stage {}: {error}", staged.display()));
            }
        }
        root
    }

    /// The exclusion collector: the topmost ignored directory of each subtree,
    /// using the same rules the project browser applies.
    #[test]
    fn excluded_directories_finds_every_ignored_subtree() {
        let root = tempfile::TempDir::new().expect("a temp dir");
        std::fs::write(
            root.path().join(IGNORE_FILE),
            ".*\ntarget\nignored\nres/generated\n",
        )
        .expect("the ignore file");
        for directory in [
            "target/debug",
            ".git/objects",
            "ignored/nested",
            "res/generated/tex",
            "res/kept",
        ] {
            std::fs::create_dir_all(root.path().join(directory)).expect("a fixture directory");
        }

        let rules = ignore_rules(root.path());
        let mut excluded: Vec<String> = excluded_directories(root.path(), &rules)
            .into_iter()
            .map(|path| path.display().to_string())
            .collect();
        excluded.sort();

        assert_eq!(
            excluded,
            vec![
                String::from(".git"),
                String::from("ignored"),
                String::from("res/generated"),
                String::from("target"),
            ],
            "each ignored subtree is named once, at its topmost directory"
        );
    }

    /// A missing ignore file is not an error: the bake falls back to the same
    /// defaults the editor seeds, so a fresh checkout bakes consistently.
    #[test]
    fn a_missing_ignore_file_falls_back_to_the_defaults() {
        let root = tempfile::TempDir::new().expect("a temp dir");

        let rules = ignore_rules(root.path());

        assert_eq!(rules, IgnoreRules::defaults());
    }

    /// The source carries the project's ignored directories as unprocessed
    /// exclusions, next to the processed root's own (ADR 0017).
    #[test]
    fn the_source_excludes_the_projects_ignored_directories() {
        let root = project();
        let rules = ignore_rules(root.path());
        let world = bake_world(source_builders(root.path(), excluded_directories(root.path(), &rules)));
        let processor = world.resource::<AssetProcessor>();
        let source = processor
            .get_source(AssetSourceId::Default)
            .expect("the default source is registered");
        let mut excludes: Vec<PathBuf> = source.unprocessed_excludes().to_vec();
        excludes.sort();

        assert_eq!(
            excludes,
            [PathBuf::from("Library"), PathBuf::from("ignored")],
            "the processed root and the ignored directory are excluded"
        );
    }

    /// The bake leaves no product for what the project browser cannot show.
    #[test]
    fn the_bake_leaves_no_product_for_an_ignored_directory() {
        let root = project();
        let rules = ignore_rules(root.path());
        let world = bake_world(source_builders(root.path(), excluded_directories(root.path(), &rules)));
        // Keep the transaction log inside the temp tree; the processor otherwise
        // writes it beside its processed root.
        world
            .resource::<AssetProcessor>()
            .data()
            .set_log_factory(Box::new(FileTransactionLogFactory {
                file_path: root.path().join("log"),
            }))
            .expect("the log factory is set before the processor starts");

        run_bake(&world);

        let processed = root.path().join("Library/imported_assets/Default");
        assert!(
            processed.join("res/white.png").is_file(),
            "a visible source still gets its product"
        );
        assert!(
            !processed.join("ignored").exists(),
            "an ignored directory must not be walked, so it has no product"
        );
    }
}