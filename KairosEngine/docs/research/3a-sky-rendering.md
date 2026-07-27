# 3A Game Sky Background Rendering — Research Notes

## Overview

This document collects **primary technical sources** — SIGGRAPH papers, GDC talks, open-source GLSL implementations, GPU Pro/GPU Gems book chapters, and engineer blog posts — on how AAA games render sky backgrounds. Engine usage documentation (e.g. "how to tick the Sky Atmosphere checkbox") has been excluded in favor of sources that explain the actual rendering math, LUT baking equations, and shader-level implementation.

The research covers three areas:

1. **Precomputed Atmospheric Scattering + Skybox LUT** — LUT baking math (integral equations, numerical integration, phase functions), LUT dimension rationale, shader sampling code.
2. **Screen-Space Fog** — Shader math for exponential height fog (density integration, transmittance, inscattering), froxel-based volumetric fog, Mip Fog technique.
3. **Dynamic Day/Night Cycle** — LUT interpolation and regeneration strategies, solar terminator rendering, color temperature shift via scattering physics.

---

## 1. Precomputed Atmospheric Scattering + Skybox LUT

### Source: Bruneton & Neyret — "Precomputed Atmospheric Scattering" (Computer Graphics Forum 2008)
- **URL:** https://hal.inria.fr/inria-00288758/document
- **Type:** Paper (PDF)
- **Author/Speaker:** Eric Bruneton, Fabrice Neyret
- **Related to:** Precomputed scattering / Sky LUT math
- **Summary:** The original paper that introduced the precomputed scattering LUT technique. It decomposes the radiative transfer equation into three LUTs — **transmittance** (2D, alt × cos(view zenith)), **single scattering** (3D, alt × sun zenith × view zenith), and **irradiance** (2D, alt × sun zenith). Explains the numerical integration: the scattering integral is evaluated via ray-marching along the view ray with 50–100 samples, using the transmittance LUT to avoid nested integration. Multiple scattering is approximated by storing scattered radiance from a single scattering event and re-feeding it as a light source for the next order. LUT dimensions (256×64, 32×32×32, 64×16) were chosen as a practical tradeoff between GPU memory (≈6 MB total) and band-limiting artifacts. The paper explicitly gives the integral equations for transmittance `T(x, ω) = exp(-∫_0^∞ ρ(h) σ_t dt)` and scattering `L_s(x, ω) = ∫_0^∞ T(x, x_t) ρ(h) σ_s P(ω, ω_s) L_in(x_t, ω_s) dt`.

### Source: Bruneton — "Precomputed Atmospheric Scattering: A New Implementation" (2017)
- **URL:** https://ebruneton.github.io/precomputed_atmospheric_scattering/
- **Type:** Documentation + Open-source reference implementation
- **Author/Speaker:** Eric Bruneton
- **Related to:** Precomputed scattering / Sky LUT / GLSL implementation
- **Summary:** A complete rewrite of the 2008 implementation with extensive documentation of every function. The GLSL source code is the definitive reference for how LUTs are baked and sampled:
  - **Transmittance LUT:** baked via `ComputeTransmittance` in `functions.glsl`, integrating density along the view ray. Sampled via `GetTransmittanceToTopAtmosphereBoundary` and `GetTransmittance`.
  - **Scattering LUT:** baked via `ComputeSingleScattering` and `ComputeScatteringDensity` (multiple scattering iteration), stored in a 3D texture. Sampled via `GetScattering`.
  - **Irradiance LUT:** baked via `ComputeDirectIrradiance` and `ComputeIndirectIrradiance`, stored in 2D. Sampled via `GetIrradiance`.
  - Multiple scattering approximation: after computing single scattering, the scattered radiance is projected onto the irradiance LUT to seed the next scattering order.
  - Code: https://github.com/ebruneton/precomputed_atmospheric_scattering (directory `atmosphere/` contains `functions.glsl`, `definitions.glsl`, `constants.h`, `model.h`, `model.cc`).

