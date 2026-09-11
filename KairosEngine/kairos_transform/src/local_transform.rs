use kairos_ecs::component::Component;
#[cfg(debug_assertions)]
use kairos_math::Vector;
use kairos_math::{affine, float3, float4x4, normalize, quaternion};
use serde::{Deserialize, Serialize};

/// An entity's own transform, relative to its parent — the kairos shape of
/// bevy's `Transform`, renamed `LocalTransform` to sit alongside its
/// world-space counterpart [`GlobalTransform`](crate::GlobalTransform).
///
/// All three fields are public:
///
/// - [`position`](Self::position) — translation in parent space (world space
///   for root-level entities);
/// - [`rotation`](Self::rotation) — orientation in parent space;
/// - [`scale`](Self::scale) — scale applied in the entity's own (local) axes,
///   before rotation.
///
/// # Semantics
///
/// [`compute_local_matrix`](Self::compute_local_matrix) yields the
/// *local → parent* matrix: it contains **no parent** (hierarchy) information,
/// so for an entity without a parent it *is* the world matrix. The
/// local → world matrix — with ancestors folded in — is the propagated
/// [`GlobalTransform`](crate::GlobalTransform) of a later system.
///
/// Coordinate system: right-handed, Y-up, -Z forward (the engine's spatial
/// convention). The default transform is the identity transform.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct LocalTransform {
    /// Translation relative to the parent (world space at the root).
    pub translation: float3,
    /// Orientation relative to the parent (world space at the root).
    pub rotation: quaternion,
    /// Scale in the entity's own axes.
    pub scale: float3,
}

