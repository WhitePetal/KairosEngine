# Kairos Editor Claw 设计案 — Phase 4：功能排期

> **状态**：进行中
> **创建日期**：2026-07-24
> **前置文档**：[Phase 1 需求分析](./claw-phase1-requirements.md) | [Phase 2 功能分析](./claw-phase2-features.md) | [Phase 3 技术选型](./claw-phase3-tech.md)

## 4.1 排期总览

```
Week │  1  │  2  │  3  │  4  │  5  │  6  │  7  │  8  │
─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┤
F0   │█████│█████│     │     │     │     │     │     │  Config + Bootstrap
F1   │     │█████│█████│     │     │     │     │     │  Daemon Process
F6   │     │██   │██   │██   │██   │██   │██   │     │  Logging（持续）
─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┤
F7   │█████│█████│█████│     │     │     │     │     │  Zed Fork（并行）
─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┤
F2   │     │     │█████│█████│     │     │     │     │  Feishu Bridge
F3   │     │     │     │     │█████│█████│     │     │  Message Router
F4   │     │     │     │     │     │█████│█████│     │  Agent Backend (Zed)
F5   │     │     │     │     │     │██   │██   │██   │  Security（增量）
─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┤
     │     │     │     │     │     │ CP2 │     │ CP3 │  检查点
                                                           CP4
```

**总工期**：8 周

**并行策略**：
- F7（Zed Fork）与 F0-F2 **完全独立**，可第 1 周立即启动
- F6（Logging）横切所有模块，持续开发
- F4.1（ClaudeCodeBackend）作为快速验证通道，可在 F7 完成前先行调试 Agent Loop

---

## 4.2 详细任务拆分

### Phase 4a：基础建设（Week 1-3）

#### F0：Config & Bootstrap（Week 1-2）

> 依赖：无
> 被依赖：所有模块

| ID | 任务 | 估时 | 说明 |
|----|------|------|------|
| F0.1 | 创建 `kairos-claw` Cargo project | 0.5d | 在 `KairosEngine/` 下创建新的 binary crate |
| F0.2 | 定义 `Config` struct + serde 反序列化 | 1d | 完整的 TOML schema，所有字段带默认值 |
| F0.3 | 实现 `Config::load()` + 校验 | 0.5d | 从 `~/.kairos-claw/config.toml` 加载，校验必填字段 |
| F0.4 | 实现 `claw init` 命令 | 1d | 创建目录结构 + 写入默认配置 + 设置文件权限（600/700） |
| F0.5 | 实现配置热重载（`notify`） | 1d | 监听文件变化，安全变更热应用，危险变更记录日志 |
| F0.6 | 环境变量替换（`$VAR` 语法） | 0.5d | 支持 `app_secret = "$FEISHU_APP_SECRET"` |
| F0.7 | 编写单元测试 | 0.5d | 覆盖：默认值、校验失败、环境变量替换、权限检查 |

**产出**：`claw init` 可用，配置系统就绪。

---

#### F1：Daemon Process（Week 2-3）

> 依赖：F0
> 被依赖：F2

| ID | 任务 | 估时 | 说明 |
|----|------|------|------|
| F1.1 | 实现 `claw daemon run`（前台模式） | 1d | tokio runtime 启动，加载配置，启动各子系统 |
| F1.2 | 实现 `claw daemon start/stop/status` CLI | 1.5d | 通过 PID 文件管理进程生命周期 |
| F1.3 | macOS launchd 集成 | 1d | `claw daemon install` 生成 plist，`KeepAlive: true` |
| F1.4 | Linux systemd 集成 | 0.5d | 生成 user service unit，`Restart=always` |
| F1.5 | 优雅关闭 | 1d | SIGTERM → 等当前 agent turn 完成 → 关闭 channel → 关闭 DB |
| F1.6 | 编写集成测试 | 0.5d | 覆盖：启动/停止/重启/崩溃重启验证 |

**产出**：daemon 可前台调试 + 后台运行 + 崩溃自恢复。

---

#### F7：Zed Fork — Agent IPC（Week 1-3，并行）

> 依赖：无
> 被依赖：F4.2（ZedBackend）

