//! Criterion benchmarks for Group A uncompressed SDR texture encode/decode.
//!
//! Benchmarks all 9 formats at 64², 256², 1024², 4096² pixel sizes.
//! Each benchmark group measures encode + decode for a single format.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use kairos_engine::graphics::texture::format::{PixelDatas, TextureFormat};
use kairos_engine::graphics::texture::format::{decode, encode};
use std::hint::black_box;

/// Generate an RGBA8 test image with a simple gradient.
fn make_rgba(w: usize, h: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            rgba[i] = (x * 255 / w.max(1)) as u8;
            rgba[i + 1] = (y * 255 / h.max(1)) as u8;
            rgba[i + 2] = 128;
            rgba[i + 3] = 255;
        }
    }
    rgba
}

const SIZES: &[usize] = &[64, 256, 1024, 4096];

// ============================================================
// RG8 variants (all use the same encode/decode)
// ============================================================

fn bench_rg8_unorm(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg8_unorm");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| encode(black_box(px), size as u32, size as u32, TextureFormat::Rg8Unorm));
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg8Unorm);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| decode(black_box(px), size as u32, size as u32, TextureFormat::Rg8Unorm));
        });
    }
}

fn bench_rg8_snorm(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg8_snorm");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| encode(black_box(px), size as u32, size as u32, TextureFormat::Rg8Snorm));
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg8Snorm);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| decode(black_box(px), size as u32, size as u32, TextureFormat::Rg8Snorm));
        });
    }
}

fn bench_rg8_uint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg8_uint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| encode(black_box(px), size as u32, size as u32, TextureFormat::Rg8Uint));
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg8Uint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| decode(black_box(px), size as u32, size as u32, TextureFormat::Rg8Uint));
        });
    }
}

fn bench_rg8_sint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg8_sint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| encode(black_box(px), size as u32, size as u32, TextureFormat::Rg8Sint));
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg8Sint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| decode(black_box(px), size as u32, size as u32, TextureFormat::Rg8Sint));
        });
    }
}

// ============================================================
// RGBA8 pass-through variants (Snorm, Uint, Sint)
// ============================================================

fn bench_rgba8_snorm(c: &mut Criterion) {
    let mut g = c.benchmark_group("rgba8_snorm");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba8Snorm,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rgba8Snorm);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba8Snorm,
                )
            });
        });
    }
}

fn bench_rgba8_uint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rgba8_uint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba8Uint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rgba8Uint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba8Uint,
                )
            });
        });
    }
}

fn bench_rgba8_sint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rgba8_sint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba8Sint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rgba8Sint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba8Sint,
                )
            });
        });
    }
}

// ============================================================
// BGRA8 swizzle variants (Unorm, Srgb)
// ============================================================

fn bench_bgra8_unorm(c: &mut Criterion) {
    let mut g = c.benchmark_group("bgra8_unorm");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Bgra8Unorm,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Bgra8Unorm);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Bgra8Unorm,
                )
            });
        });
    }
}

fn bench_bgra8_unorm_srgb(c: &mut Criterion) {
    let mut g = c.benchmark_group("bgra8_unorm_srgb");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Bgra8UnormSrgb,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Bgra8UnormSrgb);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Bgra8UnormSrgb,
                )
            });
        });
    }
}

criterion_group!(
    benches,
    bench_rg8_unorm,
    bench_rg8_snorm,
    bench_rg8_uint,
    bench_rg8_sint,
    bench_rgba8_snorm,
    bench_rgba8_uint,
    bench_rgba8_sint,
    bench_bgra8_unorm,
    bench_bgra8_unorm_srgb,
);
criterion_main!(benches);
