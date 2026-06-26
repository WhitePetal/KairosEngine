use std::path::PathBuf;

use gltf::Gltf;
use rkyv::Archive;
use serde::{Deserialize, Serialize};

use crate::{graphics::vertex::Vertex, math::{self, float2, float3, float4, float4x4, quaternion}};

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
        bin_path.set_extension(".mesh_bin");
        match std::fs::write(bin_path, bytes) {
            Ok(_) => {},
            Err(_) => {
                println!("Save mesh bytes failed")
            },
        } 
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
                (node_to_world * float4::from((float3::new(tangent[0], tangent[1], tangent[2]), 0.0)))
                    .xyz(),
            );

            vertices.push(Vertex {
                position: float4::from((position, 1.0)),
                color,
                texcoord,
                normal,
                tangent: float4::from((tangent_xyz, tangent[3])),
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
                if let Some(mesh) = Self::load_mesh_from_primitive(primitive, node_to_world, buffers) {
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
                if let Some(mesh) = Self::load_mesh_from_node(node, float4x4::identity(), buffers) {
                    return Some(mesh);
                }
            }
        }

        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshAsset {
    pub mesh: Mesh,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u16>) -> Self {
        Self { vertices, indices }
    }
}