| ID | 任务 | 估时 | 说明 |
|----|------|------|------|
| F7.1 | Fork Zed，创建 `claw-ipc` 分支 | 0.5d | 从 Zed stable release tag 创建分支 |
| F7.2 | 新增 `crates/claw_ipc/` crate | 0.5d | Cargo.toml + 目录结构 |
| F7.3 | 实现 IPC 协议定义（`protocol.rs`） | 1d | JSON newline-delimited 协议，struct 定义 + serde |
| F7.4 | 实现 Unix Socket server（`server.rs`） | 1.5d | 监听 `/tmp/zed-agent.sock`，握手 + 帧解析 |
| F7.5 | 实现 IPC → Agent Panel 桥接（`bridge.rs`） | 2d | 收到 prompt → 创建/查找 session → 调用 Agent Panel prompt 流程 → 流式返回 |
| F7.6 | 修改 `crates/cli/` 添加 `CliRequest::PromptAgent` | 0.5d | 新增 variant + 序列化 |
| F7.7 | 实现 `zed agent send` CLI 子命令 | 1d | 连接 Unix Socket → 发送 prompt → 流式打印响应 |
| F7.8 | 实现 `zed agent cancel` CLI | 0.5d | 中断指定 session |
| F7.9 | Feature gate + 构建验证 | 0.5d | `#[cfg(feature = "claw-ipc")]`，确保正常构建不受影响 |
| F7.10 | 端到端手动测试 | 1d | 启动 Zed Fork → `nc -U /tmp/zed-agent.sock` 发送 JSON → 验证 Agent Panel 响应 |

**产出**：Zed Fork 可通过 IPC 接收外部消息并流式返回 Agent 响应。

---

#### F6：Logging & Observability（Week 2-8，横切）

> 依赖：F0
> 被依赖：所有模块（写入日志）

| ID | 任务 | 估时 | 说明 |
|----|------|------|------|
| F6.1 | tracing 初始化 | 0.5d | JSON 格式输出到文件 + 控制台，按 daemon log_level 配置 |
| F6.2 | 日志轮转 | 0.5d | `tracing-appender`，按天轮转，保留 7 天 |
| F6.3 | 关键事件插桩 | 持续 | 每个模块开发时同步添加 tracing span/event |
| F6.4 | Health endpoint | 1d | `axum` HTTP server，`GET /health` 返回 daemon + channel + backend 状态 |

**产出**：结构化日志 + 健康检查。

---

### Phase 4b：消息通道（Week 3-4）

#### F2：Feishu Bridge

> 依赖：F0、F1
> 被依赖：F3

| ID | 任务 | 估时 | 说明 |
|----|------|------|------|
| F2.1 | 定义 `ChannelBridge` trait | 0.5d | `start()` → `BoxStream<RawMessage>`，`send_reply()`，`health_check()` |
| F2.2 | 实现飞书 Token 管理 | 1d | `tenant_access_token` 获取 + 缓存 + 自动刷新（token 有效期 2h） |
| F2.3 | 实现飞书 Webhook 接收 | 1.5d | axum route `POST /webhook/feishu` → 验签 → 反序列化 → `RawMessage` |
| F2.4 | 实现 URL 验证（Challenge） | 0.5d | 首次配置时飞书发送 challenge，1 秒内返回 |
| F2.5 | 实现飞书消息发送 | 1d | `POST /im/v1/messages`，支持文本 + 卡片消息 |
| F2.6 | 启动时公网可达性检查 | 0.5d | 检测 `public_url` 可达性，不可达打印错误日志 + ngrok 提示 |
| F2.7 | 实现 `FeishuBridge` struct（组合上述） | 1d | 组装为完整 struct，实现 `ChannelBridge` trait |
| F2.8 | 编写集成测试 | 0.5d | Mock 飞书 API，验证消息收发 + token 刷新 + 验签 |

**产出**：飞书消息可以收发。

---

### Phase 4c：消息路由（Week 5-6）

#### F3：Message Router

> 依赖：F2（需要真实消息流）
> 被依赖：F4

| ID | 任务 | 估时 | 说明 |
|----|------|------|------|
| F3.1 | 定义 `MessageEnvelope` + `RoutedMessage` struct | 0.5d | 归一化消息格式 |
| F3.2 | 实现 `Router` struct | 1d | 从 mpsc receiver 接收 RawMessage → 路由到 session |
| F3.3 | 实现 Session Manager | 1.5d | Session 创建/查找/超时挂起/销毁，内存 HashMap + SQLite 持久化 |
| F3.4 | 实现 Session Key 生成 | 0.5d | `main:feishu:dm` / `main:feishu:group:{id}` |
| F3.5 | 实现并发控制 | 1d | 全局并发槽位（默认 3），session 内串行，session 间并行 |
| F3.6 | 实现上下文管理 | 1d | 加载最近 N 轮对话 + `AGENTS.md`/`CONTEXT.md` 注入 |
| F3.7 | 实现 DB Writer task | 1d | 异步批量写入 transcript，WAL 模式 |
| F3.8 | 编写单元测试 | 1d | 覆盖：session 路由、并发控制、上下文加载、超时 |

**产出**：消息可正确路由到 session，上下文传递到 Agent Backend。

---

### Phase 4d：Agent 集成（Week 6-8）

#### F4：Agent Backends

> 依赖：F3（Router）+ F7（Zed Fork）
> 被依赖：无

