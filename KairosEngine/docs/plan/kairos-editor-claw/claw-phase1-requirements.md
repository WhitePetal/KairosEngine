# Kairos Editor Claw 设计案 — Phase 1：需求分析

> **状态**：进行中
> **创建日期**：2026-07-24

## 1.1 背景

### 问题来源

开发者在离开电脑时（通勤、会议、休息），仍然希望能够：
- 通过手机上的微信/飞书等 IM 工具向本机的 AI 编程助手发送指令
- 让 AI 助手在本机执行代码编写、调试、提交等操作
- 返回执行结果到手机端

当前 KairosEngine 的开发完全依赖在 Zed 编辑器内手动与 AI Agent 交互，无法远程操控。

### OpenClaw 的启示

已完成的调研（`docs/research/claw-tools-architecture.md`）表明：

- **OpenClaw** 是一个开源的自托管个人 AI 助手框架，核心架构为 Gateway Daemon + Channel Plugins + Agent Loop
- 它支持 25+ 消息平台（含微信、飞书），通过 channel plugin 将各平台消息归一化为统一信封，路由到 Agent Loop
- Agent Loop 遵循 `prompt assembly → LLM call → tool execution → reply` 循环
- 安全模型采用四层纵深防御：Identity → Scope → Capability → Model

**关键区别**：OpenClaw 是通用 AI 助手（可在聊天平台对话 + 调用工具），而 `kairos_editor_claw` 的目标更聚焦——**将消息平台的输入路由到 Zed 的 AI Agent，实现对代码仓库的远程控制**。

### Zed AI Agent 集成现状

已完成的调研（`docs/research/zed-ai-agent-integration.md`）表明：

**Zed 编辑器当前没有任何外部 API 可以注入消息到 AI Agent 对话中。** 具体情况：

| 途径 | 状态 | 说明 |
|------|------|------|
| Zed CLI (`zed` 命令) | ❌ 不可用 | 只能打开文件/目录，无法与 agent 交互 |
| ACP 协议 (Agent Control Protocol) | ❌ 内部协议 | 仅限 Zed 进程内 Thread ↔ AcpThread 通信 |
| MCP (Model Context Protocol) | ❌ 仅入站 | Zed 是 MCP Client，不作为 MCP Server |
| Extension API (WASM) | ❌ 无权限 | 沙箱环境，完全无法访问 agent 内部 |
| HTTP/WebSocket/Unix Socket | ❌ 不存在 | 无任何网络 API |

**可行方案**（按推荐度排序）：

| 方案 | 难度 | 可靠性 | 描述 |
|------|------|--------|------|
| **Fork Zed + 自定义 IPC** | 高 | ✅ 高 | 在 Zed 源码中添加 IPC 通道（Unix Socket / WebSocket），`zed agent send` CLI 子命令，Zed 内部监听并注入消息到 Agent Panel |
| **AppleScript / 系统辅助功能** | 中 | ⚠️ 低 | 用 osascript 模拟键盘输入到 Zed 窗口打开 Agent Panel 并粘贴消息。仅限 macOS，脆弱（窗口焦点、布局变化等） |
| **文件系统消息队列** | 中 | ⚠️ 中 | daemon 写消息到约定文件，Fork Zed 或自定义 Extension 监听文件变化并注入 Agent。需要 Fork 或 Extension API 支持（当前不存在） |
| **绕过 Zed，直接调用 AI Agent** | 低 | ✅ 高 | daemon 直接运行 Claude Code CLI 或类似 headless agent，不经过 Zed 的 Agent Panel。消息通过 IM 来回。失去 Zed 编辑器的可视化反馈 |

> **关键洞察**：方案 D（绕过 Zed）在技术上最简单可靠，但失去了 Zed 编辑器内的可视化交互。方案 A（Fork Zed）是最干净的长期方案，但维护成本高。实际开发中可能采用**分阶段策略**——先用方案 D 快速验证核心流程，再根据需求决定是否 Fork Zed。

## 1.2 核心概念

