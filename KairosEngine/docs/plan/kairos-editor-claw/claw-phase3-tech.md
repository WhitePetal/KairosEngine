# Kairos Editor Claw 设计案 — Phase 3：技术选型与坑点

> **状态**：✅ 完成
> **创建日期**：2026-07-24
> **前置文档**：[Phase 1 需求分析](./claw-phase1-requirements.md) | [Phase 2 功能分析](./claw-phase2-features.md)

## 3.1 技术选型总览

| 编号 | 决策点 | 选型 | 关键理由 |
|------|--------|------|----------|
| **T1** | 异步运行时 | `tokio` (full features) | Rust 生态标准，项目已有依赖 |
| **T2** | 序列化格式 | TOML（配置）+ JSON（IPC 协议） | TOML 可编辑；JSON 通用且 Zed 内部也用 |
| **T3** | SQLite 绑定 | `rusqlite` + `tokio::task::spawn_blocking` | 最成熟的 Rust SQLite 库；非异步但可桥接 |
| **T4** | 日志系统 | `tracing` + `tracing-subscriber` + `tracing-appender` | Rust 生态标准，结构化日志，与 tokio 深度集成 |
| **T5** | CLI 框架 | `clap` v4 (derive mode) | 项目已有依赖，derive 宏体验好 |
| **T6** | 配置热重载 | `notify` crate (fs watcher) | 跨平台文件监听，轻量 |
| **T7** | 消息平台接入策略 | **飞书优先（开放平台 API）+ 微信备选** | 见 3.2 坑点 1 |
| **T8** | IPC 协议（claw ↔ Zed Fork） | **Unix Socket + JSON newline-delimited** | 见 3.2 坑点 2 |
| **T9** | 流式响应 | `futures::Stream` + `async-stream` macro | 标准异步流，生成器语法简洁 |
| **T10** | 跨平台守护进程 | **条件编译 + 平台抽象层** | 见 3.2 坑点 3 |
| **T11** | Agent 子进程管理 | `tokio::process::Command` | 标准异步子进程，内置管道支持 |
| **T12** | 飞书接入方式 | **Webhook（事件订阅）+ ngrok**，不提供 Long Polling 降级 | 实时性强，实现简单；启动失败时日志报错 |

---

## 3.2 关键坑点与应对

### 坑点 1：消息平台接入策略 — 飞书优先 ⭐

**决策（T7）**：**飞书优先，微信备选**。用户主要使用飞书，且飞书开放平台 API 完善、文档齐全、有 HTTP REST API 可直接对接。

#### 飞书接入（优先，T12）

飞书开放平台提供标准 HTTP API，Rust 可直接对接，无需任何 sidecar。

**接入方式**：**Webhook（事件订阅）+ ngrok**，不提供 Long Polling 降级。

**不提供降级的理由**：
1. 延迟不可接受（Long Polling 平均 15s，最差 30s），对聊天交互场景体验太差
2. ngrok 已是开发者标配工具，`ngrok http 8899` 一行命令即可
3. 降级会引入两种模式的维护成本和 bug 面，得不偿失
4. 启动失败时输出清晰错误日志即可：

```
ERROR: 飞书 Webhook 需要公网可达的 URL。
请执行: ngrok http 8899
然后将 ngrok 提供的 https URL 配置到飞书开放平台的事件订阅中。
或在 claw 配置中填写已有的公网地址: [channels.feishu.webhook].public_url
```

**架构**：

```
┌──────────────────────────────────────────────────────────┐
│                    claw daemon                            │
│                                                           │
│  ┌─────────────┐     ┌──────────────────────────────┐    │
│  │  ngrok      │────→│  axum HTTP server (:8899)    │    │
│  │  tunnel     │     │                              │    │
│  │  (本地端口)  │     │  POST /webhook/feishu       │    │
│  └─────────────┘     │  → 验签 → 反序列化 → Router  │    │
│                      └──────────────────────────────┘    │
│                                      ↑                    │
│                         飞书服务器 (open.feishu.cn)       │
│                         推送 im.message.receive_v1 事件   │
└──────────────────────────────────────────────────────────┘
```

