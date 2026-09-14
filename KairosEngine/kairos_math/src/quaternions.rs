use std::ops::{Mul, MulAssign};

use glam::{EulerRot, Mat3A, Quat};
use serde::{Deserialize, Serialize};

#[cfg(debug_assertions)]
use crate::{Vector, direction};
use crate::{Dir3, float3, float4x4};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub struct quaternion(pub Quat);

impl quaternion {
    pub const IDENTITY: quaternion = quaternion::identity();

    #[inline(always)]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self(Quat::from_xyzw(x, y, z, w))
    }

    #[inline(always)]
    pub const fn identity() -> Self {
        Self(Quat::IDENTITY)
    }

    /// Builds a rotation from intrinsic roll (`x`), pitch (`y`) and yaw (`z`)
    /// angles, delegating to [`glam::Quat::from_euler`].
    ///
    /// The angles are applied about X, then Y, then Z in the rotating frame,
    /// which is glam's intrinsic `ZYX` order. glam names the arguments by axis
    /// sequence, so the components are passed in reverse (`z`, `y`, `x`).
    #[inline(always)]
    pub fn from_euler(euler: float3) -> Self {
        Self(Quat::from_euler(
            EulerRot::ZYX,
            euler.z(),
            euler.y(),
            euler.x(),
        ))
    }

    /// Rotates this [`Transform`] around the `X` axis by `angle` (in radians).
    ///
    /// If this [`Transform`] has a parent, the axis is relative to the rotation of the parent.
    #[inline(always)]
    pub fn from_axis_angle(axis: float3, angle: f32) -> Self {
        Self(Quat::from_axis_angle(axis.0.to_vec3(), angle))
    }

    /// Creates a quaternion from the `angle` (in radians) around the x axis.
    #[inline(always)]
    #[must_use]
    pub fn from_rotation_x(angle: f32) -> Self {
        Self(Quat::from_rotation_x(angle))
    }

    /// Creates a quaternion from the `angle` (in radians) around the y axis.
    #[inline(always)]
    #[must_use]
    pub fn from_rotation_y(angle: f32) -> Self {
        Self(Quat::from_rotation_y(angle))
    }

    /// Creates a quaternion from the `angle` (in radians) around the z axis.
    #[inline(always)]
    #[must_use]
    pub fn from_rotation_z(angle: f32) -> Self {
        Self(Quat::from_rotation_z(angle))
    }

    #[inline]
    pub fn from_look_to(direction: impl TryInto<Dir3>, up: impl TryInto<Dir3>) -> Self {
        let back = -direction.try_into().unwrap_or(Dir3::BACK);
        let up = up.try_into().unwrap_or(Dir3::UP);
        let right = up.cross(back.into()).try_normalized().unwrap_or_else(|| up.any_orthonormal_vector());
        let up = back.cross(right);
        Self(Quat::from_mat3a(&Mat3A::from_cols(right.0, up.0, back.0.0)))
    }

    /// Decomposes the rotation into intrinsic roll (`x`), pitch (`y`) and yaw
    /// (`z`) angles, delegating to [`glam::Quat::to_euler`].
    ///
    /// Inverse of [`quaternion::from_euler`]: glam's intrinsic `ZYX` order
    /// returns the angles as `(z, y, x)`, which is reordered here.
    #[inline(always)]
    pub fn to_euler(self) -> float3 {
        let (z, y, x) = self.0.normalize().to_euler(EulerRot::ZYX);
        float3::new(x, y, z)
    }

    #[inline(always)]
    pub fn normalized(self) -> Self {
        Self(self.0.normalize())
    }

    #[inline(always)]
    pub fn normalize(&self) -> Self {
        (*self).normalized()
    }

    #[inline(always)]
    pub fn to_float4x4(self) -> float4x4 {
        // Converge on glam's quat->matrix expansion so every rotation-matrix
        // path (camera views, `float4x4::trs` via `affine`) shares one
        // implementation.
        float4x4(glam::Mat4::from_quat(self.0.normalize()))
    }

    /// Gets the minimal rotation for transforming `from` to `to`.  The rotation is in the
    /// plane spanned by the two vectors.  Will rotate at most 180 degrees.
    ///
    /// The inputs must be unit vectors.
    ///
    /// `from_rotation_arc(from, to) * from ≈ to`.
    ///
    /// For near-singular cases (from≈to and from≈-to) the current implementation
    /// is only accurate to about 0.001 (for `f32`).
    ///
    /// # Panics
    ///
    /// Will panic if `from` or `to` are not normalized when `glam_assert` is enabled.
    #[inline(always)]
    pub fn from_rotation_arc(from: float3, to: float3) -> quaternion {
        Self(Quat::from_rotation_arc(from.0.to_vec3(), to.0.to_vec3()))
    }
}

impl Mul for quaternion {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl Mul<float3> for quaternion {
    type Output = float3;

    #[inline(always)]
    fn mul(self, rhs: float3) -> Self::Output {
        float3::from_inner(self.0 * rhs.0)
    }
}

impl MulAssign for quaternion {
    #[inline(always)]
    fn mul_assign(&mut self, rhs: Self) {
        self.0 = self.0 * rhs.0
    }
}

impl Mul<Dir3> for quaternion {
    type Output = Dir3;

    /// Rotates the [`Dir3`] using a [`quaternion`].
    fn mul(self, rhs: Dir3) -> Self::Output {
        let rotated = self * *rhs;
        #[cfg(debug_assertions)]
        direction::assert_is_normalized(
            "`Dir3` is denormalized after rotation.",
            rotated.len_sq(),
        );

        Dir3(rotated)
    }
}

unsafe impl bytemuck::Zeroable for quaternion {}
unsafe impl bytemuck::Pod for quaternion {}

impl Serialize for quaternion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.to_array().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for quaternion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let arry = <[f32; 4]>::deserialize(deserializer)?;
        Ok(Self::new(arry[0], arry[1], arry[2], arry[3]))
    }
}
