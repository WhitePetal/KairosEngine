use std::ops::Mul;

use glam::{Affine3A, Mat4};

use crate::{float3, float4x4, quaternion};

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
pub struct affine(pub(crate) Affine3A);

impl affine {
    pub const IDENTITY: affine = affine(glam::Affine3A::IDENTITY);

    #[inline(always)]
    pub fn trs(position: float3, rotation: quaternion, scale: float3) -> Self {
        Self(glam::Affine3A::from_scale_rotation_translation(
            glam::Vec3::from(scale.0),
            rotation.0,
            glam::Vec3::from(position.0),
        ))
    }

    #[inline(always)]
    pub fn to_float4x4(&self) -> float4x4 {
        float4x4(Mat4::from(self.0))
    }

    #[inline(always)]
    pub fn inverse(&self) -> Self {
        Self(self.0.inverse())
    }

    #[inline(always)]
    pub fn transform_point(&self, point: float3) -> float3 {
        float3::from_array(self.0.transform_point3a(point.0).to_array())
    }

    /// Transforms a direction vector: applies rotation/scale/shear but **not**
    /// translation. Use this (not `Mul`) for normals, tangents, velocities, etc.
    #[inline(always)]
    pub fn transform_vector(&self, vector: float3) -> float3 {
        float3::from_inner(self.0.transform_vector3a(vector.0))
    }

    #[inline(always)]
    pub fn translation(&self) -> float3 {
        float3::from_inner(self.0.translation)
    }

    #[inline(always)]
    pub fn to_scale_rotation_translation(&self) -> (float3, quaternion, float3) {
        let (scale, rotation, translation) = self.0.to_scale_rotation_translation();
        (
            float3::from_inner(scale.to_vec3a()),
            quaternion(rotation),
            float3::from_inner(translation.to_vec3a()),
        )
    }

    #[inline(always)]
    pub fn from_translation(translation: float3) -> Self {
        Self(Affine3A::from_translation(translation.0.to_vec3()))
    }

    #[inline(always)]
    pub fn from_rotation_translation(rotation: quaternion, translation: float3) -> Self {
        Self(Affine3A::from_rotation_translation(
            rotation.0,
            translation.0.to_vec3(),
        ))
    }

    #[inline(always)]
    pub fn from_scale_rotation_translation(scale: float3, rotation: quaternion, translation: float3) -> Self {
        Self(Affine3A::from_scale_rotation_translation(scale.0.to_vec3(), rotation.0, translation.0.to_vec3()))
    }

    #[inline(always)]
    pub fn from_scale(scale: float3) -> Self {
        Self(Affine3A::from_scale(scale.0.to_vec3()))
    }

    /// Returns the determinant of `self`.
    #[inline(always)]
    pub fn determinant(&self) -> f32 {
        self.0.matrix3.determinant()
    }

    #[inline(always)]
    pub fn x_axis(&self) -> float3 {
        float3::from_inner(self.0.matrix3.x_axis)
    }

    #[inline(always)]
    pub fn y_axis(&self) -> float3 {
        float3::from_inner(self.0.matrix3.y_axis)
    }

    #[inline(always)]
    pub fn z_axis(&self) -> float3 {
        float3::from_inner(self.0.matrix3.z_axis)
    }

    #[inline(always)]
    pub fn x_axis_length(&self) -> f32 {
        self.0.matrix3.x_axis.length()
    }

    #[inline(always)]
    pub fn y_axis_length(&self) -> f32 {
        self.0.matrix3.y_axis.length()
    }

    #[inline(always)]
    pub fn z_axis_length(&self) -> f32 {
        self.0.matrix3.z_axis.length()
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


impl Mul for affine {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl Mul<float3> for affine {
    type Output = float3;

    #[inline(always)]
    fn mul(self, rhs: float3) -> Self::Output {
        // glam's `Affine3A` has no `Mul<Vec3A>`/`Mul<Vec3>` impl, so we call the
        // transform explicitly. `*` follows the matrix*column-vector convention
        // (cf. `Mul<float4> for float4x4`): the operand is treated as a *point*,
        // so translation is applied. For a direction use `transform_vector`.
        float3::from_inner(self.0.transform_point3a(rhs.0))
    }
}
