# <img src="KairosEngine/Preferences/Textures/engine_icon.png" width = "24" height = "24" > KairosEngine

<div align="center">

**A pure Rust game engine built from first principles — Data‑Oriented Design, ECS architecture, high performance, high flexibility, and high extensibility.**

<br>

[![Rust](https://img.shields.io/badge/lang-Rust_2024_edition-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-blue?style=flat-square)](https://github.com/WhitePetal/KairosEngine)
[![wgpu](https://img.shields.io/badge/gpu-wgpu_29-green?style=flat-square&logo=webgpu)](https://wgpu.rs)
[![Status](https://img.shields.io/badge/status-active_development-2ea44f?style=flat-square)](https://github.com/WhitePetal/KairosEngine)
[![Discussions](https://img.shields.io/badge/chat-Discussions-5865F2?style=flat-square&logo=github)](https://github.com/WhitePetal/KairosEngine/discussions)
[![Bilibili](https://img.shields.io/badge/B站-私信-00A1D6?style=flat-square&logo=bilibili)](https://space.bilibili.com/232017781)

[🌐 中文文档](README-zh.md)

</div>

---

## 📋 Overview

Re-examining the game industry from a fresh perspective — **KairosEngine** is a pure Rust game engine built from scratch, pursuing **Data-Oriented Design**, **Entity Component System (ECS)**, **high performance**, **high flexibility**, and **high extensibility**.

If you have any ideas or questions, feel free to start a [Discussion](https://github.com/WhitePetal/KairosEngine/discussions) or send me a private message on [Bilibili](https://space.bilibili.com/232017781).

### Why Rust? Why start from scratch?

KairosEngine avoids runtime scripting languages entirely — your game is a Rust program compiled to native code. The ECS core is a bevy_ecs-based fork, and every subsystem around it (renderer, physics, audio, editor) is hand-crafted to fit that data-oriented architecture, giving you full control over memory layout, scheduling, and extensibility.

### Quick Start

```bash
cargo run                    # Launch the editor (dev profile)
cargo run --release          # Optimized build
cargo run --profile bench    # Benchmark-grade performance
```

> The dev profile optimizes your own code at `opt-level=1` (debuggable) and dependencies at `opt-level=3` for a fast inner loop.

---

## 🗺️ Version Roadmap

> **The destination is an open-world ARPG.** KairosEngine's end goal is to be the engine an open-world action RPG is actually built with — so version nodes ship *engine capabilities*, not game demos. Terrain, streaming, animation, state machines and AI are being built for that goal, and the finished engine is demonstrated by one thing only: a full **open-world ARPG game** shipped alongside **1.0.0**.

### 0.1.0
| Feature | Status |
|---------|--------|
| Engine GUI (editor interface) | ✅ |
| Base Graphics | ✅ |
| Base Input System | ✅ |
| Base ECS (bevy_ecs-based) | ✅ |
| Base Physics System | ✅ |
| Base Audio System | ✅ |
| Kairos Editor MCP | 🚧 |
| Kairos Editor Claw | 🚧 |

### 0.2.0
| Feature | Status |
|---------|--------|
| Project / Asset System | 🚧 |
| Terrain System | 📝 |
| Graphics Graph | 🚧 |
| Input Graph System | 📝 |
| World Scenes System | 📝 |

### 0.3.0
| Feature | Status |
|---------|--------|
| State Machine | 📝 |
| Animation System | 📝 |
| GI (Global Illumination) System | 📝 |
| Cinemachine System | 📝 |
| AI Agent | 📝 |

### 1.0.0
- ... (to be announced)
- **Showcase: an open-world ARPG game built entirely with KairosEngine** 📝

### Planned Subsystem Migrations

| Subsystem | Today | Planned |
|-----------|-------|---------|
| Physics | `kairos_physics` — standalone crate backed by **rapier3d** | standalone crate backed by **[avian](https://github.com/Jondolf/avian)** |
| Audio | in-engine module over **kira** + **symphonia** | standalone **bevy_audio**-based crate |

---

## 🧱 Architecture

KairosEngine is a Rust workspace. Each subsystem is its own crate and can be depended on independently of the editor:

| Crate | Description |
|-------|-------------|
| **`kairos_engine`** | Core engine + built-in editor. The binary target. |
| **`kairos_ecs`** | The engine's core ECS — a bevy_ecs-based (forked) standalone crate. |
| **`kairos_ecs_macros`** | Derive macros for the ECS (`Component`, `Resource`, …), with `kairos_ecs_macro_logic` / `kairos_macro_utils` internals. |
| **`kairos_graphics`** | Graphics subsystem — textures, materials, meshes, shaders, render (frame) graph, wgpu pipelines. |
| **`kairos_physics`** | Physics subsystem — `PhysicsEngine` world resource plus `RigidBody`/`Collider` components (rapier3d-backed today; see the roadmap). |
| **`kairos_asset`** | Async asset server, handles, and typed asset systems. |
| **`kairos_math`** | Spatial/geometry math over glam — vectors, quaternions, matrices, AABB, affine. |
| **`kairos_time`** | bevy_time-style clock and fixed timestep. |
| **`kairos_transform`** | bevy_transform-style `LocalTransform` / `GlobalTransform` component pair — the engine's spatial convention is right-handed, Y-up, -Z forward. |
| **`kairos_collections`** | Hashing utilities and deterministic collections shared across crates. |
| **`kairos_ptr`** | Pointer types for the ECS (bevy_ptr parity). |
| **`kairos_tasks`** | Async task pool and parallel-iteration primitives. |
| **`kairos_supervisor`** | Thin watchdog process for crash monitoring. |

### Engine Layout

Subsystems with their own crate live in the crates above; `kairos_engine` holds the editor application, the per-frame wiring, and the game entry point.

```
kairos_engine/src/
├── main.rs                 # Entry point — initializes event loop & editor runtime
├── lib.rs                  # Public module tree (re-exports the subsystem crates)
├── kairos_editor/          # Built-in editor application
│   ├── runtime.rs          # Editor event loop & windowing
│   ├── schedule/           # Editor schedule wiring
│   ├── ui/                 # egui-based editor UI
│   │   ├── inspector/      # Per-type inspectors (material, texture, mesh, shader, audio, ...)
│   │   ├── scene_window/   # 3D scene viewport
│   │   ├── game_window.rs  # Game viewport
│   │   ├── hierarchy_window.rs
│   │   ├── project_window/ # Project file browser
│   │   ├── console_window.rs
│   │   └── ...
│   ├── camera/             # Editor camera
│   ├── project_path_tree/  # Project path tree
│   └── asset_registry.rs   # Asset type registry
├── kairos_ui/              # Shared editor UI primitives (fonts, ...)
├── asset_loader/           # Async asset loading with dependency graph
│   ├── assets.rs           # AssetsServer & AssetsSystem trait
│   └── assets/             # Typed asset systems (mesh, material, texture, shader, audio, ...)
├── audio/                  # Audio engine (kira + symphonia)
│   ├── audio.rs            # Audio state & playback
│   ├── background.rs       # Background music
│   └── spatial/            # 3D spatial audio (listener, reverb zones, volumes)
├── math/                   # Math re-exports + color types
├── kairos_game.rs          # Game logic (what the developer builds with)
├── kairos_paths.rs         # Project path management
├── kairos_settings.rs      # Editor/project settings
├── inputs.rs               # Input engine (keyboard/mouse mapping)
├── spatial.rs              # Spatial helpers (right-handed, Y-up, -Z forward)
└── log.rs                  # In-editor logging
```

---

## ✨ Key Features

### ⚙️ ECS (bevy_ecs-based)

The ECS core is no longer hecs-based — it is **bevy_ecs-based**. `kairos_ecs` is an in-repo fork of bevy's ECS (tracking bevy v0.19.x parity) kept as a standalone crate, so the engine inherits a battle-tested data model instead of re-deriving one:

- **Archetype storage** — columnar component tables with sparse-set lookup and fast bundle spawning
- **Queries** — `Query` / `QueryBuilder` with filters (`With` / `Without` / `Added` / `Changed`), joins, and parallel iteration
- **Change detection** — per-component tick tracking via `Ref` / `Mut`
- **Schedules** — ordered system sets, labels, run conditions, deferred commands, and a fixed-timestep driver (`kairos_time`, bevy `Time<Fixed>` parity)
- **Observers, messages & lifecycle** — `On<…>` observers, `Messages`, and `Add` / `Insert` / `Remove` / `Despawn` hooks
- **Relationships & hierarchy** — `ChildOf` / `Children` plus generic relationships
- **Supporting crates** — `kairos_ptr` (bevy_ptr parity), `kairos_collections` (deterministic hashing/collections), `kairos_ecs_macros` (derives)

### 🎨 GPU Rendering

- **wgpu 0.29** — modern Vulkan / Metal / DX12 backend
- Full render pipeline management (shaders, bind groups, attachments)
- **Texture system** with SDR (8-bit) and HDR (f16/f32) encode/decode, sRGB handling, GPU compression support
- Frame graph abstraction for organizing render passes
- Camera, mesh, material, and vertex buffer management
- GLTF model import

### 🖥️ Built-in Editor

- Full egui-based editor with **dockable windows**
- **Inspector system** — type-specific editors for materials, textures, meshes, shaders, audio, code, TOML configs, and more
- **Scene viewport** (3D camera control)
- **Game viewport** (runtime preview)
- **Hierarchy panel** — entity tree browsing
- **Project file browser** with path tree
- **Console / log panel** — in-editor log output
- **Preferences & settings** — editor theme, fonts, project config
- Syntax highlighting for shaders and code assets

### 🏗️ Physics

`kairos_physics` is a standalone crate: rigid body dynamics, colliders, joints, and CCD (continuous collision detection), currently simulated by **rapier3d** behind a `PhysicsEngine` world resource plus `RigidBody` / `Collider` components, with parallel simulation support.

> **Planned:** this module moves to a standalone crate built on **[avian](https://github.com/Jondolf/avian)** — an ECS-native physics engine, which fits the bevy_ecs-based core more naturally. See the roadmap.

### 🔊 Audio

Spatial 3D audio via **kira** with **symphonia** decoding (MP3, WAV, FLAC, Ogg, MP4), background music tracks, reverb zones, and per-listener track budgets.

> **Planned:** the audio module becomes a standalone crate built on **bevy_audio**, so audio entities and playback follow the same ECS idioms as the rest of the engine.

### 📦 Asset System

- **Async loading** with tokio — dependency-graph-aware asset loading
- Typed asset systems: meshes, materials, textures, shaders, audio, syntax files, TOML configs
- Serialized binary asset format for fast loading
- Hot-reload ready (handle system)

### 🔬 Testing

- **Rust tests per crate** — logic/data validation without a GPU or an engine loop, via the `cargo test-crate` alias
- **Editor-driven verification** — runtime behaviour (GPU, egui, physics, input) is meant to be checked by driving the real editor through the Kairos Editor MCP, the approach that replaces the retired TOML runtime test harness; `kairos_supervisor` remains as a crash watchdog

---

## 🧪 Testing

```bash
cargo test-crate kairos_ecs   # test only the crate you changed — the default (~8s)
cargo test-fast               # every crate, no doctests (~12s)
cargo test-full               # every crate + doctests — the merge gate (~183s)
```

See [`docs/agents/testing.md`](KairosEngine/docs/agents/testing.md) for the full testing policy.

---

## 📐 Design Principles

- **Data-Oriented Design** — ECS-first architecture; components are plain data, cache-friendly memory layout
- **No scripting VM** — games are Rust programs using the engine as a library
- **GPU-first** — texture encoding, compression, and rendering are native GPU operations
- **Modular** — every subsystem owns its types; the editor is a consumer, not the core
- **Testable** — per-crate Rust tests keep pure logic fast to verify, while runtime behaviour is checked by driving the real editor
- **Extensible** — easy to swap subsystems, add new component types, and extend the editor

Architecture decisions are documented as **ADRs** in [`docs/adr/`](KairosEngine/docs/adr/).

---

## 🛠️ Dependencies

### Core Crates

| Domain | Library |
|--------|---------|
| ECS | in-repo `kairos_ecs` — a **bevy_ecs**-based fork, kept at bevy v0.19.x parity |
| Graphics | [wgpu](https://wgpu.rs) 0.29 |
| Editor UI | [egui](https://egui.rs) 0.35, egui-wgpu, egui-winit |
| Physics | [rapier3d](https://rapier.rs) 0.33 → [avian](https://github.com/Jondolf/avian) (planned) |
| Audio | [kira](https://github.com/tesselode/kira) 0.12, [symphonia](https://github.com/pdeljanov/Symphonia) 0.5 → bevy_audio (planned) |
| Asset loading | [image](https://github.com/image-rs/image) (PNG), [gltf](https://github.com/gltf-rs/gltf) 1.4 |
| Math | [glam](https://github.com/bitshifter/glam-rs) 0.33, [mint](https://github.com/kvark/mint) |
| Async | [tokio](https://tokio.rs) (full), [crossbeam-channel](https://docs.rs/crossbeam-channel) |
| Serialization | [rkyv](https://github.com/rkyv/rkyv) (zero-copy), [serde](https://serde.rs), [sonic-rs](https://github.com/cloudflare/sonic-rs) |
| Windowing | [winit](https://github.com/rust-windowing/winit) 0.30 |

> Full dependency list in [`Cargo.toml`](KairosEngine/Cargo.toml).

### Thanks

KairosEngine thanks every open-source project it stands on — **[hecs](https://github.com/Ralith/hecs)**, **[rapier](https://rapier.rs)**, and **[kira](https://github.com/tesselode/kira)** among them, each of which shaped this engine's design as well as its code. The full list lives in [Thanks.md](Thanks.md).

---

## 📁 Resource Directory

```
res/
├── models/        # GLTF & serialized mesh assets
├── textures/      # Texture assets
├── materials/     # Material definitions
├── shaders/       # Shader sources
└── audios/        # Audio files
```

---

## 🤝 Get Involved

<p align="center">
  <a href="https://github.com/WhitePetal/KairosEngine/discussions">
    <img src="https://img.shields.io/badge/💬_Join_the_Discussion-181717?style=for-the-badge&logo=github" alt="Join the Discussion">
  </a>
  <a href="https://space.bilibili.com/232017781">
    <img src="https://img.shields.io/badge/📺_B站_私信-00A1D6?style=for-the-badge&logo=bilibili" alt="Bilibili">
  </a>
</p>

### Contributing

The project uses:
- [GitHub Issues](https://github.com/WhitePetal/KairosEngine/issues) for issue tracking
- Five canonical triage labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`
- Architecture Decision Records in [`docs/adr/`](KairosEngine/docs/adr/)
- AI agents follow instructions in [`AGENTS.md`](KairosEngine/AGENTS.md)

### AI Tools Used

KairosEngine development is assisted by:
- **DeepSeek**
- **Cursor**
- **GPT**
- **Kimi**
- **即梦**

---

## 📄 License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

---

<div align="center">
  <sub>Built with 🦀 in Rust</sub>
  <br>
  <img src="https://komarev.com/ghpvc/?username=WhitePetal" alt="Profile views">
</div>
