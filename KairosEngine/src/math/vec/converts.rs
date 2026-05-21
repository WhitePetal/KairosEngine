use std::simd::f32x4;

use super::super::Color32;
use super::float4;

impl From<Color32> for float4 {
    fn from(c: Color32) -> Self {
        let cv = f32x4::from_array([c.r as f32, c.g as f32, c.b as f32, c.a as f32]);
        Self(cv / f32x4::splat(255.0))
    }
}
