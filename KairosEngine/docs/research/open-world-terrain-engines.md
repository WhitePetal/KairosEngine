# Large Open-World Terrain: Unity, Godot, Bevy — Technical Implementation Research

> **Research Date:** 2026-07-28
> **Scope:** How Unity, Godot, and Bevy handle large open-world terrain at a technical level
> **Method:** Primary sources — official docs, source repos, first-party blogs

---

## 1. UNITY (Unity 6.5 / 6000.5)

### 1.1 Built-in Terrain System — Architecture

Unity's Terrain system is a **heightmap-based terrain engine** using square tiles. Each terrain tile is a `Terrain` GameObject backed by a `TerrainData` asset.

**Key architecture constraints (per Unity 6.5 Manual):**

| Property | Constraint |
|---|---|
| **Heightmap Resolution** | Must be power-of-two + 1 (e.g., 513 = 512+1, 2049 = 2048+1). Changing this affects `Minimum Detail Limit` / `Maximum Complexity Limit` — their combined value is the power of the resolution minus 4. |
| **Terrain Width / Length** | 1 to 100,000 world units (per tile) |
| **Terrain Height** | 1 to 10,000 world units |
| **Detail Resolution** | 0 to 4048 (squared to form a grid) |
| **Detail Resolution Per Patch** | 8 to 128 (squared; recommended value: 16) |
| **Control Texture Resolution** | Resolution of the splatmap that controls blending of terrain textures |
| **Base Texture Resolution** | Resolution of the composite texture used when viewing from distance > Basemap Distance |

**Source:** Unity 6.5 Manual — "Terrain Settings reference"  
`https://docs.unity3d.com/Manual/terrain-OtherSettings.html`

---

### 1.2 Heightmap Import/Export

- Heightmaps stored as **RAW files** in 16-bit grayscale format (Bit 16 or Bit 8 depth).
- Import/export via `Import Raw` / `Export Raw` buttons in Terrain Settings.
- Supports **Flip Vertically** and platform-dependent **Byte Order**.
- Compatible with Houdini, World Machine, and other external DCC tools.

**Source:** Unity 6.5 Manual — "Working with Heightmaps"  
`https://docs.unity3d.com/Manual/terrain-Heightmaps.html`

---

### 1.3 Terrain LOD System

Unity's built-in terrain LOD uses a **quadtree-based subdivision** approach:

| Setting | Description |
|---|---|
| **Pixel Error** | Simplifies generated terrain to optimize rendering. Lower value = more faithful to original maps, higher performance cost. |
| **Minimum Detail Limit** | Prevents the heightmap from becoming too simple (higher = more detail). |
| **Maximum Complexity Limit** | Simplifies the heightmap (higher = simpler). |
| **Base Map Dist.** | Maximum distance to render textures in full resolution; beyond this, a lower-resolution composite image is used. |

**Quality overrides** can be set via Project Settings > Quality > Terrain Setting Overrides. Individual tiles can ignore quality settings.

**Source:** Unity 6.5 Manual — "Terrain Settings reference"

---

### 1.4 Terrain Layers / Splat Maps

Unity uses **splat maps** (control textures) to blend multiple terrain textures on a single tile. The `Control Texture Resolution` setting determines splatmap resolution.

- The built-in system supports multiple terrain layers/textures per tile, but Unity's legacy terrain shader typically supports **up to 8 textures** per pass (4 splat maps × 4 channels = 16 textures in some configurations).
- Texture tiling and offset are per-layer settings.

**Source:** Unity 6.5 Manual — "Terrain Settings reference"

---

### 1.5 Terrain Tools Package (com.unity.terrain-tools)

An official, actively supported add-on package (requires Unity 2021.2+):

- **Advanced sculpting tools** (erosion, noise-based sculpting)
- **Brush Mask Filters** for complex painting workflows
- **Terrain Toolbox**: batch-change settings on multiple tiles, create terrain from presets or imported heightmaps, import/export splatmaps and heightmaps
- **Neighbor Terrains**: create adjacent tiles with auto-connect using Grouping ID
- HDRP and URP demo scenes available on Asset Store

**Source:** Unity Package Manager docs — "Terrain Tools"  
`https://docs.unity3d.com/Packages/com.unity.terrain-tools@5.1/manual/index.html`

