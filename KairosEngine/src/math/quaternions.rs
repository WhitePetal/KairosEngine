use std::{
    ops::Mul,
    simd::{f32x4, num::SimdFloat, simd_swizzle},
};

use crate::math::{float3, float4, float4x4};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct quaternion(pub float4);

impl quaternion {
    #[inline(always)]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self(float4::new(x, y, z, w))
    }

    #[inline(always)]
    pub const fn identity() -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0)
    }

    #[inline(always)]
    pub fn from_euler(euler: float3) -> Self {
        let (sx, cx) = (euler.x() * 0.5).sin_cos();
        let (sy, cy) = (euler.y() * 0.5).sin_cos();
        let (sz, cz) = (euler.z() * 0.5).sin_cos();

        let a = f32x4::from_array([sx, cx, cx, cx])
            * f32x4::from_array([cy, sy, cy, cy])
            * f32x4::from_array([cz, cz, sz, cz]);
        let b = f32x4::from_array([cx, sx, sx, sx])
            * f32x4::from_array([sy, cy, sy, sy])
            * f32x4::from_array([sz, sz, cz, sz]);

        Self(float4::from_simd(
            a + b * f32x4::from_array([-1.0, 1.0, -1.0, 1.0]),
        ))
        .normalized()
    }

    #[inline(always)]
    pub fn to_euler(self) -> float3 {
        let q = self.normalized().0.0;
        let q2 = q * q;
        let xy_yz_zx = q * simd_swizzle!(q, [1, 2, 0, 3]);
        let wx_wy_wz = simd_swizzle!(q, [3, 3, 3, 0]) * simd_swizzle!(q, [0, 1, 2, 1]);

        let sin_x = 2.0 * (wx_wy_wz[0] + xy_yz_zx[1]);
        let cos_x = 1.0 - 2.0 * (q2[0] + q2[1]);
        let sin_y = 2.0 * (wx_wy_wz[1] - xy_yz_zx[2]);
        let sin_z = 2.0 * (wx_wy_wz[2] + xy_yz_zx[0]);
        let cos_z = 1.0 - 2.0 * (q2[1] + q2[2]);

        float3::new(
            sin_x.atan2(cos_x),
            sin_y.clamp(-1.0, 1.0).asin(),
            sin_z.atan2(cos_z),
        )
    }

    #[inline(always)]
    pub fn normalized(self) -> Self {
        let len_sq = (self.0.0 * self.0.0).reduce_sum();

        Self(float4::from_simd(
            self.0.0 * f32x4::splat(len_sq.sqrt().recip()),
        ))
    }

    #[inline(always)]
    pub fn normalize(&self) -> Self {
        (*self).normalized()
    }

    #[inline(always)]
    pub fn to_float4x4(self) -> float4x4 {
        let q = self.normalized().0.0;
        let q2 = q * q;
        let xy_yz_zx = q * simd_swizzle!(q, [1, 2, 0, 3]);
        let xw_yw_zw = q * simd_swizzle!(q, [3, 3, 3, 3]);

        let c0 = float4::from_simd(f32x4::from_array([
            1.0 - 2.0 * (q2[1] + q2[2]),
            2.0 * (xy_yz_zx[0] + xw_yw_zw[2]),
            2.0 * (xy_yz_zx[2] - xw_yw_zw[1]),
            0.0,
        ]));
        let c1 = float4::from_simd(f32x4::from_array([
            2.0 * (xy_yz_zx[0] - xw_yw_zw[2]),
            1.0 - 2.0 * (q2[0] + q2[2]),
            2.0 * (xy_yz_zx[1] + xw_yw_zw[0]),
            0.0,
        ]));
        let c2 = float4::from_simd(f32x4::from_array([
            2.0 * (xy_yz_zx[2] + xw_yw_zw[1]),
            2.0 * (xy_yz_zx[1] - xw_yw_zw[0]),
            1.0 - 2.0 * (q2[0] + q2[1]),
            0.0,
        ]));

        float4x4::new(c0, c1, c2, float4::new(0.0, 0.0, 0.0, 1.0))
    }
}

impl Mul for quaternion {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        let mask = f32x4::from_array([1.0, 1.0, 1.0, 0.0]);
        let av = self.0.0 * mask;
        let bv = rhs.0.0 * mask;
        let aw = f32x4::splat(self.0.w());
        let bw = f32x4::splat(rhs.0.w());

        let cross = simd_swizzle!(av, [1, 2, 0, 3]) * simd_swizzle!(bv, [2, 0, 1, 3])
            - simd_swizzle!(av, [2, 0, 1, 3]) * simd_swizzle!(bv, [1, 2, 0, 3]);
        let xyz = (aw * bv + bw * av + cross) * mask;
        let w = self.0.w() * rhs.0.w() - (av * bv).reduce_sum();

        Self(float4::from_simd(
            xyz + f32x4::from_array([0.0, 0.0, 0.0, w]),
        ))
    }
}

unsafe impl bytemuck::Zeroable for quaternion {}
unsafe impl bytemuck::Pod for quaternion {}