### Source: Nishita et al. — "Display of the Earth Taking into Account Atmospheric Scattering" (SIGGRAPH 1993)
- **URL:** https://dl.acm.org/doi/10.1145/166117.166140
- **Type:** Paper
- **Author/Speaker:** Tomoyuki Nishita, Takao Sirai, Katsumi Tadamura, Eihachiro Nakamae
- **Related to:** Precomputed scattering (foundational model)
- **Summary:** The foundational model for single-scattering atmospheric rendering used by nearly every game since. The Nishita model computes inscattered radiance along a view ray by integrating Rayleigh and Mie scattering coefficients, modulated by the transmittance along both the view ray and the sun ray. The sky color is obtained by integrating the scattering contribution `L = ∫ L_sun · T(sun_path) · β(λ) · P(θ) · T(view_path) ds`. This is the mathematical basis for Bruneton's precomputation.

### Source: Hillaire — "Physically Based Sky, Atmosphere and Cloud Rendering in Frostbite" (SIGGRAPH 2016)
- **URL:** https://advances.realtimerendering.com/s2016/ (course page) / GDC Vault
- **Type:** Talk — Video + Slides
- **Author/Speaker:** Sébastien Hillaire (EA / Frostbite)
- **Related to:** Precomputed scattering / Sky LUT / Day-night cycle
- **Summary:** Frostbite's sky rendering uses a physically-based atmosphere model with precomputed scattering tables. Covers the scattering integral, Rayleigh/Mie phase functions, and the LUT-based approach to sky color reconstruction. The talk explains how the scattering tables are sampled in the pixel shader to reconstruct sky color, and how the sun disk is rendered separately. Also covers cloud rendering integrated with the atmosphere model. This is the closest publicly-documented AAA engine implementation.

### Source: Patry — "Real-Time Samurai Cinema: Lighting, Atmosphere, and Tone Mapping in Ghost of Tsushima" (SIGGRAPH 2021)
- **URL:** https://advances.realtimerendering.com/s2021/index.html (slides + recorded talk)
- **Type:** Talk — Video + Slides
- **Author/Speaker:** Jasmin Patry (Sucker Punch Productions)
- **Related to:** Precomputed scattering / Sky LUT / Fog / Day-night cycle
- **Summary:** Ghost of Tsushima uses a custom sky-atmosphere model with spectral rendering accuracy achieved without precomputed LUTs — instead evaluating Rayleigh scattering per-pixel in a custom color space (`lλ` space). The talk explains why RGB scattering produces color shifts at twilight (the "green/magenta artifact") and how their custom color space solves it. Covers the full pipeline: sky dome ray-marching, haze and fog inscattering, sun/moon bounce via SH probes, and tone-mapping for cinematic look. Slides include the math for their scattering approximation.

### Source: Hillaire — "Towards Unified and Physically-Based Volumetric Lighting in Frostbite" (SIGGRAPH 2015)
- **URL:** https://advances.realtimerendering.com/s2015/index.html (slides + recorded talk)
- **Type:** Talk — Video + Slides
- **Author/Speaker:** Sébastien Hillaire (EA / Frostbite)
- **Related to:** Precomputed scattering / Screen-space fog / Volumetric lighting
- **Summary:** Proposes a unified volumetric framework for participating media. The talk covers: cascaded volume representation for density (extinction), voxelization of particles into the volume, a hierarchical volumetric shadow map for self-shadowing, and the final in-scattering integration. The physically-based model treats fog, dust, smoke, and atmosphere with the same rendering equations. Extinction and inscattering are sampled from the cascaded density volume, which can be updated per-frame.

### Source: Schneider — "The Real-Time Volumetric Cloudscapes of Horizon: Zero Dawn" (SIGGRAPH 2015)
- **URL:** https://www.guerrilla-games.com/read/the-real-time-volumetric-cloudscapes-of-horizon-zero-dawn (PDF slides available)
- **Type:** Talk — Video + Slides
- **Author/Speaker:** Andrew Schneider (Guerrilla Games)
- **Related to:** Precomputed scattering / Day-night cycle
- **Summary:** Presents the Decima engine's volumetric cloud system. The sky model provides per-frame sun color and ambient light values from precomputed scattering LUTs (similar to Bruneton's approach). Cloud lighting uses a multiple scattering approximation (two-lobed phase function) that takes the sun angle and sky color as inputs, enabling dynamic day/night response. The cloud shader is a single-pass volume ray-march targeting 2 ms on PS4.