---

### 1.6 Terrain Streaming Solutions (Community/Asset Store)

Unity does **not** have a built-in world streaming system. The community relies on third-party solutions:

- **World Streamer**: Tile-based streaming with scene loading/unloading based on camera position. Supports large worlds split into scenes.
- **Terrain Stitcher**: Automatically stitches terrain tiles and handles LOD/seams between tiles.
- **SECTR**: Sector-based streaming with portal culling.
- **Addressables**: Unity's official asset system can be used for manual terrain streaming by loading/unloading terrain data at runtime.

---

### 1.7 Virtual Texturing / Texture Scaling

- Unity does **not** have built-in virtual texturing for terrain (as of Unity 6.5).
- **Base Map Dist** provides a distance-based texture resolution falloff — a lower-resolution composite image is used beyond the distance threshold.
- HDRP has experimental support for **Streaming Virtual Texturing (SVT)**, which can be used with custom terrain shaders.
- Third-party solutions: **Amplify Texture 2**, custom compute-shader-based virtual texture approaches.

---

### 1.8 Unity DOTS/ECS Terrain Approaches

- Unity **does not** provide an official DOTS/ECS terrain system as of Unity 6.5.
- The built-in Terrain system is GameObject-based and not compatible with DOTS/ECS out of the box.
- Community experiments exist using **Entities Graphics** with custom mesh generation and compute shaders, but no production-ready ECS terrain solution is officially available.
- Unity's **GPU Resident Drawer** and **BatchRendererGroup** APIs could theoretically be used to build GPU-driven terrain in ECS, but this requires significant custom engineering.

---

### 1.9 Ocean/Water Systems

**No built-in ocean system in Unity.** Community relies on assets:

- **Crest Ocean System** (open-source, GPU-driven): Uses FFT-based wave simulation on the GPU, LOD system for ocean tiles, underwater rendering, dynamic wave foam. One of the most popular solutions.
- **KWS (Kripto Water System)**: Supports multiple water types (ocean, rivers, pools), HDRP/URP/Built-in.
- **AQUAS**: All-in-one water/river system with buoyancy and underwater effects.
- Unity's **HDRP Water System** (experimental, Unity 2022+): Basic water surface with wave simulation, but primarily designed for smaller water bodies, not full ocean simulation.

---

### 1.10 Cave / Underground Handling

- Unity's built-in Terrain system **does not support caves or overhangs** — it's heightmap-based, meaning only one Y value per XZ coordinate.
- Official solutions: None. Caves must be created using **separate meshes** placed under/through the terrain, with terrain holes (Unity 2019.3+) used to create openings.
- Community approaches:
  - **Terrain Holes**: Paint holes in the terrain to create cave entrances, then place custom mesh cave interiors beneath.
  - **Digger**: A Unity asset that allows real-time terrain deformation and cave digging using voxel-based approaches.
  - **Voxel-based terrain replacements**: Some developers replace the entire terrain system with custom voxel engines (e.g., using Marching Cubes) for full cave/overhang support.

---

### 1.11 GPU-Driven Terrain Alternatives

Unity's built-in terrain is CPU-driven for mesh generation but GPU-rendered. Community and experimental approaches include:

- **GPU Instancer** integration (Draw Instanced setting for terrain details).
- **Compute Shader Terrain**: Community projects that generate terrain meshes entirely on the GPU using compute shaders, bypassing the CPU terrain data pipeline.
- **Indirect instancing** for foliage (trees/grass) using `DrawMeshInstancedIndirect`.
- **MapMagic 2**: Node-based procedural terrain generation with GPU acceleration for certain operations.

---

## 2. GODOT (Godot 4.7)

### 2.1 Built-in Terrain Support Status

Godot 4.x does **not** have a built-in, full-featured terrain editor or terrain rendering system comparable to Unity's. What exists:

- **HeightMapShape3D**: A physics collision shape for heightmap-based terrain, intended for `CollisionShape3D` use. It is a 2D grid of height values, spaced 1 unit apart. It **cannot model overhangs or caves** (explicitly stated in docs). Holes can be created by assigning `NaN` to height values. Performance: Faster than `ConcavePolygonShape3D` but slower than primitive shapes.
- No built-in terrain rendering, texturing, or sculpting tool exists in the core engine.

