use std::ops::Mul;

use derive_more::From;
use kairos_ecs::component::Component;
use kairos_math::{Vector, affine, copysign, float3, float4x4, quaternion};

use crate::LocalTransform;

/// An entity's absolute (world-space) transform.
///
/// Stored as a [`kairos_math::affine`] (rotation + scale + translation, no
/// perspective) — the world-space counterpart of
/// [`LocalTransform`](crate::LocalTransform), which the transform-propagation
/// system will later maintain down the scene hierarchy.
///
/// # Do not read (yet)
///
/// The propagation system that builds this component from the
/// [`LocalTransform`](crate::LocalTransform) hierarchy has **not** landed:
///
/// - do not spawn entities with a [`GlobalTransform`], and
/// - do not read it at consumption sites.
///
/// While every scene is a root scene (`local == world`), read
/// [`LocalTransform`](crate::LocalTransform) directly. The identity
/// [`Default`] here only makes the type spawnable and the API ready for the
/// propagation system; there is deliberately no maintenance system for it yet.
///
/// Coordinate system: right-handed, Y-up, -Z forward (the engine's spatial
/// convention).
#[derive(Component, Debug, Clone, Copy, PartialEq, From)]
pub struct GlobalTransform(affine);

impl GlobalTransform {
    /// An identity [`GlobalTransform`] that maps all points in space to themselves.
    pub const IDENTITY: Self = Self(affine::IDENTITY);

    /// The translation part of the transform — the world-space position.
    #[inline(always)]
    pub fn translation(&self) -> float3 {
        self.0.translation()
    }

    /// The rotation part of the transform, decomposed from the stored affine
    /// (see [`affine::to_scale_rotation_translation`]).
    #[inline(always)]
    pub fn rotation(&self) -> quaternion {
        self.0.to_scale_rotation_translation().1
    }

    #[inline]
    pub fn scale(&self) -> float3 {
        let det = self.0.determinant();
        float3::new(
            self.0.x_axis_length() * copysign(1., det),
            self.0.y_axis_length(),
            self.0.z_axis_length()
        )
    }

    /// The equivalent column-major 4x4 matrix.
    #[inline(always)]
    pub fn to_float4x4(&self) -> float4x4 {
        self.0.to_float4x4()
    }

    /// Transforms `point` (a world-space position) into world space under this
    /// transform.
    #[inline(always)]
    pub fn transform_point(&self, point: float3) -> float3 {
        self.0.transform_point(point)
    }

    /// Multiplies `self` with `transform` component by component, returning the
    /// resulting [`GlobalTransform`]
    #[inline]
    pub fn mul_local_transform(&self, local_transform: LocalTransform) -> Self {
        Self(self.0 * local_transform.compute_affine())
    }

    #[inline]
    pub fn right(&self) -> float3 {
        float3::normalize(self.0 * float3::RIGHT)
    }

    #[inline]
    pub fn left(&self) -> float3 {
        -self.right()
    }

    #[inline]
    pub fn up(&self) -> float3 {
        float3::normalize(self.0 * float3::UP)
    }

    #[inline]
    pub fn down(&self) -> float3 {
        -self.up()
    }

    #[inline]
    pub fn forward(&self) -> float3 {
        float3::normalize(self.0 * float3::FORWARD)
    }

    #[inline]
    pub fn back(&self) -> float3 {
        -self.forward()
    }

    #[inline]
    pub fn from_translation(translation: float3) -> Self {
        Self(affine::from_translation(translation))
    }

    #[inline]
    pub fn from_rotation(rotation: quaternion) -> Self {
        Self(affine::from_rotation_translation(rotation, float3::ONE))
    }

    #[inline]
    pub fn from_scale(scale: float3) -> Self {
        Self(affine::from_scale(scale))
    }

    /// Returns the affine transformation matrix as an [`affine`].
    #[inline]
    pub fn affine(&self) -> affine {
        self.0
    }

    /// Returns the transformation as a [`LocalTransform`].
    ///
    /// The local transform is expected to be non-degenerate and without shearing, or the output
    /// will be invalid.
    #[inline]
    pub fn to_local_transform(&self) -> LocalTransform {
        let (scale, rotation, translation) = self.0.to_scale_rotation_translation();
        LocalTransform {
            translation,
            rotation,
            scale,
        }
    }

    /// Returns the [`Transform`] `self` would have if it was a child of an entity
    /// with the `parent` [`GlobalTransform`].
    ///
    /// This is useful if you want to "reparent" an [`Entity`](bevy_ecs::entity::Entity).
    /// Say you have an entity `e1` that you want to turn into a child of `e2`,
    /// but you want `e1` to keep the same global transform, even after re-parenting. You would use:
    ///
    /// ```
    /// # use kairos_transform::{GlobalTransform, Transform};
    /// # use kairos_ecs::{Entity, Query, Component, Commands, ChildOf};
    /// #[derive(Component)]
    /// struct ToReparent {
    ///     new_parent: Entity,
    /// }
    /// fn reparent_system(
    ///     mut commands: Commands,
    ///     mut targets: Query<(&mut Transform, Entity, &GlobalTransform, &ToReparent)>,
    ///     transforms: Query<&GlobalTransform>,
    /// ) {
    ///     for (mut transform, entity, initial, to_reparent) in targets.iter_mut() {
    ///         if let Ok(parent_transform) = transforms.get(to_reparent.new_parent) {
    ///             *transform = initial.reparented_to(parent_transform);
    ///             commands.entity(entity)
    ///                 .remove::<ToReparent>()
    ///                 .insert(ChildOf(to_reparent.new_parent));
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// The transform is expected to be non-degenerate and without shearing, or the output
    /// will be invalid.
    #[inline]
    pub fn reparented_to(&self, new_parent: &GlobalTransform) -> LocalTransform {
        let relative_affine = new_parent.affine().inverse() * self.affine();
        let (scale, rotation, translation) = relative_affine.to_scale_rotation_translation();
        LocalTransform {
            translation,
            rotation,
            scale,
        }
    }

    /// Extracts `scale`, `rotation` and `translation` from `self`.
    ///
    /// The transform is expected to be non-degenerate and without shearing, or the output
    /// will be invalid.
    #[inline]
    pub fn to_scale_rotation_translation(&self) -> (float3, quaternion, float3) {
        self.0.to_scale_rotation_translation()
    }
}

impl Default for GlobalTransform {
    /// The identity transform.
    #[inline(always)]
    fn default() -> Self {
        Self(affine::IDENTITY)
    }
}

impl From<LocalTransform> for GlobalTransform {
    fn from(transform: LocalTransform) -> Self {
        Self(transform.compute_affine())
    }
}

impl Mul<Self> for GlobalTransform {
    type Output = Self;

    #[inline]
    fn mul(self, global_transform: Self) -> Self::Output {
        Self(self.0 * global_transform.0)
    }
}

impl Mul<LocalTransform> for GlobalTransform {
    type Output = Self;

    #[inline]
    fn mul(self, transform: LocalTransform) -> Self::Output {
        self.mul_local_transform(transform)
    }
}

impl Mul<float3> for GlobalTransform {
    type Output = float3;

    #[inline]
    fn mul(self, value: float3) -> Self::Output {
        self.transform_point(value)
    }
}

#[cfg(test)]
mod tests;
