use kairos_ecs::component::Component;
use kairos_math::{float3, float4x4, normalize, quaternion};

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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LocalTransform {
    /// Translation relative to the parent (world space at the root).
    pub position: float3,
    /// Orientation relative to the parent (world space at the root).
    pub rotation: quaternion,
    /// Scale in the entity's own axes.
    pub scale: float3,
}

impl LocalTransform {
    /// Composes a transform from its translation, orientation and scale.
    #[inline(always)]
    pub fn new(position: float3, rotation: quaternion, scale: float3) -> Self {
        Self {
            position,
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
            position: eye,
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
    pub fn compute_local_matrix(&self) -> float4x4 {
        float4x4::trs(self.position, self.rotation, self.scale)
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

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_math::{dot, float4};

    #[test]
    fn default_is_identity() {
        let t = LocalTransform::default();
        assert_eq!(t.position, float3::ZERO);
        assert_eq!(t.rotation, quaternion::IDENTITY);
        assert_eq!(t.scale, float3::ONE);
        assert_eq!(
            t,
            LocalTransform::new(float3::ZERO, quaternion::IDENTITY, float3::ONE)
        );
    }

    #[test]
    fn compute_local_matrix_matches_float4x4_trs() {
        let t = LocalTransform::new(
            float3::new(1.0, -2.0, 3.0),
            quaternion::from_euler(float3::new(0.3, -1.1, 0.7)),
            float3::new(2.0, 0.5, 3.0),
        );
        // Delegation must stay in lock-step with `float4x4::trs`.
        assert_eq!(
            t.compute_local_matrix(),
            float4x4::trs(t.position, t.rotation, t.scale)
        );
    }

    #[test]
    fn compute_local_matrix_encodes_trs_columns() {
        // Identity rotation: the columns are the scaled basis axes and the
        // last column carries the translation (w = 1).
        let t = LocalTransform::new(
            float3::new(1.0, 2.0, 3.0),
            quaternion::IDENTITY,
            float3::new(2.0, 3.0, 4.0),
        );
        let m = t.compute_local_matrix();
        assert_eq!(m.c0(), float4::new(2.0, 0.0, 0.0, 0.0));
        assert_eq!(m.c1(), float4::new(0.0, 3.0, 0.0, 0.0));
        assert_eq!(m.c2(), float4::new(0.0, 0.0, 4.0, 0.0));
        assert_eq!(m.c3(), float4::new(1.0, 2.0, 3.0, 1.0));
    }

    /// The image of the local -Z axis under `rotation` — the transform's
    /// "forward" direction in world space.
    fn forward_of(rotation: quaternion) -> float3 {
        let m = rotation.to_float4x4();
        float3::ZERO - m.c2().xyz()
    }

    fn assert_looks_from_towards(t: LocalTransform, eye: float3, target: float3) {
        assert_eq!(t.position, eye, "look_at must place the transform at eye");
        assert_eq!(t.scale, float3::ONE, "look_at must reset scale to one");

        let forward = forward_of(t.rotation);
        let expected = normalize(target - eye);
        let d = dot(&forward, &expected);
        assert!(
            d > 1.0 - 1e-4,
            "forward {forward:?} is not aligned with the target direction {expected:?} (dot {d})"
        );
    }

    #[test]
    fn look_at_orients_forward_towards_target() {
        let eye = float3::new(1.0, 2.0, 3.0);
        for target in [
            float3::new(5.0, 2.0, 3.0),    // +X
            float3::new(1.0, 2.0, 8.0),    // +Z (behind the -Z forward axis)
            float3::new(2.0, 3.0, 4.0),    // diagonal (1,1,1)
            float3::new(-1.0, 2.0, -3.0),  // diagonal backwards
        ] {
            let t = LocalTransform::look_at(eye, target, float3::UP);
            assert_looks_from_towards(t, eye, target);
        }
    }

    #[test]
    fn look_at_handles_up_parallel_to_forward() {
        let eye = float3::ZERO;
        for target in [float3::new(0.0, 5.0, 0.0), float3::new(0.0, -5.0, 0.0)] {
            let t = LocalTransform::look_at(eye, target, float3::UP);
            assert_looks_from_towards(t, eye, target);
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip() {
        let t = LocalTransform::new(
            float3::new(1.0, 2.0, 3.0),
            quaternion::from_euler(float3::new(0.1, 0.2, 0.3)),
            float3::new(2.0, 3.0, 4.0),
        );
        let json = serde_json::to_string(&t).expect("serialize LocalTransform");
        let back: LocalTransform = serde_json::from_str(&json).expect("deserialize LocalTransform");
        assert_eq!(t, back);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_default() {
        let t = LocalTransform::default();
        let json = serde_json::to_string(&t).expect("serialize default LocalTransform");
        // Struct-shaped serialization with the three public fields.
        assert!(json.contains("\"position\""), "unexpected json: {json}");
        assert!(json.contains("\"rotation\""), "unexpected json: {json}");
        assert!(json.contains("\"scale\""), "unexpected json: {json}");
        let back: LocalTransform = serde_json::from_str(&json).expect("deserialize LocalTransform");
        assert_eq!(t, back);
    }
}
