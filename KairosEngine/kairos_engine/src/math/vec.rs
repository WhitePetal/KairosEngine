mod converts;

use std::ops::{Add, AddAssign, Div, DivAssign, Index, Mul, MulAssign, Sub, SubAssign};

use glam::{Vec2, Vec3A, Vec4, Vec4Swizzles};
use serde::{Deserialize, Serialize};

use crate::math::{Cos, Lerp, LerpFactor, Max, Min, Sin, Sqrt, Tan};

pub trait Vector
where
    Self: Add<f32, Output = Self>
        + Sub<f32, Output = Self>
        + Mul<f32, Output = Self>
        + Div<f32, Output = Self>
        + Add
        + Sub<Self, Output = Self>
        + Mul
        + Div
        + AddAssign<f32>
        + SubAssign<f32>
        + MulAssign<f32>
        + DivAssign<f32>
        + Index<usize>
        + Clone
        + Copy
        + PartialEq
        + Min
        + Max
        + Sin
        + Cos
        + Tan
        + Sqrt
        + Lerp,
{
    fn dot(&self, r: &Self) -> f32;

    type CrossOutput;
    fn cross(&self, r: Self) -> Self::CrossOutput;

    #[inline(always)]
    fn len_sq(&self) -> f32 {
        self.dot(self)
    }

    #[inline(always)]
    fn len(&self) -> f32 {
        self.len_sq().sqrt()
    }

    #[inline(always)]
    fn normalize(self) -> Self {
        self / self.len()
    }

    #[inline(always)]
    fn normalized(&mut self) {
        *self /= self.len();
    }

    #[inline(always)]
    fn distance(l: Self, r: Self) -> f32 {
        let v = l - r;
        v.len()
    }

    #[inline(always)]
    fn distance_sq(l: Self, r: Self) -> f32 {
        let v = l - r;
        v.len_sq()
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
pub fn cross<T>(l: T, r: T) -> T::CrossOutput
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
pub fn normalize<T>(v: T) -> T
where
    T: Vector,
{
    v.normalize()
}

#[inline(always)]
pub fn normalized<T>(mut v: T)
where
    T: Vector,
{
    v.normalized();
}

// ============================================================
// float2  —  wraps glam::Vec2
// ============================================================

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct float2(pub(crate) Vec2);

impl float2 {
    pub const ONE: float2 = float2::new(1.0, 1.0);
    pub const ZERO: float2 = float2::new(0.0, 0.0);

    #[inline(always)]
    pub const fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
    #[inline(always)]
    pub fn from_array(arr: [f32; 2]) -> Self {
        Self(Vec2::from_array(arr))
    }
    #[inline(always)]
    pub fn to_array(&self) -> [f32; 2] {
        self.0.to_array()
    }

    pub fn x(&self) -> f32 {
        self[0]
    }

    pub fn y(&self) -> f32 {
        self[1]
    }
}

impl Vector for float2 {
    type CrossOutput = f32;

    #[inline(always)]
    fn dot(&self, other: &Self) -> f32 {
        self.0.dot(other.0)
    }

    #[inline(always)]
    fn cross(&self, other: Self) -> Self::CrossOutput {
        self.0[0] * other.0[1] - self.0[1] * other.0[0]
    }
}

unsafe impl bytemuck::Zeroable for float2 {}
unsafe impl bytemuck::Pod for float2 {}

impl Index<usize> for float2 {
    type Output = f32;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl Add<f32> for float2 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: f32) -> Self::Output {
        Self(self.0 + Vec2::splat(rhs))
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
        float2(Vec2::splat(self) + rhs.0)
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
        Self(self.0 - Vec2::splat(rhs))
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
        float2(Vec2::splat(self) - rhs.0)
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
        Self(self.0 * Vec2::splat(rhs))
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
        float2(Vec2::splat(self) * rhs.0)
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
        Self(self.0 / Vec2::splat(rhs))
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
        float2(Vec2::splat(self) / rhs.0)
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
impl Min for float2 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }
}
impl Max for float2 {
    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }
}
impl Sin for float2 {
    #[inline(always)]
    fn sin(self) -> Self {
        Self::new(self.x().sin(), self.y().sin())
    }
}
impl Cos for float2 {
    #[inline(always)]
    fn cos(self) -> Self {
        Self::new(self.x().cos(), self.y().cos())
    }
}
impl Tan for float2 {
    #[inline(always)]
    fn tan(self) -> Self {
        Self::new(self.x().tan(), self.y().tan())
    }
}
impl Sqrt for float2 {
    #[inline(always)]
    fn sqrt(self) -> Self {
        Self::new(self.x().sqrt(), self.y().sqrt())
    }
}
impl LerpFactor<float2> for f32 {
    #[inline(always)]
    fn get_factor(self) -> float2 {
        float2(Vec2::splat(self))
    }
}
impl LerpFactor<float2> for float2 {
    #[inline(always)]
    fn get_factor(self) -> float2 {
        self
    }
}
impl Lerp for float2 {
    #[inline(always)]
    fn lerp(left: Self, right: Self, factor: impl LerpFactor<float2>) -> Self {
        left + (right - left) * factor.get_factor()
    }
}