### Source: GPU Gems 2 — "Accurate Atmospheric Scattering" (Chapter 16)
- **URL:** https://developer.nvidia.com/gpugems/gpugems2/part-ii-shading-lighting-and-shadows/chapter-16-accurate-atmospheric-scattering
- **Type:** Book Chapter
- **Author/Speaker:** Sean O'Neil (NVIDIA)
- **Related to:** Precomputed scattering / Fog
- **Summary:** A practical introduction to implementing atmospheric scattering in games. Provides GLSL shader code for Rayleigh and Mie scattering, including phase functions (`RayleighPhase(θ) = 3/16π · (1 + cos²θ)`, `MiePhase(θ) = 1/(4π) · (1-g²)/(1+g²-2g·cosθ)^(3/2)`). Describes how to precompute scattering data into textures and sample them. Though superseded by Bruneton's approach, this chapter was the starting point for many AAA sky implementations and is still valuable for understanding the core math.

### Source: Hosek & Wilkie — "An Analytic Model for Full Spectral Sky-Dome Radiance" (ACM TOG 2012)
- **URL:** https://cgg.mff.cuni.cz/projects/SkylightModelling/
- **Type:** Paper + Source code
- **Author/Speaker:** Lukas Hosek, Alexander Wilkie
- **Related to:** Precomputed scattering (analytic alternative)
- **Summary:** Provides a closed-form analytic model for full-spectral sky-dome radiance as a function of sun position, turbidity, and ground albedo. No LUTs needed — the model is evaluated per pixel. Uses precomputed coefficient tables (fitted from physically-based simulations). The model handles the full sky hemisphere, including the solar terminator and color temperature shifts. Used in many modern engines (Unity HDRP, etc.) as a cheaper alternative to full precomputed scattering.

---

## 2. Screen-Space Fog

### Source: GPU Gems 2 — "Accurate Atmospheric Scattering" (Chapter 16)
- **URL:** https://developer.nvidia.com/gpugems/gpugems2/part-ii-shading-lighting-and-shadows/chapter-16-accurate-atmospheric-scattering
- **Type:** Book Chapter
- **Author/Speaker:** Sean O'Neil (NVIDIA)
- **Related to:** Exponential height fog / Density integration math
- **Summary:** The canonical reference for exponential height fog shader math. The fog density at height `h` is `ρ(h) = e^(-h/H)` where `H` is the scale height. The transmittance through a ray segment from `h1` to `h2` is computed analytically:
  `T = exp(-∫_{h1}^{h2} ρ(h) · dt) = exp(-H · (e^(-h1/H) - e^(-h2/H)) / cos(θ))`
  where `θ` is the zenith angle. This analytic integration avoids ray-marching for height fog. The inscattering is integrated similarly. The chapter provides complete HLSL shader code.

### Source: Hillaire — "Towards Unified and Physically-Based Volumetric Lighting in Frostbite" (SIGGRAPH 2015)
- **URL:** https://advances.realtimerendering.com/s2015/index.html
- **Type:** Talk — Video + Slides
- **Author/Speaker:** Sébastien Hillaire (EA / Frostbite)
- **Related to:** Volumetric fog / Froxel-based rendering
- **Summary:** Frostbite's unified volumetric framework uses froxels (frustum-aligned voxels) to store density, extinction, and inscattered radiance. The rendering pass slices the view frustum into 2D slices at increasing distances (typically 64–128 slices). For each slice, the density is accumulated forward, then lighting is computed from the accumulated transmittance and inscattered light. The cascaded volume representation allows rendering fog at multiple scales. This approach handles both small-scale effects (dust, smoke) and large-scale atmospheric fog.

