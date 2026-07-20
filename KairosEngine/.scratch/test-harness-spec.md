## Problem Statement

When an AI agent implements features in the KairosEngine codebase, it can verify compilation via `cargo check` and validate data via unit tests. However, many bugs only manifest at **engine runtime** — constraints enforced by wgpu (GPU feature requirements, adapter capabilities), egui widget interaction flows, ECS system scheduling order, physics simulation behavior, input handling edge cases, and multi-system orchestration.

A concrete example: an agent implemented a texture inspector that lets users select a texture format. The de/encoder for that format compiled fine, but at runtime the engine crashed because wgpu required a specific **adapter feature** to be enabled, and the agent missed that requirement. No unit test or `cargo check` could catch this.

The agent lacks a way to **interact with the running engine as a real user would** — clicking UI widgets, injecting keyboard/mouse input, observing ECS state, checking GPU resource validity — to verify that all user-triggerable paths work correctly end-to-end.

## Solution

Build a **Kairos Test Harness** — a runtime interaction tool that lets the AI agent connect to a running engine instance, send commands, inject input, query engine state, assert correctness, and receive crash logs. It provides:

- A **WebSocket server** embedded in the engine (behind a compile-time feature flag), exposing a request-response protocol for agent interaction.
- A **TOML-based test definition format** that agents use to author declarative test scenarios.
- A **test runner** inside the engine that executes TOML test files step-by-step via coroutine-style async, with assertions for crash detection, ECS queries, resource existence, GPU object validity, and log pattern matching.
- **Dual engine modes**: windowed (full UI interaction + screenshots) and headless (domain API calls only, no GPU surface required).
- A lightweight **supervisor process** that manages engine lifecycle and captures crash logs reliably.
- **Input injection** for simulating keyboard and mouse events in the game runtime.
- Complete **compile-time isolation**: all test infrastructure is gated behind `#[cfg(feature = "test-harness")]` so release builds ship zero test code.

## User Stories

### Agent Workflow

1. As an agent, I want to author a TOML test file describing a user interaction scenario, so that I can declaratively define test cases without writing Rust test code.
2. As an agent, I want to start the engine with a `--test-file` CLI argument, so that the engine automatically runs my test and outputs results.
3. As an agent, I want to connect to a running engine via WebSocket and issue test commands interactively, so that I can explore engine state in real-time while debugging.
4. As an agent, I want to send a `run_test` command via WebSocket to trigger a TOML test file, so that I can run multiple tests without restarting the engine.
5. As an agent, I want the test to abort immediately on the first assertion failure with a clear error message, so that I can quickly identify and fix the problem.
6. As an agent, I want to receive crash logs reliably even when the engine process dies, so that I can diagnose fatal runtime errors.
7. As an agent, I want to `cargo build` the engine after making code changes and re-run my tests, so that I can iterate rapidly on fixes.

### Editor UI Testing

8. As an agent, I want to trigger opening a UI panel (e.g. texture inspector), so that I can verify the panel initializes without crashing.
9. As an agent, I want to select a value in an egui dropdown (e.g. texture format), so that I can verify UI-driven configuration changes work correctly.
10. As an agent, I want to assert that no panic occurred after a UI operation, so that I can confirm basic stability of UI interactions.
11. As an agent, I want to query the ECS world to check if a component (e.g. `TextureAsset`) was created or modified after a UI action, so that I can verify side effects of user interactions.
12. As an agent, I want to assert that a GPU resource (e.g. wgpu texture) is valid after creation, so that I can catch missing GPU feature requirements.
13. As an agent, I want to assert that a file-based resource (e.g. texture asset) exists at a given path, so that I can verify asset loading pipelines.
14. As an agent, I want to search engine logs for expected messages (e.g. "Texture created successfully"), so that I can verify proper logging of successful operations.

### Game Runtime Testing

15. As an agent, I want to inject keyboard press and release events (e.g. W, A, S, D), so that I can simulate player movement in a game scene.
16. As an agent, I want to inject mouse movement and click events, so that I can simulate player aiming and interaction.
17. As an agent, I want to control the exact timing of input events from the test script, so that I can reproduce specific input sequences deterministically.
18. As an agent, I want to query ECS component values (e.g. `Transform` position) after injecting input, so that I can verify game logic responds correctly to player actions.
19. As an agent, I want to run the engine in real wall-clock time during game tests, so that the simulation matches actual user experience.

### Headless and CI Testing

