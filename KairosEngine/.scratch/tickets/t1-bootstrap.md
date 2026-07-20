## Parent

Part of #5 (Spec: Kairos Test Harness)

## What to build

The thinnest possible end-to-end path through the entire test harness stack: a WebSocket server that starts inside the engine (behind a feature flag), accepts connections, and echos back messages — proving the Tokio ↔ main-thread channel bridge works end-to-end.

An agent can `cargo run --features test-harness`, see the WS server listening, connect with any WS client, send a JSON message, and receive the same message echoed back.

## Acceptance criteria

- [ ] `test-harness` feature flag declared in workspace `Cargo.toml`, not in default features
- [ ] `kairos_test_harness` module created under `kairos_engine/src/`, wired into `lib.rs` behind `#[cfg(feature = "test-harness")]`
- [ ] WS server starts on a configurable localhost port when feature is active
- [ ] Channel bridge: WS message → `mpsc::Sender` → main thread drain loop → `oneshot::Sender` → WS response
- [ ] Main thread drains the mpsc queue every frame without blocking the winit event loop
- [ ] Agent can connect via WebSocket, send a JSON message, and receive an echo response
- [ ] Engine still runs normally in editor mode when feature is NOT active
- [ ] `cargo build` succeeds with and without `--features test-harness`

## Blocked by

None — can start immediately.