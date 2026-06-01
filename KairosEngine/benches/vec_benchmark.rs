#![feature(portable_simd)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use kairos_engine::math::{self, Vector};
use std::{
    hint::black_box,
    simd::{self, num::SimdFloat, simd_swizzle},
};
// use std::simd::{f32x4, simd_swizzle};

#[derive(Clone, Copy)]
struct _float4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}
impl _float4 {
    #[inline(always)]
    fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
    #[inline(always)]
    const fn dot(&self, other: &Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }
    #[inline(always)]
    const fn cross(&self, other: &Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
            w: 0.0,
        }
    }
    #[inline(always)]
    pub const fn len_sq(&self) -> f32 {
        self.dot(self)
    }
    #[inline(always)]
    pub fn len(&self) -> f32 {
        self.len_sq().sqrt()
    }
    #[inline(always)]
    pub fn normalize(&self) -> Self {
        let len = self.len();
        Self {
            x: self.x / len,
            y: self.y / len,
            z: self.z / len,
            w: self.w / len,
        }
    }
}

#[derive(Clone, Copy)]
struct _float4_simd(simd::f32x4);

impl _float4_simd {
    #[inline(always)]
    fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self(simd::f32x4::from_array([x, y, z, w]))
    }
    #[inline(always)]
    fn dot(&self, other: &Self) -> f32 {
        (self.0 * other.0).reduce_sum()
    }
    #[inline(always)]
    fn cross(&self, other: &Self) -> Self {
        let a = self.0;
        let b = other.0;

        let a_yzx = simd_swizzle!(a, [1, 2, 0, 3]);
        let a_zxy = simd_swizzle!(a, [2, 0, 1, 3]);
        let b_yzx = simd_swizzle!(b, [1, 2, 0, 3]);
        let b_zxy = simd_swizzle!(b, [2, 0, 1, 3]);

        Self(a_yzx * b_zxy - a_zxy * b_yzx)
    }
    #[inline(always)]
    pub fn len_sq(&self) -> f32 {
        self.dot(self)
    }
    #[inline(always)]
    pub fn len(&self) -> f32 {
        self.len_sq().sqrt()
    }
    #[inline(always)]
    pub fn normalize(&self) -> Self {
        let len = self.len();
        Self(self.0 / simd::f32x4::splat(len))
    }
}
fn bench_float4_new(c: &mut Criterion) {
    let mut group = c.benchmark_group("float4_new");

    let ar: [(f32, f32, f32, f32); 2] = [(1.0, 2.0, 3.0, 4.0), (4.0, 5.0, 2.0, 3.0)];
    for (i, (x, y, z, w)) in ar.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("default", i),
            &(*x, *y, *z, *w),
            |b, input| {
                b.iter(|| {
                    let (x, y, z, w) = *input;
                    black_box(_float4::new(x, y, z, w))
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("simd", i),
            &(*x, *y, *z, *w),
            |b, input| {
                b.iter(|| {
                    let (x, y, z, w) = *input;
                    black_box(_float4_simd::new(x, y, z, w))
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("kairos", i),
            &(*x, *y, *z, *w),
            |b, input| {
                b.iter(|| {
                    let (x, y, z, w) = *input;
                    black_box(math::float4::new(x, y, z, w))
                })
            },
        );
    }
}

fn bench_float4_copy(c: &mut Criterion) {
    let mut group = c.benchmark_group("float4_copy");

    let ar: [(f32, f32, f32, f32); 2] = [(1.0, 2.0, 3.0, 4.0), (4.0, 5.0, 2.0, 3.0)];
    for (i, (x, y, z, w)) in ar.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("default", i),
            &[_float4::new(*x, *y, *z, *w), _float4::new(*w, *z, *x, *y)],
            |b, input| {
                b.iter(|| {
                    let v = black_box(input[0]);
                    black_box(v)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("simd", i),
            &[
                _float4_simd::new(*x, *y, *z, *w),
                _float4_simd::new(*w, *z, *x, *y),
            ],
            |b, input| {
                b.iter(|| {
                    let v = black_box(input[0]);
                    black_box(v)
                })
            },
        );
        group.bench_with_input(
            BenchmarkId::new("kairos", i),
            &[
                math::float4::new(*x, *y, *z, *w),
                math::float4::new(*w, *z, *x, *y),
            ],
            |b, input| {
                b.iter(|| {
                    let v = black_box(input[0]);
                    black_box(v)
                })
            },
        );
    }
}

fn bench_float4_dot(c: &mut Criterion) {
    let mut group = c.benchmark_group("float4_dot");

    let ar: [(f32, f32, f32, f32); 2] = [(1.0, 2.0, 3.0, 4.0), (4.0, 5.0, 2.0, 3.0)];
    for (i, (x, y, z, w)) in ar.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("default", i),
            &[_float4::new(*x, *y, *z, *w), _float4::new(*w, *z, *y, *x)],
            |b, input| b.iter(|| black_box(_float4::dot(&input[0], &input[1]))),
        );
        group.bench_with_input(
            BenchmarkId::new("simid", i),
            &[
                _float4_simd::new(*x, *y, *z, *w),
                _float4_simd::new(*w, *z, *y, *x),
            ],
            |b, input| b.iter(|| black_box(_float4_simd::dot(&input[0], &input[1]))),
        );
        group.bench_with_input(
            BenchmarkId::new("kairos", i),
            &[
                math::float4::new(*x, *y, *z, *w),
                math::float4::new(*w, *z, *y, *x),
            ],
            |b, input| b.iter(|| black_box(math::float4::dot(&input[0], &input[1]))),
        );
    }
}

fn bench_float4_cross(c: &mut Criterion) {
    let mut group = c.benchmark_group("float4_cross");

    let ar: [(f32, f32, f32, f32); 2] = [(1.0, 2.0, 3.0, 4.0), (4.0, 5.0, 2.0, 3.0)];
    for (i, (x, y, z, w)) in ar.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("default", i),
            &[_float4::new(*x, *y, *z, *w), _float4::new(*w, *z, *y, *x)],
            |b, input| b.iter(|| black_box(_float4::cross(&input[0], &input[1]))),
        );
        group.bench_with_input(
            BenchmarkId::new("simid", i),
            &[
                _float4_simd::new(*x, *y, *z, *w),
                _float4_simd::new(*w, *z, *y, *x),
            ],
            |b, input| b.iter(|| black_box(_float4_simd::cross(&input[0], &input[1]))),
        );
        group.bench_with_input(
            BenchmarkId::new("kairos", i),
            &[
                math::float4::new(*x, *y, *z, *w),
                math::float4::new(*w, *z, *y, *x),
            ],
            |b, input| b.iter(|| black_box(math::cross(input[0], input[1]))),
        );
    }
}

fn bench_float4_normalize(c: &mut Criterion) {
    let mut group = c.benchmark_group("float4_normalize");

    let ar: [(f32, f32, f32, f32); 2] = [(1.0, 2.0, 3.0, 4.0), (4.0, 5.0, 2.0, 3.0)];
    for (i, (x, y, z, w)) in ar.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("default", i),
            &_float4::new(*x, *y, *z, *w),
            |b, input| b.iter(|| black_box(input.normalize())),
        );
        group.bench_with_input(
            BenchmarkId::new("simid", i),
            &_float4_simd::new(*x, *y, *z, *w),
            |b, input| b.iter(|| black_box(input.normalize())),
        );
        group.bench_with_input(
            BenchmarkId::new("kairos", i),
            &math::float4::new(*x, *y, *z, *w),
            |b, input| b.iter(|| black_box(input.normalize())),
        );
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

criterion_group!(
    benches,
    bench_float4_new,
    bench_float4_copy,
    bench_float4_dot,
    bench_float4_cross,
    bench_float4_normalize,
);
criterion_main!(benches);
