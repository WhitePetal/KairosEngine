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

KairosEngine 不依赖任何运行时脚本语言 —— 你的游戏就是编译为原生代码的 Rust 程序。每个子系统（ECS、渲染器、物理、音频、编辑器）都是手工打造，遵循一致的数据导向架构，让你完全掌控内存布局、调度和可扩展性。

### 快速开始

```bash
cargo run                    # 启动编辑器（dev 模式）
cargo run --release          # 发布构建（极致性能）
cargo run --profile bench    # 基准测试级别性能
```

> Dev 模式下，自写代码使用 `opt-level=1`（可调试），依赖使用 `opt-level=3`（运行时高效），兼顾调试体验与运行速度。

---

## 🗺️ 版本计划

### 0.1.0

| 功能 | 状态 |
|---------|--------|
| 引擎 GUI 界面 | ✅ |
| Base Graphics | ✅ |
| Base Input System | ✅ |
| Base ECS（参考 ENTT / Flecs） | ✅ |
| Base Physics System | ✅ |
| Base Audio System | ✅ |
| Kairos Editor MCP | 🚧 |
| Kairos Editor Claw | 🚧 |
| Demo: 足球游戏 | 📝 |

### 0.2.0

| 功能 | 状态 |
|---------|--------|
| Project / Asset System | 🚧 |
| Terrain System | 📝 |
| Graphics Graph | 🚧 |
| Input Graph System | 📝 |
| World Scenes System | 📝 |
| Demo: 赛车游戏 | 📝 |

### 0.3.0

| 功能 | 状态 |
|---------|--------|
| State Machine | 📝 |
| Animation System | 📝 |
| GI (Global Illumination) System | 📝 |
| Cinemachine System | 📝 |
| AI Agent | 📝 |
| Demo: 动作游戏 | 📝 |

### 1.0.0

- ...（待定）
- Demo: ...（待定）

---

## 🧱 架构

KairosEngine 由三个 Cargo workspace 成员组成：

| 包 | 描述 |
|-------|-------------|
| **`kairos_engine`** | 核心引擎 + 内置编辑器（可执行目标） |
| **`kairos_ecs_macros`** | ECS 系统的过程宏 |
| **`kairos_supervisor`** | 测试框架的轻量看门狗进程（崩溃监控） |

### 引擎代码结构

```
kairos_engine/src/
├── main.rs                 # 入口 — 初始化事件循环和编辑器运行时
├── lib.rs                  # 公开模块树
├── ecs/                    # 自定义实体组件系统 (ECS)
│   ├── world.rs            # World — ECS 核心容器
│   ├── entity.rs           # 实体句柄
│   ├── component.rs        # 组件 trait 与存储
│   ├── table.rs            # Archetype 表
│   ├── table_graph.rs      # 表转换图（实体形态变化）
│   ├── sparse_set.rs       # 稀疏集存储
│   ├── batch.rs            # 批量实体操作
│   ├── borrow.rs           # 查询借用检查
│   └── ...
├── graphics/               # GPU 渲染管线
│   ├── render_pipeline.rs  # wgpu 渲染管线管理
│   ├── shader.rs           # 着色器加载与编译
│   ├── texture.rs          # 纹理编解码（SDR + HDR）
│   ├── mesh.rs             # 网格数据与 GLTF 导入
│   ├── material.rs         # 材质系统
│   ├── camera.rs           # 相机/视口管理
│   ├── vertex.rs           # 顶点布局定义
│   ├── render_state.rs     # 渲染状态管理
│   └── graphics_graph.rs   # 帧图（渲染帧结构）
├── physics/                # 物理引擎 (rapier3d)
│   ├── rigid_body.rs       # 刚体组件
│   └── collider.rs         # 碰撞体组件
├── audio/                  # 音频引擎 (kira + symphonia)
│   ├── audio.rs            # 音频状态与播放
│   ├── background.rs       # 背景音乐
│   └── spatial.rs          # 3D 空间音频
├── asset_loader/           # 异步资源加载（依赖图感知）
│   ├── assets.rs           # AssetsServer & AssetsSystem trait
│   └── asset.rs            # AssetHandle 与类型化系统
├── kairos_editor/          # 内置编辑器应用
│   ├── runtime.rs          # 编辑器事件循环与窗口
│   ├── ui/                 # 基于 egui 的编辑器界面
│   │   ├── inspector/      # 类型化检查器（材质、纹理、网格、着色器、音频……）
│   │   ├── scene_window.rs # 3D 场景视口
│   │   ├── game_window.rs  # 游戏运行视口
│   │   ├── hierarchy_window.rs  # 实体层级面板
│   │   ├── project_window.rs    # 项目文件浏览器
│   │   └── ...
│   └── asset_registry.rs   # 资源类型注册表
├── kairos_game.rs          # 游戏逻辑模板（开发者在此编写游戏）
├── kairos_paths.rs         # 项目路径管理
├── kairos_settings.rs      # 编辑器/项目设置
├── inputs.rs               # 输入引擎（键盘/鼠标映射）
├── math.rs                 # 数学库（向量、矩阵、四元数、颜色、三角函数）
├── spatial.rs              # 空间系统（Transform、AABB、右手系-Y向上）
├── timer.rs                # 帧计时与时间缩放
├── types.rs                # TypeIdMap — 优化 TypeId 键控哈希表
└── log.rs                  # 编辑器内日志面板
```