| 术语 | 定义 |
|------|------|
| **kairos_editor_claw** | 本项目的后台守护进程，类比 OpenClaw 的 Gateway |
| **Channel Bridge** | 消息平台适配器（WeChat Bridge / Feishu Bridge），负责收发平台消息并归一化 |
| **Agent Router** | 将归一化后的消息路由到 AI 编程 Agent 的中间层 |
| **AI Coding Agent** | 实际执行代码操作的 AI 助手（可以是 Zed Agent、Claude Code CLI、或自定义 agent） |
| **Message Envelope** | 归一化消息格式，屏蔽不同平台的差异 |
| **Session** | 一次远程控制会话，维护消息上下文和工具调用历史 |

## 1.3 需求全景

### 功能需求

#### R1：多平台消息接入

用户可以通过以下平台向本机 AI Agent 发送消息：

- **R1.1 微信接入**：接收微信消息（个人号或企业微信），将文本/图片/文件消息路由到 Agent
- **R1.2 飞书接入**：接收飞书消息，将文本/图片/文件消息路由到 Agent

#### R2：消息路由与归一化

- **R2.1 消息归一化**：不同平台的原始消息格式各异（微信 XML / 飞书 JSON），需归一化为统一的 Message Envelope
- **R2.2 Session 管理**：支持按来源（DM / 群聊）、按平台隔离会话上下文
- **R2.3 多轮对话**：维护会话历史，支持上下文连贯的多轮交互

#### R3：AI Agent 集成

- **R3.1 Agent 调用**：将归一化后的消息发送给 AI Agent，触发代码操作
- **R3.2 结果返回**：将 Agent 的执行结果（代码变更、命令输出、错误信息）返回给消息发送者
- **R3.3 安全审批**：对危险操作（`rm -rf`、`git push --force` 等）需要用户确认

#### R4：后台守护进程

- **R4.1 Daemon 生命周期**：作为后台服务运行，支持启动/停止/重启/状态查询
- **R4.2 进程守护**：崩溃自动重启（通过 launchd / systemd）
- **R4.3 日志系统**：结构化日志，支持按级别过滤和持久化

#### R5：权限与安全

- **R5.1 用户认证**：只有授权用户可以发送指令（白名单 / pairing 机制）
- **R5.2 操作权限**：按危险级别分级控制（读取 / 写入 / 执行 / 网络）
- **R5.3 密钥管理**：API Key、Token 等敏感信息的安全存储

### 非功能需求

#### N1：可靠性

- 守护进程应 7×24 稳定运行，崩溃后 5 秒内自动重启
- 消息不应丢失（at-least-once 传递语义）

#### N2：响应时延

- 消息从 IM 平台到达 → Agent 开始处理：< 3 秒（P95）
- Agent 处理结果返回 IM 平台：流式输出，首 token < 5 秒

#### N3：可扩展性

- Channel Bridge 应支持插件化，新增消息平台无需修改核心代码
- Agent Backend 应支持多种 AI Agent（Zed Agent / Claude Code / 自定义）

#### N4：安全性

- 不在代码中硬编码任何密钥
- 所有网络通信（如有）使用 TLS
- 进程以最小权限运行

## 1.4 用户故事

### US1：远程代码编写

> 作为开发者，我在通勤路上想到一个 bug 修复方案，通过微信向本机 AI Agent 发送 "在 kairos_engine/src/render/pipeline.rs 中修复 VkFormat 映射错误"，AI Agent 自动定位文件、修改代码、运行测试，并将结果告诉我。

### US2：远程代码提交

> 作为开发者，我在会议中需要紧急提交一个 hotfix，通过飞书发送 "git add -A && git commit -m 'hotfix: ...' && git push"，AI Agent 执行后返回提交 SHA 和推送状态。

### US3：远程调试

> 作为开发者，我在外面时收到 CI 构建失败通知，通过微信向 AI Agent 发送 "拉取最新代码，查看编译错误并修复"，AI Agent 执行并返回修复结果。

### US4：多轮对话

