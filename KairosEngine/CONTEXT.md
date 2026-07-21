# KairosEngine

A game engine written in Rust. The graphics subsystem manages textures, materials, meshes, shaders and rendering pipelines.

## Language

**PixelData**:
An enum over whole array buffers (`U8(Vec<u8>)`, `F16(Vec<u16>)`, `F32(Vec<f32>)`) representing one mip level of texture pixel data. The variant is chosen by the texture format's bit depth — never mixed within a single mip level. Used both as encode/decode IO and as `Texture.data`.
*Avoid*: Raw `Vec<u8>`, per-element enums, dynamic typing

**SDR format**:
A texture format whose pixel channels fit in 8-bit unsigned or signed integers (Unorm, Snorm, Uint, Sint, Srgb). Encoded/decoded via 8-bit-per-channel RGBA intermediate representation.

**HDR format**:
A texture format whose pixel channels require half-float (f16) or full-float (f32) precision (Float, Ufloat, HDR variants). Encoded/decoded via half-float RGBA intermediate representation.

**Encode**:
Convert a single mip level of RGBA pixel data (`PixelData`) into the target `TextureFormat`'s native GPU memory layout (compressed or uncompressed). Pure computation — infallible, no Result. Operates on one mip level per call; the caller loops over mip chains.

**Decode**:
Convert a single mip level of compressed/uncompressed data in a `TextureFormat`'s native GPU layout back to `PixelData`. Pure computation — infallible, no Result. Operates on one mip level per call.

**HDR pixel data**:
HDR formats (Float, Ufloat, ASTC HDR) use `PixelData::F16` as the input/output intermediate representation, following the target format's native precision. Native f32 formats (e.g. `Rgba32Float`) use `PixelData::F32`.

**Source color space**:
A `source_srgb: bool` parameter on encode that tells whether the input data is in sRGB (gamma-corrected) space or linear space. The encode function converts as needed based on the target format: `(source_srgb, target_is_srgb)` determines whether to apply gamma correction. PNG files produce sRGB-encoded data; procedural textures may be linear.
*Avoid*: Hardcoded assumptions about input color space

**Output color space**:
Decode always returns linear RGBA pixel data. The caller can apply sRGB encoding afterward if needed.
