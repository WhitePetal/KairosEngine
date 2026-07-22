use rayon::prelude::*;

use crate::{
    graphics::{mesh::Mesh, vertex::Vertex},
    math::{self, float3},
};

// Each edge → 4 vertices (v0+, v0-, v1+, v1-)
const VERTS_PER_EDGE: usize = 4;
// Each edge → 2 triangles × 3 indices = 6 indices
const IDXS_PER_EDGE: usize = 6;
// Each triangle → 3 edges
const EDGES_PER_TRI: usize = 3;
// Each triangle → 12 vertices, 18 indices
const VERTS_PER_TRI: usize = VERTS_PER_EDGE * EDGES_PER_TRI;
const IDXS_PER_TRI: usize = IDXS_PER_EDGE * EDGES_PER_TRI;

/// Build a wireframe-index mesh from a triangle-list mesh by emitting each
/// triangle edge as a pair of line-list indices: (a,b), (b,c), (c,a).
///
/// The result is rendered with `line-list` topology; line width is 1 px
/// (WebGPU limitation).
pub fn create_wireframe_mesh(mesh: &Mesh) -> Mesh {
    let tri_count = mesh.indices.len() / 3;
    let mut edge_indices = Vec::with_capacity(tri_count * 6);
    for chunk in mesh.indices.chunks_exact(3) {
        let i0 = chunk[0];
        let i1 = chunk[1];
        let i2 = chunk[2];
        edge_indices.push(i0);
        edge_indices.push(i1);
        edge_indices.push(i1);
        edge_indices.push(i2);
        edge_indices.push(i2);
        edge_indices.push(i0);
    }
    Mesh::new(mesh.vertices.clone(), edge_indices)
}

/// Alternative wireframe generator that expands each triangle edge into a thin
/// quad (two triangles), giving visible thickness in world space.
///
/// Rendered with `triangle-list` topology.  The thickness is ~0.4 % of the
/// mesh's bounding-box extent.  Slower than the line-list counterpart and the
/// visual quality depends on the mesh topology.
pub fn create_wireframe_mesh_quads(mesh: &Mesh) -> Mesh {
    let tri_count = mesh.indices.len() / 3;
    let aabb = mesh.compute_aabb();
    let size = aabb.max - aabb.min;
    let max_extent = size.x().max(size.y()).max(size.z()).max(0.001);
    let half_thick = max_extent * 0.004; // 0.4 % of model size

    // Coarse chunking: enough triangles to amortise rayon dispatch.
    const CHUNK_TRIS: usize = 2048;
    let chunk_count = (tri_count + CHUNK_TRIS - 1) / CHUNK_TRIS;

    // Build per-chunk metadata: (start_tri, end_tri, absolute_vertex_offset).
    let mut cursor = 0usize;
    let chunks: Vec<(usize, usize, usize)> = (0..chunk_count)
        .map(|_| {
            let start = cursor;
            let end = (start + CHUNK_TRIS).min(tri_count);
            let vert_offset = start * VERTS_PER_TRI;
            cursor = end;
            (start, end, vert_offset)
        })
        .collect();

    // ---- Phase 1: parallel chunk processing ----
    // Each chunk produces a (vertices, indices) segment whose indices already
    // use absolute positions into the final vertex array.
    let mut segments: Vec<(Vec<Vertex>, Vec<u16>)> = chunks
        .par_iter()
        .map(|&(start, end, vert_offset)| {
            let count = end - start;
            let mut verts = Vec::with_capacity(count * VERTS_PER_TRI);
            let mut idxs = Vec::with_capacity(count * IDXS_PER_TRI);

            for t in start..end {
                let i0 = mesh.indices[t * 3] as usize;
                let i1 = mesh.indices[t * 3 + 1] as usize;
                let i2 = mesh.indices[t * 3 + 2] as usize;

                // Absolute vertex offset for this triangle's first vertex.
                let tri_vo = vert_offset + (t - start) * VERTS_PER_TRI;

                emit_edge_quad(
                    &mesh.vertices,
                    i0,
                    i1,
                    half_thick,
                    tri_vo,
                    &mut verts,
                    &mut idxs,
                );
                emit_edge_quad(
                    &mesh.vertices,
                    i1,
                    i2,
                    half_thick,
                    tri_vo + VERTS_PER_EDGE,
                    &mut verts,
                    &mut idxs,
                );
                emit_edge_quad(
                    &mesh.vertices,
                    i2,
                    i0,
                    half_thick,
                    tri_vo + VERTS_PER_EDGE * 2,
                    &mut verts,
                    &mut idxs,
                );
            }

            (verts, idxs)
        })
        .collect();

    // ---- Phase 2: sequential merge ----
    let total_verts: usize = segments.iter().map(|(v, _)| v.len()).sum();
    let total_idxs: usize = segments.iter().map(|(_, i)| i.len()).sum();

    let mut vertices = Vec::with_capacity(total_verts);
    let mut indices = Vec::with_capacity(total_idxs);

    for (mut v, mut i) in segments.drain(..) {
        // Indices are already absolute — just append.
        vertices.append(&mut v);
        indices.append(&mut i);
    }

    Mesh::new(vertices, indices)
}

/// Emit a quad (two triangles) for the edge `vertices[ia]`–`vertices[ib]`,
/// offset perpendicular to the edge by `half_thick` world units.
///
/// `base_vo` is the *absolute* vertex offset of this edge's first vertex in
/// the final mesh, so indices are emitted as absolute values.
#[inline(always)]
fn emit_edge_quad(
    vertices: &[Vertex],
    ia: usize,
    ib: usize,
    half_thick: f32,
    base_vo: usize,
    verts: &mut Vec<Vertex>,
    idxs: &mut Vec<u16>,
) {
    let va = &vertices[ia];
    let vb = &vertices[ib];
    let pa = va.position.xyz();
    let pb = vb.position.xyz();

    let edge_dir = math::normalize(pb - pa);

    // Pick a reference axis least parallel to the edge to avoid degeneracy
    // when the edge direction is nearly parallel to one axis.
    let ax = edge_dir.x().abs();
    let ay = edge_dir.y().abs();
    let az = edge_dir.z().abs();
    let ref_axis = if ax <= ay && ax <= az {
        float3::new(1.0, 0.0, 0.0)
    } else if ay <= az {
        float3::new(0.0, 1.0, 0.0)
    } else {
        float3::new(0.0, 0.0, 1.0)
    };

    let perp = math::normalize(math::cross(edge_dir, ref_axis));
    let offset = perp * half_thick;

    let base = base_vo as u16;

    //  v0+  ───── v1+
    //   |  \     |
    //   |    \   |
    //  v0-  ───── v1-

    // v0+
    let mut v = va.clone();
    v.position = (pa + offset).append(1.0);
    verts.push(v);

    // v0-
    let mut v = va.clone();
    v.position = (pa - offset).append(1.0);
    verts.push(v);

    // v1+
    let mut v = vb.clone();
    v.position = (pb + offset).append(1.0);
    verts.push(v);

    // v1-
    let mut v = vb.clone();
    v.position = (pb - offset).append(1.0);
    verts.push(v);

    // Triangle 1: (v0+, v1+, v0-)
    idxs.push(base);
    idxs.push(base + 2);
    idxs.push(base + 1);
    // Triangle 2: (v1-, v0-, v1+)
    idxs.push(base + 3);
    idxs.push(base + 1);
    idxs.push(base + 2);
}
