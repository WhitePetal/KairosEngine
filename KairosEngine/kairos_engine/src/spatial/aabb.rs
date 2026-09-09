use crate::math::float3;

pub struct AABB {
    pub max: float3,
    pub min: float3,
}
impl AABB {
    /// 判断一个点是否在 AABB 内部（包含边界）。
    #[inline]
    pub fn contains_point(&self, point: float3) -> bool {
        // 过渡期逐分量比较：AABB 将于 #146 迁入 kairos_math，届时 crate 内
        // 可直接用 SIMD 内层访问（原实现 point.0.cmpge/cmp 依赖 float3 的
        // crate-private glam 内层，re-export 后不可见）。
        point.x() >= self.min.x()
            && point.y() >= self.min.y()
            && point.z() >= self.min.z()
            && point.x() <= self.max.x()
            && point.y() <= self.max.y()
            && point.z() <= self.max.z()
    }
}
