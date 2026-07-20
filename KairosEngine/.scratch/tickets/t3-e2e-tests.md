## Parent

Part of #23 (Spec: Harness Extension)

## What to build

Two end-to-end TOML test files that prove the harness correctly detects both correct behavior and bugs:

- **Case 1** (`tests/runtime/texture_format_change.toml`): Calls `texture_inspector.set_format` with a test format, calls `texture_inspector.apply`, then asserts via `toml_value_equals` that the `.texture` file's `format` field was actually changed.
- **Case 2** (`tests/runtime/widget_click.toml`): Queries a widget's position via `system.query_widget`, injects a mouse click at those coordinates, then asserts a side effect occurred (log message containing expected text, or ECS state change). Verifies that when a widget has a duplicate ID and becomes non-interactive, the test FAILS because the expected side effect never occurs.

Both tests run in windowed mode (no `--headless`), because widget rect collection and inspector commands require the egui/render pipeline.

## Acceptance criteria

- [ ] Case 1 TOML test: set_format → apply → toml_value_equals passes
- [ ] Case 1 verified: intentionally break the inspector (e.g., comment out format setter), re-run test, test FAILS
- [ ] Case 2 TOML test: query_widget → compute click coords → click → assert side effect passes
- [ ] Case 2 verified: introduce duplicate widget ID in inspector code, re-run test, test FAILS
- [ ] Both tests documented with inline comments explaining what each step verifies
- [ ] Both tests runnable via `cargo run --features test-harness -- --test-file tests/runtime/<name>.toml`

## Blocked by

- T1 (Widget rect collection + system.query_widget)
- T2 (Inspector commands + toml_value_equals + push_id)