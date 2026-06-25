use crate::{ecs::component::Component, spatial::AABB};

pub struct SpatialAudioReverbBound {
    pub aabb: AABB,
}
impl Component for SpatialAudioReverbBound {}

pub struct SpatialAudioReverb {
    pub distance_range: f32,
    pub min_volume: f32,
    pub max_volume: f32,
}
impl Component for SpatialAudioReverb {}

impl SpatialAudioReverb {
    /// 便捷构造：一次创建 SpatialAudioReverb 和对应的 SpatialAudioReverbBound，
    /// 但二者在 ECS 中分别独立存储，保持缓存友好。
    pub fn with_bound(
        distance_range: f32,
        min_volume: f32,
        max_volume: f32,
        aabb: AABB,
    ) -> (Self, SpatialAudioReverbBound) {
        (
            SpatialAudioReverb {
                distance_range,
                min_volume,
                max_volume,
            },
            SpatialAudioReverbBound { aabb },
        )
    }
}
