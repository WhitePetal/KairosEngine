#[cfg(test)]
mod tests;

use std::{ops::Mul, simd::simd_swizzle};

use crate::math::float4;

///
/// matrix base column vector
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct float4x4([float4; 4]);

impl float4x4 {
    ///
    /// ```
    /// | v1 | v2 | v3 | v4 |
    /// --------------------|
    /// | 1  |  0 |  0 |  0 |
    /// | 0  |  1 |  0 |  0 |
    /// | 0  |  0 |  1 |  0 |
    /// | 0  |  0 |  0 |  1 |
    /// ---------------------
    /// ```
    #[inline(always)]
    pub fn idenity() -> Self {
        Self([
            float4::new(1.0, 0.0, 0.0, 0.0),
            float4::new(0.0, 1.0, 0.0, 0.0),
            float4::new(0.0, 0.0, 1.0, 0.0),
            float4::new(0.0, 0.0, 0.0, 1.0),
        ])
    }
}

impl Mul for float4x4 {
    type Output = Self;

    ///
    /// ```
    /// |------------ Ml -----------|  *  |-------------Mr------------|
    /// |  l1  |  l2  |  l3  |  l4  |  *  |  r1  |  r2  |  r3  |  r4  |
    /// ---------------------------------------------------------------
    /// | l1.x | l2.x | l3.x | l4.x |  *  | r1.x | r2.x | r3.x | r4.x |
    /// | l1.y | l2.y | l3.y | l4.y |  *  | r1.y | r2.y | r3.y | r4.y |
    /// | l1.z | l2.z | l3.z | l4.z |  *  | r1.z | r2.z | r3.z | r4.z |
    /// | l1.w | l2.w | l3.w | l4.w |  *  | r1.w | r2.w | r3.w | r4.w |
    /// ----------------------------------------------------------------
    /// ----------------------------------------------------------------
    /// |    v1   |    v2   |    v3   |    v4   |
    /// ----------------------------------------|
    /// | Ml * r1 | Ml * r2 | Ml * r3 | Ml * r4 |
    /// |---------------------------------------|
    /// ```
    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Self([
            self * rhs.0[0],
            self * rhs.0[1],
            self * rhs.0[2],
            self * rhs.0[3],
        ])
    }
}
impl Mul<float4> for float4x4 {
    type Output = float4;

    ///
    /// ```
    /// |------------ Ml -----------|  *  |--r--|
    /// |  l1  |  l2  |  l3  |  l4  |  *  |  r  |
    /// ----------------------------------------|
    /// | l1.x | l2.x | l3.x | l4.x |  *  | r.x |
    /// | l1.y | l2.y | l3.y | l4.y |  *  | r.y |
    /// | l1.z | l2.z | l3.z | l4.z |  *  | r.z |
    /// | l1.w | l2.w | l3.w | l4.w |  *  | r.w |
    /// -----------------------------------------
    /// --------------------------------------------------------
    /// |       v1     |      v2     |     v3    |      v4     |
    /// -------------------------------------------------------|
    /// | l1 * r.xxxx + l2 * r.yyyy + l3 * r.zzzz + l4 * r.wwww|
    /// |------------------------------------------------------|
    /// ```
    #[inline(always)]
    fn mul(self, rhs: float4) -> Self::Output {
        float4::from_simd(
            &self.0[0].0 * simd_swizzle!(rhs.0, [0, 0, 0, 0])
                + &self.0[1].0 * simd_swizzle!(rhs.0, [1, 1, 1, 1])
                + &self.0[2].0 * simd_swizzle!(rhs.0, [2, 2, 2, 2])
                + &self.0[3].0 * simd_swizzle!(rhs.0, [3, 3, 3, 3]),
        )
    }
}
