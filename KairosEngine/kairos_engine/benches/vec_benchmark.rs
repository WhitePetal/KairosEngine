use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use glam::Vec4Swizzles;
use kairos_engine::math::{self, Vector};
use std::hint::black_box;

#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
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
    const fn dot(&self, o: &Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z + self.w * o.w
    }
    #[inline(always)]
    const fn cross(&self, o: &Self) -> Self {
        Self {
            x: self.y * o.z - self.z * o.y,
            y: self.z * o.x - self.x * o.z,
            z: self.x * o.y - self.y * o.x,
            w: 0.0,
        }
    }
    #[inline(always)]
    fn len(&self) -> f32 {
        self.dot(self).sqrt()
    }
    #[inline(always)]
    fn normalize(&self) -> Self {
        let l = self.len();
        Self {
            x: self.x / l,
            y: self.y / l,
            z: self.z / l,
            w: self.w / l,
        }
    }
}

#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
struct _float4_glam(glam::Vec4);
impl _float4_glam {
    #[inline(always)]
    fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self(glam::Vec4::new(x, y, z, w))
    }
    #[inline(always)]
    fn dot(&self, o: &Self) -> f32 {
        self.0.dot(o.0)
    }
    #[inline(always)]
    fn cross(&self, o: &Self) -> Self {
        let a = self.0;
        let b = o.0;
        Self(a.yzxw() * b.zxyw() - a.zxyw() * b.yzxw())
    }
    #[inline(always)]
    fn len(&self) -> f32 {
        self.dot(self).sqrt()
    }
    #[inline(always)]
    fn normalize(&self) -> Self {
        Self(self.0 / self.len())
    }
}

fn bench_float4_new(c: &mut Criterion) {
    let mut g = c.benchmark_group("float4_new");
    let ar = [(1.0, 2.0, 3.0, 4.0), (4.0, 5.0, 2.0, 3.0)];
    for (i, (x, y, z, w)) in ar.iter().enumerate() {
        g.bench_with_input(
            BenchmarkId::new("default", i),
            &(*x, *y, *z, *w),
            |b, inp| {
                b.iter(|| {
                    let (x, y, z, w) = *inp;
                    black_box(_float4::new(x, y, z, w))
                })
            },
        );
        g.bench_with_input(BenchmarkId::new("glam", i), &(*x, *y, *z, *w), |b, inp| {
            b.iter(|| {
                let (x, y, z, w) = *inp;
                black_box(_float4_glam::new(x, y, z, w))
            })
        });
        g.bench_with_input(
            BenchmarkId::new("kairos", i),
            &(*x, *y, *z, *w),
            |b, inp| {
                b.iter(|| {
                    let (x, y, z, w) = *inp;
                    black_box(math::float4::new(x, y, z, w))
                })
            },
        );
    }
}

fn bench_float4_copy(c: &mut Criterion) {
    let mut g = c.benchmark_group("float4_copy");
    let ar = [(1.0, 2.0, 3.0, 4.0), (4.0, 5.0, 2.0, 3.0)];
    for (i, (x, y, z, w)) in ar.iter().enumerate() {
        g.bench_with_input(
            BenchmarkId::new("default", i),
            &[_float4::new(*x, *y, *z, *w), _float4::new(*w, *z, *x, *y)],
            |b, inp| b.iter(|| black_box(inp[0])),
        );
        g.bench_with_input(
            BenchmarkId::new("glam", i),
            &[
                _float4_glam::new(*x, *y, *z, *w),
                _float4_glam::new(*w, *z, *x, *y),
            ],
            |b, inp| b.iter(|| black_box(inp[0])),
        );
        g.bench_with_input(
            BenchmarkId::new("kairos", i),
            &[
                math::float4::new(*x, *y, *z, *w),
                math::float4::new(*w, *z, *x, *y),
            ],
            |b, inp| b.iter(|| black_box(inp[0])),
        );
    }
}

fn bench_float4_dot(c: &mut Criterion) {
    let mut g = c.benchmark_group("float4_dot");
    let ar = [(1.0, 2.0, 3.0, 4.0), (4.0, 5.0, 2.0, 3.0)];
    for (i, (x, y, z, w)) in ar.iter().enumerate() {
        g.bench_with_input(
            BenchmarkId::new("default", i),
            &[_float4::new(*x, *y, *z, *w), _float4::new(*w, *z, *y, *x)],
            |b, inp| b.iter(|| black_box(_float4::dot(&inp[0], &inp[1]))),
        );
        g.bench_with_input(
            BenchmarkId::new("glam", i),
            &[
                _float4_glam::new(*x, *y, *z, *w),
                _float4_glam::new(*w, *z, *y, *x),
            ],
            |b, inp| b.iter(|| black_box(_float4_glam::dot(&inp[0], &inp[1]))),
        );
        g.bench_with_input(
            BenchmarkId::new("kairos", i),
            &[
                math::float4::new(*x, *y, *z, *w),
                math::float4::new(*w, *z, *y, *x),
            ],
            |b, inp| b.iter(|| black_box(math::float4::dot(&inp[0], &inp[1]))),
        );
    }
}

fn bench_float4_cross(c: &mut Criterion) {
    let mut g = c.benchmark_group("float4_cross");
    let ar = [(1.0, 2.0, 3.0, 4.0), (4.0, 5.0, 2.0, 3.0)];
    for (i, (x, y, z, w)) in ar.iter().enumerate() {
        g.bench_with_input(
            BenchmarkId::new("default", i),
            &[_float4::new(*x, *y, *z, *w), _float4::new(*w, *z, *y, *x)],
            |b, inp| b.iter(|| black_box(_float4::cross(&inp[0], &inp[1]))),
        );
        g.bench_with_input(
            BenchmarkId::new("glam", i),
            &[
                _float4_glam::new(*x, *y, *z, *w),
                _float4_glam::new(*w, *z, *y, *x),
            ],
            |b, inp| b.iter(|| black_box(_float4_glam::cross(&inp[0], &inp[1]))),
        );
        g.bench_with_input(
            BenchmarkId::new("kairos", i),
            &[
                math::float4::new(*x, *y, *z, *w),
                math::float4::new(*w, *z, *y, *x),
            ],
            |b, inp| b.iter(|| black_box(math::cross(inp[0], inp[1]))),
        );
    }
}

fn bench_float4_normalize(c: &mut Criterion) {
    let mut g = c.benchmark_group("float4_normalize");
    let ar = [(1.0, 2.0, 3.0, 4.0), (4.0, 5.0, 2.0, 3.0)];
    for (i, (x, y, z, w)) in ar.iter().enumerate() {
        g.bench_with_input(
            BenchmarkId::new("default", i),
            &_float4::new(*x, *y, *z, *w),
            |b, inp| b.iter(|| black_box(inp.normalize())),
        );
        g.bench_with_input(
            BenchmarkId::new("glam", i),
            &_float4_glam::new(*x, *y, *z, *w),
            |b, inp| b.iter(|| black_box(inp.normalize())),
        );
        g.bench_with_input(
            BenchmarkId::new("kairos", i),
            &math::float4::new(*x, *y, *z, *w),
            |b, inp| b.iter(|| black_box(inp.normalize())),
        );
    }
}

criterion_group!(
    benches,
    bench_float4_new,
    bench_float4_copy,
    bench_float4_dot,
    bench_float4_cross,
    bench_float4_normalize
);
criterion_main!(benches);
