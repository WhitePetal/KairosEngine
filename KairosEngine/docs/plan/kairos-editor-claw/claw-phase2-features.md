# Kairos Editor Claw 设计案 — Phase 2：功能分析

> **状态**：进行中
> **创建日期**：2026-07-24
> **前置文档**：[Phase 1 需求分析](./claw-phase1-requirements.md)

## 2.1 功能全景

基于 Phase 1 确认的 7 项决策，kairos_editor_claw 的功能域如下：

```
┌──────────────────────────────────────────────────────────────┐
│                    kairos_editor_claw                        │
│                                                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐  │
│  │   F1     │  │   F2     │  │   F3     │  │    F4      │  │
│  │ Daemon   │→│ Channel  │→│ Message  │→│  Agent     │  │
│  │Process   │  │ Bridges  │  │ Router   │  │  Backends  │  │
│  └──────────┘  └──────────┘  └──────────┘  └────────────┘  │
│                     ↑              ↑              │          │
│               ┌─────┴──────────────┴──────┐       │          │
│               │           F0              │       │          │
│               │   Config & Bootstrap      │       │          │
│               └───────────────────────────┘       │          │
│                                                   ↓          │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────────────┐ │
│  │   F5     │  │   F6     │  │         F7                 │ │
│  │ Security │  │ Logging  │  │   Zed Fork (外部依赖)       │ │
│  │  & Auth  │  │  & Obs   │  │   IPC 扩展 + CLI 子命令    │ │
│  └──────────┘  └──────────┘  └────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘
```

| ID | 功能域 | 说明 | 优先级 |
|----|--------|------|--------|
| **F0** | Config & Bootstrap | TOML 配置加载、热重载、工作目录初始化 | P0 |
| **F1** | Daemon Process | 后台进程生命周期管理（launchd/systemd/Windows Service） | P0 |
| **F2** | Channel Bridges | 消息平台接入（WeChat / Feishu），归一化 Message Envelope | P0 |
| **F3** | Message Router | Session 管理、消息分发、多轮对话上下文 | P0 |
| **F4** | Agent Backends | 可插拔 Agent 后端（ZedBackend 为主 + ClaudeCodeBackend 为备） | P0 |
| **F5** | Security & Auth | 白名单认证、危险操作确认、密钥管理 | P1 |
| **F6** | Logging & Observability | 结构化日志、健康检查、metrics | P1 |
| **F7** | Zed Fork | Fork Zed 添加 Agent IPC 扩展（`zed agent send` 命令） | P1 |

---

## 2.2 功能详细拆解

### F0：Config & Bootstrap

> 依赖：无
> 被依赖：F1、F2、F3、F4、F5、F6
> 优先级：P0（所有模块的前置依赖）

#### F0.1 配置文件格式定义

**配置层级**：
```
~/.kairos-claw/
├── config.toml          # 主配置（用户可编辑）
├── state/
│   └── claw.sqlite      # 运行时状态（会话历史、token 等）
└── credentials/         # 敏感凭证（600 权限）
    ├── wechat.token
    └── feishu.token
```

**`config.toml` schema**：

```toml
[daemon]
log_level = "info"        # trace | debug | info | warn | error

[agent]
backend = "zed"           # zed | claude-code | custom

[agent.zed]
binary_path = "/usr/local/bin/zed"
ipc_timeout_ms = 30000

[agent.claude-code]
binary_path = "/usr/local/bin/claude"
model = "claude-sonnet-4-6"

[channels.wechat]
enabled = false
# 具体配置取决于接入方式，Phase 3 确定

[channels.feishu]
enabled = false

[security]
dm_policy = "pairing"     # pairing | allowlist | open
allowlist = []            # 白名单用户 ID 列表
require_approval_for = [  # 需要二次确认的操作类型
    "exec_destructive",
    "git_force_push",
    "network_outbound",
]
```

#### F0.2 配置加载与校验

