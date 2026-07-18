use std::ops::Mul;

use glam::Vec4Swizzles;

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

        let sv = glam::Vec4::new(sx, cx, cx, cx);
        let cv = glam::Vec4::new(cx, sx, sx, sx);

        let a = sv * glam::Vec4::new(cy, sy, cy, cy) * glam::Vec4::new(cz, cz, sz, cz);
        let b = cv * glam::Vec4::new(sy, cy, sy, sy) * glam::Vec4::new(sz, sz, cz, sz);

        Self(float4::from_inner(
            a + b * glam::Vec4::new(-1.0, 1.0, -1.0, 1.0),
        ))
        .normalized()
    }

    #[inline(always)]
    pub fn to_euler(self) -> float3 {
        let q = self.normalized().0.0;
        let q2 = q * q;
        let xy_yz_zx = q * q.yzxw();
        let wx_wy_wz = q.wwwx() * q.xyzy();

        let sin_x: f32 = 2.0 * (wx_wy_wz.x + xy_yz_zx.y);
        let cos_x: f32 = 1.0 - 2.0 * (q2.x + q2.y);
        let sin_y: f32 = 2.0 * (wx_wy_wz.y - xy_yz_zx.z);
        let sin_z: f32 = 2.0 * (wx_wy_wz.z + xy_yz_zx.x);
        let cos_z: f32 = 1.0 - 2.0 * (q2.y + q2.z);

        float3::new(
            sin_x.atan2(cos_x),
            sin_y.clamp(-1.0, 1.0).asin(),
            sin_z.atan2(cos_z),
        )
    }

    #[inline(always)]
    pub fn normalized(self) -> Self {
        let len_sq = self.0.0.length_squared();
        Self(float4::from_inner(self.0.0 / len_sq.sqrt()))
    }

    #[inline(always)]
    pub fn normalize(&self) -> Self {
        (*self).normalized()
    }

    #[inline(always)]
    pub fn from_look(forward: float3, up_world: float3) -> Self {
        let f = super::normalize(forward);
        let mut r = super::cross(f, up_world);
        let r_len = super::length(&r);

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
            r *= 1.0 / r_len;
            let up = super::cross(r, f);
            (r, up)
        };

        let m00 = right.x();
        let m01 = up.x();
        let m02 = -f.x();
        let m10 = right.y();
        let m11 = up.y();
        let m12 = -f.y();
        let m20 = right.z();
        let m21 = up.z();
        let m22 = -f.z();

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
        let xy_yz_zx = q * q.yzxw();
        let xw_yw_zw = q * q.wwww();

        let c0 = float4::from_inner(glam::Vec4::new(
            1.0 - 2.0 * (q2.y + q2.z),
            2.0 * (xy_yz_zx.x + xw_yw_zw.z),
            2.0 * (xy_yz_zx.z - xw_yw_zw.y),
            0.0,
        ));
        let c1 = float4::from_inner(glam::Vec4::new(
            2.0 * (xy_yz_zx.x - xw_yw_zw.z),
            1.0 - 2.0 * (q2.x + q2.z),
            2.0 * (xy_yz_zx.y + xw_yw_zw.x),
            0.0,
        ));
        let c2 = float4::from_inner(glam::Vec4::new(
            2.0 * (xy_yz_zx.z + xw_yw_zw.y),
            2.0 * (xy_yz_zx.y - xw_yw_zw.x),
            1.0 - 2.0 * (q2.x + q2.y),
            0.0,
        ));

        float4x4::new(c0, c1, c2, float4::new(0.0, 0.0, 0.0, 1.0))
    }
}

impl Mul for quaternion {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        let mask = glam::Vec4::new(1.0, 1.0, 1.0, 0.0);
        let av = self.0.0 * mask;
        let bv = rhs.0.0 * mask;
        let aw = glam::Vec4::splat(self.0.w());
        let bw = glam::Vec4::splat(rhs.0.w());

        let cross = av.yzxw() * bv.zxyw() - av.zxyw() * bv.yzxw();
        let xyz = (aw * bv + bw * av + cross) * mask;
        let w = self.0.w() * rhs.0.w() - av.dot(bv);

        Self(float4::from_inner(xyz + glam::Vec4::new(0.0, 0.0, 0.0, w)))
    }
}

unsafe impl bytemuck::Zeroable for quaternion {}
unsafe impl bytemuck::Pod for quaternion {}