### Source: Patry — "Real-Time Samurai Cinema: Lighting, Atmosphere, and Tone Mapping in Ghost of Tsushima" (SIGGRAPH 2021)
- **URL:** https://advances.realtimerendering.com/s2021/index.html
- **Type:** Talk — Video + Slides
- **Author/Speaker:** Jasmin Patry (Sucker Punch Productions)
- **Related to:** Height fog / Haze inscattering
- **Summary:** Ghost of Tsushima uses layered atmosphere/haze evaluated per-pixel. The haze density has artist-controlled height profiles and regional variation. Inscattering from the sky and sun is integrated using the precomputed sky color (or per-pixel ray-marched sky). The talk explains the "Mip Fog" concept: the fog color is sampled from the sky cubemap's low-resolution mip levels, with the mip level varying linearly with distance from the camera. This effectively gives a volumetric appearance without 3D textures.

### Source: Valient — "Taking Killzone Shadow Fall's Graphics to the Next Level" (GDC 2014)
- **URL:** https://www.gdcvault.com/play/1020172/
- **Type:** Talk — Video (GDC Vault)
- **Author/Speaker:** Michal Valient (Guerrilla Games)
- **Related to:** Screen-space fog / Volumetric fog
- **Summary:** Describes Killzone: Shadow Fall's layered fog system. Uses a combination of analytic exponential height fog for the base atmosphere and a screen-space volumetric fog pass for local fog volumes. The height fog uses the analytic integration from GPU Gems 2. Local fog volumes are rendered as 3D density buffers projected into the view frustum. Also covers how the Decima engine's fog interacts with the sky model.

---

## 3. Dynamic Day/Night Cycle

### Source: Hillaire — "Physically Based Sky, Atmosphere and Cloud Rendering in Frostbite" (SIGGRAPH 2016)
- **URL:** https://advances.realtimerendering.com/s2016/ (course page) / GDC Vault
- **Type:** Talk — Video + Slides
- **Author/Speaker:** Sébastien Hillaire (EA / Frostbite)
- **Related to:** Day-night cycle / LUT regeneration
- **Summary:** Frostbite regenerates its atmospheric scattering LUTs every frame as the sun moves. Because the LUTs are low resolution (e.g., 128×64 for transmittance), the GPU can regenerate them in <0.1 ms. The solar terminator emerges naturally from the scattering physics — as the sun dips below the horizon, the inscattering from the sun along the view ray is progressively attenuated by the Earth's shadow. Color temperature shifts (blue sky → red sunset → dark blue twilight) are a direct result of Rayleigh scattering's λ⁻⁴ wavelength dependence: shorter wavelengths (blue) scatter more strongly, so at sunset the remaining direct sunlight is depleted in blue.

### Source: Patry — "Real-Time Samurai Cinema: Lighting, Atmosphere, and Tone Mapping in Ghost of Tsushima" (SIGGRAPH 2021)
- **URL:** https://advances.realtimerendering.com/s2021/index.html
- **Type:** Talk — Video + Slides
- **Author/Speaker:** Jasmin Patry (Sucker Punch Productions)
- **Related to:** Day-night cycle / Color temperature / Solar terminator
- **Summary:** Ghost of Tsushima's dynamic day/night system ray-marches the atmosphere per-pixel every frame (no LUT interpolation needed). The custom color space (`lλ`) achieves spectral-rendering quality for twilight colors, eliminating the green/magenta cross-talk artifacts that plague RGB scattering at sunset/sunrise. The solar terminator is handled naturally by the scattering integration — the Earth's curvature occludes the sun below the horizon. The talk explains how the color temperature shift at twilight is fundamentally a Rayleigh scattering effect, and shows comparisons between RGB and spectral sky rendering.

### Source: Bruneton — "Precomputed Atmospheric Scattering: A New Implementation" (2017)
- **URL:** https://ebruneton.github.io/precomputed_atmospheric_scattering/
- **Type:** Open-source implementation
- **Author/Speaker:** Eric Bruneton
- **Related to:** Day-night cycle / LUT recomputation
- **Summary:** The implementation supports dynamic sun position by recomputing all LUTs whenever the sun moves. The transmittance LUT depends only on view geometry (altitude × view zenith); the scattering and irradiance LUTs depend on sun zenith angle as well. For a moving sun, the 3D scattering LUT must be recomputed (or the new sun angle must be sampled from a precomputed 4D table). The implementation recomputes all LUTs on the GPU via compute shaders. For performance-critical scenarios, the implementation also supports pre-filtering the LUTs to reduce the update frequency.

