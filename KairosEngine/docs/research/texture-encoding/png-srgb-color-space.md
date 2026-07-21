# PNG sRGB / Color Space: Loading, Decoding, and GPU Upload

**Question:** When loading PNG files from disk (using Rust's `image` crate), what
color space is the decoded RGBA pixel data in? How does this affect the texture
pipeline in KairosEngine?

---

## Summary

| Layer | Color space of RGBA byte values | Notes |
|---|---|---|
| PNG file on disk | **sRGB-encoded** (if `sRGB` chunk present) or **unknown** (no chunk) | Industry convention: almost all art assets are sRGB |
| `image` crate after decode | **sRGB-encoded** (as-is from file) | The crate performs **no gamma/color-space conversion** during decode |
| `ColorType::Rgba8` | **No implied color space** | Variant name only describes channel layout (R/G/B/A × 8-bit), not transfer function |
| `DynamicImage` label | **Defaults to "sRGB"** on creation | But IO functions do not write ICC/CICP metadata to output formats |
| KairosEngine's `SerializedTexture` default | sRGB-encoded bytes stored in `.texture_bin` | Format default is `Rgba8Unorm` (linear interpretation on GPU) |
| GPU with `Rgba8Unorm` | **Treated as linear** — no sRGB decode | **MISMATCH**: sRGB-encoded data with linear interpretation |
| GPU with `Rgba8UnormSrgb` | **sRGB decoded on sample** — correct rendering | GPU hardware applies gamma → linear in shader load |

**Bottom line:** PNG files store sRGB-encoded values. The `image` crate passes
them through unchanged. KairosEngine currently uses `Rgba8Unorm` which means the
GPU treats these sRGB-encoded values as linear — this is a color-space mismatch
for color textures.

---

## 1. PNG Specification: Color Space

### 1.1. Default: no implicit color space

The PNG specification defines how pixel samples are stored (bit depth, color
type, compression), but **does not mandate a default color space**. The raw
sample values in `IDAT` chunks are just numeric values:

> `sample = integer_sample / (2^bitdepth - 1)`
>
> — PNG Specification §4.2.2.1 (gAMA chunk)

### 1.2. The `sRGB` chunk signals sRGB encoding

If an `sRGB` chunk is present, the image samples are declared to conform to the
sRGB color space:

> If the sRGB chunk is present, the image samples conform to the sRGB color
> space [sRGB], and should be displayed using the specified rendering intent
> as defined by the International Color Consortium [ICC].
>
> — PNG Specification §4.2.2.3

The `sRGB` chunk also implies specific `gAMA` (gamma = 1/2.2 ≈ 0.45455) and
`cHRM` (chromaticity) values:

```
gAMA:         45455  (gamma = 0.45455)
cHRM:
  White Point x: 31270, y: 32900
  Red x:         64000, y: 33000
  Green x:       30000, y: 60000
  Blue x:        15000, y:  6000
```

The gamma value of 45455 means: `sample = light_out ^ (1/2.2)` — the stored
sample values are the **sRGB gamma-compressed** (non-linear) representation.

### 1.3. The `gAMA` chunk independently specifies gamma

Even without an `sRGB` chunk, a `gAMA` chunk can declare the gamma:

> The gAMA chunk specifies the relationship between the image samples and the
> desired display output intensity as a power function: `sample = light_out ^ gamma`
>
> — PNG Specification §4.2.2.1

A `gAMA` of 45455 means gamma = 0.45455 ≈ 1/2.2 (sRGB-like).

### 1.4. Without any color-space chunk: color space is unknown

If neither `sRGB`, `gAMA`, nor `iCCP` chunks are present:

> the absence of a gAMA chunk indicates that the gamma is unknown.
>
> — PNG Specification §4.2.2.1

In practice, virtually all image-editing software (Photoshop, GIMP, Krita,
Aseprite) saves PNGs with sRGB encoding by default, and most include the
`sRGB` chunk.

**Sources:**
- PNG Specification §4.2.2.1 (gAMA):
  <http://www.libpng.org/pub/png/spec/1.2/PNG-Chunks.html#C.gAMA>
- PNG Specification §4.2.2.3 (sRGB):
  <http://www.libpng.org/pub/png/spec/1.2/PNG-Chunks.html#C.sRGB>
- sRGB specification (IEC 61966-2-1): <https://www.color.org/srgb.xalter>

---

## 2. Rust's `image` Crate: Decoding Behavior

KairosEngine uses `image` version 0.25.10 (from `Cargo.toml`).

### 2.1. The PNG decoder does NOT perform color-space conversion

Reading the source of `image::codecs::png::PngDecoder`:

```rust
// From image-0.25.10/src/codecs/png.rs
decoder.set_transformations(png::Transformations::EXPAND);
```

The only transformation applied is `EXPAND`, which expands bit depths < 8 to 8
(e.g., 4-bit grayscale → 8-bit). **No gamma correction, no sRGB-to-linear
conversion.**

The `read_image` method reads raw bytes from the `png` crate and converts
endianness:

```rust
fn read_image(&mut self, buf: &mut [u8]) -> ImageResult<DecodedImageAttributes> {
    let reader = self.ensure_reader_and_header()?;
    reader.next_frame(buf).map_err(ImageError::from_png)?;
    big_endian_to_native_endian(buf, layout.layout.color);
    // ...
}
```

The `big_endian_to_native_endian` call only reorders bytes (for 16-bit
channels) and does **not** touch color values.

**Source:** <https://docs.rs/image/latest/image/codecs/png/struct.PngDecoder.html>
**Source code (main branch):**
<https://raw.githubusercontent.com/image-rs/image/main/src/codecs/png.rs>

### 2.2. `ColorType::Rgba8` implies NO color space

The `ColorType` enum describes channel layout only:

```rust
pub enum ColorType {
    L8, La8, Rgb8, Rgba8,
    L16, La16, Rgb16, Rgba16,
    Rgb32F, Rgba32F,
}
```

The variant `Rgba8` means "8-bit RGBA" — it says nothing about whether the
values are sRGB-encoded or linear. There is no `ColorType::Rgba8Srgb` variant.

**Source:** <https://docs.rs/image/latest/image/enum.ColorType.html>

### 2.3. `DynamicImage` has a color-space annotation — but it's ignored during IO

As of image 0.25.x, `DynamicImage` carries a color-space annotation:

> Each image has an associated color space in the form of CICP data (ITU Rec H.273).
> ...
> The IO functions do **not yet** write ICC or CICP indications into the result
> formats. We're aware of this problem, it is tracked in #2493 and #1460.

The default color space for newly created images is **sRGB**:

> `pub fn new(w: u32, h: u32, color: ColorType) -> DynamicImage`
>
> The color space is initially set to `sRGB`.

However, the IO functions do not write this annotation to output files, and
the PNG decoder does not read it from input files.

**Source:** <https://docs.rs/image/latest/image/enum.DynamicImage.html>

### 2.4. Image ops work in encoded (non-linear) space

> The imageops functions operate in *encoded* space, directly on the channel
> values, and do **not** linearize colors internally as you might be used to
> from GPU shader programming.
>
> — `DynamicImage` documentation

This means operations like `resize`, `blur`, and `brighten` work on the
sRGB-encoded values directly, which can produce perceptually incorrect results
(though this matches the convention of most 2D image editors).

**Source:** <https://docs.rs/image/latest/image/enum.DynamicImage.html>

---

## 3. KairosEngine's Texture Pipeline

### 3.1. Current code paths

**Path A: Asset serialization** (`kairos_editor/serialize_asset/texture.rs`)

```rust
pub fn convert_img_to_asset(path: &Path) -> Result<(SerializedTexture, Vec<Vec<u8>>), Error> {
    let texture_bytes = std::fs::read(path)?;
    let texture_image = image::load_from_memory(&texture_bytes)?;
    let texture_data = texture_image.into_rgba8();
    let width = texture_data.width();
    let height = texture_data.height();
    let data = vec![texture_data.into_raw()];
    // ...
    format: TextureFormat::Rgba8Unorm,  // <-- default format: linear interpretation
    // ...
}
```

**Path B: Editor runtime** (`kairos_editor/editor_assets/texture_ext.rs`)

```rust
match image::open(&source_path) {
    Ok(img) => {
        let (w, h) = (img.width(), img.height());
        (w, h, img.into_rgba8().into_vec())
    }
    // ...
}
```

**Path C: GPU upload** (`graphics/render_pipeline.rs`)

```rust
fn create_texture(device: &Device, queue: &Queue, texture_asset: &Texture)
    -> (BindGroup, BindGroupLayout)
{
    let wgpu_fmt: wgpu::TextureFormat = texture_asset.format.into();
    // ...
    queue.write_texture(
        TexelCopyTextureInfo { texture: &gpu_texture, ... },
        level_data,
        TexelCopyBufferLayout { ... },
        Extent3d { ... },
    );
}
```

**Path D: Encoding** (`graphics/texture/format.rs`)

```rust
pub fn encode_rgba(rgba: &[u8], width: u32, height: u32, format: TextureFormat) -> Vec<u8> {
    match format {
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => rgba.to_vec(),
        // ...
    }
}
```

### 3.2. Data flow summary

```
PNG file (sRGB-encoded values)
  │
  ▼
image::load_from_memory() / image::open()
  │  NO color space conversion
  ▼
DynamicImage → into_rgba8() → Vec<u8> (still sRGB-encoded)
  │
  ▼
encode_rgba() with TextureFormat::Rgba8Unorm
  │  Pass-through (no conversion)
  ▼
queue.write_texture() (byte-copy, no conversion)
  │
  ▼
GPU memory, format=wgpu::TextureFormat::Rgba8Unorm
  │
  ▼
Shader samples → GPU interprets bytes as linear [0,1]  ← MISMATCH
```

### 3.3. The mismatch

| Component | Color space assumption |
|---|---|
| PNG source data | sRGB-encoded (gamma ≈ 0.45455) |
| `image` crate output | sRGB-encoded (pass-through) |
| `encode_rgba` pass-through | Treats both `Unorm` and `UnormSrgb` identically |
| GPU format `Rgba8Unorm` | **Linear** — no sRGB decode on sample |
| Desired behavior | sRGB decode on sample → linear in shader |

The `Rgba8Unorm` format tells the GPU: "these bytes are linear." Since the
actual bytes are sRGB-encoded, shader operations expecting linear light
(lighting, blending, etc.) will produce incorrect results.

---

## 4. Industry Convention

### 4.1. Source art assets are sRGB-encoded

In game engines and graphics programming, the convention is:

| Asset type | Color space | GPU format |
|---|---|---|
| Albedo / base color / diffuse | **sRGB** | `*UnormSrgb` |
| Specular / emissive | **sRGB** | `*UnormSrgb` |
| Normal map | **Linear** | `*Unorm` |
| Roughness / metallic / AO | **Linear** | `*Unorm` |
| Displacement / height | **Linear** | `*Unorm` |
| Data textures (lookup tables) | **Linear** | `*Unorm` or `*Float` |

### 4.2. Why sRGB for color textures?

The human visual system perceives brightness logarithmically (roughly
`~perceived ∝ intensity^0.42`). Storing values in sRGB (gamma ≈ 1/2.2) gives
more precision to perceptually-significant dark values, reducing banding with
8-bit storage.

The convention is:
1. **Author** art in sRGB space (Photoshop, Krita, etc. work in sRGB by default)
2. **Store** as sRGB-encoded PNG (standard export)
3. **Upload** to GPU with `*UnormSrgb` format
4. **GPU hardware** converts sRGB → linear on shader sample automatically
5. **Shader** computes lighting in linear space
6. **GPU hardware** converts linear → sRGB on render target output (if
   framebuffer is `*UnormSrgb`)

This is the standard "linear rendering pipeline" used by all modern game
engines (Unreal, Unity, Godot, Bevy, etc.).

**Sources:**
- Unreal Engine: "Linear Color Space" documentation:
  <https://docs.unrealengine.com/5.0/en-US/linear-color-space-in-unreal-engine/>
- LearnOpenGL: "Gamma Correction":
  <https://learnopengl.com/Advanced-Lighting/Gamma-Correction>
- Filament (Google): "Materials Guide — sRGB":
  <https://google.github.io/filament/Materials.html#srgbs>

---

## 5. The `image` Crate: What `image::open()` Gives You

### 5.1. Direct answer

When you call `image::open("texture.png")?` and call `.into_rgba8()`:

1. The PNG file is decoded by `png::Decoder` (or the internal `png` crate)
2. Only `Transformations::EXPAND` is applied (bit depth expansion)
3. The RGBA byte values are **the raw sample values from the file**
4. If the file has an `sRGB` or `gAMA` chunk, those values are sRGB-encoded
5. If the file has no color-space chunk, the values are whatever the authoring
   tool produced (likely sRGB by convention)
6. **No gamma correction, no color-space conversion** is performed by the crate

### 5.2. `ColorType::Rgba8` has no color-space semantics

`ColorType::Rgba8` → "8 bits per channel, RGBA order, 4 channels, 4 bytes per
pixel." That's all it means. The same `ColorType` enum variant is used for
sRGB and non-sRGB data interchangeably. There is no way to distinguish them at
the `ColorType` level.

### 5.3. Newer (0.25.x) `DynamicImage` tracks color space via CICP

Since image 0.25, `DynamicImage` can carry CICP color-space metadata:

```rust
// Set/get the color space:
img.set_color_space(cicp)?;
let cicp = img.color_space();
```

However, this is:
- **Not populated from PNG metadata** during decode (the PNG decoder doesn't
  read sRGB/gAMA/iCCP chunks into the CICP annotation)
- **Not written to output files** during save (tracked in issue #2493)
- Only useful if you manually set it or use `copy_from_color_space` /
  `apply_color_space` / `convert_color_space`

**Source:** <https://docs.rs/image/latest/image/enum.DynamicImage.html>

---

## 6. Recommendations for KairosEngine

### 6.1. Quick fix: Default format should be `Rgba8UnormSrgb`

The simplest change for correct color rendering: use `TextureFormat::Rgba8UnormSrgb`
instead of `TextureFormat::Rgba8Unorm` for color textures.

This tells the GPU: "these bytes are sRGB-encoded → decode to linear on sample"
— which matches what the PNG actually contains.

See `docs/research/wgpu-srgb-handling.md` for proof that:
- `queue.write_texture()` stores bytes as-is (no conversion)
- sRGB conversion is purely a shader-load-time operation
- `Rgba8Unorm` and `Rgba8UnormSrgb` are copy-compatible

### 6.2. Differentiate color vs. data textures

Not all textures should use `*UnormSrgb`. The `SerializedTexture.format` field
already supports both:

```rust
pub enum TextureFormat {
    Rgba8Unorm,      // ← for normal maps, roughness, metallic, etc.
    Rgba8UnormSrgb,  // ← for albedo, diffuse, color data
    // ...
}
```

The asset pipeline should allow the artist/technical artist to specify which
format to use per texture. This is already the case — the `format` field is
serialized in the `.texture` TOML file. The issue is just that the **default**
is `Rgba8Unorm`, which is wrong for the common case.

### 6.3. The `encode_rgba` function treats both identically — this is correct

```rust
TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => rgba.to_vec(),
```

This is correct because:
- The PNG provides sRGB-encoded bytes
- For `Rgba8UnormSrgb`: upload as-is → GPU decodes on sample ✓
- For `Rgba8Unorm`: upload as-is → GPU treats as linear (only correct for
  linear data, but wrong for typical color PNGs)

No conversion is needed at encode time. The decision is purely which GPU format
to use.

### 6.4. Future: consider sRGB-aware image operations

The `image` crate's ops work in encoded space. If KairosEngine ever uses
`image::imageops` for mipmap generation or texture resizing, the results
will be computed on sRGB-encoded values. This matches the convention used by
most game engines (mipmaps generated from sRGB source produce perceptually
better results), but is worth being aware of.

---

## References

### PNG Specification
- PNG Specification 1.2, §4.2.2.1 (gAMA chunk):
  <http://www.libpng.org/pub/png/spec/1.2/PNG-Chunks.html#C.gAMA>
- PNG Specification 1.2, §4.2.2.3 (sRGB chunk):
  <http://www.libpng.org/pub/png/spec/1.2/PNG-Chunks.html#C.sRGB>

### Rust `image` crate
- `DynamicImage` docs: <https://docs.rs/image/latest/image/enum.DynamicImage.html>
- `ColorType` docs: <https://docs.rs/image/latest/image/enum.ColorType.html>
- `PngDecoder` docs: <https://docs.rs/image/latest/image/codecs/png/struct.PngDecoder.html>
- `ImageReader` docs: <https://docs.rs/image/latest/image/struct.ImageReader.html>
- PNG decoder source (GitHub main):
  <https://raw.githubusercontent.com/Image-rs/image/main/src/codecs/png.rs>

### wgpu / WebGPU
- wgpu `TextureFormat` docs: <https://docs.rs/wgpu/latest/wgpu/enum.TextureFormat.html>
- WGPU sRGB handling research:
  <docs/research/wgpu-srgb-handling.md>

### KairosEngine source (all paths relative to repo root)
- `kairos_engine/src/kairos_editor/serialize_asset/texture.rs`
- `kairos_engine/src/kairos_editor/editor_assets/texture_ext.rs`
- `kairos_engine/src/graphics/texture.rs`
- `kairos_engine/src/graphics/texture/format.rs`
- `kairos_engine/src/graphics/render_pipeline.rs`

### Industry references
- Unreal Engine — Linear Color Space:
  <https://docs.unrealengine.com/5.0/en-US/linear-color-space-in-unreal-engine/>
- LearnOpenGL — Gamma Correction:
  <https://learnopengl.com/Advanced-Lighting/Gamma-Correction>
- Google Filament — sRGB materials:
  <https://google.github.io/filament/Materials.html#srgbs>
- sRGB specification (IEC 61966-2-1): <https://www.color.org/srgb.xalter>
