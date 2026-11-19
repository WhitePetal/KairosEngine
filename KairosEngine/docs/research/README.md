# Research Notes

## Editor Architecture

| File | Topic |
|---|---|
| `hierarchy/unity-hierarchy.md` | Unity Hierarchy 窗口实现分析与架构研究 |
| `editor-ui-messager-mvc-audit.md` | **现状审计**：Messager/Message 总线与「window = model」结构的证据清单（57 变体、365 行 god handle、73 个发送点、0 个交互测试） |
| `editor-ui-architecture-patterns.md` | 成熟编辑器 UI 架构模式：Unreal Slate / Godot / Unity / VS Code / JetBrains / Zed / Blender / Rerun 的 command、selection、inspector、undo、layout、plugin |
| `rust-ui-architecture-survey.md` | Rust UI 架构调研：MVC→MVU 谱系、iced / egui+Rerun / Slint / Xilem / GPUI / signals / Bevy，8 个可迁移模式 |
| `editor-ui-engine-integration.md` | kairos_engine 帧循环、World 资源、线程/资产时序约束，以及对新 UI 架构的 must/should/must-not 集成要求 |
| `ui-widget-component-layer-survey.md` | 纯外观 widgets 与功能 components 的分层/契约/主题/测试调研：React/Flutter/Atomic Design + Rerun re_ui / Zed ui / egui Widget / egui_kittest |
| `editor-ui-widget-catalog.md` | 现状 UI 逐文件审计 + 25 个纯 widget 目录 + 22 个 component 目录 + Theme token 清单 + 三个代表文件 before/after |
| `color-picker-eyedropper/README.md` | **取色器（吸管）调研**：egui 无现成能力、wgpu 帧回读 vs 桌面截屏库（xcap/scap/windows-capture）对比、Unity/Blender/Godot 做法、HDR 与 DPI 约束、落点清单 |

## Texture Encoding

| File | Topic |
|---|---|
| `texture-encoding/astc-compression-crates.md` | 纯 Rust ASTC encoder/decoder 生态评估 |
| `texture-encoding/png-srgb-color-space.md` | PNG 色彩空间与 wgpu 的行为 |
| `texture-encoding/texture-compression-reference-impls.md` | C++ 纹理压缩参考实现对比 |
| `texture-encoding/wgpu-srgb-handling.md` | wgpu 中 sRGB 处理方式 |

## Terrain Systems

| File | Topic |
|---|---|
| `ue5-terrain-landscape-system.md` | UE5 地形/Landscape 系统深度分析 |
| `aaa-terrain-systems-analysis.md` | AAA 开放世界地形系统竞品分析（6款游戏） |

## Material System

| File | Topic |
|---|---|
| `material-dynamic-properties-competitor-analysis.md` | Unity/Unreal/Godot/Bevy 材质动态属性系统竞品分析 |

## Rendering

| File | Topic |
|---|---|
| `render-cache-defect-inventory.md` | **渲染模块缺陷清单**（资源变更下的缓存键 / 失效 / 引用 / 泄漏；渲染模块重构参考单） |
| `pipeline-cache-key-design.md` | UE / Unity / Godot / Bevy 的 PSO 缓存键设计竞品调研 |

## Animation / Motion Matching

| File | Topic |
|---|---|
| `motion-matching-libraries.md` | 开源 motion matching 实现调研（面向动画系统，0.3.0 排期） |

## Physics

| File | Topic |
|---|---|
| `physics-rapier-facts-inventory.md` | 现役 rapier3d 0.33 后端：生命周期、静态 collider、步进 dt 语义事实清点 |
| `avian-0.7.0/roadmap.md` | **Avian 0.7.0 学习与物理模块重建路线图**（总纲：骨架、里程碑、读码清单、验收、移植决策） |
| `avian-0.7.0/01-architecture-and-schedules.md` | Avian 插件与调度架构、固定步骨架、`physics_transform` |
| `avian-0.7.0/02-colliders.md` | Avian `Collider` 子系统：形状、必需组件、碰撞层、collider tree |
| `avian-0.7.0/03-rigid-bodies-and-dynamics.md` | Avian `RigidBody`、质量属性、力与重力、积分器、求解器、CCD |
| `avian-0.7.0/04-collision-detection.md` | Avian broad phase / narrow phase / contact types / 事件 / hooks / 空间查询 |
| `avian-0.7.0/05-bevy-dependency-surface.md` | Avian 对 Bevy 的依赖面清点 + 移植到 `kairos_ecs`/`kairos_math` 的可行性 |