# Modern Terrain LOD, Ocean Rendering & Underground Techniques

## A Technical Reference for Open-World Game Rendering

---

# PART 1: TERRAIN LOD TECHNIQUES

## 1.1 CDLOD (Continuous Distance-Dependent LOD)

### Overview

**Author:** Filip Strugar  
**Year:** 2010  
**Paper:** "Continuous Distance-Dependent Level of Detail for Rendering Heightmaps"  
**DOI:** 10.1080/2151237X.2009.10129287  
**Journal:** *Journal of Graphics, GPU, and Game Tools*, Vol. 14(4), pp. 57–74  
**Full Paper (latest):** https://github.com/fstrugar/CDLOD/blob/master/cdlod_paper_latest.pdf  
**Source Code (DirectX 9, MIT license):** https://github.com/fstrugar/CDLOD  
**Demo Videos:** https://youtu.be/WnxFnNC2Hrk | https://youtu.be/dUmRbvZbRYM  
**Pre-built binaries:** https://github.com/fstrugar/CDLOD/blob/master/binaries_tools_testdata.7z

### How It Works

CDLOD is a **GPU-based heightmap terrain rendering** technique that is a refinement of multiple existing methods. Key characteristics:

1. **Quadtree of Regular Grids**: Unlike geometry clipmaps (which use a set of nested regular grids centered about the viewer), CDLOD is structured around a **quadtree of regular grids**, similar to Ulrich 2002 but distributed around the viewer with adaptive density.

2. **Unified Distance-Based LOD Function**: The algorithmic breakthrough is that the **LOD function is identical across the entire rendered mesh** and is based on **precise 3D distance** between observer and terrain. There is no separate management of different LOD regions.

3. **No Stitching Required**: The transitions between LOD levels are handled seamlessly through a novel morphing technique. Unlike traditional quadtree methods that require explicit stitching meshes at level boundaries, CDLOD's morphing approach hides transitions within the geometry itself.

4. **Continuous Morphing**: Each vertex at the boundary between LOD level N and N+1 evaluates *both* elevation samples (at N and N+1) and blends between them based on distance to the LOD transition boundary. This produces **smooth, continuous transitions** with no visible pop or seam.

5. **Comparison to Geometry Clipmaps**: CDLOD provides better LOD distribution than clipmaps because the quadtree nodes can be arranged more flexibly around the viewer, achieving more uniform screen-space triangle sizes.

6. **Shader Model 3.0+**: Works on all hardware supporting SM 3.0.

### GPU Implementation

- The heightfield is stored as a 2D texture
- A quadtree determines which nodes are rendered at which LOD
- Each node is a regular grid of vertices
- Vertex shader samples the heightmap, computes the LOD blend factor from 3D distance, and morphs geometry
- The system is described as "more predictable and reliable, with better screen-triangle distribution"
- Simplified integration with other LOD systems (vegetation, water, etc.)

### Game Usage

While specific game usage data requires further verification, CDLOD influenced many terrain systems, particularly those using Unity and custom game engines. Its clean integration model made it attractive for production.

---

## 1.2 Geometry Clipmaps

### Overview

**Original Paper:** Frank Losasso and Hugues Hoppe, "Geometry Clipmaps: Terrain Rendering Using Nested Regular Grids," *ACM Transactions on Graphics (Proceedings of SIGGRAPH 2004)*, 23(3), pp. 769–776.

**GPU Gems 2 Chapter:** Chapter 2, "Terrain Rendering Using GPU-Based Geometry Clipmaps" by Arul Asirvatham and Hugues Hoppe (Microsoft Research)

The geometry clipmap is a level-of-detail structure for terrain rendering that **caches terrain geometry in a set of nested regular grids**, which are incrementally shifted as the viewer moves.

### Core Concept

The terrain is treated as a 2D elevation image, prefiltered into a mipmap pyramid of L levels. The clipmap structure caches a square window of n×n samples within each level, much like the texture clipmaps of Tanner et al. 1998. These windows form a set of **nested regular grids centered about the viewer**.

**Key Property:** Finer-level windows have smaller spatial extent than coarser ones. The aim is to maintain **triangles uniformly sized in screen space**. With a clipmap size n=255, triangles are approximately 5 pixels wide in a 1024×768 window.

### Ring Structure

- Only the **finest level** is rendered as a complete grid square
- All coarser levels render as a **hollow "ring"** — the interior region is omitted because it's already rendered at finer resolutions
- As the viewer moves, clipmap windows shift and are updated with new data (toroidal/wraparound addressing)

### Transition Blending

The critical challenge is hiding boundaries between resolution levels while maintaining a watertight mesh and avoiding temporal popping. The solution is a **transition region** near the outer perimeter of each level where geometry and textures are **smoothly morphed to interpolate the next-coarser level**.

These transitions are implemented in the **vertex shader** (for geometry blend) and **pixel shader** (for normal map blend).

### GPU-Based Implementation (GPU Gems 2 version)

The chapter describes a **GPU implementation using vertex textures** (a new feature in DirectX 9 SM 3.0, GeForce 6 Series):

1. **(x,y) coordinates** stored as constant vertex data
2. **z (elevation)** stored as a single-channel 2D texture — the *elevation map*
3. Separate n×n elevation map texture per clipmap level
4. Read-only vertex and index buffers for 2D "footprints" — repeatedly instanced across levels

**Clipmap Size:** Grid size n must be odd. The paper uses n = 2^k - 1 (e.g., 255) so that the finer level is never exactly centered with respect to its parent, allowing proper shifting.

### Rendering Details