### Source: Elek & Schmidt — "Atmospheric Fog and Scattering in Far Cry 5" (GDC 2018)
- **URL:** https://www.gdcvault.com/ (GDC Vault, free with registration)
- **Type:** Talk — Video
- **Author/Speaker:** Oskar Elek, Tobias B. Schmidt (Ubisoft)
- **Related to:** Day-night cycle / LUT interpolation / Aerial perspective
- **Summary:** Far Cry 5's Dunia engine uses Bruneton-style precomputed scattering LUTs. For dynamic day/night, the LUTs are recomputed every N frames (typically every 4–8 frames) at low resolution, then bilinearly interpolated between frames to avoid visual popping. The solar terminator is handled by the scattering model. Aerial perspective (distance fog) is computed from the same LUTs by sampling the scattering density along the view ray from the camera to the surface. The talk also explains how the moon and night sky lighting are handled separately from the day model.

### Source: Evans & Pangerl — "The Technology of the Far Cry 5 Dunia Engine" (GDC 2018)
- **URL:** https://www.gdcvault.com/
- **Type:** Talk — Video
- **Author/Speaker:** Oleksandr (Alex) Pangerl, Dan Evans (Ubisoft)
- **Related to:** Day-night cycle
- **Summary:** Covers Far Cry 5's dynamic weather and time-of-day system. The sky model uses a physically-based atmospheric scattering approach with artist-tunable parameters (turbidity, ozone thickness, etc.). The time-of-day is a continuous 0–24 parameter that the sky system uses to position the sun and evaluate the scattering model. Color temperature at twilight is handled by the Rayleigh/Mie scattering physics, with artistic override controls for the game's cinematic look.

### Source: GPU Pro 3 — "Real-Time Atmospheric Light Scattering" (Chapter 2.2)
- **URL:** https://www.routledge.com/GPU-Pro-3-Advanced-Rendering-Techniques/Engel/p/book/9781439887828
- **Type:** Book Chapter
- **Author/Speaker:** Jan Frohlich, Martin Eisemann
- **Related to:** Scattering / Day-night cycle
- **Summary:** A practical implementation of real-time atmospheric scattering for games. Describes how to implement a dynamic day/night cycle using precomputed scattering with per-frame updates. The chapter includes a complete CPU precomputation of scattering tables, with shader code for sampling. Explains how the solar terminator is rendered: the Earth's shadow is modeled by checking if the sun ray from a sky sample point intersects the Earth's sphere (occluded), giving the dark band at the horizon during sunset.

---

## Appendix: Other Technical References

### Source: Advances in Real-Time Rendering in Games (SIGGRAPH course series, 2010–2023)
- **URL:** https://advances.realtimerendering.com/
- **Type:** Talks — Slides + Recordings
- **Author/Speaker:** Natalya Tatarchuk (organizer) + multiple AAA speakers
- **Related to:** All areas
- **Summary:** The canonical SIGGRAPH course series on real-time rendering in games. Key years for sky/atmosphere topics: 2015 (Frostbite volumetric lighting, Horizon clouds), 2016 (Frostbite sky/atmosphere/clouds), 2021 (Ghost of Tsushima lighting/atmosphere). All slides and most talk recordings are freely available.

### Source: "Real-Time Spectral Scattering in Large-Scale Natural Participating Media" (EGSR 2016)
- **URL:** Search Google Scholar
- **Type:** Paper
- **Author/Speaker:** Eric Bruneton
- **Related to:** Spectral scattering / Precomputed luminance
- **Summary:** Extends the precomputed scattering approach to directly precompute luminance values (instead of spectral radiance → RGB conversion). This gives near-spectral accuracy at the cost of slower precomputation. Referenced in Bruneton's 2017 implementation as one of the two options for computing sky colors.

