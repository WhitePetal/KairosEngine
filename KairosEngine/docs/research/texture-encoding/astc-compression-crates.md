# ASTC Texture Compression in Rust — Crates Survey

**Date:** 2026-07-21  
**Purpose:** Evaluate existing Rust crates for ASTC (and related BCn/ETC2) texture compression/decompression for use in KairosEngine.

---

## 1. `astc-encoder` — Does Not Exist on crates.io

The crate name `astc-encoder` does not exist on crates.io (404 from the API). It has never been published. The closest names on crates.io are:

| Crate | Exists? | Description |
|---|---|---|
| `astc-encoder` | **No** | Name not registered |
| `astc-rs` | **No** | Name not registered |
| `astc-decoder` | **No** | Name not registered |
| `astc-decode` | **Yes** (v0.3.1) | Pure Rust decoder only (see §2) |
| `astcenc-rs` | **Yes** (v0.2.0) | Rust bindings to ARM C++ library (see §3) |
| `astcenc-sys` | **Yes** (v0.2.0) | Low-level C++ bindings (see §3) |
| `ctt-astcenc` | **Yes** (v0.5.0) | Vendored C++ astcenc via `ctt` (see §4) |

**Sources:**
- https://crates.io/api/v1/crates/astc-encoder (404)
- https://crates.io/api/v1/crates?q=astc&per_page=20

---

## 2. `astc-decode` — Pure Rust ASTC **Decoder** Only

| Property | Value |
|---|---|
| **Latest version** | 0.3.1 (2021-06-19) |
| **Author** | Weiyi Wang (wwylele) |
| **License** | Apache-2.0 |
| **Nature** | **Pure Rust**, no external dependencies, no C++ bindings |
| **Encoding?** | **No** — decoder only |
| **Decoding?** | **Yes** — LDR profile only |
| **HDR?** | **No** — `void_extent_hdr` returns error/fill_error (confirmed from source) |
| **Block sizes** | All standard ASTC 2D block sizes: 4x4, 5x4, 5x5, 6x5, 6x6, 8x5, 8x6, 8x8, 10x5, 10x6, 10x8, 10x10, 12x10, 12x12 |
| **no_std?** | **No** — uses `std::io::Read` and `std::convert::TryFrom` |
| **Dependencies** | Zero (except `image` and `criterion` for dev/testing) |
| **Code size** | ~1,292 lines of Rust in a single file |
| **Crates.io** | https://crates.io/crates/astc-decode |
| **Repository** | https://github.com/wwylele/astc-decode |

**Key detail from source code (`src/lib.rs`):**
```rust
if weight_params.void_extent_hdr {
    fill_error(&mut writer, block_width, block_height);
    return false;
}
```
Also endpoint mode branches only handle modes 0, 1, 4, 5, 6, 8, 9, 10, 12, 13 — modes for HDR (e.g. 2, 3, 7, 11, 14, 15) fall through to `unsupported HDR modes` producing magenta error pixels.

**Verdict for KairosEngine:** Good for LDR-only decoding; unsuitable for encoding or HDR decoding.

---

## 3. `astcenc-rs` / `astcenc-sys` — C++ Bindings to ARM's astcenc

### `astcenc-rs` (v0.2.0)

