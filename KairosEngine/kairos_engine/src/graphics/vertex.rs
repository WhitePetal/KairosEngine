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
)]
pub struct Vertex {
    pub position: float4,
    pub color: float4,
    pub texcoord: float2,
    pub normal: float3,
    pub tangent: float4,
}

unsafe impl bytemuck::Zeroable for Vertex {}
unsafe impl bytemuck::Pod for Vertex {}

impl Vertex {
    pub fn with_position(position: float3) -> Self {
        Self {
            position: position.append(1.0),
            color: float4::ONE,
            texcoord: float2::ZERO,
            normal: float3::ZERO,
            tangent: float4::ZERO,
        }
    }

    pub fn with_position_color(position: float3, color: float4) -> Self {
        Self {
            position: position.append(1.0),
            color,
            texcoord: crate::math::float2::ZERO,
            normal: float3::ZERO,
            tangent: float4::new(0.0, 0.0, 0.0, 0.0),
        }
    }
}