| ID | 任务 | 说明 |
|----|------|------|
| F0.2.1 | TOML 反序列化 | `config::load(path)` → `Result<Config>`，使用 `toml` + `serde` |
| F0.2.2 | Schema 校验 | 启动时校验必填字段、枚举值合法性、路径存在性 |
| F0.2.3 | 默认值填充 | `Config::default()` 提供所有字段的合理默认值，用户配置按需覆盖 |
| F0.2.4 | 配置热重载 | 监听 `config.toml` 文件变化（`notify` crate），安全变更热应用，危险变更记录日志并在下次重启生效 |

#### F0.3 工作目录初始化

| ID | 任务 | 说明 |
|----|------|------|
| F0.3.1 | `claw init` 命令 | 首次运行时创建 `~/.kairos-claw/` 目录结构 + 默认 `config.toml` |
| F0.3.2 | 权限设置 | `credentials/` 目录 700，文件 600 |
| F0.3.3 | 迁移/升级 | 版本号写入 `state/version.txt`，后续支持配置格式迁移 |

---

### F1：Daemon Process

> 依赖：F0
> 被依赖：F2、F3、F4（daemon 是它们的宿主进程）
> 优先级：P0

#### F1.1 进程生命周期

| ID | 任务 | 说明 |
|----|------|------|
| F1.1.1 | 启动 | `claw daemon start` 启动后台进程，使用 `daemonize` 或平台原生方式脱离终端 |
| F1.1.2 | 停止 | `claw daemon stop` 发送 SIGTERM，优雅关闭（等当前 agent turn 完成） |
| F1.1.3 | 重启 | `claw daemon restart` = stop + start |
| F1.1.4 | 状态查询 | `claw daemon status` 返回 PID、运行时间、活跃 session 数、健康状态 |
| F1.1.5 | 前台模式 | `claw daemon run` 前台运行（调试用），stdout 输出日志 |

#### F1.2 平台进程守护

| ID | 任务 | 说明 |
|----|------|------|
| F1.2.1 | macOS launchd | 生成 `~/Library/LaunchAgents/com.kairos.claw.plist`，`KeepAlive: true` 保证崩溃重启 |
| F1.2.2 | Linux systemd | 生成 `~/.config/systemd/user/kairos-claw.service`，`Restart=always` |
| F1.2.3 | Windows Service | 使用 `windows-service` crate 或 `Task Scheduler` 触发登录时自动启动 |

#### F1.3 进程内并发模型

```
┌─────────────────────────────────────┐
│         tokio runtime               │
│                                      │
│  ┌────────┐  ┌────────┐  ┌───────┐ │
│  │WeChat  │  │Feishu  │  │Signal │ │  ← 每 channel 一个 task
│  │task    │  │task    │  │task   │ │
│  └───┬────┘  └───┬────┘  └───┬───┘ │
│      │           │           │      │
│      └───────────┼───────────┘      │
│                  ↓                  │
│          ┌──────────────┐           │
│          │ mpsc channel  │           │  ← 消息汇聚
│          └──────┬───────┘           │
│                 ↓                   │
│          ┌──────────────┐           │
│          │ Router task   │           │  ← 单 task 串行处理（保证顺序）
│          └──────┬───────┘           │
│                 ↓                   │
│          ┌──────────────┐           │
│          │ Agent task    │           │  ← 每 session 一个并发槽位
│          │ (per-session) │           │
│          └──────────────┘           │
└─────────────────────────────────────┘
```

**关键设计**：
- Channel tasks 通过 `tokio::sync::mpsc` 发送到 Router task，保证消息顺序
- Router task 单线程串行处理（避免 session 竞态）
- Agent task 按 session 隔离，不同 session 可并行

---

### F2：Channel Bridges

> 依赖：F0、F1
> 被依赖：F3
> 优先级：P0

#### F2.1 Channel Trait 定义

