use crate::math::{self, float3, float4, float4x4};

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
    pub fn new(
        position: float3,
        forward: float3,
        right: float3,
        fov: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self {
            position,
            forward,
            right,
            fov,
            aspect,
            near,
            far,
        }
    }

    pub fn get_view_matrix(&self) -> float4x4 {
        let f = self.forward;
        let r = self.right;
        let u = math::cross(&f, &r);

        let v1 = float4::new(r[0], u[0], f[0], 0.);
        let v2 = float4::new(r[1], u[1], f[1], 0.);
        let v3 = float4::new(r[2], u[2], f[2], 0.);
        let v4 = float4::new(
            -math::dot(&r, &self.position),
            -math::dot(&u, &self.position),
            -math::dot(&f, &self.position),
            1.,
        );

        float4x4::new(v1, v2, v3, v4)
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
    pub fn get_view_projection_matrix(&self) -> float4x4 {
        self.get_projection_matrix() * self.get_view_matrix()
    }
}
