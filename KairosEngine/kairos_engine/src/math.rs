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
use rapier3d::glamx::FloatExt;
pub use trigonometric::*;
pub use vec::*;

pub trait Min {
    fn min(self, other: Self) -> Self;
}
pub trait Max {
    fn max(self, other: Self) -> Self;
}
pub trait Sqrt {
    fn sqrt(self) -> Self;
}
pub trait LerpFactor<T> {
    fn get_factor(self) -> T;
}
pub trait Lerp {
    fn lerp(left: Self, right: Self, factor: impl LerpFactor<Self>) -> Self
    where
        Self: Sized;
}

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
pub fn sin<T: Sin>(value: T) -> T {
    value.sin()
}
#[inline(always)]
pub fn sqrt<T: Sqrt>(value: T) -> T {
    value.sqrt()
}
#[inline(always)]
pub fn min<T: Min>(a: T, b: T) -> T {
    a.min(b)
}
#[inline(always)]
pub fn max<T: Max>(a: T, b: T) -> T {
    a.max(b)
}
#[inline(always)]
pub fn lerp<T: Lerp>(left: T, right: T, factor: impl LerpFactor<T>) -> T {
    T::lerp(left, right, factor)
}

impl Min for f32 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        self.min(other)
    }
}
impl Max for f32 {
    #[inline(always)]
    fn max(self, other: Self) -> Self {
        self.max(other)
    }
}
impl Sqrt for f32 {
    #[inline(always)]
    fn sqrt(self) -> Self {
        self.sqrt()
    }
}
impl LerpFactor<f32> for f32 {
    #[inline(always)]
    fn get_factor(self) -> f32 {
        self
    }
}
impl Lerp for f32 {
    #[inline(always)]
    fn lerp(left: Self, right: Self, factor: impl LerpFactor<f32>) -> Self {
        left.lerp(right, factor.get_factor())
    }
}