```rust
#[async_trait]
trait ChannelBridge: Send + Sync {
    /// 平台标识
    fn channel_id(&self) -> &'static str;  // "wechat" | "feishu"

    /// 启动监听，返回消息流
    async fn start(&mut self) -> Result<BoxStream<RawMessage>>;

    /// 发送回复
    async fn send_reply(&self, target: &MessageTarget, content: &ReplyContent) -> Result<()>;

    /// 健康检查
    async fn health_check(&self) -> Result<ChannelStatus>;
}

struct RawMessage {
    channel_id: String,        // "wechat" | "feishu"
    sender_id: String,         // 发送者 ID
    chat_type: ChatType,       // Direct | Group
    group_id: Option<String>,  // 群聊 ID
    content: MessageContent,   // 消息体
    timestamp: DateTime<Utc>,
}

enum ChatType { Direct, Group }

enum MessageContent {
    Text(String),
    // 后续扩展：
    // Image { url: String, caption: Option<String> },
    // File { url: String, filename: String },
}
```

#### F2.2 WeChat Bridge

| ID | 任务 | 说明 |
|----|------|------|
| F2.2.1 | 确定接入方案 | Phase 3 确认企业微信 / 个人微信协议方案 |
| F2.2.2 | 实现 `WeChatBridge` | 实现 `ChannelBridge` trait |
| F2.2.3 | 消息收发 | 接收消息（文本）+ 发送回复 |
| F2.2.4 | 连接保活 | 心跳/重连机制 |

**注意**：具体实现细节依赖 Phase 3 选定的接入方式。F2.2 先占位 trait 实现框架。

#### F2.3 Feishu Bridge

| ID | 任务 | 说明 |
|----|------|------|
| F2.3.1 | 确定接入方案 | Phase 3 确认飞书 Bot / 自定义应用方案 |
| F2.3.2 | 实现 `FeishuBridge` | 实现 `ChannelBridge` trait |
| F2.3.3 | 消息收发 | 接收消息（文本）+ 发送回复 |
| F2.3.4 | 事件订阅 | Long Polling 或 Webhook 接收事件 |

#### F2.4 Channel 注册与管理

| ID | 任务 | 说明 |
|----|------|------|
| F2.4.1 | Channel Registry | 根据配置启用/禁用 channel，动态注册 |
| F2.4.2 | 统一启动/停止 | Daemon 启动时启动所有已启用 channel，停止时优雅关闭 |

---

### F3：Message Router

> 依赖：F2（接收归一化消息）
> 被依赖：F4（将消息路由到 Agent Backend）
> 优先级：P0

#### F3.1 Message Envelope 归一化

`F2` 的 `ChannelBridge` 已经产出 `RawMessage`。Router 将其转换为 `RoutedMessage`：

```rust
struct RoutedMessage {
    envelope: MessageEnvelope,
    session_key: SessionKey,
}

struct MessageEnvelope {
    source: MessageSource,       // 来源平台 + 发送者
    content: MessageContent,     // 归一化后的消息内容
    reply_context: ReplyContext, // 回复所需的路由信息
    timestamp: DateTime<Utc>,
}

struct MessageSource {
    channel_id: String,         // "wechat" | "feishu"
    sender_id: String,          // 发送者唯一标识
    chat_type: ChatType,        // Direct | Group
    group_id: Option<String>,   // 群聊 ID（Group 时有值）
}

struct ReplyContext {
    channel_id: String,
    target: MessageTarget,      // 回复目标（DM 对端 ID 或 group ID）
    thread_id: Option<String>,  // 话题 ID（飞书等支持线程的平台）
}
```

#### F3.2 Session 管理

| ID | 任务 | 说明 |
|----|------|------|
| F3.2.1 | Session Key 生成 | 规则：`{agent_id}:{channel_id}:{chat_type}:{peer_or_group_id}` |
| F3.2.2 | Session 生命周期 | 创建、激活、挂起（超时未活动自动挂起）、销毁 |
| F3.2.3 | Session 隔离 | DM 默认共享一个 session（`dmScope = "main"`），群聊每个群独立 session |
| F3.2.4 | 会话历史持久化 | SQLite 存储 `(session_key, timestamp, role, content)`，支持加载最近 N 轮 |

