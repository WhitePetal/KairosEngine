## Agent skills

### Issue tracker

Issues live in GitHub Issues (WhitePetal/KairosEngine). See `docs/agents/issue-tracker.md`.

### Triage labels

Five canonical labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Multi-context layout: `CONTEXT-MAP.md` at the repo root points at one `CONTEXT.md` per context (e.g. `kairos_asset/CONTEXT.md`, `kairos_graphics/CONTEXT.md`); `docs/adr/` holds system-wide decisions. See `docs/agents/domain.md`.

### Testing after implementation

Test only the crate you changed: `cargo test-crate <crate>`. Never run bare `cargo test` from the workspace root, and do not run `cargo test-full` (~183s) — the full suite is the merge gate, not your verification step. If you changed a crate that others depend on, add `cargo check --workspace --all-targets`. See `docs/agents/testing.md`.
