use crate::math::quaternion;

impl From<quaternion> for mint::Quaternion<f32> {
    fn from(rot: quaternion) -> Self {
        mint::Quaternion {
            v: rot.0.xyz().into(),
            s: rot.0.w(),
        }
    }
}
