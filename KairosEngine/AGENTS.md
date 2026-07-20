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

<!-- CODEGRAPH_START -->
## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), reach for it BEFORE grep/find or reading files when you need to understand or locate code:

- **MCP tool** (when available): `codegraph_explore` answers most code questions in one call — the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep can't follow. Name a file or symbol in the query to read its current line-numbered source. If it's listed but deferred, load it by name via tool search.
- **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely — indexing is the user's decision.
<!-- CODEGRAPH_END -->
