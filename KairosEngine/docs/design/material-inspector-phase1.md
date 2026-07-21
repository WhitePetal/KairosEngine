# Material Inspector 设计案 — Phase 1：需求概述

## 1.1 背景

Material Inspector 是 Kairos Editor 的 Inspector 系统的一员。当用户在项目树中点击 `.mat` 文件时，`InspectorCreater` 当前以 `todo!()` 占位，需要实现完整的 Material Inspector。

## 1.2 目标

让用户可以在 Material Inspector 中查看和编辑材质的 Shader、纹理和渲染状态参数，并提供 3D 预览功能。

## 1.3 用户交互方式

| 字段 | 交互方式 |
|---|---|
| Shader 路径 | 下拉栏，列出项目目录下所有 `.wgsl` 文件，默认选中 `res/shaders/shader.wgsl`。下拉列表在 Inspector 打开时通过 ProjectPathGraph 查询一次并缓存 |
| Texture 路径 | 拖拽交互，用户从项目窗口将 `.texture` 节点拖入 Inspector 上的目标框。支持点击清除按钮清空纹理 |
| Render State | 使用对应 egui 组件（ComboBox 等） |

## 1.4 数据模型

- Inspector 不创建额外的 `MaterialExt` 编辑端运行时资源
- 直接持有 `SerializedMaterial` + 通过 `AssetsServer` 查询运行时句柄

## 1.5 UI 布局

自上而下：

1. **Source 路径**：显示当前 `.mat` 文件路径
2. **Shader**：下拉栏选择
3. **Texture**：拖拽目标框
4. **Render State**：各字段的 egui 控件
5. **Apply 按钮**（支持 Ctrl+S）
6. **3D Preview**：可 orbit/zoom 预览

## 1.6 3D 预览

- 使用 Style TOML 配置一组可用预览网格路径，默认只配置 `Suzanne.mesh`
- Preview 上方添加下拉栏，用户可在配置的网格中切换
- 支持 Orbit（拖拽旋转）和 Zoom（滚轮）
- 使用当前材质的运行时数据渲染模型
- Preview 的渲染实现参考 MeshInspector（创建临时 attachment + 渲染通道 + bind_attachment_to_egui）

## 1.7 无效路径的降级策略

- **Shader**：项目级 `error_shader.wgsl`，输出纯紫色（类似 Unity Missing Shader）
- **Texture**：项目级 `white.texture`，2×2 纯白纹理

当 Inspector 设置的 shader_path 或 texture_path 对应文件不存在时，使用上述降级资源替换运行时材质中的句柄，保证渲染和预览不会崩溃。

> 注意：这两个降级资源需要随项目一同创建并提交到版本控制。

## 1.8 编辑与保存策略

**编辑即生效**：用户在 Inspector 上修改任何字段时，立即通过 `Assets::insert()` 原地更新资源系统中的运行时 `Material`。这样 Preview 在下一次渲染帧就能看到新效果，无须等待 Apply。

**Apply 显式保存**：点击 Apply 按钮（或 Ctrl+S）时将 `SerializedMaterial` 写回 `.mat` TOML 文件到磁盘。

**未保存修改检测**：使用 `dirty: Cell<bool>` 标记，关闭 Inspector 时如有未 Apply 的修改，弹出确认对话框（同 TextureInspector 模式）。

**保存反馈**：静默处理（同现有 Script/Texture Inspector），保存失败时打 log。

实现可行性通过代码调研确认：
- `Assets::insert()` 在 `path` 已存在时，会用新值替换旧值并返回同一个 `Arc<AssetHandle>`，所有引用方立即看到新数据
- Shader/Texture Handle 的加载是异步的（通过 `assets_server.load()`），因此编辑 shader 时 Preview 会有一个短暂的 "Loading shader…" 过渡

## 1.9 范围

**本次实现范围**：覆盖 `SerializedMaterial` 现有字段（shader_path, texture_path, render_state）。
**注意**：后期大概率会扩展甚至重构 Material，因此设计需保持可扩展性，不做超前设计。

## 1.10 风险/注意点

- Texture 拖拽输入是新的交互模式，代码库中暂无现成的"从项目树拖拽到 Inspector"的实现
- `.wgsl` 下拉栏需要扫描整个项目目录，需考虑性能（通过 ProjectPathGraph 查询）
- Preview 需要渲染通道支持（参考 MeshInspector 的 `render()` 方法 + `SceneCamera`）
