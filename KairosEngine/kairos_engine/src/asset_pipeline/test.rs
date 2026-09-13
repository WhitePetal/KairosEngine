//! Tests for the offline processing run: it must reuse the engine's own install
//! path (so `install_processor` is reached) and write decodable processed assets.

use std::path::{Path, PathBuf};

use kairos_asset::io::{AssetSourceBuilder, AssetSourceBuilders, AssetSourceId, get_meta_path};
use kairos_asset::{AssetProcessor, FileTransactionLogFactory};

use super::{bake_world, run_bake};

/// The workspace root holds `res/` and the committed processed assets; the crate
/// lives one level below it.
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace root sits one level above the crate")
}

/// A committed sample model and its `.meta`, staged under a fresh temp tree whose
/// source and processed roots both live inside it: the run never touches the
/// tracked `res/` or `imported_assets/`.
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
/// `<root>/imported_assets/Default/models/Ball.glb`. Running the bake over the
/// default (working-directory) source puts the same file at
/// `imported_assets/Default/res/models/Ball.glb`.
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