**配置设计**：

```toml
[channels.feishu]
enabled = true
app_id = "cli_xxxxxxxxxxxx"
app_secret = "$FEISHU_APP_SECRET"   # 从环境变量读取

[channels.feishu.webhook]
listen_port = 8899
verification_token = "xxx"          # 飞书事件订阅的 Verification Token
public_url = ""                      # 留空则需 ngrok；有公网 IP 可直接填
```

**Webhook 验证流程**（飞书要求）：
```
1. 飞书首次配置 URL 时发送 POST，body 含 { token, challenge, type: "url_verification" }
2. claw 必须 1 秒内返回 { challenge: "<收到的challenge值>" }
3. 验证通过后，后续消息正常推送
4. 启动时检测 public_url 可达性，不可达则打印错误并退出
```

**飞书 Bot 创建步骤**：
1. 飞书开放平台 → 创建企业自建应用
2. 开启机器人能力
3. 配置事件订阅（`im.message.receive_v1` 事件），填入 ngrok URL
4. 获取 App ID + App Secret
5. claw 配置中填入凭证

**飞书 API 关键端点**：
- 获取 tenant_access_token：`POST https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal`
- 发送消息：`POST https://open.feishu.cn/open-apis/im/v1/messages`
- 事件回调：claw 本地 HTTP server 接收飞书推送

#### 微信接入（备选，后续实现）

微信（个人号）没有任何官方 Rust SDK。如果需要微信支持，两种路径：

| 路径 | 可行性 | 说明 |
|------|--------|------|
| **企业微信** | ✅ 高 | Webhook + REST API 纯 HTTP，Rust 可直接对接 |
| **个人微信 + Node.js sidecar** | ⚠️ 中 | 引入 Node.js sidecar 处理微信协议，claw 通过 HTTP/stdio 与之通信 |

**实现策略**：
- Phase 1 只实现飞书 Bridge（`FeishuBridge`）
- `ChannelBridge` trait 设计保证后续微信接入无需修改核心代码
- 微信 Bridge 实现（`WeChatBridge`）标记为后续迭代

---

### 坑点 2：IPC 协议跨平台一致性

**问题**：claw (Rust daemon) 与 Zed Fork 之间的 IPC 通信需要同时支持 macOS、Linux、Windows。

**候选方案**：

| 传输层 | macOS/Linux | Windows | 复杂度 |
|--------|-------------|---------|--------|
| **Unix Domain Socket** | ✅ 原生 | ❌ 不支持 | 低（Unix 平台） |
| **Named Pipe** | ❌ 不支持 | ✅ 原生 | 需要两套实现 |
| **TCP loopback** | ✅ | ✅ | 低，但端口冲突风险 |
| **stdio (subprocess)** | ✅ | ✅ | 无法连接已运行的 Zed 实例 |

**决策（T8）**：**Unix Socket（Unix）+ Named Pipe（Windows），统一封装为 `IpcStream` trait**。

```rust
// 平台抽象
#[cfg(unix)]
type IpcStream = tokio::net::UnixStream;

#[cfg(windows)]
type IpcStream = tokio::net::windows::named_pipe::NamedPipeClient;

// 协议层（平台无关）
// JSON 序列化，newline-delimited framing
// { "type": "req", "id": 1, "method": "prompt", "params": {...} }\n
```

**序列化格式**：**JSON（newline-delimited）**，理由：
1. Zed 内部已有 JSON 序列化基础设施（`serde_json`）
2. 协议可调试（`nc -U /tmp/zed-agent.sock` 即可看到明文）
3. 性能不是瓶颈（Agent 交互是低频事件，每秒 1-10 条消息）

**协议帧格式**：

```
请求:  {"type":"req","id":1,"method":"prompt","params":{...}}\n
响应:  {"type":"res","id":1,"ok":true,"payload":{...}}\n
事件:  {"type":"event","event":"text_delta","payload":{"delta":"hello"}}\n
```

---

