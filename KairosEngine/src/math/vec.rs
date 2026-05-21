#[cfg(test)]
mod tests;

mod converts;

use std::{
    ops::{Add, AddAssign, Div, DivAssign, Index, Mul, MulAssign, Sub, SubAssign},
    simd::{f32x2, f32x4, num::SimdFloat, simd_swizzle},
};

pub trait Vector
where
    Self: Add<f32, Output = Self>
        + Sub<f32, Output = Self>
        + Mul<f32, Output = Self>
        + Div<f32, Output = Self>
        + Add
        + Sub
        + Mul
        + Div
        + AddAssign<f32>
        + SubAssign<f32>
        + MulAssign<f32>
        + DivAssign<f32>
        + Index<usize>
        + Clone
        + Copy
        + PartialEq,
{
    fn dot(&self, r: &Self) -> f32;

    type CrossOutput;
    fn cross(&self, r: &Self) -> Self::CrossOutput;

    #[inline(always)]
    fn len_sq(&self) -> f32 {
        self.dot(self)
    }

    #[inline(always)]
    fn len(&self) -> f32 {
        self.len_sq().sqrt()
    }

    #[inline(always)]
    fn normalize(&self) -> Self {
        *self / self.len()
    }

    #[inline(always)]
    fn normalize_mut(&mut self) {
        *self /= self.len();
    }
}

#[inline(always)]
pub fn dot<T>(l: &T, r: &T) -> f32
where
    T: Vector,
{
    l.dot(r)
}

#[inline(always)]
pub fn cross<T>(l: &T, r: &T) -> T::CrossOutput
where
    T: Vector,
{
    l.cross(r)
}

#[inline(always)]
pub fn length_sq<T>(v: &T) -> f32
where
    T: Vector,
{
    v.len_sq()
}

#[inline(always)]
pub fn length<T>(v: &T) -> f32
where
    T: Vector,
{
    v.len()
}

#[inline(always)]
pub fn normalize<T>(v: &T) -> T
where
    T: Vector,
{
    v.normalize()
}

#[inline(always)]
pub fn normalize_mut<T>(v: &mut T)
where
    T: Vector,
{
    v.normalize_mut();
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct float2(pub f32x2);

impl float2 {
    #[inline(always)]
    pub fn new(x: f32, y: f32) -> Self {
        Self(f32x2::from_array([x, y]))
    }
    #[inline(always)]
    pub fn from_array(arr: [f32; 2]) -> Self {
        Self(f32x2::from_array(arr))
    }
}

impl Vector for float2 {
    #[inline(always)]
    fn dot(&self, other: &Self) -> f32 {
        (self.0 * other.0).reduce_sum()
    }

    type CrossOutput = f32;
    #[inline(always)]
    fn cross(&self, other: &Self) -> Self::CrossOutput {
        self.0[0] * other.0[1] - self.0[1] * other.0[0]
    }
}

impl Index<usize> for float2 {
    type Output = f32;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        self.0.index(index)
    }
}

impl Add<f32> for float2 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: f32) -> Self::Output {
        Self(self.0 + f32x2::splat(rhs))
    }
}
impl Add<f32> for &float2 {
    type Output = float2;

    #[inline(always)]
    fn add(self, rhs: f32) -> Self::Output {
        *self + rhs
    }
}
impl Add<float2> for f32 {
    type Output = float2;

    #[inline(always)]
    fn add(self, rhs: float2) -> Self::Output {
        float2(f32x2::splat(self) + rhs.0)
    }
}
impl Add<&float2> for f32 {
    type Output = float2;

    #[inline(always)]
    fn add(self, rhs: &float2) -> Self::Output {
        self + *rhs
    }
}
impl AddAssign<f32> for float2 {
    #[inline(always)]
    fn add_assign(&mut self, rhs: f32) {
        *self = *self + rhs;
    }
}

impl Sub<f32> for float2 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: f32) -> Self::Output {
        Self(self.0 - f32x2::splat(rhs))
    }
}
impl Sub<f32> for &float2 {
    type Output = float2;

    #[inline(always)]
    fn sub(self, rhs: f32) -> Self::Output {
        *self - rhs
    }
}
impl Sub<float2> for f32 {
    type Output = float2;

    #[inline(always)]
    fn sub(self, rhs: float2) -> Self::Output {
        float2(f32x2::splat(self) - rhs.0)
    }
}
impl Sub<&float2> for f32 {
    type Output = float2;

    fn sub(self, rhs: &float2) -> Self::Output {
        self - *rhs
    }
}
impl SubAssign<f32> for float2 {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: f32) {
        *self = *self + rhs;
    }
}

