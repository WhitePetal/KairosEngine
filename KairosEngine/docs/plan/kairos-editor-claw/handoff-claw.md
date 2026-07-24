# Handoff: Kairos Editor Claw 设计阶段

> 从 2026-07-24 的设计对话中提取，供接手 agent 继续工作。

## 背景

KairosEngine 需要一个工具 `kairos_editor_claw`，类似 OpenClaw，作为后台守护进程将飞书消息路由到本机 Zed 的 AI Agent，实现远程控制开发。

## 当前状态

**设计阶段已完成**。Spec + 7 张 tracer-bullet 子任务票已发布到 GitHub。

### 关键产物

**Spec Issue**：[#77 — Spec: Kairos Editor Claw](https://github.com/WhitePetal/KairosEngine/issues/77) — `ready-for-agent`

**设计文档**（`docs/plan/kairos-editor-claw/`）：
- `README.md` — 主索引 + 决策速查
- `claw-phase1-requirements.md` — Phase 1：9 项架构决策
- `claw-phase2-features.md` — Phase 2：7 大功能域，45 个任务
- `claw-phase3-tech.md` — Phase 3：12 项技术选型，8 个坑点
- `claw-phase4-schedule.md` — Phase 4：8 周排期，4 个检查点

**调研文档**（`docs/research/`）：
- `claw-tools-architecture.md` — OpenClaw 架构深入分析
- `zed-ai-agent-integration.md` — Zed AI Agent 集成方案调研

**子任务票**（全部 `ready-for-agent`）：

| 票号 | 标题 | 阻塞于 |
|------|------|--------|
| [#78](https://github.com/WhitePetal/KairosEngine/issues/78) | T-01: Daemon 骨架 + 配置 | 无 — 可立即开始 |
| [#79](https://github.com/WhitePetal/KairosEngine/issues/79) | T-05: Zed Fork IPC 扩展 | 无 — 可立即开始 |
| [#80](https://github.com/WhitePetal/KairosEngine/issues/80) | T-02: 飞书 Ping-Pong | #78 |
| [#81](https://github.com/WhitePetal/KairosEngine/issues/81) | T-03: 会话路由 + 持久化 | #80 |
| [#83](https://github.com/WhitePetal/KairosEngine/issues/83) | T-04: ClaudeCode Backend | #81 |
| [#84](https://github.com/WhitePetal/KairosEngine/issues/84) | T-06: Zed Backend 集成 | #81 + #79 |
| [#85](https://github.com/WhitePetal/KairosEngine/issues/85) | T-07: 安全加固 + 生产守护 | #81 |

### 核心决策速查

| 决策 | 结论 |
|------|------|
| 语言 | Rust（tokio + axum + rusqlite） |
| 消息平台 | 飞书优先（Webhook + ngrok），微信后续 |
| Agent 后端 | 可插拔 trait，ZedBackend 为主 |
| Zed 集成 | Fork Zed 最小 IPC 扩展（~200 行） |
| 消息类型 | Phase 1 纯文本，枚举预留 Image/File |
| 安全 | 两级：读/写直接放行，危险操作确认 |
| 飞书接入 | Webhook only，无 Long Polling 降级 |
| 测试接缝 | `AgentBackend` trait 边界 |

### 架构图

```
Feishu (phone) → ngrok → axum HTTP (:8899) → FeishuBridge
                                                   ↓
                                             Message Router
                                             (SQLite session)
                                                   ↓
                                             AgentBackend trait
                                        ┌───────┴────────┐
                                   ZedBackend    ClaudeCodeBackend
                                   (IPC→Zed)     (subprocess)
```

## 下一步

领一张前沿票开始实现。两者互不阻塞，可并行：

- **[#78](https://github.com/WhitePetal/KairosEngine/issues/78)** — 创建 Cargo project + 配置 + daemon 骨架
- **[#79](https://github.com/WhitePetal/KairosEngine/issues/79)** — Fork Zed + IPC 扩展

## Suggested Skills

- `kairos-test` — 测试策略参考
- `codebase-design` — deep-module 接口设计