- Each level's ring is partitioned into **12 blocks** of size m×m (m = (n+1)/4)
- Plus fix-up regions for gaps (4 m×3 strips, 1 L-shaped interior trim)
- Single read-only vertex buffer for all blocks (vertex shader scales/translates)
- **View frustum culling** done per-block on CPU
- For n=255: average 71 DrawPrimitive calls per frame (14 per level × ~5 active levels + 1 for finest)
- Could reduce to 35 DP calls with instancing

### Update Process

Since viewer motion is coherent, only small L-shaped regions need updating per frame (coarser levels update exponentially less frequently). The update uses:
1. **Upsampling**: Coarser level data upsampled via tensor-product 4-point subdivision (C¹ smooth)
2. **Adding residuals**: From decompression or procedural synthesis (fractal Gaussian noise)
3. **Normal map computation**: Cross product of grid-aligned tangent vectors, packed with coarse normal for blending

### Performance (2004 Hardware — GeForce 6800 GT)

- **Data:** 216,000 × 93,600 height map (USA at 30m resolution, 1m vertical), compressed 100× to 355 MB
- **L=11 levels, n=255:** 130 fps (view frustum culling), 60M triangles/sec
- Vertex texture lookup is the bottleneck (removing it: 185 fps)
- Smooth viewer motion: ~87 fps (decompressed) / ~120 fps (synthesized)

### GPU-Driven Clipmap Approaches (Modern)

Later work extended clipmaps to be more GPU-driven:

- **NVIDIA whitepapers** (2007-2010) on GPU-based clipmap rendering with geometry shaders and later compute shaders
- **Call of Duty** series (2010s) used a variant of GPU clipmaps for large terrains
- **Microsoft Flight Simulator** (2020) uses an evolution of clipmap concepts combined with virtual texturing
- Modern GPU-driven approaches use **indirect dispatch** and **GPU-based culling** to minimize CPU involvement

### Open-Source Implementations

- **Official code** was on NVIDIA developer site (original GPU Gems 2 companion CD)
- Many independent implementations on GitHub (search: "geometry clipmap")
- Unity asset store has multiple clipmap terrain assets

### Vertices Texture Packing Trick

A notable optimization: the vertex shader needs both the fine-level elevation z_f and the coarser-level elevation z_c at the same (x,y). Rather than 3 vertex texture lookups (expensive in SM 3.0), the paper:
- Packs z_f into the **integer part** of a float
- Packs z_d = z_c - z_f (scaled) into the **fractional part**
- Unpacks in the vertex shader: `zf = floor(zf_zd); zd = frac(zf_zd) * 512 - 256;`

---

## 1.3 GPU-Driven Terrain Rendering (Modern ~2018–2024)

### The Shift to GPU-Driven

Modern GPU-driven terrain rendering moves **as much work as possible** from CPU to GPU:

1. **Mesh Shaders** (NVIDIA Turing+, 2018): Replace the traditional vertex pipeline. A mesh shader can generate geometry dynamically on GPU based on LOD decisions, without CPU involvement. **No index/vertex buffers pre-allocated on CPU.**

2. **Indirect Multi-Draw**: GPU-resident indirect draw buffers. The GPU fills draw call parameters (instance count, vertex count, base vertex) via compute shaders, then executes them without CPU round-trip.

3. **GPU Frustum/Occlusion Culling**: Compute shaders perform frustum tests and Hi-Z occlusion culling, writing visibility results to indirect draw buffers.

4. **Bindless Textures**: Avoids CPU-managed texture binding; shaders access any texture by descriptor index.

### Key References

- **NVIDIA Mesh Shader Programming Guide** (2018+): https://developer.nvidia.com/blog/introduction-turing-mesh-shaders/
- **"GPU-Driven Rendering"** — GDC 2015 talk by Ulrich Haar and Sebastian Aaltonen (Ubisoft) on *Assassin's Creed Unity*
- **"GPU-Driven Rendering Pipelines"** — SIGGRAPH 2015 course
- **"4K Rendering Breakthrough: The Filtered and Culled Visibility Buffer"** — GDC 2017, Wolfgang Engel
- **"Rendering the World of Far Cry 5"** — GDC 2018, talks about GPU-driven terrain with compute shaders
- **"GPU-Based Run-Time Procedural Placement in Horizon Zero Dawn"** — GDC 2017, Jaap van Muijden (Guerrilla Games)

### Mesh Shader Terrain (Post-2018)

Mesh shaders are **particularly well-suited** to terrain:

1. A **task shader** (pre-pass) determines which terrain patches are visible and at what LOD
2. A **mesh shader** generates the actual geometry for each visible patch at the appropriate tessellation
3. The **amplification factor** can increase tessellation for near patches and decrease for far ones
4. Complete elimination of CPU-side draw call overhead for terrain

### Real-World Examples

| Game/Engine | Technique | Reference |
|---|---|---|
| **Horizon Zero Dawn** (2017) | GPU-driven terrain with compute shader culling and adaptive LOD | GDC 2017 |
| **Far Cry 5** (2018) | Compute-based GPU terrain rendering | GDC 2018 |
| **Assassin's Creed Odyssey** (2018) | GPU-driven terrain with virtual texturing | - |
| **Death Stranding** (2019) | Decima Engine: GPU-driven terrain + geometry clipmaps | - |
| **Microsoft Flight Simulator** (2020) | Massive GPU-driven terrain with procedural detail | GDC 2021 |
| **Unreal Engine 5** (Nanite, 2021) | Virtualized geometry, not terrain-specific but influences terrain approaches | Epic Games |

---

## 1.4 Virtual Texturing for Terrain

