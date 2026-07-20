## Parent

Part of #5 (Spec: Kairos Test Harness)

## What to build

The command dispatch and TOML test runner that turns the echo bridge into a functional test execution engine. An agent authors a TOML file describing a sequence of call steps, sends `run_test` via WebSocket, and the engine executes each step through a hard-coded dispatch table using coroutine-style async.

At least one real domain command is registered in the dispatch table (e.g., `system.ping` or a simple inspector operation), proving the dispatch → engine function call path works.

## Acceptance criteria

- [ ] Hard-coded `match` dispatch table routes string command names to engine function calls
- [ ] TOML test files parsed into a `Vec<TestStep>` using the existing `toml` crate dependency
- [ ] Test runner implemented as an `async fn` spawned on tokio, awaiting each step's execution via oneshot
- [ ] WS endpoint accepts `{"cmd": "run_test", "file": "..."}` and returns results when test completes
- [ ] At least one domain command registered and executable (e.g., opening an inspector panel, or a simple system call)
- [ ] Test aborts and reports error when a command fails or is unknown
- [ ] Each step execution is traceable — agent can see which step is currently running via WS status updates
- [ ] `cargo test` passes for TOML parsing and dispatch unit tests

## Blocked by

- #T1 (Bootstrap: Feature flag + Module skeleton + WS echo)