impl Mul<f32> for float2 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * f32x2::splat(rhs))
    }
}
impl Mul<f32> for &float2 {
    type Output = float2;

    #[inline(always)]
    fn mul(self, rhs: f32) -> Self::Output {
        *self * rhs
    }
}
impl Mul<float2> for f32 {
    type Output = float2;

    #[inline(always)]
    fn mul(self, rhs: float2) -> Self::Output {
        float2(f32x2::splat(self) * rhs.0)
    }
}
impl Mul<&float2> for f32 {
    type Output = float2;

    #[inline(always)]
    fn mul(self, rhs: &float2) -> Self::Output {
        self * *rhs
    }
}
impl MulAssign<f32> for float2 {
    #[inline(always)]
    fn mul_assign(&mut self, rhs: f32) {
        *self = *self + rhs;
    }
}

impl Div<f32> for float2 {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: f32) -> Self::Output {
        Self(self.0 / f32x2::splat(rhs))
    }
}
impl Div<f32> for &float2 {
    type Output = float2;

    #[inline(always)]
    fn div(self, rhs: f32) -> Self::Output {
        *self / rhs
    }
}
impl Div<float2> for f32 {
    type Output = float2;

    #[inline(always)]
    fn div(self, rhs: float2) -> Self::Output {
        float2(f32x2::splat(self) / rhs.0)
    }
}
impl Div<&float2> for f32 {
    type Output = float2;

    fn div(self, rhs: &float2) -> Self::Output {
        self / *rhs
    }
}
impl DivAssign<f32> for float2 {
    #[inline(always)]
    fn div_assign(&mut self, rhs: f32) {
        *self = *self / rhs;
    }
}

impl Add for float2 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}
impl Add for &float2 {
    type Output = float2;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        *self + *rhs
    }
}

impl Sub for float2 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}
impl Sub for &float2 {
    type Output = float2;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        *self - *rhs
    }
}

impl Mul for float2 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}
impl Mul for &float2 {
    type Output = float2;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        *self * *rhs
    }
}

impl Div for float2 {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}
impl Div for &float2 {
    type Output = float2;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self::Output {
        *self / *rhs
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct float3(pub f32x4);

impl float3 {
    #[inline(always)]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(f32x4::from_array([x, y, z, 0.0]))
    }
    #[inline(always)]
    pub fn from_array(arr: [f32; 3]) -> Self {
        Self(f32x4::from_array([arr[0], arr[1], arr[2], 0.0]))
    }
    #[inline(always)]
    pub fn from_array_4(arr: [f32; 4]) -> Self {
        Self(f32x4::from_array(arr))
    }
}

impl Vector for float3 {
    #[inline(always)]
    fn dot(&self, other: &Self) -> f32 {
        (self.0 * other.0).reduce_sum()
    }

    type CrossOutput = float3;
    #[inline(always)]
    fn cross(&self, other: &Self) -> Self::CrossOutput {
        let a = self.0;
        let b = other.0;

        let a_yzx = simd_swizzle!(a, [1, 2, 0, 3]);
        let a_zxy = simd_swizzle!(a, [2, 0, 1, 3]);
        let b_zxy = simd_swizzle!(b, [2, 0, 1, 3]);
        let b_yzx = simd_swizzle!(b, [1, 2, 0, 3]);

        Self((a_yzx * b_zxy - a_zxy * b_yzx) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }
}

impl Index<usize> for float3 {
    type Output = f32;

    fn index(&self, index: usize) -> &Self::Output {
        self.0.index(index)
    }
}

impl Add<f32> for float3 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: f32) -> Self::Output {
        Self(self.0 + f32x4::splat(rhs))
    }
}
impl Add<f32> for &float3 {
    type Output = float3;

    #[inline(always)]
    fn add(self, rhs: f32) -> Self::Output {
        *self + rhs
    }
}
impl Add<float3> for f32 {
    type Output = float3;

    #[inline(always)]
    fn add(self, rhs: float3) -> Self::Output {
        float3(f32x4::splat(self) + rhs.0)
    }
}
impl Add<&float3> for f32 {
    type Output = float3;

    #[inline(always)]
    fn add(self, rhs: &float3) -> Self::Output {
        self + *rhs
    }
}
impl AddAssign<f32> for float3 {
    #[inline(always)]
    fn add_assign(&mut self, rhs: f32) {
        *self = *self + rhs;
    }
}

impl Sub<f32> for float3 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: f32) -> Self::Output {
        Self(self.0 - f32x4::splat(rhs))
    }
}
impl Sub<f32> for &float3 {
    type Output = float3;

