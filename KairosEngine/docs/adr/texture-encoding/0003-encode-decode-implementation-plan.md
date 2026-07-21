# ADR-0003: Encode/Decode implementation plan — 6 groups, ordered by dependency

## 功能分组与执行顺序

### 前提条件（所有组之前）
- 定义 `PixelData` enum（U8/F16/F32）
- 修改 `Texture.data` 为 `Vec<PixelData>`
- 实现 sRGB 转换工具函数
- 更新 `TextureFormat::encode_rgba` / `decode_to_rgba8` 签名为新 API
- 定义 `format::uncompressed.rs` 中的通用辅助函数（channel extract/insert）

### Group A：补齐 Uncompressed SDR（~12 格式）
**格式：**
- `Rg8Unorm`, `Rg8Snorm`, `Rg8Uint`, `Rg8Sint`
- `Rgba8Snorm`, `Rgba8Uint`, `Rgba8Sint`
- `Bgra8Unorm`, `Bgra8UnormSrgb`
- 复用 `Rgba8Unorm`/`Rgba8UnormSrgb` pass-through（已有）
**技巧点：** channel swizzle（RG↔RGBA, BGRA↔RGBA），复用 R8 模式的通道选择模式
**交付物：** `supports_encoding` 更新 + encode/decode + correctness tests + benchmark

### Group B：补齐 Uncompressed 宽格式（~9 格式）
**格式：**
- `R16Uint`, `R16Sint`, `R16Float`
- `Rg16Uint`, `Rg16Sint`, `Rg16Float`
- `Rgba16Uint`, `Rgba16Sint`, `Rgba16Float`
**技巧点：** f16 ↔ f32 转换（half crate），宽整数的 reinterpret cast
**依赖：** `PixelData::F16`
**交付物：** 同上

### Group C：补齐 Uncompressed packed + f32（~9 格式）
**格式：**
- `Rgb10a2Unorm`（packed）
- `Rg11b10Ufloat`（packed）
- `R32Uint`, `R32Sint`, `R32Float`
- `Rg32Uint`, `Rg32Sint`, `Rg32Float`
- `Rgba32Uint`, `Rgba32Sint`, `Rgba32Float`
**技巧点：** 位操作解包/打包，`PixelData::F32`
**交付物：** 同上

### Group D：BC6h + BC7（2 格式，6 变体）
**格式：**
- `Bc6hRgbUfloat`, `Bc6hRgbFloat`
- `Bc7RgbaUnorm`, `Bc7RgbaUnormSrgb`
**技巧点：** BC6h 是 HDR（输入 F16），BC7 算法复杂需要查表
**依赖：** 现有 `bc.rs` 的 block/parallel 基础设施
**交付物：** 同上

### Group E：ETC2 + EAC（10 格式）
**格式：**
- `Etc2Rgb8Unorm`, `Etc2Rgb8UnormSrgb`
- `Etc2Rgb8A1Unorm`, `Etc2Rgb8A1UnormSrgb`
- `Etc2Rgba8Unorm`, `Etc2Rgba8UnormSrgb`
- `EacR11Unorm`, `EacR11Snorm`
- `EacRg11Unorm`, `EacRg11Snorm`
**技巧点：** ETC2 分块编码，EAC 类似 BC4/5（单通道/双通道）
**交付物：** 同上

### Group F：ASTC（36 格式）
**格式：** 所有 Astc{4..12}x{4..12}{Unorm,UnormSrgb,Hdr} 共 36 变体
**技巧点：** weight grid 编码，color endpoint mode（LDR + HDR），partition 系统，整数序列编解码
**交付物：** 同上

## 技术选型记录

### F16 处理：使用 `half` crate
所有涉及 half-float 的格式（R16Float, Rg16Float, Rgba16Float, BC6h, ASTC HDR）统一使用 `half` crate 的 `f16` 类型。`Vec<half::f16>` 直接对应 `PixelData::F16`，reinterpret 为 `&[u8]` 后可安全传给 wgpu。

### 模块结构
```
texture/
├── format.rs           — TextureFormat + PixelData + encode()/decode() 入口
├── format/
│   ├── bc.rs           — 已有 BC1-5，扩展 BC6h/BC7
│   ├── etc.rs          — ETC2 + EAC
│   ├── astc.rs         — ASTC（如果太大可拆为 astc/ 目录）
│   ├── uncompressed.rs — 所有 uncompressed 格式
│   └── srgb.rs         — sRGB↔linear 转换工具
```