**Session Key 规则（单用户简化版）**：

| 来源 | Session Key | 说明 |
|------|-------------|------|
| 微信 DM | `main:wechat:dm` | 所有微信私聊共享（来自不同人的消息 merge 到同一上下文） |
| 微信群聊 | `main:wechat:group:{group_id}` | 每个群独立 |
| 飞书 DM | `main:feishu:dm` | 所有飞书私聊共享 |
| 飞书群聊 | `main:feishu:group:{group_id}` | 每个群独立 |

> 后续可扩展 `dmScope = "per-peer"` 按发送者隔离 DM session。

#### F3.3 消息路由与并发控制

| ID | 任务 | 说明 |
|----|------|------|
| F3.3.1 | 消息入队 | 同一 session 的消息串行入队，不同 session 可并行 |
| F3.3.2 | 并发槽位 | 全局并发数上限（默认 3），防止 LLM API 并发过高 |
| F3.3.3 | 中断处理 | 用户发送 "停止"/"取消" 等指令时，中断当前 Agent turn |
| F3.3.4 | 超时处理 | Agent turn 超时（默认 5 分钟）自动中止，返回超时提示 |

#### F3.4 多轮对话上下文

| ID | 任务 | 说明 |
|----|------|------|
| F3.4.1 | 上下文窗口管理 | 加载最近 N 轮对话（默认 20 轮），超 token 限制时自动压缩/截断 |
| F3.4.2 | 上下文注入 | 将项目 `AGENTS.md`、`CONTEXT.md` 作为 system context 注入 |
| F3.4.3 | Skills 注入 | 加载 Zed Skills（`.agents/skills/`）描述作为可用工具列表 |

---

### F4：Agent Backends

> 依赖：F3（接收 RoutedMessage）
> 被依赖：无
> 优先级：P0

#### F4.1 Backend Trait 定义

```rust
#[async_trait]
trait AgentBackend: Send + Sync {
    /// 后端标识
    fn backend_id(&self) -> &'static str;

    /// 发送 prompt 并获取流式响应
    async fn send_prompt(
        &self,
        session: &SessionContext,
        prompt: &str,
    ) -> Result<BoxStream<AgentEvent>>;

    /// 中断当前运行
    async fn cancel(&self, session_key: &str) -> Result<()>;

    /// 健康检查
    async fn health_check(&self) -> Result<BackendStatus>;
}

struct SessionContext {
    session_key: String,
    history: Vec<ChatMessage>,    // 最近 N 轮对话
    workspace_path: PathBuf,      // 项目路径
    skills: Vec<SkillMetadata>,   // 可用 skills
    config: AgentConfig,          // 后端特定配置
}

enum AgentEvent {
    Thinking(String),              // 思考过程（streaming）
    TextDelta(String),             // 文本增量（streaming）
    ToolCall { name: String, params: serde_json::Value },
    ToolResult { name: String, result: String },
    Completed { usage: UsageStats },
    Error { message: String },
}

struct BackendStatus {
    healthy: bool,
    backend_id: String,
    details: String,
}
```

#### F4.2 ZedBackend（主后端）

> 依赖：F7（Zed Fork 提供 IPC 能力）

| ID | 任务 | 说明 |
|----|------|------|
| F4.2.1 | 通过 Zed Fork IPC 发送 prompt | 调用 `zed agent send --session {key} --prompt "..."` 或直连 Unix Socket |
| F4.2.2 | 流式响应接收 | 读取 Zed Fork IPC 返回的流式 AgentEvent |
| F4.2.3 | Session 映射 | claw session key ↔ Zed session ID 的双向映射表 |
| F4.2.4 | 中断处理 | 通过 IPC 发送 cancel 指令 |
| F4.2.5 | Skills 同步 | 将 claw 侧的 skills 列表同步给 Zed（或直接读取 Zed 的 skills） |
| F4.2.6 | 健康检查 | 检查 Zed 进程是否运行 + Fork IPC 是否可达 |

