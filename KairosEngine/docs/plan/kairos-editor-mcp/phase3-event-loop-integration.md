# Phase 3（补充）: 事件循环挂接设计

**日期**: 2026-07-24  
**父Ticket**: [#71](https://github.com/WhitePetal/KairosEngine/issues/71)  
**关联Ticket**: [D6](https://github.com/WhitePetal/KairosEngine/issues/74)

---

## 1. 核心结论

D3 中间层架构已经完整设计了事件循环集成方案。D6 的产出是对时序语义、超时策略、`wait_for` 扩展的**细化补充**。

---

## 2. 事件循环集成（D3 回顾）

D3 设计了两个 hook 点：

```rust
self.egui_ctx.run_ui(raw_input, |ui| {
    //    ^^^^^^^^ 一帧边界 ^^^^^^^^

    // === 1. Pre-frame: process MCP commands (mutations) ===
    self.editor_channel.process_commands(&mut self.kairos_engine);

    // --- egui 渲染 ---
    graphics_commands.append(&mut self.kairos_engine.render_ui());
    self.kairos_engine.handle_ui(ui);
    self.kairos_engine.draw_ui(ui);

    // === 2. Post-frame: process MCP queries (reads latest state) ===
    self.editor_channel.process_queries(&self.kairos_engine);
});
```

**时序图**：

```
Agent                    MCP Server               Editor Render Loop
─────                    ──────────               ──────────────────
│                         │                        │
├─ create_asset() ──────► │                        │
│                         ├─ EditorCommand ───────► │  (mpsc send)
│   [await oneshot]       │                        ├─ process_commands()
│                         │                        │   dispatch_command()
│                         │◄─ Ok ───────────────── │  (oneshot reply)
│◄── Ok ──────────────── │                        │
│                         │                        │
├─ get_project_tree() ──► │                        │
│                         ├─ EditorQuery ─────────► │  (mpsc send)
│   [await oneshot]       │                        ├─ draw_ui()
│                         │                        ├─ process_queries()
│                         │                        │   dispatch_query()
│                         │◄─ ProjectTree ──────── │  (oneshot reply)
│◄── ProjectTree ─────── │                        │
│                         │                        │
├─ click("Save") ────────►│                        │  (next frame)
│                         ├─ ApplyEvents ─────────► │  (egui_mcp Bridge)
│   [await Bridge]        │                        ├─ on_events → inject
│                         │◄─ Done ─────────────── │
│◄── ok ───────────────── │                        │
```

---

## 3. 帧边界语义

### 3.1 一帧内处理多个消息

`process_commands` 使用 `while let Ok(msg) = try_recv()`，一帧内消费队列中所有积压消息：

```rust
pub fn process_commands(&mut self, engine: &mut Engine, log: &mut Log) {
    while let Ok(msg) = self.receiver.try_recv() {
        match msg {
            ChannelMessage::Command(cmd, reply) => {
                dispatch_command(cmd, engine, log);
                let _ = reply.send(EditorResponse::Ok);
            }
            ChannelMessage::Query(query, reply) => {
                let resp = dispatch_query(&query, engine, log);
                let _ = reply.send(resp);
            }
        }
    }
}
```

### 3.2 三种时序模式

| 模式 | Agent 行为 | 帧分配 | 延迟 |
|------|-----------|--------|------|
| **逐操作** | `await click("A"); await click("B"); await click("C");` | 3 帧（每 await 等一帧） | 3×16ms |
| **批量** | `batch([click("A"), click("B"), click("C")])` | 1 帧（egui_mcp 打包所有事件） | 1×16ms |
| **无 await 连续发** | `send click("A"); send click("B"); send click("C"); await ...` | 取决于时序——如果 3 个都在同一帧的 `try_recv` 前到达 → 1 帧；否则 → N 帧 | 不确定 |

**推荐 Agent 行为**：关键路径上逐 `await` 确保确定性；性能敏感场景用 `batch`。

### 3.3 命令的帧内顺序

由于 `process_commands` 在 `draw_ui()` **之前**执行，命令修改的状态**本帧渲染可见**。
查询在 `draw_ui()` **之后**执行，返回的是**最新渲染后**的状态。

这确保了 "click → 下一帧 query_tree 看到变化" 的语义与 egui_mcp 一致。

---

## 4. 超时策略

### 4.1 设计：与 egui_mcp 保持一致

egui_mcp 的 `wait_for` 使用 `timeout_secs` 参数。kairos_editor_mcp 的工具也采用相同模式：

```rust
// egui_mcp 模式
wait_for { query: {...}, timeout_secs: 5.0 }

// kairos_editor_mcp 模式（编辑器 wait 工具）
wait_for_console { pattern: "Saved", timeout_secs: 5.0 }
wait_for_asset { guid: "...", timeout_secs: 10.0 }
```

### 4.2 非 wait 工具的超时

对于普通工具（`get_project_tree`、`create_asset` 等），超时由底层机制处理：

| 场景 | 机制 | 行为 |
|------|------|------|
| **编辑器正常运行** | oneshot 在 1 帧内返回（~16ms） | 无超时问题 |
| **编辑器死循环** | oneshot 永不返回 → Agent 客户端超时 | Agent 的 MCP Client（Claude/Codex）通常有内置超时 |
| **编辑器崩溃** | mpsc channel drop → oneshot 返回 error | `channel.query()` 返回 `EditorResponse::Error("handler dropped")` |
| **MCP Server 自身** | `rmcp` 有请求超时机制 | 默认约 60s，可配置 |

**结论**：不为普通工具单独加超时参数。wait 类工具（需要轮询等待条件满足的）加 `timeout_secs`，对齐 egui_mcp 惯例。

---

## 5. wait_for 扩展

### 5.1 设计原则

egui_mcp 的 `wait_for` 轮询 AccessKit 树直到条件满足，在 **MCP Server 内部循环**（避免 Agent 多次 round-trip）：

```
Agent                     MCP Server                     Editor
─────                     ──────────                     ──────
├─ wait_for_console() ──► │                              │
│                          ├─ loop:                       │
│                          │   get_console_logs() ──────► │
│                          │◄── logs ──────────────────── │
│                          │   if match → break           │
│                          │   else → sleep 100ms         │
│                          │   if timeout → error         │
│◄── matched entry ────── │                              │
```

**价值**：1 次 MCP round-trip vs Agent 手动轮询的 N 次 round-trip。

### 5.2 新增工具

#### `wait_for_console`

等待控制台出现匹配指定模式的日志条目。

| 参数 | 类型 | 说明 |
|------|------|------|
| `pattern` | `string` | 在日志 message 中搜索的子串（大小写不敏感） |
| `level` | `string?` | 可选：只检查特定级别（"info"、"warn"、"error"） |
| `timeout_secs` | `number?` | 超时秒数（默认 5.0，最大 30.0） |
| `poll_interval_ms` | `number?` | 轮询间隔毫秒（默认 100，最小 16） |

**返回**：
```json
{
  "ok": true,
  "matched": {
    "level": "info",
    "message": "Asset saved successfully",
    "timestamp": 1234567890,
    "caller": "kairos_editor::serialize_asset"
  },
  "attempts": 3,
  "elapsed_ms": 312.5
}
```

**实现**：
```rust
async fn wait_for_console(
    channel: &EditorChannel,
    pattern: &str,
    level: Option<&str>,
    timeout: Duration,
    poll_interval: Duration,
) -> EditorResponse {
    let deadline = Instant::now() + timeout;
    let mut after: Option<u64> = None;
    let mut attempts = 0;

    loop {
        attempts += 1;
        let resp = channel.query(EditorQuery::GetConsoleLogs {
            level: level.map(|s| s.to_string()),
            pattern: Some(pattern.to_string()),
            after,
            limit: Some(1),
        }).await;

        if let EditorResponse::ConsoleLogs { entries, .. } = &resp {
            if let Some(entry) = entries.first() {
                return EditorResponse::WaitForMatched {
                    entry: Box::new(entry.clone()),
                    attempts,
                    elapsed_ms: deadline.saturating_duration_since(deadline - timeout).as_millis() as u64,
                };
            }
        }

        if Instant::now() > deadline {
            return EditorResponse::Error(format!(
                "wait_for_console timed out after {}ms ({} attempts)",
                timeout.as_millis(), attempts
            ));
        }

        tokio::time::sleep(poll_interval).await;
    }
}
```

#### `wait_for_asset`

等待资产加载完成。

| 参数 | 类型 | 说明 |
|------|------|------|
| `guid` | `string` | 资产 GUID |
| `timeout_secs` | `number?` | 超时秒数（默认 10.0） |

**返回**：`{ ok, asset: AssetInfo, attempts, elapsed_ms }`

#### `wait_for_state_change`

通用的编辑器状态变化等待。

| 参数 | 类型 | 说明 |
|------|------|------|
| `query` | `string` | 要轮询的查询类型（"inspector"、"selection"、"project_tree"） |
| `condition` | `object` | 条件描述（查询特定时：`{ field: "selected.kind", equals: "Material" }`） |
| `timeout_secs` | `number?` | 超时秒数（默认 5.0） |

**返回**：`{ ok, snapshot, attempts, elapsed_ms }`

---

## 6. 编辑器重启与重连

D1 要求 Agent 能触发 `cargo_build` + `cargo_run`。编辑器重启后需要重新 `attach`。

**流程**：

```
Agent                           MCP Server
─────                           ──────────
├─ cargo_run() ───────────────► │ 重启编辑器进程
│◄── ok ──────────────────────  │
│                                │
│   [编辑器启动中...]             │  TCP :5719 就绪
│                                │
├─ attach() ──────────────────► │  connect TCP
│◄── attached ────────────────  │
│                                │
│   [恢复正常操作]                │
```

**职责划分**：
- `cargo_run` 工具负责杀掉旧进程 + 启动新进程 + 等待 TCP 端口就绪
- Agent 负责在 `cargo_run` 后主动调用 `attach`
- MCP Server 自身不自动重连（Agent 全流程自驱）

---

## 7. 设计决策总表

| 决策 | 结论 | 来源 |
|------|------|------|
| 请求模型 | mpsc + oneshot await（与 egui_mcp 一致） | D3 |
| 帧内批量 | `try_recv` 循环消费所有积压消息 | D3 |
| Commands 时序 | draw_ui 前执行，本帧生效 | D3 |
| Queries 时序 | draw_ui 后执行，读最新状态 | D3 |
| 超时策略 | wait 工具自带 `timeout_secs`，普通工具无显式超时（依赖底层机制） | D6 |
| 轮询间隔 | 100ms 默认（与 egui_mcp 一致），最小 16ms（单帧） | D6 |
| wait 工具实现 | MCP Server 内部循环，避免 Agent 多次 round-trip | D6 |
| 编辑器重启 | Agent 手动 `cargo_run` → `attach`，MCP 不自动重连 | D1 + D6 |

---

## 8. 对 D2 工具清单的补充

新增工具：

| 工具 | 优先级 | 归类 |
|------|--------|------|
| `wait_for_console` | P0 (MVP) | 等待类 |
| `wait_for_asset` | P1 (Enhancement) | 等待类 |
| `wait_for_state_change` | P2 (Complete) | 等待类 |
