use crate::{float3, float4, float4x4, quaternion};

// The parent module is `#[path]`-backed (`affine_impl`), so the test
// submodule resolves relative to `src/`; point it back at `affine/test.rs`.
#[cfg(test)]
#[path = "affine/test.rs"]
mod test;

///
/// 3D affine transform (rotation + scale + translation, no perspective),
/// backed by glam::Affine3A.
///
/// This is the storage type for the future `GlobalTransform` ECS component
/// (see wayfinder #141/#145).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct affine(pub(crate) glam::Affine3A);

impl affine {
    pub const IDENTITY: affine = affine(glam::Affine3A::IDENTITY);

    #[inline(always)]
    pub fn trs(position: float3, rotation: quaternion, scale: float3) -> Self {
        Self(glam::Affine3A::from_scale_rotation_translation(
            glam::Vec3::from(scale.0),
            glam::Quat::from_vec4(rotation.0.0),
            glam::Vec3::from(position.0),
        ))
    }

    #[inline(always)]
    pub fn to_float4x4(&self) -> float4x4 {
        float4x4(glam::Mat4::from(self.0))
    }

    #[inline(always)]
    pub fn inverse(&self) -> Self {
        Self(self.0.inverse())
    }

    #[inline(always)]
    pub fn transform_point(&self, point: float3) -> float3 {
        float3::from_array(self.0.transform_point3a(point.0).to_array())
    }

    #[inline(always)]
    pub fn translation(&self) -> float3 {
        float3::from_array(self.0.translation.to_array())
    }

    #[inline(always)]
    pub fn to_scale_rotation_translation(&self) -> (float3, quaternion, float3) {
        let (scale, rotation, translation) = self.0.to_scale_rotation_translation();
        (
            float3::from_array(scale.to_array()),
            quaternion(float4::from_inner(glam::Vec4::from(rotation))),
            float3::from_array(translation.to_array()),
        )
    }
}

impl Default for affine {
    #[inline(always)]
    fn default() -> Self {
        Self::IDENTITY
    }
}

// `glam::Affine3A` contains SIMD padding inside its `Vec3A` columns, so glam
// itself only derives `AnyBitPattern` for it (not `Pod`); mirror that here —
// all-zero bytes are a valid transform, but a strict `Pod` impl would be
// unsound for a padded layout.
unsafe impl bytemuck::Zeroable for affine {}
