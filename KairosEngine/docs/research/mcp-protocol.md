# Model Context Protocol (MCP) — Research Report

> **Date:** 2026-07-24
> **Status:** Complete
> **Sources:** [modelcontextprotocol.io](https://modelcontextprotocol.io), [github.com/modelcontextprotocol](https://github.com/modelcontextprotocol), [github.com/modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk)

---

## Table of Contents

1. [What is MCP?](#1-what-is-mcp)
2. [How MCP Works Internally](#2-how-mcp-works-internally)
3. [How MCP Servers Are Implemented (Rust)](#3-how-mcp-servers-are-implemented-rust)
4. [The MCP Tool Call Flow](#4-the-mcp-tool-call-flow)
5. [Comparison with LSP](#5-comparison-with-lsp-language-server-protocol)
6. [Key Takeaways for KairosEngine](#6-key-takeaways-for-kairosengine)

---

## 1. What is MCP?

### Origin & Governance

The Model Context Protocol (MCP) is an **open-source standard** for connecting AI applications (LLM-powered agents, IDEs, chatbots) to external systems — data sources, tools, and workflows. It was **initially created by Anthropic** and is now hosted by **The Linux Foundation** as a community-driven project.

- **Website:** https://modelcontextprotocol.io
- **GitHub Org:** https://github.com/modelcontextprotocol (~49k followers)
- **Current stable spec version:** `2025-11-25`
- **Development draft:** `2026-07-28` (tracked by the Rust SDK)
- **Governance:** SEP process (Specification Enhancement Proposals), Working Groups, Interest Groups

### Design Philosophy

MCP is described as **"a USB-C port for AI applications"** — just as USB-C standardizes device connectivity, MCP standardizes how AI applications connect to external tools and data:

- **AI applications (hosts)** like Claude Desktop, VS Code, Cursor, Zed connect to MCP servers
- **MCP servers** expose capabilities: Tools, Resources, Prompts
- **One protocol, many integrations** — build a server once, use it across any MCP-compatible client

### What Problem Does It Solve?

Before MCP, every AI application integrated with external tools through bespoke, one-off code. MCP provides:

1. **Standardized tool exposure** — Tools with JSON Schema-defined inputs/outputs
2. **Context provision** — Resources (files, DB records, API responses) accessible by URI
3. **Prompt templates** — Reusable interaction patterns for LLMs
4. **Capability negotiation** — Client and server declare what they support upfront
5. **Transport abstraction** — Same protocol over stdio (local) or HTTP (remote)

### Architecture: Three Participants

```
┌─────────────────────────────────────────┐
│              MCP Host                    │
│  (Claude Desktop, VS Code, Zed, etc.)    │
│                                          │
│  ┌──────────┐  ┌──────────┐             │
│  │MCP Client│  │MCP Client│  ...        │
│  │(per conn)│  │(per conn)│             │
│  └────┬─────┘  └────┬─────┘             │
└───────┼─────────────┼───────────────────┘
        │             │
   stdio│        HTTP │ (Streamable HTTP)
        │             │
┌───────┴──┐    ┌─────┴────────┐
│   MCP    │    │     MCP      │
│  Server  │    │   Server     │
│ (local)  │    │  (remote)    │
└──────────┘    └──────────────┘
```

- **MCP Host:** The AI application (e.g., Claude Desktop) that manages MCP clients
- **MCP Client:** Per-connection component that talks to one MCP server
- **MCP Server:** A program exposing Tools/Resources/Prompts to AI applications

*(Source: [Architecture overview](https://modelcontextprotocol.io/docs/concepts/architecture))*

---

## 2. How MCP Works Internally

### 2.1 Two-Layer Architecture

MCP is split into two layers:

| Layer | Responsibility |
|---|---|
| **Data Layer** | JSON-RPC 2.0 message format, lifecycle, primitives (Tools, Resources, Prompts) |
| **Transport Layer** | Communication channel: stdio or Streamable HTTP |

The data layer is the "inner" layer; the transport layer wraps around it. The same JSON-RPC 2.0 messages flow through either transport.

### 2.2 Transport Layer

#### stdio Transport

- **Local only** — the client spawns the server as a subprocess
- Messages are newline-delimited JSON-RPC over stdin/stdout
- stderr is used for logging (may be captured or ignored by the client)
- **No embedded newlines** allowed in messages
- Fastest, zero network overhead

#### Streamable HTTP Transport

- **Replaces** the old `2024-11-05` HTTP+SSE transport
- Server exposes a **single HTTP endpoint** (e.g., `https://example.com/mcp`)
- **POST** — client → server (JSON-RPC requests, notifications, responses)
- **GET** — opens an SSE stream for server → client messages
- Supports **stateless mode** (protocol `2026-07-28`, SEP-2567): no `Mcp-Session-Id`, each request is independent
- Supports **legacy stateful mode**: `Mcp-Session-Id` header for session management
- Uses standard HTTP headers for routing: `Mcp-Method`, `Mcp-Name`, `Mcp-Param-*`
- **Security requirements:** validate `Origin` header, bind to localhost when local, implement auth

*(Source: [Transports](https://modelcontextprotocol.io/docs/concepts/transports))*

### 2.3 JSON-RPC 2.0 Message Format

All messages are UTF-8 encoded JSON-RPC 2.0. There are three message types:

**Request** (expects a response):
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list",
  "params": { "cursor": "optional" }
}
```

**Response** (to a request):
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": { "tools": [...] }
}
```

**Notification** (no response expected — no `id` field):
```json
{
  "jsonrpc": "2.0",
  "method": "notifications/tools/list_changed"
}
```

**Error Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "error": {
    "code": -32602,
    "message": "Unknown tool: invalid_tool_name"
  }
}
```

### 2.4 Lifecycle Phases

MCP is a **stateful protocol** with a defined lifecycle:

```
Client                                    Server
  │                                          │
  │──── initialize ────────────────────────▶ │  (1) Capability negotiation
  │◀─── InitializeResult ────────────────── │
  │                                          │
  │──── notifications/initialized ─────────▶ │  (2) Ready signal
  │                                          │
  │──── tools/list ────────────────────────▶ │  (3) Operation phase
  │◀─── tools list ──────────────────────── │
  │──── tools/call ────────────────────────▶ │
  │◀─── tool result ─────────────────────── │
  │                                          │
  │  ... (ongoing requests/notifications) ...│
  │                                          │
  │──── [connection close] ────────────────▶ │  (4) Shutdown
```

**Step 1 — Initialize (handshake):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-06-18",
    "capabilities": { "elicitation": {} },
    "clientInfo": { "name": "example-client", "version": "1.0.0" }
  }
}
```

Server response:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2025-06-18",
    "capabilities": {
      "tools": { "listChanged": true },
      "resources": {}
    },
    "serverInfo": { "name": "example-server", "version": "1.0.0" }
  }
}
```

Key aspects of initialization:
- **Protocol version negotiation** — ensures compatibility
- **Capability discovery** — both parties declare supported features
- **Identity exchange** — `clientInfo`/`serverInfo` for debugging

**Step 2 — Ready signal:**
```json
{
  "jsonrpc": "2.0",
  "method": "notifications/initialized"
}
```

Note: Protocol `2026-07-28` adds a `Discover` lifecycle mode (via `server/discover`) that replaces the legacy `initialize` + `initialized` dance for newer servers.

### 2.5 Core Primitives

#### Server Primitives (exposed by servers)

| Primitive | Purpose | Key Methods |
|---|---|---|
| **Tools** | Executable functions AI can invoke | `tools/list`, `tools/call` |
| **Resources** | Data sources with URI addressing | `resources/list`, `resources/read`, `resources/templates/list` |
| **Prompts** | Reusable LLM interaction templates | `prompts/list`, `prompts/get` |

#### Client Primitives (exposed by clients)

| Primitive | Purpose | Key Methods |
|---|---|---|
| **Sampling** ⚠️ | Server requests LLM completion from client | `sampling/createMessage` |
| **Elicitation** | Server requests user input | `elicitation/create` |
| **Logging** ⚠️ | Server sends structured logs to client | `notifications/logging/message` |

> ⚠️ **Sampling, Roots, and Logging are deprecated** (SEP-2577, protocol `2026-07-28`). They remain functional but will be removed in a future release.

#### Utility Features

- **Notifications** — real-time updates (e.g., `notifications/tools/list_changed`)
- **Progress** — tracking long-running operations
- **Cancellation** — cancel in-progress requests
- **Pagination** — cursor-based pagination for list methods
- **Completions** — auto-complete suggestions for prompt/resource arguments
- **Tasks** (experimental) — durable execution wrappers for long-running operations (SEP-2663)
- **Subscriptions** (protocol `2026-07-28`) — transport-neutral, long-lived `subscriptions/listen`
- **Multi-Round-Trip Requests** (protocol `2026-07-28`, SEP-2322) — server can ask for more input mid-request
- **Caching** (SEP-2549) — TTL-based response caching with automatic invalidation

### 2.6 How Servers Expose Capabilities

During `initialize`, the server declares supported capabilities:

```json
{
  "capabilities": {
    "tools": { "listChanged": true },
    "resources": { "subscribe": true, "listChanged": true },
    "prompts": { "listChanged": false }
  }
}
```

- `listChanged: true` means the server will send notifications when the list changes
- `subscribe: true` means clients can subscribe to individual resource changes

### 2.7 How Clients Discover and Call Tools

**Discovery:**
1. Client sends `tools/list` (supports pagination via `cursor`)
2. Server returns array of `Tool` objects with `name`, `description`, `inputSchema` (JSON Schema)

**Invocation:**
1. Client sends `tools/call` with `name` and `arguments` matching the `inputSchema`
2. Server returns `{ content: [...], isError: false }` or `{ content: [...], isError: true }`

Tools can return multiple content types:
- `text` — plain text
- `image` — base64-encoded image with MIME type
- `audio` — base64-encoded audio with MIME type
- `resource_link` — URI reference to a resource
- `resource` — embedded resource content
- `structuredContent` — typed JSON object conforming to `outputSchema`

---

## 3. How MCP Servers Are Implemented (Rust)

### 3.1 The Official Rust SDK: `rmcp`

The official Rust SDK is **`rmcp`** (Repository: [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk)):

- **Crate:** `rmcp` on crates.io
- **Stars:** ~3.7k on GitHub
- **Runtime:** `tokio` async
- **Macros:** `rmcp-macros` companion crate for `#[tool]`, `#[prompt]`, `#[tool_router]`, etc.
- **JSON Schema:** `schemars` for generating `inputSchema`/`outputSchema`
- **Serialization:** `serde`/`serde_json`
- **Tracked protocol versions:** `2025-11-25` (stable) and `2026-07-28` (development draft)

```toml
[dependencies]
rmcp = { version = "1", features = ["server"] }
# or dev channel:
# rmcp = { git = "https://github.com/modelcontextprotocol/rust-sdk", branch = "main", features = ["server"] }
```

### 3.2 Minimal MCP Server in Rust

A complete tools-only server using the declarative macro approach:

```rust
use rmcp::{
    handler::server::wrapper::Parameters,
    schemars, tool, tool_router,
    ServiceExt,
    transport::stdio,
};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct AddParams {
    /// First number
    a: i32,
    /// Second number
    b: i32,
}

#[derive(Clone)]
struct Calculator;

#[tool_router(server_handler)]  // auto-generates ServerHandler impl
impl Calculator {
    #[tool(description = "Add two numbers")]
    fn add(&self, Parameters(AddParams { a, b }): Parameters<AddParams>) -> String {
        (a + b).to_string()
    }

    #[tool(description = "Greet someone by name")]
    fn greet(
        &self,
        #[tool(param)] name: String,
    ) -> String {
        format!("Hello, {}!", name)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // stdio() creates a transport over stdin/stdout
    let server = Calculator.serve(stdio()).await?;
    // Block until the server shuts down
    server.waiting().await?;
    Ok(())
}
```

Key points:
- `#[tool_router(server_handler)]` on an `impl` block auto-generates the `ServerHandler` trait impl
- `#[tool(description = "...")]` marks methods as callable tools
- `Parameters<T>` wrapper handles JSON Schema generation and deserialization from `T`
- `#[tool(param)]` for simple scalar parameters
- `serve(transport)` handles the entire lifecycle (initialize handshake, etc.)

### 3.3 Server with Multiple Capabilities (Tools + Prompts + Resources)

For servers needing custom metadata or multiple capabilities:

```rust
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_router, tool_handler,
    transport::stdio,
};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct AddParams { a: i32, b: i32 }

#[derive(Clone)]
struct Calculator;

#[tool_router]
impl Calculator {
    #[tool(description = "Add two numbers")]
    fn add(&self, Parameters(AddParams { a, b }): Parameters<AddParams>) -> String {
        (a + b).to_string()
    }
}

#[tool_handler(name = "calculator", version = "1.0.0")]
impl ServerHandler for Calculator {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![
                Resource::new("config://app", "App Configuration"),
            ],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        match request.uri.as_str() {
            "config://app" => Ok(ReadResourceResult::new(vec![
                ResourceContents::text(r#"{"version": "1.0"}"#, &request.uri),
            ])),
            _ => Err(McpError::resource_not_found(
                "resource_not_found",
                Some(serde_json::json!({"uri": request.uri})),
            )),
        }
    }
}
```

### 3.4 How Tools Are Registered and Invoked

**Registration** (compile-time via macros):
- `#[tool_router]` generates a method-router that maps tool names → handler functions
- `#[tool(description = "...")]` registers the function with metadata
- The `inputSchema` is derived from the function parameter types via `schemars::JsonSchema`
- `outputSchema` (protocol `2026-07-28`) can be derived from the return type

**Invocation** (runtime):
1. Client sends `tools/call` with `{ name: "add", arguments: { a: 1, b: 2 } }`
2. SDK deserializes `arguments` into the `AddParams` struct
3. SDK calls the matching handler function
4. Return value is serialized into a `CallToolResult` with `content: [TextContent { text: "3" }]`

### 3.5 Error Handling Patterns

MCP has two error categories:

**Protocol errors** (JSON-RPC level — returned as error responses):
```rust
// Tool not found
Err(McpError::method_not_found("tools/call", None))

// Invalid parameters
Err(McpError::invalid_params("Missing required field", None))

// Internal error
Err(McpError::internal_error("Database connection failed", None))
```

Standard JSON-RPC error codes:
| Code | Meaning |
|---|---|
| `-32600` | Invalid Request |
| `-32601` | Method not found |
| `-32602` | Invalid params |
| `-32603` | Internal error |
| `-32002` | Resource not found (MCP-specific) |

**Tool execution errors** (returned as successful responses with `isError: true`):
```rust
// Return a tool execution error
Ok(CallToolResult::error(vec![
    ContentBlock::text("Failed to fetch data: API rate limit exceeded")
]))
```

This results in:
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "content": [
      { "type": "text", "text": "Failed to fetch data: API rate limit exceeded" }
    ],
    "isError": true
  }
}
```

The distinction matters: protocol errors mean the request couldn't be processed; tool execution errors mean the tool ran but encountered a domain-level problem.

### 3.6 Server-Side Notifications

Servers can push real-time updates to clients:

```rust
use rmcp::service::RequestContext;

// Inside any handler with access to `context`:
context.peer.notify_tool_list_changed().await?;
context.peer.notify_resource_list_changed().await?;
context.peer.notify_prompt_list_changed().await?;

// Resource-specific update:
context.peer.notify_resource_updated(
    ResourceUpdatedNotificationParam::new("file:///config.json"),
).await?;

// Progress during long operations:
context.peer.notify_progress(
    ProgressNotificationParam::new(
        ProgressToken(NumberOrString::Number(42)),
        50.0,  // percentage
    )
    .with_total(100.0)
    .with_message("Processing..."),
).await?;
```

### 3.7 Transport Options in Rust

**stdio (local):**
```rust
use rmcp::transport::stdio;
let server = my_service.serve(stdio()).await?;
```

**Tokio child process (spawn a server):**
```rust
use rmcp::{ServiceExt, transport::{TokioChildProcess, ConfigureCommandExt}};
use tokio::process::Command;

let client = ().serve(TokioChildProcess::new(
    Command::new("npx").configure(|cmd| {
        cmd.arg("-y").arg("@modelcontextprotocol/server-everything");
    })
)?).await?;
```

**Streamable HTTP (remote):**
```rust
use rmcp::transport::streamable_http_server::{
    StreamableHttpService, StreamableHttpServerConfig,
    session::local::LocalSessionManager,
};

let config = StreamableHttpServerConfig::default()
    .with_legacy_session_mode(false)  // stateless for 2026-07-28
    .with_json_response(true);

let service = StreamableHttpService::new(
    || Ok(Counter::new()),  // factory: new handler per request
    LocalSessionManager::default().into(),
    config,
);

// Mount on axum router:
// let router = axum::Router::new().nest_service("/mcp", service);
```

### 3.8 Other Notable Rust Ecosystem Crates

Beyond the official `rmcp`:

| Crate | Description |
|---|---|
| `rmcp-actix-web` | Actix Web backend for `rmcp` |
| `rmcp-openapi` | Transform OpenAPI endpoints into MCP tools |
| `rmcp-openapi-server` | High-performance MCP server exposing OpenAPI endpoints |

Third-party servers built with `rmcp`:
- **goose** — open-source extensible AI agent
- **nvim-mcp** — MCP server for Neovim interaction
- **terminator** — AI-powered desktop automation
- **systemprompt-template** — MCP governance (auth, rate-limiting, audit)

---

## 4. The MCP Tool Call Flow

### Complete Step-by-Step Sequence

The following is a full wire-level trace of a tool call:

```
┌────────┐                                          ┌────────┐
│ Client │                                          │ Server │
└───┬────┘                                          └───┬────┘
    │                                                   │
    │  1. INITIALIZE (handshake)                        │
    │══════════════════════════════════════════════════▶│
    │  {                                                │
    │    "jsonrpc": "2.0",                              │
    │    "id": 1,                                       │
    │    "method": "initialize",                        │
    │    "params": {                                    │
    │      "protocolVersion": "2025-06-18",             │
    │      "capabilities": { "elicitation": {} },       │
    │      "clientInfo": {                              │
    │        "name": "kairos-client",                   │
    │        "version": "1.0.0"                         │
    │      }                                            │
    │    }                                              │
    │  }                                                │
    │                                                   │
    │  2. INITIALIZE RESULT                             │
    │◀══════════════════════════════════════════════════│
    │  {                                                │
    │    "jsonrpc": "2.0",                              │
    │    "id": 1,                                       │
    │    "result": {                                    │
    │      "protocolVersion": "2025-06-18",             │
    │      "capabilities": {                            │
    │        "tools": { "listChanged": true }           │
    │      },                                           │
    │      "serverInfo": {                              │
    │        "name": "kairos-mcp-server",               │
    │        "version": "1.0.0"                         │
    │      }                                            │
    │    }                                              │
    │  }                                                │
    │                                                   │
    │  3. INITIALIZED (notification — no response)      │
    │══════════════════════════════════════════════════▶│
    │  {                                                │
    │    "jsonrpc": "2.0",                              │
    │    "method": "notifications/initialized"          │
    │  }                                                │
    │                                                   │
    │  ═══════ OPERATION PHASE ═══════════════════       │
    │                                                   │
    │  4. TOOLS/LIST (discover available tools)         │
    │══════════════════════════════════════════════════▶│
    │  {                                                │
    │    "jsonrpc": "2.0",                              │
    │    "id": 2,                                       │
    │    "method": "tools/list"                         │
    │  }                                                │
    │                                                   │
    │  5. TOOLS/LIST RESPONSE                           │
    │◀══════════════════════════════════════════════════│
    │  {                                                │
    │    "jsonrpc": "2.0",                              │
    │    "id": 2,                                       │
    │    "result": {                                    │
    │      "tools": [                                   │
    │        {                                          │
    │          "name": "scene_load",                    │
    │          "description": "Load a 3D scene file",   │
    │          "inputSchema": {                         │
    │            "type": "object",                      │
    │            "properties": {                        │
    │              "path": {                            │
    │                "type": "string",                  │
    │                "description": "Path to .gltf"     │
    │              }                                    │
    │            },                                     │
    │            "required": ["path"]                   │
    │          }                                        │
    │        },                                         │
    │        {                                          │
    │          "name": "entity_spawn",                  │
    │          "description": "Spawn an entity",        │
    │          "inputSchema": {                         │
    │            "type": "object",                      │
    │            "properties": {                        │
    │              "prefab": {                          │
    │                "type": "string"                   │
    │              },                                   │
    │              "position": {                        │
    │                "type": "array",                   │
    │                "items": { "type": "number" },     │
    │                "minItems": 3,                     │
    │                "maxItems": 3                      │
    │              }                                    │
    │            },                                     │
    │            "required": ["prefab"]                 │
    │          }                                        │
    │        }                                          │
    │      ]                                            │
    │    }                                              │
    │  }                                                │
    │                                                   │
    │  6. TOOLS/CALL (invoke a tool)                    │
    │══════════════════════════════════════════════════▶│
    │  {                                                │
    │    "jsonrpc": "2.0",                              │
    │    "id": 3,                                       │
    │    "method": "tools/call",                        │
    │    "params": {                                    │
    │      "name": "scene_load",                        │
    │      "arguments": {                               │
    │        "path": "assets/level1.gltf"               │
    │      }                                            │
    │    }                                              │
    │  }                                                │
    │                                                   │
    │  7. TOOLS/CALL RESULT                             │
    │◀══════════════════════════════════════════════════│
    │  {                                                │
    │    "jsonrpc": "2.0",                              │
    │    "id": 3,                                       │
    │    "result": {                                    │
    │      "content": [                                 │
    │        {                                          │
    │          "type": "text",                          │
    │          "text": "Loaded scene with 142 entities" │
    │        }                                          │
    │      ],                                           │
    │      "isError": false                             │
    │    }                                              │
    │  }                                                │
    │                                                   │
    │  8. TOOLS/LIST_CHANGED (server notification)      │
    │◀══════════════════════════════════════════════════│
    │  {                                                │
    │    "jsonrpc": "2.0",                              │
    │    "method": "notifications/tools/list_changed"   │
    │  }                                                │
    │                                                   │
    │  9. TOOLS/LIST (client re-fetches)                │
    │══════════════════════════════════════════════════▶│
    │  {                                                │
    │    "jsonrpc": "2.0",                              │
    │    "id": 4,                                       │
    │    "method": "tools/list"                         │
    │  }                                                │
    │                                                   │
    │  ... and so on ...                                │
```

### Key Observations

1. **Every request has a unique `id`** — used to correlate responses
2. **Notifications have no `id`** — no response is expected
3. **`isError` in tool results** — distinguishes protocol-level failures from domain-level failures
4. **Tools can return multiple content blocks** — text, images, resources, etc.
5. **Real-time updates** via `list_changed` notifications keep clients in sync without polling

### Error at Each Step

| Step | Possible Error | Response |
|---|---|---|
| Initialize | Protocol version mismatch | Server rejects with error, client should disconnect |
| tools/list | Server internal error | `-32603` Internal error |
| tools/call | Unknown tool name | `-32602` "Unknown tool: X" |
| tools/call | Invalid arguments | `-32602` Invalid params with details |
| tools/call | Domain error (API failure) | `result.isError: true` with descriptive text |

---

## 5. Comparison with LSP (Language Server Protocol)

MCP and LSP share architectural DNA but serve different domains:

### Similarities

| Aspect | LSP | MCP |
|---|---|---|
| **Architecture** | Client-server | Client-server |
| **Wire protocol** | JSON-RPC 2.0 | JSON-RPC 2.0 |
| **Transport** | stdio, TCP, pipes | stdio, Streamable HTTP |
| **Lifecycle** | `initialize` → `initialized` → operate → `shutdown` | `initialize` → `initialized` → operate → close |
| **Capability negotiation** | `ServerCapabilities` / `ClientCapabilities` in initialize | `capabilities` object in initialize |
| **Notifications** | `$/`-prefixed (e.g., `$/progress`) | `notifications/`-prefixed |
| **Discovery** | Server declares capabilities; client uses what's available | Server declares primitives; client discovers via `*/list` |
| **Spec versioning** | Date-based (`3.17`) | Date-based (`2025-11-25`) |
| **Origin** | Microsoft | Anthropic → Linux Foundation |

### Differences

| Aspect | LSP | MCP |
|---|---|---|
| **Domain** | Code editing (diagnostics, completion, hover, refactoring) | AI-tool integration (tools, data, prompts for LLMs) |
| **Core primitives** | TextDocumentSync, Completion, Hover, Definition, etc. | Tools, Resources, Prompts |
| **Content model** | Text documents with positions/ranges | Content blocks (text, image, audio, resource links) with URIs |
| **Execution model** | Server provides language intelligence | Server provides **executable actions** (tools) that modify state |
| **User-in-the-loop** | Implicit (editor UI) | Explicit (elicitation, tool confirmation) |
| **Streaming responses** | Partial results for completions | Progress notifications + Tasks for long operations |
| **Caching** | Not built-in | Built-in TTL-based response caching (SEP-2549) |
| **Auth model** | None (assumes trusted local process) | OAuth 2.1, bearer tokens, API keys, enterprise IdP |
| **Remote servers** | Unusual (usually local) | First-class with Streamable HTTP |
| **LLM integration** | None | Sampling (server→client LLM calls), prompt templates |
| **Stateless mode** | N/A | Supported (protocol `2026-07-28`, SEP-2567) |

### Why the Comparison Matters

LSP proved that a standard protocol can unlock an ecosystem. Before LSP, every editor had N language-specific plugins. After LSP, one language server works everywhere. MCP aims for the same dynamic: one MCP server can provide tools/resources/prompts to Claude Desktop, VS Code, Cursor, Zed, and any future MCP host — and vice versa, any MCP host can use tools from any MCP server.

---

## 6. Key Takeaways for KairosEngine

### Relevance to KairosEngine

MCP could enable KairosEngine to:
1. **Expose engine functionality as tools** — scene loading, entity spawning, physics queries, material editing — controllable by AI agents
2. **Provide editor context as resources** — scene hierarchy, asset catalog, performance metrics
3. **Integrate with AI-assisted workflows** — code generation, debugging, performance tuning via LLM tool calls

### Implementation Path

If KairosEngine wanted an MCP server:

1. **Add `rmcp` dependency** with `server` feature
2. **Define tool structs** with `#[derive(Deserialize, schemars::JsonSchema)]`
3. **Create a `ServerHandler`** using `#[tool_router(server_handler)]` or explicit trait impl
4. **Wire up transport** — stdio for local editor integration, or Streamable HTTP for remote
5. **Register tools** for engine operations:
   - `scene_load(path: String)`
   - `entity_spawn(prefab: String, position: [f64; 3])`
   - `entity_query(filter: EntityFilter)`
   - `asset_list(category: String)`
   - `render_stats()` → metrics resource
6. **Expose resources** — scene graph as `scene://hierarchy`, config as `config://app`

### Protocol Maturity Considerations

- MCP is **stable and production-ready** (widely deployed: Claude Desktop, VS Code, Cursor, etc.)
- The official Rust SDK (`rmcp`) is **tier-1 maintained** with ~3.7k stars and active development
- Some features (sampling, roots, logging) are **deprecated** and will be removed — avoid building on them
- The `2026-07-28` draft adds MRTR (multi-round-trip requests), stateless HTTP, and long-running tasks — all useful for engine operations

### Potential Risks

1. **Spec churn** — the protocol is evolving rapidly (new spec draft every 6 months)
2. **Deprecated features** — sampling, roots, and logging are deprecated; avoid investing in them
3. **LLM dependency** — MCP tools are designed for LLM control; usefulness depends on LLM quality
4. **Security boundary** — engines executing arbitrary tool calls from AI agents need careful sandboxing

---

## References

1. [MCP Official Site](https://modelcontextprotocol.io) — Overview, architecture, concepts
2. [MCP Specification v2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/) — Full spec
3. [MCP GitHub Organization](https://github.com/modelcontextprotocol) — SDKs, spec repo, example servers
4. [Rust SDK (`rmcp`)](https://github.com/modelcontextprotocol/rust-sdk) — Official Rust implementation
5. [MCP Architecture Docs](https://modelcontextprotocol.io/docs/concepts/architecture) — Architecture overview
6. [MCP Transports Docs](https://modelcontextprotocol.io/docs/concepts/transports) — stdio and Streamable HTTP
7. [MCP Tools Docs](https://modelcontextprotocol.io/docs/concepts/tools) — Tool definition and invocation
8. [MCP Resources Docs](https://modelcontextprotocol.io/docs/concepts/resources) — Resource model
9. [MCP Prompts Docs](https://modelcontextprotocol.io/docs/concepts/prompts) — Prompt templates
10. [MCP LLMs.txt Index](https://modelcontextprotocol.io/llms.txt) — Full documentation index