---

## ✨ 核心特性

### ⚙️ ECS（hecs 风格，自实现）

设计上参考 **hecs**，采用 Archetype 表、稀疏集、列式存储和实体代际 ID。额外扩展了表转换图（追踪 Archetype 变化）、批量实体操作和显式的借用检查系统以支持安全的并发查询。

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

通过 **rapier3d** 实现刚体动力学、碰撞体、关节和连续碰撞检测（CCD），支持并行模拟。

### 🔊 音频引擎

通过 **kira** 实现 3D 空间音频，**symphonia** 解码多种格式（MP3、WAV、FLAC、Ogg、MP4），支持背景音乐轨道和混响。

### 📦 资源系统

- **异步加载** — 基于 tokio，依赖图感知的资源加载
- 类型化资源系统：网格、材质、纹理、着色器、音频、语法文件、TOML 配置
- 序列化二进制格式，支持快速加载
- 句柄系统，为热重载做好准备

### 🔬 测试策略

两层测试架构：
- **Rust 集成测试** — 逻辑和数据验证，无需 GPU 或引擎循环
- **TOML 运行时测试** — GPU、egui UI、物理、ECS 调度和输入路径的运行时验证，通过 `kairos_supervisor` 看门狗监控崩溃

---

## 🧪 运行测试

```bash
# 集成测试（逻辑与数据，无需 GPU）
cargo test

# 运行时测试（GPU、物理、egui 等）
cargo run --features test-harness          # 启动测试框架
```

详见 [Kairos Test Harness 说明](.agents/skills/kairos-test/SKILL.md)。

---

## 📐 设计原则

- **面向数据设计 (Data-Oriented Design)** — ECS 优先架构；组件就是纯数据，缓存友好的内存布局
- **无脚本 VM** — 游戏就是使用引擎作为库的 Rust 程序
- **GPU 优先** — 纹理编码、压缩和渲染都是原生 GPU 操作
- **模块化** — 每个子系统拥有自己的类型；编辑器是消费者而非核心
- **可测试** — 两层测试架构将纯逻辑与运行时 GPU 交互分离
- **可扩展** — 易于替换子系统、添加新组件类型、扩展编辑器

架构决策记录（ADR）详见 [`docs/adr/`](docs/adr/)。

---

## 🛠️ 核心依赖

### 主要 Crate

| 领域 | 库 |
|--------|------|
| 图形 | [wgpu](https://wgpu.rs) 0.29 |
| 编辑器 UI | [egui](https://egui.rs) 0.35, egui-wgpu, egui-winit |
| 物理 | [rapier3d](https://rapier.rs) 0.33 |
| 音频 | [kira](https://github.com/tesselode/kira) 0.12, [symphonia](https://github.com/pdeljanov/Symphonia) 0.5 |
| 资源加载 | [image](https://github.com/image-rs/image) (PNG), [gltf](https://github.com/gltf-rs/gltf) 1.4 |
| 数学 | [glam](https://github.com/bitshifter/glam-rs) 0.33, [mint](https://github.com/kvark/mint) |
| 异步 | [tokio](https://tokio.rs) (full), [crossbeam-channel](https://docs.rs/crossbeam-channel) |
| 序列化 | [rkyv](https://github.com/rkyv/rkyv) (零拷贝), [serde](https://serde.rs), [sonic-rs](https://github.com/cloudflare/sonic-rs) |
| 窗口管理 | [winit](https://github.com/rust-windowing/winit) 0.30 |

> 完整依赖列在 [`Cargo.toml`](KairosEngine/Cargo.toml)。

### 致谢 / 间接依赖

参见 [Thanks.md](Thanks.md) —— 感谢使 KairosEngine 成为可能的所有开源项目。

---

## 📁 资源目录

```
res/
├── models/        # GLTF 和序列化网格资产
├── textures/      # 纹理资产
├── materials/     # 材质定义
├── shaders/       # 着色器源码
└── audios/        # 音频文件
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
- 架构决策记录（ADR）位于 [`docs/adr/`](docs/adr/)
- AI 代理遵循 [`AGENTS.md`](AGENTS.md) 中的指引

### 使用的 AI 工具

KairosEngine 开发过程中使用的 AI 辅助工具：
- **DeepSeek**
- **Cursor**
- **GPT**
- **Kimi**
- **即梦**

---

## 📄 许可证

本项目的许可协议为 [MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE)，您可任选其一。

---

<div align="center">
  <sub>🦀 使用 Rust 构建</sub>
  <br>
  <img src="https://komarev.com/ghpvc/?username=WhitePetal" alt="访问计数">
</div>
