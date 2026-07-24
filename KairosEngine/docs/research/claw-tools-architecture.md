# OpenClaw / "Claw" Tools — Architecture & Internals Research Report

> **Date:** 2026-07-24
> **Status:** Complete
> **Sources:** [github.com/openclaw/openclaw](https://github.com/openclaw/openclaw), [docs.openclaw.ai](https://docs.openclaw.ai), [openclaw.ai](https://openclaw.ai)

---

## Table of Contents

1. [What is OpenClaw?](#1-what-is-openclaw)
2. [Technical Architecture Overview](#2-technical-architecture-overview)
3. [Message Routing & Channel Bridges](#3-message-routing--channel-bridges)
4. [AI Agent Integration](#4-ai-agent-integration)
5. [Daemon/Background Process Model](#5-daemonbackground-process-model)
6. [Security Model](#6-security-model)
7. [Alternatives and Related Tools](#7-alternatives-and-related-tools)
8. [Key Takeaways for KairosEngine](#8-key-takeaways-for-kairosengine)

---

## 1. What is OpenClaw?

### Definition

**OpenClaw** is an open-source, self-hosted **personal AI assistant** framework. It runs as a long-lived daemon on your own devices (macOS, Linux, Windows), giving you an AI agent that you can talk to through the messaging platforms you already use (WhatsApp, Telegram, Discord, Slack, Signal, iMessage, WeChat, Feishu, and 15+ others).

- **Repository:** [github.com/openclaw/openclaw](https://github.com/openclaw/openclaw)
- **Website:** [openclaw.ai](https://openclaw.ai)
- **Docs:** [docs.openclaw.ai](https://docs.openclaw.ai)
- **Governance:** OpenClaw Foundation (non-profit), created by Peter Steinberger ([steipete.me](https://steipete.me))
- **Stars:** ~384k, **Forks:** ~80.7k
- **License:** MIT
- **Runtime:** Node.js 24.15+ (recommended), or Node 22.22.3+/25.9+
- **Distributed as:** npm package (`openclaw`), companion apps (macOS, iOS, Android, Windows Hub)

### Core Philosophy

OpenClaw is a **local-first, personal, single-user AI assistant**. It is NOT a multi-tenant SaaS platform. The design assumes one trusted operator per Gateway host:

- **The Gateway is the control plane** — all messaging surfaces, AI model connections, and tool execution flow through one long-lived process.
- **The product is the assistant** (named "Molty", a space lobster 🦞), not the Gateway itself.
- **Bring your own model** — supports OpenAI (ChatGPT/Codex), Anthropic, Google, OpenRouter, and self-hosted providers.
- **Run it where you want** — on your laptop, a home server, or a VPS.

### What Makes It Different

Most AI coding assistants (Copilot, Cursor, Claude Code) are specialized for **code editing inside an IDE**. OpenClaw is a **general-purpose messaging-bot assistant** that operates across **social/chat platforms**, not inside an editor. Its primary interaction surface is chat (DM + groups), with code execution, file access, and browser control as **tools** the AI can call.

---

## 2. Technical Architecture Overview

### High-Level System Architecture

```
┌────────────────────────────────────────────────────────────────────────────┐
│                           OpenClaw System                                   │
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────┐      │
│  │                        GATEWAY (Daemon)                           │      │
│  │                                                                    │      │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐         │      │
│  │  │Telegram  │  │ WhatsApp │  │ Discord  │  │  Slack   │  ...    │      │
│  │  │grammY    │  │ Baileys  │  │discord.js│  │@slack/   │         │      │
│  │  │long poll │  │WebSocket │  │ Gateway  │  │ web-api  │         │      │
│  │  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘         │      │
│  │       │              │             │             │                │      │
│  │       └──────────────┴─────────────┴─────────────┘                │      │
│  │                          │                                        │      │
│  │               ┌──────────▼──────────┐                             │      │
│  │               │   Message Router    │                             │      │
│  │               │ (session resolution)│                             │      │
│  │               └──────────┬──────────┘                             │      │
│  │                          │                                        │      │
│  │               ┌──────────▼──────────┐                             │      │
│  │               │  Agent Loop (RPC)   │                             │      │
│  │               │ prompt → LLM → tool │                             │      │
│  │               └──────────┬──────────┘                             │      │
│  │                          │                                        │      │
│  │     ┌────────────────────┼────────────────────┐                   │      │
│  │     │                    │                    │                   │      │
│  │  ┌──▼──┐  ┌──────────┐  ┌▼─────────┐  ┌──────▼──────┐          │      │
│  │  │LLMs │  │  Tools   │  │ Sandbox  │  │   Plugins   │          │      │
│  │  │APIs │  │read/write│  │ (Docker/ │  │ (channels,  │          │      │
│  │  │     │  │exec/edit │  │  SSH/    │  │  skills,    │          │      │
│  │  │     │  │browser.. │  │ OpenShell│  │  hooks)     │          │      │
│  │  └─────┘  └──────────┘  └──────────┘  └─────────────┘          │      │
│  │                                                                    │      │
│  │  ┌──────────────────────────────────────────────────────────┐     │      │
│  │  │              WebSocket Server (port 18789)                │     │      │
│  │  │   type: req|res|event, JSON payloads, mandatory handshake │     │      │
│  │  └──────────────────────────────────────────────────────────┘     │      │
│  └────────────────────────────────────────────────────────────────────┘      │
│                                    │                                          │
│          ┌────────────────────────┼────────────────────────┐                 │
│          │                        │                        │                 │
│     ┌────▼─────┐          ┌───────▼──────┐         ┌──────▼──────┐          │
│     │ CLI Tool │          │  macOS App   │         │ Control UI  │          │
│     │openclaw  │          │ menu bar +   │         │HTTP on      │          │
│     │ agent/   │          │ WebSocket    │         │127.0.0.1:   │          │
│     │ message  │          │ client       │         │18789        │          │
│     └──────────┘          └──────────────┘         └─────────────┘          │
│                                                                             │
│     ┌──────────┐          ┌──────────────┐         ┌──────────────┐         │
│     │iOS Node  │          │ Android Node │         │headless Node │         │
│     │WS client │          │ WS client    │         │ WS client    │         │
│     │role:node │          │ role:node    │         │ role:node    │         │
│     └──────────┘          └──────────────┘         └──────────────┘         │
└────────────────────────────────────────────────────────────────────────────┘
```

### Component Breakdown

| Component | Role | Technology |
|-----------|------|------------|
| **Gateway (Daemon)** | Long-lived process owning all messaging surfaces, AI model connections, tool execution, and WebSocket API | Node.js, TypeScript |
| **WebSocket Server** | Multiplexes HTTP + WS on one port (default `18789`). Control plane for all clients, nodes, and WebChat | Custom protocol over WS |
| **Agent Loop** | Embedded runtime: prompt assembly → LLM call → tool execution → streaming output → persistence | Node.js, SQLite (via `node:sqlite`) |
| **Message Router** | Routes inbound messages to sessions based on channel, peer, group, and `dmScope` config | In-process routing |
| **Channel Plugins** | Bridge between messaging platforms and the Gateway's internal envelope | Bundled + external plugins |
| **Sandbox Backend** | Isolated execution environment for tools (Docker, SSH, OpenShell) | Docker daemon, SSH |
| **Skills System** | Markdown instruction files (SKILL.md) that teach the agent tool usage | YAML frontmatter + Markdown |
| **Plugin System** | Extensible hooks into agent lifecycle, gateway pipeline, and channel contracts | Plugin manifest + hooks API |
| **State Store** | Per-agent SQLite databases for session rows, transcripts, auth profiles, and runtime metadata | SQLite (`node:sqlite`) |
| **Config System** | JSON5 config (`openclaw.json`) with hot-reload, `$include`, env var substitution, SecretRef providers | JSON5, file watcher |

### Key Architectural Invariants

1. **Exactly one Gateway** controls a single WhatsApp (Baileys) session per host.
2. **WebSocket handshake is mandatory** — any non-JSON or non-connect first frame is a hard close.
3. **Events are not replayed** — clients must refresh on gaps.
4. **Config is the single source of truth** — not a database, not an admin UI.
5. **The Gateway process stays on the host** — only tool execution moves into sandboxes when enabled.

---

## 3. Message Routing & Channel Bridges

### Message Flow: End-to-End

```
External Platform          Channel Plugin          Gateway Core           Agent
─────────────────          ──────────────          ─────────────          ─────
                           ┌──────────┐
WhatsApp ───WebSocket────► │ Baileys  │──► normalize into ──► route to ──► agent
         (WhatsApp Web)    │ plugin   │    shared envelope    session       loop
                           └──────────┘
                                          
Telegram ──long polling───► ┌──────────┐
         or webhook         │ grammY   │──► normalize into ──► route to ──► agent
                           │ built-in │    shared envelope    session       loop
                           └──────────┘
                                          
Discord ───Gateway────────► ┌──────────┐
                            │discord.js│──► normalize into ──► route to ──► agent
                            │ built-in │    shared envelope    session       loop
                            └──────────┘
```

### Channel Plugin Architecture

OpenClaw uses a **channel plugin contract** that separates core from platform-specific code:

1. **Built-in channels** ship in the core repo: Telegram (grammY), Discord (discord.js), Slack, Signal (signal-cli via HTTP JSON-RPC), iMessage (imsg via stdio JSON-RPC), WhatsApp (Baileys, shipped as bundled external plugin).

2. **External channel plugins** are distributed as separate npm packages:
   - **WeChat**: `@tencent-weixin/openclaw-weixin` — maintained by Tencent Weixin team
   - **Feishu (Lark)**: external plugin using Feishu's API
   - Others: LINE, Mattermost, Nextcloud Talk, Nostr, Synology Chat, Tlon, Twitch, Zalo, QQ

3. **Plugin lifecycle:**
   ```
   openclaw plugins install → Gateway discovers manifest → loads plugin entrypoint
   → registers channel id → openclaw channels login → QR/oauth flow
   → plugin stores credentials under ~/.openclaw/credentials/
   → Gateway starts → plugin starts monitor for each configured account
   → inbound messages normalized through channel contract → routed to agent
   → agent reply → sent back through plugin outbound path
   ```

### Message Normalization

All inbound messages, regardless of source platform, are normalized into a **shared inbound envelope**:

- **Metadata:** sender ID, channel ID, account ID, chat type (direct/group/channel)
- **Content:** text body, media placeholders (`<media:image>`, `<media:video>`, etc.)
- **Context:** reply metadata (quoted message body, sender, stanza ID), thread/topic IDs
- **Security markers:** mention detection flags, authorization status

### Session Routing Logic

| Source | Session Key Pattern |
|--------|---------------------|
| Direct messages | `agent:<agentId>:main` (default) or `agent:<agentId>:<channel>:dm:<peerId>` (isolated) |
| Group chats | `agent:<agentId>:<channel>:group:<groupId>` |
| Forum topics | `agent:<agentId>:<channel>:group:<groupId>:topic:<threadId>` |
| Cron jobs | Fresh session per run: `agent:<agentId>:cron:<jobId>:<runId>` |
| Webhooks | Isolated per hook: `agent:<agentId>:hook:<hookId>` |
| Channels/newsletters | `agent:<agentId>:<channel>:channel:<jid>` |

`session.dmScope` controls DM isolation:
- `main` (default): All DMs share one session
- `per-channel-peer`: Isolate by channel + sender (recommended for multi-user)
- `per-account-channel-peer`: Split further by account
- `per-peer`: Isolate by sender across channels

### Bridge Authentication Patterns

Different channels use different auth mechanisms:

| Channel | Auth Method | Transport |
|---------|-------------|-----------|
| **Telegram** | Bot token (BotFather) | Long polling (grammY) or webhook |
| **WhatsApp** | QR code linking (Baileys Web) | WebSocket to WhatsApp Web |
| **Discord** | Bot token | Gateway Intents (WebSocket) |
| **Slack** | Bot token + Socket Mode | WebSocket |
| **Signal** | signal-cli daemon | HTTP JSON-RPC + SSE events |
| **iMessage** | imsg rpc child process | stdio JSON-RPC (line-delimited) |
| **WeChat** | QR login via Tencent iLink API | External plugin managed |
| **Feishu** | App credentials | External plugin API |

### RPC Patterns for External CLIs

Two integration patterns are documented (from [docs.openclaw.ai/reference/rpc](https://docs.openclaw.ai/reference/rpc)):

**Pattern A: HTTP daemon (e.g., signal-cli)**
- signal-cli runs as a daemon with JSON-RPC over HTTP
- Event stream via SSE (`/api/v1/events`)
- Health probe via `/api/v1/check`
- Gateway owns lifecycle when `transport.kind="managed-native"`

**Pattern B: stdio child process (e.g., imsg)**
- Gateway spawns `imsg rpc` as a child process
- JSON-RPC is line-delimited over stdin/stdout
- No TCP port, no daemon required
- Core methods: `watch.subscribe`, `watch.unsubscribe`, `send`, `chats.list`

### Reply Routing

**Routing is deterministic**: Telegram inbound replies go back to Telegram. The AI model does not pick the output channel. This is a key security invariant. Message-tool sends use explicit channel/account/target parameters.

---

## 4. AI Agent Integration

### Agent Runtime

OpenClaw ships one **embedded agent runtime** — a built-in agent loop, tool wiring, and prompt assembly. It is NOT a separate harness process. Each configured agent has its own workspace, bootstrap files, and session store.

```
                    ┌─────────────┐
   Inbound ───────► │  Entrypoint │
   Message          │ agent RPC   │
   (from router)    └──────┬──────┘
                           │
               ┌───────────▼───────────┐
               │  Queue & Concurrency  │
               │  per-session lane     │
               │  (steer/followup/     │
               │   collect/interrupt)  │
               └───────────┬───────────┘
                           │
               ┌───────────▼───────────┐
               │  Workspace & Session  │
               │  - resolve workspace  │
               │  - load skills snapshot│
               │  - inject bootstrap   │
               │  - acquire write lock │
               └───────────┬───────────┘
                           │
               ┌───────────▼───────────┐
               │  Prompt Assembly      │
               │  - base system prompt │
               │  - skills XML block   │
               │  - bootstrap context  │
               │  - conversation history│
               └───────────┬───────────┘
                           │
               ┌───────────▼───────────┐
               │  Model Resolution     │
               │  provider/model       │
               │  auth profile         │
               │  thinking/verbose     │
               └───────────┬───────────┘
                           │
               ┌───────────▼───────────┐
               │  LLM Call             │
               │  (streaming)          │
               └───────────┬───────────┘
                           │
               ┌───────────▼───────────┐
               │  Tool Execution       │
               │  read/write/edit/exec │
               │  browser/canvas/...   │
               │  (sandboxed if config)│
               └───────────┬───────────┘
                           │
               ┌───────────▼───────────┐
               │  Reply Assembly       │
               │  - filter NO_REPLY    │
               │  - deduplicate sends  │
               │  - shape final payload│
               └───────────┬───────────┘
                           │
               ┌───────────▼───────────┐
               │  Persistence          │
               │  SQLite session rows  │
               │  transcript JSONL     │
               │  audit ledger         │
               └───────────┬───────────┘
                           │
               Channel ◄───┘  (deliver to messaging platform)
```

### Agent Loop Details

The agent run sequence (from [docs.openclaw.ai/concepts/agent-loop](https://docs.openclaw.ai/concepts/agent-loop)):

1. **`agent` RPC** validates params, resolves session (`sessionKey`/`sessionId`), persists metadata, returns `{runId, acceptedAt}` immediately.

2. **`agentCommand`** runs the turn:
   - Resolves model + thinking/verbose/trace defaults
   - Loads skills snapshot
   - Calls `runEmbeddedAgent`

3. **`runEmbeddedAgent`:**
   - Serializes runs via per-session and global queues
   - Resolves model + auth profile
   - Builds OpenClaw session
   - Subscribes to runtime events
   - Streams assistant/tool deltas
   - Enforces run timeout (default: 48h, abort on expiry)

4. **`subscribeEmbeddedAgentSession`** bridges runtime events:
   - Tool events → `stream: "tool"`
   - Assistant deltas → `stream: "assistant"`
   - Lifecycle events → `stream: "lifecycle"` (`phase: "start" | "end" | "error"`)

### Queueing and Concurrency

- Runs are **serialized per session key** (session lane)
- Optional **global lane** prevents tool/session races
- Messaging channels choose a queue mode: **steer** (default, inject into active run), **followup** (wait for next turn), **collect** (batch), **interrupt** (abort active run)
- Transcript writes protected by a **process-aware file-based write lock** (60s default timeout)

### Prompt Assembly

The system prompt is built from:
- OpenClaw's base prompt
- Skills prompt (compact XML block from eligible skills)
- Bootstrap context files (`AGENTS.md`, `SOUL.md`, `TOOLS.md`, `IDENTITY.md`, `USER.md`, `BOOTSTRAP.md`, `MEMORY.md`)
- Per-run overrides
- Conversation history (auto-compacted when approaching token limits)

Model-specific limits and compaction reserve tokens are enforced. The skills block format:

```xml
<available_skills>
  <skill>
    <name>image-lab</name>
    <description>Generate or edit images via a provider-backed image workflow</description>
    <location>workspace</location>
  </skill>
  ...
</available_skills>
```

Per-skill token cost: ~97 chars + name/description/location lengths (≈24 tokens per skill before field lengths).

### Skills System

Skills are markdown instruction files (`SKILL.md`) with YAML frontmatter:

```markdown
---
name: image-lab
description: Generate or edit images via a provider-backed image workflow
metadata:
  openclaw:
    requires: { bins: ["uv"], env: ["GEMINI_API_KEY"] }
---
When the user asks to generate an image, use the `image_generate` tool...
```

Loading order (highest precedence first):
1. Workspace skills (`<workspace>/skills`)
2. Project agent skills (`<workspace>/.agents/skills`)
3. Personal agent skills (`~/.agents/skills`)
4. Managed/local (`~/.openclaw/skills`)
5. Bundled (shipped with install)
6. Extra directories + plugin skills

Skills are **snapshotted at session start** and reused for all turns. They refresh mid-session when watcher detects `SKILL.md` changes or a new node connects.

### Model Integration

- **Model refs** use `provider/model` format (e.g., `anthropic/claude-sonnet-4-6`)
- **Auth profiles** support rotation and fallbacks
- **Multiple provider support:** OpenAI, Anthropic, Google, OpenRouter, self-hosted (vLLM, Ollama, SGLang), Codex app-server
- **HTTP transport** for cloud providers, **stdio child process** for Codex CLI
- **Thinking levels:** off, low, medium, high (controls reasoning budget)
- **Block streaming** emits partial replies on `text_end` or `message_end` boundaries
- **Streaming modes:** partial (live edits), block (discrete chunks), progress (status draft + final)

### Built-in Tools

Core tools always available (subject to tool policy):
- `read`, `write`, `edit`, `apply_patch` — filesystem operations
- `exec`, `process` — shell command execution
- `browser` — Puppeteer/Playwright-based web browser control
- `canvas` — agent-editable HTML/CSS/JS workspace
- `nodes` — paired device control
- `cron` — scheduled job creation
- `gateway` — config inspection (read-only)
- `sessions_list`, `sessions_history`, `sessions_send`, `sessions_spawn` — cross-session operations
- Channel-specific: `discord`, `slack`, `telegram`, `whatsapp` actions

### Plugin Hooks

Two hook systems (from [docs.openclaw.ai/concepts/agent-loop](https://docs.openclaw.ai/concepts/agent-loop)):

**Internal hooks (Gateway hooks):**
- `agent:bootstrap` — runs during bootstrap file building
- Command hooks: `/new`, `/reset`, `/stop` events

**Plugin hooks** (extension points in agent/tool lifecycle):

| Hook | When It Runs |
|------|-------------|
| `before_model_resolve` | Pre-session, override provider/model |
| `before_prompt_build` | After session load, inject context before submission |
| `before_agent_reply` | After inline actions, before LLM call (can claim turn) |
| `agent_end` | After completion, with final message list |
| `before_compaction` / `after_compaction` | Observe/annotate compaction cycles |
| `before_tool_call` / `after_tool_call` | Intercept tool params/results |
| `tool_result_persist` | Transform tool results before transcript write |
| `message_received` / `message_sending` / `message_sent` | Inbound and outbound message hooks |
| `session_start` / `session_end` | Session lifecycle boundaries |
| `gateway_start` / `gateway_stop` | Gateway lifecycle events |

---

## 5. Daemon/Background Process Model

### Gateway as a Long-Lived Daemon

```
┌──────────────────────────────────────────────────┐
│                Process Lifecycle                   │
│                                                    │
│  openclaw onboard --install-daemon                │
│       │                                            │
│       ▼                                            │
│  ┌─────────────┐    ┌─────────────────────┐       │
│  │  launchd    │ or │  systemd user       │       │
│  │  (macOS)    │    │  service (Linux)    │       │
│  └──────┬──────┘    └──────────┬──────────┘       │
│         │                      │                   │
│         └──────────┬───────────┘                   │
│                    │                               │
│                    ▼                               │
│         ┌─────────────────────┐                   │
│         │  Gateway Process    │                   │
│         │  (Node.js)          │                   │
│         │                     │                   │
│         │  • WebSocket server │                   │
│         │  • HTTP server      │                   │
│         │  • Channel monitors │                   │
│         │  • Config watcher   │                   │
│         │  • Cron scheduler   │                   │
│         │  • Heartbeat timer  │                   │
│         └─────────────────────┘                   │
│                                                    │
│  Operations:                                       │
│  • Start:  openclaw gateway (foreground)           │
│            openclaw gateway start (service)        │
│  • Stop:   openclaw gateway stop                   │
│  • Status: openclaw gateway status                 │
│  • Restart:openclaw gateway restart                │
│  • Logs:   openclaw logs                           │
│  • Health: openclaw health                         │
└──────────────────────────────────────────────────┘
```

### Process Supervision

- **macOS**: `launchd` user agent (`~/Library/LaunchAgents/`)
- **Linux**: `systemd` user service (`~/.config/systemd/user/`)
- **Windows**: Windows Hub companion app handles lifecycle
- Auto-restart on crash via the supervisor
- `openclaw gateway --port 18789 --verbose` for foreground/debug mode

### Config Hot-Reload

The Gateway watches `~/.openclaw/openclaw.json` and applies changes automatically without restart for most settings:

| Reload Mode | Behavior |
|-------------|----------|
| `hybrid` (default) | Hot-applies safe changes; auto-restarts for critical ones |
| `hot` | Hot-applies safe changes; warns when restart needed |
| `restart` | Restarts Gateway on any config change |
| `off` | Disables file watching; manual restart needed |

**What hot-applies:** Channels (restarts that channel subsystem), agents/models, automation (hooks/cron/heartbeat), sessions/messages, tools/skills/MCP, plugin config, UI/logging.

**What needs restart:** Gateway server settings (port, bind, auth, TLS, HTTP, push), infrastructure (discovery, browser, plugins.load/installs).

### State and Persistence

| Path | Contents |
|------|----------|
| `~/.openclaw/openclaw.json` | Main configuration (JSON5) |
| `~/.openclaw/state/openclaw.sqlite` | Shared runtime state (MCP OAuth tokens, device pairing, discovery) |
| `~/.openclaw/agents/<id>/agent/openclaw-agent.sqlite` | Per-agent: session rows, transcripts, auth profiles |
| `~/.openclaw/agents/<id>/sessions/` | Legacy JSONL transcript archives |
| `~/.openclaw/credentials/` | Channel credentials (WhatsApp creds, pairing allowlists, OAuth) |
| `~/.openclaw/skills/` | Managed/local skills |
| `~/.openclaw/workspace/` | Default agent workspace |

### Gateway Protocol (WebSocket)

The Gateway exposes a typed WebSocket API on the configured bind host (default `127.0.0.1:18789`):

**Wire Protocol:**
- Transport: WebSocket, text frames with JSON payloads
- First frame **must** be `connect`
- After handshake:
  - Requests: `{type:"req", id, method, params}` → `{type:"res", id, ok, payload|error}`
  - Events: `{type:"event", event, payload, seq?, stateVersion?}`
- Idempotency keys required for side-effecting methods (`send`, `agent`)
- Nodes include `role: "node"` plus caps/commands/permissions in `connect`
- Protocol typed via TypeBox schemas → JSON Schema → Swift model codegen

**Connection Lifecycle:**
```
Client                  Gateway
  │                        │
  ├── req:connect ────────►│
  │◄── res (ok) ──────────┤ (or error + close)
  │                        │
  │◄── event:presence ────┤ (snapshot: presence + health)
  │◄── event:tick ────────┤
  │                        │
  ├── req:agent ──────────►│
  │◄── res:agent ─────────┤ {runId, status:"accepted"}
  │◄── event:agent ───────┤ (streaming deltas)
  │◄── res:agent ─────────┤ {runId, status, summary} (final)
```

---

## 6. Security Model

### Trust Model

OpenClaw's security stance is explicitly a **personal-assistant trust model**, NOT a hostile multi-tenant security boundary.

**Design assumptions:**
- One trusted operator per Gateway host
- Gateway process and config are trusted
- `sessionKey` is a routing selector, NOT an authorization token
- For adversarial-user isolation: separate gateways + OS users/hosts

### Defense-in-Depth Layers

```
                    ┌─────────────────────────┐
Layer 1:            │  Identity (who can talk) │
DM pairing          │  • DM pairing codes      │
Allowlists          │  • allowFrom lists       │
                    │  • group allowlists      │
                    ├─────────────────────────┤
Layer 2:            │  Scope (where can act)   │
Group controls      │  • groupPolicy           │
Mention gating      │  • requireMention        │
                    │  • contextVisibility     │
                    ├─────────────────────────┤
Layer 3:            │  Capability (what tools) │
Tool policy         │  • tools.allow/deny      │
Sandboxing          │  • workspaceAccess       │
Exec approvals      │  • exec.security         │
                    ├─────────────────────────┤
Layer 4:            │  Model (last resort)     │
Prompt hardening    │  • external content wrap │
Model choice        │  • special-token strip   │
                    │  • instruction-hardened  │
                    └─────────────────────────┘
```

### DM Access Control

Default: **pairing**. Unknown senders receive a pairing code; bot ignores them until approved.

| Policy | Behavior |
|--------|----------|
| `pairing` (default) | Unknown senders get pairing code; ignored until `openclaw pairing approve` |
| `allowlist` | Only pre-approved senders; no pairing handshake |
| `open` | Anyone can DM (requires `allowFrom: ["*"]`, explicit opt-in) |
| `disabled` | All DMs ignored |

### Gateway Auth

Auth modes for WebSocket connections:
- **`token`**: shared bearer token (recommended)
- **`password`**: password (prefer env var `OPENCLAW_GATEWAY_PASSWORD`)
- **`trusted-proxy`**: identity-aware reverse proxy passes auth via headers
- **`none`**: private-ingress only

The Gateway fails-closed: with no auth path configured, all WebSocket connections are refused.

Tailscale Serve identity integration: accepts `tailscale-user-login` header, verifies identity via `tailscale whois` against `x-forwarded-for`.

### Device Pairing

- All WS clients include **device identity** on `connect`
- New devices require pairing approval → Gateway issues **device token**
- Local loopback auto-approved for UX smoothness
- Non-local connects always require explicit approval
- All connects must sign `connect.challenge` nonce (signature binds `platform` + `deviceFamily`)

### Sandboxing

Three backends for tool execution isolation:

| Backend | Where | Setup |
|---------|-------|-------|
| **Docker** (default) | Local container | `scripts/sandbox-setup.sh` |
| **SSH** | Any SSH-accessible host | SSH key + target host |
| **OpenShell** | Managed sandbox | OpenShell plugin |

**Modes:** `off` (default), `non-main` (sandbox all but main session), `all` (every session sandboxed).

**Scopes:** `agent` (one container per agent), `session` (one per session), `shared` (one shared).

**Docker defaults:** `network: "none"` (no egress), `readOnlyRoot: true`, `capDrop: ["ALL"]`, image `openclaw-sandbox:bookworm-slim`.

**Workspace access:** `none` (isolated sandbox workspace), `ro` (read-only mount at `/agent`), `rw` (read/write at `/workspace`).

### Prompt Injection Defense

Model choice matters: "Prompt-injection resistance is not uniform across model tiers."

Defense layers:
1. **Identity first**: lock down DMs (pairing/allowlists)
2. **Scope next**: mention gating, tool policy, sandboxing
3. **Model last**: instruction-hardened models for tool-enabled agents
4. **External content wrapping**: `<<<EXTERNAL_UNTRUSTED_CONTENT ...>>>` boundary markers
5. **Special-token stripping**: removes chat-template tokens from external content

Weak models + tools = high risk. Strong recommendation: "Do not run tool-enabled agents on weak model tiers."

### Secret Management

- Provider credentials blocked from workspace `.env` files
- `OPENCLAW_*` namespace reserved for runtime (blocked from workspace `.env`)
- SecretRef providers: `env`, `file`, `exec`
- Secrets stored in `openclaw.json`, `credentials/`, `state/openclaw.sqlite`, per-agent SQLite
- File permissions: `600` on files, `700` on dirs
- `openclaw security audit` checks for misconfigurations
- `openclaw doctor --fix` applies safe remediations

---

## 7. Alternatives and Related Tools

### Direct Competitors (Messaging-Bot AI Assistants)

OpenClaw is relatively unique as an open-source, self-hosted, multi-channel AI assistant. Direct alternatives include:

| Tool | Type | Key Differences |
|------|------|-----------------|
| **OpenClaw** | Self-hosted multi-channel AI assistant | Single Gateway, 25+ channels, local-first, MIT license |
| **ChatGPT** (official) | Cloud SaaS | No self-hosting, limited channel integrations |
| **Claude** (official) | Cloud SaaS | No self-hosting, API-only integration |
| **Various Telegram/Discord bots** | Fragmented ecosystem | Usually single-channel, no tool execution, no sandboxing |

### Coding AI Assistants (Different Category)

These are **code-focused** assistants that operate inside an IDE, NOT messaging platforms:

| Tool | Architecture | Relevance |
|------|-------------|-----------|
| **Claude Code** (Anthropic) | Terminal-based agent with MCP tool support, file access, shell execution | Shares "agent loop" concept but inside terminal, not messaging |
| **Cursor Agent Mode** | IDE-integrated, uses LSP + codebase indexing, terminal access | Focused on code editing, not messaging |
| **GitHub Copilot** | IDE-integrated, code completion + chat | Agent mode (2025+) adds file/tool access, but IDE-only |
| **Continue.dev** | Open-source IDE plugin, pluggable LLM backends, MCP support | IDE-focused, similar MCP integration pattern |
| **Cline** (VS Code) | Autonomous coding agent in VS Code, terminal + file access | Shares tool-execution model but IDE-only |
| **Aider** | Terminal-based, git-aware code editing | Very different architecture (git diff-based) |

### Architectural Comparisons

#### Claude Code vs OpenClaw

| Aspect | Claude Code | OpenClaw |
|--------|------------|----------|
| **Interaction surface** | Terminal CLI | Messaging platforms (25+ channels) |
| **Agent model** | Loop: prompt → tools → LLM → tools → reply | Same loop pattern |
| **Tool execution** | Host shell, file read/write, MCP servers | Host/sandbox shell, file ops, browser, canvas, plugins |
| **Session management** | Per-project, saved in files | SQLite per-agent, transcript archives |
| **Multi-turn context** | Compaction via summarization | Same compaction pattern |
| **Plugin/Skill system** | MCP servers + slash commands + hooks | Skills (SKILL.md) + plugin hooks + MCP |
| **Security** | Ask-user approval for dangerous ops | Pairing + allowlists + sandbox + exec approvals |
| **Daemon model** | Interactive CLI process | Long-lived daemon via launchd/systemd |
| **Remote access** | SSH terminal | WebSocket + Tailscale/SSH tunnel |

#### MCP Integration Comparison

Both Claude Code and OpenClaw support MCP (Model Context Protocol), but with different emphasis:

- **Claude Code**: MCP is the **primary extension mechanism** for adding tools/resources/prompts
- **OpenClaw**: MCP is **one of several extension mechanisms** (alongside skills, plugins, and channel plugins). MCP is used for OAuth-based connections and external tool servers
- **OpenClaw Gateway** can act as an MCP client connecting to external MCP servers
- **OpenClaw** stores MCP OAuth tokens in its shared SQLite database

#### Continue.dev vs OpenClaw

| Aspect | Continue.dev | OpenClaw |
|--------|-------------|----------|
| **Purpose** | IDE coding assistant | Personal AI assistant |
| **Interface** | VS Code / JetBrains sidebar | Messaging platforms |
| **LLM backends** | Pluggable (OpenAI, Anthropic, local) | Pluggable (same pattern) |
| **Extension** | MCP, slash commands, config rules | Skills, plugins, channel plugins |
| **Context** | Codebase indexing, IDE state | Conversation history, workspace files |
| **Architecture** | IDE extension process | Standalone daemon process |
| **Open source** | Yes (Apache 2.0) | Yes (MIT) |

#### Cursor Agent Mode vs OpenClaw

| Aspect | Cursor Agent Mode | OpenClaw |
|--------|-------------------|----------|
| **Purpose** | Autonomous code editing | General-purpose assistant |
| **Interface** | IDE chat + inline edits | Messaging platforms |
| **Tool access** | File system, terminal, LSP | File system, terminal, browser, canvas |
| **Planning** | Multi-step plan before execution | Single-turn with compaction |
| **Approval model** | Per-diff review | Per-exec approval, allowlists |

### Key Takeaway

OpenClaw is **category-defining** as a general-purpose, self-hosted AI assistant that operates across messaging platforms. Its closest ideological relatives are coding AI assistants (Claude Code, Cursor, Continue.dev) that share the **agent-loop-with-tools architecture**, but for a different domain (messaging vs. code editing).

The "agent loop + tools + sandbox + plugin hooks" pattern is converging across:
- **OpenClaw**: messaging assistant
- **Claude Code**: terminal coding agent
- **Cursor**: IDE coding agent
- **Continue.dev**: IDE coding agent

The architectural patterns (prompt assembly → LLM → tool execution → reply → persistence) are nearly identical; the difference is in the **interaction surface** and **tool ecosystem**.

---

## 8. Key Takeaways for KairosEngine

### Patterns Worth Studying

1. **Gateway-as-control-plane**: A single long-lived process owning all external connections (messaging, LLMs, tools) with a typed WebSocket API for clients. This is directly applicable to KairosEngine's editor server architecture.

2. **Channel plugin contract**: The separation between core (message routing, agent loop) and platform-specific code (WhatsApp, Telegram, etc.) via a channel plugin SDK is a clean pattern for any system that needs multi-platform support.

3. **Skills-as-markdown**: The SKILL.md format (YAML frontmatter + Markdown body) is a lightweight, git-friendly way to distribute agent instructions. The gating system (`requires.bins`, `requires.env`) provides dependency management without a package manager.

4. **Sandbox-as-config**: The layered sandbox model (mode × scope × backend × workspace access) is well-designed. The Docker backend defaults (`network: "none"`, `readOnlyRoot: true`, `capDrop: ["ALL"]`) represent security-conscious defaults.

5. **Session routing with dmScope**: The `dmScope` system for isolating conversation context (main/per-peer/per-channel-peer) could inform KairosEngine's multi-user editor session design.

6. **Hot-reload config with schema validation**: JSON5 config with `$include`, hot-reload, and strict validation at startup. Config changes are applied incrementally (restart only the affected subsystem, not the whole process).

7. **Plugin hooks for agent lifecycle**: The before/after hooks at key agent lifecycle points (model resolve, prompt build, tool call, reply) provide clean extension points without modifying core logic.

8. **Security as layered defense**: Identity → Scope → Capability → Model layers, with explicit "not vulnerabilities by design" documentation. The `openclaw security audit` tool with structured `checkId` codes is a good pattern.

### Patterns NOT to Emulate

1. **Single-operator trust model**: OpenClaw explicitly does NOT support multi-tenant security. For KairosEngine (a game editor with potential multi-user scenarios), this would need rethinking.

2. **Node.js dependency**: For a Rust-based engine, the Node.js runtime requirement is incompatible. However, the architectural concepts are language-agnostic.

3. **Messaging-platform focus**: KairosEngine's interaction surface is an editor GUI, not messaging platforms. The channel bridge concept may not be directly applicable.

---

## References

- [OpenClaw GitHub Repository](https://github.com/openclaw/openclaw) — primary source code
- [OpenClaw Documentation](https://docs.openclaw.ai) — official docs
- [Gateway Architecture](https://docs.openclaw.ai/concepts/architecture) — architecture overview
- [Agent Runtime](https://docs.openclaw.ai/concepts/agent) — agent workspace, bootstrap files, tools, skills
- [Agent Loop](https://docs.openclaw.ai/concepts/agent-loop) — execution cycle, queueing, streaming, timeouts
- [Session Management](https://docs.openclaw.ai/concepts/session) — routing, lifecycle, DM isolation
- [Security](https://docs.openclaw.ai/gateway/security) — threat model, DM policy, sandboxing, prompt injection
- [Sandboxing](https://docs.openclaw.ai/gateway/sandboxing) — Docker/SSH/OpenShell backends, workspace access
- [Configuration](https://docs.openclaw.ai/gateway/configuration) — config format, hot-reload, env vars, RPC
- [RPC Adapters](https://docs.openclaw.ai/reference/rpc) — signal-cli, imsg integration patterns
- [Skills](https://docs.openclaw.ai/tools/skills) — SKILL.md format, loading order, gating, ClawHub
- [Telegram Channel](https://docs.openclaw.ai/channels/telegram) — grammY integration, access control, streaming
- [WhatsApp Channel](https://docs.openclaw.ai/channels/whatsapp) — Baileys integration, multi-account, media handling
- [WeChat Channel](https://docs.openclaw.ai/channels/wechat) — external plugin model, Tencent iLink API
- [OpenClaw Website](https://openclaw.ai) — project homepage
- [VISION.md](https://github.com/openclaw/openclaw/blob/main/VISION.md) — project vision (rate-limited during research)
- [Claude Code Documentation](https://docs.anthropic.com/en/docs/claude-code) — comparison reference
- [Continue.dev](https://docs.continue.dev) — comparison reference
