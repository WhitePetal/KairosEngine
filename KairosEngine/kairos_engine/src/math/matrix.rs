use std::ops::Mul;

use crate::math::{float3, float4};

use super::quaternions::quaternion;

#[cfg(test)]
mod test;

///
/// column-major 4x4 matrix, backed by glam::Mat4
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct float4x4(pub(crate) glam::Mat4);

impl float4x4 {
    pub const IDENTITY: float4x4 = float4x4(glam::Mat4::IDENTITY);

    #[inline(always)]
    pub fn new(v1: float4, v2: float4, v3: float4, v4: float4) -> Self {
        Self(glam::Mat4::from_cols(v1.0, v2.0, v3.0, v4.0))
    }

    #[inline(always)]
    pub fn trs(position: float3, rotation: quaternion, scale: float3) -> Self {
        let r = rotation.to_float4x4();
        Self(glam::Mat4::from_cols(
            r.c0().0 * scale.x(),
            r.c1().0 * scale.y(),
            r.c2().0 * scale.z(),
            float4::new(position.x(), position.y(), position.z(), 1.0).0,
        ))
    }

    #[inline(always)]
    pub fn to_array(&self) -> [[f32; 4]; 4] {
        self.0.to_cols_array_2d()
    }

    #[inline(always)]
    pub fn c0(&self) -> float4 {
        float4::from_inner(self.0.x_axis)
    }
    #[inline(always)]
    pub fn c1(&self) -> float4 {
        float4::from_inner(self.0.y_axis)
    }
    #[inline(always)]
    pub fn c2(&self) -> float4 {
        float4::from_inner(self.0.z_axis)
    }
    #[inline(always)]
    pub fn c3(&self) -> float4 {
        float4::from_inner(self.0.w_axis)
    }
}

impl Mul for float4x4 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl Mul<float4> for float4x4 {
    type Output = float4;

    #[inline(always)]
    fn mul(self, rhs: float4) -> Self::Output {
        float4::from_inner(self.0 * rhs.0)
    }
}

unsafe impl bytemuck::Zeroable for float4x4 {}
unsafe impl bytemuck::Pod for float4x4 {}
