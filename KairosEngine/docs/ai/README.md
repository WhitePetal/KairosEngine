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