**Zed Fork IPC 协议设计**（`F7` 产出）：

```
claw daemon                       Zed (Fork)
    │                                 │
    ├── connect ─────────────────────→│  Unix Socket
    │←─ { accepted, session_id } ────│
    │                                 │
    ├── req:prompt ──────────────────→│  { session_key, message, history, skills }
    │←─ event:thinking ──────────────│  { delta }
    │←─ event:text ──────────────────│  { delta }
    │←─ event:tool_call ────────────│  { name, params }
    │←─ event:tool_result ──────────│  { name, result }
    │←─ event:completed ────────────│  { usage }
    │                                 │
    ├── req:cancel ──────────────────→│
    │←─ event:cancelled ─────────────│
```

#### F4.3 ClaudeCodeBackend（备后端）

> 依赖：无（只需 `claude` CLI 可用）

| ID | 任务 | 说明 |
|----|------|------|
| F4.3.1 | CLI subprocess 调用 | `claude --print --session-id {key} -p "{prompt}"` 或等效命令 |
| F4.3.2 | 流式输出解析 | 解析 `claude` CLI 的 stdout 输出为 AgentEvent 流 |
| F4.3.3 | Session 持久化 | `claude` CLI 自身维护 session，claw 侧记录映射 |
| F4.3.4 | 中断处理 | 向 subprocess 发送 SIGINT |

#### F4.4 Backend 注册与切换

| ID | 任务 | 说明 |
|----|------|------|
| F4.4.1 | Backend Registry | 类似 Channel Registry，根据 `config.agent.backend` 选择激活的 backend |
| F4.4.2 | 运行时切换 | 支持 `claw backend switch zed|claude-code` CLI 命令（需等当前 turn 完成） |
| F4.4.3 | 故障转移 | 主 backend 不可用时自动 fallback 到备 backend（可配置） |

---

### F5：Security & Auth

> 依赖：F0、F3
> 被依赖：无（横切关注点）
> 优先级：P1（首批可简化，后续完善）

#### F5.1 DM 访问控制

| ID | 任务 | 说明 |
|----|------|------|
| F5.1.1 | Pairing 模式 | 未知发送者收到 pairing code，需 `claw pairing approve {code}` 方可通信 |
| F5.1.2 | Allowlist 模式 | 仅 `config.security.allowlist` 中的 ID 可以发送指令 |
| F5.1.3 | Open 模式 | 任何人可发（需显式开启，默认关闭） |

#### F5.2 危险操作确认

| ID | 任务 | 说明 |
|----|------|------|
| F5.2.1 | 操作分级 | 定义危险操作类别：`exec_destructive`、`git_force_push`、`network_outbound`、`file_delete` |
| F5.2.2 | 确认流程 | Agent 执行危险操作前 → claw 拦截 → 发送确认消息到 IM → 用户回复 "确认" / "取消" |
| F5.2.3 | Session 级授权 | 支持 "本次 session 不再确认" 选项（减少打扰） |

#### F5.3 密钥管理

| ID | 任务 | 说明 |
|----|------|------|
| F5.3.1 | 密钥存储 | `~/.kairos-claw/credentials/` 目录，文件权限 600 |
| F5.3.2 | 环境变量注入 | 支持从环境变量读取敏感值（`$ENV_VAR` 语法），不在配置文件中明文写密钥 |
| F5.3.3 | LLM API Key | 通过环境变量传递（`ANTHROPIC_API_KEY`、`OPENAI_API_KEY` 等），不经过 claw 存储 |

---

### F6：Logging & Observability

> 依赖：F1
> 被依赖：无（横切关注点）
> 优先级：P1

#### F6.1 结构化日志

| ID | 任务 | 说明 |
|----|------|------|
| F6.1.1 | 日志输出 | `tracing` crate，支持 JSON 格式输出到文件 + 控制台 |
| F6.1.2 | 日志级别 | 运行时可通过 `claw daemon log-level set debug` 动态调整 |
| F6.1.3 | 日志轮转 | 按天轮转，保留最近 7 天 |
| F6.1.4 | 关键事件记录 | 消息收发、Agent turn 开始/结束、错误、认证事件 |

