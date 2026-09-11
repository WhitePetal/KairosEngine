use super::*;
use kairos_math::{dot, float4};

#[test]
fn default_is_identity() {
    let t = LocalTransform::default();
    assert_eq!(t.translation, float3::ZERO);
    assert_eq!(t.rotation, quaternion::IDENTITY);
    assert_eq!(t.scale, float3::ONE);
    assert_eq!(
        t,
        LocalTransform::new(float3::ZERO, quaternion::IDENTITY, float3::ONE)
    );
}

#[test]
fn compute_matrix_matches_float4x4_trs() {
    let t = LocalTransform::new(
        float3::new(1.0, -2.0, 3.0),
        quaternion::from_euler(float3::new(0.3, -1.1, 0.7)),
        float3::new(2.0, 0.5, 3.0),
    );
    // Delegation must stay in lock-step with `float4x4::trs`.
    assert_eq!(
        t.compute_matrix(),
        float4x4::trs(t.translation, t.rotation, t.scale)
    );
}

#[test]
fn compute_matrix_encodes_trs_columns() {
    // Identity rotation: the columns are the scaled basis axes and the
    // last column carries the translation (w = 1).
    let t = LocalTransform::new(
        float3::new(1.0, 2.0, 3.0),
        quaternion::IDENTITY,
        float3::new(2.0, 3.0, 4.0),
    );
    let m = t.compute_matrix();
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
    assert_eq!(t.translation, eye, "look_at must place the transform at eye");
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
