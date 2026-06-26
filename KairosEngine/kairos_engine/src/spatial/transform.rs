use crate::{
    ecs::component::Component,
    math::{float3, float4x4, quaternion},
};

#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub position: float3,
    pub rotation: quaternion,
    pub scale: float3,
}
impl Component for Transform {}

impl Transform {
    pub fn new(position: float3, rotation: quaternion, scale: float3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }

    pub fn get_local_to_world(&self) -> float4x4 {
        float4x4::trs(self.position, self.rotation, self.scale)
    }
}
