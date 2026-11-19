# ADR 0013: Color is one linear eight-byte value; HDR is field policy

- **Status**: Accepted
- **Date**: 2026-11-19
- **Context**:
  - The engine had two shared color types: `Color32` (eight-bit sRGB) and
    `HdrColor` (linear `f32`, unbounded). The editor's picker had to switch on
    an enum to know which one it was editing. HDR was therefore encoded twice:
    once in the value type and once in the inspector.
  - Unity's `[ColorUsage]` attribute configures the Color Field/Picker (HDR,
    alpha, brightness range); the stored `Color` is still one value. The same
    separation is a better fit for Kairos: HDR is a field constraint, not a
    different representation, and a value that crosses 1.0 must not change type.
  - A GPU-facing color should have a compact, uniform layout. Four `f16`
    channels are eight bytes and match a `vec4<f16>`; adding a per-value HDR
    tag would make the value ten or sixteen bytes and force a second packed type
    at upload.
  - The editor still needs sRGB for hex style files, the color wheel, and egui
    painting. Eight-bit sRGB and linear `f16` round-trip exactly for all 256
    byte values, so the encoding can live at the boundary rather than in the
    value.
- **Decision**:
  1. **One value type.** `Color` is four `f16` channels in linear light,
     straight alpha, `#[repr(C, align(8))]`, eight bytes, RGB unbounded. It is
     the canonical engine/asset/material color.
  2. **HDR is metadata.** `ColorUsage` carries `hdr`, `alpha`, display
     `space`, and `eyedropper`; `ColorProperty` pairs a `Color` with its
     usage. HDR/LDR is never a variant of the value and never enters a uniform
     buffer.
  3. **Linear storage, sRGB edges.** `Color::from_srgb8` / `to_srgb8`,
     `from_srgb_f32` / `to_srgb_f32`, and `from_hex` / `to_hex` are the only
     sRGB conversions. HDR picker editing uses an sRGB base plus an exposure in
     stops.
  4. **Serialization follows the value, not the usage.** A displayable value is
     written as sRGB hex; an HDR or out-of-range value as `[r, g, b, a]` linear
     floats. Field-aware serializers may force the float form for data colors.
  5. **No second eight-bit color type.** `Color32` is removed: style structs,
     the legacy editor, and controls all use `Color`. The editor's paired
     `ColorProperty { color, usage }` and its HDR base/exposure math live in
     `kairos_editor_widgets::color_picker`, not in `kairos_color`.
  6. **Material properties.** A reflected `vec4<f32>` tagged `@color` maps to
     `MaterialProperty::Color(Color)`; `@hdr` and `@linear` set the usage.
     The GPU ABI may be `vec4<f32>` or `vec4<f16>`; `Color` is independent of
     that choice.
- **Consequences**:
  - `Color` is eight bytes with no padding and is `bytemuck::Pod`; arrays can
    be uploaded without repacking.
  - Editor math stays in `f32`; writes quantize to `f16`. An arbitrary 0.5
    sRGB value may display as 127/255 after storage; an 8-bit sRGB source value
    round-trips exactly.
  - The shared picker currently presents sRGB for every field. `ColorSpace::Linear`
    is metadata for a future numeric-linear editor; it never changes storage.
  - `ColorProperty` is deliberately not in `kairos_color`: it is the widget's
    edit model, while engine/material code needs only `Color` and `ColorUsage`.