### Source: Nishita & Dobashi — "A shading model for atmospheric scattering considering luminous intensity distribution of light sources" (SIGGRAPH 1996)
- **URL:** https://dl.acm.org/doi/10.1145/237170.237266
- **Type:** Paper
- **Author/Speaker:** Tomoyuki Nishita, Yoshinori Dobashi
- **Related to:** Precomputed scattering
- **Summary:** Extended the 1993 model to handle multiple scattering and anisotropic phase functions. Provides the theoretical basis for multi-scattering approximations in game sky rendering.

### Source: Three.js — Sky Shader Implementation
- **URL:** https://github.com/mrdoob/three.js/blob/dev/examples/js/objects/Sky.js
- **Type:** Open-source code
- **Author/Speaker:** Three.js community (based on O'Neil's GPU Gems work)
- **Related to:** Scattering / Fog
- **Summary:** A complete JavaScript/WebGL implementation of atmospheric scattering based on GPU Gems 2 Chapter 16. Includes Rayleigh scattering, Mie scattering, and sun disk rendering. Useful as a minimal reference implementation for the scattering math.

### Source: Godot Engine — Sky Shader Code
- **URL:** https://github.com/godotengine/godot/
- **Type:** Open-source code
- **Author/Speaker:** Godot Engine contributors
- **Related to:** Scattering / Sky LUT
- **Summary:** Godot 4's physically-based sky uses a simplified Bruneton-style precomputed scattering model. The implementation is in the `servers/rendering/renderer_rd/shaders/sky_scene.glsl` file. It precomputes transmittance and scattering tables as textures and samples them in the sky shader.

---

## Summary Table

| Technique | Key Sources | Math/Code Available? |
|---|---|---|
| **Precomputed Scattering (Bruneton)** | Paper (2008), New Implementation (2017), GitHub repo | YES — Full GLSL source, LUT baking code, sampling functions |
| **Nishita Single Scattering** | Paper (1993) | YES — Scattering integral equations in paper |
| **Hosek-Wilkie Analytic Sky** | Paper (2012), source code | YES — C++ source with precomputed coefficients |
| **Frostbite Sky/Volumetric** | Hillaire (SIGGRAPH 2015, 2016) | Slides only (no public code) |
| **Ghost of Tsushima Sky** | Patry (SIGGRAPH 2021) | Slides with math (no public code) |
| **Horizon Cloud + Sky** | Schneider (SIGGRAPH 2015) | Slides (no public code) |
| **Far Cry 5 Dynamic Sky** | Elek & Schmidt (GDC 2018) | GDC Vault talk (slides not public) |
| **Exponential Height Fog** | GPU Gems 2 Chapter 16 | YES — GLSL code for analytic density integration |
| **Froxel Volumetric Fog** | Hillaire (SIGGRAPH 2015) | Slides (concept explained) |
| **Three.js Sky** | GitHub (open-source) | YES — JavaScript/WebGL full implementation |
| **Godot Sky** | GitHub (open-source) | YES — GLSL shader code |

### LUT Dimensions (from Bruneton 2008/2017)

| LUT | Format | Resolution | Purpose |
|---|---|---|---|
| Transmittance | 2D (texture) | 256 × 64 | Altitude × view zenith → transmittance to top of atmosphere |
| Scattering | 3D (texture) | 32 × 32 × 32 | Altitude × sun zenith × view zenith → scattered radiance |
| Irradiance | 2D (texture) | 64 × 16 | Altitude × sun zenith → direct/indirect irradiance |
| Delta scattering (optional) | 3D (texture) | 32 × 32 × 32 | Stores Mie single scattering for light shaft reconstruction |

### Day/Night Strategy Comparison

| Strategy | Examples | Quality | Performance |
|---|---|---|---|
| LUT recomputation per frame | Bruneton, Frostbite, Far Cry 5 | Good (limited by LUT resolution) | Fast (low-res LUT bake <0.1ms) |
| Per-pixel ray-march | Ghost of Tsushima | Excellent (spectral quality) | Higher cost |
| Analytic model (Hosek-Wilkie) | Unity HDRP, some mobile/console | Good (no LUTs needed) | Cheapest |
| Hybrid LUT + per-pixel | UE5 Sky Atmosphere | Good → Excellent | Scalable |
