# wgpu 学习路线

> 来源：AI 辅助整理（2026-05-21）  
> 状态：学习参考，非实现规范  
> 受众：熟悉 OpenGL、了解 DX12、略懂 Vulkan，在 Unity 中搭建过 SRP 自定义渲染管线的开发者  
> 项目上下文：KairosEngine 使用 **wgpu 27** + **winit 0.30** + **egui-wgpu**，见 `src/kairos_editor/runtime.rs`

## 1. 为什么这条路线适合你

wgpu 是 **WebGPU 规范的 Rust 安全封装**，底层可走 Vulkan / DX12 / Metal 等。你的经验可以概括为：

- **Vulkan / DX12**：显式资源、命令录制、Render Pass → 与 wgpu 高度对应  
- **Unity SRP**：Pass、RenderTarget、管线状态对象 → 概念上类似，但 wgpu 更底层、无 `ScriptableRenderContext`  
- **OpenGL**：需放弃「全局状态 + 随时改 shader/混合」的习惯，转向 **不可变 Pipeline + BindGroup**

主要新负担：**Rust 所有权/生命周期**、**异步初始化**（`request_adapter` / `request_device`），以及 **WGSL** 着色器语言。

## 2. 概念对照表

| 你熟悉的 | wgpu / WebGPU |
|---------|----------------|
| GL 全局状态、`glUseProgram` | 无全局状态；`RenderPipeline` + `BindGroup` |
| `GLuint` 纹理 + `glActiveTexture` | `Texture` + `TextureView` + `BindGroupLayout` |
| VBO/VAO + `glVertexAttribPointer` | `Buffer` + `VertexBufferLayout`（定义在 pipeline 中） |
| FBO / Render Target | `Texture`（`RENDER_ATTACHMENT` 用途）+ `TextureView` |
| `glDraw*` | `render_pass.draw` / `draw_indexed` |
| Vulkan `VkCommandBuffer` | `CommandEncoder` → `CommandBuffer` |
| Vulkan `RenderPass` + `Framebuffer` | `RenderPassDescriptor` + attachment 的 `view` |
| DX12 Root Signature | `BindGroupLayout` + `PipelineLayout` |
| Unity SRP `ScriptableRenderPass` | 自行组织 `RenderPassDescriptor` + pipeline |
| 命令提交 | `queue.submit` + surface `present` |

**与 OpenGL 的最大习惯转变**：混合、深度、顶点格式、shader 入口等全部锁在 **`RenderPipeline`** 内；改状态 = 新建或切换 pipeline（类似 DX12/Vulkan）。

**与 Vulkan 的差别**：同步、barrier、image layout 多由 wgpu 处理；需主动关心 **资源生命周期** 和 **async 设备创建**。

## 3. 与本仓库的对应关系

当前编辑器 runtime 已是标准交换链帧循环，路径：`src/kairos_editor/runtime.rs`。

初始化链路（与 Vulkan swapchain 设置类似）：

```text
Instance → Surface → Adapter → Device/Queue → surface.configure
```

单帧链路：

```text
get_current_texture → TextureView
  → CommandEncoder
  → RenderPass（清屏 + egui 绘制）
  → queue.submit → present
```

相关接力文档：

- [runtime-code-walkthrough.md](./runtime-code-walkthrough.md) — 自有 runtime 逐段走读  
- [scene-window-wgpu-integration.md](./scene-window-wgpu-integration.md) — Scene 视口接入 wgpu 的架构取舍  
- [eframe-runtime-migration-notes.md](./eframe-runtime-migration-notes.md) — 从 eframe 迁到 winit+wgpu 的笔记  

## 4. 分阶段学习路径

### 阶段 1：完整教程（约 1–2 周）

**目标**：独立写出「三角形 → 纹理 → 深度 → 多 Pass」，不先啃引擎架构。