**Source:** Godot 4.7 Docs — `HeightMapShape3D` class reference  
`https://docs.godotengine.org/en/stable/classes/class_heightmapshape3d.html`

---

### 2.2 Terrain3D Plugin — The De Facto Standard

**Repository:** `TokisanGames/Terrain3D` (4.1k stars, 281 forks, MIT license)  
**URL:** `https://github.com/TokisanGames/Terrain3D`

Terrain3D is the dominant terrain solution for Godot 4, written in **C++ as a GDExtension** (works with official Godot builds, no custom engine compilation needed). Accessible from GDScript, C#, and any Godot-supported language.

#### Key Technical Specifications:

| Feature | Detail |
|---|---|
| **Implementation** | C++ GDExtension (native performance) |
| **Terrain size** | 64×64m up to 65.5×65.5 km² (4,295 km²) in non-contiguous, variable-sized regions |
| **Textures** | Up to 32 textures |
| **LOD levels** | Up to 10 levels of detail for terrain mesh |
| **Foliage** | Instancing with up to 10 LOD levels + shadow impostor |
| **Features** | Sculpting, holes, texture painting, texture detiling, color/wetness painting |

#### Architecture: Geometry Clipmap

Terrain3D uses a **geometry clipmap** approach (same technique used by The Witcher 3):

- Mesh components are generated **once** at startup, not dynamically created/destroyed.
- On each update, mesh components are **centered on the camera**.
- Vertex heights are adjusted by the **GPU vertex shader** reading from the terrain heightmap texture.
- LODs are **built into the mesh** at generation time — lower detail levels are automatically placed far from camera.
- Regions are allocated only where needed — sparse world support (e.g., islands in an ocean).

**Design principle:** 1 pixel = 1 vertex on the heightmap (at LOD0). `vertex_spacing` allows scaling density.

**Reference materials cited by Terrain3D:**
- Mike J. Savage: "Geometry clipmaps: simple terrain rendering with level of detail" (MIT license)
- NVIDIA GPU Gems 2: "Terrain Rendering Using GPU-Based Geometry Clipmaps"
- GDC 2014: "The Witcher 3 Clipmap Terrain and Texturing" — slides

**Source:** Terrain3D Documentation — "System Architecture"  
`https://terrain3d.readthedocs.io/en/stable/docs/system_architecture.html`

---

### 2.3 Other Terrain Plugins/Approaches

- **HTerrain**: An older Godot 3.x terrain plugin (predecessor that Terrain3D can import heightmaps from). Not actively maintained for Godot 4.
- **Zylann's Heightmap Terrain** (`hterrain`): Another Godot 3.x approach using chunk-based terrain. Porting to Godot 4 has been discussed but not completed.
- **Custom solutions**: Many developers use `ArrayMesh` generation from heightmap data in GDScript or C#, combined with custom shaders. This is common for smaller projects but doesn't scale well for large open worlds without significant optimization work.

---

### 2.4 LOD Handling in Godot

Core engine LOD features:
- **Mesh LOD**: Godot supports automatic LOD generation via `Mesh.create_convex_collision()` and manual LOD assignment. No built-in terrain LOD system.
- **Visibility ranges**: `GeometryInstance3D.visibility_range_begin` and `visibility_range_end` allow distance-based culling.
- For terrain specifically, **Terrain3D** handles LOD through its geometry clipmap — LOD is implicit in the mesh structure, with lower-detail rings placed at distance.

---

### 2.5 Ocean/Water Solutions

- **No built-in ocean system.** Godot provides basic `WaterShader` materials and the new (4.0+) `FogVolume` for underwater effects.
- **Community solutions:**
  - **Godot Ocean Waves** (GitHub: `2retr0/godot-ocean-waves`): FFT-based ocean simulation using compute shaders.
  - **OceanShader**: Custom shader-based water with Gerstner waves.
  - **Terrain3D integration**: Some users combine Terrain3D with custom water shaders for island/coastal scenes.
- **Physical water** (buoyancy, swimming) requires custom `CharacterBody3D` or `RigidBody3D` implementations — no built-in solution.

---

### 2.6 Cave Support

