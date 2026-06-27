use crate::math::quaternion;

impl From<quaternion> for mint::Quaternion<f32> {
    fn from(rot: quaternion) -> Self {
        mint::Quaternion {
            v: rot.0.xyz().into(),
            s: rot.0.w(),
        }
    }
}

impl From<rapier3d::math::Rotation> for quaternion {
    fn from(value: rapier3d::math::Rotation) -> Self {
        Self::new(value.x, value.y, value.z, value.w)
    }
}