impl Serialize for float2 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_array().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for float2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let arry = <[f32; 2]>::deserialize(deserializer)?;
        Ok(Self::from_array(arry))
    }
}

impl rkyv::Archive for float2 {
    type Archived = [rkyv::primitive::ArchivedF32; 2];
    type Resolver = ();

    fn resolve(&self, _: Self::Resolver, out: rkyv::Place<Self::Archived>) {
        out.write([
            rkyv::primitive::ArchivedF32::from_native(self.x()),
            rkyv::primitive::ArchivedF32::from_native(self.y()),
        ]);
    }
}

impl<S: rkyv::rancor::Fallible + ?Sized> rkyv::Serialize<S> for float2 {
    fn serialize(&self, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}

impl<D: rkyv::rancor::Fallible + ?Sized> rkyv::Deserialize<float2, D>
    for <float2 as rkyv::Archive>::Archived
{
    fn deserialize(&self, _: &mut D) -> Result<float2, D::Error> {
        Ok(float2::from_array([
            self[0].to_native(),
            self[1].to_native(),
        ]))
    }
}

// ============================================================
// float3  —  wraps glam::Vec3A (SIMD-aligned Vec3)
//            w lane is always 0.0 for dot / cross correctness
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct float3(pub(crate) Vec3A);

impl float3 {
    pub const ZERO: float3 = float3::new(0.0, 0.0, 0.0);
    pub const ONE: float3 = float3::new(1.0, 1.0, 1.0);

    // 右手系, Y up, -Z forward
    pub const RIGHT: float3 = float3::new(1.0, 0.0, 0.0);
    pub const LEFT: float3 = float3::new(-1.0, 0.0, 0.0);
    pub const UP: float3 = float3::new(0.0, 1.0, 0.0);
    pub const DOWN: float3 = float3::new(0.0, -1.0, 0.0);
    pub const FORWARD: float3 = float3::new(0.0, 0.0, -1.0);
    pub const BACK: float3 = float3::new(0.0, 0.0, 1.0);

    #[inline(always)]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3A::new(x, y, z))
    }
    #[inline(always)]
    pub fn from_array(arr: [f32; 3]) -> Self {
        Self(Vec3A::from_array(arr))
    }
    #[inline(always)]
    pub fn from_array_4(arr: [f32; 4]) -> Self {
        Self(Vec3A::new(arr[0], arr[1], arr[2]))
    }
    #[inline(always)]
    pub fn to_array(&self) -> [f32; 4] {
        [self.x(), self.y(), self.z(), 0.0]
    }

    #[inline(always)]
    pub fn x(&self) -> f32 {
        self[0]
    }

    #[inline(always)]
    pub fn y(&self) -> f32 {
        self[1]
    }

    #[inline(always)]
    pub fn z(&self) -> f32 {
        self[2]
    }

    #[inline(always)]
    pub fn append(&self, w: f32) -> float4 {
        float4(Vec4::new(self.x(), self.y(), self.z(), w))
    }
}