> 作为开发者，我通过飞书与 AI Agent 进行多轮对话：
> - "看看最近的 commit 历史"
> - "第三个 commit 的改动有什么问题？"
> - "修复它并创建 PR"
> Agent 在上下文中理解每一轮指令。

### US5：安全确认

> 作为开发者，当我发送危险指令（如 `git push --force origin main`）时，AI Agent 应该在执行前向我确认，防止误操作。

## 1.5 已确认决策

### D-001：Zed Agent 集成方案 — Fork Zed 最小 IPC 扩展

**决策**：**Fork Zed**，在其已有的 CLI IPC 基础设施上添加最小扩展。

**关键洞察**：Zed 已有完整的 IPC 基础设施（Unix Domain Socket + `CliRequest`/`CliResponse` 枚举 + 握手协议），无需从零搭建。Fork 只需：

1. `CliRequest` 枚举新增 `PromptAgent { message, session_id }` variant
2. Zed 主进程收到后路由到 Agent Panel 的 `prompt()` 方法
3. `CliResponse` 新增 `AgentResponse` variant（流式返回结果）
4. CLI 新增 `zed agent send "message"` 子命令

**预估改动量**：~200 行 Rust，集中在 `crates/cli/` 和 agent 路由层。

**首次验证策略**：可先用临时 IPC 方案（如 Unix Socket 直连）跑通流程，确认可行后再正式 Fork。

**理由**：
- Zed 是日常主力编辑器，Agent Panel + Skills 体系完善
- IPC 基础设施已存在，改动量极小
- 可以向上游贡献（提 PR 回 Zed 主仓库）

---

### D-002：AI Agent 后端架构 — 可插拔后端（Pluggable Backend）

**决策**：`kairos_editor_claw` 的 Agent 后端采用**可插拔架构**，支持通过配置文件切换不同后端实现。

**设计**：

```
┌─────────────────────────────────────────────┐
│              kairos_editor_claw              │
│                                              │
│  ┌──────────┐  ┌────────────────────────┐   │
│  │ Channel  │  │    Agent Backend Router │   │
│  │ Bridges  │→│    (trait 派发)          │   │
│  │ WeChat   │  │                         │   │
│  │ Feishu   │  │  ┌───────────────────┐  │   │
│  │ ...      │  │  │ ZedBackend        │  │   │
│  └──────────┘  │  │ (Fork Zed IPC)    │  │   │
│                │  ├───────────────────┤  │   │
│                │  │ ClaudeCodeBackend │  │   │
│                │  │ (CLI subprocess)  │  │   │
│                │  ├───────────────────┤  │   │
│                │  │ CustomAgentBackend│  │   │
│                │  │ (HTTP/LLM direct) │  │   │
│                │  └───────────────────┘  │   │
│                └────────────────────────┘   │
└─────────────────────────────────────────────┘
```

**Backend trait（Rust）**：

```rust
#[async_trait]
trait AgentBackend: Send + Sync {
    /// 发送 prompt，返回流式响应
    async fn send_prompt(
        &self,
        session_id: &str,
        message: &str,
    ) -> Result<BoxStream<AgentEvent>>;

    /// 中断当前运行
    async fn cancel(&self, session_id: &str) -> Result<()>;

    /// 后端健康检查
    async fn health_check(&self) -> Result<BackendStatus>;
}
```

**配置文件示例**：

```toml
[agent]
backend = "zed"  # zed | claude-code | custom

[agent.zed]
# Zed Fork 的 CLI 路径
binary_path = "/usr/local/bin/zed"
# Fork Zed 暴露的 IPC socket 路径
ipc_socket = "/tmp/zed-agent.sock"

[agent.claude-code]
# Claude Code CLI 路径
binary_path = "/usr/local/bin/claude"
model = "claude-sonnet-4-6"

[agent.custom]
# 自定义 HTTP endpoint
endpoint = "http://localhost:11434/v1/chat"
api_key_env = "CUSTOM_LLM_API_KEY"
```

**首批实现**：**ZedBackend**（主）+ **ClaudeCodeBackend**（备），CustomAgentBackend 后续按需添加。

