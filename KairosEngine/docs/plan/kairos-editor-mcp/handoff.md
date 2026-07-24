# Handoff: kairos_editor_mcp — Wayfinder 完成，实施就绪

**日期**: 2026-07-24  
**本次会话**: 完成 Wayfinder 规划 + To-Spec + To-Tickets  
**下一会话**: 实施 T1–T10

---

## 本次产出总览

为 `kairos_editor_mcp` 完成了完整的四阶段规划设计，将所有决策转化为可执行 ticket。

### 设计文档（`docs/plan/kairos-editor-mcp/`）

| 文档 | 内容 |
|------|------|
| `phase1-requirements.md` | 6 大使用场景（开发验证、回归、交互测试、探索、调试、取代 TOML 测试） |
| `phase2-tools-catalog.md` | 38 MCP 工具/资源完整规格（P0 20 个 + P1 10 个 + P2 8 个），含 JSON Schema |
| `phase2-observability.md` | 0 新工具；stderr 日志 + MCP notification 告警 |
| `phase3-middleware-architecture.md` | QueryHandler trait + EditorChannelPlugin 双层架构设计 |
| `phase3-event-loop-integration.md` | 帧边界语义 + 超时策略 + wait_for 扩展 |
| `phase4-work-schedule.md` | 56 任务，5 Milestone，20-28 人天估算 |

### 调研（`docs/research/`）

| 文档 | 内容 |
|------|------|
| `mcp-protocol.md` | MCP 协议深度分析（JSON-RPC 2.0、rmcp SDK、完整工具调用流程） |
| `egui-mcp-analysis.md` | egui_mcp (rerun-io/kittest_inspector) 源码分析 + 集成建议 |

### 原型

- `prototypes/kairos-editor-mcp-middleware/` — 可编译原型，6/6 测试通过。包含 `EditorQuery`/`EditorResponse`/`EditorCommand` 类型、`QueryHandler` trait、`EditorChannel`/`EditorChannelPlugin`。

### Spec + Tickets

- **[PRD #82]** — 40 条 user stories，`ready-for-agent`
- **[T1–T10]** — 10 个 tracer-bullet ticket，全部 `ready-for-agent`

---

## 核心架构决策速查

| 决策 | 结论 |
|------|------|
| **架构** | Design A：复用 egui_mcp UiServer + 独立 EditorTools |
| **传输** | 独立进程 TCP + 编辑器内部 mpsc/oneshot |
| **ECS 暴露** | `QueryHandler` trait + Snapshot 序列化（无锁，拒绝 `Arc<RwLock<World>>`） |
| **Drawer 扩展** | `Drawer::as_query_handler()` 默认返回 `None`，向后兼容 |
| **Render loop** | Commands 在 `draw_ui()` 前，Queries 在后 |
| **操作日志** | stderr |
| **超时** | wait 工具自带 `timeout_secs`（对齐 egui_mcp） |
| **告警** | crash/panic → MCP notification；其他 → `get_console_logs` 轮询 |
| **egui 版本** | 0.35.0（KairosEngine 和 egui_mcp 兼容） |

---

## Ticket 依赖图

```
T1 (#86) ──► T2 (#87) ──► T3 (#93) ─┬── T4 (#92) ─┬── T9 (#94) ── T10 (#88)
                 │                    ├── T5 (#90) ─┤
                 │                    └── T6 (#89) ─┘
                 └── T7 (#95) ── T8 (#91)
```

| # | 一句话 | 前沿 |
|---|--------|------|
| T1 | 启用 egui_inspection，验证 egui-mcp 可连接 | 🔥 |
| T2 | 中间件类型 + 通道 + render loop hook + mock 测试 | 等 T1 |
| T3 | 项目树浏览：get_project_tree 等 4 个工具 | 等 T2 |
| T4 | 资产 CRUD：create/delete/rename/select/open | 等 T3 |
| T5 | Inspector + 控制台 + 编辑器状态 + crate 骨架 + lifecycle | 等 T3 |
| T6 | 场景相机：get/camera_orbit/zoom/screenshot | 等 T3 |
| T7 | cargo_build/run + wait_for_console + stderr 日志 | 等 T1+T5 |
| T8 | crash notification + error 加固 | 等 T2+T7 |
| T9 | 游戏视口、资产保存、Tab 管理（11 P1 工具） | 等 T4+T5+T6 |
| T10 | 偏好设置、音频、材质、batch + 集成测试 | 等 T9 |

---

## 实施提示

1. **T1 先行**：只加依赖 + 注册 plugin，改动量最小
2. **T2 用原型代码**：`prototypes/kairos-editor-mcp-middleware/` 中的类型和测试可直接移植
3. **T3–T6 可并行**：T2 完成后，4 个 ticket 修改不同 Drawer，无冲突
4. **T7 中的 cargo 工具可提前**：`cargo_build`/`cargo_run` 只依赖 T1，不依赖中间件

---

## Suggested Skills

- **`/implement`** — 逐个实施 T1→T10
- **`kairos-test`** — T2/T3 完成后为 QueryHandler dispatch 写单元测试
- **`/prototype`** — 不确定的实现细节时快速原型验证