impl Vector for float3 {
    #[inline(always)]
    fn dot(&self, other: &Self) -> f32 {
        self.0.dot(other.0)
    }

    type CrossOutput = float3;
    #[inline(always)]
    fn cross(&self, other: Self) -> Self::CrossOutput {
        Self(self.0.cross(other.0))
    }
}

impl Index<usize> for float3 {
    type Output = f32;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl Add<f32> for float3 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: f32) -> Self::Output {
        Self(self.0 + Vec3A::splat(rhs))
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
        float3(Vec3A::splat(self) + rhs.0)
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
        Self(self.0 - Vec3A::splat(rhs))
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
        float3(Vec3A::splat(self) - rhs.0)
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
        Self(self.0 * Vec3A::splat(scalar))
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
        float3(Vec3A::splat(self) * rhs.0)
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
        Self(self.0 / Vec3A::splat(scalar))
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
        float3(Vec3A::splat(self) / rhs.0)
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
impl Eq for float3 {}

impl Min for float3 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }
}
impl Max for float3 {
    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }
}
impl Sin for float3 {
    #[inline(always)]
    fn sin(self) -> Self {
        Self::new(self.x().sin(), self.y().sin(), self.z().sin())
    }
}
impl Cos for float3 {
    #[inline(always)]
    fn cos(self) -> Self {
        Self::new(self.x().cos(), self.y().cos(), self.z().cos())
    }
}
impl Tan for float3 {
    #[inline(always)]
    fn tan(self) -> Self {
        Self::new(self.x().tan(), self.y().tan(), self.z().tan())
    }
}
impl Sqrt for float3 {
    #[inline(always)]
    fn sqrt(self) -> Self {
        Self::new(self.x().sqrt(), self.y().sqrt(), self.z().sqrt())
    }
}
impl LerpFactor<float3> for f32 {
    #[inline(always)]
    fn get_factor(self) -> float3 {
        float3(Vec3A::splat(self))
    }
}
impl LerpFactor<float3> for float3 {
    #[inline(always)]
    fn get_factor(self) -> float3 {
        self
    }
}
impl Lerp for float3 {
    #[inline(always)]
    fn lerp(left: Self, right: Self, factor: impl LerpFactor<float3>) -> Self {
        left + (right - left) * factor.get_factor()
    }
}

impl From<[f32; 3]> for float3 {
    #[inline(always)]
    fn from(value: [f32; 3]) -> Self {
        Self::from_array(value)
    }
}

impl Serialize for float3 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_array().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for float3 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let arry = <[f32; 4]>::deserialize(deserializer)?;
        Ok(Self::from_array_4(arry))
    }
}

impl rkyv::Archive for float3 {
    type Archived = [rkyv::primitive::ArchivedF32; 3];
    type Resolver = ();

    fn resolve(&self, _: Self::Resolver, out: rkyv::Place<Self::Archived>) {
        out.write([
            rkyv::primitive::ArchivedF32::from_native(self.x()),
            rkyv::primitive::ArchivedF32::from_native(self.y()),
            rkyv::primitive::ArchivedF32::from_native(self.z()),
        ]);
    }
}

impl<S: rkyv::rancor::Fallible + ?Sized> rkyv::Serialize<S> for float3 {
    fn serialize(&self, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}

impl<D: rkyv::rancor::Fallible + ?Sized> rkyv::Deserialize<float3, D>
    for <float3 as rkyv::Archive>::Archived
{
    fn deserialize(&self, _: &mut D) -> Result<float3, D::Error> {
        Ok(float3::from_array([
            self[0].to_native(),
            self[1].to_native(),
            self[2].to_native(),
        ]))
    }
}

// ============================================================
// float4  —  wraps glam::Vec4 (16-byte SIMD on SSE2/NEON)
//           swizzle methods delegate to glam::Vec4Swizzles
// ============================================================

///
/// column vector
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct float4(pub(crate) Vec4);

impl float4 {
    pub const ONE: float4 = float4::new(1.0, 1.0, 1.0, 1.0);
    pub const ZERO: float4 = float4::new(0.0, 0.0, 0.0, 0.0);