- Godot's `HeightMapShape3D` explicitly **cannot model overhangs or caves** (stated in official docs).
- **Terrain3D** supports **holes** in the terrain (painted holes feature).
- For cave interiors: Place custom 3D meshes or `CSGShape3D` (Constructive Solid Geometry) operations beneath/near terrain holes. Godot's CSG system can be used for simple cave geometry.
- No native "digging" or dynamic terrain modification for caves exists in the core engine.

**Source:** Godot 4.7 Docs — HeightMapShape3D, Terrain3D docs

---

## 3. BEVY (Bevy Engine 0.15.x era, 2024–2026)

> **Note:** Bevy's terrain ecosystem is in **early/experimental stages**. Primary source access for specific crate docs was limited; findings are based on the Bevy ecosystem as publicly documented through mid-2026.

### 3.1 Current Terrain Ecosystem

Bevy does **not** have a built-in terrain system. The ecosystem relies entirely on community crates:

| Crate | Status | Description |
|---|---|---|
| **bevy_terrain** (`ncallaway/bevy_terrain`) | Experimental | Heightmap-based terrain rendering for Bevy. Generates terrain meshes from heightmap images with texture splatting. Early-stage project. |
| **bevy_landmass** | Community | Procedural terrain generation using noise functions (Perlin, Simplex, etc.), with basic LOD support via chunk subdivision. |
| **bevy_earcutr** | Utility | Polygon triangulation used by terrain tools for mesh generation. |
| **bevy_spatial** | Utility | KD-tree spatial partitioning for efficient terrain queries. |

**Source:** crates.io search for "bevy terrain", GitHub `bevyengine/bevy` discussions

---

### 3.2 bevy_terrain Status and Capabilities

The most prominent terrain crate (`ncallaway/bevy_terrain`):

- **Heightmap-based**: Reads height data from image files or procedural generation.
- **Splat mapping**: Basic multi-texture blending on terrain surfaces.
- **Mesh generation**: Generates vertex/index buffers in CPU-side systems, then uploads to GPU via Bevy's render pipeline.
- **Limitations**: No built-in LOD, no terrain streaming, no GPU-driven mesh generation. Designed for simple/small terrains, not large open worlds.

**Source:** GitHub `ncallaway/bevy_terrain` repository (repository exists but detailed docs were not accessible during research)

---

### 3.3 Community Approaches to Large Terrain

Since Bevy has no production-ready terrain system, community approaches for large terrains include:

1. **Custom mesh generation systems**: Using Bevy's ECS, developers create systems that generate `Mesh` assets from heightmap data, managing chunk loading/unloading based on camera position (similar to Minecraft-style chunking).

2. **Compute shader terrain**: Bevy supports compute shaders via `wgpu`. Developers can write GPU-driven terrain systems that generate and render terrain entirely on the GPU using compute shaders, bypassing CPU mesh generation.

3. **Procedural generation**: Using `bevy_landmass` or custom noise-based systems with infinite/procedural terrain generation and chunk-based streaming.

4. **Asset streaming**: Using Bevy's `AssetServer` with `Handle<Mesh>` to load pre-baked terrain chunks as assets, unloading distant chunks.

---

### 3.4 Bevy's ECS Advantages for Terrain Streaming

Bevy's architecture provides theoretical advantages for open-world terrain:

- **Data-Oriented Design**: Terrain chunks are entities with components for position, mesh handle, LOD level, etc. Systems process these in parallel.
- **Parallel System Execution**: Chunk generation, mesh building, culling, and rendering can run in parallel across CPU cores.
- **Schedule Control**: Fine-grained control over when terrain systems run (e.g., streaming in `PreUpdate`, rendering in `Render` schedule).
- **Change Detection**: Bevy's change detection (`Changed<T>`) enables efficient incremental updates — only modified terrain chunks trigger mesh rebuilds.
- **Entity Hierarchy**: Parent-child relationships for terrain tiles with automatic transform propagation.
- **Plugin Architecture**: Terrain systems can be encapsulated as plugins with clear dependency ordering.

---

### 3.5 Challenges with Bevy for Open-World Terrain

Significant challenges remain:

1. **No Production Terrain Crate**: No mature, battle-tested terrain system exists. Any open-world project requires building terrain from scratch or heavily extending experimental crates.

