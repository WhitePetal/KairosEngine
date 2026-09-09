//! GPU vertex layout.
//!
//! # Storage model (Option B — compute/storage decoupling, #147 / #148)
//!
//! `Vertex` is a strict [`bytemuck::Pod`] type: every byte in memory belongs
//! to one of the five attributes, with **no inter-field and no trailing
//! padding**, so uploading `bytemuck::cast_slice(&vertices)` into a GPU vertex
//! buffer can never read uninitialized padding bytes.
//!
//! The math side stays untouched and SIMD-friendly: `float3` keeps its
//! `glam::Vec3A` inner (16 B / align 16), and inside `kairos_math` the
//! `float2`/`float4` wrappers keep storing their glam types directly. Those
//! inner types cannot all live inside a padding-free `Vertex`: `glam::Vec3A`
//! and `Vec4` are 16-byte aligned, so any struct holding them is padded to a
//! multiple of 16 B. `float2` (glam `Vec2`, 8 B / align 4) is itself
//! padding-free and is stored directly; the other attributes use compact
//! no-pad representations here whose size matches exactly what the GPU reads
//! (`Float32x4`/`Float32x3`), converted to/from the math types at the
//! construction / asset boundaries.
//!
//! Per-vertex size: 68 B (previously 80 B — the `float3`(Vec3A) field padded
//! the vertex to 16-byte alignment).

use rkyv::Archive;
use serde::{Deserialize, Serialize};

use crate::math::{float2, float3, float4};

#[repr(C)]
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    bytemuck::Zeroable,
    bytemuck::Pod,
)]
pub struct Vertex {
    /// Position (x, y, z, w = 1.0), 16 B.
    pub position: [f32; 4],
    /// RGBA color, 16 B.
    pub color: [f32; 4],
    /// UV coordinates, 8 B. `float2` is itself padding-free (8 B, align 4),
    /// so it is stored directly.
    pub texcoord: float2,
    /// Normal (x, y, z), 12 B. See [`Vertex::pack_normal`] /
    /// [`Vertex::unpack_normal`] for the math-`float3` bridge.
    pub normal: [f32; 3],
    /// Tangent (xyz + handedness), 16 B.
    pub tangent: [f32; 4],
}

impl Vertex {
    /// Build a vertex at `position` with default color/uv/normal/tangent.
    pub fn with_position(position: float3) -> Self {
        Self {
            position: position.append(1.0).to_array(),
            color: float4::ONE.to_array(),
            texcoord: float2::ZERO,
            normal: [0.0; 3],
            tangent: float4::ZERO.to_array(),
        }
    }

    /// Build a vertex at `position` with an explicit color; the rest default.
    pub fn with_position_color(position: float3, color: float4) -> Self {
        Self {
            position: position.append(1.0).to_array(),
            color: color.to_array(),
            texcoord: float2::ZERO,
            normal: [0.0; 3],
            tangent: float4::new(0.0, 0.0, 0.0, 0.0).to_array(),
        }
    }

    /// Pack a math-side [`float3`] (glam `Vec3A`, 16 B / align 16) into the
    /// 12 B no-pad storage of [`Vertex::normal`].
    #[inline(always)]
    pub fn pack_normal(normal: float3) -> [f32; 3] {
        [normal.x(), normal.y(), normal.z()]
    }

    /// Unpack [`Vertex::normal`] storage back into a math-side [`float3`].
    #[inline(always)]
    pub fn unpack_normal(normal: [f32; 3]) -> float3 {
        float3::from(normal)
    }
}

#[cfg(test)]
mod test {
    use std::mem::{align_of, offset_of, size_of};

    use super::Vertex;
    use crate::math::{float3, float4};

    /// The vertex layout must be a strict, padding-free Pod. Rather than
    /// hard-coding magic offsets, each invariant is expressed through
    /// `offset_of!` and the field sizes, so the assertions hold iff no
    /// inter-field padding exists.
    #[test]
    fn layout_has_no_padding() {
        // Fields are laid out in declaration order (repr(C)) and every field
        // alignment is <= 4, so each offset is the sum of the preceding sizes.
        assert_eq!(offset_of!(Vertex, position), 0);
        assert_eq!(
            offset_of!(Vertex, color),
            offset_of!(Vertex, position) + size_of::<[f32; 4]>()
        );
        assert_eq!(
            offset_of!(Vertex, texcoord),
            offset_of!(Vertex, color) + size_of::<[f32; 4]>()
        );
        assert_eq!(
            offset_of!(Vertex, normal),
            offset_of!(Vertex, texcoord) + size_of::<crate::math::float2>()
        );
        assert_eq!(
            offset_of!(Vertex, tangent),
            offset_of!(Vertex, normal) + size_of::<[f32; 3]>()
        );
        // ... and the struct ends exactly after the last field (no trailing
        // padding). Total: 16 + 16 + 8 + 12 + 16 = 68 B per vertex.
        assert_eq!(size_of::<Vertex>(), 68);
        assert_eq!(align_of::<Vertex>(), 4);
    }

    /// Round-trip between the math `float3` and the compact `normal` storage.
    #[test]
    fn normal_storage_conversion_roundtrip() {
        let normal = float3::new(0.3, -0.7, 0.9);
        assert_eq!(Vertex::unpack_normal(Vertex::pack_normal(normal)), normal);
        assert_eq!(Vertex::pack_normal(normal), [0.3, -0.7, 0.9]);
    }

    /// The builders go through the compact storage and keep defaults sane.
    #[test]
    fn builders_write_compact_storage() {
        let v = Vertex::with_position(float3::new(1.0, 2.0, 3.0));
        assert_eq!(v.position, [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(v.normal, [0.0; 3]);
        assert_eq!(v.tangent, [0.0; 4]);
        assert_eq!(v.color, [1.0; 4]);

        let v = Vertex::with_position_color(
            float3::new(1.0, 2.0, 3.0),
            float4::new(0.1, 0.2, 0.3, 0.4),
        );
        assert_eq!(v.position, [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(v.color, [0.1, 0.2, 0.3, 0.4]);
    }
}