| Property | Value |
|---|---|
| **Nature** | **Rust bindings** to ARM's C++ `astcenc` library |
| **License** | Unlicense |
| **Encoding?** | **Yes** (via ARM's encoder) |
| **Decoding?** | **Yes** (via ARM's decoder) |
| **Block sizes** | All (delegated to ARM library) |
| **HDR** | Yes (delegated) |
| **Dependencies** | `astcenc-sys` (the C++ glue), `num_cpus`, `bitflags`, `half` |
| **Repository** | https://github.com/eira-fransham/astcenc-rs |

### `astcenc-sys` (v0.2.0)

| Property | Value |
|---|---|
| **Nature** | **Low-level C++ bindings** via `*-sys` pattern |
| **License** | Unlicense |
| **Platform** | **Linux-only** (uses GNU Make build system) |
| **Code** | 40,746 lines C + 20,121 lines C++ + 22,067 lines C headers = **~83K lines** of native code |
| **Repository** | https://github.com/Vurich/astcenc-sys |

**Key quote from crates.io description:**
> "Low-level bindings to the official ARM ASTC encoding library (currently Linux-only due to use of GNU Make)"

**Verdict for KairosEngine:** Full-featured (enc+dec, LDR+HDR, all block sizes) but binds to 83K lines of C/C++ code and is Linux-only. Not suitable for cross-platform or pure-Rust requirements.

**Sources:**
- https://crates.io/crates/astcenc-rs
- https://crates.io/crates/astcenc-sys
- https://github.com/eira-fransham/astcenc-rs
- https://github.com/Vurich/astcenc-sys

---

## 4. `ctt` Project (cwfitzgerald) — Unified C++ Binding Umbrella

The [ctt](https://github.com/cwfitzgerald/ctt) project provides a unified API over multiple C/C++ encoder backends, each as its own crate:

| Sub-crate | Native Backend | Formats |
|---|---|---|
| `ctt-astcenc` | ARM's `astcenc` (vendored C++) | ASTC (all) |
| `ctt-bc7enc-rdo` | `bc7enc_rdo` + ISPC `bc7e` (vendored) | BC7 |
| `ctt-intel-texture-compressor` | Intel ISPC TC (vendored) | BC1, BC3, BC4, BC5, BC6H, BC7, ETC1 |
| `ctt-etcpak` | `etcpak` (vendored C++) | ETC1, ETC2 RGBA, EAC R, EAC RG, BC1, BC3, BC4, BC5 |
| `ctt-compressonator` | AMD Compressonator CMP_Core (vendored) | BC1-BC7 |
| `ctt` (umbrella) | All of the above | All of the above |

**Key facts:**
- All sub-crates are **vendored C/C++ bindings** — none are pure Rust
- `ctt` umbrella crate at v0.5.0, MSRV 1.90, edition 2024
- License: MIT OR Apache-2.0 OR Zlib
- Ships prebuilt ISPC static libraries; needs a C++ compiler for the C++ backends
- Feature: `rayon` for parallel compression
- Output containers: KTX2 and DDS

**Verdict for KairosEngine:** The most comprehensive solution, but entirely C++ binding based. Best option if native bindings are acceptable.

**Source:** https://github.com/cwfitzgerald/ctt

---

## 5. `texture2ddecoder` — Pure Rust, no_std Texture Decoder

| Property | Value |
|---|---|
| **Latest version** | 0.1.2 (2025-03-21) |
| **License** | MIT OR Apache-2.0 |
| **Nature** | **Pure Rust, no_std** (with `alloc` feature) |
| **Encoding?** | **No** |
| **Decoding?** | **Yes** — extensive format support |
| **Formats decoded** | ATC, **ASTC**, BC1-BC7, BC6 (signed+unsigned), ETC1, ETC2 RGB/RGBA1/RGBA8, EAC R/RG (signed+unsigned), PVRTC, Crunch |
| **ASTC detail** | `decode_astc()` plus specific `decode_astc_4_4()` through `decode_astc_12_12()` variants |
| **Dependencies** | `paste` (only runtime dependency) |
| **Code size** | ~7,666 lines of Rust across 33 files |
| **Crates.io** | https://crates.io/crates/texture2ddecoder |
| **Repository** | https://github.com/UniversalGameExtraction/texture2ddecoder |

**Note:** All decoding functions output `&mut [u32]` (packed RGBA), not individual channels.

**Verdict for KairosEngine:** Excellent pure-Rust, no_std decoder for ASTC and many other formats. But decoding-only — no encoding support at all.

**Source:** https://docs.rs/texture2ddecoder/0.1.2/texture2ddecoder/index.html

---

## 6. `bcdec_rs` — Pure Rust BCn Decoder (No ASTC)

| Property | Value |
|---|---|
| **Latest version** | 0.2.0 |
| **Nature** | **Pure Rust, no_std** — safe port of the `bcdec` C library |
| **Formats** | BC1, BC2, BC3, BC4, BC5, **BC6H**, **BC7** |
| **ASTC?** | **No** |
| **Encoding?** | **No** |
| **Repository** | https://github.com/ScanMountGoat/image_dds/tree/master/bcdec_rs |

Part of the `image_dds` workspace. Fuzzed against the original C code for bit-exact behavior.

**Source:** https://raw.githubusercontent.com/ScanMountGoat/image_dds/master/bcdec_rs/README.md

---

## 7. `tbc` — Pure Rust BCn Encoder/Decoder (No ASTC)

| Property | Value |
|---|---|
| **Latest version** | 0.3.0 |
| **License** | MIT |
| **Nature** | **Pure Rust** — both encoding and decoding |
| **Formats** | BC1 (DXT1), BC3 (DXT5), BC4 (R8 + RG8) |
| **BC6H/BC7?** | **No** |
| **ASTC?** | **No** |
| **ETC2?** | **No** |
| **Dependencies** | None |
| **Repository** | https://github.com/mrDIMAS/tbc |

**Verdict for KairosEngine:** Pure Rust but very limited format support (BC1/BC3/BC4 only). Not useful for ASTC, BC6H, BC7, or ETC2.

**Source:** https://github.com/mrDIMAS/tbc

---

## 8. `bcn` — Placeholder Crate (Do Not Use)

| Property | Value |
|---|---|
| **Latest version** | 0.1.1 |
| **License** | MIT OR Apache-2.0 |
| **Code** | 249 lines of Rust across 2 files |
| **Dependencies** | None |
| **ASTC?** | **No** |
| **BC6H/BC7?** | **No** |
| **ETC2?** | **No** |

This crate is a skeleton/placeholder. The 0.1.0 version description was "Work in progress" and 0.1.1 says "Texture Block Compression" but has no actual compression implementation. **Do not use.**

**Source:** https://crates.io/crates/bcn

---

## 9. `intel_tex_2` — C++/ISPC Bindings (BC6H, BC7, ETC1, ASTC WIP)

| Property | Value |
|---|---|
| **Latest version** | 0.5.0 |
| **License** | MIT / Apache-2.0 |
| **Nature** | **Rust bindings** to Intel's ISPC texture compressor (C++) |
| **Encoding** | BC1, BC3, BC4, BC5, BC6H, BC7, ETC1 |
| **ASTC** | "Work in progress" — LDR only, block sizes up to 8x8 |
| **Decoding** | No (encoding only) |
| **Repository** | https://github.com/Traverse-Research/intel-tex-rs-2 |

Prebuilt ISPC binaries included for convenience. Used by `image_dds` for its `encode` feature.

**Verdict for KairosEngine:** Good for BC6H/BC7/ETC1 encoding if native bindings are OK. ASTC support is incomplete/WIP.

**Source:** https://github.com/Traverse-Research/intel-tex-rs-2

---

## 10. ARM's Official `astcenc` (C++ Reference Encoder)

The upstream C++ project that the Rust bindings wrap:

| Property | Value |
|---|---|
| **Repository** | https://github.com/ARM-software/astc-encoder |
| **License** | Apache-2.0 |
| **Language** | C++ (87%), Python (9.8%), CMake (2.9%) |
| **Latest release** | 5.6.0 (2026-07-01) |
| **Features** | Full ASTC support: LDR, HDR, all block sizes, 3D, sRGB, quality presets, SIMD (SSE2/SSE4.1/AVX2/NEON/SVE) |
| **Decoding** | Yes |
| **Building** | CMake — cross-platform (Windows, macOS, Linux) |

This is the authoritative ASTC encoder. All Rust crates that offer ASTC encoding (`astcenc-rs`, `ctt-astcenc`) bind to this library.

**Source:** https://github.com/ARM-software/astc-encoder

---

## 11. The `image` crate — ASTC Support

The Rust `image` crate (v0.25+) supports **decoding** ASTC textures via KTX containers. It depends on `astc-decode` (wwylele) internally for ASTC decoding. It does **not** support ASTC encoding.

---

## 12. Summary Table

| Crate | Pure Rust? | Encode | Decode | ASTC | BC6H | BC7 | ETC2/EAC | no_std | License |
|---|---|---|---|---|---|---|---|---|---|
| `astc-decode` | ✅ | ❌ | ✅ LDR only | ✅ | ❌ | ❌ | ❌ | ❌ | Apache-2.0 |
| `astcenc-rs` | ❌ (C++ bind) | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | Unlicense |
| `astcenc-sys` | ❌ (C++ bind) | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | Unlicense |
| `ctt-astcenc` | ❌ (C++ bind) | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | MIT/Apache-2.0/Zlib |
| `ctt` (umbrella) | ❌ (C++ bind) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | MIT/Apache-2.0/Zlib |
| `texture2ddecoder` | ✅ | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ (alloc) | MIT/Apache-2.0 |
| `bcdec_rs` | ✅ | ❌ | ✅ | ❌ | ✅ | ✅ | ❌ | ✅ | MIT |
| `tbc` | ✅ | ✅ partial | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | MIT |
| `bcn` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | MIT/Apache-2.0 |
| `intel_tex_2` | ❌ (ISPC bind) | ✅ partial | ❌ | WIP | ✅ | ✅ | ETC1 only | ❌ | MIT/Apache-2.0 |
| `image_dds` | Partial (dec) | ❌ (C++ bind) | ✅ | ❌ | ✅ | ✅ | ❌ | ❌ | MIT |
| `etcpak` (via ctt) | ❌ (C++ bind) | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ | MIT/Apache-2.0/Zlib |

---

## 13. Conclusions for KairosEngine

### ASTC — Encoding
**No pure-Rust ASTC encoder exists.** Every available Rust ASTC encoder binds to ARM's C++ `astcenc` library:
- `astcenc-rs` / `astcenc-sys` — simple bindings, but `astcenc-sys` is Linux-only
- `ctt-astcenc` — vendored C++, cross-platform, part of the `ctt` ecosystem
- If the project is willing to accept C++ bindings, **`ctt`** is the recommended choice (most maintained, cross-platform, unified API across many formats)

If pure-Rust ASTC encoding is required, it would need to be written from scratch (a very large undertaking — ARM's reference encoder is ~34K lines of C++).

### ASTC — Decoding
**Two excellent pure-Rust options:**
1. **`astc-decode`** — LDR-only, zero dependencies, Apache-2.0, but not no_std
2. **`texture2ddecoder`** — LDR + HDR through BC6H path (ASTC still LDR-only from its code), no_std, supports many more formats (BCn, ETC2, PVRTC, Crunch)

### BC6h / BC7 — Encoding
- **Pure Rust:** None available. `tbc` only supports BC1/BC3/BC4.
- **C++ bindings:** `intel_tex_2` (Intel ISPC) or `ctt-bc7enc-rdo` (bc7enc_rdo)

### BC6h / BC7 / ETC2 — Decoding
- **Pure Rust:** `texture2ddecoder` and `bcdec_rs` both support all BCn formats including BC6H/BC7. `texture2ddecoder` additionally supports ETC2/EAC.

### ETC2/EAC — Encoding
- **Pure Rust:** None available.
- **C++ bindings:** `ctt-etcpak` (vendored etcpak) or `intel_tex_2` (ETC1 only, not ETC2).

### Recommended Approach

| Need | Best Option | Pure? |
|---|---|---|
| **ASTC encode** | `ctt-astcenc` or `ctt` umbrella | ❌ (C++ vendored) |
| **ASTC decode (LDR)** | `texture2ddecoder` | ✅ |
| **BC6H/BC7 decode** | `texture2ddecoder` or `bcdec_rs` | ✅ |
| **BC6H/BC7 encode** | `ctt-bc7enc-rdo` / `intel_tex_2` | ❌ (C++ vendored) |
| **ETC2/EAC decode** | `texture2ddecoder` | ✅ |
| **ETC2/EAC encode** | `ctt-etcpak` | ❌ (C++ vendored) |

If the team accepts C++/native dependencies, **`ctt`** provides the most complete solution with a unified API. If pure Rust is mandatory, encoding of ASTC/BC6H/BC7/ETC2 will require either writing a new encoder or using an external tool as a build step.