    #[inline(always)]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self(Vec4::new(x, y, z, w))
    }

    #[inline(always)]
    pub(crate) fn from_inner(v: Vec4) -> Self {
        Self(v)
    }

    #[inline(always)]
    pub fn from_array(array: [f32; 4]) -> Self {
        Self(Vec4::from_array(array))
    }

    #[inline(always)]
    pub fn max(&self, other: &Self) -> Self {
        Self(self.0.max(other.0))
    }

    #[inline(always)]
    pub fn min(&self, other: &Self) -> Self {
        Self(self.0.min(other.0))
    }

    #[inline(always)]
    pub fn x(&self) -> f32 {
        self.0.x
    }

    #[inline(always)]
    pub fn y(&self) -> f32 {
        self.0.y
    }

    #[inline(always)]
    pub fn z(&self) -> f32 {
        self.0.z
    }

    #[inline(always)]
    pub fn w(&self) -> f32 {
        self.0.w
    }

    #[inline(always)]
    pub fn set_w(&mut self, w: f32) {
        self.0.w = w;
    }

    // ── Swizzles returning float2 ──
    #[inline(always)]
    pub fn xx(&self) -> float2 {
        float2(self.0.xx())
    }
    #[inline(always)]
    pub fn xy(&self) -> float2 {
        float2(self.0.xy())
    }
    #[inline(always)]
    pub fn xz(&self) -> float2 {
        float2(self.0.xz())
    }
    #[inline(always)]
    pub fn xw(&self) -> float2 {
        float2(self.0.xw())
    }
    #[inline(always)]
    pub fn yx(&self) -> float2 {
        float2(self.0.yx())
    }
    #[inline(always)]
    pub fn yy(&self) -> float2 {
        float2(self.0.yy())
    }
    #[inline(always)]
    pub fn yz(&self) -> float2 {
        float2(self.0.yz())
    }
    #[inline(always)]
    pub fn yw(&self) -> float2 {
        float2(self.0.yw())
    }
    #[inline(always)]
    pub fn zx(&self) -> float2 {
        float2(self.0.zx())
    }
    #[inline(always)]
    pub fn zy(&self) -> float2 {
        float2(self.0.zy())
    }
    #[inline(always)]
    pub fn zz(&self) -> float2 {
        float2(self.0.zz())
    }
    #[inline(always)]
    pub fn zw(&self) -> float2 {
        float2(self.0.zw())
    }
    #[inline(always)]
    pub fn wx(&self) -> float2 {
        float2(self.0.wx())
    }
    #[inline(always)]
    pub fn wy(&self) -> float2 {
        float2(self.0.wy())
    }
    #[inline(always)]
    pub fn wz(&self) -> float2 {
        float2(self.0.wz())
    }
    #[inline(always)]
    pub fn ww(&self) -> float2 {
        float2(self.0.ww())
    }

    // ── Swizzles returning float3 ──
    #[inline(always)]
    pub fn xxx(&self) -> float3 {
        float3(self.0.xxx().into())
    }
    #[inline(always)]
    pub fn xxy(&self) -> float3 {
        float3(self.0.xxy().into())
    }
    #[inline(always)]
    pub fn xxz(&self) -> float3 {
        float3(self.0.xxz().into())
    }
    #[inline(always)]
    pub fn xxw(&self) -> float3 {
        float3(self.0.xxw().into())
    }
    #[inline(always)]
    pub fn xyx(&self) -> float3 {
        float3(self.0.xyx().into())
    }
    #[inline(always)]
    pub fn xyy(&self) -> float3 {
        float3(self.0.xyy().into())
    }
    #[inline(always)]
    pub fn xyz(&self) -> float3 {
        float3(self.0.xyz().into())
    }
    #[inline(always)]
    pub fn xyw(&self) -> float3 {
        float3(self.0.xyw().into())
    }
    #[inline(always)]
    pub fn xzx(&self) -> float3 {
        float3(self.0.xzx().into())
    }
    #[inline(always)]
    pub fn xzy(&self) -> float3 {
        float3(self.0.xzy().into())
    }
    #[inline(always)]
    pub fn xzz(&self) -> float3 {
        float3(Vec3A::new(self.x(), self.z(), self.z()))
    }
    #[inline(always)]
    pub fn xzw(&self) -> float3 {
        float3(self.0.xzw().into())
    }
    #[inline(always)]
    pub fn xwx(&self) -> float3 {
        float3(self.0.xwx().into())
    }
    #[inline(always)]
    pub fn xwy(&self) -> float3 {
        float3(self.0.xwy().into())
    }
    #[inline(always)]
    pub fn xwz(&self) -> float3 {
        float3(self.0.xwz().into())
    }
    #[inline(always)]
    pub fn xww(&self) -> float3 {
        float3(Vec3A::new(self.x(), self.w(), self.w()))
    }