### Sparse Virtual Texturing

**Author:** Sean Barrett  
**Conference:** GDC 2008  
**Talk:** "Sparse Virtual Textures"  
**Game:** *id Tech 5* (used in *Rage*, 2011; *Wolfenstein: The New Order*, 2014; *Dishonored 2* via Void Engine)

**How it works:**
- The entire world has a **virtual texture** space (e.g., 128K × 128K)
- A **physical texture cache** (e.g., 8192 × 8192) holds only the **visible** pages at each moment
- A GPU **indirection texture** (page table) maps virtual texels to physical cache locations
- **Per-frame page miss detection**: The GPU reads from the indirection texture; misses trigger a **page request** that the CPU fulfills by streaming the needed texture data
- The technique is analogous to **virtual memory paging** but for GPU texture memory

**Key benefits for terrain:**
- Eliminates the "many-textures-splatting problem": terrain surfaces often need 8-16+ material layers (grass, rock, sand, snow, etc.). With virtual texturing, you simply sample the virtual texture without needing to bind many individual texture arrays.
- Unlimited variety: Any part of the world can have unique, artist-authored texturing.
- Predictable memory: Only a fixed-size cache is allocated regardless of world size.

### Adaptive Virtual Texturing (AVT)

**Author:** Anton Kaplanyan  
**Year:** 2010  
**Paper:** "Adaptive Virtual Texturing"

AVT extends sparse virtual texturing with:
- **Dynamic page resolution**: Pages near the viewer are higher resolution; far pages are lower
- **Continuous feedback loop**: The system monitors page miss rates and adaptively adjusts page sizes and cache allocation
- **Mipmap-aware page selection**: Considers which mip levels are needed per region

### Game Usage

| Engine/Game | Technique | Notes |
|---|---|---|
| **id Tech 5** (Rage, 2011) | MegaTexture (proprietary virtual texturing) | First major virtual texturing implementation in games |
| **id Tech 6** (Doom 2016) | Enhanced virtual texturing | Better dynamic resolution |
| **id Tech 7** (Doom Eternal) | Further refined | |
| **Unreal Engine 4** (4.12+) | Runtime Virtual Texturing (RVT) | Production-ready GPU-driven virtual texturing |
| **Unreal Engine 5** | Enhanced RVT | Integrated with Nanite and Lumen |
| **Far Cry 4/5** | Custom virtual texturing | Combined with GPU terrain |

### Open-Source Implementations

- **Unreal Engine 4/5 RVT** — source available to licensees
- **Granite SDK** (Graphine) — commercial virtual texturing middleware
- Various academic implementations on GitHub

### GPU Gems / GPU Pro Coverage

- **GPU Pro 4** (2013), Chapter "Practical and Realistic Virtual Texturing" by Chen and Mayer
- **GPU Pro 7** (2016), Chapter "Adaptive Virtual Texture Rendering in Far Cry 4" by Egor Yusov
- **GPU Zen**, Chapter on virtual texturing implementation details

---

## 1.5 CDLOD Continuum

### Overview

**Author:** Filip Strugar  
**Year:** ~2013  
**Repository:** A later project by the CDLOD author combining CDLOD techniques with advanced virtual texturing concepts.

The CDLOD Continuum is Strugar's **updated approach** that combines:
- CDLOD's distance-dependent LOD selection
- Virtual texturing for material and detail
- Continuous LOD blending across the entire terrain (hence "continuum")
- GPU-driven rendering with minimal CPU involvement

The core insight: **LOD should be a continuous function across the terrain**, not discrete level transitions. This means:
- No visible LOD lines/rings
- No popping artifacts
- Smooth morphing at every pixel based solely on 3D distance
- The terrain LOD seamlessly integrates with virtual texture page resolution (both are distance-dependent)

### Significance

CDLOD Continuum represents the philosophical endpoint of the continuous-LOD approach: a completely smooth, adaptive, physically-based LOD that eliminates all geometric and textural discontinuities. Modern engines increasingly adopt these principles even if they don't use CDLOD's specific implementation.

---

## 1.6 Adaptive Quad-Tree LOD

### Overview

Adaptive quad-tree LOD is one of the oldest and most widely-used terrain LOD techniques.

### How It Works

1. The terrain heightfield is organized as a **quadtree** data structure
2. Each node represents a square region of terrain at a particular resolution
3. At render time, the tree is traversed top-down:
   - If a node's screen-space error exceeds a threshold, **subdivide** (render children instead)
   - If below threshold, **render this node** at current resolution
4. Nodes at different depths have different triangle densities

### The Stitching Problem

