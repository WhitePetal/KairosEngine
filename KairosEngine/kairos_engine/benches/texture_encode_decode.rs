//! Criterion benchmarks for Group A uncompressed SDR texture encode/decode.
//!
//! Benchmarks all 9 formats at 64², 256², 1024², 4096² pixel sizes.
//! Each benchmark group measures encode + decode for a single format.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use half::f16;
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

/// Generate an RGBA F16 test image with a simple gradient.
fn make_rgba_f16(w: usize, h: usize) -> Vec<f16> {
    let mut rgba = vec![f16::ZERO; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            rgba[i] = f16::from_f32(x as f32 / w.max(1) as f32);
            rgba[i + 1] = f16::from_f32(y as f32 / h.max(1) as f32);
            rgba[i + 2] = f16::from_f32(0.5);
            rgba[i + 3] = f16::from_f32(1.0);
        }
    }
    rgba
}

// ============================================================
// RG8 variants (all use the same encode/decode)
// ============================================================

fn bench_rg8_unorm(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg8_unorm");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg8Unorm,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg8Unorm);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg8Unorm,
                )
            });
        });
    }
}

fn bench_rg8_snorm(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg8_snorm");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg8Snorm,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg8Snorm);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg8Snorm,
                )
            });
        });
    }
}

fn bench_rg8_uint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg8_uint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg8Uint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg8Uint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg8Uint,
                )
            });
        });
    }
}

fn bench_rg8_sint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg8_sint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg8Sint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg8Sint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg8Sint,
                )
            });
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
        let encoded = encode(
            &input,
            size as u32,
            size as u32,
            TextureFormat::Bgra8UnormSrgb,
        );
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

// ============================================================
// Group B: Wide format benchmarks (R16, Rg16, Rgba16 Uint/Sint/Float)
// ============================================================

fn bench_r16_uint(c: &mut Criterion) {
    let mut g = c.benchmark_group("r16_uint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::R16Uint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::R16Uint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::R16Uint,
                )
            });
        });
    }
}

fn bench_r16_sint(c: &mut Criterion) {
    let mut g = c.benchmark_group("r16_sint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::R16Sint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::R16Sint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::R16Sint,
                )
            });
        });
    }
}

fn bench_r16_float(c: &mut Criterion) {
    let mut g = c.benchmark_group("r16_float");
    for &size in SIZES {
        let rgba = make_rgba_f16(size, size);
        let input = PixelDatas::F16(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::R16Float,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::R16Float);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::R16Float,
                )
            });
        });
    }
}

fn bench_rg16_uint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg16_uint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg16Uint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg16Uint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg16Uint,
                )
            });
        });
    }
}

fn bench_rg16_sint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg16_sint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg16Sint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg16Sint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg16Sint,
                )
            });
        });
    }
}

fn bench_rg16_float(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg16_float");
    for &size in SIZES {
        let rgba = make_rgba_f16(size, size);
        let input = PixelDatas::F16(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg16Float,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg16Float);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg16Float,
                )
            });
        });
    }
}

fn bench_rgba16_uint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rgba16_uint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba16Uint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rgba16Uint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba16Uint,
                )
            });
        });
    }
}

fn bench_rgba16_sint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rgba16_sint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba16Sint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rgba16Sint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba16Sint,
                )
            });
        });
    }
}

fn bench_rgba16_float(c: &mut Criterion) {
    let mut g = c.benchmark_group("rgba16_float");
    for &size in SIZES {
        let rgba = make_rgba_f16(size, size);
        let input = PixelDatas::F16(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba16Float,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rgba16Float);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba16Float,
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
    bench_r16_uint,
    bench_r16_sint,
    bench_r16_float,
    bench_rg16_uint,
    bench_rg16_sint,
    bench_rg16_float,
    bench_rgba16_uint,
    bench_rgba16_sint,
    bench_rgba16_float,
);
criterion_main!(benches);
