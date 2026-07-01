use crate::{
    ecs::component::Component,
    math::{self, float3, float4, float4x4},
    spatial::Transform,
};

/// Pure projection parameters; view matrix is derived from a `Transform`.
pub struct Camera {
    pub fov: f32,
    /// width / height
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}
impl Component for Camera {}

impl Camera {
    pub fn new(fov: f32, aspect: f32, near: f32, far: f32) -> Self {
        Self {
            fov,
            aspect,
            near,
            far,
        }
    }

    /// World→View matrix from the camera's world-space `Transform`.
    ///
    /// `view = inverse(camera_world)`, where `camera_world` is derived from
    /// `transform.rotation` (right = +X, up = +Y, forward = -Z) and
    /// `transform.position`.
    pub fn get_view_matrix(&self, transform: Transform) -> float4x4 {
        // Columns of the rotation matrix R = [right | up | -forward]
        let m = transform.rotation.to_float4x4();
        let r = float3::new(m.c0().x(), m.c0().y(), m.c0().z()); // right
        let u = float3::new(m.c1().x(), m.c1().y(), m.c1().z()); // up
        let f = float3::new(m.c2().x(), m.c2().y(), m.c2().z()) * -1.0; // forward = -c2
        let p = transform.position;

        // view = transpose([r | u | f]) with translation = -(view_3x3) * p
        float4x4::new(
            float4::new(r.x(), u.x(), f.x(), 0.),
            float4::new(r.y(), u.y(), f.y(), 0.),
            float4::new(r.z(), u.z(), f.z(), 0.),
            float4::new(
                -math::dot(&r, &p),
                -math::dot(&u, &p),
                -math::dot(&f, &p),
                1.,
            ),
        )
    }

    pub fn get_projection_matrix(&self) -> float4x4 {
        let y = 1. / math::tan(self.fov * math::TO_RADIUS * 0.5);
        let x = y / self.aspect;
        let l = self.far - self.near;
        let a = self.far / l;
        let b = -self.near * self.far / l;

        float4x4::new(
            float4::new(x, 0., 0., 0.),
            float4::new(0., y, 0., 0.),
            float4::new(0., 0., a, 1.),
            float4::new(0., 0., b, 0.),
        )
    }

    #[inline(always)]
    pub fn get_view_projection_matrix(&self, transform: Transform) -> float4x4 {
        self.get_projection_matrix() * self.get_view_matrix(transform)
    }
}
