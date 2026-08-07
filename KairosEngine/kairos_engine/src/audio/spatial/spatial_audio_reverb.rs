use crate::{ecs::component::Component, math::float3, spatial::AABB};

pub struct SpatialAudioReverbBound {
    pub aabb: AABB,
}
// TODO!
// impl Component for SpatialAudioReverbBound {}

impl SpatialAudioReverbBound {
    pub fn contains_point(&self, point: float3) -> bool {
        self.aabb.contains_point(point)
    }
}

pub struct SpatialAudioReverb {
    pub distance_range: f32,
    pub min_volume: f32,
    pub max_volume: f32,
    pub feed_back: f32,
    pub damping: f32,
    pub mix: f32,
}
// impl Component for SpatialAudioReverb {}

impl SpatialAudioReverb {
    pub fn new(
        distance_range: f32,
        min_volume: f32,
        max_volume: f32,
        feed_back: f32,
        damping: f32,
        mix: f32,
        aabb: AABB,
    ) -> (Self, SpatialAudioReverbBound) {
        (
            SpatialAudioReverb {
                distance_range,
                min_volume,
                max_volume,
                feed_back,
                damping,
                mix,
            },
            SpatialAudioReverbBound { aabb },
        )
    }
}
