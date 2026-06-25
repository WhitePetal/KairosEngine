use crate::math::float3;

pub struct AABB {
    pub max: float3,
    pub min: float3,
}
impl AABB {
    /// 判断一个点是否在 AABB 内部（包含边界）。
    /// 使用 SIMD 一次比较全部 3 个轴，4 条指令完成。
    #[inline]
    pub fn contains_point(&self, point: float3) -> bool {
        use std::simd::cmp::SimdPartialOrd;
        let ge = point.0.simd_ge(self.min.0);
        let le = point.0.simd_le(self.max.0);
        // 只检查 xyz (lane 0,1,2)，忽略 w
        (ge & le).to_bitmask() & 0b0111 == 0b0111
    }
}
