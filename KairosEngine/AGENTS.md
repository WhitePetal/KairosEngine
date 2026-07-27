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