    #[inline(always)]
    pub fn yxx(&self) -> float3 {
        float3(self.0.yxx().into())
    }
    #[inline(always)]
    pub fn yxy(&self) -> float3 {
        float3(self.0.yxy().into())
    }
    #[inline(always)]
    pub fn yxz(&self) -> float3 {
        float3(self.0.yxz().into())
    }
    #[inline(always)]
    pub fn yxw(&self) -> float3 {
        float3(self.0.yxw().into())
    }
    #[inline(always)]
    pub fn yyx(&self) -> float3 {
        float3(self.0.yyx().into())
    }
    #[inline(always)]
    pub fn yyy(&self) -> float3 {
        float3(self.0.yyy().into())
    }
    #[inline(always)]
    pub fn yyz(&self) -> float3 {
        float3(self.0.yyz().into())
    }
    #[inline(always)]
    pub fn yyw(&self) -> float3 {
        float3(self.0.yyw().into())
    }
    #[inline(always)]
    pub fn yzx(&self) -> float3 {
        float3(self.0.yzx().into())
    }
    #[inline(always)]
    pub fn yzy(&self) -> float3 {
        float3(self.0.yzy().into())
    }
    #[inline(always)]
    pub fn yzz(&self) -> float3 {
        float3(Vec3A::new(self.y(), self.z(), self.z()))
    }
    #[inline(always)]
    pub fn yzw(&self) -> float3 {
        float3(self.0.yzw().into())
    }
    #[inline(always)]
    pub fn ywx(&self) -> float3 {
        float3(self.0.ywx().into())
    }
    #[inline(always)]
    pub fn ywy(&self) -> float3 {
        float3(self.0.ywy().into())
    }
    #[inline(always)]
    pub fn ywz(&self) -> float3 {
        float3(self.0.ywz().into())
    }
    #[inline(always)]
    pub fn yww(&self) -> float3 {
        float3(Vec3A::new(self.y(), self.w(), self.w()))
    }

    #[inline(always)]
    pub fn zxx(&self) -> float3 {
        float3(self.0.zxx().into())
    }
    #[inline(always)]
    pub fn zxy(&self) -> float3 {
        float3(self.0.zxy().into())
    }
    #[inline(always)]
    pub fn zxz(&self) -> float3 {
        float3(self.0.zxz().into())
    }
    #[inline(always)]
    pub fn zxw(&self) -> float3 {
        float3(self.0.zxw().into())
    }
    #[inline(always)]
    pub fn zyx(&self) -> float3 {
        float3(self.0.zyx().into())
    }
    #[inline(always)]
    pub fn zyy(&self) -> float3 {
        float3(self.0.zyy().into())
    }
    #[inline(always)]
    pub fn zyz(&self) -> float3 {
        float3(self.0.zyz().into())
    }
    #[inline(always)]
    pub fn zyw(&self) -> float3 {
        float3(self.0.zyw().into())
    }
    #[inline(always)]
    pub fn zzx(&self) -> float3 {
        float3(self.0.zzx().into())
    }
    #[inline(always)]
    pub fn zzy(&self) -> float3 {
        float3(self.0.zzy().into())
    }
    #[inline(always)]
    pub fn zzz(&self) -> float3 {
        float3(self.0.zzz().into())
    }
    #[inline(always)]
    pub fn zzw(&self) -> float3 {
        float3(self.0.zzw().into())
    }
    #[inline(always)]
    pub fn zwx(&self) -> float3 {
        float3(self.0.zwx().into())
    }
    #[inline(always)]
    pub fn zwy(&self) -> float3 {
        float3(self.0.zwy().into())
    }
    #[inline(always)]
    pub fn zwz(&self) -> float3 {
        float3(self.0.zwz().into())
    }
    #[inline(always)]
    pub fn zww(&self) -> float3 {
        float3(Vec3A::new(self.z(), self.w(), self.w()))
    }

