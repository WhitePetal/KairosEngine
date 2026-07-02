use std::{
    ops::Mul,
    simd::{f32x4, num::SimdFloat, simd_swizzle},
};

use crate::math::{float3, float4, float4x4};

mod converts;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct quaternion(pub float4);

impl quaternion {
    pub const IDENTITY: quaternion = quaternion::identity();

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

    /// Build a quaternion that rotates identity forward `(0,0,-1)` to `forward`,
    /// using `up_world` as the world-up reference (e.g. `(0,1,0)`).
    /// `forward` must be non-zero; `up_world` must not be parallel to `forward`.
    #[inline(always)]
    pub fn from_look(forward: float3, up_world: float3) -> Self {
        let f = super::normalize(forward);
        let mut r = super::cross(f, up_world);
        let r_len = super::length(&r);

        // Guard against forward ∥ up_world (gimbal-lock).
        // Fall back: pick a different world-up (e.g. X or Z).
        let (right, up) = if r_len < 1e-7 {
            let alt_up = if up_world.y().abs() > 0.999 {
                float3::new(1.0, 0.0, 0.0)
            } else {
                float3::new(0.0, 1.0, 0.0)
            };
            let right = super::normalize(super::cross(f, alt_up));
            let up = super::cross(right, f);
            (right, up)
        } else {
            r *= 1.0 / r_len; // normalize right in-place
            let up = super::cross(r, f);
            (r, up)
        };

        // Rotation matrix columns (column-major):
        //   c0 = right, c1 = up, c2 = -forward
        // (because local forward is (0,0,-1), so -forward = local +Z)
        let m00 = right.x();
        let m01 = up.x();
        let m02 = -f.x();
        let m10 = right.y();
        let m11 = up.y();
        let m12 = -f.y();
        let m20 = right.z();
        let m21 = up.z();
        let m22 = -f.z();

        // Matrix → quaternion (standard trace-based conversion)
        let trace = m00 + m11 + m22;

        if trace > 0.0 {
            let s = super::sqrt(trace + 1.0) * 2.0;
            Self(float4::new(
                (m21 - m12) / s,
                (m02 - m20) / s,
                (m10 - m01) / s,
                s * 0.25,
            ))
        } else if m00 > m11 && m00 > m22 {
            let s = super::sqrt(1.0 + m00 - m11 - m22) * 2.0;
            Self(float4::new(
                s * 0.25,
                (m01 + m10) / s,
                (m02 + m20) / s,
                (m21 - m12) / s,
            ))
        } else if m11 > m22 {
            let s = super::sqrt(1.0 + m11 - m00 - m22) * 2.0;
            Self(float4::new(
                (m01 + m10) / s,
                s * 0.25,
                (m12 + m21) / s,
                (m02 - m20) / s,
            ))
        } else {
            let s = super::sqrt(1.0 + m22 - m00 - m11) * 2.0;
            Self(float4::new(
                (m02 + m20) / s,
                (m12 + m21) / s,
                s * 0.25,
                (m10 - m01) / s,
            ))
        }
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