### 坑点 3：跨平台 daemon 守护

**问题**：三个平台的进程守护机制完全不同，且 Rust 生态对 daemon 化的支持参差不齐。

**平台差异**：

| 平台 | 守护机制 | 配置位置 | 崩溃重启 |
|------|----------|----------|----------|
| **macOS** | `launchd` (user agent) | `~/Library/LaunchAgents/com.kairos.claw.plist` | `KeepAlive: true` |
| **Linux** | `systemd` (user service) | `~/.config/systemd/user/kairos-claw.service` | `Restart=always` |
| **Windows** | Windows Service / Task Scheduler | 注册表 / XML 任务定义 | Service 自动恢复 |

**决策（T10）**：**条件编译 + 平台抽象**。不是写一个跨平台的 daemon，而是为每个平台写一个轻量级的"安装器 + 启动脚本"。

```
claw daemon install   → 写入 launchd plist / systemd unit / 创建 Windows Task
claw daemon uninstall → 删除对应配置
claw daemon start     → launchctl load / systemctl start / schtasks run
claw daemon stop      → launchctl unload / systemctl stop / schtasks end
claw daemon run       → 前台运行（调试用，所有平台通用）
```

**核心洞察**：`claw daemon run`（前台模式）是**纯 Rust 代码**，平台无关。而 `install/start/stop` 则是**轻量级的平台胶水代码**，负责写入正确格式的配置文件并调用平台命令。

---

### 坑点 4：Agent Backend 的流式响应与超时

**问题**：LLM 调用可能很长（几分钟），流式响应需要在 tokio 异步模型下正确处理超时、中断、和连接断开。

**设计**：

```rust
use tokio::time::timeout;
use futures::stream::StreamExt;

async fn run_agent_turn(
    backend: &dyn AgentBackend,
    session: &SessionContext,
    prompt: &str,
) -> Result<Vec<AgentEvent>> {
    let stream = backend.send_prompt(session, prompt).await?;

    // 整体超时：5 分钟
    let result = timeout(
        Duration::from_secs(300),
        stream.collect::<Vec<_>>()
    ).await;

    match result {
        Ok(events) => Ok(events),
        Err(_) => {
            backend.cancel(&session.session_key).await?;
            Err(anyhow::anyhow!("Agent turn timed out after 5 minutes"))
        }
    }
}
```

**注意事项**：
- `send_prompt` 返回的 `BoxStream` 必须是 cancel-safe 的
- 超时后必须调用 `cancel()` 通知后端释放资源（尤其是 ZedBackend）
- 流式输出时考虑心跳机制：超过 60 秒无任何 event 视为卡死

---

### 坑点 5：Session 并发与 SQLite 写入锁

**问题**：SQLite 在并发写入时性能很差（全局写锁）。多个 session 同时写入 transcript 记录可能导致阻塞。

**应对**：

1. **写入收敛到单 task**：所有 SQLite 写操作通过 `tokio::sync::mpsc` 发送到专用的 Writer task
2. **WAL 模式**：`PRAGMA journal_mode=WAL;` 允许并发读 + 串行写
3. **批量写入**：transcript 记录攒到一定数量（或一定时间）后批量 `INSERT`

```rust
struct DbWriter {
    tx: mpsc::UnboundedSender<DbCommand>,
}

enum DbCommand {
    AppendTranscript { session_key: String, message: ChatMessage },
    UpdateSessionMeta { session_key: String, meta: SessionMeta },
    // ...
}

// Writer task
async fn db_writer_task(mut rx: mpsc::UnboundedReceiver<DbCommand>, conn: Connection) {
    let mut batch = Vec::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(100));

    loop {
        tokio::select! {
            cmd = rx.recv() => {
                if let Some(cmd) = cmd {
                    batch.push(cmd);
                }
            }
            _ = ticker.tick() => {
                if !batch.is_empty() {
                    conn.execute_batch(&/* flush batch */);
                    batch.clear();
                }
            }
        }
    }
}
```

---

### 坑点 6：Zed Fork 的维护成本

