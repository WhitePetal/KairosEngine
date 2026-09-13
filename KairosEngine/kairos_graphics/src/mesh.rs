use futures_lite::AsyncWriteExt;
use gltf::Gltf;
use rkyv::Archive;
use serde::{Deserialize, Serialize};

use kairos_asset::{
    Asset, AssetLoader, AssetProcessor, AssetSaver, AssetWorldExt, IdentityAssetTransformer,
    LoadContext, LoadTransformAndSave, LoadTransformAndSaveSettings, Process, ProcessContext,
    ProcessError, Reader, SavedAsset, VisitAssetDependencies, Writer,
};
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_math::{self as math, AABB, float2, float3, float4, float4x4, quaternion};
use kairos_tasks::ConditionalSendFuture;

use crate::consts::MESH_ASSETS_CAPACITY;
use crate::vertex::Vertex;

#[cfg(test)]
mod test;

pub mod wireframe;

#[derive(Debug, Clone, Serialize, Deserialize, Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

impl Asset for Mesh {}
impl VisitAssetDependencies for Mesh {}

/// Loads the **processed** [`Mesh`] produced by [`MeshProcessor`].
///
/// This is the processor's [`OutputLoader`](Process::OutputLoader): the product
/// is a single `rkyv` archive of the geometry, so the loader decodes the whole
/// reader with `rkyv` directly. There is no descriptor file and no companion
/// anymore — the settings the loader needs are carried by the product's `.meta`
/// (`AssetAction::Load` naming this loader with `()` settings).
#[derive(Debug)]
pub struct MeshLoader;

impl AssetLoader for MeshLoader {
    type Asset = Mesh;
    type Settings = ();
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Mesh, KairosError>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let mesh = rkyv::from_bytes::<Mesh, rkyv::rancor::Error>(&bytes)?;
            Ok(mesh)
        }
    }

    fn extensions(&self) -> &[&str] {
        &["mesh_bin"]
    }
}

/// Loads a [`Mesh`] from a `.glb` source: the load step of [`MeshProcessor`].
///
/// The geometry carries over from the retired `save_from_glb_file` path
/// byte-for-byte; only the shell changed — the bytes come from the processor's
/// reader instead of being opened from a path, and errors are typed rather than
/// printed.
#[derive(Debug)]
pub struct GltfMeshLoader;

impl AssetLoader for GltfMeshLoader {
    type Asset = Mesh;
    type Settings = ();
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Mesh, KairosError>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let gltf = Gltf::from_slice(&bytes)?;
            let buffers = gltf::import_buffers(&gltf.document, None, gltf.blob)?;
            let mesh = load_first_scene_mesh(&gltf.document, &buffers)
                .ok_or_else(|| KairosError::error("the .glb holds no triangle mesh"))?;
            Ok(mesh)
        }
    }

    fn extensions(&self) -> &[&str] {
        &["glb"]
    }
}

/// Writes a [`Mesh`] as the `rkyv` archive [`MeshLoader`] reads: the save step of
/// [`MeshProcessor`].
#[derive(Debug)]
pub struct MeshSaver;

impl AssetSaver for MeshSaver {
    type Asset = Mesh;
    type Settings = ();
    type OutputLoader = MeshLoader;
    type Error = KairosError;

    fn save(
        &self,
        writer: &mut Writer,
        asset: SavedAsset<'_, Mesh>,
        _settings: &(),
        _asset_path: kairos_asset::AssetPath<'_>,
    ) -> impl kairos_tasks::ConditionalSendFuture<Output = Result<(), KairosError>> {
        async move {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(asset.get())?;
            writer.write_all(&bytes).await?;
            Ok(())
        }
    }
}

/// Turns a `.glb` source into the processed [`Mesh`] product.
///
/// A `LoadTransformAndSave` wrapper: [`GltfMeshLoader`] loads the source,
/// an identity transform passes the mesh through, and [`MeshSaver`] writes the
/// archive. It has no settings of its own, so the `.meta`'s nested
/// [`LoadTransformAndSaveSettings`] is all-empty.
pub struct MeshProcessor(
    LoadTransformAndSave<GltfMeshLoader, IdentityAssetTransformer<Mesh>, MeshSaver>,
);

impl MeshProcessor {
    /// Creates the mesh processor.
    pub fn new() -> Self {
        Self(LoadTransformAndSave::new(
            IdentityAssetTransformer::new(),
            MeshSaver,
        ))
    }
}

impl Default for MeshProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Process for MeshProcessor {
    type Settings = LoadTransformAndSaveSettings<(), (), ()>;
    type OutputLoader = MeshLoader;

    fn process(
        &self,
        context: &mut ProcessContext,
        settings: &Self::Settings,
        writer: &mut Writer,
    ) -> impl ConditionalSendFuture<Output = Result<(), ProcessError>> {
        self.0.process(context, settings, writer)
    }
}

/// Registers the [`Mesh`] asset and its [`MeshLoader`] with the core.
///
/// Must run after [`kairos_asset::install`], which creates the
/// `AssetServer` and the `AssetStages` this reads.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<Mesh>(MESH_ASSETS_CAPACITY);
    world.register_asset_loader(MeshLoader);
    world.register_asset_loader(GltfMeshLoader);
}

/// Registers [`MeshProcessor`] with the processor and makes it the default for
/// `.glb` sources.
///
/// Called by the graphics install when the host runs in layout ②; a host with no
/// processor has nothing to register against.
pub fn install_processor(processor: &AssetProcessor) {
    processor.register_processor(MeshProcessor::new());
    processor.set_default_processor::<MeshProcessor>("glb");
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
    let node_to_world = parent_to_world * node_transform_matrix(&node);

    if let Some(gltf_mesh) = node.mesh() {
        for primitive in gltf_mesh.primitives() {
            if let Some(mesh) = load_mesh_from_primitive(primitive, node_to_world, buffers) {
                return Some(mesh);
            }
        }
    }

    for child in node.children() {
        if let Some(mesh) = load_mesh_from_node(child, node_to_world, buffers) {
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
            if let Some(mesh) = load_mesh_from_node(node, float4x4::IDENTITY, buffers) {
                return Some(mesh);
            }
        }
    }

    None
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u16>) -> Self {
        Self { vertices, indices }
    }

    pub fn compute_aabb(&self) -> AABB {
        use kairos_math::{Max, Min};
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
