use std::path::{Path, PathBuf};

use super::{Mesh, SerializedMeshAsset};

/// The three committed sample models (issue #149). They are decoded
/// exactly the way the runtime asset loader does (`rkyv::from_bytes` on
/// the `.mesh_bin` companion), so a future `Vertex` archive-layout change
/// that invalidates the checked-in binaries fails `cargo test` instead of
/// producing silent garbage or a runtime deserialization error.
///
/// Verification context (#149): exporting from the checked-in `.glb`
/// sources yields byte-identical archives to the committed files (the
/// rkyv layout is stable across the #148 68 B storage switch), so there
/// was no binary diff to commit; these tests pin that invariant. A
/// windowed editor smoke run also booted cleanly (frames rendered, no
/// rkyv/mesh/wgpu errors).
const SAMPLE_MODELS: [&str; 3] = ["Ball", "Plane", "Suzanne"];

/// The workspace root holds `res/`; the crate lives one level below it.
fn models_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("kairos_engine sits under the workspace root")
        .join("res/models")
}

fn read_or_panic(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn committed_bin(name: &str) -> Vec<u8> {
    read_or_panic(&models_dir().join(name).with_extension("mesh_bin"))
}

#[test]
fn committed_model_binaries_decode_at_current_layout() {
    for name in SAMPLE_MODELS {
        let mesh = decode_mesh(&committed_bin(name), name);
        assert_mesh_sane(name, &mesh);
    }
}

/// A fresh export must be byte-identical to the committed binaries
/// (guards the runtime exporter against drift). The export runs in a
/// throwaway directory so tracked `res/models` files are never touched.
#[test]
fn fresh_export_is_byte_identical_to_committed_binaries() {
    for name in SAMPLE_MODELS {
        let temp = tempfile::tempdir().expect("temp dir for export");
        let glb_path = temp.path().join(name).with_extension("glb");
        std::fs::copy(models_dir().join(name).with_extension("glb"), &glb_path)
            .unwrap_or_else(|e| panic!("copy {name}.glb into temp dir: {e}"));

        SerializedMeshAsset::save_from_glb_file(glb_path.clone());
        let exported = read_or_panic(&glb_path.with_extension("mesh_bin"));

        assert_eq!(
            exported,
            committed_bin(name),
            "{name}: fresh export differs from the committed .mesh_bin"
        );
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