**问题**：Fork Zed 后需要持续跟进 upstream 更新，否则可能产生合并冲突。

**应对**：

1. **最小侵入原则**：只修改必要的文件（`crates/cli/` + 一个 IPC handler），尽量不改核心逻辑
2. **独立 IPC handler 模块**：新增一个 crate（`crates/claw_ipc/`）而不是修改现有的 agent 代码
3. **定期 rebase**：每月从 upstream 拉取一次，冲突面积极小
4. **Feature gate**：所有改动在 `#[cfg(feature = "claw-ipc")]` 下，不影响正常 Zed 构建

```
zed/
├── crates/
│   ├── cli/
│   │   └── src/cli.rs              # + CliRequest::PromptAgent variant [~20 行]
│   ├── claw_ipc/                   # [新增] IPC handler（独立 crate）
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── protocol.rs         # JSON 协议定义
│   │       ├── server.rs           # Unix Socket server
│   │       └── bridge.rs           # IPC → Agent Panel 桥接
│   └── zed/
│       └── src/main.rs             # + 启动 claw_ipc server [~5 行]
```

---

### 坑点 7：飞书 Webhook 的公网可达性

**问题**：飞书 Webhook 要求回调 URL 公网可达。本地开发环境通常没有公网 IP。

**应对（T12）**：

```
启动流程:
1. 读取配置 [channels.feishu.webhook].public_url
2. 如果 public_url 非空 → 直接使用（用户已有公网 IP）
3. 如果 public_url 为空 → 检查 ngrok 是否在 PATH 中
   - 是 → 提示用户执行 `ngrok http <port>`
   - 否 → 打印错误日志，提示安装 ngrok 或配置 public_url
4. 启动 axum HTTP server 监听 :8899
5. 飞书开放平台配置事件订阅 URL = public_url/webhook/feishu
```

**不提供 Long Polling 降级**：保持实现简单，启动失败时日志报错即可。ngrok 免费版完全够用。

---

### 坑点 8：`async_trait` 的 Send + Sync 约束

**问题**：`AgentBackend` 和 `ChannelBridge` trait 使用 `#[async_trait]` 宏，返回的 `BoxStream` 需要 `Send` 约束才能在 tokio 多线程 runtime 中使用。

**应对**：

```rust
// ✅ 正确的 trait 定义
#[async_trait]
trait AgentBackend: Send + Sync {
    async fn send_prompt(
        &self,
        session: &SessionContext,
        prompt: &str,
    ) -> Result<BoxStream<'static, AgentEvent>>;
    // BoxStream 自动满足 Send（AgentEvent: Send）
}

// 使用 async-stream 宏创建流
use async_stream::stream;

fn text_stream(text: String) -> impl Stream<Item = AgentEvent> + Send {
    stream! {
        for word in text.split_whitespace() {
            yield AgentEvent::TextDelta(word.to_string());
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        yield AgentEvent::Completed { usage: UsageStats::default() };
    }
}
```

---

## 3.3 外部依赖评估

### Rust Crate 依赖

| Crate | 用途 | 版本 | 风险 |
|-------|------|------|------|
| `tokio` (full) | 异步运行时 | 1.x | ✅ 标准，已有依赖 |
| `serde` + `serde_json` | 序列化 | 1.x | ✅ 标准，已有依赖 |
| `toml` | 配置解析 | 0.8.x | ✅ 已有依赖 |
| `clap` | CLI 框架 | 4.x | ✅ 已有依赖 |
| `rusqlite` | SQLite 绑定 | 0.31+ | ⚠️ 需新增，需 libsqlite3-dev |
| `tracing` + `tracing-subscriber` + `tracing-appender` | 日志 | 0.1.x | ⚠️ 需新增 |
| `notify` | 文件监听 | 6.x | ⚠️ 需新增 |
| `async-trait` | async trait | 0.1.x | ⚠️ 需新增，稳定 |
| `async-stream` | 流生成器 | 0.3.x | ⚠️ 需新增 |
| `anyhow` + `thiserror` | 错误处理 | 1.x | ✅ 已有依赖 |
| `futures` | 流工具 | 0.3.x | ⚠️ 需新增 |
| `axum` | HTTP server（Webhook + health） | 0.7.x | ⚠️ 需新增，用于飞书 Webhook 接收 + 健康检查 |
| `reqwest` | HTTP client | 0.12.x | ⚠️ 需新增，用于调用飞书 API（发送消息、获取 token） |
| `dirs` | 系统目录 | 5.x | ⚠️ 需新增，获取 `~/.kairos-claw/` |

