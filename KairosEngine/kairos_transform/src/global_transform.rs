use derive_more::From;
use kairos_ecs::component::Component;
use kairos_math::{affine, float3, float4x4, quaternion};

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
}

impl Default for GlobalTransform {
    /// The identity transform.
    #[inline(always)]
    fn default() -> Self {
        Self(affine::IDENTITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Column-major arrays; compare every element with `eps` tolerance
    /// (decompositions are not bit-exact).
    fn assert_matrix_approx_eq(a: &float4x4, b: &float4x4, eps: f32) {
        let a = a.to_array();
        let b = b.to_array();
        for (row, a_row) in a.iter().enumerate() {
            for (col, a_el) in a_row.iter().enumerate() {
                assert!(
                    (a_el - b[row][col]).abs() <= eps,
                    "matrix mismatch at [{row}][{col}]: {} vs {}",
                    a_el,
                    b[row][col]
                );
            }
        }
    }

    #[test]
    fn default_is_identity() {
        let g = GlobalTransform::default();
        // Default == `affine::IDENTITY` semantics.
        assert_eq!(g.to_float4x4(), affine::IDENTITY.to_float4x4());
        assert_eq!(g.to_float4x4(), float4x4::IDENTITY);
        assert_eq!(g.translation(), float3::ZERO);
        // Rotation compare is sign-agnostic (`q` and `-q` are the same
        // rotation), so go through matrices.
        assert_matrix_approx_eq(&g.rotation().to_float4x4(), &quaternion::IDENTITY.to_float4x4(), 1e-6);
    }

    #[test]
    fn accessors_reflect_the_stored_affine() {
        let position = float3::new(1.0, 2.0, 3.0);
        let rotation = quaternion::from_euler(float3::new(0.0, 0.6, 0.0));
        let scale = float3::new(2.0, 2.0, 2.0);
        let inner = affine::trs(position, rotation, scale);
        let g = GlobalTransform(inner);

        assert_eq!(g.translation(), position);
        assert_matrix_approx_eq(&g.rotation().to_float4x4(), &rotation.to_float4x4(), 1e-4);
        assert_eq!(g.to_float4x4(), inner.to_float4x4());
    }

    #[test]
    fn transform_point_maps_points_into_world_space() {
        // Pure translation.
        let g = GlobalTransform(affine::trs(
            float3::new(10.0, 0.0, 0.0),
            quaternion::IDENTITY,
            float3::ONE,
        ));
        assert_eq!(
            g.transform_point(float3::new(1.0, 2.0, 3.0)),
            float3::new(11.0, 2.0, 3.0)
        );

        // Translation + uniform scale, identity rotation: p -> scale * p + t.
        let g = GlobalTransform(affine::trs(
            float3::new(1.0, 1.0, 1.0),
            quaternion::IDENTITY,
            float3::new(2.0, 2.0, 2.0),
        ));
        assert_eq!(
            g.transform_point(float3::new(1.0, 0.0, 0.0)),
            float3::new(3.0, 1.0, 1.0)
        );
    }
}
