mod color;
mod vec; 

pub use color::Color32;
pub use vec::*;

pub fn float2(x: f32, y: f32) -> float2 {
    float2::from_array([x, y])
}
pub fn float3(x: f32, y: f32, z: f32) -> float3 {
    float3::from_array_4([x, y, z, 0.0])
}
pub fn float4(x: f32, y: f32, z: f32, w: f32) -> float4 {
    float4::from_array([x, y, z, w])
}