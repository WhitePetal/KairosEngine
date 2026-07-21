# wgpu / WebGPU sRGB Texture Format Handling

**Question:** When uploading texture data to a wgpu texture with an sRGB format
(e.g. `TextureFormat::Rgba8UnormSrgb`, `Bc1RgbaUnormSrgb`, etc.), does wgpu or the
GPU driver automatically convert the pixel data from linear space to sRGB space?
Or are the bytes stored as-is and the sRGB conversion only happens at shader
sampling time?

## Short Answer

**Bytes are stored as-is.** Neither `queue.write_texture`, `queue.copyExternalImageToTexture`,
nor `CommandEncoder::copy_buffer_to_texture` performs any sRGB conversion on uploaded data.
The sRGB transfer function is applied *automatically by GPU hardware* when the texture is
*sampled in a shader* (read path) or *written as a render target* (output-merger write path).

The `Srgb` suffix on a format is a *storage interpretation hint* for the GPU's texture
unit and output-merger — it changes what hardware conversion happens during shader
load/store, not during copy/upload.

---

## Evidence

### 1. wgpu `TextureFormat` documentation

From <https://docs.rs/wgpu/latest/wgpu/enum.TextureFormat.html>:

> `UnormSrgb` formats apply the sRGB transfer function so that the storage is sRGB
> encoded while the shader works with linear intensity values.

Per-variant descriptions all say the conversion happens **"in shader"**:

- `Rgba8UnormSrgb`: *"Srgb-color [0, 255] converted to/from linear-color float
  [0, 1] **in shader**."*
- `Bc1RgbaUnormSrgb`: *"Srgb-color [0, 63] ([0, 1] for alpha) converted to/from
  linear-color float [0, 1] **in shader**."*
- `Bc7RgbaUnormSrgb`: *"Srgb-color [0, 255] converted to/from linear-color float
  [0, 1] **in shader**."*
- `Etc2Rgb8UnormSrgb`: *"Srgb-color [0, 255] converted to/from linear-color float
  [0, 1] **in shader**."*

The phrase "in shader" means the conversion is part of the texture-load and texture-store
operations in the shader pipeline, not part of data upload.

### 2. wgpu `Queue::write_texture` documentation

From <https://docs.rs/wgpu/latest/wgpu/struct.Queue.html#method.write_texture>:

> `data` contains the texels to be written, **which must be in the same format as
> the texture.**

No mention of any conversion. The user is responsible for providing bytes in the
correct format (sRGB-encoded or not, depending on intent).

### 3. WebGPU specification — Texture format semantics

From <https://www.w3.org/TR/webgpu/#texture-format-capabilities> (section 6.3):

> If the format has the `-srgb` suffix, then sRGB conversions from gamma to linear
> and vice versa are applied **during the reading and writing of color values in
> the shader.**

This directly states the conversion happens in shader load/store, not during copy.

### 4. WebGPU specification — Texel copy rules (copy-compatible formats)

