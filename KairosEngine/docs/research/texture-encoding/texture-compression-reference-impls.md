# Texture Compression Reference Implementations

> Research date: 2026-07-21
>
> Question: What are the canonical C++ reference implementations for BC6h/BC7, ETC2/EAC, and ASTC texture compression that could be translated to pure Rust?

---

## Table of Contents

1. [ASTC](#1-astc)
2. [BC6h / BC7](#2-bc6h--bc7)
3. [ETC2 / EAC](#3-etc2--eac)
4. [UASTC](#4-uastc)
5. [Basis Universal (cross-format supercompression)](#5-basis-universal)
6. [Bit-Layout Specifications](#6-bit-layout-specifications)
7. [Testing & Reference Data](#7-testing--reference-data)
8. [Summary Table](#8-summary-table)

---

## 1. ASTC

### Official Specification

- **Khronos Data Format Specification v1.4.0** — Section 23 ("ASTC Compressed Texture Image Formats")
  - <https://www.khronos.org/registry/DataFormat/specs/1.4/dataformat.1.4.html#ASTC>
  - Copyright Khronos Group, publicly available specification (not open-source but freely readable)
  - Contains the complete decode procedure: block mode parsing, integer sequence encoding (trits/quints/bits), color endpoint modes, weight decoding, interpolation, void-extent blocks, partition pattern generation, and the hash52 function.

### Primary Reference Implementation

**ARM `astcenc`** (<https://github.com/ARM-software/astc-encoder>)

| Property | Value |
|---|---|
| **License** | Apache 2.0 |
| **Language** | C++ (87%), Python (9.8%), CMake (2.9%) |
| **Stars** | ~1.3k |
| **Status** | Active development (main branch), stable releases tagged |
| **Supports** | Full profile: LDR, HDR, 2D, 3D, all block sizes (4x4 through 12x12), all compression modes |

#### File structure (core encoder library in `Source/`)

| File | Purpose |
|---|---|
| `astcenc.h` | Public API header |
| `astcenc_internal.h` / `astcenc_internal_entry.h` | Internal data structures and constants |
| `astcenc_entry.cpp` | Library entry point, compression API |
| `astcenc_compress_symbolic.cpp` | Main compression logic, mode selection |
| `astcenc_decompress_symbolic.cpp` | Main decompression logic |
| `astcenc_block_sizes.cpp` | Block size table and handling |
| `astcenc_partition_tables.cpp` | Partition pattern generation (hash52) |
| `astcenc_find_best_partitioning.cpp` | Partition search |
| `astcenc_averages_and_directions.cpp` | PCA-based endpoint fitting |
| `astcenc_ideal_endpoints_and_weights.cpp` | Endpoint optimization |
| `astcenc_color_quantize.cpp` | Color endpoint quantization |
| `astcenc_color_unquantize.cpp` | Color endpoint unquantization |
| `astcenc_quantization.cpp` | Integer sequence encoding (trit/quint/bit packing) |
| `astcenc_integer_sequence.cpp` | BISE (Bit-Integer Sequence Encoding) encode/decode |
| `astcenc_symbolic_physical.cpp` | Conversion between symbolic and physical block representations |
| `astcenc_weight_align.cpp` | Weight grid alignment |
| `astcenc_weight_quant_xfer_tables.cpp` | Weight quantization transfer tables |
| `astcenc_pick_best_endpoint_format.cpp` | Endpoint format selection |
| `astcenc_compute_variance.cpp` | Variance analysis for block partitioning |
| `astcenc_image.cpp` | Image data loading |
| `astcenc_mathlib.cpp` / `astcenc_mathlib_softfloat.cpp` | Math utilities |
| `astcenc_vecmathlib_*.h` (8 files) | SIMD abstraction layer (SSE2, SSE4.1, AVX2, NEON, SVE, RVV) |
| `astcenc_percentile_tables.cpp` | Percentile tables for encoding heuristics |

#### Core files needed for encoding

For a **minimal re-implementation**, the essential algorithm files are:

1. `astcenc_block_sizes.cpp` — Block size definitions
2. `astcenc_partition_tables.cpp` — Partition pattern generation
3. `astcenc_integer_sequence.cpp` — BISE packing/unpacking
4. `astcenc_quantization.cpp` — Quantization tables
5. `astcenc_color_unquantize.cpp` — Endpoint unquantization
6. `astcenc_color_quantize.cpp` — Endpoint quantization
7. `astcenc_ideal_endpoints_and_weights.cpp` — Interpolation math
8. `astcenc_symbolic_physical.cpp` — Bit packing to 128-bit blocks
9. `astcenc_compress_symbolic.cpp` — Compression orchestrator
10. `astcenc_decompress_symbolic.cpp` — Decompression orchestrator

**Approximate code size**: ~50,000 lines of C++ across ~40 source files. The core compression algorithm (excluding CLI, image I/O, SIMD backends) is roughly ~25,000 lines.

#### Algorithmic features

- 2D block footprints: 4×4 through 12×12 (14 sizes, 0.89–8.00 bpp)
- 3D block footprints: 3×3×3 through 6×6×6 (10 sizes)
- Up to 4 partitions per block (via hash52 deterministic function)
- Dual-plane mode for independent weight interpolation of one channel
- 16 color endpoint modes (LDR luminance, LDR luminance+alpha, LDR RGB, LDR RGBA, HDR variants)
- Integer Sequence Encoding (trits/quints/bits) for non-power-of-two ranges
- Void-extent blocks (constant-color with spatial hints)
- Weight grid upsampling for low-bitrate modes
- sRGB, linear LDR, and HDR operation modes

### Google ASTC Codec (decoder only)

<https://android.googlesource.com/platform/external/astc-codec/>

| Property | Value |
|---|---|
| **License** | Apache 2.0 |
| **Language** | C++ |
| **Scope** | Decoder only (LDR profile) |
| **Notes** | Bazel + CMake build. Clean, well-structured C++ reference for decode path. |

---

## 2. BC6h / BC7

### Official Specification

- **Khronos Data Format Specification v1.4.0** — Section 20 ("BPTC Compressed Texture Image Formats")
  - <https://www.khronos.org/registry/DataFormat/specs/1.4/dataformat.1.4.html#BPTC>
  - Contains the complete decode procedure for both BC6H (signed/unsigned HDR) and BC7 (LDR RGBA).

### Reference Implementations

#### 2.1 Intel ISPC Texture Compressor

<https://github.com/GameTechDev/ISPCTextureCompressor>

| Property | Value |
|---|---|
| **License** | MIT |
| **Language** | C++ (64.4%), ISPC (29.3% assembly kernels), C (3.1%) |
| **Stars** | ~477 |
| **Status** | **Archived** (Sep 2024) — no longer maintained by Intel |
| **Supports** | BC6H, BC7, ASTC (LDR up to 8×8), ETC1, BC1–BC5 |

##### File structure

| File | Purpose |
|---|---|
| `ispc_texcomp/ispc_texcomp.h` | Public API |
| `ispc_texcomp/ispc_texcomp.cpp` | C++ dispatch layer |
| `ispc_texcomp/kernel.ispc` | ISPC kernels for BC1–BC7 |
| `ispc_texcomp/kernel_astc.ispc` | ISPC kernels for ASTC |
| `ispc_texcomp/ispc_texcomp_astc.cpp` | ASTC C++ glue |

**Key considerations for translation**:
- The ISPC kernels (`kernel.ispc`) use Intel's ISPC language (C-like with SIMD extensions)
- A pure Rust translation would need to either: (a) rewrite the ISPC kernels as explicit SIMD in Rust, or (b) implement the scalar fallback algorithms
- The library is archived/unmaintained, but the algorithms are well-documented in the accompanying PDF (`ISPC Texture Compressor.pdf`)

#### 2.2 Microsoft DirectXTex

<https://github.com/microsoft/DirectXTex>

| Property | Value |
|---|---|
| **License** | MIT |
| **Language** | C++ (78.7%), C (11.9%), HLSL (6.3%) |
| **Stars** | ~2.1k |
| **Status** | Active (latest release May 2026) |

##### Key files for BC6H/BC7

| File | Purpose |
|---|---|
| `DirectXTex/BC6HBC7.cpp` | Encoder + decoder for BC6H and BC7 |
| `DirectXTex/BC.h` | BC block common definitions |
| `DirectXTex/BC4BC5.cpp` | BC4/BC5 encoders |
| `DirectXTex/BCDirectCompute.cpp` | GPU compute path via DirectCompute |
| `DirectXTex/DirectXTexCompress.cpp` | Compression dispatch |
| `DirectXTex/DirectXTex.h` | Public API |

**Dependency structure**:
- Core library (`DirectXTex/`) depends on Windows headers for WIC, but the BC compression modules (`BC6HBC7.cpp`, `BC.cpp`, `BC4BC5.cpp`, `BC.h`) are largely portable C++ with minimal platform dependencies
- The `BC6HBC7.cpp` file is ~7,000 lines and contains both encoder and decoder
- The decoder is straightforward; the encoder is more complex with quality heuristics
- Also includes a GPU compute path via DirectCompute compute shaders

#### 2.3 `bc7enc16` (Rich Geldreich)

<https://github.com/richgel999/bc7enc16>

| Property | Value |
|---|---|
| **License** | MIT or Public Domain (dual-licensed) |
| **Language** | C (encoder), C++ (decoder) |
| **Stars** | ~154 |
| **Supports** | BC7 modes 1 and 6 only (opaque + basic alpha) |

**Why it matters for Rust translation**:
- Single-file encoder (`bc7enc16.c`) — ~1,400 lines of plain C
- Single-file decoder (`bc7decomp.c`) — compact and easy to translate
- Perceptual color metrics support (weighted YCbCr)
- Intentionally limited to modes 1 and 6, which are the most commonly used
- Excellent quality-vs-speed tradeoff
- MIT/Public Domain means no attribution concerns

#### 2.4 `bc7enc_rdo` (Rich Geldreich)

<https://github.com/richgel999/bc7enc_rdo>

| Property | Value |
|---|---|
| **License** | MIT or Public Domain (dual-licensed) |
| **Language** | C++, ISPC |
| **Stars** | ~272 |
| **Supports** | BC1–BC7, with RDO post-processing |

**Key features**:
- Full BC7 support (all modes, BC7E ISPC encoder) plus `bc7enc.cpp` (4-mode CPU encoder)
- RDO (Rate Distortion Optimization) post-process for any block-based format (including ETC, ASTC)
- BC1–BC5 encoders in `rgbcx.cpp`

#### 2.5 `rgbcx` (inside `bc7enc_rdo`)

<https://github.com/richgel999/bc7enc_rdo/blob/master/rgbcx.cpp>

- Single-file BC1–BC5 encoder/decoder in portable C++
- MIT/Public Domain license
- Useful for BC1 (DXT1) and BC3 (DXT5) encoding

---

## 3. ETC2 / EAC

### Official Specification

- **Khronos Data Format Specification v1.4.0** — Section 22 ("ETC2 Compressed Texture Image Formats")
  - <https://www.khronos.org/registry/DataFormat/specs/1.4/dataformat.1.4.html#ETC2>
  - Covers: RGB ETC2 (individual, differential, T-mode, H-mode, planar), RGBA ETC2, R11/RG11 EAC (signed & unsigned), punchthrough alpha, and ETC1S.

### Reference Implementations

#### 3.1 `etcpak` (Bartosz Taudul / wolfpld)

<https://github.com/wolfpld/etcpak>

| Property | Value |
|---|---|
| **License** | 3-Clause BSD |
| **Language** | C++ (77.7%), C (12.8%), CMake (6.4%), Python (3.1%) |
| **Stars** | ~321 |
| **Status** | Active (latest release v2.1, Feb 2026) |

**Supported formats**:
- ETC1, ETC2, BC1, BC3, BC7
- The **fastest** known ETC compressor (797 Mpx/s single-threaded, 9613 Mpx/s multi-threaded for ETC1)
- Both encoder and decoder included

**Key files for ETC2/EAC**:

| File | Purpose |
|---|---|
| `ProcessRGB.cpp` / `.hpp` | ETC1/ETC2 RGB encoder |
| `ProcessDxtc.cpp` / `.hpp` | BC1/BC3 encoder |
| `Decode.cpp` / `.hpp` | Decompression (ETC1, ETC2, BC1, BC3, BC7) |
| `bc7enc.cpp` / `bc7enc.h` | BC7 encoder (bundled) |
| `bcdec.c` / `bcdec.h` | BC decompression library (BC1–BC7) |
| `BlockData.cpp` / `.hpp` | Block data types |
| `Tables.cpp` / `.hpp` | Look-up tables for ETC encoding |
| `ColorSpace.cpp` / `.hpp` | Color space conversion |

**Algorithmic features**:
- Extremely fast (heuristic-based, not exhaustive search)
- ETC2 individual, differential, T-mode, H-mode, planar encoding
- R11/RG11 EAC via ETC2 alpha channel machinery
- Quality is lower than ARM's reference encoder but acceptable for real-time content

#### 3.2 AOSP ETC2 (Android)

<https://android.googlesource.com/platform/frameworks/native/+/refs/heads/main/libs/etl/>

The Android Open Source Project includes ETC2 decode support in its native graphics libraries. The code is part of `frameworks/native/libs/etl/` and is licensed Apache 2.0. It is a decoder-only implementation used for loading ETC2 textures at runtime.

*(Note: A separate repo `platform/external/astc-codec/` exists for ASTC decoding.)*

#### 3.3 Mali Texture Compression Tool (deprecated)

ARM's legacy tool. No longer actively maintained. The `astcenc` project supersedes ARM's texture compression offerings.

#### 3.4 ETC1S (subset for Basis Universal)

ETC1S is a constrained subset of ETC1 used by Basis Universal for supercompression:
- Differential mode only (`diff bit` = 1)
- Color deltas are zero: `Rd = Gd = Bd = 0` (so both subblocks share the same base color)
- Same table codeword for both subblocks
- Flip bit = 0

Khronos Data Format spec Section 21.1 and Basis Universal source both serve as references.

---

## 4. UASTC

### Official Specification

- **Khronos Data Format Specification v1.4.0** — Section 25 ("Universal ASTC Compressed Texture Images")
  - <https://www.khronos.org/registry/DataFormat/specs/1.4/dataformat.1.4.html#UASTC>
  - Complete bitstream specification, all 19 modes, partition tables, endpoint unquantization tables, weight encoding, and transcoding procedures.

### Primary Reference Implementation

**Basis Universal** (<https://github.com/BinomialLLC/basis_universal>) v2.5

| Property | Value |
|---|---|
| **License** | Apache 2.0 |
| **Language** | C++ (75.5%), C (20.3%) |
| **Stars** | ~3.1k |
| **Status** | Active |

**Key files**:
- `encoder/basisu_uastc_enc.cpp` — UASTC encoder
- `transcoder/basisu_transcoder.cpp` — UASTC → BC7/ASTC/ETC transcoder (single .cpp file, no external deps)
- `transcoder/basisu_transcoder.h` — Transcoder public API

The UASTC specification was created by Rich Geldreich and placed in the public domain.

---

## 5. Basis Universal

<https://github.com/BinomialLLC/basis_universal>

While not a single-format reference implementation, Basis Universal v2.5 is the most comprehensive open-source texture compression system, supporting:

| Codec | Description |
|---|---|
| ETC1S | Supercompressed ETC1 subset, high compression |
| UASTC LDR 4×4 | Custom ASTC-like format for fast transcoding |
| UASTC HDR 4×4 | Constrained ASTC HDR for fast transcoding to BC6H |
| ASTC HDR 6×6 | Standard ASTC HDR |
| ASTC LDR 4×4–12×12 | All 14 standard block sizes |
| XUASTC LDR | Supercompressed ASTC with Weight Grid DCT |
| XUBC7 | Supercompressed BC7 |

**Transcoder** (`transcoder/basisu_transcoder.cpp`): Single-file, no external dependencies, supports all transcoding paths.

---

## 6. Bit-Layout Specifications

The single authoritative document for all compressed texture bit layouts is:

### Khronos Data Format Specification v1.4.0

<https://www.khronos.org/registry/DataFormat/specs/1.4/dataformat.1.4.html>

This document provides:

- **Section 18**: S3TC (BC1/BC2/BC3) — bit layouts, endpoint interpolation tables, alpha encoding
- **Section 19**: RGTC (BC4/BC5) — signed & unsigned variants
- **Section 20**: BPTC (BC6H/BC7) — all 8 BC7 modes with bit positions, 14 BC6H modes, partition tables, anchor indices, interpolation weights
- **Section 21**: ETC1 — individual & differential modes, modifier tables, pixel index mapping
- **Section 22**: ETC2 — all 5 modes (individual, differential, T, H, planar), alpha encoding, EAC (R11/RG11), punchthrough alpha
- **Section 23**: ASTC — complete decode procedure, block modes, integer sequence encoding, endpoint unquantization, 16 color endpoint modes, partition generation via hash52, weight infill via bilinear/simplex interpolation
- **Section 25**: UASTC — 19 modes, solid color blocks, endpoint/weight formats, partition tables, transcoding to ASTC and BC7

Each format section includes example Khronos Data Format Descriptor blocks showing how the format is described in the DFD model.

---

## 7. Testing & Reference Data

### Known compressed blobs with known RGBA output

| Tool | Description |
|---|---|
| **ARM `astcenc`** | `-tl` mode compresses then decompresses, printing PSNR. Can generate reference `.astc` files. |
| **Basis Universal** | `-test` runs automated encoding/transcoding tests. `-test_hdr_4x4`, `-test_hdr_6x6`, `-test_xuastc_ldr` for specific codecs. |
| **Microsoft DirectXTex** | `Texconv` and `Texdiag` tools for format conversion and analysis. |
| **AMD Compressonator** | GUI and CLI for BC compression/decompression with PSNR measurement. |
| **PVRTexTool** | Imagination Tech's texture tool for PVRTC and other format analysis. |
| **RenderDoc** | Frame debugger with texture viewer supporting BC1–7 formats. |

### Generating reference test vectors

1. Create a known RGBA image (e.g., gradient, checkerboard, solid colors)
2. Compress with a reference encoder (e.g., `astcenc -cl input.png output.astc 4x4 -exhaustive`)
3. The compressed `.astc`/`.dds` file is the known compressed blob
4. Decompress using the same tool's test mode to get the expected RGBA output
5. Compare with the Rust implementation's output (pixel-exact for lossless decode)

For ASTC, bit-exact decode is required by the specification. Any conformant decoder must produce identical pixel values for the same input block.

### Test images

- **Kodak Lossless True Color Image Suite** — 24 commonly used test images
- **Basis Universal test corpus** — `test_files/` directory in the repo
- **ARM astcenc test suite** — `Test/` directory with unit tests

---

## 8. Summary Table

| Format | Spec Source | Primary C++ Ref | License | Code Size | Key Features |
|---|---|---|---|---|---|
| **ASTC** | KDF §23 | ARM `astcenc` | Apache 2.0 | ~50K lines / ~25K core | 14 block sizes, 16 CEMs, HDR/LDR, 3D, 4 partitions, dual-plane, BISE |
| **BC6H** | KDF §20.2 | DirectXTex `BC6HBC7.cpp` | MIT | ~7K lines (BC6H+BC7) | 14 modes, signed/unsigned HDR, shared exponent, partition-based |
| **BC7** | KDF §20.1 | DirectXTex `BC6HBC7.cpp` | MIT | (same file as BC6H) | 8 modes, 1–3 subsets, 3-bit to 4-bit indices, p-bits |
| **BC7 (fast)** | — | `bc7enc16` `bc7enc16.c` | MIT/Public Domain | ~1,400 lines | Modes 1 & 6 only, perceptual metrics, very fast |
| **BC7 (full)** | — | `bc7enc_rdo` `bc7e.ispc` | MIT/Public Domain | ~5K lines | All modes, RDO, ISPC-accelerated |
| **ETC1** | KDF §21 | `etcpak` `ProcessRGB.cpp` | BSD-3 | ~2K lines | 2 subblock modes, individual/differential, 8 modifier tables |
| **ETC2** | KDF §22 | `etcpak` `ProcessRGB.cpp` | BSD-3 | ~3K lines | T/H/planar modes, punchthrough alpha |
| **EAC R11/RG11** | KDF §22.5–22.8 | `etcpak` `ProcessRGB.cpp` | BSD-3 | ~1K lines | 11-bit signed/unsigned, modifier tables |
| **UASTC** | KDF §25 | Basis Universal `basisu_uastc_enc.cpp` | Apache 2.0 | ~8K lines | 19 modes, transcode hints, BC1/ETC hints |
| **Cross-format** | — | Basis Universal `basisu_transcoder.cpp` | Apache 2.0 | Single file (~12K lines) | Transcodes UASTC/ASTC → BC7/ETC/PVRTC |

### Key Insights for Rust Translation

1. **Start with the decoder**: For any format, implement the decoder first. The specifications (KDF) provide complete decode procedures. This gives you a correctness baseline before tackling encoder heuristics.

2. **`astcenc` is the most complex**: The ASTC encoder is large and heavily SIMD-optimized. A pure-Rust port would benefit from focusing on the scalar algorithms first, then adding SIMD via `core::simd` or `wide`/`safe_arch`.

3. **BC6H/BC7 have clean C++ refs**: DirectXTex's `BC6HBC7.cpp` is well-structured and relatively portable. The `bc7enc16` single-file C encoder is the easiest starting point.

4. **`etcpak` is optimal for ETC2/EAC**: It is the fastest encoder and has a clean BSD license. The decoder in `Decode.cpp` and `bcdec.c` is straightforward.

5. **Basis Universal's transcoder is single-file**: The `basisu_transcoder.cpp` file has zero external dependencies and handles all transcoding paths. It is the most practical reference for a UASTC pipeline.

6. **The KDF spec is the ground truth**: All implementations must match the Khronos Data Format specification. When implementations disagree, the KDF spec wins.

---

## Sources Cited

- Khronos Data Format Specification v1.4.0: <https://www.khronos.org/registry/DataFormat/specs/1.4/dataformat.1.4.html>
- ARM astc-encoder: <https://github.com/ARM-software/astc-encoder> (Apache 2.0)
- Intel ISPC Texture Compressor: <https://github.com/GameTechDev/ISPCTextureCompressor> (MIT, archived)
- Microsoft DirectXTex: <https://github.com/microsoft/DirectXTex> (MIT)
- etcpak: <https://github.com/wolfpld/etcpak> (BSD-3)
- Basis Universal: <https://github.com/BinomialLLC/basis_universal> (Apache 2.0)
- bc7enc16: <https://github.com/richgel999/bc7enc16> (MIT/Public Domain)
- bc7enc_rdo: <https://github.com/richgel999/bc7enc_rdo> (MIT/Public Domain)
- Google ASTC Codec: <https://android.googlesource.com/platform/external/astc-codec/> (Apache 2.0)
