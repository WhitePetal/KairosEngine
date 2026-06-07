use crate::{
    ecs::component::Component,
    math::{float3, float4x4, quaternion},
};

pub struct TransformComponent {
    pub position: float3,
    pub rotation: quaternion,
    pub scale: float3,
}
impl Component for TransformComponent {}

impl TransformComponent {
    pub fn get_local_to_world(&self) -> float4x4 {
        float4x4::trs(self.position, self.rotation, self.scale)
    }
}
