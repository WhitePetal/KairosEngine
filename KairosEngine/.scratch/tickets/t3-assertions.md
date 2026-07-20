## Parent

Part of #5 (Spec: Kairos Test Harness)

## What to build

The assertion engine that validates engine state after each test step. An agent writes `[[step]] action = "assert"` in their TOML test file, choosing from five assertion types. The test runner evaluates each assertion against live engine state, aborts on first failure, and reports a clear error message back via WebSocket.

The five assertion types are:
- `no_crash` — engine has not panicked since the last check
- `ecs_query` — query the ECS world and assert count or component values
- `resource_exists` — assert a file/resource exists at a given path
- `wgpu_valid` — assert a GPU resource (texture, buffer) is valid
- `log_contains` — assert the engine log buffer contains a pattern string

## Acceptance criteria

- [ ] All five assertion types implemented and registered in the dispatch table
- [ ] `no_crash` tracks panic state across test steps
- [ ] `ecs_query` accepts a query string and an `expect` condition (e.g., `"count >= 1"`), evaluates against live ECS world
- [ ] `resource_exists` checks file system path existence
- [ ] `wgpu_valid` checks wgpu resource handle validity
- [ ] `log_contains` searches the engine's in-memory log buffer for a pattern
- [ ] Assertion failure aborts the test immediately with step number and failure reason in WS response
- [ ] Successful assertions produce a `pass` result visible in the test output
- [ ] Each assertion type has a unit test

## Blocked by

- #T2 (Dispatch + TOML Test Runner)