# ADR-0002: sRGB conversion is handled inside encode via `source_srgb` flag

The encode function needs to handle color-space conversion because wgpu stores bytes verbatim — neither `queue.write_texture` nor any texel copy operation performs sRGB conversion. The GPU's sRGB→linear decoding only happens at shader sample time.

Rather than pushing sRGB conversion to the caller, we add a `source_srgb: bool` parameter to `encode()`. The function internally decides whether to gamma-correct based on `(source_srgb, target_is_srgb)`:

| source_srgb | Target is Srgb | Action |
|:---:|:---:|---|
| true | true | No conversion — data is already in sRGB space |
| true | false | sRGB→linear conversion |
| false | true | linear→sRGB conversion |
| false | false | No conversion — data is already in linear space |

This choice was made because:
- PNG files store sRGB-encoded data; `image::open()` passes bytes through unchanged
- Most source art assets are authored in sRGB space
- A separate conversion step before every encode call would be error-prone and easy to forget
- The cost of the gamma lookup table is negligible relative to compression algorithms

Decode always returns linear data. Callers who need sRGB output apply `linear_to_srgb()` themselves.
