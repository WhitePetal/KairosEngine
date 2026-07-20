# Code Review — Model Inspector (feat: base model inspector)

**Commit**: `82cebb8`  
**Date**: 2026-07-20  
**Issues**: #13, #19, #20, #21, #22

---

## Standards （Smell Baseline）

> 无 `CODING_STANDARDS` 文件，仅基于 Fowler 代码异味基线评估。

### 1. Divergent Change — `inspector.rs`

**文件**: `kairos_editor/ui/inspector.rs`

`Inspector` trait 现在承担了两个不相关的责任：
- UI 布局（`draw`）
- 渲染图构建（`render_preview`）

修复方向：如果长期只有 `MeshInspector` 需要预览，考虑将 `render_preview` 拆为独立 trait（如 `Previewable`），避免污染其他 inspector。

---

### 2. Speculative Generality + Refused Bequest — `inspector.rs`

```rust
fn render_preview(&self) -> Vec<GraphicsCommand> {
    Vec::new()
}
```

10 个 `Inspector` 实现者中只有 `MeshInspector` 覆写了此方法，其余 9 个“拒绝继承”。同时将 `GraphicsCommand`（渲染层类型）引入纯 inspector trait 的 import scope。

修复方向：同上，拆为独立 trait 或至少在短期内评估是否其他 inspector 真的需要预览能力。

---

### 3. Mysterious Name — `mesh.rs`

**文件**: `kairos_editor/ui/inspector/mesh.rs`

```rust
bind_receiver: Option<tokio::sync::oneshot::Receiver<egui::TextureId>>,
```

`bind_receiver` 没有揭示承载内容——它是异步 attachment bind 返回的 egui texture ID 接收器。

修复方向：重命名为 `pending_texture_bind` 或 `texture_id_receiver`。

---

### 4. Message Chains — `inspector_window.rs`

```rust
self.model
    .selected
    .as_ref()
    .and_then(|info| info.inspector.render_preview().into_iter().next())
```

4 节链条穿透：`model → selected → inspector → render_preview()`。调用者不应需要了解内部结构。

修复方向：在 `InspectorWindowModel` 上封装方法 `render_preview_command()`。

---

## Spec （对照 Issues）

> 对照 #13, #19, #20, #21, #22

### (a) 缺失/不完整

1. **#20 — `Vec<GraphicsCommand>` vs `Option` 语义不匹配**

   `render_preview()` 返回 `Vec<GraphicsCommand>`，但 `InspectorWindow::render()` 只取 `.into_iter().next()`。若将来有 inspector 返回多条命令，其余会被静默丢弃。当前仅 MeshInspector 返回 1 条命令，暂无实际影响，但接口契约模糊。

   修复方向：要么统一为 `Option<GraphicsCommand>`，要么让 `InspectorWindow::render()` 返回多个命令。

---

### (b) 范围蔓延

无。

---

### (c) 实现可能有误

1. **#21 — Mesh 未加载时仍创建 render pass（无效 GPU 开销）**

   `render_preview()` 无条件创建 color + depth attachment、begin render pass、发出 `draw()`——即使 mesh 尚未加载（VP 退化为 `float4x4::IDENTITY`，draw handle 可能未就绪）。每帧都产生无意义的 GPU 工作直到资源加载完成。

   修复方向：在 `mesh.lock().is_none()` 时跳过整个渲染 pass 的创建；或至少 skip draw，只保留 attachment + clear（保持 egui bind 流程不断）。

2. **#21 — `render_preview()` 重复获取 preview 锁**

   preview 锁在获取 `(drop_id, bind_ready, size)` 后释放，随后又单独获取一次只为读取 `(yaw, pitch, zoom)`。第一次锁内即可读完。

   修复方向：将 yaw/pitch/zoom 的读取合并到第一次锁作用域内。

3. **#22 — Receiver 轮询模式偏离 SceneWindow/GameWindow**

   SceneWindow/GameWindow 在 `render()` 中通过 `messager.send(Message::TryReceTextureId)` 发出消息，receiver 在 `Context::handle()` 中轮询。MeshInspector 改为在 `draw_preview()`（`ui()` 阶段）直接轮询。

   当前帧序下可行（`render_ui()` → `draw_ui()`），但如果同帧内 `draw()` 在 render 完成后被 re-invoke，`try_recv()` 可能误触发。

   修复方向：统一使用消息机制（`Message::MeshPreviewTryReceTextureId`）或显式记录此偏差为有意设计。

---

## 修复优先级建议

| 优先级 | 问题 | 影响 |
|--------|------|------|
| 🔴 高 | Spec #21 空 render pass | 每帧浪费 GPU 资源 |
| 🟡 中 | Spec #21 重复锁 | 微小性能损耗 |
| 🟡 中 | Standards #1 Divergent Change | 架构债务 |
| 🟢 低 | Standards #3 Mysterious Name | 可读性 |
| 🟢 低 | Standards #4 Message Chains | 封装性 |
| 🟢 低 | Spec #1 Vec vs Option | 接口清晰度 |
| 🟢 低 | Spec #22 receiver 模式 | 一致性 |
| 🟢 低 | Standards #2 Refused Bequest | 与 #1 同源 |