    #[inline(always)]
    pub fn wxx(&self) -> float3 {
        float3(self.0.wxx().into())
    }
    #[inline(always)]
    pub fn wxy(&self) -> float3 {
        float3(self.0.wxy().into())
    }
    #[inline(always)]
    pub fn wxz(&self) -> float3 {
        float3(self.0.wxz().into())
    }
    #[inline(always)]
    pub fn wxw(&self) -> float3 {
        float3(self.0.wxw().into())
    }
    #[inline(always)]
    pub fn wyx(&self) -> float3 {
        float3(self.0.wyx().into())
    }
    #[inline(always)]
    pub fn wyy(&self) -> float3 {
        float3(self.0.wyy().into())
    }
    #[inline(always)]
    pub fn wyz(&self) -> float3 {
        float3(self.0.wyz().into())
    }
    #[inline(always)]
    pub fn wyw(&self) -> float3 {
        float3(self.0.wyw().into())
    }
    #[inline(always)]
    pub fn wzx(&self) -> float3 {
        float3(self.0.wzx().into())
    }
    #[inline(always)]
    pub fn wzy(&self) -> float3 {
        float3(self.0.wzy().into())
    }
    #[inline(always)]
    pub fn wzz(&self) -> float3 {
        float3(Vec3A::new(self.w(), self.z(), self.z()))
    }
    #[inline(always)]
    pub fn wzw(&self) -> float3 {
        float3(self.0.wzw().into())
    }
    #[inline(always)]
    pub fn wwx(&self) -> float3 {
        float3(self.0.wwx().into())
    }
    #[inline(always)]
    pub fn wwy(&self) -> float3 {
        float3(self.0.wwy().into())
    }
    #[inline(always)]
    pub fn wwz(&self) -> float3 {
        float3(self.0.wwz().into())
    }
    #[inline(always)]
    pub fn www(&self) -> float3 {
        float3(self.0.www().into())
    }

    #[inline(always)]
    pub fn to_array(self) -> [f32; 4] {
        self.0.to_array()
    }
}

impl Vector for float4 {
    #[inline(always)]
    fn dot(&self, other: &Self) -> f32 {
        self.0.dot(other.0)
    }

    type CrossOutput = Self;
    #[inline(always)]
    fn cross(&self, other: Self) -> Self::CrossOutput {
        let a = self.0;
        let b = other.0;

        let a_yzxw = a.yzxw();
        let a_zxyw = a.zxyw();
        let b_yzxw = b.yzxw();
        let b_zxyw = b.zxyw();

        Self(a_yzxw * b_zxyw - a_zxyw * b_yzxw)
    }
}

impl From<[f32; 4]> for float4 {
    #[inline(always)]
    fn from(arry: [f32; 4]) -> Self {
        Self(Vec4::from_array(arry))
    }
}
impl From<(float2, float2)> for float4 {
    #[inline(always)]
    fn from((xy, zw): (float2, float2)) -> Self {
        let xy = xy.0;
        let zw = zw.0;
        Self(Vec4::new(xy.x, xy.y, zw.x, zw.y))
    }
}
impl From<(float2, f32, f32)> for float4 {
    #[inline(always)]
    fn from((xy, z, w): (float2, f32, f32)) -> Self {
        let xy = xy.0;
        Self(Vec4::new(xy.x, xy.y, z, w))
    }
}
impl From<(f32, f32, float2)> for float4 {
    #[inline(always)]
    fn from((x, y, zw): (f32, f32, float2)) -> Self {
        let zw = zw.0;
        Self(Vec4::new(x, y, zw.x, zw.y))
    }
}
impl From<(float3, f32)> for float4 {
    #[inline(always)]
    fn from((xyz, w): (float3, f32)) -> Self {
        Self(Vec4::new(xyz.x(), xyz.y(), xyz.z(), w))
    }
}
impl From<(f32, float3)> for float4 {
    #[inline(always)]
    fn from((x, yzw): (f32, float3)) -> Self {
        Self(Vec4::new(x, yzw.x(), yzw.y(), yzw.z()))
    }
}

