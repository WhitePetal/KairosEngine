# Zed AI Agent Integration — Research Report

> **Date**: 2026-07-24
> **Status**: Complete
> **Sources**: [Zed official docs](https://zed.dev/docs), [Zed source code (GitHub)](https://github.com/zed-industries/zed), agent.rs, cli.rs

## Table of Contents

1. [Zed AI Agent Architecture](#1-zed-ai-agent-architecture)
2. [Zed's Extension & Plugin System](#2-zeds-extension--plugin-system)
3. [Sending Messages to Zed's AI Agent from Outside](#3-sending-messages-to-zeds-ai-agent-from-outside)
4. [Zed's Workspace / Editor APIs](#4-zeds-workspace--editor-apis)
5. [Alternative Approaches](#5-alternative-approaches)
6. [Key Takeaways for KairosEngine](#6-key-takeaways-for-kairosengine)
7. [References](#7-references)

---

## 1. Zed AI Agent Architecture

### 1.1 Overview

Zed's AI agent (the "Agent Panel" / "Agent Mode") is a deep integration of AI-assisted coding directly into the editor. It goes beyond a simple chat interface by giving the LLM **tool-calling capabilities** that allow it to read, write, and execute code.

**Source**: [Zed Agent Panel docs](https://zed.dev/docs/assistant/assistant-panel)

### 1.2 Core Components (from source code)

The agent is implemented primarily in `crates/agent/src/agent.rs`. Key components:

| Component | Type | Purpose |
|-----------|------|---------|
| `NativeAgent` | `Entity<NativeAgent>` | Top-level orchestrator; owns sessions, projects, models, skills |
| `NativeAgentConnection` | Wrapper struct | Implements `AgentConnection` trait; bridges ACP protocol |
| `Thread` | `Entity<Thread>` | Internal thread processing messages, managing model turns |
| `AcpThread` | `Entity<acp_thread::AcpThread>` | ACP protocol frontend; handles UI rendering side of a session |
| `Session` | Internal struct | Holds `Thread` + `AcpThread` for a single conversation |
| `ProjectState` | Per-project state | Skills, project context, context server registry |
| `LanguageModels` | Model registry | Cached model list with authentication tracking |
| `NativeThreadEnvironment` | Environment | Provides terminal creation, subagent spawning, sibling threads |

**Source**: `crates/agent/src/agent.rs` (Zed source, GitHub `zed-industries/zed`)

### 1.3 Communication Protocol: ACP (Agent Client Protocol)

Zed uses an internal protocol called **ACP** (Agent Client Protocol) — defined in `crates/agent_client_protocol/`. This is the schema that governs how the UI panel (`AcpThread`) communicates with the backend agent (`Thread`).

Key types include:
- `acp::SessionId` — unique identifier for a conversation
- `acp::PromptRequest` — input message from user
- `acp::PromptResponse` — streamed response
- `acp::ContentBlock` — text, image, resource-link blocks
- `acp::StopReason` — EndTurn, Cancelled, MaxTokens, MaxTurnRequests, Refusal
- `acp::ToolCall` — tool invocation request
- `acp::SessionUpdate` — real-time updates (available commands, tool calls)

**Source**: `crates/agent_client_protocol/` and agent.rs

### 1.4 Tool System

The agent has a built-in tool system:

1. **Built-in tools**: Code search, file editing, terminal commands, subagent spawning
2. **MCP tools**: External tools exposed via Model Context Protocol servers
3. **Skill tools**: Custom skill invocations loaded from `~/.agents/skills/` and project-local `.agents/skills/`
4. **Sibling thread tool**: `create_thread` / `list_agents_and_models` for spawning sibling agent threads

Tools are registered per-thread via `Thread::add_default_tools()` and `Thread::add_tool()`.

### 1.5 Turn Processing Flow

```
User types message → AcpThread.prompt()
  → NativeAgentConnection.prompt() [implements AgentConnection]
    → Parse slash commands (/compact, /skill-name, /server.prompt)
    → NativeAgent.run_turn()
      → Thread.send() or Thread.resume()
        → Model API call with tool-calling
        → Stream ThreadEvents back
          → AgentText, AgentThinking, ToolCall, Stop, etc.
          → Forwarded to AcpThread for UI rendering
```

**Source**: agent.rs `impl AgentConnection for NativeAgentConnection`

### 1.6 Skills System

Zed has a sophisticated skills system that loads from:

1. **Built-in skills**: Shipped with Zed
2. **Global skills**: `~/.agents/skills/<skill-name>/SKILL.md`
3. **Project-local skills**: `.agents/skills/<skill-name>/SKILL.md` (per worktree)

Skills have:
- YAML frontmatter with `name`, `description`, `disable_model_invocation`
- A Markdown body loaded on demand (not kept in memory)
- Priority: Project-local > Global > Built-in
- Support for `/skill-name` slash commands and model-driven `skill` tool

**Source**: agent.rs skills-related functions (`combine_skills`, `apply_skill_overrides`, `select_catalog_skills`)

---

## 2. Zed's Extension & Plugin System

### 2.1 Extension Architecture

Zed extensions are Git repositories containing an `extension.toml` manifest. They can provide:
- **Languages**: Tree-sitter grammars, LSP configs
- **Themes**: JSON theme files
- **Icon Themes**: Icon sets
- **Snippets**: JSON snippet files
- **Debuggers**: Debug adapter protocol (DAP) support
- **MCP Servers**: Model Context Protocol context servers

**Source**: [Zed Extensions docs](https://zed.dev/docs/extensions)

### 2.2 Rust/WASM Extension API

Procedural extensions are written in Rust and compiled to **WebAssembly**. They use the `zed_extension_api` crate (on crates.io, currently v0.1.0+).

```rust
use zed_extension_api as zed;

struct MyExtension { /* state */ }

impl zed::Extension for MyExtension {
    // Lifecycle hooks
}

zed::register_extension!(MyExtension);
```

Key API methods:
- `zed::current_platform()` — detect OS
- `Worktree` methods — read files, env vars, find binaries
- Language server / context server / debugger registration

**Limitations**: WASM sandbox; no filesystem, network, or process access outside what the API provides. `cfg` directives don't work. Extensions run in a sandboxed environment.

**Source**: [Zed Developing Extensions docs](https://zed.dev/docs/extensions/developing-extensions)

### 2.3 Extension MCP Servers

Zed supports MCP servers as extensions. An extension can register a context server that implements MCP, making tools available to the agent.

**Source**: Agent panel docs mention "MCP Servers" integration

### 2.4 What Extensions CANNOT Do

- Cannot access arbitrary filesystem paths (only worktree paths via API)
- Cannot spawn arbitrary processes
- Cannot make arbitrary network requests
- Cannot interact with the editor UI directly (GPUI)
- Cannot inject messages into agent conversations
- No access to the Zed internal state (agents, threads, sessions)
- The WASM sandbox is strict and limited

**Conclusion**: Standard Zed extensions are **not a viable path** for injecting messages into the AI Agent from an external program.

---

## 3. Sending Messages to Zed's AI Agent from Outside

### 3.1 Zed CLI (`zed` command)

The `zed` CLI communicates with a running instance via **IPC channels** (Unix domain sockets / named pipes).

**Source**: `crates/cli/src/cli.rs`

The IPC protocol:
```rust
struct IpcHandshake {
    requests: IpcSender<CliRequest>,
    responses: IpcReceiver<CliResponse>,
}

enum CliRequest {
    Open {
        paths: Vec<String>,
        urls: Vec<String>,
        // ... open behavior flags
    },
    SetOpenBehavior { behavior: CliBehaviorSetting },
}
```

**What the CLI CAN do**:
- Open files and directories in Zed
- Control how paths open (new window, existing window, add to sidebar)
- Pass environment variables

**What the CLI CANNOT do**:
- Send messages to the AI Agent
- Execute editor commands
- Access any agent/assistant functionality
- The `CliRequest` enum has no agent-related variants

**Obvious gap**: There is no `CliRequest::SendToAgent` or similar command.

### 3.2 MCP (Model Context Protocol) Integration

Zed supports MCP context servers configured in `.mcp.json`. However:

- **MCP in Zed is inbound only**: Zed acts as an MCP **client**, not a server
- External MCP servers expose tools TO Zed's agent, not the other way around
- There is no way to make Zed listen as an MCP server that accepts tool calls from outside
- Zed's agent cannot be addressed as an MCP resource or tool by external programs

**Source**: Agent panel docs ("MCP Servers" section), existing `docs/research/mcp-protocol.md`

### 3.3 ACP (Agent Client Protocol)

ACP is an **internal** protocol used between Zed's UI layer and its agent backend. It runs in-process within the Zed application.

- **Not exposed over any network transport** (no HTTP, no WebSocket, no Unix socket)
- **No external API surface**
- Designed for internal Zed component communication only

**Source**: agent.rs, `acp_thread` crate

### 3.4 Collaboration Protocol

Zed's collaboration protocol allows multiple Zed instances to work on the same project simultaneously. However:
- It's designed for **human-to-human** collaborative editing
- It syncs buffers, cursors, selections — not agent conversations
- There is no agent-related functionality exposed through collaboration channels

### 3.5 Summary: No Official API

**Zed currently has NO official API for external programs to:**
- Send messages to the AI Agent
- Inject prompts into an agent conversation
- Read agent responses programmatically
- Trigger agent tool calls
- Control the agent programmatically

This is a deliberate design choice — the agent is deeply integrated into the editor and not exposed as a standalone service.

---

## 4. Zed's Workspace / Editor APIs

### 4.1 What Can Be Done Via CLI

```
zed <file>              # Open file in existing window
zed -n <dir>            # Open directory in new window
zed -a <dir>            # Add directory to existing workspace
zed -e <file>           # Open in existing window sidebar
zed --wait <file>       # Open and wait for file to close
```

**Source**: `crates/cli/src/cli.rs`, `zed --help`

### 4.2 What Can Be Done Via Extension API

- Download and manage language servers
- Provide Tree-sitter grammars
- Custom themes and icon themes
- Snippets
- MCP context servers (provide tools TO the agent)
- DAP debug adapters

### 4.3 What CANNOT Be Done

- No programmatic buffer editing API (extensions cannot modify editor content)
- No command execution API (cannot run `editor: save`, `workspace: close`, etc.)
- No event subscription (cannot listen for editor events)
- No UI customization (cannot add panels, modify layouts, etc.)
- No access to agent state or conversations

---

## 5. Alternative Approaches

Given the lack of official APIs, here are the workaround options:

### 5.1 Approach A: AppleScript / OSA (macOS Only)

On macOS, you can use AppleScript or `osascript` to send keystrokes to Zed:

```applescript
tell application "System Events"
    tell process "Zed"
        set frontmost to true
        -- Focus the agent panel (Cmd+Shift+I or similar)
        keystroke "i" using {command down, shift down}
        -- Type a message
        keystroke "Hello, can you help me refactor this code?"
        -- Press Enter to submit
        keystroke return
    end tell
end tell
```

**Pros**:
- Works today without any changes to Zed
- Can script any interaction

**Cons**:
- macOS only
- Fragile — depends on exact UI layout, keyboard shortcuts, timing
- No way to read responses programmatically
- Accessibility permissions required
- Breaks if Zed UI changes

**Verdict**: Viable for quick hacks, unsuitable for production integration.

### 5.2 Approach B: Accessibility APIs

Similar to AppleScript but using the OS accessibility API (AX API on macOS, UI Automation on Windows, AT-SPI on Linux):

- Can simulate clicks and keystrokes
- Can potentially read text from the UI
- More reliable than AppleScript for reading responses

**Pros**: Cross-platform (with different implementations)
**Cons**: Fragile, hacky, complex to implement correctly

### 5.3 Approach C: Zed Fork / Custom Build

Since Zed is open source (GPL/AGPL), you could:

1. Fork Zed
2. Add a custom IPC endpoint that accepts agent messages
3. Add a `CliRequest::SendToAgent { message: String }` variant
4. Build and use the custom Zed binary

**Pros**:
- Clean, proper integration
- Can add exactly the APIs needed
- Could upstream changes

**Cons**:
- Maintenance burden (keeping up with upstream)
- Need to distribute custom builds
- Large codebase to understand

### 5.4 Approach D: MCP Bridge via Extension

Zed extensions CAN create MCP context servers. You could:

1. Write a Zed extension that registers an MCP context server
2. That MCP server communicates with an external program (e.g., via stdio)
3. The external program uses MCP tools exposed by Zed's agent

But this is **inbound** (external → Zed). The extension SDK does not give access to Zed's agent, so you cannot implement "send a message to the agent" from extension code.

### 5.5 Approach E: Filesystem Message Queue (Fragile Workaround)

```
External Program                Zed Agent
     │                              │
     ├─ writes prompt to ──────────→│
     │  /tmp/zed-agent-queue/msg    │  (watches directory)
     │                              │
     │                              ├─ reads prompt
     │                              ├─ submits to agent
     │                              │
     │  ←─── writes response ───────┤
     │       /tmp/zed-agent-queue/resp
```

**How this could work**:
1. External program writes a message to a known directory
2. A Zed extension (or custom fork) watches that directory using the Worktree API
3. The extension submits the prompt to the agent (requires internal APIs)
4. The agent response is written back to a file

**Reality check**: Cannot be done with the current extension API. Extensions have no access to agent internals. This would require a Zed fork.

### 5.6 Approach F: Wait for Zed to Expose APIs

Zed is actively developed. Features like MCP support suggest the team is thinking about interoperability. Potential future APIs:

- Headless agent mode (run agent from CLI)
- MCP server mode (Zed as MCP server, exposing agent)
- HTTP/WebSocket API for agent interactions
- Extension API access to agent internals

Check these tracking locations:
- [Zed GitHub Issues](https://github.com/zed-industries/zed/issues)
- [Zed Blog / Changelog](https://zed.dev/blog)
- [Zed Roadmap](https://zed.dev/roadmap)

### 5.7 Approach G: Custom IPC via Zed Fork (Recommended for KairosEngine)

For a project like KairosEngine that needs reliable programmatic access to Zed's AI agent, the most practical approach is:

1. **Fork Zed** at a stable release tag
2. **Add a minimal IPC endpoint** that accepts agent messages
3. **Add a CLI subcommand**: `zed agent --send "prompt" --session <id>`
4. **Stream responses** back via stdout/stderr or a Unix socket

This is the approach that provides:
- The most reliable integration
- Full control over the API surface
- Access to all internal agent capabilities (tools, skills, context)
- A path to upstream the changes

---

## 6. Key Takeaways for KairosEngine

### 6.1 Current State

| Capability | Supported? | Notes |
|-----------|-----------|-------|
| External program → Agent message | ❌ No | No official API |
| Agent response → External program | ❌ No | No streaming/output API |
| Open files from CLI | ✅ Yes | `zed <file>` |
| MCP tools inbound to agent | ✅ Yes | Via `.mcp.json` config |
| MCP tools outbound from agent | ❌ No | Zed is MCP client only |
| Extension access to agent | ❌ No | WASM sandbox prevents it |
| Custom fork for APIs | ✅ Yes (theoretical) | Zed is open source |

### 6.2 Recommended Path for KairosEngine

**Short-term (immediate)**:
- Use `zed <file>` CLI to open relevant files for the AI coding agent
- The AI coding agent (Claude Code / Zed Agent) already works through the agent panel directly

**Medium-term (if integration needed)**:
- Fork Zed, add a custom IPC endpoint
- Add a `zed agent send --session <id> --prompt "..."` CLI command
- Stream responses to stdout

**Long-term (upstream)**:
- Contribute the IPC additions back to Zed upstream
- Advocate for a headless agent mode or MCP server support

### 6.3 For the Current Task (AI Coding Agent in Zed)

The existing setup already works: The AI coding agent (Claude Code / Zed Agent) interacts with Zed through the agent panel. For KairosEngine, this means:

- The agent can read/write files in the KairosEngine workspace
- The agent can run terminal commands for building/testing
- The agent has full context of the codebase through Zed's project system

If you need an external program to trigger the agent, consider the **filesystem approach (Approach E)** as the simplest immediate hack: have an external program write a task description to a file, and have the agent watch and process it.

However, for the AI coding agent use case, the most natural workflow is to interact with the agent **directly through the Zed Agent Panel UI**. The agent is designed to be invoked by the human developer, not by external automation.

---

## 7. References

1. **Zed Agent Panel Docs**: https://zed.dev/docs/assistant/assistant-panel
2. **Zed Extensions Docs**: https://zed.dev/docs/extensions
3. **Zed Developing Extensions Docs**: https://zed.dev/docs/extensions/developing-extensions
4. **Zed Source — agent.rs**: `crates/agent/src/agent.rs` in `github.com/zed-industries/zed`
5. **Zed Source — cli.rs**: `crates/cli/src/cli.rs` in `github.com/zed-industries/zed`
6. **Zed Source — ACP protocol**: `crates/agent_client_protocol/` in `github.com/zed-industries/zed`
7. **MCP Protocol Research**: `docs/research/mcp-protocol.md` (this repo)
8. **egui MCP Analysis**: `docs/research/egui-mcp-analysis.md` (this repo)
9. **Zed GitHub**: https://github.com/zed-industries/zed
10. **Zed Blog / Changelog**: https://zed.dev/blog
