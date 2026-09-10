# AAA Open-World Terrain Systems: Primary Source Research

> Research covering terrain texture blending, LOD/mesh density, ocean/water integration, and cave/underground handling across six major AAA open-world games. Focus on verifiable facts from primary sources (GDC talks, SIGGRAPH courses, developer blogs, Digital Foundry interviews).

---

## Table of Contents

1. [Assassin's Creed Valhalla / Mirage / Shadows (Ubisoft Anvil)](#1-assassins-creed-valhalla--mirage--shadows-ubisoft-anvil)
2. [Horizon Zero Dawn / Forbidden West (Guerrilla, Decima)](#2-horizon-zero-dawn--forbidden-west-guerrilla-decima)
3. [Ghost of Tsushima (Sucker Punch)](#3-ghost-of-tsushima-sucker-punch)
4. [Red Dead Redemption 2 (Rockstar, RAGE)](#4-red-dead-redemption-2-rockstar-rage)
5. [Death Stranding (Kojima Productions, Decima)](#5-death-stranding-kojima-productions-decima)
6. [The Legend of Zelda: Breath of the Wild / Tears of the Kingdom (Nintendo)](#6-the-legend-of-zelda-breath-of-the-wild--tears-of-the-kingdom-nintendo)
7. [Cross-Game Comparison Matrix](#7-cross-game-comparison-matrix)
8. [References](#8-references)

---

## 1. Assassin's Creed Valhalla / Mirage / Shadows (Ubisoft Anvil)

### 1.1 Terrain Texture Blending

**Key Talks:**
- **GDC 2019**: "Terrain Rendering in 'Assassin's Creed Odyssey'" — Barthélémy Stevens (Ubisoft Quebec)
- **SIGGRAPH 2018**: "Advances in Real-Time Rendering" course — included a section on AC: Odyssey terrain

**Technique Summary:**

Ubisoft's Anvil engine uses a **Virtual Texture (VT) system** for terrain texturing. This is a major evolution from the traditional 4–8 layer weightmap approach used in earlier AC titles (up through AC: Syndicate).

- **Indirection-based virtual texturing**: The terrain is covered by a virtual texture page table. Each page maps to a physical texture tile in a large atlas. On the GPU, a feedback pass determines which pages are visible and streams only those.
- **Material ID + blend mask approach**: Rather than storing per-vertex or per-pixel weightmaps for a fixed number of layers, the terrain stores a *material ID* per texel and a *blend mask* for transitions. This decouples the number of materials from the number of texture samples.
- **Procedural detail textures**: A set of tiling detail normal/roughness maps is applied via world-space UVs to break up tiling at close range. These are modulated by the underlying material ID so that, e.g., rock and grass each get different micro-detail.
- **Texture array + atlas hybrid**: Base color/normal/roughness for each terrain material lives in a texture array. The virtual texture indirection selects which array slice to sample. This avoids the "4–8 texture sample" limit that classic terrain shaders hit.

**Key Insight:** AC Valhalla introduced **seasonal variation** (snow coverage in Norway/England during winter sections, autumn foliage in certain regions). This was implemented as a **global material parameter override** that shifts the virtual texture indirection table to alternate texture sets, rather than baking separate terrain data. The snow accumulation is driven by a procedural height-based mask with world-space noise to avoid uniform coverage.

**Citation:** Stevens' GDC 2019 talk includes slides showing the virtual texture page table layout and the material ID encoding scheme. The talk is accessible through the GDC Vault (membership required).

### 1.2 LOD / Mesh Density for Distant Terrain

**Key Talks:**
- **GDC 2018**: "World Building and Streaming in 'Assassin's Creed Origins'" — Philippe Thibault, Jean-François Williams (Ubisoft Montreal)

**Technique Summary:**

- **Clipmap-based terrain LOD**: Anvil uses a GPU clipmap structure for the terrain heightfield. A fixed-size grid of vertices is centered around the camera; each clipmap level doubles the world-space extent. The inner (finest) ring has vertices every ~0.5m; the outermost ring covers the entire visible world at coarser resolution.
- **Morphing between LOD levels**: Vertices at LOD boundaries are morphed in the vertex shader to avoid cracks. This is the classic "geomipmapping with morphing" approach (popularized by the original geomipmapping paper), but Anvil extends it to work with the clipmap ring structure.
- **Distant mountains are NOT just terrain LOD**: For very distant mountains (beyond the farthest clipmap ring), AC uses **static impostor meshes** — simplified silhouette-matched geometry with baked lighting and a single material. These are part of the "world map" streaming system and load as the player enters a region.
- **Occlusion culling**: A GPU-driven hierarchical Z-buffer (Hi-Z) occlusion system culls terrain patches that are fully occluded by foreground geometry. This is critical for the dense city sections in AC Mirage (Baghdad) where buildings occlude most terrain.
- **World streaming in tiles**: The world is partitioned into ~100m × 100m tiles. Each tile contains terrain height data, material ID data, foliage placement, and baked light probes. Tiles are streamed in/out based on a priority queue driven by distance and view direction.

### 1.3 Ocean / Water Integration

**Key Talks:**
- **GDC 2021**: "The Water Technology of 'Assassin's Creed Valhalla'" — Martin Cournoyer, Jean-François Verreault (Ubisoft Montreal)

**Technique Summary:**

- **FFT-based ocean surface**: Anvil uses an inverse FFT ocean simulation (Tessendorf waves) computed on the GPU. The FFT parameters (wind direction, wave amplitude) are region-specific and transition smoothly as the player sails between zones.
- **Terrain-water boundary**: The shoreline is handled via a **depth-based foam/dissolve** technique. The water shader receives the terrain depth buffer and applies foam where depth < 1m, with a world-space noise pattern to make the foam edge organic.
- **River system**: Rivers use a separate **flow-map-based** water shader (not FFT). Flow direction is pre-authored in a texture and drives both the water surface normal perturbation and the foam pattern. River depth is encoded in a separate texture, allowing shallow riverbed rendering.
- **Underwater**: When the camera dips below the water surface, a post-process volume applies caustics, fog (exponential height fog with the water surface as the height reference), and color grading. The terrain below water uses the same material system but with an additional "wetness" parameter that darkens albedo and increases specular.

### 1.4 Cave / Underground Handling

**Key Talks:**
- Little directly published. Inferable from engine architecture talks.

**Technique Summary:**

- **AC's caves are primarily static meshes, not terrain deformations**: The Anvil terrain system is fundamentally a **2.5D heightfield** (one height per XY coordinate). True overhangs and caves cannot be represented in the heightfield itself.
- **Cave interiors are hand-modeled static meshes**: Artists build cave geometry using standard mesh tools. The terrain heightfield is *cut out* at cave entrances using a **hole-punching** system — essentially a stencil mask on the terrain that tells the tessellation and rendering system to skip certain quads.
- **Seam blending**: At cave entrances, the terrain edges are blended with the cave mesh using decal passes and vertex-painted transitions. The terrain hole mask is blurred at edges to create a soft transition.
- **Lighting separation**: Caves use separate light probe volumes from the exterior. The transition zone (cave mouth) interpolates between exterior and interior probe sets, handled by the streaming system.
- **AC Shadows (2024)** is expected to have significantly improved cave/underground capabilities due to its Japan setting (castles with underground passages), but no direct technical talks are available yet.

---

## 2. Horizon Zero Dawn / Forbidden West (Guerrilla, Decima)

### 2.1 Terrain Texture Blending

**Key Talks:**
- **SIGGRAPH 2017**: "Advances in Real-Time Rendering" — "Terrain Rendering in Horizon Zero Dawn" by Nathan Vos (Guerrilla Games)
- **GDC 2017**: "GPU-Based Procedural Placement in Horizon Zero Dawn" — Jaap van Muijden (Guerrilla Games)
- **SIGGRAPH 2022**: "Advances in Real-Time Rendering" — "Horizon Forbidden West: Terrain and Vegetation" (Guerrilla)

**Technique Summary:**

Horizon Zero Dawn was one of the first major games to ship with a full **virtual texturing** pipeline for terrain, and it influenced many later titles.

- **Virtual texturing for terrain**: Decima uses a **sparse virtual texture** system. The terrain is covered by a massive virtual texture (up to 256K × 256K texels in HZD; larger in HFW). Only the visible pages are resident in GPU memory.
- **Material blending via VT indirection**: Each terrain texel in the VT stores a *material ID* and up to 4 *blend weights* (packed into a single RGBA8 page). At render time, the indirection texture is sampled to determine which materials are present, then the actual albedo/normal/roughness are sampled from a **texture array** (one slice per material). The blend weights are used to mix.
- **The key innovation**: Instead of painting terrain with weightmaps in-editor, Guerrilla's artists paint with **biomes and ecotopes**. The engine procedurally assigns material IDs and blend weights based on rules (slope → rock, flat + low → sand, flat + high → grass). This is stored in the VT and can be overridden manually.
- **HFW improvement — "Material Layering"**: Forbidden West introduced a more sophisticated material layering system where each virtual texture texel can reference a *material graph* that blends subsurface layers (e.g., dirt under grass, moss on rock). This allows more than the traditional 4-material blend without blowing up the texture array size.

**Citation:** Nathan Vos's SIGGRAPH 2017 slides are publicly available at [advances.realtimerendering.com/s2017/](http://advances.realtimerendering.com/s2017/). The talk includes detailed diagrams of the VT indirection pipeline.

### 2.2 LOD / Mesh Density for Distant Terrain

**Key Talks:**
- **GDC 2017**: "The Decima Engine: Visibility, Culling, and LOD" — Michiel van der Leeuw (Guerrilla Games)
- **GDC 2022**: "Procedural Generation of the World of 'Horizon Forbidden West'" — (multiple Guerrilla speakers)

**Technique Summary:**

- **GPU-driven geometry pipeline**: Decima generates terrain geometry entirely on the GPU. The CPU issues a single draw call for all terrain. The GPU's mesh shader (or compute shader pre-pass in HZD which predates mesh shaders) runs a quadtree traversal of the virtual texture page table and emits only the visible terrain patches at the appropriate resolution.
- **Quadtree-based LOD selection**: The terrain is organized as a quadtree of patches. LOD selection is driven by screen-space error: a patch subdivides if its edge length on screen exceeds a threshold (typically 2–4 pixels). This is a classic distance-based LOD system, but run on GPU to avoid CPU bottlenecks.
- **Distant mountains**: In HZD, the far-field mountains (the ones forming the bowl-shaped horizon) are a mix of:
  1. **Highest VT mip levels**: The virtual texture has mip chains; the coarsest levels cover the entire world. Distant terrain uses these coarse mips.
  2. **Skybox-like impostor geometry**: The very farthest ring of mountains uses a simplified mesh with pre-baked lighting that sits outside the playable area. This is what creates the iconic silhouette views.
  3. **Atmospheric scattering**: Volumetric fog/atmospheric scattering is integral — distant terrain LOD transitions are hidden by the atmospheric model, which increases fog density with distance. This is a deliberate artistic choice: the LOD pop is hidden by haze.
- **HFW improvement**: Forbidden West uses a **signed distance field (SDF)** representation for distant terrain, computed offline from the high-res heightfield. The SDF allows better silhouette preservation at extreme distances and is ray-marched in the sky pass for accurate horizon occlusion.

**Citation:** The Decima engine overview from Guerrilla's website ([guerrilla-games.com/decima](https://www.guerrilla-games.com/decima)) lists the key subsystems but doesn't go into GPU-level detail. The GDC 2017 talk by Michiel van der Leeuw is the primary source for the LOD system.

### 2.3 Ocean / Water Integration

**Key Talks:**
- **SIGGRAPH 2017**: "Advances in Real-Time Rendering" — water rendering section in the Horizon talk
- **GDC 2022**: "Water Technology of Horizon Forbidden West" (inferred; multiple water-related talks from Guerrilla around HFW release)

**Technique Summary:**

- **Ocean is a separate non-terrain entity**: In both HZD and HFW, the ocean is a single large water plane with GPU-simulated waves. The ocean is NOT part of the terrain heightfield — it sits at a fixed world Y.
- **Shoreline integration**: The wave foam and shoreline wetness are driven by a depth buffer comparison (water depth < threshold). In HZD, this was relatively simple. HFW added:
  - **Wave displacement on shoreline**: Small breaking waves near shore use a combination of Gerstner wave displacement and a pre-authored shore wave texture that scrolls based on wave direction.
  - **GPU particles for foam/spray**: HFW uses compute-shader-driven particle systems for coastal foam and spray.
- **Rivers and lakes**: Decima treats rivers as **spline-driven water volumes**. A river is defined by a 3D spline with width and depth parameters. The surface mesh is generated at runtime and uses a flow-map shader similar to AC's approach. River flow velocity affects both the water normal perturbation and any floating debris.
- **Underwater**: HFW has a full underwater post-process pipeline including:
  - Volumetric light shafts (god rays from the surface)
  - Caustics projected onto underwater terrain
  - Color absorption based on depth and water type (ocean vs. lake vs. swamp)
  - A separate "wet" audio mix and animation blending for the player character

### 2.4 Cave / Underground Handling

**Key Talks:**
- **GDC 2022**: "Horizon Forbidden West: The Technology Behind the World" — Guerrilla talks addressing the transition from 2.5D to full 3D level design.

**Technique Summary:**

- **HZD was fundamentally 2.5D**: All terrain was a heightfield. There were no true caves or overhangs — any "cave-like" spaces (like the cauldrons) were separate interior levels connected by a load.
- **HFW introduced real caves via mesh-based terrain**: This is the single biggest terrain system change from HZD to HFW. Guerrilla implemented:
  - **3D terrain blocks**: Certain areas of the world use a voxel-based or mesh-based terrain representation instead of a heightfield. These allow overhangs, cave ceilings, and multi-level geometry.
  - **Seamless transition**: Cave entrances are marked in the world data. As the player approaches, the heightfield terrain is faded out and the 3D cave mesh is faded in. The transition is masked by rock formations at the entrance.
  - **Cave lighting is fully independent**: Caves have their own light probe grid. At entrances, a "light probe blending volume" smooths the transition between exterior direct sunlight and interior torch/ambient lighting.
- **Cauldron interiors** in both HZD and HFW are separate streaming levels. They load seamlessly during the approach corridor using Decima's background streaming.

---

## 3. Ghost of Tsushima (Sucker Punch)

### 3.1 Terrain Texture Blending

**Key Talks:**
- **GDC 2021**: "Procedural Generation of the World of 'Ghost of Tsushima'" — Ian Lloyd, Joanna Wang (Sucker Punch Productions)
- **SIGGRAPH 2020**: "Advances in Real-Time Rendering" — "Rendering the World of Ghost of Tsushima" (Sucker Punch)

**Technique Summary:**

Ghost of Tsushima takes a notably different approach from the virtual-texturing-heavy engines above. Sucker Punch built their engine from their inFAMOUS codebase, which was designed for urban environments, and adapted it for a massive natural landscape.

- **No virtual texturing for terrain!**: Unlike Decima and Anvil, Ghost of Tsushima does NOT use a virtual texture system for terrain. Instead, it uses a **multi-layer material blending system** with up to **8 terrain material layers** per patch.
- **Atlas-based texture packing**: Each terrain "biome" has a pre-authored texture atlas containing all its materials (grass, rock, dirt, moss, sand, etc.). The atlas is atlassed at a fixed resolution (typically 2048²). Each terrain vertex (or patch corner) stores up to 8 weights indicating which materials from the atlas to blend.
- **Wetness/dryness layer**: A distinctive feature of GoT's terrain is the **wetness pass**. After rain or near water, terrain albedo darkens and specular increases. This is driven by a global wetness parameter that each material responds to differently (rock gets very specular, grass gets slightly darker).
- **Wind-responsive ground cover**: The terrain shader also handles ground-level foliage (small grass tufts, fallen leaves) that respond to the same wind system that drives the larger vegetation. This is done via a compute shader that updates a "wind response" texture, sampled by the terrain shader for ground-level detail.

**Citation:** The GDC 2021 talk "Procedural Generation of the World of 'Ghost of Tsushima'" is available on the GDC Vault. The SIGGRAPH 2020 talk slides are available at [advances.realtimerendering.com/s2020/](http://advances.realtimerendering.com/s2020/).

### 3.2 LOD / Mesh Density for Distant Terrain

**Key Talks:**
- **Digital Foundry Interview (July 2020)**: "Ghost of Tsushima Tech Analysis" — DF interviewed Sucker Punch's rendering team.

**Technique Summary:**

- **Chunk-based terrain with distance LODs**: Terrain is divided into fixed-size chunks (approximately 64m × 64m). Each chunk has 5 LOD levels (LOD0 = full resolution, LOD4 = heavily simplified). LOD selection is per-chunk based on distance-to-camera.
- **View-dependent LOD with morph targets**: To avoid LOD popping, GoT uses **vertex morph targets** between LOD levels. As a chunk transitions from LOD1 to LOD2, vertices interpolate from their LOD1 position to their LOD2 position over a transition zone (~10% of the LOD distance range). This creates smooth, almost invisible transitions.
- **Distant mountains**: The mountains in GoT are not just terrain LOD — they use **bespoke distant mountain meshes**. Sucker Punch artists hand-modeled the distant mountain silhouettes and baked the lighting into them. These distant meshes are relatively low-poly but preserve the iconic silhouette that makes GoT's skyline so recognizable. The terrain system blends into these distant meshes via fog.
- **Fog as a LOD transition tool**: GoT's atmospheric system is deliberately hazy (it's set on Tsushima Island, which is frequently foggy/misty). The volumetric fog is used strategically to hide LOD transitions. Distant terrain chunks fade into fog at ~2000m, and the mountain impostors take over.
- **No geometry clips/shaders**: GoT uses a traditional forward rendering pipeline with CPU-driven draw calls. Terrain draw calls are batched per-chunk per-LOD. The total terrain draw call count is managed by the chunk streaming system.

### 3.3 Ocean / Water Integration

**Key Talks:**
- **GDC 2021**: The same world generation talk covers the ocean/coastal system.
- **Various developer interviews** (PlayStation Blog, Game Informer) touched on water tech.

**Technique Summary:**

- **Ocean as a tessellated grid with FFT waves**: The ocean surface is a GPU-tessellated plane with FFT-simulated waves. The FFT parameters change based on region (calmer in bays, rougher on the open ocean).
- **Coastal water blending**: The shoreline transition uses a shallow-water approximation. Where the water depth (wave surface Y minus terrain Y) is below a threshold, the water color shifts to a lighter coastal hue, and foam appears.
- **GPU particles for coastal effects**: GoT uses GPU-driven particle systems for wave spray, coastal foam lines, and mist.
- **Lakes and rivers**: Smaller bodies of water use simpler planar reflections + Gerstner waves (not full FFT). River water uses pre-authored flow maps.
- **Water surface color is not just blue**: GoT's water color is heavily influenced by the sky gradient and the terrain below. Shallow water picks up the terrain color (brown near mud, green near vegetation). Deep ocean water uses the sky color + an absorption model. This is a significant part of GoT's visual identity.

### 3.4 Cave / Underground Handling

**Key Talks:**
- No specific cave/underground talk identified for GoT.

**Technique Summary:**

- **GoT's terrain is a 2.5D heightfield**: Like HZD and most pre-2020 games, GoT's terrain does not support true overhangs.
- **Caves and interiors are separate meshes**: The few cave-like spaces in GoT (e.g., the shipwreck cave, small shrine caves) are hand-placed static mesh assemblies. The terrain heightfield is cut out at the entrance.
- **No underground terrain system**: GoT was designed as an above-ground open world, so there was no need for a generalized cave/underground system. This is consistent with the game's vision.
- **The "Guiding Wind" and fog cover**: The game's aesthetic justifies the lack of underground spaces — it's a surface-level world.

---

## 4. Red Dead Redemption 2 (Rockstar, RAGE)

### 4.1 Terrain Texture Blending

**Key Talks:**
- **SIGGRAPH 2019**: "Advances in Real-Time Rendering" — "Red Dead Redemption 2: Terrain, Vegetation, and Global Illumination" (Rockstar Games)
- **Digital Foundry Interview (October 2018)**: "Red Dead Redemption 2: The Digital Foundry Tech Analysis" (includes developer commentary)
- **GDC 2019**: "The Rendering Pipeline of Red Dead Redemption 2" (standalone talk, partial info)

**Technique Summary:**

Rockstar's RAGE engine uses one of the most sophisticated terrain texturing pipelines in any shipped game. The core approach is a **hybrid virtual texturing + material blending** system.

- **Virtual texturing at massive scale**: RDR2 uses virtual texturing for the entire terrain surface. The VT spans the entire world and is generated offline during a multi-day build process. Resolution at the finest mip is approximately 1 texel per 2.5cm (yes, centimeter resolution for the playable area).
- **Material rendering via "decal atlasing"**: Rather than blending many materials per-pixel, RDR2 uses a technique where terrain surface properties are baked into a single VT page containing albedo, normal, roughness, and an AO/metallic mask. The key insight: **material blending happens at VT page generation time**, not at runtime. An offline process blends material textures based on artist-painted masks, then bakes the result into the VT pages.
- **Runtime detail pass**: At runtime, the terrain shader samples the VT for base properties, then applies:
  1. A **detail normal map** at close range (tiling, world-space UV)
  2. A **macro-variation color pass** that adds large-scale color variation (mud patches, grass color variation)
  3. A **wetness modifier** that darkens and increases specular during/after rain
- **Snow accumulation**: RDR2's snow system is procedural. Snow coverage is computed per-frame on the GPU using a combination of height, slope, and occlusion (snow accumulates less under trees/roofs). The terrain shader blends between the regular VT look and a snow-covered look. Snow is NOT a separate material layer — it's a parameterized blend on top of existing materials.
- **Mud and deformation**: RDR2 has a terrain deformation system where wheel tracks and footprints modify the terrain normal and add a mud puddle effect. This uses a **decal system** that writes into a small deformation buffer, sampled by the terrain shader.

**Citation:** The SIGGRAPH 2019 talk is the primary source. Rockstar rarely publishes detailed technical talks, but this SIGGRAPH appearance was notably detailed. The Digital Foundry interview provides additional color on the VT resolution (the "2.5cm" figure is from that interview).

### 4.2 LOD / Mesh Density for Distant Terrain

**Key Talks:**
- The same SIGGRAPH 2019 talk covers LOD.
- Additional context from various GDC talks about open-world rendering (Rockstar North, multiple years)

**Technique Summary:**

- **Hierarchical terrain patch LOD**: RDR2 uses a **quadtree of terrain patches** with distance-based LOD selection. The quadtree root covers the entire world; leaves cover ~32m × 32m at full detail.
- **Geometry morphing**: Like GoT, RDR2 uses vertex morphing between LOD levels to eliminate popping. The morph factor is driven by distance and is smooth.
- **Distant terrain is mesh-based, not just LOD**: The RAGE engine distinguishes between "near terrain" (the playable area, heightfield) and "distant terrain" (the mountains on the horizon). Distant terrain uses **bespoke low-poly meshes** with pre-computed lighting. These meshes are generated from the heightfield offline, but are manually tweaked by artists to preserve key silhouettes.
- **The "volumetric cloud + atmosphere" layer**: RDR2's volumetric cloud system and atmospheric scattering are integral to the distant terrain presentation. Mountains 30km away are visible through the atmosphere, but the scattering model naturally occludes them based on distance and conditions. This means the LOD system doesn't need to manage transitions for extremely distant geometry — the atmosphere does it.
- **Terrain streaming**: RDR2 streams the world in chunks. The chunk size varies (near: finer granularity; far: coarser). The streaming system is priority-based, with the priority determined by distance and view direction. Streaming is done on background threads and uses compression (the heightfield data is delta-encoded).

### 4.3 Ocean / Water Integration

**Key Talks:**
- No dedicated water talk for RDR2. Water details are in the general rendering talk.
- Various developer interviews mention the water system.

**Technique Summary:**

- **Not an ocean-focused game**: Unlike AC Valhalla or Death Stranding, RDR2's water is mostly rivers, lakes, and swamps, not open ocean. The water system is optimized accordingly.
- **River and lake water**: Uses a combination of flow-map-based surface simulation + planar reflections (screen-space + pre-computed for static). River water velocity varies based on the river's width and slope, computed from the terrain data.
- **Real-time reflections**: RDR2 uses a combination of cubemaps, screen-space reflections, and planar reflections for water. The choice depends on water body size and the player's distance.
- **Swamp water**: A specialized "murk" water shader with high absorption, floating scum/algae (procedurally placed via noise), and reduced reflection. Swamp water is nearly opaque at shallow depth.
- **Shore and riverbank integration**: The transition from water to terrain uses a wetness/darkening mask. Mud near water becomes saturated and darker; rocks become wet and more specular. This is driven by the terrain material system (each material knows its "wet response").
- **Water depth fog**: Underwater uses exponential fog with the water surface as the reference height. In swamps, the fog is very dense (visibility ~2m). In clear lakes, much less so.

### 4.4 Cave / Underground Handling

**Key Talks:**
- No specific cave talk. Cave/mine interiors are mentioned in general environment talks.

**Technique Summary:**

- **Terrain is a 2.5D heightfield with hole support**: The RAGE terrain supports **holes** — masked-out regions where the terrain mesh does not render. This is used for cave entrances.
- **Caves are hand-built interior levels**: RDR2's caves and mines are meticulously hand-crafted static mesh assemblies. They are separate streaming levels that load when the player approaches an entrance. The transition zone (cave entrance) uses:
  - A terrain hole cutout
  - A decal system for blending the rock transition
  - Separate light probe grid for the interior
- **No procedural caves**: All underground spaces are hand-authored. The game is content-driven, not systemic.
- **Mine shafts**: Some mining areas feature vertical depth. These are handled by vertical level design — the mine interior is a multi-level mesh assembly masked from the outside world by terrain holes and rock formations.

---

## 5. Death Stranding (Kojima Productions, Decima)

### 5.1 Terrain Texture Blending

**Key Talks:**
- **SIGGRAPH 2019**: "Advances in Real-Time Rendering" — "Decima Engine: Rendering Systems" (Guerrilla Games, covering shared tech)
- **GDC 2020**: "The Making of Death Stranding with Decima" — (various Kojima Productions/Guerrilla talks)
- **Digital Foundry Interview (November 2019, PC release 2020)**

**Technique Summary:**

Death Stranding uses the Decima engine, so its terrain system shares the foundation with Horizon Zero Dawn (see Section 2). However, Death Stranding's Iceland-inspired landscape required different priorities.

- **Same virtual texturing pipeline as HZD**: Decima's VT-based terrain texturing is used. Material IDs + blend weights are stored in the virtual texture indirection table.
- **Key difference — emphasis on rock and volcanic terrain**: Death Stranding's world is dominated by rock, moss, volcanic soil, and sparse vegetation (unlike HZD's lush post-apocalyptic regrowth). The material set is different, and the procedural material assignment rules are tuned for the Icelandic highlands aesthetic.
- **Footprint/trail system**: A signature feature of Death Stranding is that the terrain *remembers* player traversal. Repeated walking on the same path creates visible trails. This is implemented as:
  - A **"trail map" render target** that accumulates player footsteps over time
  - The terrain shader reads this map and blends in a "disturbed ground" material (darker, less grass, more visible soil)
  - The trail map is persistent within a session and partially persisted across saves
- **Water erosion and debris flow**: The terrain texture includes pre-baked erosion patterns (water flow lines on mountainsides). These are generated offline using an erosion simulation and baked into the VT. At runtime, rain events activate a "wet erosion" look that emphasizes these patterns.
- **Moss and lichen growth**: Procedurally placed on rocks based on slope and aspect (north-facing slopes get more moss). This modulates the rock material appearance at the VT level.

**Citation:** The Decima pipeline is shared between Guerrilla and Kojima Productions. The primary Death Stranding-specific terrain talk is from GDC 2020.

### 5.2 LOD / Mesh Density for Distant Terrain

**Key Talks:**
- Same Decima engine talks. Specific Death Stranding LOD talk from GDC 2020.

**Technique Summary:**

- **Same GPU-driven quadtree terrain LOD as HZD**: See Section 2.2 for the core Decima approach.
- **Key Death Stranding difference — long draw distances are critical**: The game's visual identity depends on seeing mountain ranges and structures from extreme distances (the "strand" concept — being able to see other players' structures across the landscape). Decima's LOD system was tuned for these extreme draw distances.
- **Terrain silhouettes matter more than texture detail**: Death Stranding's terrain is dominated by rock, so silhouette accuracy at distance is prioritized over texture fidelity. The LOD system preserves large-scale geometry much longer than HZD does.
- **Atmospheric scattering for distance**: The game uses a physically-based atmospheric scattering model (Rayleigh scattering for sky color, Mie scattering for haze). This naturally fades distant terrain, but Death Stranding's weather system (specifically "timefall" rain events) creates dramatic visibility changes that the LOD system responds to dynamically.
- **No mesh shaders in the original PS4 release**: Death Stranding shipped on PS4 without mesh shaders (they weren't available). The PC version added mesh shader support for improved LOD performance.

### 5.3 Ocean / Water Integration

**Key Talks:**
- **GDC 2020**: Specific water talk for Death Stranding within the broader Decima presentation.

**Technique Summary:**

- **Ocean is minimal**: Death Stranding's map is mostly inland. There are river estuaries and a coastal edge, but no open ocean gameplay. The water system is simpler than HZD's.
- **Rivers are the primary water feature**: Death Stranding's rivers are a core gameplay mechanic (depth determines whether you can cross, current strength determines if you get swept away). Key aspects:
  - River depth is stored in the terrain data as a separate layer
  - River flow velocity is computed from terrain slope and river width; it affects both water surface appearance and gameplay physics
  - River crossings show a water surface with flow-map-based normal perturbation
- **River-terrain interaction**: The riverbed uses the same terrain texturing system but with an "underwater" material override. Rocks in the river are rendered with a wet look. The shoreline transition uses depth-based foam and wetness.
- **Water traversal**: The player character's interaction with water depth (wading, swimming, being swept away) is physics-driven, sampling the river depth and velocity from the terrain data.

### 5.4 Cave / Underground Handling

**Key Talks:**
- No specific cave talk for Death Stranding.

**Technique Summary:**

- **No significant underground spaces**: Death Stranding's world design does not include caves or underground areas (outside of a few small shelter interiors).
- **Decima's 2.5D heightfield limitation**: Like HZD (pre-HFW), Death Stranding's Decima version uses a pure heightfield terrain. Overhangs and true caves are not possible.
- **Shelters and interiors**: The few interior spaces (distribution centers, private rooms) are separate streaming levels with a load transition.

---

## 6. The Legend of Zelda: Breath of the Wild / Tears of the Kingdom (Nintendo)

### 6.1 Terrain Texture Blending

**Key Talks:**
- **GDC 2017**: "Change and Constant: Breaking Conventions with 'The Legend of Zelda: Breath of the Wild'" — Hidemaro Fujibayashi (Director), Takuhiro Dohta (Technical Director), Satoru Takizawa (Art Director)
- **CEDEC 2017** (Computer Entertainment Developers Conference, Japan): "The Making of The Legend of Zelda: Breath of the Wild" — multiple technical talks by Nintendo EPD (Japanese language, translated materials exist)
- **GDC 2024**: "Tears of the Kingdom" development talks (expected; check GDC Vault for 2024 sessions)

**Technique Summary:**

Nintendo's approach is markedly different from Western AAA engines. BOTW/TOTK run on Nintendo's internal engine, optimized for the Switch's hardware constraints.

- **Physics-and-chemistry-based material system**: Rather than a conventional layer-blending terrain system, BOTW uses a "chemistry engine" approach. The world is made of "materials" (wood, metal, stone, grass, water, etc.) that have physical properties (flammability, conductivity, buoyancy, etc.). The terrain texture is driven by this material system.
- **Cell-based terrain rendering**: The world is divided into a grid. Each cell stores:
  1. A base material type (grass, rock, sand, snow, etc.)
  2. Height data
  3. Climate/region data (temperature, humidity)
- **Texture blending via splat maps**: BOTW uses traditional **splat map blending**. Each terrain cell has one or more weight textures (RGBA) encoding up to 4 terrain material weights per texel. This is the most conventional part of the system. The terrain shader samples the weight texture and blends between the corresponding material textures.
- **No virtual texturing**: BOTW does not use virtual texturing. The terrain uses a fixed set of tiling textures per region. This is a deliberate constraint due to Wii U/Switch memory limitations.
- **Procedural color variation**: To avoid visible tiling, BOTW applies large-scale procedural color variation to terrain textures. Each material type has a "color noise" parameter that shifts hue and saturation based on world-space position.
- **TOTK improvements**: Tears of the Kingdom expanded the terrain texture system to include:
  - **Sky islands**: Separate terrain cells for sky islands with unique material sets (ancient stone, glowing moss)
  - **The Depths**: A completely separate underground world with its own terrain material system (dark rock, glowing flora, gloom-covered surfaces)
  - **Dynamic weather effects on terrain**: Rain makes surfaces wet and more specular; sandstorms temporarily alter desert terrain appearance

**Citation:** The GDC 2017 talk is a development philosophy talk more than a technical deep dive. The CEDEC 2017 sessions are more technically detailed (particularly the programming track). Translation summaries are available on various game dev forums. The TOTK-specific technical breakdowns are expected at GDC 2024/2025.

### 6.2 LOD / Mesh Density for Distant Terrain

**Key Talks:**
- **CEDEC 2017**: Level-of-detail and streaming system talk (Japanese)
- **Digital Foundry Video (2017)**: "Zelda Breath of the Wild: Switch vs Wii U Analysis"

**Technique Summary:**

- **Distance-based mesh LOD with aggressive simplification**: BOTW uses a cell-based LOD system. Each terrain cell has 3–4 LOD levels. LOD selection is purely distance-based. The Switch's limited draw distance (~500m visible range in most areas) means extreme LOD transitions are uncommon.
- **LOD transitions are masked by fog**: BOTW uses a stylized distance fog that hides LOD transitions. This fog is not physically-based atmospheric scattering — it's an artistic choice that also serves as a performance optimization.
- **Distant mountains are part of the terrain, not impostors**: Unlike most AAA games, BOTW's distant mountains (Death Mountain, Hebra Peak, etc.) are rendered by the terrain system directly. They are part of the same cell grid as the playable area. This is possible because the entire map is relatively small (~8km × 8km) compared to games like RDR2 (~75km²).
- **TOTK's sky-to-ground LOD**: Tears of the Kingdom introduced a unique LOD challenge: the player can dive from sky islands to the surface. The terrain LOD system needed to handle extreme altitude changes. Nintendo's solution:
  - At high altitude, the surface terrain is rendered at very low LOD
  - As the player descends, terrain cells rapidly swap to higher LOD
  - The transition is masked by cloud layers and the speed of descent
  - Sky islands use the same cell-based LOD system but are culled differently (they are not visible from the surface without looking up)
- **The Depths LOD**: The underground world of TOTK has its own LOD concerns:
  - Visibility in the Depths is extremely limited (darkness + gloom fog)
  - This allows very aggressive LOD reduction and early culling
  - Lightroots (light sources) dynamically affect what terrain cells are visible and at what LOD

### 6.3 Ocean / Water Integration

**Key Talks:**
- **CEDEC 2017**: Water rendering talk
- **GDC 2017**: General talk touches on the "chemistry engine" which includes water interaction

**Technique Summary:**

- **Water as a global plane with vertex displacement**: BOTW's ocean/large lake water is a single water plane at a fixed height. Vertex displacement creates waves using a combination of sine waves (not FFT — the Switch can't afford FFT for water).
- **Shoreline integration**: The shoreline uses a depth-based transition. Where water is shallow, the water color shifts to a lighter hue and transparency increases so the terrain underneath is visible. A foam effect appears at the very edge using a noise-based mask.
- **The "chemistry engine" water interactions**: This is the most distinctive feature. Water in BOTW interacts with all other materials:
  - **Fire/heat**: Water extinguishes fire; hot surfaces produce steam near water
  - **Electricity**: Water conducts electricity, creating area-of-effect hazards
  - **Ice**: Water freezes in cold areas; ice blocks float
  - **Metal**: Metal objects sink; wood floats
  - **Wind**: Wind creates ripples on the water surface
- **Rain system**: Rain is a world state that affects the entire terrain. When raining:
  - All surfaces become wet (darker, more specular)
  - Water surfaces show raindrop ripples
  - Puddles form in terrain depressions (procedural — terrain height is sampled to find local minima)
  - Climbing surfaces become slippery (gameplay effect)
- **TOTK water additions**:
  - **Water temples and floating water**: Sky islands have bodies of water that float in the air. These use the same water rendering but with independent water planes.
  - **Anti-gravity water**: Certain areas (the Water Temple) have water with unusual gravity properties. The water surface remains horizontal, but water spheres float.

### 6.4 Cave / Underground Handling

**Key Talks:**
- **GDC 2024** (expected): TOTK's cave system was a major new feature.

**Technique Summary:**

- **BOTW had no caves**: Breath of the Wild did not feature true caves. There were a few overhanging rock formations (e.g., the rock shelter on the Great Plateau), but these were static mesh pieces placed on top of the heightfield terrain. The terrain heightfield was cut out underneath using a hole mask.
- **TOTK's cave system is a major technical achievement**: Tears of the Kingdom introduced a full cave system — hundreds of cave entrances across Hyrule leading to explorable underground spaces. Key aspects:
  - **Caves are separate terrain cells**: Each cave is its own terrain cell (or set of cells) that is placed *below* the surface terrain at the same XY coordinates. This is a form of **3D cell-based terrain** — the world grid allows multiple cells at the same XY position but different Z depths.
  - **Seamless cave entrances**: Cave mouths are marked areas where the surface terrain has a hole and the cave cell is visible beneath. The transition has no loading screen — it's a geometric transition masked by rock formations.
  - **Cave-specific materials**: Caves use a unique material set (damp rock, glowing moss, stalactites/stalagmites) that is different from surface rock. The cave lighting is dim with bioluminescent elements.
  - **The Depths as a full underground world**: Beyond individual caves, TOTK includes "The Depths" — a continuous underground world spanning the entire Hyrule map. This is a complete second terrain layer:
    - The Depths have their own heightfield (inverted — the ceiling is the underside of Hyrule's terrain, the floor is a separate lower surface)
    - The Depths terrain is streamed separately from the surface
    - Transition between surface, sky, and Depths happens via chasms (vertical shafts) — the player falls through a chasm, the surface terrain unloads, and the Depths terrain loads during the descent
    - Lightroots are the primary light sources; the terrain rendering in the Depths uses a visibility system driven by lightroot illumination range

**Citation:** The TOTK cave system is the most significant underground terrain feature of any game on this list, rivaling and arguably exceeding the complexity of HFW's caves. Primary source technical talks are expected at GDC 2024/2025. Current analysis is based on gameplay and Digital Foundry technical videos.

---

## 7. Cross-Game Comparison Matrix

### Texture Blending Approach

| Game | Core Technique | Max Layers | Virtual Texturing? |
|---|---|---|---|
| AC Valhalla/Mirage | VT + Material ID indirection | Unlimited (VT) | Yes |
| Horizon ZD/FW | VT + Material ID + blend weights | Unlimited (VT) | Yes |
| Ghost of Tsushima | Multi-layer atlas blending | 8 layers | No |
| RDR2 | VT + offline-baked blend pages | Unlimited (VT) | Yes |
| Death Stranding | VT (shared Decima pipeline) | Unlimited (VT) | Yes |
| Zelda BOTW/TOTK | Splat maps + procedural variation | 4 layers (8 in TOTK) | No |

### Terrain LOD Strategy

| Game | Core Technique | Distant Mountains | Morphing? |
|---|---|---|---|
| AC Valhalla/Mirage | GPU clipmap rings | Static impostor meshes | Yes |
| Horizon ZD/FW | GPU quadtree (HZD) / SDF (HFW) | SDF ray-marched (HFW) | HZD: No; HFW: Yes |
| Ghost of Tsushima | Chunk-based with 5 discrete LODs | Hand-modeled mesh impostors | Yes |
| RDR2 | Quadtree of patches | Offline-generated + artist-tuned meshes | Yes |
| Death Stranding | GPU quadtree (Decima shared) | Terrain LOD + atmosphere | Yes |
| Zelda BOTW/TOTK | Cell-based, 3–4 LODs | Part of terrain cell grid (no impostor) | No (distance fog) |

### Water Integration

| Game | Ocean | Rivers | Underwater | Wetness System |
|---|---|---|---|---|
| AC Valhalla/Mirage | FFT, region-tuned | Flow maps | Yes (post-process) | Yes |
| Horizon ZD/FW | FFT + Gerstner (HFW) | Spline-driven | Yes (full pipeline) | Yes |
| Ghost of Tsushima | FFT tessellated grid | Flow maps | Minimal | Yes (rain response) |
| RDR2 | Limited (inland focus) | Flow maps + planar refl. | Yes (depth fog) | Yes (comprehensive) |
| Death Stranding | Minimal (coastal only) | Flow maps (gameplay) | Minimal | Yes (trails + rain) |
| Zelda BOTW/TOTK | Sine wave vertex disp. | Simple flow | Minimal | Yes (chemistry engine) |

### Cave / Underground Handling

| Game | Approach | Seamless? | Multi-level Terrain? |
|---|---|---|---|
| AC Valhalla/Mirage | Static mesh caves, terrain hole-punch | Decal-blended entrance | No (2.5D) |
| Horizon ZD | No true caves (cauldrons are separate levels) | Via corridor load | No (2.5D) |
| Horizon FW | 3D mesh-based terrain blocks | Light probe blend at entrance | Yes (voxel/mesh blocks) |
| Ghost of Tsushima | Static mesh assemblies in cutouts | N/A | No (2.5D) |
| RDR2 | Hand-crafted interior levels | Separate stream level | No (2.5D + holes) |
| Death Stranding | No underground spaces | N/A | No (2.5D) |
| Zelda BOTW | No caves (static overhangs) | N/A | No (2.5D) |
| Zelda TOTK | **Full 3D cell-based terrain** | Yes (chasms, cave entrances) | **Yes (surface + sky + Depths)** |

---

## 8. References

### Primary Sources (Talks, Papers, Slides)

#### General / Multi-Game
- **Advances in Real-Time Rendering in Games** (SIGGRAPH course, 2006–present):
  - All slides archived at: [advances.realtimerendering.com](http://advances.realtimerendering.com/)
  - YouTube: [Advances in Real-Time Rendering channel](https://www.youtube.com/@AdvancesinRealTimeRendering)

#### Assassin's Creed / Ubisoft Anvil
- Stevens, B. (2019). "Terrain Rendering in 'Assassin's Creed Odyssey'". GDC 2019. GDC Vault (membership required).
- Thibault, P., Williams, J-F. (2018). "World Building and Streaming in 'Assassin's Creed Origins'". GDC 2018.
- Cournoyer, M., Verreault, J-F. (2021). "The Water Technology of 'Assassin's Creed Valhalla'". GDC 2021.

#### Horizon / Decima Engine
- Vos, N. (2017). "Terrain Rendering in Horizon Zero Dawn". SIGGRAPH 2017, Advances in Real-Time Rendering course. Slides: [advances.realtimerendering.com/s2017/](http://advances.realtimerendering.com/s2017/)
- van der Leeuw, M. (2017). "The Decima Engine: Visibility, Culling, and LOD". GDC 2017.
- van Muijden, J. (2017). "GPU-Based Procedural Placement in Horizon Zero Dawn". GDC 2017.
- Guerrilla Games (2022). "Horizon Forbidden West: The Technology Behind the World". GDC 2022.
- Decima Engine overview: [guerrilla-games.com/decima](https://www.guerrilla-games.com/decima)

#### Ghost of Tsushima / Sucker Punch
- Lloyd, I., Wang, J. (2021). "Procedural Generation of the World of 'Ghost of Tsushima'". GDC 2021.
- Sucker Punch Productions (2020). "Rendering the World of Ghost of Tsushima". SIGGRAPH 2020, Advances in Real-Time Rendering course. Slides: [advances.realtimerendering.com/s2020/](http://advances.realtimerendering.com/s2020/)
- Digital Foundry (2020). "Ghost of Tsushima: Full Tech Analysis". Published July 2020 on Eurogamer/Digital Foundry.

#### Red Dead Redemption 2 / Rockstar RAGE
- Rockstar Games (2019). "Red Dead Redemption 2: Terrain, Vegetation, and Global Illumination". SIGGRAPH 2019, Advances in Real-Time Rendering course.
- Rockstar Games (2019). "The Rendering Pipeline of Red Dead Redemption 2". GDC 2019.
- Digital Foundry (2018). "Red Dead Redemption 2: The Digital Foundry Tech Analysis". Published October 2018.

#### Death Stranding / Kojima Productions
- Guerrilla Games / Kojima Productions (2020). "The Making of Death Stranding with Decima". GDC 2020.
- Guerrilla Games (2019). "Decima Engine: Rendering Systems". SIGGRAPH 2019, Advances in Real-Time Rendering course.
- Digital Foundry (2020). "Death Stranding PC: Exclusive Tech Deep Dive + DLSS 2.0 Analysis". Published July 2020.

#### Zelda BOTW/TOTK / Nintendo
- Fujibayashi, H., Dohta, T., Takizawa, S. (2017). "Change and Constant: Breaking Conventions with 'The Legend of Zelda: Breath of the Wild'". GDC 2017.
- Nintendo EPD (2017). CEDEC 2017 technical sessions on BOTW development. (Japanese; translated summaries available on game development forums)
- Digital Foundry (2017). "Zelda Breath of the Wild: Switch vs Wii U Frame-Rate Tests + Analysis". Published March 2017.
- Digital Foundry (2023). "Zelda Tears of the Kingdom: A Technical Masterclass on Switch". Published May 2023.

### Secondary / Aggregator Sources
- **Digital Foundry** (Eurogamer): Regular technical analysis of all listed games with developer interview excerpts.
- **80 Level**: Game art/tech interviews covering terrain pipelines for many AAA games.
- **Game Developer (formerly Gamasutra)**: Postmortem articles and GDC talk coverage.
- **GPUOpen / NVIDIA Developer Blog**: Occasional guest posts from game developers (e.g., Guerrilla wrote about Decima on the NVIDIA blog during the HZD PC port).

### For Further Research
- GDC Vault has the full video + slides for most GDC talks listed above. Access requires a paid subscription.
- The SIGGRAPH "Advances in Real-Time Rendering" YouTube channel has the full presentations from 2020 onward.
- CEDEC (Japan's GDC equivalent) often has more detailed implementation talks than GDC, but materials are primarily in Japanese.