**理由**：
- 用户不使用 Claude Code CLI，Zed 是主力工具 → ZedBackend 为第一优先级
- 可插拔设计保证未来灵活性（换个编辑器 / 换 AI 提供商不需要重写整个系统）
- trait 抽象层约 50 行，架构成本极低

---

### D-003：守护进程实现语言

**决策**：**Rust**。

**理由**：
- 与 KairosEngine 技术栈完全一致，复用项目已有的 `tokio`、`serde`、`toml` 等依赖
- 单二进制部署，无运行时依赖（对比 Node.js 需安装 Node 24+）
- launchd/systemd 集成直接使用系统 API
- Zed Fork 部分也是 Rust，技术统一降低认知负担

**候选方案被否原因**：
- Node.js：需要额外运行时，且本项目无 JS/TS 基础设施
- Go：并发模型好但引入第二语言增加维护成本

---

### D-004：消息存储与状态管理

**决策**：**SQLite**（与 OpenClaw 一致）+ **TOML 配置文件**。

- **SQLite**：会话历史、transcript 记录、认证 token
- **TOML 文件**：用户可编辑的配置（`~/.kairos-claw/config.toml`），与 KairosEngine 项目配置格式一致

---

## 1.6 决策汇总（全部已锁定）

| # | 决策 | 结论 |
|---|------|------|
| D-001 | Zed Agent 集成方案 | Fork Zed 最小 IPC 扩展 |
| D-002 | AI Agent 后端架构 | 可插拔，ZedBackend 为主 |
| D-003 | 实现语言 | Rust（tokio） |
| D-004 | 消息存储 | SQLite + TOML 配置 |
| D-005 | 消息平台策略 | 飞书优先，微信后续 |
| D-006 | 用户模型 | 单用户 |
| D-007 | 目标平台 | macOS + Windows 为主，Linux 支持 |
| **D-008** | **消息类型范围** | **Phase 1 仅纯文本，枚举预留 Image/File variant** |
| **D-009** | **安全审批粒度** | **两级：读/写/编译直接放行，危险操作需飞书确认** |

### D-005：消息平台接入方式（待确认）

**问题**：如何在本地机器上接收微信/飞书消息？

**候选方案**：

**微信**：
| 方案 | 难度 | 可靠性 | 说明 |
|------|------|--------|------|
| 企业微信机器人 | 低 | ✅ 高 | Webhook 接收消息，需企业微信管理员权限 |
| 个人微信 IPad/Mac 协议 | 高 | ⚠️ 中 | `wechaty` + Puppet，非官方协议有封号风险 |
| 微信测试号 | 中 | ⚠️ 中 | 功能受限，不适合生产使用 |

**飞书**：
| 方案 | 难度 | 可靠性 | 说明 |
|------|------|--------|------|
| 飞书开放平台 Bot | 低 | ✅ 高 | Webhook + 事件订阅，需创建飞书应用 |
| 飞书自定义应用 | 中 | ✅ 高 | Long Polling 接收消息，功能完整 |

---

### D-006：用户模型 — 单用户

**决策**：**单用户**。kairos_editor_claw 只有一个可信操作者（机器 owner）。

**影响**：
- 认证模型简化为白名单 + pairing code，不需要 RBAC / 多租户隔离
- Session 管理只需按来源（DM / 群聊 / 平台）隔离，不需要按用户隔离
- 与 OpenClaw 的 trust model 一致

---

### D-007：目标平台

**决策**：**macOS + Windows 为主，Linux 支持**。

**影响**：
- 进程守护：macOS（launchd）、Linux（systemd）、Windows（Windows Service）
- Zed Fork：macOS 优先，Windows 和 Linux 跟随
- 安装/部署：需要三平台的安装脚本或包

---

## 1.7 调研资料索引

| 文档 | 主题 |
|------|------|
| `docs/research/claw-tools-architecture.md` | OpenClaw 架构深入分析（801 行） |
| `docs/research/zed-ai-agent-integration.md` | Zed AI Agent 集成方案调研 |