    #[inline(always)]
    fn sub(self, rhs: f32) -> Self::Output {
        *self - rhs
    }
}
impl Sub<float3> for f32 {
    type Output = float3;

    #[inline(always)]
    fn sub(self, rhs: float3) -> Self::Output {
        float3(f32x4::splat(self) - rhs.0)
    }
}
impl Sub<&float3> for f32 {
    type Output = float3;

    #[inline(always)]
    fn sub(self, rhs: &float3) -> Self::Output {
        self - *rhs
    }
}
impl SubAssign<f32> for float3 {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: f32) {
        *self = *self - rhs
    }
}

impl Mul<f32> for float3 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, scalar: f32) -> Self::Output {
        Self(self.0 * f32x4::splat(scalar))
    }
}
impl Mul<f32> for &float3 {
    type Output = float3;

    #[inline(always)]
    fn mul(self, rhs: f32) -> Self::Output {
        *self * rhs
    }
}
impl Mul<float3> for f32 {
    type Output = float3;

    #[inline(always)]
    fn mul(self, rhs: float3) -> Self::Output {
        float3(f32x4::splat(self) * rhs.0)
    }
}
impl Mul<&float3> for f32 {
    type Output = float3;

    #[inline(always)]
    fn mul(self, rhs: &float3) -> Self::Output {
        self * *rhs
    }
}
impl MulAssign<f32> for float3 {
    #[inline(always)]
    fn mul_assign(&mut self, rhs: f32) {
        *self = *self * rhs;
    }
}

impl Div<f32> for float3 {
    type Output = Self;

    #[inline(always)]
    fn div(self, scalar: f32) -> Self::Output {
        Self(self.0 / f32x4::splat(scalar))
    }
}
impl Div<f32> for &float3 {
    type Output = float3;

    #[inline(always)]
    fn div(self, rhs: f32) -> Self::Output {
        *self / rhs
    }
}
impl Div<float3> for f32 {
    type Output = float3;

    #[inline(always)]
    fn div(self, rhs: float3) -> Self::Output {
        float3(f32x4::splat(self) / rhs.0)
    }
}
impl Div<&float3> for f32 {
    type Output = float3;

    #[inline(always)]
    fn div(self, rhs: &float3) -> Self::Output {
        self / *rhs
    }
}
impl DivAssign<f32> for float3 {
    #[inline(always)]
    fn div_assign(&mut self, rhs: f32) {
        *self = *self / rhs;
    }
}

impl Add for float3 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}
impl Add for &float3 {
    type Output = float3;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        *self + *rhs
    }
}

impl Sub for float3 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}
impl Sub for &float3 {
    type Output = float3;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        *self - *rhs
    }
}

impl Mul for float3 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}
impl Mul for &float3 {
    type Output = float3;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        *self * *rhs
    }
}

