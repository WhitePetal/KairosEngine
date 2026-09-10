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

Test only the crate you changed: `cargo test-crate <crate>`. Never run bare `cargo test` from the workspace root, and do not run `cargo test-full` (~183s) — the full suite is the merge gate, not your verification step. If you changed a crate that others depend on, add `cargo check --workspace --all-targets`. See `docs/agents/testing.md`.
