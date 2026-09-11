use crate::{affine, float3, float4, float4x4, quaternion};

fn assert_float3_eq(a: float3, b: float3, eps: f32) {
    let msg = format!("float3 {a:?} != {b:?}");
    assert!((a.x() - b.x()).abs() < eps, "{msg}");
    assert!((a.y() - b.y()).abs() < eps, "{msg}");
    assert!((a.z() - b.z()).abs() < eps, "{msg}");
}

fn assert_quaternion_eq(a: quaternion, b: quaternion, eps: f32) {
    let msg = format!("quaternion {a:?} != {b:?}");
    assert!((a.0.x - b.0.x).abs() < eps, "{msg}");
    assert!((a.0.y - b.0.y).abs() < eps, "{msg}");
    assert!((a.0.z - b.0.z).abs() < eps, "{msg}");
    assert!((a.0.w - b.0.w).abs() < eps, "{msg}");
}

fn assert_matrix_eq(a: float4x4, b: float4x4, eps: f32) {
    let (aa, bb) = (a.to_array(), b.to_array());
    for (col_a, col_b) in aa.iter().zip(bb.iter()) {
        for (x, y) in col_a.iter().zip(col_b.iter()) {
            assert!((x - y).abs() < eps, "matrix {aa:?} != {bb:?}");
        }
    }
}

#[test]
fn default_is_identity() {
    assert_eq!(affine::default(), affine::IDENTITY);
}

#[test]
fn identity_trs_is_pure_translation() {
    let a = affine::trs(
        float3::new(1.0, 2.0, 3.0),
        quaternion::IDENTITY,
        float3::ONE,
    );

    // Pure translation: translation is stored exactly, and transforming the
    // origin returns it exactly.
    assert_eq!(a.translation(), float3::new(1.0, 2.0, 3.0));
    assert_eq!(a.transform_point(float3::ZERO), float3::new(1.0, 2.0, 3.0));

    let (scale, rotation, translation) = a.to_scale_rotation_translation();
    assert_eq!(scale, float3::ONE);
    assert_float3_eq(translation, float3::new(1.0, 2.0, 3.0), 1e-6);
    assert_quaternion_eq(rotation, quaternion::IDENTITY, 1e-6);
}

#[test]
fn trs_to_float4x4_matches_matrix_trs() {
    let position = float3::new(1.0, -2.0, 3.0);
    let rotation = quaternion::new(0.1, 0.2, 0.3, 0.9).normalized();
    let scale = float3::new(2.0, 3.0, 4.0);

    let via_affine = affine::trs(position, rotation, scale).to_float4x4();
    let via_matrix = float4x4::trs(position, rotation, scale);

    // `float4x4::trs` delegates to `affine::trs` (both route through glam's
    // `from_scale_rotation_translation`), so the two are bit-identical.
    assert_eq!(via_affine, via_matrix);
}

#[test]
fn trs_accessors_roundtrip() {
    let position = float3::new(1.0, -2.0, 3.0);
    let rotation = quaternion::new(0.1, 0.2, 0.3, 0.9).normalized();
    let scale = float3::new(2.0, 3.0, 4.0);
    let a = affine::trs(position, rotation, scale);

    let (scale_out, rotation_out, translation_out) = a.to_scale_rotation_translation();

    assert_float3_eq(scale_out, scale, 1e-4);
    assert_quaternion_eq(rotation_out, rotation, 1e-4);
    assert_float3_eq(translation_out, position, 1e-4);

    // Rebuilding from the decomposed parts lands on the same transform.
    let rebuilt = affine::trs(translation_out, rotation_out, scale_out);
    assert_matrix_eq(rebuilt.to_float4x4(), a.to_float4x4(), 1e-3);
}

#[test]
fn transform_point_matches_float4x4_mul() {
    let a = affine::trs(
        float3::new(1.0, -2.0, 3.0),
        quaternion::new(0.1, 0.2, 0.3, 0.9).normalized(),
        float3::new(2.0, 3.0, 4.0),
    );
    let point = float3::new(0.5, -1.0, 2.0);

    let via_affine = a.transform_point(point);
    let via_matrix = a.to_float4x4() * float4::new(point.x(), point.y(), point.z(), 1.0);

    assert_float3_eq(
        via_affine,
        float3::new(via_matrix.x(), via_matrix.y(), via_matrix.z()),
        1e-4,
    );
}

#[test]
fn mul_transforms_point_like_transform_point() {
    let a = affine::trs(
        float3::new(1.0, -2.0, 3.0),
        quaternion::new(0.1, 0.2, 0.3, 0.9).normalized(),
        float3::new(2.0, 3.0, 4.0),
    );
    let point = float3::new(0.5, -1.0, 2.0);

    // `*` is defined as a point transform: it applies translation.
    assert_float3_eq(a * point, a.transform_point(point), 1e-6);
}

#[test]
fn transform_vector_ignores_translation() {
    let a = affine::trs(
        float3::new(1.0, -2.0, 3.0),
        quaternion::IDENTITY,
        float3::ONE,
    );
    let direction = float3::new(0.5, -1.0, 2.0);

    // Pure translation must leave a direction untouched...
    assert_float3_eq(a.transform_vector(direction), direction, 1e-6);
    // ...whereas `*`/`transform_point` shifts it by the translation.
    assert_float3_eq(
        a.transform_vector(direction),
        a.transform_point(direction) - a.translation(),
        1e-6,
    );
}

#[test]
fn inverse_roundtrip() {
    let a = affine::trs(
        float3::new(1.0, -2.0, 3.0),
        quaternion::new(0.1, 0.2, 0.3, 0.9).normalized(),
        float3::new(2.0, 3.0, 4.0),
    );
    let point = float3::new(0.5, -1.0, 2.0);

    let moved = a.transform_point(point);
    let back = a.inverse().transform_point(moved);

    assert_float3_eq(back, point, 1e-3);
}

#[test]
fn affine_is_bytemuck_zeroable() {
    fn assert_zeroable<T: bytemuck::Zeroable>() {}
    assert_zeroable::<affine>();
}
