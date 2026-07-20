## Parent

Part of #5 (Spec: Kairos Test Harness)

## What to build

Automatic documentation generation that extracts available test commands from the hard-coded dispatch table and writes them to `docs/ai/test-harness-commands.md`. This gives agents a discoverable reference of what engine operations they can invoke in TOML test files, following the existing `docs/ai/` project convention.

The documentation stays in sync with the dispatch table because it is generated from the same source — no manual updates needed when commands are added or changed.

## Acceptance criteria

- [ ] Documentation generator extracts command names, argument schemas, and descriptions from the dispatch match
- [ ] Generated file written to `docs/ai/test-harness-commands.md`
- [ ] Output format follows existing `docs/ai/` conventions (look at existing files for style reference)
- [ ] Each command listed with its name, description, arguments (name + type), and which mode it's available in (windowed / headless / both)
- [ ] Generation triggered via cargo feature or a dedicated subcommand
- [ ] Generated file is checked into version control so agents always have the latest reference

## Blocked by

- #T2 (Dispatch + TOML Test Runner)