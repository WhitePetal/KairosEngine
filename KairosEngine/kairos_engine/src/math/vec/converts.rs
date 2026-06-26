use std::simd::f32x4;

use super::super::Color32;
use super::{float3, float4};

impl From<Color32> for float4 {
    fn from(c: Color32) -> Self {
        let cv = f32x4::from_array([c.r as f32, c.g as f32, c.b as f32, c.a as f32]);
        Self(cv / f32x4::splat(255.0))
    }
}

impl From<float3> for mint::Vector3<f32> {
    fn from(v: float3) -> Self {
        mint::Vector3 {
            x: v.x(),
            y: v.y(),
            z: v.z(),
        }
    }
}