| 优先级 | 资源 | 说明 |
|--------|------|------|
| 1 | [Learn Wgpu](https://sotrh.github.io/learn-wgpu/) | 首选；winit + wgpu，与项目栈一致 |
| 2 | [wgpu examples](https://github.com/gfx-rs/wgpu/tree/trunk/examples) | 官方示例；注意与 **wgpu 27** 同 major 版本对照 |
| 3 | [docs.rs/wgpu](https://docs.rs/wgpu/) | API 字典 |
| 4 | [WebGPU 规范](https://www.w3.org/TR/webgpu/) | 理解 BindGroup、Pass 等命名与语义 |

中文辅助（非官方，作对照）：[learn-wgpu-zh-CN](https://github.com/insthync/learn-wgpu-zh-CN)。

### 阶段 2：对照 KairosEngine（约数天）

在 **不拆掉 egui** 的前提下读懂现有帧循环，建议动手验证：

1. 修改 `GpuState::paint` 中 `LoadOp::Clear` 颜色，确认控制的是 **back buffer**  
2. 跟踪窗口 resize 时的 `surface.configure`（`GpuState::resize`）  
3. 理解 `SurfaceError::Lost` / `Outdated`（类似 Vulkan `OUT_OF_DATE`）  
4. 阅读 **egui-wgpu** 如何创建 pipeline、处理 `TexturesDelta`（UI 作为一个 Render Pass）

### 阶段 3：自建最小渲染管线（约 2–4 周）

按 Unity SRP 思路在 Rust 中自建模块（无内置 `ScriptableRenderContext`）：

| 模块 | 内容 |
|------|------|
| RHI 薄封装 | Device/Queue、Buffer/Texture 创建、staging upload |
| Shader | `wgpu::ShaderModule`（WGSL） |
| 材质 / PSO | `RenderPipeline` + `BindGroup` |
| Pass 图 | Color/Depth RT → 多个 `RenderPass` → composite 到 swapchain |
| 每帧录制 | `CommandEncoder`；uniform 用 `Buffer` + `queue.write_buffer` |

选读参考（不必通读）：

- wgpu 仓库：`render_bundle`、`boids`、`water` 等示例  
- Bevy `bevy_render`：工业级 Pass 图（按关键词 `RenderGraph` / `Phase` 检索阅读）  
- [Roguelike Rust](https://bfnightly.bracketproductions.com/) 后半 wgpu 章节 — 游戏向  
- Ray Tracing in One Weekend 的 Rust/wgpu 移植 — 练 buffer/compute 时可选  

### 阶段 4：WGSL 与调试（持续）

- [WGSL 规范](https://www.w3.org/TR/WGSL/)  
- 旧资产迁移： [Naga](https://github.com/gfx-rs/naga)（wgpu 生态的 shader 转换）  
- 调试：**RenderDoc** 抓 Vulkan/DX12 帧；环境变量与日志见 [wgpu wiki](https://github.com/gfx-rs/wgpu/wiki)  
- 能力上限：以 `adapter.limits()` / `device.limits()` 为准，勿假设 GL 式宽松限制  

**不建议作为第一条线**：直接通读 wgpu-hal 全源码；写过几个 Pass 后再看 hal 更轻松。

## 5. 精选资料清单

| 类型 | 资源 |
|------|------|
| 系统教程 | [Learn Wgpu](https://sotrh.github.io/learn-wgpu/) |
| 官方示例 | [gfx-rs/wgpu examples](https://github.com/gfx-rs/wgpu/tree/trunk/examples) |
| API 参考 | [docs.rs/wgpu](https://docs.rs/wgpu/) |
| 概念规范 | [WebGPU](https://www.w3.org/TR/webgpu/) / [WGSL](https://www.w3.org/TR/WGSL/) |
| 项目 Wiki | [wgpu wiki](https://github.com/gfx-rs/wgpu/wiki) |
| 社区 | Reddit r/rust_gamedev；中文可参考 learn-wgpu 翻译仓库 |

## 6. 实践建议（结合 OpenGL / SRP 背景）

1. **用 Vulkan 心智，不用 GL 心智**  
   Pipeline 不可变、descriptor = BindGroup、一帧一个 encoder。与 SRP 里 Pass 内状态固化类似。

2. **新代码优先 WGSL**  
   KairosEngine 新 shader 建议直接 WGSL；旧 HLSL/GLSL 再走路径导入。

3. **在现有工程小步扩展**  
   例如在 egui Pass **之前** 增加一个全屏三角形 Pass，或离屏 `Texture` 再 blit 到 swapchain，比空项目更接近未来引擎管线。

4. **版本对齐**  
   教程与示例尽量与 `Cargo.toml` 中 `wgpu = "27.0"` 同代；API 跨 major 可能有破坏性变更。

## 7. Unity SRP 与 wgpu 类比

| Unity SRP | wgpu 中对应 |
|-----------|-------------|
| `RenderTexture` | `Texture` + `RENDER_ATTACHMENT` |
| `ScriptableRenderPass.ConfigureTarget` | `RenderPassDescriptor` 中的 `view` |
| `ScriptableRenderPass.Execute` | `begin_render_pass` + `set_pipeline` + `draw` |
| `Shader` + `Material` | `ShaderModule` + `RenderPipeline` + `BindGroup` |
| 内置 `CommandBuffer` | 自管 `CommandEncoder` + `queue.submit` |
| Renderer Feature | 多 Pass / 多 encoder 或 `RenderBundle` |

## 8. 在 KairosEngine 中的下一步（可选实践）

从当前 egui 帧循环扩展到第一个自定义 Mesh Pass 的推荐顺序：

1. 新建 `RenderPipeline` + WGSL（顶点色或单 uniform MVP）  
2. 上传顶点 `Buffer`，在 `GpuState::paint` 里于 egui **之前** 增加一次 `RenderPass`  
3. 将 Scene 视口改为渲染到离屏 `Texture`，再在 ImGui/egui 中显示（见 [scene-window-wgpu-integration.md](./scene-window-wgpu-integration.md)）  
4. 引入深度缓冲与 resize 时 RT 重建  
5. 抽象 Pass 列表（向 SRP 式 Render Graph 演进）

实现时以仓库实际代码为准；本文档仅作学习路径索引。
