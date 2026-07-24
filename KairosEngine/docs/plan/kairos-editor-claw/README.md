# Kairos Editor Claw 设计案

> 从 2026-07-24 的设计对话中产出。完整的设计文档、技术决策和功能排期。

## 设计文档

| 阶段 | 文件 | 内容 |
|------|------|------|
| Phase 1 | [claw-phase1-requirements.md](./claw-phase1-requirements.md) | 需求分析：7 项架构决策（D-001~D-007），5 条用户故事 |
| Phase 2 | [claw-phase2-features.md](./claw-phase2-features.md) | 功能拆分：7 大功能域（F0~F7），45 个任务 |
| Phase 3 | [claw-phase3-tech.md](./claw-phase3-tech.md) | 技术选型：12 项技术决策（T1~T12），8 个坑点 |
| Phase 4 | [claw-phase4-schedule.md](./claw-phase4-schedule.md) | 功能排期：8 周计划，4 个检查点 |

## 调研文档

| 文档 | 主题 |
|------|------|
| `docs/research/claw-tools-architecture.md` | OpenClaw 架构深入分析 |
| `docs/research/zed-ai-agent-integration.md` | Zed AI Agent 集成方案调研 |

## 核心决策速查

| 决策 | 结论 |
|------|------|
| **Zed 集成** | Fork Zed 最小 IPC 扩展（~200 行改动） |
| **Agent 后端** | 可插拔架构，ZedBackend 为主 + 配置可切换 |
| **实现语言** | Rust（`tokio` + `axum` + `rusqlite`） |
| **消息平台** | 飞书优先（Webhook + ngrok），微信后续迭代 |
| **用户模型** | 单用户 |
| **目标平台** | macOS + Windows 为主，Linux 支持 |
| **IPC 协议** | Unix Socket + JSON newline-delimited |
| **存储** | SQLite（WAL 模式）+ TOML 配置 |

## 8 周排期

```
Week 1-2: F0 Config + F1 Daemon (基础建设)
Week 1-3: F7 Zed Fork (并行)
Week 3-4: F2 Feishu Bridge
Week 5-6: F3 Message Router
Week 6-8: F4 Agent Backend + F5 Security
```

## 已确认决策（全部 9 项）

| # | 决策 | 结论 |
|---|------|------|
| D-001 | Zed 集成 | Fork Zed 最小 IPC 扩展 |
| D-002 | Agent 后端 | 可插拔，ZedBackend 为主 |
| D-003 | 实现语言 | Rust（tokio） |
| D-004 | 消息存储 | SQLite + TOML |
| D-005 | 消息平台 | 飞书优先，微信后续 |
| D-006 | 用户模型 | 单用户 |
| D-007 | 目标平台 | macOS + Windows 为主，Linux 支持 |
| D-008 | 消息类型 | Phase 1 纯文本，枚举预留接口 |
| D-009 | 安全审批 | 两级：读/写直接放行，危险操作确认 |

## 下一步

从 F0.1（创建 Cargo project）或 F7.1（Fork Zed）开始实现。
