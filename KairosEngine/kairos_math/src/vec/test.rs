#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
struct float4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl float4 {
    #[inline]
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    #[inline]
    pub fn dot(vl: &float4, vr: &float4) -> f32 {
        vl.x * vr.x + vl.y * vr.y + vl.z * vr.z + vl.w * vr.w
    }
}

#[test]
fn float4_dot() {
    let v0 = float4::new(1.0, 2.0, 3.0, 4.0);
    let v1 = float4::new(4.0, 3.0, 2.0, 1.0);

    assert_eq!(
        1.0 * 4.0 + 2.0 * 3.0 + 3.0 * 2.0 + 4.0 * 1.0,
        float4::dot(&v0, &v1)
    )
}

// float3 wraps `Vec3A`, whose SIMD layout carries w-lane padding — glam only
// implements `AnyBitPattern`/`Zeroable` (not `Pod`) for `Vec3A`, and we mirror
// that. Assert `Zeroable`; `Pod` is intentionally absent (see the impl in
// vec.rs).
#[test]
fn float3_is_bytemuck_zeroable() {
    fn assert_zeroable<T: bytemuck::Zeroable>() {}
    assert_zeroable::<crate::float3>();
}
