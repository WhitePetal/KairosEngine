<!-- CODEGRAPH_START -->
## ⚡ First: use CodeGraph before any grep / read_file

If `.codegraph/` exists at the repo root, **your first step for ANY code-understanding or code-locating task must be** `codegraph explore "<what you're looking for>"` (shell) or the MCP `codegraph_explore` tool — BEFORE you reach for grep, find_path, or read_file.

CodeGraph gives you verbatim source + call paths in one call, including dynamic-dispatch hops grep can't follow. It's faster and more complete than piecing answers together from multiple reads.

If there is no `.codegraph/` directory, skip CodeGraph entirely.
<!-- CODEGRAPH_END -->

## Agent skills

### Issue tracker

Issues live in GitHub Issues (WhitePetal/KairosEngine). See `docs/agents/issue-tracker.md`.

### Triage labels

Five canonical labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout: `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

### Testing after implementation

After implementing a feature or fixing a bug, always verify correctness through tests. Use the `kairos-test` skill (`.agents/skills/kairos-test/SKILL.md`) to decide the right test strategy:

- **Rust integration tests** (`kairos_engine/tests/integration/`) — for data/logic validation that doesn't need the engine loop or GPU.
- **Kairos Test Harness TOML tests** (`tests/runtime/`) — for runtime interactions (GPU, egui, physics, ECS scheduling, input paths) that can't be exercised by `cargo test`.

If the test harness lacks a needed command, assertion, or input type, extend it first (see the skill for extension points), then write the TOML test.
