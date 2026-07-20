## Parent

Part of #5 (Spec: Kairos Test Harness)

## What to build

A thin supervisor process that manages the engine's lifecycle and reliably captures crash information. When the engine panics and dies, the supervisor captures the exit code, stderr output, and writes it to a crash log file that the agent can read to diagnose the failure.

The supervisor does NOT proxy WebSocket traffic — the agent continues to connect directly to the engine. The supervisor is purely for process management and crash log capture, enabling the agent's "fix → rebuild → retest" loop to work reliably even when the engine crashes hard.

## Acceptance criteria

- [ ] Supervisor binary (or script) that accepts engine binary path and arguments
- [ ] Supervisor starts the engine as a child process and monitors its status
- [ ] On engine crash (non-zero exit or signal): captures exit code and stderr, writes to `kairos_crash.log` in a well-known location
- [ ] On engine clean exit: supervisor exits cleanly with the same exit code
- [ ] Supervisor does not interfere with engine's stdout/stderr during normal operation
- [ ] Agent can read `kairos_crash.log` after a crash to get the panic message and backtrace
- [ ] Supervisor can be instructed to restart the engine for retry cycles

## Blocked by

- #T4 (CLI + Headless 双模式)