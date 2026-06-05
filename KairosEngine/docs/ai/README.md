# AI 输出文档

本目录存放由 AI 辅助生成、经人工审阅的设计说明与方案草稿，**不属于可执行代码**，也不参与构建。

## 约定

- 单篇文档使用英文 kebab-case 文件名（如 `resource-manager-design.md`）。
- 文档正文使用简体中文，代码示例使用 Rust。
- 实现前请以仓库内实际代码与需求为准，本文档仅作设计参考。

## 索引

| 文档 | 说明 |
|------|------|
| [resource-manager-design.md](./resource-manager-design.md) | 资源管理器设计（TypeMap + Handle）及 `KairosEngine` → `ui::Context` 传递方案 |
| [scene-window-wgpu-integration.md](./scene-window-wgpu-integration.md) | SceneWindow 中接入 wgpu 渲染，以及保留或移除 `eframe` 的架构取舍 |
| [eframe-runtime-migration-notes.md](./eframe-runtime-migration-notes.md) | 剥离 `eframe`、迁移到 `winit + wgpu + egui-winit + egui-wgpu` 自有 runtime 的接力笔记 |
| [runtime-code-walkthrough.md](./runtime-code-walkthrough.md) | 自有 runtime 代码走读：`main`、`new`、`resumed/create_window`、`redraw` 的逐段讲解 |
| [wgpu-learning-roadmap.md](./wgpu-learning-roadmap.md) | wgpu 学习路线：概念对照、分阶段教程与资料、SRP 类比及与本项目 runtime 的衔接 |
| [wgpu-threading-render-graph-notes.md](./wgpu-threading-render-graph-notes.md) | wgpu 多线程渲染、Unity Main/Render Thread 类比、Render Thread 取舍与 Render Graph 设计笔记 |
| [rust-crate-facade-and-workspace-split.md](./rust-crate-facade-and-workspace-split.md) | 统一 `Color32` 门面、IDE 补全噪音原因、rust-analyzer 缓解与 Workspace 拆 crate 分阶段方案 |
| [texture-asset-toml-rkyv-hybrid-format.md](./texture-asset-toml-rkyv-hybrid-format.md) | `.texture` 资产 TOML + rkyv 混合格式：分隔符 split、memchr、预写 binary 偏移与加载方案 |
| [coordinate-system-matrix-conventions.md](./coordinate-system-matrix-conventions.md) | 坐标系选择、DX/wgpu 风格 M/V/P 矩阵、列向量乘法与 SIMD 矩阵乘实现笔记 |
| [graphics-graph-render-graph-conversation.md](./graphics-graph-render-graph-conversation.md) | GraphicsGraph / RenderGraph 渲染管线拆分设计讨论的完整对话记录，包含基于 `petgraph` 的编译、入口判定与执行顺序方案 |
| [render-pipeline-resource-cache-conversation.md](./render-pipeline-resource-cache-conversation.md) | `render_pipeline.rs` 当前渲染路径中 GPU 资源、pipeline 状态、render target 与每帧临时对象的缓存边界讨论 |
| [gltf-vertex-attribute-sets.md](./gltf-vertex-attribute-sets.md) | glTF 顶点颜色、UV 等属性集编号：`COLOR_n`、`TEXCOORD_n` 与 `gltf` reader 的 `set` 参数说明 |
| [ecs-learning-resources-conversation.md](./ecs-learning-resources-conversation.md) | ECS 学习资料、Flecs/Bevy/EnTT/Shipyard/hecs 参考路线，以及 Kairos 自研 ECS 的分阶段落地建议 |