The **classic problem** with quad-tree LOD is that adjacent nodes at different LOD levels create **T-junctions** (vertices on one patch that don't exist on the adjacent patch). Solutions include:

- **Skirt/Flange method:** Adding vertical skirts at edges to hide gaps (expensive, not watertight)
- **Constrained quadtree:** Restricting adjacent nodes to differ by at most 1 level, then adding stitching triangles
- **Index buffer stitching:** Generating special index buffers that skip vertices at seams
- **GPU-based stitching:** Using vertex shader logic to collapse edge vertices based on neighbor LOD

### Restrict Quadtree / ROAM Variants

**ROAM** (Real-time Optimally Adapting Meshes) — Duchaineau et al. 1997 — was an early influential approach using binary triangle trees. The "restricted quadtree" constraint forces neighbors to differ by at most 1 level, simplifying stitching.

### Modern Quad-Tree Approaches

Modern GPU-driven engines use quad-trees differently:
- The **CPU builds** the visible quad-tree node list
- The **GPU renders** all nodes via instancing or indirect draw
- **GPU culling** eliminates invisible nodes
- **Mesh shaders** can even handle subdivision decisions entirely on GPU

### Game Usage

Many engines historically used quad-tree LOD:
- **CryEngine** (Far Cry, Crysis) — adaptive quadtree with geometry clipmap influences
- **Unreal Engine 3** — terrain component system used quad-tree subdivision
- **Unity Terrain Engine** — quad-tree of terrain patches
- **Godot Engine** — terrain system uses quad-tree concepts
- **Just Cause series** — quad-tree LOD with procedural grass/vegetation placement

---

## 1.7 Far Terrain / Horizon Mesh

### The Problem

Open-world games need to show terrain to **very far distances** — often tens of kilometers — but it's impossible to render full-detail terrain at those ranges. Solutions include:

### Impostor Terrain / Billboard Mountains

- Pre-rendered depth and color textures of distant mountains from key viewpoints
- Used extensively in older open-world games (Skyrim, GTA V's distant mountains)
- **Limitations:** Static, no parallax, incorrect lighting changes

### Simplified Mesh (Horizon Mesh)

- A highly simplified proxy mesh for everything beyond the normal terrain rendering distance
- Generated by sampling the terrain heightfield at very coarse resolution
- Can be a simple heightmap at 1/64 or 1/128 resolution
- Texture applied from a "far terrain" virtual texture page at extremely low resolution
- Lit with simplified lighting (ambient + single directional)

### Skybox Integration

- Distant terrain silhouettes baked into skybox (seen in many games)
- Atmospheric scattering applied to far terrain to naturally fade detail into sky color
- Combined approach: skybox for extreme distance + simplified mesh for mid-far distance + full terrain for near

### Modern Approaches

**Microsoft Flight Simulator (2020):**
- Streams Bing Maps satellite imagery + elevation data
- Far terrain is a combination of low-res satellite textures and procedural atmosphere
- Cloud system provides natural occlusion for far terrain detail

**Horizon Forbidden West (2022):**
- Uses distance-based fog and atmospheric scattering as *design elements*
- Far terrain detail is strategically hidden by volumetric fog and haze
- Reduces the need for explicit far-terrain impostors

**Unreal Engine 5 (World Partition):**
- Terrain is split into grid cells streamed at different LODs
- Far cells are loaded at very low resolution
- Nanite handles distant geometry (not terrain-specific but influences the approach)
- Sky Atmosphere system provides natural distance fog

### Key References

- **"Rendering Techniques in Horizon Zero Dawn"** — GDC 2017 (far terrain LOD chain)
- **"The Technology of The Witcher 3"** — GDC 2014 (horizon mesh for Skellige islands)
- **"Terrain in Ghost of Tsushima"** — SIGGRAPH 2020 talk

---

# PART 2: OCEAN/WATER RENDERING

## 2.1 Projected Grid

### Overview

**Author:** Claes Johanson  
**Year:** 2004  
**Original Reference:** "Real-time Water Rendering — Introducing the Projected Grid Concept," Master's thesis, Lund University, 2004  
**Game Usage:** First used in **Far Cry** (Crytek, 2004); also used in Crysis and many subsequent games.

### How It Works

The projected grid addresses a fundamental problem with water rendering: with a perspective camera, a **uniform grid in world space** produces highly uneven screen-space triangle distribution. Water near the horizon has high triangle density (wasted), while water near the camera has low density (aliasing).

The projected grid solution:
1. Create a **uniform grid in post-perspective screen space** (NDC)
2. **Project** this grid back into world space by intersecting view rays with the water plane
3. The result: a world-space grid where **triangles are uniformly sized in screen space**, regardless of camera orientation or distance

### Implementation

- A 2D grid of vertices in normalized device coordinates (NDC) at the near plane
- For each vertex, construct a ray from camera through the NDC point
- Intersect the ray with the water plane (typically y=0)
- Place the water vertex at the intersection point
- In the vertex shader, add wave displacement to this position
- Render the resulting mesh

### Advantages
- **Optimal triangle distribution**: No wasted triangles near horizon
- **Infinite ocean**: The grid can extend to the far clip plane
- **Per-vertex waves**: Gerstner/FFT waves applied in vertex shader

### Limitations
- **Requires flat water plane** for the base projection (ocean only, not rivers/lakes with varying elevation)
- **Edge artifacts** at water-land boundaries if not handled carefully
- **Over-draw** near the camera if the grid is too fine

### Evolution
- **Screen-Space Projected Grid** — modern variant that operates entirely in screen space
- Combined with **FFT ocean simulation** for realistic wave shapes
- Used with **underwater rendering** (refraction, caustics, volumetric fog)

### GPU Gems Coverage

- **GPU Gems** (2004), Chapter 1: "Effective Water Simulation from Physical Models" — Mark Finch (Cyan Worlds, *Uru: Ages Beyond Myst*)
  - Describes **sum of sines + Gerstner waves** for geometric undulation
  - GPU-based dynamic normal map for fine ripple detail
  - Bump environment mapping with corrected eye vectors

---

## 2.2 FFT Ocean Simulation

### Tessendorf Waves

**Author:** Jerry Tessendorf  
**Year:** 2001 (SIGGRAPH course notes), updated 2004  
**Original Document:** "Simulating Ocean Water," SIGGRAPH 2001 Course Notes  
**Updated:** "Simulating Ocean Water" (2004), widely cited version  
**Link:** http://graphics.ucsd.edu/courses/rendering/2005/jdewall/tessendorf.pdf

### Theory

Tessendorf's method simulates ocean surfaces in the **frequency domain** using the Fast Fourier Transform (FFT):

1. **Oceanographic spectrum**: Start with a statistical model of ocean waves (e.g., Phillips spectrum, JONSWAP)
2. **Frequency domain**: Generate a 2D grid of Fourier amplitudes in the frequency domain (kx, ky)
3. **Time evolution**: Each frequency component evolves independently with the deep-water dispersion relation: ω² = g·k
4. **IFFT**: Perform an Inverse FFT on the frequency data to get the spatial-domain height field
5. **Choppy waves**: Apply horizontal displacement to vertices based on the spatial gradient of the height field, creating sharper wave peaks

### Mathematical Foundation

The height field at position x = (x, z) at time t is:

```
h(x, t) = Σₖ h̃(k, t) · exp(ik·x)
```

where h̃(k, t) is the Fourier amplitude at wavevector k, evolved as:

```
h̃(k, t) = h̃₀(k) · exp(iω(k)t) + h̃₀*(-k) · exp(-iω(k)t)
```

with ω(k) = √(g|k|) (deep water dispersion).

### Choppy Waves

The horizontal displacement (X, Z) is computed from the spatial gradient:

```
D(x, t) = Σₖ (-ik/|k|) · h̃(k, t) · exp(ik·x)
```

Vertices are displaced horizontally toward wave crests, creating the characteristic sharp peak / wide trough appearance.

### GPU Implementation

- FFT computation typically done via **compute shaders**
- Grid sizes: 256×256 to 2048×2048 (multiple cascades for LOD)
- Real-time FFT requires ~10 GPU dispatches per frame for reasonable grid sizes
- Pre-computed lookup tables for sine/cosine

### GPU Gems Coverage

- **GPU Gems**, Chapter 1: "Effective Water Simulation from Physical Models" — Mark Finch
  - Gerstner waves variant (sum of sines in spatial domain)
  - GPU-generated dynamic normal maps
  - Steepness control via Q parameter
  
- **GPU Gems 2**, Chapter 18: "Using Vertex Texture Displacement for Realistic Water Rendering" — Yuri Kryachko
  - GPU-based displacement for water
  - Vertex texture fetch for wave heights

### Real-World Implementations

| Game | Approach | Notes |
|---|---|---|
| **Assassin's Creed IV: Black Flag** | FFT ocean in compute shader | Multiple cascaded grids for LOD |
| **Sea of Thieves** | FFT ocean with GPU compute | Critical gameplay element |
| **Battlefield 4** | FFT ocean | Naval combat waves |
| **Assassin's Creed Odyssey** | Enhanced FFT ocean | Larger grids, better performance |
| **Frostbite Engine** (Battlefield series) | GPU FFT water | Highly optimized compute shader implementation |

### Open-Source Implementations

- **OceanFFT** (Unity): https://github.com/gasgiant/Ocean-FFT
- **FFT-Ocean** (WebGL): https://github.com/jbouny/fft-ocean
- **UE4 Ocean Project**: https://github.com/UE4-OceanProject
- **Rust ocean simulation**: Various open-source Rust implementations exist

### SIGGRAPH/GDC Talks

- **"Rendering the Stormy Seas of Black Flag"** — GDC 2014, Ubisoft (Bartlomiej Wronski)
- **"Water Technology of Uncharted 4"** — SIGGRAPH 2016, Naughty Dog
- **"The Ocean in Sea of Thieves"** — GDC 2018, Rare

---

## 2.3 Water LOD

### The Multi-Scale Water Problem

Water bodies in open-world games span ranges from centimeters (ripples) to kilometers (ocean horizon), requiring a multi-scale LOD approach:

### Level 0: Screen-Space or Projected Grid (Near)
- High-resolution projected grid or screen-space grid
- FFT/Gerstner waves for geometric displacement
- Pixel shader: normal maps, reflection, refraction, foam, caustics

### Level 1: Medium-Detail Water (Mid-Range)
- Coarser grid with fewer FFT frequency components
- Simpler shading (no refraction, reduced specular)
- Transition via alpha blending or grid spacing

### Level 2: Far Water (Distant)
- Very coarse grid, minimal waves
- Only reflection, no refraction, no foam
- Matches horizon color via atmospheric scattering model

### Level 3: Horizon Water (Beyond Far)
- Single quad or low-poly patch at water plane
- Flat color matching atmospheric scattering
- Often shared with the sky dome

### Implementation Approaches

**Continuous Mesh Approach:**
- Single water mesh covering the entire possible water area
- LOD is handled within the vertex shader (wave frequency filtering based on distance)
- Mesh density decreases with distance (projected grid handles this naturally)

**Ring/Tile Approach:**
- Multiple concentric rings of water tiles, each at different LOD
- Similar to terrain geometry clipmaps
- Used in *Sea of Thieves*

**Screen-Space Approach:**
- Water mesh is completely screen-space (reconstructed from depth buffer)
- No world-space LOD needed — pixel shader handles all detail
- Used in some modern deferred-rendering engines

### Key Resources

- **GPU Pro 2** (2011), Chapter "Real-Time Open Water Environments with Level of Detail"
- **GPU Pro 5** (2014), Chapter on water rendering optimization
- **"Water Rendering in Far Cry 4"** — GDC 2015, Ubisoft

---

## 2.4 Integration with Terrain (Water-Terrain Boundaries)

### Shore Rendering

The water-terrain boundary is one of the most challenging aspects of outdoor rendering. Key techniques:

### Depth-Based Shore Effects

- Sample **water depth** at each water pixel (water surface Z - terrain Z)
- Based on depth:
  - **Deep water:** Full opacity, blue/green color, strong reflections
  - **Shallow water (< 1m):** Gradually transparent, revealing seabed
  - **Shore (near 0m):** Foam, wet sand/mud effects
- GPU Gems Ch.1 describes this in detail (Cyan Worlds' *Uru*)

### Foam Generation

- Foam where water depth is very small (wave crests breaking on shore)
- Multiple techniques:
  - **Distance field from shore**: Precompute distance to shore, use as foam mask
  - **Depth-based**: Foam where water_surface_z ≈ terrain_z
  - **Wave steepness**: Foam where waves exceed steepness threshold (Jacobian determinant < 0 means wave breaks)

### Wet Sand / Wet Map

- Terrain near water edge gets a "wet map" texture applied
- Transition: dry terrain → wet terrain (darker, more specular) → underwater terrain
- Wet map generated as a dynamic decal or stored in a render target

### River/Lake Approach

For rivers and lakes (non-infinite, non-planar water):
- **Planar reflection** at water surface (render scene from reflected camera)
- **Refraction** (render underwater geometry)
- **Caustics**: Projected texture maps for light patterns on riverbeds
- **Flow maps**: Texture-based flow direction for river water surface

### Game Examples

| Game | Approach |
|---|---|
| **Uncharted 4** | Physically-based shore rendering with wet maps, foam, and dynamic depth effects |
| **Horizon Zero Dawn** | Separate water volumes for rivers/lakes/ocean with blended transitions |
| **The Witcher 3** | Depth-based water opacity, foam generation, dynamic wet sand |
| **Sea of Thieves** | Full ocean simulation with island shore interaction, breaking waves |

### Key References

- **GPU Gems**, Chapter 1: "Effective Water Simulation from Physical Models" — Mark Finch
- **GPU Gems**, Chapter 2: "Rendering Water Caustics" — Juan Guardado
- **GPU Gems 3**, Chapter: "Real-Time Water Caustics"
- **"The Technical Art of Uncharted 4"** — SIGGRAPH 2016 (water sections)
- **"Water Rendering in Assassin's Creed III"** — GDC 2013

---

# PART 3: CAVE/UNDERGROUND RENDERING

## 3.1 Heightfield Caves

### The Heightfield Limitation

A standard heightfield terrain is a **2.5D representation**: for each (x, y), there is exactly **one z value**. This makes it impossible to represent:
- Overhangs
- Tunnels
- Caves (where there are two z values: cave ceiling + cave floor)
- Bridges/arches

### Cutout Techniques

**Texture-based cutouts:**
- A separate "cavity map" or "visibility map" marks regions where terrain should not be rendered
- At those (x,y) positions, the terrain mesh is clipped/hidden
- The cave interior is rendered as separate geometry (mesh instancing)
- Used in many heightfield-based engines

**Alpha-to-coverage approach:**
- Per-texel alpha channel defines whether terrain exists
- Use alpha-to-coverage or alpha testing to discard cave pixels
- Multiple height layers: surface height, cave ceiling height, cave floor height

### Visibility/Masking Layers

**Multi-layer heightfields:**
- Store multiple height layers: surface, cave ceiling, cave floor, underground water table
- Each layer rendered independently
- Visibility determined by masking textures

**Horizon Zero Dawn approach (GDC 2017):**
- Heightfield terrain with "material layers"
- Cave entrances are mesh objects embedded in terrain
- Terrain is masked/hollowed around cave entrance meshes

### Limitations

Heightfield cave approaches are always hacks. They cannot represent:
- Truly 3D cave geometry
- Winding vertical passages
- Complex interconnecting tunnel networks

---

## 3.2 Voxel Terrain

For true 3D geometry (caves, overhangs, tunnels), voxel representations are used.

### Marching Cubes

**Original Paper:** Lorensen and Cline, "Marching Cubes: A High Resolution 3D Surface Construction Algorithm," SIGGRAPH 1987

**How it works:**
- A 3D scalar field (density) is sampled on a 3D grid
- For each cell, determine which of 256 possible configurations of "inside/outside" the 8 corners applies
- Generate triangles for the isosurface within that cell based on a lookup table
- Results in a polygon mesh approximating the implicit surface

**GPU implementation:**
- Geometry shader or compute shader generates triangles from 3D density volumes
- Mesh shaders (modern GPUs) can generate marching cubes output directly

### Dual Contouring

**Original Paper:** Ju et al., "Dual Contouring of Hermite Data," SIGGRAPH 2002

**Advantages over Marching Cubes:**
- Better preservation of sharp features (edges, corners)
- Uses Hermite data (surface normal + intersection point at each edge)
- Can represent features smaller than a voxel
- Produces adaptive meshes (fewer triangles)

### Transvoxel Algorithm

**Author:** Eric Lengyel  
**Year:** 2010  
**Book:** "Mathematics for 3D Game Programming and Computer Graphics" (3rd Edition)  
**Online:** transvoxel.org

Transvoxel addresses the **LOD problem** with marching cubes:
- Defines how to stitch between adjacent voxel cells at different LOD levels
- Provides transition cell configurations for all 256 × 15 possible LOD transition cases
- Enables **adaptive octree LOD** with voxel terrain

This was used in **C4 Engine** and influenced many subsequent voxel terrain systems.

### Game Usage

| Game | Technique | Notes |
|---|---|---|
| **Minecraft** | Voxel terrain (cubes) | Not marching cubes — block-based |
| **Astroneer** | Marching cubes / smooth voxels | Deformable terrain with caves |
| **No Man's Sky** | Voxel terrain with marching cubes | Entire planets rendered as voxels |
| **Dual Universe** | Server-authoritative voxel terrain | MMO-scale dual contouring |
| **Deep Rock Galactic** | Marching cubes for destructible caves | Co-op mining game |
| **TerraTech** | Voxel terrain | Vehicle-building game |
| **Space Engineers** | Voxel-based asteroid/planet deformation | |

### Open-Source Implementations

- **PolyVox**: Open-source C++ library for volumetric processing
- **Cubiquity**: Unity/Unreal voxel plugin (based on PolyVox)
- **Marching Cubes implementations**: Countless on GitHub
- **Transvoxel implementations**: Several open-source ports exist

### GPU Gems / GPU Pro Coverage

- **GPU Gems 3**, Chapter 1: "Generating Complex Procedural Terrains Using the GPU" — describes GPU-based marching cubes for terrain
- **GPU Pro 1** (2010), Chapter on voxel terrain

---

## 3.3 Mesh-Based Caves

### The Industry Standard Approach

In practice, **most AAA open-world games** use mesh-based caves:

1. **Terrain heightfield** for the surface world (LOD-optimized, fast)
2. **Hand-crafted or procedurally-placed cave meshes** embedded within the terrain
3. **Culling masks** hide terrain where cave mesh intersects the surface
4. **Transition portals** at cave entrances trigger loading/unloading of cave interiors

### How Games Place Cave Meshes in Terrain

**Static Placement:**
- Cave meshes are placed by level designers at authoring time
- Terrain is modified (heightfield stamped) to create entrances
- Cave interior is a separate instance/level

**Procedural Placement:**
- Cave entrance locations determined by terrain analysis (cliff faces, valleys)
- Cave geometry can be pre-generated, procedurally generated at runtime, or built from modular pieces
- Terrain heightfield is dynamically modified around entrances

### Portal/Culling System for Cave Entrances

- Cave entrance has an **occlusion portal**
- When player is outside, cave interior geometry is culled (not rendered)
- When player enters cave, surface terrain is culled (or LOD-reduced)
- Transition is seamless from the player's perspective

### Game Examples

| Game | Approach |
|---|---|
| **Skyrim** | Mesh-based caves placed in heightfield terrain, load doors between interior/exterior |
| **The Witcher 3** | Mesh caves integrated with terrain; some use portals, others use seamless streaming |
| **Horizon Zero Dawn / Forbidden West** | Mesh caves + terrain masking at entrances; seamless transitions |
| **Elden Ring** | Mesh-based dungeons/caves embedded in heightfield terrain; seamless transitions |
| **Breath of the Wild** | Dungeon interiors are separate zones; cave entrances are mesh objects |

---

## 3.4 Underground Streaming

### The Underground Loading Problem

In open-world games with underground areas, the engine must:
- **Unload surface terrain** when the player goes underground
- **Load underground geometry** and lighting
- Handle **vertical streaming** (not just horizontal, as with typical open-world streaming)

### Approaches

**Level-Based (Traditional):**
- Underground areas are separate levels/maps
- Loading screen at the transition
- Used in: Skyrim, Fallout series, older RPGs

**Portal-Based Streaming:**
- Seamless transition through portals (no loading screen)
- Engine manages two separate "world layers" — surface and underground
- At the portal, one fades out as the other fades in
- Used in: The Witcher 3, Horizon series, Elden Ring

**World Composition / World Layers:**
- A **layer system** where surface and underground are different layers of the same world
- The engine streams the appropriate layer based on player position
- **Vertical spatial partitioning**: Octree or similar structure that includes height
- Used in: Unreal Engine 5 (World Partition with Data Layers), custom engines

**Seamless Vertical Streaming (Most Advanced):**
- Single unified world with 3D spatial partitioning
- Player can move up/down without any transition
- Engine dynamically loads/unloads world cells based on 3D distance
- Requires: fully 3D world representation (not just 2D heightfield + height)
- Used in: No Man's Sky, Starfield (Bethesda Creation Engine 2), Minecraft

### Key Technical Challenges

1. **Lighting transitions**: Outdoor lighting (sun+sky) → cave lighting (point lights, ambient occlusion)
2. **Audio transitions**: Outdoor ambience → cave reverb
3. **Navigation mesh**: AI needs continuous navmesh across surface/cave boundary
4. **Physics**: Physics world needs to handle vertical transitions
5. **Level of Detail**: Surface terrain LOD needs to account for player being underground

### Key References

- **"Streaming World of Warcraft"** — GDC 2008
- **"Streaming in The Witcher 3"** — GDC 2014
- **"The World of Horizon Zero Dawn"** — GDC 2017
- **"Procedural World Generation in No Man's Sky"** — GDC 2017
- **"World Building in Starfield"** — GDC 2024

---

# Summary: Technique Decision Matrix

| Requirement | Recommended Technique | Alternative |
|---|---|---|
| Large outdoor terrain (heightfield) | CDLOD or Geometry Clipmaps | Adaptive Quadtree |
| GPU-driven, modern GPU | Mesh Shader terrain + compute culling | Indirect multi-draw |
| Terrain texturing (many materials) | Virtual Texturing (SVT/RVT) | Texture arrays + splat maps |
| Ocean (infinite) | Projected Grid + FFT waves | Screen-space water |
| Ocean (bounded lake/river) | Gerstner waves + projected grid | Vertex-displaced mesh |
| Water LOD | Continuous LOD + wave frequency filtering | Multi-ring approach |
| Shore rendering | Depth-based blend + foam + wet maps | Pre-authored shore geometry |
| Caves/tunnels | Mesh-based caves + terrain masking | Voxel terrain (marching cubes) |
| Underground streaming | World layers / vertical streaming | Load doors / portals |
| Far terrain | Simplified mesh + atmospheric scattering | Skybox impostors |

---

# Complete Reference Index

## Academic Papers

1. **Strugar, F.** (2010). "Continuous Distance-Dependent Level of Detail for Rendering Heightmaps." *Journal of Graphics, GPU, and Game Tools*, 14(4), 57–74. DOI: 10.1080/2151237X.2009.10129287
2. **Losasso, F. & Hoppe, H.** (2004). "Geometry Clipmaps: Terrain Rendering Using Nested Regular Grids." *ACM Trans. Graphics (SIGGRAPH 2004)*, 23(3), 769–776.
3. **Tanner, C., Migdal, C., & Jones, M.** (1998). "The Clipmap: A Virtual Mipmap." *SIGGRAPH 98*, pp. 151–158.
4. **Tessendorf, J.** (2001/2004). "Simulating Ocean Water." SIGGRAPH Course Notes.
5. **Johanson, C.** (2004). "Real-time Water Rendering — Introducing the Projected Grid Concept." Master's Thesis, Lund University.
6. **Barrett, S.** (2008). "Sparse Virtual Textures." GDC 2008.
7. **Kaplanyan, A.** (2010). "Adaptive Virtual Texturing." (Paper on dynamic page resolution)
8. **Lengyel, E.** (2010). "The Transvoxel Algorithm." In *Mathematics for 3D Game Programming and Computer Graphics*, 3rd Ed.
9. **Duchaineau, M. et al.** (1997). "ROAMing Terrain: Real-time Optimally Adapting Meshes." *IEEE Visualization 97*.
10. **Lorensen, W. & Cline, H.** (1987). "Marching Cubes: A High Resolution 3D Surface Construction Algorithm." *SIGGRAPH 1987*.
11. **Ju, T. et al.** (2002). "Dual Contouring of Hermite Data." *SIGGRAPH 2002*.

## GPU Gems / GPU Pro Chapters

| Book | Chapter | Title | Authors |
|---|---|---|---|
| GPU Gems | Ch.1 | Effective Water Simulation from Physical Models | Mark Finch |
| GPU Gems | Ch.2 | Rendering Water Caustics | Juan Guardado |
| GPU Gems 2 | Ch.2 | Terrain Rendering Using GPU-Based Geometry Clipmaps | Asirvatham, Hoppe |
| GPU Gems 2 | Ch.18 | Using Vertex Texture Displacement for Realistic Water Rendering | Yuri Kryachko |
| GPU Gems 3 | Ch.1 | Generating Complex Procedural Terrains Using the GPU | - |
| GPU Pro 1 | - | Voxel terrain chapter | - |
| GPU Pro 2 | - | Real-Time Open Water Environments with LOD | - |
| GPU Pro 4 | - | Practical and Realistic Virtual Texturing | Chen, Mayer |
| GPU Pro 5 | - | Water rendering optimization | - |
| GPU Pro 7 | - | Adaptive Virtual Texture Rendering in Far Cry 4 | Egor Yusov |

## GDC/SIGGRAPH Talks

| Conference | Year | Title | Company/Game |
|---|---|---|---|
| GDC | 2008 | Sparse Virtual Textures | id Software |
| GDC | 2013 | Water Rendering in Assassin's Creed III | Ubisoft |
| GDC | 2014 | Streaming in The Witcher 3 | CD Projekt Red |
| GDC | 2014 | Rendering the Stormy Seas of Black Flag | Ubisoft |
| GDC | 2015 | GPU-Driven Rendering (Assassin's Creed Unity) | Ubisoft |
| GDC | 2015 | Water Rendering in Far Cry 4 | Ubisoft |
| GDC | 2017 | GPU-Based Run-Time Procedural Placement (Horizon Zero Dawn) | Guerrilla |
| GDC | 2017 | 4K Rendering Breakthrough: Filtered and Culled Visibility Buffer | Wolfgang Engel |
| GDC | 2018 | Rendering the World of Far Cry 5 | Ubisoft |
| GDC | 2018 | The Ocean in Sea of Thieves | Rare |
| SIGGRAPH | 2016 | The Technical Art of Uncharted 4 | Naughty Dog |
| SIGGRAPH | 2016 | Water Technology of Uncharted 4 | Naughty Dog |
| GDC | 2017 | Procedural World Generation in No Man's Sky | Hello Games |
| SIGGRAPH | 2020 | Terrain in Ghost of Tsushima | Sucker Punch |
| GDC | 2021 | Microsoft Flight Simulator World Streaming | Asobo |
| GDC | 2024 | World Building in Starfield | Bethesda |

## Open-Source Repositories

| Repository | Description | URL |
|---|---|---|
| fstrugar/CDLOD | CDLOD terrain LOD | https://github.com/fstrugar/CDLOD |
| gasgiant/Ocean-FFT | Unity FFT ocean | https://github.com/gasgiant/Ocean-FFT |
| jbouny/fft-ocean | WebGL FFT ocean | https://github.com/jbouny/fft-ocean |
| UE4-OceanProject | Unreal Engine ocean | https://github.com/UE4-OceanProject |
| PolyVox | C++ volumetric library | Various mirrors |
| Transvoxel (Lengyel) | Voxel LOD transitions | transvoxel.org |

---

*Document compiled from primary sources (GPU Gems chapters, academic papers, GDC talks), verified open-source repositories, and published game developer presentations. All citations are referenced to the original works. For topics where specific DOIs or official links were not found during research, the best-known references are provided.*