impl Div for float3 {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}
impl Div for &float3 {
    type Output = float3;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self::Output {
        *self / *rhs
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct float4(pub f32x4);

impl float4 {
    #[inline(always)]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self(f32x4::from_array([x, y, z, w]))
    }
    #[inline(always)]
    pub const fn from_array(arr: [f32; 4]) -> Self {
        Self(f32x4::from_array(arr))
    }

    #[inline(always)]
    pub fn max(&self, other: &Self) -> Self {
        Self(self.0.simd_max(other.0))
    }

    #[inline(always)]
    pub fn min(&self, other: &Self) -> Self {
        Self(self.0.simd_min(other.0))
    }

    #[inline(always)]
    pub fn from_float2_x2(xy: &float2, zw: &float2) -> Self {
        Self(simd_swizzle!(xy.0, zw.0, [0, 1, 2, 3]))
    }

    #[inline(always)]
    pub fn from_float2_s(xy: &float2, z: f32, w: f32) -> Self {
        Self(simd_swizzle!(xy.0, f32x2::from_array([z, w]), [0, 1, 2, 3]))
    }

    #[inline(always)]
    pub fn from_s_float2(x: f32, y: f32, zw: &float2) -> Self {
        Self(simd_swizzle!(f32x2::from_array([x, y]), zw.0, [0, 1, 2, 3]))
    }

    #[inline(always)]
    pub fn from_float3_s(xyz: &float3, w: f32) -> Self {
        Self(simd_swizzle!(xyz.0, f32x4::splat(w), [0, 1, 2, 4]))
    }

    #[inline(always)]
    pub fn from_s_float3(x: f32, yzw: &float3) -> Self {
        Self(simd_swizzle!(f32x4::splat(x), yzw.0, [0, 4, 5, 6]))
    }

    #[inline(always)]
    pub fn x(&self) -> f32 {
        self.0[0]
    }

    #[inline(always)]
    pub fn y(&self) -> f32 {
        self.0[1]
    }

    #[inline(always)]
    pub fn z(&self) -> f32 {
        self.0[2]
    }

    #[inline(always)]
    pub fn w(&self) -> f32 {
        self.0[3]
    }

    #[inline(always)]
    pub fn xx(&self) -> float2 {
        float2(simd_swizzle!(self.0, [0, 0]))
    }

    #[inline(always)]
    pub fn xy(&self) -> float2 {
        float2(simd_swizzle!(self.0, [0, 1]))
    }

    #[inline(always)]
    pub fn xz(&self) -> float2 {
        float2(simd_swizzle!(self.0, [0, 2]))
    }

    #[inline(always)]
    pub fn xw(&self) -> float2 {
        float2(simd_swizzle!(self.0, [0, 3]))
    }

    #[inline(always)]
    pub fn yx(&self) -> float2 {
        float2(simd_swizzle!(self.0, [1, 0]))
    }

    #[inline(always)]
    pub fn yy(&self) -> float2 {
        float2(simd_swizzle!(self.0, [1, 1]))
    }

    #[inline(always)]
    pub fn yz(&self) -> float2 {
        float2(simd_swizzle!(self.0, [1, 2]))
    }

    #[inline(always)]
    pub fn yw(&self) -> float2 {
        float2(simd_swizzle!(self.0, [1, 3]))
    }

    #[inline(always)]
    pub fn zx(&self) -> float2 {
        float2(simd_swizzle!(self.0, [2, 0]))
    }

    #[inline(always)]
    pub fn zy(&self) -> float2 {
        float2(simd_swizzle!(self.0, [2, 1]))
    }

    #[inline(always)]
    pub fn zz(&self) -> float2 {
        float2(simd_swizzle!(self.0, [2, 2]))
    }

    #[inline(always)]
    pub fn zw(&self) -> float2 {
        float2(simd_swizzle!(self.0, [2, 3]))
    }

    #[inline(always)]
    pub fn wx(&self) -> float2 {
        float2(simd_swizzle!(self.0, [3, 0]))
    }

    #[inline(always)]
    pub fn wy(&self) -> float2 {
        float2(simd_swizzle!(self.0, [3, 1]))
    }

    #[inline(always)]
    pub fn wz(&self) -> float2 {
        float2(simd_swizzle!(self.0, [3, 2]))
    }

    #[inline(always)]
    pub fn ww(&self) -> float2 {
        float2(simd_swizzle!(self.0, [3, 3]))
    }

    #[inline(always)]
    pub fn xxx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 0, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xxy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 0, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xxz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 0, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xxw(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 0, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xyx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 1, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xyy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 1, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xyz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 1, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xyw(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 1, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xzx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 2, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xzy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 2, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xzz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 2, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xzw(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 2, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xwx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 3, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xwy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 3, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xwz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 3, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn xww(&self) -> float3 {
        float3(simd_swizzle!(self.0, [0, 3, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yxx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 0, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yxy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 0, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yxz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 0, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yxw(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 0, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yyx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 1, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yyy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 1, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yyz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 1, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yyw(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 1, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yzx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 2, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yzy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 2, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yzz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 2, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yzw(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 2, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn ywx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 3, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn ywy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 3, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn ywz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 3, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn yww(&self) -> float3 {
        float3(simd_swizzle!(self.0, [1, 3, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zxx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 0, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zxy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 0, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zxz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 0, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zxw(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 0, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zyx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 1, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zyy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 1, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zyz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 1, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zyw(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 1, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zzx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 2, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zzy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 2, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zzz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 2, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zzw(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 2, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zwx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 3, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zwy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 3, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zwz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 3, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn zww(&self) -> float3 {
        float3(simd_swizzle!(self.0, [2, 3, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wxx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 0, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wxy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 0, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wxz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 0, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wxw(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 0, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wyx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 1, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wyy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 1, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wyz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 1, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wyw(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 1, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wzx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 2, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wzy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 2, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wzz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 2, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wzw(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 2, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wwx(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 3, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wwy(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 3, 1, 1]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn wwz(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 3, 2, 2]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }

    #[inline(always)]
    pub fn www(&self) -> float3 {
        float3(simd_swizzle!(self.0, [3, 3, 3, 3]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]))
    }
}

impl Vector for float4 {
    #[inline(always)]
    fn dot(&self, other: &Self) -> f32 {
        (self.0 * other.0).reduce_sum()
    }

    type CrossOutput = Self;
    #[inline(always)]
    fn cross(&self, other: &Self) -> Self::CrossOutput {
        let a = self.0;
        let b = other.0;

        let a_yzx = simd_swizzle!(a, [1, 2, 0, 3]);
        let a_zxy = simd_swizzle!(a, [2, 0, 1, 3]);
        let b_yzx = simd_swizzle!(b, [1, 2, 0, 3]);
        let b_zxy = simd_swizzle!(b, [2, 0, 1, 3]);

        Self(a_yzx * b_zxy - a_zxy * b_yzx)
        // (self * other.yzxx() - self.yzxx() * other()).yzxx()
    }
}

impl Index<usize> for float4 {
    type Output = f32;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        self.0.index(index)
    }
}

impl Add<f32> for float4 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: f32) -> Self::Output {
        Self(self.0 + f32x4::splat(rhs))
    }
}
impl Add<f32> for &float4 {
    type Output = float4;

    #[inline(always)]
    fn add(self, rhs: f32) -> Self::Output {
        *self + rhs
    }
}
impl Add<float4> for f32 {
    type Output = float4;

    #[inline(always)]
    fn add(self, rhs: float4) -> Self::Output {
        float4(f32x4::splat(self) + rhs.0)
    }
}
impl Add<&float4> for f32 {
    type Output = float4;

    #[inline(always)]
    fn add(self, rhs: &float4) -> Self::Output {
        self + *rhs
    }
}
impl AddAssign<f32> for float4 {
    #[inline(always)]
    fn add_assign(&mut self, rhs: f32) {
        *self = *self + rhs;
    }
}

impl Sub<f32> for float4 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: f32) -> Self::Output {
        Self(self.0 - f32x4::splat(rhs))
    }
}
impl Sub<f32> for &float4 {
    type Output = float4;

    #[inline(always)]
    fn sub(self, rhs: f32) -> Self::Output {
        *self - rhs
    }
}
impl Sub<float4> for f32 {
    type Output = float4;

    #[inline(always)]
    fn sub(self, rhs: float4) -> Self::Output {
        float4(f32x4::splat(self) - rhs.0)
    }
}
impl Sub<&float4> for f32 {
    type Output = float4;

    #[inline(always)]
    fn sub(self, rhs: &float4) -> Self::Output {
        self - *rhs
    }
}
impl SubAssign<f32> for float4 {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: f32) {
        *self = *self - rhs;
    }
}

impl Mul<f32> for float4 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, scalar: f32) -> Self::Output {
        Self(self.0 * f32x4::splat(scalar))
    }
}
impl Mul<f32> for &float4 {
    type Output = float4;

    #[inline(always)]
    fn mul(self, rhs: f32) -> Self::Output {
        *self * rhs
    }
}
impl Mul<float4> for f32 {
    type Output = float4;

    #[inline(always)]
    fn mul(self, rhs: float4) -> Self::Output {
        float4(f32x4::splat(self) * rhs.0)
    }
}
impl Mul<&float4> for f32 {
    type Output = float4;

    #[inline(always)]
    fn mul(self, rhs: &float4) -> Self::Output {
        self * *rhs
    }
}
impl MulAssign<f32> for float4 {
    #[inline(always)]
    fn mul_assign(&mut self, rhs: f32) {
        *self = *self * rhs;
    }
}

impl Div<f32> for float4 {
    type Output = Self;

    #[inline(always)]
    fn div(self, scalar: f32) -> Self::Output {
        Self(self.0 / f32x4::splat(scalar))
    }
}
impl Div<f32> for &float4 {
    type Output = float4;

    #[inline(always)]
    fn div(self, rhs: f32) -> Self::Output {
        *self / rhs
    }
}
impl Div<float4> for f32 {
    type Output = float4;

    #[inline(always)]
    fn div(self, rhs: float4) -> Self::Output {
        float4(f32x4::splat(self) / rhs.0)
    }
}
impl Div<&float4> for f32 {
    type Output = float4;

    #[inline(always)]
    fn div(self, rhs: &float4) -> Self::Output {
        self / *rhs
    }
}
impl DivAssign<f32> for float4 {
    #[inline(always)]
    fn div_assign(&mut self, rhs: f32) {
        *self = *self / rhs;
    }
}

impl Add for float4 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}
impl Add for &float4 {
    type Output = float4;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        *self + *rhs
    }
}

impl Sub for float4 {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}
impl Sub for &float4 {
    type Output = float4;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        *self - *rhs
    }
}

impl Mul for float4 {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}
impl Mul for &float4 {
    type Output = float4;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        *self * *rhs
    }
}

impl Div for float4 {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}
impl Div for &float4 {
    type Output = float4;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self::Output {
        *self / *rhs
    }
}
