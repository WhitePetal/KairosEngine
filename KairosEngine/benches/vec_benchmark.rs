// #![feature(portable_simd)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use kairos_engine::math;
// use std::simd::{f32x4, simd_swizzle};

struct _float4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32
}
impl _float4 {
    fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
    fn dot(&self, other: &Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }
}

fn bench_float4_dot(c: &mut Criterion) {
    let mut group = c.benchmark_group("float4_dot");

    let ar: [(f32, f32, f32, f32); 2] = [
        (1.0, 2.0, 3.0, 4.0),
        (4.0, 5.0, 2.0, 3.0)
    ];
    let mut i = 0;

    for (x, y, z, w) in ar.iter() {
        i += 1;

        group.bench_with_input(BenchmarkId::new("leagcy", i), &(x, y, z, w), |b, i| b.iter(|| {
            let vl = _float4::new(*x, *y, *z, *w);
            let vr = _float4::new(*z, *w, *y, *x);
            
            black_box(_float4::dot(&vl, &vr));
        }));
        group.bench_with_input(BenchmarkId::new("simid", i), &(x, y, z, w), |b, i| b.iter(|| {
            let vl = math::float4::new(*x, *y, *z, *w);
            let vr = math::float4::new(*z, *w, *y, *x);
            
            math::float4::dot(&vl, &vr);
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