impl LocalTransform {
    /// Composes a transform from its translation, orientation and scale.
    #[inline(always)]
    pub fn new(translation: float3, rotation: quaternion, scale: float3) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }

    /// Returns a root-level transform placed at `eye` and oriented to look
    /// toward `target`.
    ///
    /// `eye` and `target` are **world-space** coordinates, so this builds a
    /// world-space (parent-less) transform — use it for root-level entities
    /// only; with a parent, the world orientation would need converting into
    /// parent space first.
    ///
    /// The rotation is built so the entity's local -Z axis (its forward, see
    /// the coordinate-system notes on [`LocalTransform`]) points from `eye`
    /// toward `target`; `up` selects the remaining roll degree of freedom.
    /// [`scale`](Self::scale) is reset to [`float3::ONE`]: looking at
    /// something is a rotation plus placement.
    ///
    /// Mirrors the legacy engine `Transform::look_at`, which this replaces.
    pub fn look_at(eye: float3, target: float3, up: float3) -> Self {
        let forward = normalize(target - eye);
        let rotation = quaternion::from_look(forward, up);
        Self {
            translation: eye,
            rotation,
            scale: float3::ONE,
        }
    }

    /// Returns this transform as a 4x4 matrix: the *local → parent* matrix.
    ///
    /// Contains no parent — for an entity without a parent this *is* the world
    /// matrix. (Replaces the legacy `Transform::get_local_to_world`; once a
    /// scene hierarchy exists, ancestors must be folded in separately to reach
    /// the world.)
    #[inline(always)]
    pub fn compute_matrix(&self) -> float4x4 {
        float4x4::trs(self.translation, self.rotation, self.scale)
    }

    /// Returns the 3d affine transformation matrix from this transforms translation,
    /// rotation, and scale.
    #[inline]
    pub fn compute_affine(&self) -> affine {
        affine::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    #[inline]
    pub fn right(&self) -> float3 {
        self.rotation * float3::RIGHT
    }

    #[inline]
    pub fn left(&self) -> float3 {
        -self.right()
    }

    #[inline]
    pub fn up(&self) -> float3 {
        self.rotation * float3::UP
    }

    #[inline]
    pub fn down(&self) -> float3 {
        -self.up()
    }

    #[inline]
    pub fn forward(&self) -> float3 {
        self.rotation * float3::FORWARD
    }

    #[inline]
    pub fn back(&self) -> float3 {
        -self.forward()
    }

    /// Rotates this [`LocalTransform`] by the given rotation.
    ///
    /// If this [`LocalTransform`] has a parent, the `rotation` is relative to the rotation of the parent.
    #[inline]
    pub fn rotate(&mut self, rotation: quaternion) {
        self.rotation = rotation * self.rotation
    }

    /// Rotates this [`LocalTransform`] around the given `axis` by `angle` (in radians).
    ///
    /// If this [`LocalTransform`] has a parent, the `axis` is relative to the rotation of the parent.
    ///
    /// # Warning
    ///
    /// If you pass in an `axis` based on the current rotation (e.g. obtained via [`Transform::local_x`]),
    /// floating point errors can accumulate exponentially when applying rotations repeatedly this way. This will
    /// result in a denormalized rotation. In this case, it is recommended to normalize the [`Transform::rotation`] after
    /// each call to this method.
    #[inline]
    pub fn rotate_axis(&mut self, axis: float3, angle: f32) {
        #[cfg(debug_assertions)]
        assert_is_normalized(
            "The axis given to `Transform::rotate_axis` is not normalized. This may be a result of obtaining \
            the axis from the transform. See the documentation of `Transform::rotate_axis` for more details.",
            axis.len_sq(),
        );
        self.rotate(quaternion::from_axis_angle(axis, angle));
    }

    /// Rotates this [`LocalTransform`] around the `X` axis by `angle` (in radians).
    ///
    /// If this [`LocalTransform`] has a parent, the axis is relative to the rotation of the parent.
    #[inline]
    pub fn rotate_x(&mut self, angle: f32) {
        self.rotate(quaternion::from_rotation_x(angle));
    }

    /// Rotates this [`LocalTransform`] around the `Y` axis by `angle` (in radians).
    ///
    /// If this [`LocalTransform`] has a parent, the axis is relative to the rotation of the parent.
    #[inline]
    pub fn roate_y(&mut self, angle: f32) {
        self.rotate(quaternion::from_rotation_y(angle));
    }

    /// Rotates this [`LocalTransform`] around the `Z` axis by `angle` (in radians).
    ///
    /// If this [`LocalTransform`] has a parent, the axis is relative to the rotation of the parent.
    #[inline]
    pub fn rotate_z(&mut self, angle: f32) {
        self.rotate(quaternion::from_rotation_z(angle));
    }

    /// Rotates this [`LocalTransform`] by the given `rotation`.
    ///
    /// The `rotation` is relative to this [`LocalTransform`]'s current rotation.
    #[inline]
    pub fn rotate_local(&mut self, rotation: quaternion) {
        self.rotation *= rotation
    }

    /// Rotates this [`LocalTransform`] around its local `axis` by `angle` (in radians).
    ///
    /// # Warning
    ///
    /// If you pass in an `axis` based on the current rotation (e.g. obtained via [`LocalTransform::right`]),
    /// floating point errors can accumulate exponentially when applying rotations repeatedly this way. This will
    /// result in a denormalized rotation. In this case, it is recommended to normalize the [`LocalTransform::rotation`] after
    /// each call to this method.
    #[inline]
    pub fn rotate_local_axis(&mut self, axis: float3, angle: f32) {
        #[cfg(debug_assertions)]
        assert_is_normalized(
            "The axis given to `Transform::rotate_axis_local` is not normalized. This may be a result of obtaining \
            the axis from the transform. See the documentation of `Transform::rotate_axis_local` for more details.",
            axis.len_sq(),
        );
        self.rotate_local(quaternion::from_axis_angle(axis.into(), angle));
    }
}

impl Default for LocalTransform {
    /// The identity transform: zero translation, identity rotation, unit
    /// scale.
    #[inline(always)]
    fn default() -> Self {
        Self::new(float3::ZERO, quaternion::IDENTITY, float3::ONE)
    }
}

/// Checks that a vector with the given squared length is normalized.
///
/// Warns for small error with a length threshold of approximately `1e-4`,
/// and panics for large error with a length threshold of approximately `1e-2`.
#[cfg(debug_assertions)]
fn assert_is_normalized(message: &str, length_squared: f32) {
    use kairos_math::abs;

    let length_error_squared = abs(length_squared - 1.0);

    if length_error_squared > 2e-2 || length_error_squared.is_nan()  {
        panic!("Error: {message}",);
    } else if length_error_squared > 2e-4 {
        // Length error is approximately 1e-4 or more.
        eprintln!("Warning: {message}",)
    }
}

#[cfg(test)]
mod tests;
