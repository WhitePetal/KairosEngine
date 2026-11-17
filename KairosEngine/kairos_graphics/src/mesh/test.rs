use std::path::{Path, PathBuf};
use std::{thread, time::Duration};

use kairos_asset::io::{AssetSourceBuilder, AssetSourceBuilders, AssetSourceId};
use kairos_asset::{
    AssetAction, AssetEvent, AssetLoadFailedEvent, AssetMeta, AssetMetaDyn, AssetOptions,
    AssetProcessor, AssetServer, Assets, FileTransactionLogFactory, UntypedAssetLoadFailedEvent,
    handle_internal_asset_events, install,
};
use kairos_ecs::message::Messages;
use kairos_ecs::schedule::ScheduleLabel;
use kairos_ecs::world::World;
use kairos_math::float3;

use super::{GltfMeshLoader, Mesh, MeshLoader, MeshProcessor, install as install_mesh};
use crate::vertex::Vertex;

/// The three committed sample models (issue #149). They are decoded
/// exactly the way the runtime asset loader does (`rkyv::from_bytes` on
/// the product), so a future `Vertex` archive-layout change that
/// invalidates the checked-in binaries fails `cargo test` instead of
/// producing silent garbage or a runtime deserialization error.
///
/// Verification context (#149): exporting from the checked-in `.glb`
/// sources yields byte-identical archives to the committed files (the
/// rkyv layout is stable across the #148 68 B storage switch), so there
/// was no binary diff to commit; these tests pin that invariant. A
/// windowed editor smoke run also booted cleanly (frames rendered, no
/// rkyv/mesh/wgpu errors).
const SAMPLE_MODELS: [&str; 3] = ["Ball", "Plane", "Suzanne"];

/// The committed products mirror the sources under the processed root
/// (ADR 0004's `imported_assets/Default`, issue #239).
fn imported_models_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("kairos_engine sits under the workspace root")
        .join("imported_assets/Default/res/models")
}

fn read_or_panic(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn committed_product(name: &str) -> Vec<u8> {
    read_or_panic(&imported_models_dir().join(format!("{name}.glb")))
}

#[test]
fn committed_model_binaries_decode_at_current_layout() {
    for name in SAMPLE_MODELS {
        let mesh = decode_mesh(&committed_product(name), name);
        assert_mesh_sane(name, &mesh);
    }
}

fn decode_mesh(bytes: &[u8], what: &str) -> Mesh {
    rkyv::from_bytes::<Mesh, rkyv::rancor::Error>(bytes)
        .unwrap_or_else(|e| panic!("rkyv decode of {what}: {e}"))
}

/// Semantic sanity of a decoded mesh. Under a stale/repacked archive the
/// per-vertex fields shift and these checks fail loudly: unit-length
/// normals, in-range indices and `w == 1.0` positions cannot survive a
/// layout mismatch across hundreds of vertices by accident. The normals
/// are the data behind the GPU `Float32x3` attribute at shader location
/// 3, i.e. what lit shading reads.
fn assert_mesh_sane(name: &str, mesh: &Mesh) {
    let vertices = mesh.vertices();
    let indices = mesh.indices();

    assert!(!vertices.is_empty(), "{name}: empty vertex list");
    assert_eq!(
        indices.len() % 3,
        0,
        "{name}: {} indices is not a triangle multiple",
        indices.len()
    );
    for &i in indices {
        assert!(
            usize::from(i) < vertices.len(),
            "{name}: index {i} out of range ({} vertices)",
            vertices.len()
        );
    }

    for (i, v) in vertices.iter().enumerate() {
        // 68 B/vertex, strict no-pad layout is asserted by
        // `vertex::test::layout_has_no_padding`; here we only verify the
        // decoded payload is well-formed under that layout.
        for c in v.position[..3]
            .iter()
            .chain(v.color.iter())
            .chain(v.texcoord.to_array().iter())
        {
            assert!(c.is_finite(), "{name}: vertex {i} attribute not finite");
        }
        assert_eq!(v.position[3], 1.0, "{name}: vertex {i} position.w != 1");

        // The `Float32x3` normal attribute drives lit shading: it must be
        // a finite unit vector at the current stride/offset.
        let [nx, ny, nz] = v.normal;
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        assert!(
            (len - 1.0).abs() < 1e-3,
            "{name}: vertex {i} normal not unit length ({len})"
        );
    }
}

/// The three ad-hoc stages the asset drivers are installed into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Tracking;

impl ScheduleLabel for Tracking {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Events;

impl ScheduleLabel for Events {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Boot;

impl ScheduleLabel for Boot {
    fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
        Box::new(*self)
    }
}

/// A product + `.meta` pair loads through the core: the sidecar names the output
/// loader, so the extension is only a fallback.
#[test]
fn mesh_loads_through_the_core() {
    let dir = tempfile::Builder::new()
        .tempdir_in(".")
        .expect("a temp dir in the cwd");
    let full_path = dir.path().join("Probe").with_extension("mesh_bin");

    let mesh = Mesh::new(
        vec![
            Vertex::with_position(float3::new(0.0, 0.0, 0.0)),
            Vertex::with_position(float3::new(1.0, 0.0, 0.0)),
            Vertex::with_position(float3::new(0.0, 1.0, 0.0)),
        ],
        vec![0, 1, 2],
    );
    std::fs::write(
        &full_path,
        rkyv::to_bytes::<rkyv::rancor::Error>(&mesh).expect("archive the mesh"),
    )
    .expect("write the mesh product");

    let meta = AssetMeta::<MeshLoader, ()>::new(AssetAction::Load {
        loader: kairos_asset::loader_name::<MeshLoader>().to_string(),
        settings: (),
    });
    std::fs::write(
        kairos_asset::io::get_meta_path(&full_path),
        AssetMetaDyn::serialize(&meta),
    )
    .expect("write the mesh meta");

    let cwd = std::env::current_dir().expect("the cwd");
    let rel_path = full_path
        .strip_prefix(&cwd)
        .expect("the temp dir is under the cwd")
        .to_path_buf();

    let mut world = World::new();
    install(&mut world, AssetOptions::new(Tracking, Events, Boot));
    install_mesh(&mut world);

    let handle = world.resource::<AssetServer>().load::<Mesh>(rel_path);

    let mut loaded = None;
    for _ in 0..200 {
        world.run_schedule(Tracking);
        if let Some(mesh) = world.resource::<Assets<Mesh>>().get(handle.id()) {
            loaded = Some((mesh.vertices.len(), mesh.indices.clone()));
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    assert_eq!(loaded, Some((3, vec![0, 1, 2])));
}
