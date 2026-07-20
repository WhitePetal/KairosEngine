## Parent

Part of #5 (Spec: Kairos Test Harness)

## What to build

CLI-based test triggering and headless engine mode. An agent runs `cargo run --features test-harness -- --headless --test-file path/to/test.toml` — the engine boots without a window or GPU surface, executes the TOML test, prints results to stdout, and exits with an appropriate exit code. This is the primary CI-compatible execution path.

Headless mode still creates a wgpu adapter and device so GPU-dependent features (texture format validation, shader compilation) can be tested, just without rendering to a screen.

## Acceptance criteria

- [ ] `--test-file <path>` CLI argument accepted by the engine binary
- [ ] `--headless` flag switches engine to headless mode: no winit window, no wgpu surface, but wgpu adapter + device created
- [ ] Engine boots, runs the TOML test, prints pass/fail result to stdout, and exits
- [ ] Exit code 0 on all assertions pass, non-zero on failure or crash
- [ ] Headless mode works without a physical display attached (CI-safe)
- [ ] Windowed mode still works as before when flags are absent
- [ ] WS server still runs in both modes so agent can also trigger tests interactively

## Blocked by

- #T3 (断言引擎)