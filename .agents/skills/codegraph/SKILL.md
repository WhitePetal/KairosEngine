---
name: codegraph
description: Install, initialize, and verify CodeGraph (semantic code knowledge graph) in the KairosEngine project. Use when the user wants to set up CodeGraph, index the codebase for AI agents, or verify an existing CodeGraph installation.
---

# CodeGraph — Install, Initialize & Verify

CodeGraph pre-indexes the KairosEngine Rust workspace into a semantic
knowledge graph (SQLite).  After setup, AI agents (Claude Code, Cursor, Zed,
Codex, etc.) can answer structural questions with one tool call instead of a
file-by-file grep/read crawl.

**Supported in KairosEngine:** Rust (`.rs`) — full tree-sitter extraction
(functions, structs, traits, impls, macros, call edges).

---

## 1. Detect the environment

- Check the default shell: **PowerShell** on this machine.
- The project root is `D:\KairosEngine\KairosEngine`.
- CodeGraph stores its index in `<project>/.codegraph/` (already in `.gitignore`).

## 2. Check if CodeGraph is already installed

Run:

```powershell
codegraph version
```

If the command resolves, skip to step 4 (initialize).  Otherwise, continue to
step 3.

## 3. Install CodeGraph CLI

Choose the installation method that fits the environment.  In order of
preference:

### Option A — Self-contained bundle (no Node.js required, recommended)

```powershell
irm https://raw.githubusercontent.com/colbymchenry/codegraph/main/install.ps1 | iex
```

If the GitHub API is rate-limited, pin the version:

```powershell
$env:CODEGRAPH_VERSION = "v1.4.1"; irm https://raw.githubusercontent.com/colbymchenry/codegraph/main/install.ps1 | iex
```

After the installer finishes, **open a new terminal** so `codegraph` is on
`PATH`.  Then continue from step 2 to confirm.

### Option B — npm global (if Node.js is available)

```powershell
npm i -g @colbymchenry/codegraph
```

### Option C — npx (one-shot, no global install)

Use `npx @colbymchenry/codegraph` in place of every `codegraph` command below.

## 4. Wire up the Agent (one-time, global)

In a **new terminal** (important — the installer may not have updated the
current shell's PATH):

```powershell
codegraph install
```

This auto-detects installed agents (Claude Code, Cursor, Codex, opencode,
Gemini, etc.) and writes their MCP server configs.

**Non-interactive (CI-friendly):**

```powershell
codegraph install --yes --target=auto --location=local
```

### Zed agent — manual steps required

`codegraph install` does **not** auto-detect Zed.  After running it,
manually:

**a) Append the CodeGraph marker-fenced block to `AGENTS.md`:**

```markdown
<!-- CODEGRAPH_START -->
## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists
at the repo root), reach for it BEFORE grep/find or reading files when
you need to understand or locate code:

- **MCP tool** (when available): `codegraph_explore` answers most code
  questions in one call — the relevant symbols' verbatim source plus the
  call paths between them, including dynamic-dispatch hops grep can't
  follow. Name a file or symbol in the query to read its current
  line-numbered source. If it's listed but deferred, load it by name
  via tool search.
- **Shell** (always works): `codegraph explore "<symbol names or question>"`
  prints the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely —
indexing is the user's decision.
<!-- CODEGRAPH_END -->
```

**b) Create `.mcp.json` at the project root if it doesn't exist:**

```json
{
  "mcpServers": {
    "codegraph": {
      "type": "stdio",
      "command": "codegraph",
      "args": ["serve", "--mcp"]
    }
  }
}
```

## 5. Initialize the project index

Navigate to the project root and build the graph:

```powershell
cd D:\KairosEngine\KairosEngine
codegraph init
```

`codegraph init` both creates the `.codegraph/` directory and builds the
full index in one step.  For the KairosEngine workspace (~20 Rust modules
across 2 crates) this should complete in under a minute.

## 6. Verify

Run the status check:

```powershell
codegraph status
```

**Expected output** — a summary listing:

- Total indexed files
- Total symbols (functions, structs, traits, impls, enums, …)
- Total edges (calls, imports, impls, …)
- Supported language: Rust (`.rs`)
- Auto-sync status: **watching** (file watcher is active)

If `codegraph status` shows no errors and a non-zero symbol count, the
installation is working.

### Quick smoke-test

Run an explore query to confirm the graph is queryable:

```powershell
codegraph explore "How does GraphicsGraph compile render passes?"
```

The output should return the relevant source snippets from
`kairos_engine/src/graphics/graphics_graph/` with symbol relationships.

## 7. Post-setup notes

- **Auto-sync is on by default** — the file watcher uses native OS events
  (ReadDirectoryChangesW on Windows).  Every file save triggers an
  incremental re-index after a 2-second debounce.
- **No manual `codegraph sync` needed** in normal use.
- **`.codegraph/` is already gitignored** in KairosEngine's `.gitignore`.
  If not, add it:
  ```
  echo ".codegraph/" >> .gitignore
  ```
- **Restart your agent** (Zed, Claude Code, Cursor, etc.) after running
  `codegraph install` so it picks up the MCP server config.

## 8. Troubleshooting

| Symptom | Fix |
|---------|-----|
| `codegraph: command not found` | Open a **new terminal** — the installer added the binary to PATH but the current shell hasn't reloaded. |
| `CodeGraph not initialized` | Run `codegraph init` inside the project root. |
| `database is locked` | Run `codegraph status` — confirm Journal mode is `wal`. If not, move the project to a local disk (WAL is unreliable on network shares and WSL2 `/mnt` paths). |
| MCP server not connecting | Re-run `codegraph install` to rewrite the agent config. |
| Missing symbols after edits | Wait 2 seconds for debounced auto-sync, or run `codegraph sync` once. |
| Zed agent doesn't use CodeGraph | Check that `AGENTS.md` contains the `<!-- CODEGRAPH_START -->` block and `.mcp.json` exists at the project root. |

## 9. Uninstall (if needed)

```powershell
codegraph uninstall
```

Removes agent MCP configs **and** the CLI.  Pass `--keep-cli` to remove only
the agent wiring.  Project indexes (`.codegraph/`) are untouched; delete them
per-project with `codegraph uninit`.
