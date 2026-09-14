use std::ops::Mul;

use kairos_ecs::component::Component;
#[cfg(debug_assertions)]
use kairos_math::Vector;
use kairos_math::{Dir3, affine, float3, float4x4, quaternion};
use serde::{Deserialize, Serialize};

use crate::GlobalTransform;

#[cfg(test)]
mod tests;

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
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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
    pub fn right(&self) -> Dir3 {
        Dir3::new_unchecked(self.rotation * float3::RIGHT)
    }

    #[inline]
    pub fn left(&self) -> Dir3 {
        -self.right()
    }

    #[inline]
    pub fn up(&self) -> Dir3 {
        Dir3::new_unchecked(self.rotation * float3::UP)
    }

    #[inline]
    pub fn down(&self) -> Dir3 {
        -self.up()
    }

    #[inline]
    pub fn forward(&self) -> Dir3 {
        Dir3::new_unchecked(self.rotation * float3::FORWARD)
    }

    #[inline]
    pub fn back(&self) -> Dir3 {
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
    pub fn rotate_axis(&mut self, axis: Dir3, angle: f32) {
        #[cfg(debug_assertions)]
        assert_is_normalized(
            "The axis given to `Transform::rotate_axis` is not normalized. This may be a result of obtaining \
            the axis from the transform. See the documentation of `Transform::rotate_axis` for more details.",
            axis.len_sq(),
        );
        self.rotate(quaternion::from_axis_angle(axis.into(), angle));
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
    pub fn rotate_local_axis(&mut self, axis: Dir3, angle: f32) {
        #[cfg(debug_assertions)]
        assert_is_normalized(
            "The axis given to `Transform::rotate_axis_local` is not normalized. This may be a result of obtaining \
            the axis from the transform. See the documentation of `Transform::rotate_axis_local` for more details.",
            axis.len_sq(),
        );
        self.rotate_local(quaternion::from_axis_angle(axis.into(), angle));
    }

    /// Rotates this [`LocalTransform`] around its local `X` axis by `angle` (in radians).
    #[inline]
    pub fn rotate_local_x(&mut self, angle: f32) {
        self.rotate_local(quaternion::from_rotation_x(angle));
    }

    /// Rotates this [`LocalTransform`] around its local `Y` axis by `angle` (in radians).
    #[inline]
    pub fn rotate_local_y(&mut self, angle: f32) {
        self.rotate_local(quaternion::from_rotation_y(angle));
    }

    /// Rotates this [`LocalTransform`] around its local `Z` axis by `angle` (in radians).
    #[inline]
    pub fn rotate_local_z(&mut self, angle: f32) {
        self.rotate_local(quaternion::from_rotation_z(angle));
    }

    /// Translates this [`LocalTransform`] around a `point` in space.
    ///
    /// If this [`LocalTransform`] has a parent, the `point` is relative to the [`LocalTransform`] of the parent.
    #[inline]
    pub fn translate_around(&mut self, point: float3, rotation: quaternion) {
        self.translation = point + rotation * (self.translation - point);
    }

    /// Rotates this [`LocalTransform`] around a `point` in space.
    ///
    /// If this [`LocalTransform`] has a parent, the `point` is relative to the [`LocalTransform`] of the parent.
    #[inline]
    pub fn rotate_around(&mut self, point: float3, rotation: quaternion) {
        self.translate_around(point, rotation);
        self.rotate(rotation);
    }

    /// Rotates this [`LocalTransform`] so that [`LocalTransform::forward`] points towards the `target` position,
    /// and [`LocalTransform::up`] points towards `up`.
    ///
    /// In some cases it's not possible to construct a rotation. Another axis will be picked in those cases:
    /// * if `target` is the same as the transform translation, `float3::FORWARD` is used instead
    /// * if `up` fails converting to `Dir3` (e.g if it is `float3::ZERO`), `Dir3::UP` is used instead
    /// * if the resulting forward direction is parallel with `up`, an orthogonal vector is used as the "right" direction
    pub fn look_at(&mut self, target: float3, up: impl TryInto<Dir3>) {
        let rotation = quaternion::from_look_to(target - self.translation, up);
        self.rotation = rotation;
    }

    /// Rotates this [`LocalTransform`] so that [`LocalTransform::forward`] points in the given `direction`
    /// and [`LocalTransform::up`] points towards `up`.
    ///
    /// In some cases it's not possible to construct a rotation. Another axis will be picked in those cases:
    /// * if `direction` fails converting to `Dir3` (e.g if it is `float3::ZERO`), `Dir3::BACK` is used instead
    /// * if `up` fails converting to `Dir3`, `Dir3::UP` is used instead
    /// * if `direction` is parallel with `up`, an orthogonal vector is used as the "right" direction
    #[inline]
    pub fn look_to(&mut self, direction: impl TryInto<Dir3>, up: impl TryInto<Dir3>) {
        let rotation = quaternion::from_look_to(direction, up);
        self.rotation = rotation
    }

    /// Multiplies `self` with `transform` component by component, returning the
    /// resulting [`LocalTransform`]
    #[inline]
    #[must_use]
    pub fn mul_transform(&self, transform: LocalTransform) -> Self {
        let translation = self.transform_point(transform.translation);
        let rotation = self.rotation * transform.rotation;
        let scale = self.scale * transform.scale;
        LocalTransform {
            translation,
            rotation,
            scale,
        }
    }

    /// Transforms the given `point`, applying scale, rotation and translation.
    ///
    /// If this [`LocalTransform`] has an ancestor entity with a [`LocalTransform`] component,
    /// [`LocalTransform::transform_point`] will transform a point in local space into its
    /// parent transform's space.
    ///
    /// If this [`LocalTransform`] does not have a parent, [`LocalTransform::transform_point`] will
    /// transform a point in local space into worldspace coordinates.
    ///
    /// If you always want to transform a point in local space to worldspace, or if you need
    /// the inverse transformations, see [`GlobalTransform::transform_point()`].
    #[inline]
    pub fn transform_point(&self, mut point: float3) -> float3 {
        point = self.scale * point;
        point = self.rotation * point;
        point += self.translation;
        point
    }

    /// Rotates this [`LocalTransform`] so that the `main_axis` vector, reinterpreted in local coordinates, points
    /// in the given `main_direction`, while `secondary_axis` points towards `secondary_direction`.
    ///
    /// For example, if a spaceship model has its nose pointing in the X-direction in its own local coordinates
    /// and its dorsal fin pointing in the Y-direction, then `align(Dir3::X, v, Dir3::Y, w)` will make the spaceship's
    /// nose point in the direction of `v`, while the dorsal fin does its best to point in the direction `w`.
    ///
    /// More precisely, the [`LocalTransform::rotation`] produced will be such that:
    /// * applying it to `main_axis` results in `main_direction`
    /// * applying it to `secondary_axis` produces a vector that lies in the half-plane generated by `main_direction` and
    ///   `secondary_direction` (with positive contribution by `secondary_direction`)
    ///
    /// [`LocalTransform::look_to`] is recovered, for instance, when `main_axis` is `Dir3::BACK` (the [`LocalTransform::forward`]
    /// direction in the default orientation) and `secondary_axis` is `Dir3::UP` (the [`LocalTransform::up`] direction in the default
    /// orientation). (Failure cases may differ somewhat.)
    ///
    /// In some cases a rotation cannot be constructed. Another axis will be picked in those cases:
    /// * if `main_axis` or `main_direction` fail converting to `Dir3` (e.g are zero), `Dir3::RIGHT` takes their place
    /// * if `secondary_axis` or `secondary_direction` fail converting, `Dir3::UP` takes their place
    /// * if `main_axis` is parallel with `secondary_axis` or `main_direction` is parallel with `secondary_direction`,
    ///   a rotation is constructed which takes `main_axis` to `main_direction` along a great circle, ignoring the secondary
    ///   counterparts
    ///
    /// Example
    /// ```
    /// # use kairos_math::{Dir3, float3, quaternion};
    /// # use kairos_transform::LocalTransform;
    /// # let mut t1 = LocalTransform::IDENTITY;
    /// # let mut t2 = LocalTransform::IDENTITY;
    /// t1.align(Dir3::RIGHT, Dir3::UP, float3::new(1., 1., 0.), Dir3::FORWARD);
    /// let main_axis_image = t1.rotation * Dir3::RIGHT;
    /// let secondary_axis_image = t1.rotation * float3::new(1., 1., 0.);
    /// assert!(main_axis_image.abs_diff_eq(float3::UP, 1e-5));
    /// assert!(secondary_axis_image.abs_diff_eq(float3::new(0., 1., 1.), 1e-5));
    ///
    /// t1.align(float3::ZERO, Dir3::FORWARD, float3::ZERO, Dir3::RIGHT);
    /// t2.align(Dir3::RIGHT, Dir3::FORWARD, Dir3::UP, Dir3::RIGHT);
    /// assert_eq!(t1.rotation, t2.rotation);
    ///
    /// t1.align(Dir3::RIGHT, Dir3::FOWARD, Dir3::RIGHT, Dir3::UP);
    /// assert_eq!(t1.rotation, quaternion::from_rotation_arc(float3::RIGHT, float3::FORWARD));
    /// ```
    #[inline]
    pub fn align(
        &mut self,
        main_axis: impl TryInto<Dir3>,
        main_direction: impl TryInto<Dir3>,
        secondary_axis: impl TryInto<Dir3>,
        secondary_direction: impl TryInto<Dir3>,
    ) {
        let main_axis = main_axis.try_into().unwrap_or(Dir3::RIGHT);
        let main_direction = main_direction.try_into().unwrap_or(Dir3::RIGHT);
        let secondary_axis = secondary_axis.try_into().unwrap_or(Dir3::UP);
        let secondary_direction = secondary_direction.try_into().unwrap_or(Dir3::UP);

        // The solution quaternion will be constructed in two steps.
        // First, we start with a rotation that takes `main_axis` to `main_direction`.
        let first_rotation = quaternion::from_rotation_arc(main_axis.into(), main_direction.into());

        // Let's follow by rotating about the `main_direction` axis so that the image of `secondary_axis`
        // is taken to something that lies in the plane of `main_direction` and `secondary_direction`. Since
        // `main_direction` is fixed by this rotation, the first criterion is still satisfied.
        let secondary_image = first_rotation * secondary_axis;
        let secondary_image_ortho = secondary_image
            .reject_from_normalized(main_direction.into())
            .try_normalized();
        let secondary_direction_ortho = secondary_direction
            .reject_from_normalized(main_direction.into())
            .try_normalized();

        // If one of the two weak vectors was parallel to `main_direction`, then we just do the first part
        self.rotation = match (secondary_image_ortho, secondary_direction_ortho) {
            (Some(secondary_img_ortho), Some(secondary_dir_ortho)) => {
                let second_rotation = quaternion::from_rotation_arc(secondary_img_ortho, secondary_dir_ortho);
                second_rotation * first_rotation
            },
            _ => first_rotation
        }
    }

    /// Rotates this [`LocalTransform`] so that the `main_axis` vector, reinterpreted in local coordinates, points
    /// in the given `main_direction`, while `secondary_axis` points towards `secondary_direction`.
    /// For example, if a spaceship model has its nose pointing in the X-direction in its own local coordinates
    /// and its dorsal fin pointing in the Y-direction, then `align(Dir3::RIGHT, v, Dir3::UP, w)` will make the spaceship's
    /// nose point in the direction of `v`, while the dorsal fin does its best to point in the direction `w`.
    ///
    ///
    /// In some cases a rotation cannot be constructed. Another axis will be picked in those cases:
    /// * if `main_axis` or `main_direction` fail converting to `Dir3` (e.g are zero), `Dir3::RIGHT` takes their place
    /// * if `secondary_axis` or `secondary_direction` fail converting, `Dir3::UP` takes their place
    /// * if `main_axis` is parallel with `secondary_axis` or `main_direction` is parallel with `secondary_direction`,
    ///   a rotation is constructed which takes `main_axis` to `main_direction` along a great circle, ignoring the secondary
    ///   counterparts
    ///
    /// See [`Transform::align`] for additional details.
    #[inline]
    #[must_use]
    pub fn aligned_by(
        mut self,
        main_axis: impl TryInto<Dir3>,
        main_direction: impl TryInto<Dir3>,
        secondary_axis: impl TryInto<Dir3>,
        secondary_direction: impl TryInto<Dir3>,
    ) -> Self {
        self.align(main_axis, main_direction, secondary_axis, secondary_direction);
        self
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

impl From<GlobalTransform> for LocalTransform {
    fn from(transform: GlobalTransform) -> Self {
        transform.to_local_transform()
    }
}

impl Mul<LocalTransform> for LocalTransform {
    type Output = Self;

    fn mul(self, transform: LocalTransform) -> Self::Output {
        self.mul_transform(transform)
    }
}

impl Mul<GlobalTransform> for LocalTransform {
    type Output = GlobalTransform;

    #[inline]
    fn mul(self, global_transform: GlobalTransform) -> Self::Output {
        GlobalTransform::from(self) * global_transform
    }
}

impl Mul<float3> for LocalTransform {
    type Output = float3;

    fn mul(self, rhs: float3) -> Self::Output {
        self.transform_point(rhs)
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

    if length_error_squared > 2e-2 || length_error_squared.is_nan() {
        panic!("Error: {message}",);
    } else if length_error_squared > 2e-4 {
        // Length error is approximately 1e-4 or more.
        eprintln!("Warning: {message}",)
    }
}

/// An optimization for transform propagation. This ZST marker component uses change detection to
/// mark all entities of the hierarchy as "dirty" if any of their descendants have a changed
/// `Transform`. If this component is *not* marked `is_changed()`, propagation will halt.
#[derive(Component, Clone, Copy, Default, PartialEq, Debug)]
pub struct TransformTreeChanged;
