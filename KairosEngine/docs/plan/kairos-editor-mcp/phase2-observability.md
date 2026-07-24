# Phase 2（补充）: Agent 可观测性设计

**日期**: 2026-07-24  
**父Ticket**: [#71](https://github.com/WhitePetal/KairosEngine/issues/71)  
**关联Ticket**: [D4](https://github.com/WhitePetal/KairosEngine/issues/72)

---

## 1. 核心发现

经过 Grilling 确认，**大部分可观测性需求已由 egui_mcp + D2 工具清单覆盖**，无需新增 MCP Tools。D4 的产出主要是**设计决策**而非功能清单。

---

## 2. 可观测性全景

| 维度 | 方案 | 新工具？ | 依赖 |
|------|------|---------|------|
| **Widget 定位** | egui_mcp `query_tree` 返回的 `NodeView`（id/role/label/value/bounds/focused/disabled/hidden/parent_id） | ❌ 无需 | egui_mcp 已有 |
| **布局验证** | Agent 调用两次 `query_tree` → 自己 JSON diff。预期布局来自 Agent 对代码的理解，通过交互验证（click → 查 query_tree/logs 确认生效） | ❌ 无需 | Agent 能力 |
| **视觉验证** | `screenshot`（egui_mcp 已有，可选） | ❌ 无需 | egui_mcp 已有 |
| **日志注入** | Agent 修改源码插入 `log::info!()` → 编译运行 → `get_console_logs` 获取（D1 结论） | ❌ 无需 | D2 工具 |
| **操作追溯** | MCP Server 输出到 **stderr**：工具名 + 参数 + 结果 + 时间戳 | ❌ 无需 | stderr |
| **状态快照** | 完整 AccessKit 树（`query_tree` 不加过滤）；引擎内部状态通过 D2 查询工具按需获取 | ❌ 无需 | egui_mcp + D2 |
| **告警通知** | 混合：crash/panic → MCP notification 推送；layout overflow/GPU error → 轮询 `get_console_logs` | ⚠️ 需 notification 能力 | MCP 协议 |

---

## 3. 操作日志设计

### 3.1 问题

MCP 协议中 stdout 被 JSON-RPC 独占（Agent ↔ Server 通信通道），日志不能混入 stdout。

### 3.2 方案：stderr

MCP Server 的所有操作日志输出到 **stderr**，Agent 通过读取 stderr 获取操作追溯。

```
stdin  ←── JSON-RPC 请求（Agent → Server）
stdout ──→ JSON-RPC 响应（Server → Agent）
stderr ──→ 操作日志（Server → Agent 可读）
```

### 3.3 日志格式

基础信息（时间戳 + 工具名 + 参数摘要 + 结果）：

```
[2026-07-24T10:30:01.234Z] REQ  get_project_tree {}
[2026-07-24T10:30:01.245Z] RES  get_project_tree → ok (36 nodes)
[2026-07-24T10:30:02.010Z] REQ  click { target: { role: "Button", label_contains: "Save" } }
[2026-07-24T10:30:02.120Z] RES  click → ok (clicked_id: "42", pos: [120, 340])
[2026-07-24T10:30:03.000Z] REQ  get_console_logs { level: "info", limit: 10 }
[2026-07-24T10:30:03.005Z] RES  get_console_logs → ok (3 entries)
```

### 3.4 实现

在 `kairos_editor_mcp` 的 tool dispatch 层，每个工具调用前后自动输出到 `eprintln!()`。无需额外依赖。

```rust
fn log_tool_call(tool_name: &str, params: &str, result: &str, elapsed: Duration) {
    eprintln!(
        "[{}] REQ  {} {}",
        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        tool_name,
        params,
    );
    eprintln!(
        "[{}] RES  {} → {} ({:.3}ms)",
        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        tool_name,
        result,
        elapsed.as_secs_f64() * 1000.0,
    );
}
```

---

## 4. 告警通知设计

### 4.1 MCP Notification 机制

MCP 协议支持 Server → Client 的 **notification**（无需 Agent 请求的单向推送）：

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/message",
  "params": {
    "level": "error",
    "message": "Editor process crashed: SIGSEGV",
    "data": { "backtrace": "..." }
  }
}
```

`rmcp` SDK 支持通过 `Peer` 发送 notification。

### 4.2 推送告警（Push）

| 事件 | 检测方式 | Notification |
|------|---------|-------------|
| **编辑器崩溃** | TCP 连接断开（`Bridge` 检测） | `notifications/message { level: "error", message: "editor process exited" }` |
| **Editor panic** | `std::panic::set_hook` 在编辑器启动时注册 | 捕获 panic 信息 → 通过 channel 发送给 MCP Server → notification |
| **GPU 设备丢失** | `device.set_device_lost_callback`（已有） | 通过 Log → channel → notification |

### 4.3 轮询告警（Poll）

| 事件 | Agent 操作 | 依赖工具 |
|------|-----------|---------|
| **Layout overflow** | `get_console_logs { level: "warn", pattern: "overflow" }` | D2: `get_console_logs` |
| **GPU 错误** | `get_console_logs { level: "error", pattern: "GPU" }` | D2: `get_console_logs` |
| **Asset 加载失败** | `get_console_logs { level: "error" }` | D2: `get_console_logs` |

### 4.4 集成点

```
KairosEngine Editor          MCP Server              AI Agent
───────────────────          ──────────              ─────────
panic_hook ──channel──>  crash notification ──JSONRPC──> reads
device_lost ──Log──>     (via Peer)                   

layout_overflow ──Log──>                            <── get_console_logs
GPU_error ──Log──>                                  <── get_console_logs
```

---

## 5. 设计决策

| 决策 | 结论 | 理由 |
|------|------|------|
| 操作日志通道 | stderr | stdout 被 JSON-RPC 独占，stderr 是 MCP 惯例 |
| 日志格式 | 基础（时间戳 + 工具名 + 参数摘要 + 结果） | Agent 只需定位失败步骤，不需要帧级 detail |
| 告警模型 | 混合（push + poll） | 严重事件不能等 Agent 轮询；常规异常 agnet 自定节奏 |
| Notification vs 工具 | crash/panic 用 notification，其他用工具 | notification 设计为"紧急通知"语义，不承载数据查询 |
| 不需新增工具 | Widget 定位/布局验证/状态快照/日志注入 | 已有能力全覆盖 |

---

## 6. 对技术选型的影响

| 影响 | 说明 |
|------|------|
| MCP Server 需要持有 `Peer` 引用 | 用于发送 crash/panic notification |
| 编辑器需要注册 panic hook | 捕获 panic 信息并发送给 MCP Server |
| `KairosEditorRuntime` 已有 `device_lost_callback` | 需要扩展为通过 channel 通知 MCP Server |
| 不需要引入新的 MCP Tool | stderr 日志 + notification 都是协议层能力 |
| D2 工具清单无需修订 | 已有能力已覆盖全部可观测性需求 |
