mod color;
mod consts;
mod matrix;
mod quaternions;
mod trigonometric;
mod vec;

pub use color::Color32;
pub use consts::*;
pub use matrix::*;
pub use quaternions::*;
pub use trigonometric::*;
pub use vec::*;

#[inline(always)]
pub fn float2(x: f32, y: f32) -> float2 {
    float2::from_array([x, y])
}
#[inline(always)]
pub fn float3(x: f32, y: f32, z: f32) -> float3 {
    float3::from_array_4([x, y, z, 0.0])
}
#[inline(always)]
pub fn float4(x: f32, y: f32, z: f32, w: f32) -> float4 {
    float4::from([x, y, z, w])
}
#[inline(always)]
pub fn sin(value: f32) -> f32 {
    value.sin()
}
