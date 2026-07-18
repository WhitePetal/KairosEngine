use super::super::Color32;
use super::{float3, float4};

impl From<Color32> for float4 {
    fn from(c: Color32) -> Self {
        float4::from_inner(glam::Vec4::new(
            c.r as f32 / 255.0,
            c.g as f32 / 255.0,
            c.b as f32 / 255.0,
            c.a as f32 / 255.0,
        ))
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

impl From<float3> for rapier3d::math::Vector3 {
    fn from(value: float3) -> Self {
        rapier3d::math::Vector3::new(value.x(), value.y(), value.z())
    }
}

impl From<rapier3d::math::Vector3> for float3 {
    fn from(value: rapier3d::math::Vector3) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}
