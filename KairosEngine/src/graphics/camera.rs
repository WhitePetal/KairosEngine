use crate::math::float3;


pub struct Camera {
    pub position: float3,
    pub forward: float3,
    pub right: float3,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn get_view_matrix(&self) {
        
    }
}