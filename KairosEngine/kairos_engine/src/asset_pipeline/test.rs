//! Tests for the offline processing run: it must reuse the engine's own install
//! path (so `install_processor` is reached) and write decodable processed assets.

use std::path::{Path, PathBuf};

use kairos_asset::io::{AssetSourceBuilder, AssetSourceBuilders, AssetSourceId, get_meta_path};
use kairos_asset::{AssetProcessor, FileTransactionLogFactory};

use super::{bake_source_builders, bake_world, excluded_directories, ignore_rules, run_bake};

/// The workspace root holds `res/` and the committed processed assets; the crate
/// lives one level below it.
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace root sits one level above the crate")
}

/// A committed sample model and its `.meta`, staged under a fresh temp tree whose
/// source and processed roots both live inside it: the run never touches the
/// tracked `res/` or `Library/imported_assets/`.
fn staged_source(name: &str) -> (tempfile::TempDir, AssetSourceBuilders) {
    let root = tempfile::TempDir::new().expect("a temp dir");
    let sources = root.path().join("res/models");
    std::fs::create_dir_all(&sources).expect("the source tree");

    for suffix in [".glb", ".glb.meta"] {
        let staged = workspace_root().join(format!("res/models/{name}{suffix}"));
        std::fs::copy(&staged, sources.join(format!("{name}{suffix}")))
            .unwrap_or_else(|error| panic!("stage {}: {error}", staged.display()));
    }

    let mut builders = AssetSourceBuilders::default();
    builders.insert(
        AssetSourceId::Default,
        AssetSourceBuilder::platform_default(
            root.path().join("res").to_str().expect("utf-8 path"),
            Some(
                root.path()
                    .join("imported_assets/Default")
                    .to_str()
                    .expect("utf-8 path"),
            ),
        ),
    );
    (root, builders)
}

fn read_or_panic(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// The processed asset mirrors the source's path *relative to the source root*
/// under the processed root: `<root>/res/models/Ball.glb` becomes
/// `<root>/imported_assets/Default/models/Ball.glb` — the processed root is whatever
/// the caller passes, so these tests keep their own literals. Running the bake over
/// the default (working-directory) source puts the same file at
/// `Library/imported_assets/Default/res/models/Ball.glb` (ADR 0017).
fn processed_path(root: &Path, name: &str) -> PathBuf {
    root.join("imported_assets/Default/models")
        .join(format!("{name}.glb"))
}

/// The run reaches `install_processor` through the engine's own install path,
/// and the processors it registers turn a `.glb` source into a processed asset
/// the runtime's loader decodes.
#[test]
fn the_run_registers_the_graphics_processors_and_writes_a_processed_asset() {
    let (root, builders) = staged_source("Ball");
    let world = bake_world(builders);

    // `graphics::install_assets` found the core's `AssetProcessor` and registered
    // the mesh and texture processors with it, one per source extension.
    let processor = world.resource::<AssetProcessor>();
    assert!(
        processor.get_default_processor("glb").is_some(),
        "the mesh processor must be registered for `.glb` sources"
    );
    assert!(
        processor.get_default_processor("png").is_some(),
        "the texture processor must be registered for `.png` sources"
    );

    // Keep the transaction log inside the temp tree; the processor otherwise
    // writes it beside its processed root.
    processor
        .data()
        .set_log_factory(Box::new(FileTransactionLogFactory {
            file_path: root.path().join("log"),
        }))
        .expect("the log factory is set before the processor starts");

    run_bake(&world);

    let asset = read_or_panic(&processed_path(root.path(), "Ball"));
    let mesh = rkyv::from_bytes::<kairos_graphics::mesh::Mesh, rkyv::rancor::Error>(&asset)
        .expect("the processed asset decodes as a `Mesh`");
    assert!(
        !mesh.vertices().is_empty(),
        "the processed asset holds no vertices"
    );
    assert_eq!(
        mesh.indices().len() % 3,
        0,
        "the processed asset's indices are not a triangle multiple"
    );

    // The processed asset's sidecar is the `AssetAction::Load` hand-off to the
    // loader the runtime reads it with.
    let meta = read_or_panic(&get_meta_path(&processed_path(root.path(), "Ball")));
    let meta = String::from_utf8(meta).expect("the processed asset's `.meta` is text");
    assert!(
        meta.contains("kairos_graphics::mesh::MeshLoader"),
        "the processed asset's `.meta` must name the output loader, got:\n{meta}"
    );
}

/// A project root whose `.kairosignore` rules out `ignored`, with the same model
/// staged twice: under `res/` (visible) and under `ignored/` (not visible).
fn project_with_an_ignored_directory() -> tempfile::TempDir {
    let root = tempfile::TempDir::new().expect("a temp dir");
    std::fs::write(root.path().join(".kairosignore"), "ignored\n").expect("the ignore file");
    for directory in ["res/models", "ignored/models"] {
        let target = root.path().join(directory);
        std::fs::create_dir_all(&target).expect("the source tree");
        for suffix in [".glb", ".glb.meta"] {
            let staged = workspace_root().join(format!("res/models/Ball{suffix}"));
            std::fs::copy(&staged, target.join(format!("Ball{suffix}")))
                .unwrap_or_else(|error| panic!("stage {}: {error}", staged.display()));
        }
    }
    root
}

/// The bake's source carries the project's ignored directories as unprocessed
/// exclusions (plus the processed root's own, ADR 0018 / 0017).
#[test]
fn the_bake_source_excludes_the_projects_ignored_directories() {
    let root = project_with_an_ignored_directory();
    let world = bake_world(bake_source_builders(root.path()));
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

/// What the project browser cannot show the bake does not process: the ignored
/// copy of the model leaves no product, the visible one does.
#[test]
fn the_bake_leaves_no_product_for_an_ignored_directory() {
    let root = project_with_an_ignored_directory();
    let world = bake_world(bake_source_builders(root.path()));
    let processor = world.resource::<AssetProcessor>();
    // Keep the transaction log inside the temp tree; the processor otherwise
    // writes it beside its processed root.
    processor
        .data()
        .set_log_factory(Box::new(FileTransactionLogFactory {
            file_path: root.path().join("log"),
        }))
        .expect("the log factory is set before the processor starts");

    run_bake(&world);

    let processed = root.path().join("Library/imported_assets/Default");
    assert!(
        processed.join("res/models/Ball.glb").is_file(),
        "a visible source still gets its product"
    );
    assert!(
        !processed.join("ignored").exists(),
        "an ignored directory must not be walked, so it has no product"
    );
}

/// The exclusion collector: the topmost ignored directory of each subtree, using
/// the same rules the project browser applies.
#[test]
fn excluded_directories_finds_every_ignored_subtree() {
    let root = tempfile::TempDir::new().expect("a temp dir");
    // The file replaces the built-in defaults, so a realistic project file spells
    // the defaults out (the editor seeds exactly these two lines).
    std::fs::write(
        root.path().join(".kairosignore"),
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

/// A missing ignore file is not an error: the bake falls back to the same defaults
/// the editor seeds, so a fresh checkout bakes consistently.
#[test]
fn a_missing_ignore_file_falls_back_to_the_defaults() {
    let root = tempfile::TempDir::new().expect("a temp dir");

    let rules = ignore_rules(root.path());

    assert_eq!(rules, kairos_ignore::IgnoreRules::defaults());
}