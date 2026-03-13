
extern crate test;

use super::*;
use test::Bencher;

#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
struct float4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32
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

#[bench]
fn kairos_float4_dot_bench_test(b: &mut Bencher) {
    let vl = float4::new(1.0, 2.0, 3.0, 4.0);
    let vr = float4::new(5.0, 6.0, 7.0, 8.0);
    b.iter(|| {
        let mut  value = 0.0;
        for _i in 0..1000 {
            // vl.x = vl.x + i;
            // vr.y = vr.y + i;
            value += float4::dot(&vl, &vr);
        }
        return value;
    });
}

#[bench]
fn kairos_float4_simd_dot_bench_test(b: &mut Bencher) {
    let vl = super::float4::new(1.0, 2.0, 3.0, 4.0);
    let vr = super::float4::new(5.0, 6.0, 7.0, 8.0);
    b.iter(|| {
        let mut  value = 0.0;
        for _i in 0..1000 {
            // vl.x = vl.x + i;
            // vr.y = vr.y + i;
            value += super::float4::dot(&vl, &vr);
        }
        return value;;
    });
}

#[bench]
fn kairos_float4_xxx_bench_test(b: &mut Bencher) {
    b.iter(|| {
        let mut  v = f32x4::splat(0.0);
        for i in 0..100 {
            let v4 = f32x4::from_array([i as f32, (i + 1) as f32, (i + 2) as f32, (i + 3) as f32]);
            let v3 = f32x4::from_array([v4[0], v4[1], v4[2], 0.0]);
            v += v3;
        }
        return v;
    });
}

#[bench]
fn kairos_float4_simd_xxx_bench_test(b: &mut Bencher) {
    b.iter(|| {
        let mut  v = f32x4::splat(0.0);
        for i in 0..100 {
            let v4 = f32x4::from_array([i as f32, (i + 1) as f32, (i + 2) as f32, (i + 3) as f32]);
            let v3 = simd_swizzle!(v4, [0, 0, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]);
            v += v3;
        }
        return v;
    });
}