unsafe impl bytemuck::Zeroable for float4 {}
unsafe impl bytemuck::Pod for float4 {}

impl Index<usize> for float4 {
    type Output = f32;

    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl Add<f32> for float4 {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: f32) -> Self::Output {
        Self(self.0 + Vec4::splat(rhs))
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
        float4(Vec4::splat(self) + rhs.0)
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
        Self(self.0 - Vec4::splat(rhs))
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
        float4(Vec4::splat(self) - rhs.0)
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
        Self(self.0 * Vec4::splat(scalar))
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
        float4(Vec4::splat(self) * rhs.0)
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
        Self(self.0 / Vec4::splat(scalar))
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
        float4(Vec4::splat(self) / rhs.0)
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

impl Min for float4 {
    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }
}
impl Max for float4 {
    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }
}
impl Sin for float4 {
    #[inline(always)]
    fn sin(self) -> Self {
        Self::new(
            self.x().sin(),
            self.y().sin(),
            self.z().sin(),
            self.w().sin(),
        )
    }
}
impl Cos for float4 {
    #[inline(always)]
    fn cos(self) -> Self {
        Self::new(
            self.x().cos(),
            self.y().cos(),
            self.z().cos(),
            self.w().cos(),
        )
    }
}
impl Tan for float4 {
    #[inline(always)]
    fn tan(self) -> Self {
        Self::new(
            self.x().tan(),
            self.y().tan(),
            self.z().tan(),
            self.w().tan(),
        )
    }
}
impl Sqrt for float4 {
    #[inline(always)]
    fn sqrt(self) -> Self {
        Self::new(
            self.x().sqrt(),
            self.y().sqrt(),
            self.z().sqrt(),
            self.w().sqrt(),
        )
    }
}
impl LerpFactor<float4> for f32 {
    #[inline(always)]
    fn get_factor(self) -> float4 {
        float4(Vec4::splat(self))
    }
}
impl LerpFactor<float4> for float4 {
    #[inline(always)]
    fn get_factor(self) -> float4 {
        self
    }
}
impl Lerp for float4 {
    #[inline(always)]
    fn lerp(left: Self, right: Self, factor: impl LerpFactor<float4>) -> Self {
        left + (right - left) * factor.get_factor()
    }
}

impl Serialize for float4 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_array().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for float4 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let arry = <[f32; 4]>::deserialize(deserializer)?;
        Ok(Self::from_array(arry))
    }
}

impl rkyv::Archive for float4 {
    type Archived = [rkyv::primitive::ArchivedF32; 4];
    type Resolver = ();

    fn resolve(&self, _: Self::Resolver, out: rkyv::Place<Self::Archived>) {
        out.write([
            rkyv::primitive::ArchivedF32::from_native(self.x()),
            rkyv::primitive::ArchivedF32::from_native(self.y()),
            rkyv::primitive::ArchivedF32::from_native(self.z()),
            rkyv::primitive::ArchivedF32::from_native(self.w()),
        ]);
    }
}

impl<S: rkyv::rancor::Fallible + ?Sized> rkyv::Serialize<S> for float4 {
    fn serialize(&self, _: &mut S) -> Result<Self::Resolver, S::Error> {
        Ok(())
    }
}

impl<D: rkyv::rancor::Fallible + ?Sized> rkyv::Deserialize<float4, D>
    for <float4 as rkyv::Archive>::Archived
{
    fn deserialize(&self, _: &mut D) -> Result<float4, D::Error> {
        Ok(float4::from_array([
            self[0].to_native(),
            self[1].to_native(),
            self[2].to_native(),
            self[3].to_native(),
        ]))
    }
}