20. As an agent, I want to run tests in headless mode where the engine initializes without a GPU surface, so that tests can run in CI environments without displays.
21. As an agent, I want to call domain API commands directly (e.g. `texture_inspector.select_format`) in headless mode instead of simulating UI, so that I can test logic without rendering overhead.
22. As an agent, I want the headless engine to still create wgpu adapter and device, so that GPU-dependent features can be validated without a window.

### Test Authoring

23. As an agent, I want to discover available test commands via project documentation (auto-generated from code), so that I know what operations I can test.
24. As an agent, I want to design my own test scenarios and assertions, so that I can cover all user-triggerable paths for the feature I implemented.
25. As an agent, I want to choose between windowed and headless modes based on what aspects of the feature I need to validate.

### Production Isolation

26. As a release engineer, I want the test harness code to be completely excluded from release builds, so that the published game binary contains no test infrastructure and has no increased attack surface.
27. As a developer, I want the test harness to live as a separate module alongside existing modules like `kairos_editor`, so that it can later be extracted into its own crate without major refactoring.

## Implementation Decisions

### Architecture Overview

The test harness consists of four components:
- **KairosEngine module** (`kairos_test_harness`): WebSocket server, TOML test runner, command dispatch, assertion engine, input injection
- **Supervisor process**: Thin watchdog that manages engine lifecycle and captures crash logs
- **TOML test files**: Declarative test definitions authored by agents
- **Auto-generated documentation**: Lists available test commands for agent discoverability

### WebSocket Protocol

- **Transport**: WebSocket on localhost. Agent connects directly to the engine process (not through supervisor).
- **Model**: Request-response. Each command from the agent returns a response. Agent awaits each response before sending the next command — matching the step-by-step nature of user interaction simulation.
- **Message format**: JSON. Commands include a correlation ID for matching responses.
- **Command types**: Both general primitives (input injection, log query) and domain-specific API calls (inspector operations, ECS queries). High-frequency operations get dedicated high-level APIs.

### Engine Dual Mode

- **Windowed mode**: Full winit event loop with wgpu surface. Agent can interact with egui UI and (in future) capture screenshots. Used for full-stack UI interaction testing.
- **Headless mode**: Engine creates wgpu adapter + device but no surface/window. egui layer is bypassed entirely. Agent uses domain API commands directly. Used for logic validation without rendering overhead, and for CI environments without displays.
- Mode selection: CLI flag and configurable per test run.

### Tokio / Winit Bridge

The engine has two concurrency domains:
- **Tokio runtime** (separate thread): Runs the WS server and the async test runner coroutine.
- **Winit event loop** (main thread): Owns the ECS world, wgpu device, and egui context.

Communication uses channels:
- Tokio → main: `mpsc::Sender` (commands queued for main thread execution)
- Main → Tokio: `oneshot::Sender` (execution results returned to the awaiting test runner coroutine)
- Each frame, the main thread drains the mpsc queue and processes commands.

### Test Runner: Coroutine Model

- TOML tests are executed as an `async fn` spawned on the tokio runtime.
- Each step in the TOML is `await`ed — sending a command to the main thread via oneshot and awaiting the result.
- This produces linear, readable test execution that matches the TOML structure naturally, without explicit state machine management.
- On assertion failure: the coroutine aborts immediately, reports the failed step number and error details back to the agent via WS.
- The engine process remains alive after test failure, allowing the agent to manually continue interacting if desired.

### Hard-Coded Command Dispatch

- A `match` statement in the WS handler routes string command names to engine functions.
- Each supported command is manually mapped — no reflection or dynamic registration in v1.
- Commands are added to the dispatch match when new engine features need test support.
- Future consideration: proc-macro-based registration for cleaner subsystem decoupling.

### TOML Test Format

Tests are linear sequences of steps. Each step is either an action (call, input) or an assertion. Example:

```toml
[[step]]
action = "call"
target = "texture_inspector.open"

[[step]]
action = "call"
target = "texture_inspector.select_format"
args = { format = "BC7" }

[[step]]
action = "assert"
target = "no_crash"

[[step]]
action = "assert"
target = "ecs_query"
query = "TextureAsset"
expect = "count >= 1"
```

Input injection:

```toml
[[step]]
action = "input"
device = "keyboard"
event = "press"
key = "W"

[[step]]
action = "input"
device = "mouse"
event = "click"
button = "Left"
```

### Assertion Types (v1)

