# 🙏 Thanks

KairosEngine stands on the shoulders of giants. This page acknowledges the open-source projects that make this engine possible — some are dependencies today, others shaped the design without ever being depended on. Every row says which.

---

## ECS & Engine Architecture

| Project | Contribution |
|---------|--------------|
| [bevy](https://bevy.org/) / bevy_ecs | The ECS KairosEngine's core is now built on — `kairos_ecs` is a fork kept at bevy v0.19.x parity — plus broad guidance from the wider Rust engine ecosystem |
| [hecs](https://github.com/Ralith/hecs) | Archetype tables, sparse sets, and columnar storage — the design that shaped KairosEngine's first (hecs-based) ECS core |
| [ENTT](https://github.com/skypjack/entt) | ECS design patterns, component management model |
| [Flecs](https://www.flecs.dev/flecs/) | ECS design patterns, query system reference |
| [beef-lang](https://www.beeflang.org/) | Language design philosophy |

## Graphics & GPU

| Project | Contribution |
|---------|--------------|
| [wgpu](https://wgpu.rs) | Modern GPU abstraction — Vulkan, Metal, DX12 backends |
| [naga](https://github.com/gfx-rs/naga) | Shader translation (via wgpu) |
| [gltf](https://github.com/gltf-rs/gltf) | GLTF 2.0 loader |
| [image](https://github.com/image-rs/image) | Image decoding and processing |
| [half](https://github.com/starkat99/half-rs) | f16 (half-precision float) for HDR texture pipelines |

## Editor UI

| Project | Contribution |
|---------|--------------|
| [egui](https://egui.rs) | Immediate-mode GUI library |
| [egui-wgpu](https://github.com/emilk/egui) | egui + wgpu renderer integration |
| [egui-winit](https://github.com/emilk/egui) | egui + winit platform integration |
| [egui_extras](https://github.com/emilk/egui) | Additional egui widgets |
| [egui_commonmark](https://github.com/emilk/egui) | Markdown rendering in egui |
| [syntect](https://github.com/trishume/syntect) | Syntax highlighting for code/shader inspectors |
| [Dear ImGui](https://github.com/ocornut/imgui) | The immediate-mode GUI paradigm that egui and this editor follow |
| [egui_dock](https://github.com/anhosh/egui_dock) | Dockable window layout reference |

## Windowing & Platform

| Project | Contribution |
|---------|--------------|
| [winit](https://github.com/rust-windowing/winit) | Cross-platform window creation and event loop |
| [objc2](https://github.com/madsmtm/objc2) | Objective-C bindings (macOS) |

## Physics

| Project | Contribution |
|---------|--------------|
| [rapier3d](https://rapier.rs) | 3D physics engine — rigid bodies, colliders, joints, CCD (the current simulation backend) |
| [parry3d](https://github.com/dimforge/parry) | Shape / collision queries (via rapier3d) |

## Audio

| Project | Contribution |
|---------|--------------|
| [kira](https://github.com/tesselode/kira) | Audio engine — playback, mixing, spatial audio (the current backend) |
| [symphonia](https://github.com/pdeljanov/Symphonia) | Audio decoding — MP3, WAV, FLAC, Ogg, MP4 |
| [rustfft](https://github.com/rustfft/rustfft) | FFT for audio processing |

## Async & Concurrency

| Project | Contribution |
|---------|--------------|
| [tokio](https://tokio.rs) | Async runtime — used for asset loading |
| [crossbeam-channel](https://github.com/crossbeam-rs/crossbeam) | Multi-producer, multi-consumer channels |
| [rayon](https://github.com/rayon-rs/rayon) | Data parallelism |
| [parking_lot](https://github.com/Amanieu/parking_lot) | Fast synchronization primitives |

## Serialization & Data

| Project | Contribution |
|---------|--------------|
| [rkyv](https://github.com/rkyv/rkyv) | Zero-copy deserialization for binary assets |
| [serde](https://serde.rs) | Generic serialization framework |
| [sonic-rs](https://github.com/cloudflare/sonic-rs) | Fast JSON parsing |
| [toml](https://github.com/toml-rs/toml) | TOML configuration format |
| [bytemuck](https://github.com/Lokathor/bytemuck) | Safe zero-cost bit casting |

## Math & Utilities

| Project | Contribution |
|---------|--------------|
| [glam](https://github.com/bitshifter/glam-rs) | SIMD-friendly vectors, matrices, quaternions (used directly and shared via rapier3d) |
| [mint](https://github.com/kvark/mint) | Math type interoperability standard |
| [rand](https://github.com/rust-random/rand) | Random number generation |
| [smallvec](https://github.com/servo/smallvec) | Small-vector optimization |
| [foldhash](https://github.com/orlp/foldhash) | Fast, folded hash function |
| [uuid](https://github.com/uuid-rs/uuid) | Unique identifier generation |
| [petgraph](https://github.com/petgraph/petgraph) | Graph data structures — used by the editor's project path tree |

## Macros

| Project | Contribution |
|---------|--------------|
| [syn](https://github.com/dtolnay/syn) | Rust tokenizer and parser for proc macros |
| [quote](https://github.com/dtolnay/quote) | Token stream interpolation |
| [proc-macro2](https://github.com/dtolnay/proc-macro2) | Proc macro abstraction |

## Development & Debugging

| Project | Contribution |
|---------|--------------|
| [env_logger](https://github.com/rust-cli/env_logger) | Logging configuration |
| [log](https://github.com/rust-lang/log) | Logging facade |
| [anyhow](https://github.com/dtolnay/anyhow) | Flexible error handling |
| [criterion](https://github.com/bheisler/criterion.rs) | Benchmarking framework |

## Fonts

| Project | Contribution |
|---------|--------------|
| [NotoSansSC-Regular](https://fonts.google.com/noto/specimen/Noto+Sans+SC) | Chinese font support |

---

## Tools & Infrastructure

| Tool | Purpose |
|------|---------|
| [Rust](https://www.rust-lang.org) & [Cargo](https://doc.rust-lang.org/cargo/) | Language, build system, package manager |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | License and dependency linting |

---

## ❤️ Special Thanks

To the entire Rust open-source ecosystem — every library, every contributor, every issue filed. None of this would be possible without the community.

---

*If you believe a project is missing from this list, please open an issue or a PR.*