### 外部服务依赖

| 服务 | 用途 | 必选？ | 说明 |
|------|------|--------|------|
| 飞书开放平台 | Feishu Bridge（Webhook + 消息发送） | ✅ 优先 | 首批实现，用户主要使用平台 |
| ngrok | 内网穿透（飞书 Webhook 公网可达） | ✅ 开发期必选 | 生产环境如有公网 IP 则不需要 |
| 企业微信 API | WeChat Bridge | 🔵 后续 | 迭代中实现 |
| Anthropic / OpenAI API | LLM 调用 | ✅ 必选 | Zed Agent 或 Claude Code 都需要 |

---

## 3.4 架构影响汇总

```
T7/T12: 飞书优先 + Webhook only
 ↓   纯 Rust 实现，axum + reqwest
 ↓   启动时检测公网可达性，失败则报错退出
 ↓   微信备选（后续迭代）
 ↓
T8: Unix Socket + Named Pipe IPC
 ↓   JSON newline-delimited framing
 ↓   封装为平台无关的 IpcStream trait
 ↓
T10: 条件编译 + 平台抽象守护进程
 ↓   claw daemon run    → 纯 Rust 前台（通用）
 ↓   claw daemon install → 写入平台配置（3 套胶水代码）
 ↓
T3: SQLite + WAL 模式 + DbWriter task
 ↓   所有写操作收敛到单 task
 ↓   避免并发写入锁争用
 ↓
T6: Zed Fork 最小侵入（独立 claw_ipc crate）
 ↓   #[cfg(feature = "claw-ipc")] feature gate
 ↓   每月 rebase upstream，冲突面 < 50 行
```

---

## 3.5 技术风险矩阵

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 微信个人号协议封堵 | 中 | 高 | 飞书优先，微信仅作为备选且后续实现 |
| Zed upstream 重构 agent 代码 | 低 | 中 | 独立 crate + feature gate，冲突面小 |
| SQLite 并发写入瓶颈 | 低 | 低 | WAL + 单 task 写入 + 批量 flush |
| tokio 多线程 runtime 下的 Send 约束 | 中 | 低 | 所有类型标注 Send，编译期可发现 |
| 飞书 Webhook 需公网可达 | 中 | 中 | ngrok 作为标准方案，文档明确说明，不提供降级 |
| ngrok 免费版稳定性 | 低 | 中 | 可作为开发者工具依赖，生产环境用户自行配置公网 IP |
| Windows Service 调试困难 | 中 | 低 | `claw daemon run` 前台模式跨平台通用，调试期不依赖 Service |

---

## 3.6 决策汇总

| 编号 | 决策点 | 结论 |
|------|--------|------|
| T1 | 异步运行时 | `tokio` (full) |
| T2 | 序列化格式 | TOML（配置）+ JSON（IPC） |
| T3 | SQLite 绑定 | `rusqlite` + spawn_blocking |
| T4 | 日志系统 | `tracing` 全家桶 |
| T5 | CLI 框架 | `clap` v4 derive |
| T6 | 配置热重载 | `notify` crate |
| T7 | 消息平台策略 | 飞书优先，微信备选 |
| T8 | IPC 协议 | Unix Socket + JSON newline-delimited |
| T9 | 流式响应 | `futures::Stream` + `async-stream` |
| T10 | 跨平台守护 | 条件编译 + 平台胶水代码 |
| T11 | Agent 子进程管理 | `tokio::process::Command` |
| T12 | 飞书接入方式 | Webhook + ngrok，无 Long Polling 降级 |