| ID | 任务 | 估时 | 说明 |
|----|------|------|------|
| F4.1 | 定义 `AgentBackend` trait | 0.5d | `send_prompt()` → `BoxStream<AgentEvent>`，`cancel()`，`health_check()` |
| F4.2 | 实现 Backend Registry | 0.5d | 根据配置激活对应 backend |
| F4.3 | 实现 `ClaudeCodeBackend`（快速验证） | 1.5d | `tokio::process::Command` 调用 `claude` CLI，解析 stdout 流 |
| F4.4 | 实现 `ZedBackend` | 2d | 连接 Zed Fork IPC Socket → JSON 协议交互 → 流式解析 |
| F4.5 | Session 映射（claw ↔ Zed） | 0.5d | claw session_key ↔ Zed session_id 双向映射表 |
| F4.6 | 中断处理 | 0.5d | Ctrl+C / 用户发送 "取消" → `cancel()` IPC 调用 |
| F4.7 | 流式响应 → IM 回复 | 1d | AgentEvent stream → 分段发送飞书消息（thinking 不发送，text_delta 攒批发送） |
| F4.8 | 故障转移 | 0.5d | ZedBackend 不可用 → 自动 fallback 到 ClaudeCodeBackend |
| F4.9 | 端到端集成测试 | 1d | 飞书消息 → Router → Agent → 回复，全链路验证 |

**产出**：完整的消息 → Agent → 回复闭环。

---

#### F5：Security & Auth（Week 6-8，增量）

> 依赖：F3
> 被依赖：F4（工具执行前的安全检查）

| ID | 任务 | 估时 | 说明 |
|----|------|------|------|
| F5.1 | DM Allowlist 实现 | 1d | 仅 `config.security.allowlist` 中的 sender_id 可触发 Agent |
| F5.2 | Pairing 模式实现 | 1.5d | 未知发送者收到 pairing code，需 `claw pairing approve` 确认 |
| F5.3 | 危险操作分级 + 拦截 | 1d | 定义危险类别，Agent tool_call 前检查，需用户确认 |
| F5.4 | 密钥管理 | 0.5d | 环境变量读取，凭证目录 600 权限 |
| F5.5 | 安全审计命令 | 0.5d | `claw security audit` 检查配置安全性 |

**产出**：基本安全防护就绪。

---

## 4.3 检查点

| 检查点 | 时间 | 完成标记 | 验证方式 |
|--------|------|----------|----------|
| **CP1** | Week 3 末 | F0 + F1 + F7 完成 | `claw daemon run` 前台运行正常；Zed Fork IPC 可收发消息 |
| **CP2** | Week 5 初 | F2 完成 | 飞书发送消息 → claw 收到 → 日志记录，回复 "pong" 确认 |
| **CP3** | Week 6 末 | F3 完成 + F4.3 | 飞书消息 → Router → ClaudeCodeBackend → 飞书回复，首条端到端链路 |
| **CP4** | Week 8 末 | F4 + F5 完成 | 飞书消息 → Router → ZedBackend → Zed Agent 执行代码操作 → 飞书回复 |

## 4.4 关键路径

```
F0 ──→ F1 ──→ F2 ──→ F3 ──→ F4.3 (ClaudeCode验证) ──→ F4.4 (ZedBackend) ──→ CP4
                ↑                          ↑
                │                          │
         F7 (Zed Fork) ────────────────────┘  (并行，Week 1-3)
```

**最长路径**：F0 → F1 → F2 → F3 → F4.4 = 8 周

**并行机会**：
- F7 与 F0-F2 完全独立，Week 1 可立即启动
- F4.3（ClaudeCodeBackend）可在 F7 完成前先行验证 Agent Loop，降低 ZedBackend 集成风险
- F5 和 F6 是横切关注点，不占额外时间线

---

## 4.5 风险缓冲

| 风险 | 缓冲策略 |
|------|----------|
| Zed Fork IPC 集成复杂度高于预期 | 提前在 Week 2 末做 PoC 验证；F4.3（ClaudeCodeBackend）作为降级方案 |
| 飞书 Webhook 验证调试耗时 | Week 3 预留 1 天 buffer |
| Rust crate 兼容性问题 | Week 1 即引入所有新依赖，提前发现冲突 |

---

## 4.6 后续迭代（Outside Phase 1）

以下功能不在 8 周排期内，标记为后续迭代：

| 功能 | 说明 |
|------|------|
| WeChat Bridge | 企业微信或个人微信接入 |
| CustomAgentBackend | 自定义 HTTP endpoint 后端 |
| 图片/文件消息 | 飞书图片/文件消息接收和回复 |
| Windows Service 集成 | `claw daemon install` Windows 版 |
| ClaudeCodeBackend 生产就绪 | 完善 session 管理和错误恢复 |
| 多用户支持 | 如果未来需要团队使用 |
