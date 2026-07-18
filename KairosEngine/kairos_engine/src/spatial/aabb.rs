use crate::math::float3;

pub struct AABB {
    pub max: float3,
    pub min: float3,
}
impl AABB {
    /// 判断一个点是否在 AABB 内部（包含边界）。
    /// 使用 SIMD 一次比较全部 3 个轴。
    #[inline]
    pub fn contains_point(&self, point: float3) -> bool {
        let ge = point.0.cmpge(self.min.0);
        let le = point.0.cmple(self.max.0);
        // 只检查 xyz
        ge.all() && le.all()
    }
}
