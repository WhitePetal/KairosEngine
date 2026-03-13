#![feature(portable_simd)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::{hint::black_box, simd, simd::num::SimdFloat};
use kairos_engine::math;
// use std::simd::{f32x4, simd_swizzle};

struct _float4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32
}
impl _float4 {
    #[inline]
    fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
    #[inline(always)]
    fn dot(&self, other: &Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }
}

struct _float4_simd(simd::f32x4);

impl _float4_simd {
    #[inline]
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self(simd::f32x4::from_array([x, y, z, w]))
    }

    #[inline(always)]
    pub fn dot_simd(&self, other: &Self) -> f32 {
        (self.0 * other.0).reduce_sum()
    }
}

fn bench_float4_dot(c: &mut Criterion) {
    let mut group = c.benchmark_group("float4_dot");

    let ar: [(f32, f32, f32, f32); 2] = [
        (1.0, 2.0, 3.0, 4.0),
        (4.0, 5.0, 2.0, 3.0)
    ];
    for (i, (x, y, z, w)) in ar.iter().enumerate() {
        group.bench_with_input(BenchmarkId::new("simid", i), &(x, y, z, w), |b, input| b.iter(|| {
            let (x, y, z, w) = *black_box(input);
            let vl = _float4_simd::new(*x, *y, *z, *w);
            let vr = _float4_simd::new(*w, *z, *y, *x);
            
            black_box(_float4_simd::dot_simd(&vl, &vr));
        }));
        group.bench_with_input(BenchmarkId::new("kairos", i), &(x, y, z, w), |b, input| b.iter(|| {
            let (x, y, z, w) = *black_box(input);
            let vl = math::float4::new(*x, *y, *z, *w);
            let vr = math::float4::new(*w, *z, *y, *x);
            
            black_box(math::float4::dot(&vl, &vr));
        }));
    }
}


// fn kairos_float4_xxx_bench_test(c: &mut Criterion) {
//     b.iter(|| {
//         let mut  v = f32x4::splat(0.0);
//         for i in 0..100 {
//             let v4 = f32x4::from_array([i as f32, (i + 1) as f32, (i + 2) as f32, (i + 3) as f32]);
//             let v3 = f32x4::from_array([v4[0], v4[1], v4[2], 0.0]);
//             v += v3;
//         }
//         return v;
//     });
// }


// fn kairos_float4_simd_xxx_bench_test(c: &mut Criterion) {
//     b.iter(|| {
//         let mut  v = f32x4::splat(0.0);
//         for i in 0..100 {
//             let v4 = f32x4::from_array([i as f32, (i + 1) as f32, (i + 2) as f32, (i + 3) as f32]);
//             let v3 = simd_swizzle!(v4, [0, 0, 0, 0]) * f32x4::from_array([1.0, 1.0, 1.0, 0.0]);
//             v += v3;
//         }
//         return v;
//     });
// }

criterion_group!(benches, bench_float4_dot);
criterion_main!(benches);