#### F6.2 健康检查与监控

| ID | 任务 | 说明 |
|----|------|------|
| F6.2.1 | Health endpoint | 可选 HTTP endpoint（`127.0.0.1:PORT/health`）返回 daemon + 各 channel + backend 状态 |
| F6.2.2 | Metrics | 消息吞吐量、Agent turn 耗时、错误率（可选集成 `prometheus` 或仅日志） |

---

### F7：Zed Fork — Agent IPC 扩展

> 依赖：无（独立项目）
> 被依赖：F4.2（ZedBackend 依赖此 Fork）
> 优先级：P1

这是 `kairos_editor_claw` 的外部依赖项。需要在 Zed 源码中添加以下能力：

#### F7.1 IPC 协议扩展

| ID | 任务 | 说明 |
|----|------|------|
| F7.1.1 | `CliRequest::PromptAgent` | 新增 variant：`{ session_key: String, message: String, history: Vec<ChatMessage>, skills: Vec<String> }` |
| F7.1.2 | `CliResponse::AgentEvent` | 新增 variant：流式返回 `AgentEvent`（thinking / text_delta / tool_call / tool_result / completed / error） |
| F7.1.3 | `CliRequest::CancelAgent` | 新增 variant：中断当前 agent turn |

#### F7.2 Agent 路由

| ID | 任务 | 说明 |
|----|------|------|
| F7.2.1 | IPC → Agent Panel 路由 | 收到 `PromptAgent` 后，找到或创建对应 session，调用 Agent Panel 的 prompt 流程 |
| F7.2.2 | Session CRUD | 支持通过 IPC 查询活跃 session、关闭 session、切换 session |

#### F7.3 CLI 子命令

| ID | 任务 | 说明 |
|----|------|------|
| F7.3.1 | `zed agent send` | `zed agent send --session {key} --prompt "..."`，通过 IPC 发送到运行中的 Zed 实例 |
| F7.3.2 | `zed agent status` | `zed agent status` 列出活跃 session 及其状态 |
| F7.3.3 | `zed agent cancel` | `zed agent cancel --session {key}` 中断指定 session |

---

## 2.3 依赖关系

```
                           F0 Config & Bootstrap
                          /          |          \
                         /           |           \
                        v            v            v
                      F1           F7            F5
                    Daemon      Zed Fork       Security
                       |           |              |
                       v           |              |
                      F2 ──────────┘              |
                   Channel Bridges                |
                       |                          |
                       v                          |
                      F3                          |
                  Message Router                  |
                       |                          |
                       v                          |
                      F4 ─────────────────────────┘
                  Agent Backends
                       |
                       v
                      F6
                 Logging & Obs
```

```
依赖链：

F0 ──→ F1 ──→ F2 ──→ F3 ──→ F4
  │                          ↑
  └──→ F5 ──────────────────┘  (F4 调用 F5 检查权限)
  │      ↑
  └──→ F7 ────→ F4.2  (F4.2 ZedBackend 依赖 F7 Fork)
  │
  └──→ F6 (所有模块写入日志)
```

**关键依赖关系**：
- **F0 是全局前置**：所有模块的配置来源
- **F1 → F2 → F3 → F4 是核心数据流**：从左到右串行依赖
- **F7 与 F1-F3 可并行开发**：Fork Zed 是独立项目，只需在 F4.2 集成时联调
- **F5 横切**：F3（消息接收）、F4（工具执行）都需要调用 F5 做安全检查
- **F6 横切**：所有模块写入日志

---

## 2.4 待确认事项

| # | 问题 | 状态 |
|---|------|------|
| ~~Q6~~ | ~~消息类型范围？~~ | ✅ D-008（Phase 1 纯文本，枚举预留接口） |
| ~~Q7~~ | ~~安全审批粒度？~~ | ✅ D-009（两级模型） |
