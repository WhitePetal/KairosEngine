# <img src="KairosEngine/Preferences/Textures/engine_icon.png" width = "24" height = "24" > KairosEngine

<div align="center">

**一个从新开始审视游戏工业的纯 Rust 游戏引擎 — 面向数据设计、ECS 架构、高性能、高灵活性、高可扩展性。**

<br>

[![Rust](https://img.shields.io/badge/语言-Rust_2024_edition-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/许可证-MIT_OR_Apache--2.0-blue?style=flat-square)](https://github.com/WhitePetal/KairosEngine)
[![wgpu](https://img.shields.io/badge/GPU-wgpu_29-green?style=flat-square&logo=webgpu)](https://wgpu.rs)
[![Status](https://img.shields.io/badge/状态-积极开发中-2ea44f?style=flat-square)](https://github.com/WhitePetal/KairosEngine)
[![Discussions](https://img.shields.io/badge/讨论-Discussions-5865F2?style=flat-square&logo=github)](https://github.com/WhitePetal/KairosEngine/discussions)
[![Bilibili](https://img.shields.io/badge/B站-私信-00A1D6?style=flat-square&logo=bilibili)](https://space.bilibili.com/232017781)

[English Documentation](README.md)

</div>

---

## 📋 简介

从新开始审视游戏工业 —— **KairosEngine** 是完全用 Rust 自底向上构建的游戏引擎，目标是 **面向数据设计（Data-Oriented Design）**、**ECS 架构**、**高性能**、**高灵活性** 和 **高可扩展性**。

如果你有任何想法和问题，欢迎在 [Discussions](https://github.com/WhitePetal/KairosEngine/discussions) 中讨论，也可以直接在 [B站(Bilibili)](https://space.bilibili.com/232017781) 私信我。

### 为什么用 Rust？为什么从零开始？

KairosEngine 不依赖任何运行时脚本语言 —— 你的游戏就是编译为原生代码的 Rust 程序。ECS 核心基于 bevy_ecs 的 fork，围绕它的每个子系统（渲染器、物理、音频、编辑器）都是手工打造，遵循一致的数据导向架构，让你完全掌控内存布局、调度和可扩展性。

### 快速开始

```bash
cargo run                    # 启动编辑器（dev 模式）
cargo run --release          # 发布构建（极致性能）
cargo run --profile bench    # 基准测试级别性能
```

> Dev 模式下，自写代码使用 `opt-level=1`（可调试），依赖使用 `opt-level=3`（运行时高效），兼顾调试体验与运行速度。

---

## 🗺️ 版本计划

> **终点是一款开放世界 ARPG。** KairosEngine 的最终目标，是成为真正可以用来制作开放世界动作角色扮演游戏的引擎 —— 因此每个版本节点交付的是**引擎能力**，而不是游戏 Demo。地形、流式加载、动画、状态机与 AI 都在为这个目标服务，而引擎的最终成果只用一样东西来展示：随 **1.0.0** 一同发布的完整 **开放世界 ARPG 游戏**。

### 0.1.0

| 功能 | 状态 |
|---------|--------|
| 引擎 GUI 界面 | ✅ |
| Base Graphics | ✅ |
| Base Input System | ✅ |
| Base ECS（基于 bevy_ecs） | ✅ |
| Base Physics System | ✅ |
| Base Audio System | ✅ |
| Kairos Editor MCP | 🚧 |
| Kairos Editor Claw | 🚧 |

### 0.2.0

| 功能 | 状态 |
|---------|--------|
| Project / Asset System | 🚧 |
| Terrain System | 📝 |
| Graphics Graph | 🚧 |
| Input Graph System | 📝 |
| World Scenes System | 📝 |

### 0.3.0

| 功能 | 状态 |
|---------|--------|
| State Machine | 📝 |
| Animation System | 📝 |
| GI (Global Illumination) System | 📝 |
| Cinemachine System | 📝 |
| AI Agent | 📝 |

### 1.0.0

- ……（待定）
- **展示作品：完全使用 KairosEngine 制作的开放世界 ARPG 游戏** 📝

### 计划中的子系统迁移

| 子系统 | 当前 | 计划 |
|-----------|-------|---------|
| 物理 | `kairos_physics` —— 基于 **rapier3d** 的独立 crate | 基于 **[avian](https://github.com/Jondolf/avian)** 的独立 crate |
| 音频 | 引擎内模块，基于 **kira** + **symphonia** | 基于 **bevy_audio** 的独立 crate |

---

## 🧱 架构

KairosEngine 是一个 Rust workspace，每个子系统都是独立 crate，可以脱离编辑器单独依赖：

| 包 | 描述 |
|-------|-------------|
| **`kairos_engine`** | 核心引擎 + 内置编辑器（可执行目标） |
| **`kairos_ecs`** | 引擎的 ECS 核心 —— 基于 bevy_ecs 的 fork，独立 crate |
| **`kairos_ecs_macros`** | ECS 的过程宏（`Component`、`Resource`……），内部为 `kairos_ecs_macro_logic` / `kairos_macro_utils` |
| **`kairos_graphics`** | 图形子系统 —— 纹理、材质、网格、着色器、帧图、wgpu 管线 |
| **`kairos_physics`** | 物理子系统 —— `PhysicsEngine` World 资源与 `RigidBody`/`Collider` 组件（当前由 rapier3d 驱动，见版本计划） |
| **`kairos_asset`** | 异步资源服务器、资源句柄与类型化资源系统 |
| **`kairos_math`** | 基于 glam 的空间/几何数学 —— 向量、四元数、矩阵、AABB、仿射 |
| **`kairos_time`** | bevy_time 风格的时钟与固定时间步 |
| **`kairos_transform`** | bevy_transform 风格的 `LocalTransform` / `GlobalTransform` 组件对 —— 引擎空间约定为右手系、Y 向上、-Z 向前 |
| **`kairos_collections`** | 各 crate 共用的哈希工具与确定性集合 |
| **`kairos_ptr`** | ECS 指针类型（与 bevy_ptr 对齐） |
| **`kairos_tasks`** | 异步任务池与并行迭代原语 |
| **`kairos_supervisor`** | 轻量看门狗进程（崩溃监控） |

### 引擎代码结构

拥有独立 crate 的子系统在上面列出；`kairos_engine` 负责编辑器应用、逐帧接线以及游戏入口。

```
kairos_engine/src/
├── main.rs                 # 入口 — 初始化事件循环和编辑器运行时
├── lib.rs                  # 公开模块树（重导出各子系统 crate）
├── kairos_editor/          # 内置编辑器应用
│   ├── runtime.rs          # 编辑器事件循环与窗口
│   ├── schedule/           # 编辑器调度接线
│   ├── ui/                 # 基于 egui 的编辑器界面
│   │   ├── inspector/      # 类型化检查器（材质、纹理、网格、着色器、音频……）
│   │   ├── scene_window/   # 3D 场景视口
│   │   ├── game_window.rs  # 游戏运行视口
│   │   ├── hierarchy_window.rs  # 实体层级面板
│   │   ├── project_window/ # 项目文件浏览器
│   │   ├── console_window.rs    # 控制台/日志面板
│   │   └── ...
│   ├── camera/             # 编辑器相机
│   ├── project_path_tree/  # 项目路径树
│   └── asset_registry.rs   # 资源类型注册表
├── kairos_ui/              # 编辑器通用 UI 基础件（字体等）
├── asset_loader/           # 异步资源加载（依赖图感知）
│   ├── assets.rs           # AssetsServer & AssetsSystem trait
│   └── assets/             # 类型化资源系统（网格、材质、纹理、着色器、音频……）
├── audio/                  # 音频引擎 (kira + symphonia)
│   ├── audio.rs            # 音频状态与播放
│   ├── background.rs       # 背景音乐
│   └── spatial/            # 3D 空间音频（监听器、混响区域、音量）
├── math/                   # 数学重导出 + 颜色类型
├── kairos_game.rs          # 游戏逻辑（开发者在此编写游戏）
├── kairos_paths.rs         # 项目路径管理
├── kairos_settings.rs      # 编辑器/项目设置
├── inputs.rs               # 输入引擎（键盘/鼠标映射）
├── spatial.rs              # 空间辅助（右手系、Y 向上、-Z 向前）
└── log.rs                  # 编辑器内日志面板
```

---

## ✨ 核心特性

### ⚙️ ECS（基于 bevy_ecs）

ECS 核心已经不再基于 hecs，而是**基于 bevy_ecs**：`kairos_ecs` 是 bevy ECS 的仓库内 fork（与 bevy v0.19.x 保持对齐），作为独立 crate 维护 —— 引擎因此直接继承了久经考验的数据模型，而不是重新推导一套：

- **Archetype 存储** — 列式组件表、稀疏集查找、快速的 bundle 生成
- **查询** — `Query` / `QueryBuilder`，支持过滤器（`With` / `Without` / `Added` / `Changed`）、join 与并行迭代
- **变更检测** — 通过 `Ref` / `Mut` 进行逐组件 tick 追踪
- **调度** — 有序 system set、label、run condition、延迟命令，以及固定时间步驱动（`kairos_time`，与 bevy `Time<Fixed>` 对齐）
- **观察者、消息与生命周期** — `On<…>` 观察者、`Messages`，以及 `Add` / `Insert` / `Remove` / `Despawn` 钩子
- **关系与层级** — `ChildOf` / `Children` 以及通用 relationship
- **配套 crate** — `kairos_ptr`（与 bevy_ptr 对齐）、`kairos_collections`（确定性哈希/集合）、`kairos_ecs_macros`（派生宏）

### 🎨 GPU 渲染

- **wgpu 0.29** — 现代 Vulkan / Metal / DX12 后端
- 完整的渲染管线管理（着色器、绑定组、附件）
- **纹理系统** — 支持 SDR（8-bit）和 HDR（f16/f32）编解码、sRGB 处理、GPU 压缩格式
- **帧图（Graphics Graph）** 抽象 — 组织渲染帧
- 相机、网格、材质和顶点缓冲管理
- GLTF 模型导入

### 🖥️ 内置编辑器

- 基于 egui 的完整编辑器，支持 **可停靠窗口**
- **检查器系统（Inspector）** — 针对材质、纹理、网格、着色器、音频、代码、TOML 配置等的类型化编辑工具
- **场景视口**（3D 相机控制）
- **游戏视口**（运行时预览）
- **层级面板** — 实体树浏览
- **项目文件浏览器**
- **控制台/日志面板** — 编辑器内日志输出
- **偏好设置** — 编辑器主题、字体、项目配置
- 着色器和代码资产的语法高亮

### 🏗️ 物理引擎

`kairos_physics` 是独立 crate：刚体动力学、碰撞体、关节和连续碰撞检测（CCD），当前由 **rapier3d** 在 `PhysicsEngine` World 资源与 `RigidBody` / `Collider` 组件背后驱动，支持并行模拟。

> **计划中：** 该模块将迁移为基于 **[avian](https://github.com/Jondolf/avian)** 的独立 crate —— avian 是 ECS 原生的物理引擎，与基于 bevy_ecs 的核心更契合。详见版本计划。

### 🔊 音频引擎

通过 **kira** 实现 3D 空间音频，**symphonia** 解码多种格式（MP3、WAV、FLAC、Ogg、MP4），支持背景音乐轨道、混响区域以及每个监听器的轨道预算。

> **计划中：** 音频模块将迁移为基于 **bevy_audio** 的独立 crate，使音频实体与播放遵循与引擎其余部分一致的 ECS 习惯。

### 📦 资源系统

- **异步加载** — 基于 tokio，依赖图感知的资源加载
- 类型化资源系统：网格、材质、纹理、着色器、音频、语法文件、TOML 配置
- 序列化二进制格式，支持快速加载
- 句柄系统，为热重载做好准备

### 🔬 测试策略

- **按 crate 运行 Rust 测试** — 逻辑与数据验证，无需 GPU 或引擎循环，通过 `cargo test-crate` 别名执行
- **编辑器驱动验证** — 运行时行为（GPU、egui、物理、输入）的目标验证方式是通过 Kairos Editor MCP 驱动真实编辑器，取代已废弃的 TOML 运行时测试框架；`kairos_supervisor` 仍作为崩溃看门狗保留

---

## 🧪 运行测试

```bash
cargo test-crate kairos_ecs   # 只测你改动的那个 crate —— 日常默认（约 8s）
cargo test-fast               # 全工作区、跳过 doctest（约 12s）
cargo test-full               # 全工作区含 doctest —— 合并前的门禁（约 183s）
```

详见 [`docs/agents/testing.md`](KairosEngine/docs/agents/testing.md)。

---

## 📐 设计原则

- **面向数据设计 (Data-Oriented Design)** — ECS 优先架构；组件就是纯数据，缓存友好的内存布局
- **无脚本 VM** — 游戏就是使用引擎作为库的 Rust 程序
- **GPU 优先** — 纹理编码、压缩和渲染都是原生 GPU 操作
- **模块化** — 每个子系统拥有自己的类型；编辑器是消费者而非核心
- **可测试** — 按 crate 的 Rust 测试让纯逻辑的验证保持廉价，运行时行为则通过驱动真实编辑器来验证
- **可扩展** — 易于替换子系统、添加新组件类型、扩展编辑器

架构决策记录（ADR）详见 [`docs/adr/`](KairosEngine/docs/adr/)。

---

## 🛠️ 核心依赖

### 主要 Crate

| 领域 | 库 |
|--------|------|
| ECS | 仓库内 `kairos_ecs` —— 基于 **bevy_ecs** 的 fork，与 bevy v0.19.x 对齐 |
| 图形 | [wgpu](https://wgpu.rs) 0.29 |
| 编辑器 UI | [egui](https://egui.rs) 0.35, egui-wgpu, egui-winit |
| 物理 | [rapier3d](https://rapier.rs) 0.33 → [avian](https://github.com/Jondolf/avian)（计划中） |
| 音频 | [kira](https://github.com/tesselode/kira) 0.12, [symphonia](https://github.com/pdeljanov/Symphonia) 0.5 → bevy_audio（计划中） |
| 资源加载 | [image](https://github.com/image-rs/image) (PNG), [gltf](https://github.com/gltf-rs/gltf) 1.4 |
| 数学 | [glam](https://github.com/bitshifter/glam-rs) 0.33, [mint](https://github.com/kvark/mint) |
| 异步 | [tokio](https://tokio.rs) (full), [crossbeam-channel](https://docs.rs/crossbeam-channel) |
| 序列化 | [rkyv](https://github.com/rkyv/rkyv) (零拷贝), [serde](https://serde.rs), [sonic-rs](https://github.com/cloudflare/sonic-rs) |
| 窗口管理 | [winit](https://github.com/rust-windowing/winit) 0.30 |

> 完整依赖列在 [`Cargo.toml`](KairosEngine/Cargo.toml)。

### 致谢

KairosEngine 感谢它所依托的每一个开源项目 —— 其中包括 **[hecs](https://github.com/Ralith/hecs)**、**[rapier](https://rapier.rs)** 与 **[kira](https://github.com/tesselode/kira)**，它们既塑造了本引擎的设计，也塑造了它的代码。完整列表见 [Thanks.md](Thanks.md)。

---

## 📁 资源目录

```
res/
├── models/        # GLTF 和序列化网格资产
├── textures/      # 纹理资产
├── materials/     # 材质定义
├── shaders/       # 着色器源码
└── audios/        # 音频文件
```

---

## 🤝 参与贡献 & 联系

<p align="center">
  <a href="https://github.com/WhitePetal/KairosEngine/discussions">
    <img src="https://img.shields.io/badge/💬_加入讨论-181717?style=for-the-badge&logo=github" alt="加入讨论">
  </a>
  <a href="https://space.bilibili.com/232017781">
    <img src="https://img.shields.io/badge/📺_B站_私信-00A1D6?style=for-the-badge&logo=bilibili" alt="Bilibili">
  </a>
</p>

### 贡献指南

本项目使用：
- [GitHub Issues](https://github.com/WhitePetal/KairosEngine/issues) 进行问题跟踪
- 五个分类标签：`needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human`、`wontfix`
- 架构决策记录（ADR）位于 [`docs/adr/`](KairosEngine/docs/adr/)
- AI 代理遵循 [`AGENTS.md`](KairosEngine/AGENTS.md) 中的指引

### 使用的 AI 工具

KairosEngine 开发过程中使用的 AI 辅助工具：
- **DeepSeek**
- **Cursor**
- **GPT**
- **Kimi**
- **即梦**

---

## 📄 许可证

本项目的许可协议为以下二者之一（任选其一）：

- Apache License, Version 2.0（[LICENSE-APACHE](LICENSE-APACHE)）
- MIT 许可证（[LICENSE-MIT](LICENSE-MIT)）

---

<div align="center">
  <sub>🦀 使用 Rust 构建</sub>
  <br>
  <img src="https://komarev.com/ghpvc/?username=WhitePetal" alt="访问计数">
</div>
