## Parent

Part of #5 (Spec: Kairos Test Harness)

## What to build

Keyboard and mouse input injection for game runtime testing. An agent writes `action = "input"` steps in a TOML test file to simulate keyboard presses/releases and mouse movements/clicks. The injected input flows through the engine's existing `inputs` module, and the agent verifies the resulting game state using ECS query assertions.

The agent controls all timing by sending discrete events — the engine does not manage key hold duration.

## Acceptance criteria

- [ ] `input` action type registered in the dispatch table
- [ ] Keyboard injection: `device = "keyboard"`, `event = "press" | "release"`, `key = "W"` (and all common keys)
- [ ] Mouse injection: `device = "mouse"`, `event = "move"` with `x`/`y` coordinates, and `event = "click"` with `button = "Left" | "Right" | "Middle"`
- [ ] Injected input reaches the engine's `inputs` module and affects the input state queried by game systems
- [ ] Agent can write a TOML test that injects WASD input, then asserts ECS `Transform` component position changed
- [ ] Input injection works in both windowed and headless modes
- [ ] Unit tests verify input event deserialization and routing

## Blocked by

- #T3 (断言引擎)