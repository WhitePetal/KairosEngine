# Material Dynamic Properties — Competitor Analysis

> **Date:** 2026-07-27
> **Status:** Complete
> **Sources:** Unity 6.5 docs ([docs.unity3d.com](https://docs.unity3d.com)), Unreal Engine 5 docs ([dev.epicgames.com](https://dev.epicgames.com)), Godot 4.x docs ([docs.godotengine.org](https://docs.godotengine.org)), Bevy source ([github.com/bevyengine/bevy](https://github.com/bevyengine/bevy))

---

## Table of Contents

1. [Unity — Material / ShaderLab System](#1-unity--material--shaderlab-system)
2. [Unreal Engine — Material Expressions & Material Instances](#2-unreal-engine--material-expressions--material-instances)
3. [Godot — ShaderMaterial & Uniform Reflection](#3-godot--shadermaterial--uniform-reflection)
4. [Bevy — Material Trait & AsBindGroup](#4-bevy--material-trait--asbindgroup)
5. [Cross-Engine Comparison Matrix](#5-cross-engine-comparison-matrix)
6. [Design Implications for KairosEngine](#6-design-implications-for-kairosengine)

---

## 1. Unity — Material / ShaderLab System

### 1.1 Architecture Overview

Unity separates the **shader definition** (ShaderLab/HLSL) from the **material instance** (asset file). A shader declares *properties* in a `Properties` block; a material stores *values* for those properties.

```
Shader (ShaderLab + HLSL)
  └── Properties block ── declares: name, display-name, type, default, [attributes]
Material (.mat asset)
  └── Stores values keyed by property name
```

**Source:** [Unity Manual — Properties block reference in ShaderLab](https://docs.unity3d.com/Manual/SL-Properties.html)

### 1.2 Shader Reflection — The `Properties` Block

Every `Shader` object contains a `Properties` block written in ShaderLab syntax. This is the **single source of truth** for what parameters a material can expose:

```hlsl
Shader "Example/MyShader" {
    Properties {
        [MainColor] _Color ("Main Color", Color) = (1,1,1,1)
        [Normal] _NormalMap ("Normal Map", 2D) = "bump" {}
        _Roughness ("Roughness", Range(0,1)) = 0.5
        [Toggle] _EnableFog ("Enable Fog", Float) = 0
        [HDR] _Emission ("Emission Color", Color) = (0,0,0,1)
    }
    SubShader { /* HLSL code */ }
}
```

**Declaration format:** `[optional: attribute] name("display text", type name) = default value`

**Supported types:** `Integer`, `Int` (legacy/float-backed), `Float`, `Range(min,max)`, `Color`, `Vector`, `2D`, `2DArray`, `3D`, `Cube`, `CubeArray`

**Source:** [Unity Manual — Material property declaration syntax by type](https://docs.unity3d.com/Manual/SL-Properties.html#material-property-declaration-syntax-by-type)

### 1.3 Property Metadata & Attributes

Unity uses **C#-like attributes** in front of property declarations. These serve dual purpose: GPU behavior hints *and* Inspector UI instructions.

| Attribute | Function |
|---|---|
| `[Gamma]` | sRGB-space property (color-space conversion) |
| `[HDR]` | High Dynamic Range color picker |
| `[HideInInspector]` | Hidden from UI |
| `[MainTexture]` | Designates the "main" texture |
| `[MainColor]` | Designates the "main" color |
| `[NoScaleOffset]` | Hides tiling/offset UI for textures |
| `[Normal]` | Validates normal-map assignment |
| `[PerRendererData]` | Property comes from MaterialPropertyBlock |

Additionally, **MaterialPropertyDrawers** extend the UI:
- `[Toggle]` / `[ToggleOff]` — bool as float + shader keyword
- `[KeywordEnum(None, Add, Multiply)]` — enum dropdown + keyword
- `[Enum(UnityEngine.Rendering.BlendMode)]` — C# enum dropdown
- `[PowerSlider(3.0)]` — non-linear slider response
- `[IntRange]` — integer slider
- `[Space]` / `[Header("text")]` — layout decorators
- Custom drawers via `MaterialPropertyDrawer` subclass

**Source:** [Unity Manual — Material property attributes](https://docs.unity3d.com/Manual/SL-Properties.html#material-property-attributes), [MaterialPropertyDrawer API](https://docs.unity3d.com/ScriptReference/MaterialPropertyDrawer.html)

### 1.4 Material Data Model

**`Material`** (runtime class): A C# object that holds a dictionary of property values keyed by name (or integer `Shader.PropertyToID`). The material is backed by a `.mat` asset file (YAML-like serialization).

Key API methods:
- `Material.SetFloat(name, value)` / `.SetColor()` / `.SetTexture()` / `.SetVector()` / `.SetMatrix()`
- `Material.GetFloat(name)` / etc.
- `Material.HasProperty(name)` — checks if shader declares this property
- `Material.shader` — the shader asset reference
- `Material.mainTexture` / `Material.color` — convenience accessors for `[MainTexture]`/`[MainColor]`-tagged properties

### 1.5 MaterialPropertyBlock — Per-Renderer Override

`MaterialPropertyBlock` allows **per-renderer property overrides without creating new material instances** — critical for draw-call batching:

```csharp
MaterialPropertyBlock block = new MaterialPropertyBlock();
block.SetColor("_Color", Color.red);
renderer.SetPropertyBlock(block);
```

- Values are stored in `ConstantBuffer` / structured buffer
- **Not compatible with SRP Batcher** (performance tradeoff)
- Properties declared `[PerRendererData]` in shader show as read-only in inspector

**Source:** [MaterialPropertyBlock API](https://docs.unity3d.com/ScriptReference/MaterialPropertyBlock.html)

### 1.6 GPU Data Transfer

- **Built-in RP:** Properties are set via `SetPass` calls — individual `SetFloat`/`SetVector` etc. per draw call.
- **SRP Batcher (URP/HDRP):** Per-material properties are packed into a **GPU constant buffer** (`CBUFFER`). All per-material variables must be in the *same* `CBUFFER` block. This enables persistent GPU data across draws, reducing CPU overhead.
- Per-object data (transform etc.) is in a separate `UnityPerDraw` CBUFFER.

**Source:** [Unity Manual — Properties in Shader Programs](https://docs.unity3d.com/Manual/SL-Properties.html): "In your HLSL code, you must put per-material variables in the same `CBUFFER` for SRP Batcher compatibility."

### 1.7 Editor/Inspector UI Generation

The Unity Editor automatically generates the Material Inspector from the shader's `Properties` block:
1. Parse ShaderLab → extract property declarations
2. For each property, instantiate the appropriate editor widget:
   - `Float` → numeric field
   - `Range(min,max)` → slider
   - `Color` → color picker
   - `2D` → texture slot with preview
   - `[Toggle]` → checkbox
3. Apply attributes/decorators (`[Header]`, `[Space]`, `PowerSlider`, etc.)
4. Apply custom `MaterialPropertyDrawer` delegates

**Customization path:** `MaterialPropertyDrawer` base class with `OnGUI(Rect, MaterialProperty, string, MaterialEditor)` override.

### 1.8 Serialization Format

`.mat` files are Unity's internal YAML-based format. Serialized fields map directly to shader property names. Example (simplified):

```yaml
Material:
  m_Shader: {fileID: 46, guid: abc123...}
  m_SavedProperties:
    m_TexEnvs:
    - _MainTex: {m_Texture: {fileID: 2800000, guid: def456...}}
    m_Floats:
    - _Roughness: 0.5
    m_Colors:
    - _Color: {r: 1, g: 1, b: 1, a: 1}
```

---

## 2. Unreal Engine — Material Expressions & Material Instances

### 2.1 Architecture Overview

Unreal Engine uses a **node-based Material Editor** to create material graphs. Materials are compiled into HLSL shaders, with exposed parameters becoming **Material Instances** that can override values without recompilation.

```
Material (graph of MaterialExpression nodes)
  └── Exposed parameters → ScalarParameter, VectorParameter, TextureParameter
Material Instance (constant or dynamic)
  └── Overrides parent Material parameters
```

**Source:** [Unreal Engine — Materials documentation](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-materials)

### 2.2 Shader Reflection — Material Parameters

UE's approach is fundamentally different from Unity: parameters are **not declared in the shader source**. Instead, they are **MaterialExpression nodes** in the node graph:

| Parameter Node | Type | Usage |
|---|---|---|
| `ScalarParameter` | `float` | Named scalar value |
| `VectorParameter` | `float4` | Named RGBA vector |
| `TextureSampleParameter2D` | `Texture2D` | Named texture |
| `StaticBoolParameter` | `bool` | Static switch (compile-time branch) |
| `TextureSampleParameterCube` | `Cubemap` | Named cubemap |
| `MaterialAttributes` | struct | Bundle of material outputs |

Each parameter node has a **`ParameterName`** (FName). When the material is compiled, these become the "visible" parameters.

**Source:** [Unreal Engine — Material Expressions Reference](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-material-expressions)

### 2.3 Material Data Model

- **`UMaterial`** — the parent material (compiled shader). Contains the expression graph.
- **`UMaterialInstance`** — child that overrides specific parameters. **No shader recompilation needed.**
- **`UMaterialInstanceDynamic`** (MID) — runtime-created instance for programmatic changes.
- **`UMaterialInstanceConstant`** — asset-based instance for artist-authored variations.

The parameter override model:
```
UMaterial (parent)
 ├─ _BaseColor: (1,0,0,1)   [default]
 ├─ _Roughness: 0.5          [default]
 └─ _NormalTex: T_DefaultN   [default]

UMaterialInstance (child)
 ├─ _BaseColor: (0,0,1,1)   [OVERRIDE]
 └─ _Roughness: 0.5          [inherits default]
```

### 2.4 Editor UI — Material Instance Editor

The Material Instance Editor reads the parent material's exposed parameters and generates a property sheet:
- `ScalarParameter` → numeric slider
- `VectorParameter` → color picker or 4-component float edit
- `TextureParameter` → texture thumbnail slot
- `StaticSwitchParameter` → checkbox

Each parameter row shows:
- **Override checkbox** — whether the instance overrides the parent value
- **Parameter name** (from the expression node's `ParameterName`)
- **Group/SortPriority** — organizational metadata from the parameter node

**Source:** [Unreal Engine — Material Instance Editor UI](https://dev.epicgames.com/documentation/en-us/unreal-engine/material-instance-editor-ui)

### 2.5 GPU Data Transfer

- UE bundles material parameters into **constant buffers** per material.
- The `FMaterialShaderMap` compiles shader permutations based on static switches and quality levels.
- Material Instances update only the **UniformBuffer** (constant buffer) for their overrides — no shader variant needed.
- `UMaterialInstanceDynamic::SetScalarParameterValue()` / `SetVectorParameterValue()` / `SetTextureParameterValue()` write directly to the parameter collection.

### 2.6 Serialization Format

UE materials are serialized as `.uasset` files (proprietary binary format). The material graph is stored in `UMaterial`'s `Expressions` array. Material Instances store a `Parent` reference plus an override map.

---

## 3. Godot — ShaderMaterial & Uniform Reflection

### 3.1 Architecture Overview

Godot uses **text-based shaders** (GLSL-like shading language) with `uniform` declarations. A `ShaderMaterial` resource pairs a `Shader` with uniform values.

```
Shader (.gdshader text resource)
  └── uniform declarations
ShaderMaterial (resource)
  └── Shader reference + uniform values (Dictionary)
```

**Source:** [Godot Docs — Shaders](https://docs.godotengine.org/en/stable/tutorials/shaders/index.html)

### 3.2 Shader Reflection — `uniform` Declarations

Godot parses the shader text at load time and extracts all `uniform` declarations. The engine uses **naga** (or its own parser) for reflection.

```glsl
shader_type spatial;

uniform vec4 albedo : source_color = vec4(1.0);
uniform float roughness : hint_range(0.0, 1.0) = 0.5;
uniform sampler2D normal_map : hint_normal;
uniform bool use_emission = false;
```

**Uniform hints** control how the Inspector renders the property:
- `hint_range(min, max[, step])` → slider
- `hint_range(min, max, "or_greater")` / `"or_less"` → slider with extended range
- `hint_color` / `source_color` → color picker
- `hint_normal` → normal-map aware texture slot
- `hint_albedo` / `hint_black_albedo` / `hint_white` → texture with sRGB hint
- `hint_default_black` / `hint_default_white` / `hint_default_transparent` → default texture
- `hint_anisotropic` → anisotropic filtering texture
- `hint_roughness_[normal|gray|R|G|B|A]` → roughness map import hint
- `hint_filter_[nearest|linear|nearest_mipmap|linear_mipmap|nearest_mipmap_anisotropic|linear_mipmap_anisotropic]` → sampler filtering
- `hint_screen_texture` → screen-space texture
- `hint_depth_texture` → depth texture
- `hint_multiline` → multi-line string editor

**Source:** [Godot Docs — Shading language: Uniforms](https://docs.godotengine.org/en/stable/tutorials/shaders/shader_reference/shading_language.html#uniforms)

### 3.3 Material Data Model

**`ShaderMaterial`** class:
- Inherits from `Material` (base class)
- `shader` property: reference to a `Shader` resource
- `set_shader_parameter(name, value)` / `get_shader_parameter(name)` — runtime uniform manipulation
- Internally stores a `Dictionary<Variant>` of parameter values

**Source:** [Godot Class Reference — ShaderMaterial](https://docs.godotengine.org/en/stable/classes/class_shadermaterial.html)

Godot also provides a **`StandardMaterial3D`** (PBR, fixed-function) as the non-shader path. This uses pre-defined properties (albedo, metallic, roughness, etc.) with no shader code.

### 3.4 GPU Data Transfer

- Godot's `RenderingServer` allocates a **uniform set** per material.
- `ShaderMaterial` → `MaterialStorage` → uniform set cache (`UniformSetCacheRD`).
- On parameter change, the uniform buffer data is updated and the descriptor set is re-bound.
- The RenderingDevice abstraction (Vulkan/D3D12/Metal) handles the actual buffer allocation and binding.

### 3.5 Inspector UI Generation

The Godot editor generates the Inspector UI from shader uniforms:
1. Parse shader source → extract `uniform` declarations with hints
2. For each uniform:
   - `float` → `EditorSpinSlider`
   - `float` with `hint_range` → slider with spinbox
   - `vec4` with `source_color` → color picker
   - `sampler2D` → texture slot (`EditorResourcePicker`)
   - `bool` → checkbox
3. Group uniforms by their declaration order (no `[Header]` equivalent; use **`group_uniforms`** subgroup syntax)
4. **Instance uniforms** (with `instance uniform` keyword) are grouped separately for per-instance data

### 3.6 Serialization Format

Godot resources use a custom text format (`.tres` / `.tscn`) or binary (`.res`):

```gdresource
[gd_resource type="ShaderMaterial" load_steps=2 format=3]

[ext_resource type="Shader" path="res://my_shader.gdshader" id="1"]

[resource]
shader = ExtResource("1")
shader_parameter/albedo = Color(1, 0, 0, 1)
shader_parameter/roughness = 0.5
shader_parameter/normal_map = ExtResource("2")
```

---

## 4. Bevy — Material Trait & AsBindGroup

### 4.1 Architecture Overview

Bevy (Rust ECS) takes a fundamentally different approach: materials are **Rust types** that implement the `Material` trait. The shader is a separate WGSL (or GLSL) asset. The `AsBindGroup` derive macro bridges the Rust struct ↔ WGSL bind group.

```
#[derive(AsBindGroup, TypePath, Asset, Clone)]
struct MyMaterial { ... }    ← Rust struct defines properties

impl Material for MyMaterial { ... }  ← Binds to a shader

Shader (.wgsl asset)
  └── @group(1) @binding(0) var<uniform> material: MyMaterialUniform;
  └── @group(1) @binding(1) var base_color_texture: texture_2d<f32>;
```

**Source:** [Bevy source — `bevy_pbr/src/material.rs`](https://github.com/bevyengine/bevy/blob/main/crates/bevy_pbr/src/material.rs)

### 4.2 Shader Reflection — `AsBindGroup` Derive Macro

This is the **key innovation**: the `AsBindGroup` proc-macro generates the GPU bind group layout **from the Rust struct definition** at compile time:

```rust
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub struct MyMaterial {
    #[uniform(0)]
    pub base_color: LinearRgba,
    #[uniform(0)]
    pub roughness: f32,
    #[texture(1)]
    #[sampler(2)]
    pub base_color_texture: Option<Handle<Image>>,
}
```

The `#[uniform(N)]` / `#[texture(N)]` / `#[sampler(N)]` attributes specify bind group binding indices. The macro generates:
- A WGSL-compatible uniform struct (field ordering, alignment)
- `BindGroupLayout` creation code
- `BindGroup` preparation code (extract from material + asset server)
- `AsBindGroupShaderType` trait impl for the uniform type

**Source:** [Bevy source — `bevy_render/src/render_resource/`](https://github.com/bevyengine/bevy/tree/main/crates/bevy_render/src/render_resource)

### 4.3 Material Data Model

```rust
pub trait Material: AsBindGroup + Asset + Clone + Sized {
    fn fragment_shader() -> ShaderRef { ... }
    fn vertex_shader() -> ShaderRef { ... }
    fn prepass_fragment_shader() -> ShaderRef { ... }
    fn alpha_mode(&self) -> AlphaMode { AlphaMode::Opaque }
    fn specialize(descriptor, key) -> RenderPipelineDescriptor { ... }
}
```

- **`Material` trait** — defines which shader to use, and optionally customizes the render pipeline.
- **`MaterialPlugin<M>`** — ECS plugin that wires up the rendering pipeline for a material type.
- **`MaterialMesh2dBundle<M>`** / **`MaterialMeshBundle<M>`** — spawnable bundles.
- Material data is stored in the ECS as `Assets<M>` (handle-based, like Bevy's other assets).

### 4.4 GPU Data Transfer — Bind Groups & Uniform Buffers

Bevy's material pipeline:
1. `ExtractMaterialsPlugin` extracts material handles into the render world.
2. `PrepareMaterials` system: for each material, call `AsBindGroup::as_bind_group()` which writes the uniform data into an `encase`-allocated buffer and creates a `BindGroup`.
3. The extracted material data is stored in **`PreparedMaterial`** resources in the render world.
4. During draw, `SetMaterialBindGroup` sets the bind group for the current material.

Each material maintains **its own uniform buffer and bind group**. There is no shared material constant buffer like Unity's SRP Batcher.

**Key constraint:** `#[uniform(N)]` fields must go into a single uniform buffer at `@binding(N)`. The `encase` crate handles WGSL std140 layout.

### 4.5 Editor/Inspector UI — `bevy_inspector_egui`

Bevy has no built-in material editor. However, `bevy_inspector_egui` provides generic reflection-based UI:

- Uses Bevy's `Reflect` trait to introspect struct fields
- For `Handle<Image>`, shows a texture preview thumbnail
- For `Color`/`LinearRgba`, shows a color picker
- For `f32`, shows a numeric field
- No shader-driven property reflection — the Rust struct is the source of truth

The UI is **struct-driven** rather than **shader-driven**. If a material adds a new shader uniform, both the Rust struct *and* the WGSL shader must be updated manually.

### 4.6 Serialization Format

Bevy uses its own scene format (RON or custom binary). Materials are `Asset` types and can be serialized:

```rust
// Scene format (RON-like)
(
    materials: {
        "my_material": MyMaterial(
            base_color: Rgba(1.0, 0.0, 0.0, 1.0),
            roughness: 0.5,
            base_color_texture: "textures/checkerboard.png",
        ),
    },
)
```

---

## 5. Cross-Engine Comparison Matrix

| Dimension | Unity | Unreal | Godot | Bevy |
|---|---|---|---|---|
| **Shader source language** | ShaderLab + HLSL | HLSL (via node graph) | Godot Shading Language (GLSL-like) | WGSL (or GLSL via `naga`) |
| **Parameter declaration** | `Properties` block in shader | `MaterialExpression` nodes in graph | `uniform` keyword in shader text | `#[uniform(N)]` on Rust struct fields |
| **Reflection mechanism** | Parse ShaderLab at import | Walk expression graph at compile | Parse shader text at load (naga) | Proc-macro at compile time (`AsBindGroup`) |
| **Source of truth** | Shader file | Node graph | Shader file | Rust struct |
| **Material model** | `Material` class + `.mat` assets | `UMaterial` → `UMaterialInstance` → `UMaterialInstanceDynamic` | `ShaderMaterial` resource | `Material` trait impl + `Asset` |
| **Per-renderer override** | `MaterialPropertyBlock` | Custom Primitive Data | `GeometryInstance3D.material_override` | No direct equivalent (use separate material) |
| **GPU transfer** | `CBUFFER` (SRP Batcher) or per-draw `SetPass` | UniformBuffer per material instance | Uniform set cache (descriptor sets) | Individual bind groups per material |
| **Editor UI generation** | Auto from `Properties` + `MaterialPropertyDrawer` | Material Instance Editor (auto from params) | Auto from `uniform` hints | `bevy_inspector_egui` (struct reflection) |
| **Property metadata** | C# attributes (`[Range]`, `[HDR]`, `[Header]`) | Parameter metadata (Group, SortPriority) | Hint system (`hint_range`, `hint_color`) | None built-in (type-driven: `f32`→slider) |
| **Serialization** | YAML `.mat` | Binary `.uasset` | Text `.tres` or binary `.res` | Scene RON or custom |
| **Shader → property coupling** | Tight (shader declares properties) | Medium (graph declares params) | Tight (shader declares uniforms) | Loose (Rust struct and WGSL must match manually) |

---

## 6. Design Implications for KairosEngine

### 6.1 Current State

KairosEngine's material system (`kairos_engine/src/graphics/material.rs`) is currently minimal:

```rust
pub struct SerializedMaterial {
    pub source_path: PathBuf,
    pub shader_path: PathBuf,
    pub render_state: RenderState,
    pub texture_path: Option<PathBuf>,
}

pub struct Material {
    pub shader: Option<Arc<AssetHandle<ShaderAssetsSystem>>>,
    pub render_state: RenderState,
    pub texture: Option<Arc<AssetHandle<TextureAssetsSystem>>>,
}
```

There is **no dynamic property system** — materials reference a shader and a texture through TOML-based serialization. Properties are not reflected from shaders.

### 6.2 Key Design Decisions from Competitor Analysis

#### Approach to Reflection: Shader-Driven vs. Type-Driven

| Approach | Engines | Pros | Cons |
|---|---|---|---|
| **Shader-driven** (properties declared in shader) | Unity, Godot | Single source of truth; non-programmer friendly; editor auto-generates UI | Requires shader parsing infrastructure; harder to validate at compile time |
| **Graph-driven** (properties declared in node graph) | Unreal | Visual authoring; rich metadata per parameter | Requires a node graph editor |
| **Type-driven** (properties declared in host code) | Bevy | Compile-time type safety; no shader parsing needed | Shader and code must stay in sync manually; no editor auto-generation |

**Recommendation for KairosEngine:** Adopt a **hybrid approach** aligned with Bevy's `AsBindGroup` pattern (type-driven) but add shader-driven *validation*. Since KairosEngine already uses Rust types for materials and WGSL shaders, the `AsBindGroup` derive pattern is the natural fit. However, we should add a **build-time validation step** that verifies the WGSL `@group`/`@binding` declarations match the Rust `#[uniform(N)]` / `#[texture(N)]` attributes.

#### Property Metadata System

The most valuable pattern from Unity and Godot is the **hint/attribute system** for property metadata:

```rust
// Proposed KairosEngine approach
#[derive(AsBindGroup, Material)]
pub struct PbrMaterial {
    #[uniform(0)]
    #[range(0.0, 1.0)]
    #[display_name("Roughness")]
    pub roughness: f32,

    #[uniform(0)]
    #[color_picker]
    pub albedo: LinearRgba,

    #[texture(1)]
    #[sampler(2)]
    #[normal_map]
    pub normal_texture: Option<Handle<Image>>,
}
```

These attributes serve dual purpose:
1. **Editor UI generation** — auto-create sliders, color pickers, texture slots
2. **Validation** — e.g., `#[normal_map]` can warn on non-normal texture assignment

#### Per-Renderer Data Override

Unity's `MaterialPropertyBlock` and UE's Custom Primitive Data are essential for GPU instancing. KairosEngine should support:
- A **`MaterialPropertyBlock`** equivalent for per-entity overrides
- Properties declared with `#[per_instance]` attribute are eligible
- GPU-side: stored in instance-rate vertex buffer or storage buffer

#### Serialization Format

Godot's `.tres` resource format is a good model: human-readable, property-keyed, references external resources by path/GUID. KairosEngine's existing TOML-based serialization can be extended with a `[material.properties]` section:

```toml
[material]
shader = "shaders/pbr.wgsl"

[material.properties]
roughness = 0.5
albedo = [1.0, 0.0, 0.0, 1.0]
normal_texture = "textures/default_normal.ktx2"
```

### 6.3 Summary of Recommendations

1. **Implement `AsBindGroup`-style derive macro** for bridging Rust material structs ↔ WGSL bind groups
2. **Add property metadata attributes** (`#[range]`, `#[color_picker]`, `#[texture_hint]`, etc.) for editor UI generation
3. **Build-time shader-material validation** to ensure WGSL bindings match Rust struct layout
4. **Per-entity overrides** via `MaterialPropertyBlock` equivalent
5. **Extend TOML serialization** with a `[material.properties]` section for dynamic properties
6. **Editor inspector auto-generation** using the `#[reflect]` + material-specific attribute system