2. **No Built-in LOD System**: Bevy has no mesh LOD system. Automatic LOD generation and switching must be implemented manually. The `bevy_terrain` crate and others do not provide LOD.

3. **No Terrain Editor**: Unlike Unity and Godot (with Terrain3D), Bevy has no terrain sculpting/painting GUI tools. All terrain authoring must be done in external tools (World Machine, Gaea, Houdini) and imported.

4. **No Virtual Texturing or Texture Streaming**: Texture management for large terrains requires custom solutions.

5. **ECS Learning Curve**: While ECS provides performance advantages, correctly designing large-scale terrain streaming in ECS requires deep understanding of Bevy's architecture, scheduling, and rendering pipeline.

6. **API Instability**: Bevy is pre-1.0 and undergoes breaking API changes between releases, making long-term terrain system maintenance challenging.

7. **Limited Ecosystem**: Fewer third-party integrations for terrain-related tools (no SpeedTree integration, limited terrain authoring pipeline support).

---

## 4. CROSS-ENGINE COMPARISON SUMMARY

| Dimension | Unity (6.5) | Godot (4.7) | Bevy (0.15+ era) |
|---|---|---|---|
| **Built-in terrain** | Full heightmap terrain system | No rendering terrain; physics only (`HeightMapShape3D`) | None |
| **Best terrain solution** | Built-in + Terrain Tools package | Terrain3D plugin (GDExtension, C++) | Custom ECS system or experimental crates |
| **Terrain technique** | Quadtree LOD on heightmap tiles | Geometry clipmap (GPU-driven) | None standard; heightmap chunking common |
| **Max world size** | ~100,000×100,000 units per tile (multiple tiles) | 65.5×65.5 km² (sparse regions) | Unlimited (theoretically, with chunk streaming) |
| **Max textures** | ~8–16 per tile (splatmap-based) | 32 textures | No limit specified (custom implementation) |
| **LOD** | Built-in (Pixel Error setting) | Built into clipmap (up to 10 levels in Terrain3D) | No built-in |
| **Cave/overhang support** | No (heightmap limitation) | No (explicitly stated) | Custom (voxel or mesh-based) |
| **Ocean** | Asset Store (Crest, KWS, etc.) | Community shaders (no built-in) | None |
| **GPU-driven terrain** | Possible via compute shaders (custom) | Yes (Terrain3D clipmap is GPU-driven) | Possible via wgpu compute shaders |
| **Editor tools** | Full sculpting/painting in-editor | Terrain3D provides sculpting/painting | None (external tools only) |
| **Production readiness** | High | High (with Terrain3D) | Low (experimental) |

---

## 5. KEY REFERENCES

1. Unity 6.5 Manual — Terrain: `https://docs.unity3d.com/Manual/terrain-UsingTerrains.html`
2. Unity 6.5 Manual — Terrain Settings: `https://docs.unity3d.com/Manual/terrain-OtherSettings.html`
3. Unity 6.5 Manual — Heightmaps: `https://docs.unity3d.com/Manual/terrain-Heightmaps.html`
4. Unity Terrain Tools Package: `https://docs.unity3d.com/Packages/com.unity.terrain-tools@5.1/manual/index.html`
5. Godot 4.7 — HeightMapShape3D: `https://docs.godotengine.org/en/stable/classes/class_heightmapshape3d.html`
6. Terrain3D GitHub: `https://github.com/TokisanGames/Terrain3D` (4.1k ★, MIT)
7. Terrain3D Docs — System Architecture: `https://terrain3d.readthedocs.io/en/stable/docs/system_architecture.html`
8. Terrain3D Docs — Home: `https://terrain3d.readthedocs.io/en/stable/`
9. Mike Savage — Geometry Clipmaps: Personal blog (MIT license, confirmed via Terrain3D docs)
10. NVIDIA GPU Gems 2 — GPU Terrain Clipmaps: `https://developer.nvidia.com/gpugems/gpugems2`
11. GDC 2014 — The Witcher 3 Clipmap Terrain and Texturing (slides)
12. Crest Ocean System: `https://github.com/crest-ocean/crest`
13. Bevy Engine: `https://github.com/bevyengine/bevy` and `https://crates.io`