### Block 级并行处理模式
所有压缩格式共用同一套 rayon block 调度基础设施，核心算法在每个格式模块中手写。统一基础设施内置 false sharing mitigation：

```rust
// 编码模式
macro_rules! encode_blocks {
    ($name:ident, $block_w:expr, $block_h:expr, $block_size:expr, $block_fn:ident) => {
        pub fn $name(rgba: &PixelData, width: usize, height: usize) -> Vec<u8> {
            let bx = div_ceil(width, $block_w);
            let by = div_ceil(height, $block_h);
            let mut out = vec![0u8; bx * by * $block_size];
            out.par_chunks_mut($block_size)
                .enumerate()
                .for_each(|(i, chunk)| {
                    let bx_i = i % bx;
                    let by_i = i / bx;
                    let block = extract_block(rgba, width, height, bx_i * $block_w, by_i * $block_h);
                    chunk.copy_from_slice(&$block_fn(&block));
                });
            out
        }
    };
}

// 解码模式（带 false sharing 防护）
fn decode_blocks(
    data: &[u8],
    width: usize,
    height: usize,
    block_w: usize,
    block_h: usize,
    block_size: usize,
    decode: impl Fn(&[u8]) -> [u8; <MAX_BLOCK_PIXELS * 4>] + Sync,
) -> PixelData {
    let bx = div_ceil(width, block_w);
    let by = div_ceil(height, block_h);
    let out = vec![0u8; width * height * 4];
    
    (0..bx * by).into_par_iter().for_each(|i| {
        let bx_i = i % bx;
        let by_i = i / bx;
        let off = i * block_size;
        let pixels = decode(&data[off..off + block_size]);
        // 一次性写入整 block，减少 cache line 竞争
        write_block_to_output(&pixels, width, height, bx_i * block_w, by_i * block_h,
                              block_w, block_h, &mut out);
    });
    PixelData::U8(out)
}
```

关键优化点：
1. **chunks_mut** 天然让每个 rayon task 写一段连续的输出区域，减少与相邻线程的 cache line 冲突
2. **一次性写完整 block** 而非逐像素写 → 写合并 + 减少 MESI 消息
3. 大 block（ASTC 12×12）用 `par_chunks_mut(4096)` 级别的 chunk size 平衡负载
4. 对于小纹理，在 rayon 的 `min_len` 上做配置，避免并行开销超过收益

### 各压缩家族的实现策略
所有格式完全自实现纯 Rust encoder/decoder，不引入任何 C++ binding 依赖。

参考以下成熟 C++ 实现翻译为纯 Rust：

| 格式 | 参考实现 | License | 特点 |
|------|---------|---------|------|
| **BC6h/BC7** | DirectXTex `BC6HBC7.cpp` | MIT | ~7K 行，完整 encoder + decoder |
| **BC7 快速路径** | `bc7enc16.c` | MIT/Public Domain | ~1,400 行，modes 1&6 |
| **ETC2/EAC** | `etcpak` (ProcessRGB.cpp, Decode.cpp) | BSD-3 | ~3K 行，最快的已知 encoder |
| **ASTC** | ARM `astcenc` C++ 参考实现 | Apache 2.0 | ~50K 行，完整支持所有 block size/HDR |
| **ASTC decode (LDR)** | Google `astc-codec` | Apache 2.0 | 更干净的代码，LDR only |

格式规范遵循 **Khronos Data Format Specification**（KDF）§20-23。

### 压缩格式编码质量策略
第一版实现优先保证**足够好 + 快速**，不追求最高 PSNR。
- BC7 先实现常见 modes 子集（modes 1, 2, 3, 6, 7）
- BC6h 从 DirectXTex 翻译基础实现
- 后续可定义质量等级（Fast / Normal / High），每级用不同的算法

### ETC2/EAC 实现边界
所有 ETC2/EAC 变体放在 `format/etc.rs` 中：
- `Etc2Rgb8*`：RGB block（8 bytes），decode 时 alpha 填 255
- `Etc2Rgb8A1*`：RGB + 1-bit alpha mask，alpha 映射到 0/255
- `Etc2Rgba8*`：RGB block + EAC alpha block（8+8=16 bytes），全 8-bit alpha
- `EacR11*` / `EacRg11*`：类似 BC4/BC5 的单/双通道编码

### ASTC 实现范围
全部 14 种 block size（4×4 到 12×12）都需实现，覆盖 LDR（Unorm/Srgb）和 HDR。
放在 `format/astc.rs`（如果文件过大可拆为 `format/astc/` 目录）。

