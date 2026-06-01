use crate::math::{float2, float3, float4};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub position: float4,
    pub color: float4,
    pub texcoord: float2,
    pub normal: float3,
    pub tangent: float4,
}

unsafe impl bytemuck::Zeroable for Vertex {}
unsafe impl bytemuck::Pod for Vertex {}
