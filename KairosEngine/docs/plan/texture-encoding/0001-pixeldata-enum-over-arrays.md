# ADR-0001: PixelDatas enum wraps whole arrays, not per-element variants

Texture formats have different bit depths (8-bit, 16-bit f16, 32-bit f32). The encode/decode API needs to accept both SDR and HDR pixel data. Rather than creating separate function pairs (`encode_rgba` / `encode_rgba_f16`) or a generic trait, we use an enum whose variants wrap entire `Vec<T>` buffers.

This design:
- Avoids per-element enum overhead (discriminant per pixel) — the enum is per-mip-level, not per-pixel
- Keeps heap memory identical to raw `Vec<u8>` for each variant
- Enables zero-cost conversion to wgpu upload data via `match + bytemuck::cast_slice`
- Makes the `Texture.data` field type-checkable at compile time