### Packed 格式实现（Rgb10a2Unorm, Rg11b10Ufloat）
**Rgb10a2Unorm**：4 bytes/pixel，bits 分布 R:10/G:10/B:10/A:2
- Encode: U8→U10 零填充 `(r << 2) - r >> 6)`，逐 bit 组合为 u32
- Decode: u10→u8 截断 `(r >> 2)`，u2→u8 `(a << 6)`
- 精度损失在 2-bit alpha 上是符合预期的

**Rg11b10Ufloat**：4 bytes/pixel，bits 分布 R:11/G:11/B:10（无符号浮点）
- Encode: PixelData::F16 → 提取 exponent/mantissa → RG11B10 Ufloat
- Decode: RG11B10 Ufloat → f16
- 参考实现：Khronos Data Format Specification §20.6

### Uncompressed 编码原则
encode/decode 对于 uncompressed 格式只做**通道布局变换**，不做数值解释。
- U8 输入中的字节值直接拷贝/swizzle 到目标布局
- Snorm 的 `128=0.0`、`0=-1.0`、`255=1.0` 是 GPU 采样时的解释，encode 不处理
- 这条原则适用于所有非压缩格式（Group A/B/C）

### 宽格式输入精度约定
| 输入 PixelData | 适用的格式类型 |
|:--------------:|---------------|
| `PixelData::U8` | 所有 8-bit/通道 SDR 格式 + 16-bit Uint/Sint（zero-extend） |
| `PixelData::F16` | 所有 f16 格式（R16F, Rg16F, Rgba16F, BC6h, ASTC HDR） |
| `PixelData::F32` | 原生 f32 格式（R32F, Rg32F, Rgba32F） |

16-bit 整数源（如 16-bit PNG）暂不支持，后续可加 `PixelData::U16` 变体。

### sRGB 转换实现方案
SDR 路径使用 256-entry u8 LUT（512 bytes，零运行时计算）：
- `sRGB_TO_LINEAR: [u8; 256]` — 8-bit sRGB → 8-bit linear
- `LINEAR_TO_SRGB: [u8; 256]` — 8-bit linear → 8-bit sRGB

HDR 数据（F16/F32）不涉及 sRGB 转换，`source_srgb` 通常为 false。
当 `source_srgb = true` 且数据为 `PixelData::F16` 时，不会 panic，但结果精度不保证——此组合在实际中不会出现。

### PixelData 序列化
`.texture_bin` 使用自定义二进制格式，不依赖 rkyv：
```
[mip_count: 4 bytes]       u32 LE
[mip0_size: 4 bytes]       u32 LE
[mip0_data: mip0_size bytes]
[mip1_size: 4 bytes]
[mip1_data: ...]
```
不需要 type tag——`.texture` TOML 中的 `format: TextureFormat` 决定了应该用 U8/F16/F32 reinterpret。

加载性能：自定义格式 vs rkyv 差异可忽略（纹理数据最终必须拷贝到 GPU staging buffer）。

## 测试策略

- **正确性测试**：集中在 `kairos_engine/tests/texture_format/` 目录下，包括 golden data（参考实现编码的已知数据）和 random roundtrip
- **性能基准测试**：集中在 `kairos_engine/benches/texture_format/`，criterion crate
  - 每个格式组一个 benchmark group（encode + decode throughput）
  - 当不确定某项优化技术的实际效果时，在 benchmark 中**同时实现多个版本做对比**，避免负优化
  - 覆盖多种 texture 尺寸（64×64, 256×256, 1024×1024, 4096×4096）

### 文件/API 迁移策略
直接重构，不保留旧 API：
- 旧 `encode_rgba` / `decode_to_rgba8` 替换为新 `encode` / `decode`
- `Texture.data` 从 `Vec<Vec<u8>>` 改为 `Vec<PixelData>`
- `.texture_bin` 从 rkyv 格式改为自定义格式
- 不保留向后兼容——项目纹理资源极少（目前仅一个），直接用新算法重新生成

## 排期原则

1. **严格按 A→B→C→D→E→F 顺序执行**，每组交付物包含 encode + decode + supports_encoding + tests + benchmarks
2. 每组的 `supports_encoding()` 在实现完成后立即改为 `true`，使格式在 texture inspector 中立即可选
3. 每个压缩家族（BC/ETC2/ASTC）需要各自独立的子模块文件
4. 测试要求：正确性测试（roundtrip + 已知数据验证）+ 性能基准测试（criterion）
5. 性能优先：rayon 并行 + cache-friendly 数据布局
