use kairos_math::{self as math, float3, float4, float4x4};
use kairos_ecs::component::Component;
use kairos_transform::LocalTransform;

/// Pure projection parameters; view matrix is derived from a `LocalTransform`.
///
/// Intrinsics only — the *view's* aspect ratio is not an intrinsic, so it is a
/// parameter of the projection methods and lives on [`CameraView`], which is
/// derived from the view's physical size each frame.
#[derive(Component)]
#[require(CameraView)]
pub struct Camera {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

/// What a camera derives from the size of the view it renders this frame —
/// the kairos shape of bevy's `Camera::computed`.
///
/// [`Camera`] requires it, so spawning a camera already yields a readable
/// value; the default reads as "no view size known yet" and stays that way
/// until the render extract stage starts filling it in. That stage is then the
/// only writer, and it resets `view_projection` to `None` before recomputing,
/// so a stale matrix never survives a frame.
#[derive(Component, Debug, Clone, Copy)]
pub struct CameraView {
    /// View size in physical pixels; `(0, 0)` = nothing to render this frame.
    pub physical_size: (u32, u32),
    /// width / height
    pub aspect: f32,
    /// `None` = no usable view this frame (the extract stage resets it before
    /// recomputing, so a stale matrix never survives a frame).
    pub view_projection: Option<float4x4>,
}

impl Default for CameraView {
    /// Fallback `aspect` is `1.0` rather than `0.0` so the projection math can
    /// never divide by zero before the first view size arrives.
    fn default() -> Self {
        Self {
            physical_size: (0, 0),
            aspect: 1.0,
            view_projection: None,
        }
    }
}

impl Camera {
    pub fn new(fov: f32, near: f32, far: f32) -> Self {
        Self { fov, near, far }
    }

    /// World→View matrix from the camera's root-level `LocalTransform`
    /// (world-space while every scene is a root scene).
    ///
    /// `view = inverse(camera_world)`, where `camera_world` is derived from
    /// `transform.rotation` (right = +X, up = +Y, forward = -Z) and
    /// `transform.position`.
    pub fn get_view_matrix(&self, transform: LocalTransform) -> float4x4 {
        // Columns of the rotation matrix R = [right | up | -forward]
        let m = transform.rotation.to_float4x4();
        let r = float3::new(m.c0().x(), m.c0().y(), m.c0().z()); // right
        let u = float3::new(m.c1().x(), m.c1().y(), m.c1().z()); // up
        let f = float3::new(m.c2().x(), m.c2().y(), m.c2().z()) * -1.0; // forward = -c2
        let p = transform.translation;

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

    /// Projection matrix for a view of the given `aspect` (`width / height`).
    pub fn get_projection_matrix(&self, aspect: f32) -> float4x4 {
        let y = 1. / math::tan(self.fov * math::TO_RADIUS * 0.5);
        let x = y / aspect;
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
    pub fn get_view_projection_matrix(
        &self,
        transform: LocalTransform,
        aspect: f32,
    ) -> float4x4 {
        self.get_projection_matrix(aspect) * self.get_view_matrix(transform)
    }
}

#[cfg(test)]
mod test;
