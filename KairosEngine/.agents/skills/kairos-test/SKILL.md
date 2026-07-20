---
name: kairos-test
description: Write tests after implementing a feature. Use Rust unit/integration tests for logic and data validation, and the kairos_test_harness TOML-based runtime tests for engine interaction paths. When the harness lacks a needed capability, extend it.
---

# Kairos Test

> ⚠️ **STATUS: Test harness needs refactoring.**
> The TOML runtime test harness has known limitations with egui popup/Area widget interaction
> (ComboBox dropdowns, floating menus, etc.). See [#27](https://github.com/WhitePetal/KairosEngine/issues/27).
> **Do not write new TOML tests that depend on ComboBox dropdown interaction until the harness is refactored.**
> Normal widgets (Buttons, Labels, rect queries) and keyboard event injection are functional.

After implementing a feature, write tests to verify correctness. Choose the right test
type based on what you're validating, and extend the test harness when it's missing
capabilities you need.

## Decision tree

```
Feature implemented
  │
  ├─ Can this be validated purely with data/logic (no GPU, no engine loop)?
  │    └─ Yes → Write a Rust unit test or integration test
  │
  └─ Does this require engine runtime interaction (GPU, egui, physics, ECS scheduling)?
       ├─ Yes, and the harness already supports the needed commands →
       │     Write a TOML test file under `tests/runtime/`
       │
       └─ Yes, but the harness is missing a command/assertion/input type →
             First extend the harness, then write the TOML test
```

## Rust integration tests

Location: `kairos_engine/tests/integration/`

Follow the existing test structure — one module per tested area (e.g., `audio/`,
`ecs/`, `kairos_editor/`). Use `#[test]` for pure logic tests, or `#[tokio::test]`
when tokio runtime is needed.

Tests go here when:
- Validating data transformations, serialization, math, ECS component logic
- Testing pure functions that don't need a live engine loop
- Anything `cargo test` can run without a GPU

Requirements:
- Follow the project's domain vocabulary from `CONTEXT.md` (if it exists)
- Test through public interfaces, not internal implementation details

## Kairos Test Harness (runtime tests)

Location: `tests/runtime/` — TOML files executed by the engine.

Tests go here when:
- Validating GPU-dependent features (texture formats, shader compilation, wgpu adapter features)
- Testing egui widget interactions and UI flows
- Verifying physics simulation or ECS system scheduling order
- Simulating user keyboard/mouse input paths
- Any path that `cargo test` cannot exercise

### Writing a TOML test

```toml
# tests/runtime/my_feature.toml
[[step]]
action = "call"
target = "system.ping"

[[step]]
action = "call"
target = "texture_inspector.open"

[[step]]
action = "assert"
target = "no_crash"

[[step]]
action = "assert"
target = "ecs_query"
args = { query = "all", expect = "count >= 1" }
```

Available commands are documented in `docs/ai/test-harness-commands.md`.
Regenerate with `cargo run --features test-harness -- --gen-docs` after extending
the dispatch table.

### Running TOML tests

```bash
# Headless mode (CI-safe, no window) — for logic-only tests
cargo run -p kairos_engine --features test-harness -- --headless --test-file tests/runtime/my_feature.toml

# Windowed mode (full UI) — for tests that need egui, inspector, or widget interaction
cargo run -p kairos_engine --features test-harness -- --test-file tests/runtime/my_feature.toml

# With supervisor (captures crash logs)
cargo run -p kairos_supervisor -- \
  target/debug/kairos_engine.exe --features test-harness -- --headless --test-file tests/runtime/my_feature.toml
```

**Which mode to use:**

| Mode | When |
|---|---|
| `--headless` | Tests that only need call/assert commands, no UI interaction. CI-safe. |
| Windowed (no flag) | Tests that need inspector commands, widget rect queries, or input injection against egui widgets. Requires a display. |

When running in windowed mode, the TOML test MUST include setup steps:

```toml
# Open necessary panels
[[step]]
action = "call"
target = "ui.open_inspector"

# Select the asset to work on
[[step]]
action = "call"
target = "project.select_asset"
args = { path = "res/textures/my_asset.texture" }
```

### Triggering tests via WebSocket

The WS server runs on port 9999 in both modes. The agent can connect and
send `run_test` commands interactively without restarting the engine:

```json
{"cmd": "run_test", "file": "tests/runtime/my_feature.toml"}
```

### Extending the harness

When a needed capability is missing, extend the harness before writing the TOML test.
There are four extension points, each in `kairos_engine/src/kairos_test_harness/`:

| Extension | File | When |
|---|---|---|
| New call command | `dispatch.rs` → `dispatch_call()` | Need a new engine operation (open panel, set value, trigger action) |
| New assertion | `assertions.rs` + `dispatch.rs` → `dispatch_assert()` | Need a new way to verify engine state |
| New input type | `input_injector.rs` | Need to simulate a new input device or key |
| New test action | `bridge.rs` → `execute_step()` | Need a fundamentally new action category |

After extending:
1. Add entries to `docs_gen.rs` → `all_commands()` (or the assertion/input tables)
2. Regenerate docs: `cargo run --features test-harness -- --gen-docs`
3. Write unit tests for the new capability
4. Then write the TOML test

### Verification checklist

After writing tests:
- [ ] `cargo test` passes (unit + integration tests)
- [ ] `cargo check --features test-harness` compiles
- [ ] TOML tests run successfully in headless mode
- [ ] Docs regenerated if harness was extended