| Assertion | Description |
|---|---|
| `no_crash` | Engine process has not panicked since last assertion |
| `ecs_query` | Query the ECS world; assert count or component value condition |
| `resource_exists` | Assert a file/resource exists at a given path |
| `wgpu_valid` | Assert a GPU resource (texture, buffer, etc.) is valid |
| `log_contains` | Assert the engine log contains a pattern string |

### Input Injection

- Keyboard: `press` and `release` events per key, timing controlled by agent via WS message sequencing.
- Mouse: `move` (absolute screen coordinates) and `click` (button + coordinates) events.
- Input is injected into the engine's existing input pipeline (the `inputs` module).
- The engine does not manage key hold duration — the agent controls all timing by sending discrete events.

### Supervisor Process

- Thin watchdog: only manages process lifecycle and crash log capture.
- Does NOT proxy WebSocket traffic — agent connects directly to the engine.
- On engine crash: captures exit code and stderr, writes to crash log file, reports to agent.
- Agent can instruct supervisor to restart the engine for a retry cycle.

### Crash Handling

- Engine panics terminate the process (no `catch_unwind` — let it die cleanly).
- Supervisor captures the crash log and makes it available.
- Agent reads the crash log, fixes code, recompiles, and re-runs.

### Compile-Time Isolation

- All test harness code lives in `kairos_engine/src/kairos_test_harness/`.
- Gated behind `#[cfg(feature = "test-harness")]` at every entry point.
- Feature flag declared in `Cargo.toml`; not part of default features.
- Release builds have zero test code in the binary.

### Test Trigger Mechanism

Two entry points:
- **CLI**: `cargo run --features test-harness -- --test-file tests/runtime/scenario.toml`
- **WS**: `{"cmd": "run_test", "file": "tests/runtime/scenario.toml"}` to running engine

### Documentation Generation

- Available test commands are documented in `docs/ai/` (following existing project convention).
- Documentation is auto-generated from the hard-coded dispatch match, ensuring it stays in sync.
- Agents read the documentation to discover test capabilities when authoring TOML test files.

### Module Structure

```
kairos_engine/src/
├── kairos_editor/
├── kairos_test_harness.rs # NEW
├── kairos_test_harness/   # NEW
│   ├── mod.rs
│   ├── ws_server.rs       # WebSocket server
│   ├── test_runner.rs     # TOML parser + coroutine executor
│   ├── dispatch.rs        # Hard-coded command dispatch table
│   ├── assertions.rs      # Assertion engine
│   ├── input_injector.rs  # Keyboard/mouse injection
│   └── bridge.rs          # Tokio <-> main thread channel bridge
```

## Testing Decisions

### What Makes a Good Test

- Tests validate **external behavior** only — what the user sees/experiences, not internal implementation details.
- Tests are authored as TOML files, not Rust code, because the primary test author is an AI agent.
- A good TOML test covers a **complete user interaction path** from start to expected outcome.
- Assertions are placed at meaningful checkpoints: after each state-changing operation.
- Tests should be **deterministic** where possible (ECS queries, log patterns) and **observational** where not (crash detection after GPU operations).

### Modules to Test

- The TOML test runner itself should be tested with Rust unit tests (parsing, step execution, error reporting).
- The assertion engine should be unit-tested for each assertion type.
- The WS protocol message serialization should have round-trip tests.
- The channel bridge should be tested for message ordering and error propagation.

### Prior Art

- The project uses `tests/integration/main.rs` as an integration test entry point. The TOML test harness is a higher-level complement.
- The project follows a `docs/ai/` convention for agent-facing documentation.

## Out of Scope

- **Screenshot capture and visual analysis**: v1 does not transmit framebuffers for visual inspection.
- **CI pipeline configuration**: Supervisor and headless mode enable CI, but CI setup is deferred.
- **egui widget-level state querying**: Headless mode bypasses egui. Windowed mode runs egui but agent does not read widget trees.
- **Physics determinism guarantees**: Real-time input injection may have floating-point variance between runs. Fixed-stepping is deferred.
- **Profile-guided test coverage**: No coverage analysis tooling in v1.
- **Crate extraction**: Test harness starts as a module; separate crate is future work.

## Further Notes

- This spec is the result of a 27-question grilling session that resolved all architectural decisions before spec writing.
- Feature flag `test-harness` added to workspace `Cargo.toml`, with `kairos_engine` enabling it conditionally.
- `tokio` already exists in the project, providing the async runtime for WS and test runner.
- The `winit` `EventLoopProxy` pattern already exists in the codebase (used in `main.rs` for `KairosEditorRuntimeEvent`).
- When the test harness is later extracted into its own crate, the engine will need to expose a public API surface.