From section [11.2.6](https://www.w3.org/TR/webgpu/#texel-copy-compatibility):

> Two `GPUTextureFormat`s format1 and format2 are **copy-compatible** if:
> - format1 equals format2, **or**
> - format1 and format2 differ only in whether they are `srgb` formats (have the
>   `-srgb` suffix).

This means `Rgba8Unorm` and `Rgba8UnormSrgb` are interchangeable as source/destination
in copies. If the driver performed any sRGB conversion during copy, these formats
would *not* be copy-compatible — copying between them would produce different
numeric results.

### 5. WebGPU specification — `writeTexture()` algorithm

From section [19.2](https://www.w3.org/TR/webgpu/#GPUQueue):

The `writeTexture()` algorithm (implemented as `queue.write_texture` in wgpu) is
specified as a texel copy. The spec says:

> In a texel copy, the bytes written to the destination texel blocks will have an
> **equivalent texel representation** to the source value.

No sRGB conversion step is documented. The bytes are copied verbatim.

### 6. WebGPU specification — `copyExternalImageToTexture()` special case

From section [19.2](https://www.w3.org/TR/webgpu/#GPUQueue):

`copyExternalImageToTexture()` has a *special explicit* sRGB handling step for
its platform-image-source path:

> If texture.format is an `-srgb` format: Set dstColor to the result of applying
> the sRGB non-linear-to-linear conversion to it.
>
> **Note:** This cancels out the sRGB linear-to-non-linear conversion that occurs
> when writing an `-srgb` format in the next step, so that precision from an
> sRGB-like input image is not lost and the *linear* color values of the original
> image can be read from the texture.

The fact that this needs explicit cancellation *proves* that write/store operations
on sRGB textures do perform sRGB encoding (linear → gamma) — but only through
the **shader output-merger path**, not through copy paths. `copyExternalImageToTexture`
works around this by pre-converting the source to linear so that the output-merger
converts it *back* to sRGB matching the original.

### 7. wgpu source — `TextureFormat::remove_srgb_suffix` / `has_srgb_suffix`

From <https://github.com/gfx-rs/wgpu/blob/trunk/wgpu-types/src/texture/format.rs>:

```rust
/// Returns `true` for `*Srgb` formats.
pub fn has_srgb_suffix(&self) -> bool {
    *self != self.remove_srgb_suffix()
}

/// Changes `*UnormSrgb` texture formats to `*Unorm`.
pub fn remove_srgb_suffix(&self) -> TextureFormat {
    match *self {
        Self::Rgba8UnormSrgb => Self::Rgba8Unorm,
        Self::Bc1RgbaUnormSrgb => Self::Bc1RgbaUnorm,
        // ... etc.
        _ => *self,
    }
}
```

These methods operate purely at the type level with no data transformation —
consistent with sRGB being a *sampling interpretation* hint, not a data
transformation.

### 8. WebGPU specification — GPUTextures are not color managed

From section [3.11](https://www.w3.org/TR/webgpu/#color-spaces):

> As described above, `GPUTexture`s are not color managed. This includes `-srgb`
> formats, which despite their name are not *tagged* with an sRGB color space
> (like those described by `PredefinedColorSpace` and the CSS color spaces srgb
> and srgb-linear).
>
> However, `-srgb` texture formats *do* have gamma-encoding/decoding properties
> which are algorithmically close to those used for gamma encoding in `"srgb"`
> and `"display-p3"`.

---

## Practical Consequences

### Q1: Same BC1 encoding for `Bc1RgbaUnorm` vs `Bc1RgbaUnormSrgb`?

If you encode the same linear RGBA data into BC1 blocks and upload the identical
bytes to both formats:

- `Bc1RgbaUnorm`: Sampling produces the expected linear values. The GPU decodes BC1
  (5:6:5 endpoints → float [0,1]) with linear scaling.
- `Bc1RgbaUnormSrgb`: Sampling produces *different* (too-dark) values. The GPU
  decodes BC1 (5:6:5 endpoints → float [0,1]), then **additionally applies the
  sRGB gamma-to-linear transfer function** to the decoded result.

Since your linear data is already linear, the GPU's sRGB decoding over-corrects it.
**You must compress sRGB-encoded (gamma-corrected) source data for sRGB texture
formats.**

| Source data | `Bc1RgbaUnorm` | `Bc1RgbaUnormSrgb` |
|---|---|---|
| Linear data | Correct | Too dark (double-compressed) |
| sRGB-encoded data | Too bright (no gamma decode) | Correct |

### Q2: Does `queue.write_texture` do any sRGB conversion?

**No.** Never. `write_texture` is a straight byte copy. You provide bytes matching the
format, and they are written verbatim to GPU memory. No transfer function is
applied.

### Q3: For compressed sRGB formats, does "sRGB" affect encoding or just sampling?

**It only affects sampling and render-target output.** The compressed block format
(BC1, BC3, BC7, ETC2, ASTC) is identical between `Unorm` and `UnormSrgb` variants
— same block size, same bit layout, same byte count. The difference is purely in
how the GPU hardware interprets the decompressed values:

- `Unorm`: decompressed → linear unorm → float [0, 1]
- `UnormSrgb`: decompressed → linear unorm → float [0, 1] → **sRGB transfer function**

However, for correct *visual results*, the source data fed into the compressor
should be sRGB-encoded (non-linear). Compressing linear data and storing in an
sRGB format yields incorrect colors because the GPU applies an extra gamma decode.

---

## Summary Table

| Operation | sRGB conversion applied? | Source |
|---|---|---|
| `queue.write_texture()` | **No** — bytes stored as-is | WebGPU §19.2, wgpu docs |
| `CommandEncoder::copy_buffer_to_texture()` | **No** — bytes stored as-is | WebGPU §11.2 |
| `CommandEncoder::copy_texture_to_texture()` | **No** — copy-compatible | WebGPU §11.2.6 |
| Texture creation (`create_texture`) | **No** — allocates zero-initialized memory | WebGPU §6.1.3 |
| `copyExternalImageToTexture()` *(from HTML/video/canvas)* | **Special** — cancels sRGB encoding to preserve linear values | WebGPU §19.2 |
| Shader sampling of `*UnormSrgb` texture | **Yes** — gamma → linear (sRGB decode) | WebGPU §6.3, wgpu docs |
| Render target output to `*UnormSrgb` attachment | **Yes** — linear → gamma (sRGB encode) | WebGPU §23.2.7 |
| Canvas `getCurrentTexture()` | **No** — canvas uses `bgra8unorm` / `rgba8unorm` without `-srgb`, then `viewFormats` can create sRGB views | WebGPU §21.4 |

---

## References

- wgpu `TextureFormat` enum docs: <https://docs.rs/wgpu/latest/wgpu/enum.TextureFormat.html>
- wgpu `Queue::write_texture` docs: <https://docs.rs/wgpu/latest/wgpu/struct.Queue.html#method.write_texture>
- wgpu source (`wgpu-types/src/texture/format.rs`): <https://github.com/gfx-rs/wgpu/blob/trunk/wgpu-types/src/texture/format.rs>
- WebGPU spec — Texture formats (§6.3): <https://www.w3.org/TR/webgpu/#texture-format-capabilities>
- WebGPU spec — Texel copies (§11.2): <https://www.w3.org/TR/webgpu/#texel-copies>
- WebGPU spec — Copy-compatible formats (§11.2.6): <https://www.w3.org/TR/webgpu/#texel-copy-compatibility>
- WebGPU spec — `writeTexture()` (§19.2): <https://www.w3.org/TR/webgpu/#GPUQueue>
- WebGPU spec — `copyExternalImageToTexture()` (§19.2): <https://www.w3.org/TR/webgpu/#GPUQueue>
- WebGPU spec — Color spaces and encoding (§3.11): <https://www.w3.org/TR/webgpu/#color-spaces>
