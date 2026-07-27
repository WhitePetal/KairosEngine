# 纹理编解码实现计划 — 文档索引

## 术语表

**文件：** `../CONTEXT.md`

定义了 PixelDatas、Encode/Decode、SDR/HDR、source_srgb、Output color space 等 9 个核心术语。

---

## ADR（架构决策记录）

### ADR-0001：PixelDatas enum wraps whole arrays, not per-element variants

**文件：** `../adr/texture-encoding/0001-pixeldata-enum-over-arrays.md`

PixelDatas 枚举包装整个 `Vec<T>` 缓冲区（U8/F16/F32），而非逐像素枚举。这样 heap 内存大小不变，stack overhead 仅每个 mip level ~32 bytes，可以零开销 reinterpret 为 `&[u8]` 传给 wgpu。

---

### ADR-0002：sRGB conversion is handled inside encode via `source_srgb` flag

**文件：** `../adr/texture-encoding/0002-srgb-handling-in-encode.md`

wgpu 的 `queue.write_texture` 不做任何 sRGB 转换——字节原样存储。encode 内部通过 `(source_srgb, target_is_srgb)` 决定是否 gamma 校正。Decode 始终返回 linear 数据。

---

### ADR-0003：Encode/Decode implementation plan — 6 groups, ordered by dependency

**文件：** `../adr/texture-encoding/0003-encode-decode-implementation-plan.md`

完整实现计划，覆盖：
- 技术选型（F16、sRGB LUT、压缩算法来源、序列化等）
- 实现细节（每个组的格式清单、边界处理）
- 测试策略（golden data + random roundtrip + 多版本 benchmark 对比）
- 排期原则（A→F 顺序，每组交付后立即更新 `supports_encoding()`）

---

## 研究文档

### wgpu sRGB 行为

**文件：** `../research/texture-encoding/wgpu-srgb-handling.md`

wgpu 在 upload 时不做 sRGB 转换。sRGB↔linear 只在 shader 采样时由 GPU 硬件完成。压缩格式的 Unorm 和 Srgb 变体 block 数据是 byte-identical 的。

### PNG 色彩空间

**文件：** `../research/texture-encoding/png-srgb-color-space.md`

`image::open()` 不做色彩空间转换——返回的是 PNG 文件中的原始 sRGB 编码字节。当前引擎用 `Rgba8Unorm` 存储 sRGB 数据是色彩空间不匹配，需要在纹理编码工作中一并修复。

### ASTC 压缩 Rust crate 生态

**文件：** `../research/texture-encoding/astc-compression-crates.md`

纯 Rust ASTC encoder **不存在**（所有 encoder 都是 C++ binding），但 decoder 有纯 Rust 实现。对于 BC6h/BC7/ETC2 也是类似情况。决策：全自实现，参考 C++ 库翻译。

### C++ 纹理压缩参考实现

**文件：** `../research/texture-encoding/texture-compression-reference-impls.md`

| 格式 | 参考实现 | License | 代码量 |
|------|---------|---------|:------:|
| BC6h/BC7 | DirectXTex `BC6HBC7.cpp` | MIT | ~7K 行 |
| BC7 快速 | `bc7enc16.c` | MIT/Public Domain | ~1.4K 行 |
| ETC2/EAC | etcpak (ProcessRGB.cpp, Decode.cpp) | BSD-3 | ~3K 行 |
| ASTC | ARM `astcenc` C++ | Apache 2.0 | ~50K 行 |
| ASTC decode (LDR) | Google `astc-codec` | Apache 2.0 | 更简洁 |

---

## 性能指引

**文件：** `../performance/texture-encode-decode.md`

包含性能准则和 benchmark 路径。

---

## 核心 API 签名

```rust
// encode/decode 入口 (format.rs)
pub fn encode(pixels: &PixelDatas, width: u32, height: u32,
              format: TextureFormat, source_srgb: bool) -> Vec<u8>;

pub fn decode(data: &[u8], width: u32, height: u32,
              format: TextureFormat) -> PixelDatas;

// 数据容器
pub enum PixelDatas {
    U8(Vec<u8>),
    F16(Vec<u16>),
    F32(Vec<f32>),
}

// Texture 结构体
pub struct Texture {
    pub data: Vec<PixelDatas>,  // 每个 mip level 一个
    // ... width, height, format, sampler
}
```

## 执行顺序

| 序 | 组 | 格式数 | 参考 | 工作量 |
|:--:|:--:|:------:|------|:------:|
| 1 | **A**: Uncompressed SDR | ~12 | channel swizzle | 小 |
| 2 | **B**: Uncompressed 宽格式 | ~9 | half crate + zero-extend | 中 |
| 3 | **C**: Uncompressed packed+f32 | ~9 | 位操作 + half crate | 中 |
| 4 | **D**: BC6h + BC7 | 2(6) | DirectXTex / bc7enc16.c | 中~高 |
| 5 | **E**: ETC2 + EAC | 10 | etcpak | 高 |
| 6 | **F**: ASTC | 36 | ARM astcenc | 极高 |

## 已知问题

以下问题需要在纹理编解码工作中一并修复：

- 当前 pipeline 将 PNG 的 sRGB 编码字节以 `Rgba8Unorm` 格式存入 `.texture_bin` — 这是色彩空间不匹配：GPU 将 sRGB 数据当作 linear 处理，导致渲染结果不正确
- 修复方案：对颜色纹理使用 `Rgba8UnormSrgb`，或在存储前将 sRGB→linear 转换
