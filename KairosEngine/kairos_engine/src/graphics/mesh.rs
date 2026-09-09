use std::path::PathBuf;

use gltf::Gltf;
use rkyv::Archive;
use serde::{Deserialize, Serialize};

use crate::{
    graphics::vertex::Vertex,
    math::{self, float2, float3, float4, float4x4, quaternion},
    spatial::AABB,
};

pub mod wireframe;

#[derive(Debug, Clone, Serialize, Deserialize, Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedMeshAsset {
    pub source_path: PathBuf,
}
impl SerializedMeshAsset {
    pub fn save_from_glb_file(path: PathBuf) {
        let Ok(gltf) = Gltf::open(path.clone()) else {
            println!("Open gltf fiel failed");
            return;
        };
        let Ok(buffer_data) = gltf::import_buffers(&gltf.document, Some(&path), gltf.blob) else {
            println!("Import mesh buffers failed");
            return;
        };
        let mesh = Self::load_first_scene_mesh(&gltf.document, &buffer_data);
        let Some(mesh) = mesh else {
            println!("Load Mesh failed");
            return;
        };
        let Ok(bytes) = rkyv::to_bytes::<rkyv::rancor::Error>(&mesh) else {
            println!("Serialize mesh to bytes failed");
            return;
        };
        let mut bin_path = path.clone();
        bin_path.set_extension("mesh_bin");
        match std::fs::write(bin_path.clone(), bytes) {
            Ok(_) => {}
            Err(_) => {
                println!("Save mesh bytes failed");
            }
        }
        let serialized_mesh = SerializedMeshAsset {
            source_path: bin_path.clone(),
        };
        let Ok(serialized_mesh_toml) = toml::to_string(&serialized_mesh) else {
            println!("Serialize mesh toml failed");
            return;
        };
        bin_path.set_extension("mesh");
        match std::fs::write(bin_path, serialized_mesh_toml) {
            Ok(_) => {}
            Err(_) => {
                println!("Save mesh toml failed");
            }
        };
    }

    fn node_transform_matrix(node: &gltf::Node<'_>) -> float4x4 {
        let (translation, rotation, scale) = node.transform().decomposed();

        float4x4::trs(
            float3::from(translation),
            quaternion::new(rotation[0], rotation[1], rotation[2], rotation[3]),
            float3::from(scale),
        )
    }

    fn load_mesh_from_primitive(
        primitive: gltf::Primitive<'_>,
        node_to_world: float4x4,
        buffers: &[gltf::buffer::Data],
    ) -> Option<Mesh> {
        if primitive.mode() != gltf::mesh::Mode::Triangles {
            return None;
        }

        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()].0));
        let positions = reader.read_positions()?;
        let vertex_count = positions.len();
        let mut colors = reader.read_colors(0).map(|colors| colors.into_rgba_f32());
        let mut texcoords = reader
            .read_tex_coords(0)
            .map(|texcoords| texcoords.into_f32());
        let mut normals = reader.read_normals();
        let mut tangents = reader.read_tangents();

        let mut vertices = Vec::with_capacity(vertex_count);
        for position in positions {
            let color = colors
                .as_mut()
                .and_then(|colors| colors.next())
                .map(float4::from)
                .unwrap_or(float4::new(1.0, 1.0, 1.0, 1.0));
            let texcoord = texcoords
                .as_mut()
                .and_then(|texcoords| texcoords.next())
                .map(float2::from_array)
                .unwrap_or(float2::new(0.0, 0.0));
            let normal = normals
                .as_mut()
                .and_then(|normals| normals.next())
                .map(float3::from)
                .unwrap_or(float3::new(0.0, 0.0, 1.0));
            let tangent = tangents
                .as_mut()
                .and_then(|tangents| tangents.next())
                .unwrap_or([1.0, 0.0, 0.0, 1.0]);

            let position = (node_to_world * float4::from((float3::from(position), 1.0))).xyz();
            let normal = math::normalize((node_to_world * float4::from((normal, 0.0))).xyz());
            let tangent_xyz = math::normalize(
                (node_to_world
                    * float4::from((float3::new(tangent[0], tangent[1], tangent[2]), 0.0)))
                .xyz(),
            );

            vertices.push(Vertex {
                position: float4::from((position, 1.0)).to_array(),
                color: color.to_array(),
                texcoord,
                normal: Vertex::pack_normal(normal),
                tangent: float4::from((tangent_xyz, tangent[3])).to_array(),
            });
        }

        let indices = reader
            .read_indices()
            .map(|indices| {
                indices
                    .into_u32()
                    .map(u16::try_from)
                    .collect::<Result<Vec<_>, _>>()
                    .ok()
            })
            .unwrap_or_else(|| {
                (0..vertices.len())
                    .map(u16::try_from)
                    .collect::<Result<Vec<_>, _>>()
                    .ok()
            })?;

        Some(Mesh::new(vertices, indices))
    }

    fn load_mesh_from_node(
        node: gltf::Node<'_>,
        parent_to_world: float4x4,
        buffers: &[gltf::buffer::Data],
    ) -> Option<Mesh> {
        let node_to_world = parent_to_world * Self::node_transform_matrix(&node);

        if let Some(gltf_mesh) = node.mesh() {
            for primitive in gltf_mesh.primitives() {
                if let Some(mesh) =
                    Self::load_mesh_from_primitive(primitive, node_to_world, buffers)
                {
                    return Some(mesh);
                }
            }
        }

        for child in node.children() {
            if let Some(mesh) = Self::load_mesh_from_node(child, node_to_world, buffers) {
                return Some(mesh);
            }
        }

        None
    }

    fn load_first_scene_mesh(
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> Option<Mesh> {
        for scene in document.scenes() {
            for node in scene.nodes() {
                if let Some(mesh) = Self::load_mesh_from_node(node, float4x4::IDENTITY, buffers) {
                    return Some(mesh);
                }
            }
        }

        None
    }
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u16>) -> Self {
        Self { vertices, indices }
    }

    pub fn compute_aabb(&self) -> AABB {
        use crate::math::{Max, Min};
        use rayon::prelude::*;

        let (min, max) = self
            .vertices
            .par_iter()
            .map(|v| float3::from_array_4(v.position))
            .map(|p| (p, p))
            .reduce(
                || {
                    (
                        float3::new(f32::MAX, f32::MAX, f32::MAX),
                        float3::new(f32::MIN, f32::MIN, f32::MIN),
                    )
                },
                |(min1, max1), (min2, max2)| (min1.min(min2), max1.max(max2)),
            );

        AABB { max, min }
    }

    /// Access the vertex slice (read-only).
    #[inline(always)]
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// Access the index slice (read-only).
    #[inline(always)]
    pub fn indices(&self) -> &[u16] {
        &self.indices
    }
}

#[cfg(test)]
mod test {
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
}
