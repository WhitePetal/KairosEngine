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

/// Generate an RGBA F32 test image with a simple gradient.
fn make_rgba_f32(w: usize, h: usize) -> Vec<f32> {
    let mut rgba = vec![0.0f32; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            rgba[i] = x as f32 / w.max(1) as f32;
            rgba[i + 1] = y as f32 / h.max(1) as f32;
            rgba[i + 2] = 0.5;
            rgba[i + 3] = 1.0;
        }
    }
    rgba
}

// ============================================================
// Group C: Packed + f32 formats (Rgb10a2, Rg11b10, R32/Rg32/Rgba32)
// ============================================================

fn bench_r32_uint(c: &mut Criterion) {
    let mut g = c.benchmark_group("r32_uint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::R32Uint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::R32Uint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::R32Uint,
                )
            });
        });
    }
}

fn bench_r32_sint(c: &mut Criterion) {
    let mut g = c.benchmark_group("r32_sint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::R32Sint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::R32Sint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::R32Sint,
                )
            });
        });
    }
}

fn bench_r32_float(c: &mut Criterion) {
    let mut g = c.benchmark_group("r32_float");
    for &size in SIZES {
        let rgba = make_rgba_f32(size, size);
        let input = PixelDatas::F32(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::R32Float,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::R32Float);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::R32Float,
                )
            });
        });
    }
}

fn bench_rg32_uint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg32_uint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg32Uint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg32Uint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg32Uint,
                )
            });
        });
    }
}

fn bench_rg32_sint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg32_sint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg32Sint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg32Sint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg32Sint,
                )
            });
        });
    }
}

fn bench_rg32_float(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg32_float");
    for &size in SIZES {
        let rgba = make_rgba_f32(size, size);
        let input = PixelDatas::F32(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg32Float,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg32Float);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg32Float,
                )
            });
        });
    }
}

fn bench_rgba32_uint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rgba32_uint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba32Uint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rgba32Uint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba32Uint,
                )
            });
        });
    }
}

fn bench_rgba32_sint(c: &mut Criterion) {
    let mut g = c.benchmark_group("rgba32_sint");
    for &size in SIZES {
        let rgba = make_rgba(size, size);
        let input = PixelDatas::U8(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba32Sint,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rgba32Sint);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba32Sint,
                )
            });
        });
    }
}

fn bench_rgba32_float(c: &mut Criterion) {
    let mut g = c.benchmark_group("rgba32_float");
    for &size in SIZES {
        let rgba = make_rgba_f32(size, size);
        let input = PixelDatas::F32(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba32Float,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rgba32Float);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgba32Float,
                )
            });
        });
    }
}

fn bench_rgb10a2_unorm(c: &mut Criterion) {
    let mut g = c.benchmark_group("rgb10a2_unorm");
    for &size in SIZES {
        let rgba = make_rgba_f32(size, size);
        let input = PixelDatas::F32(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgb10a2Unorm,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rgb10a2Unorm);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rgb10a2Unorm,
                )
            });
        });
    }
}

fn bench_rg11b10_ufloat(c: &mut Criterion) {
    let mut g = c.benchmark_group("rg11b10_ufloat");
    for &size in SIZES {
        let rgba = make_rgba_f32(size, size);
        let input = PixelDatas::F32(rgba);
        g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
            b.iter(|| {
                encode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg11b10Ufloat,
                )
            });
        });
        let encoded = encode(&input, size as u32, size as u32, TextureFormat::Rg11b10Ufloat);
        g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
            b.iter(|| {
                decode(
                    black_box(px),
                    size as u32,
                    size as u32,
                    TextureFormat::Rg11b10Ufloat,
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

// ============================================================
// Group E — ETC2 / EAC (4×4 block-compressed formats)
// ============================================================

macro_rules! bench_etc_format {
    ($name:ident, $fmt:ident) => {
        fn $name(c: &mut Criterion) {
            let mut g = c.benchmark_group(stringify!($fmt));
            for &size in SIZES {
                let rgba = make_rgba(size, size);
                let input = PixelDatas::U8(rgba);
                g.bench_with_input(BenchmarkId::new("encode", size), &input, |b, px| {
                    b.iter(|| {
                        encode(
                            black_box(px),
                            size as u32,
                            size as u32,
                            TextureFormat::$fmt,
                        )
                    });
                });
                let encoded =
                    encode(&input, size as u32, size as u32, TextureFormat::$fmt);
                g.bench_with_input(BenchmarkId::new("decode", size), &encoded, |b, px| {
                    b.iter(|| {
                        decode(
                            black_box(px),
                            size as u32,
                            size as u32,
                            TextureFormat::$fmt,
                        )
                    });
                });
            }
        }
    };
}

bench_etc_format!(bench_etc2_rgb8_unorm, Etc2Rgb8Unorm);
bench_etc_format!(bench_etc2_rgb8_unorm_srgb, Etc2Rgb8UnormSrgb);
bench_etc_format!(bench_etc2_rgb8_a1_unorm, Etc2Rgb8A1Unorm);
bench_etc_format!(bench_etc2_rgb8_a1_unorm_srgb, Etc2Rgb8A1UnormSrgb);
bench_etc_format!(bench_etc2_rgba8_unorm, Etc2Rgba8Unorm);
bench_etc_format!(bench_etc2_rgba8_unorm_srgb, Etc2Rgba8UnormSrgb);
bench_etc_format!(bench_eac_r11_unorm, EacR11Unorm);
bench_etc_format!(bench_eac_r11_snorm, EacR11Snorm);
bench_etc_format!(bench_eac_rg11_unorm, EacRg11Unorm);
bench_etc_format!(bench_eac_rg11_snorm, EacRg11Snorm);

// ============================================================
// Group F: ASTC (representative subset)
// ============================================================

bench_etc_format!(bench_astc_4x4_unorm, Astc4x4Unorm);
bench_etc_format!(bench_astc_4x4_unorm_srgb, Astc4x4UnormSrgb);
bench_etc_format!(bench_astc_6x6_unorm, Astc6x6Unorm);
bench_etc_format!(bench_astc_8x8_unorm, Astc8x8Unorm);
bench_etc_format!(bench_astc_12x12_unorm, Astc12x12Unorm);

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
    // Group C
    bench_r32_uint,
    bench_r32_sint,
    bench_r32_float,
    bench_rg32_uint,
    bench_rg32_sint,
    bench_rg32_float,
    bench_rgba32_uint,
    bench_rgba32_sint,
    bench_rgba32_float,
    bench_rgb10a2_unorm,
    bench_rg11b10_ufloat,
    // Group E — ETC2 / EAC
    bench_etc2_rgb8_unorm,
    bench_etc2_rgb8_unorm_srgb,
    bench_etc2_rgb8_a1_unorm,
    bench_etc2_rgb8_a1_unorm_srgb,
    bench_etc2_rgba8_unorm,
    bench_etc2_rgba8_unorm_srgb,
    bench_eac_r11_unorm,
    bench_eac_r11_snorm,
    bench_eac_rg11_unorm,
    bench_eac_rg11_snorm,
    // Group F — ASTC
    bench_astc_4x4_unorm,
    bench_astc_4x4_unorm_srgb,
    bench_astc_6x6_unorm,
    bench_astc_8x8_unorm,
    bench_astc_12x12_unorm,
);
criterion_main!(benches);
