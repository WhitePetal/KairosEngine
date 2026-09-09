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

// Baseline gap fixed in kairos_math (#143): float3 was the only wrapper
// missing bytemuck impls while `Vertex` (engine) needs them.
#[test]
fn float3_is_bytemuck_pod() {
    fn assert_pod<T: bytemuck::Pod>() {}
    fn assert_zeroable<T: bytemuck::Zeroable>() {}
    assert_pod::<crate::float3>();
    assert_zeroable::<crate::float3>();
}
