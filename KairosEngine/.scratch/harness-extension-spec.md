## Problem Statement

The kairos_test_harness was built to let AI agents verify engine runtime behavior, but its command set is currently limited to a single `system.ping` call and 5 assertions. Real engine features — like the texture inspector's format selection and apply flow, or egui widget interaction paths — cannot be tested because the harness lacks:

- Domain-specific call commands (set format, apply)
- File-content assertions (is the `.texture` file's `format` field actually changed?)
- Widget-level interaction (where is a button on screen? can I click it?)

To validate that the harness itself is effective, we need to exercise it against two real scenarios that require these missing capabilities.

## Solution

Extend the kairos_test_harness with:

1. **New call commands** — `texture_inspector.set_format`, `texture_inspector.apply`, `system.query_widget`
2. **New assertion** — `toml_value_equals`: read a TOML file and assert a key's value matches
3. **Widget rect collection** — egui callback records widget IDs + screen rects into `KairosEngine`, exposed to harness dispatch

Then write two TOML test files that use these capabilities to verify the harness correctly detects both correct behavior and bugs:

- **Case 1**: Set a texture format via inspector → apply → assert the `.texture` file's `format` field changed
- **Case 2**: Inject a known-bad UI (duplicate egui widget ID) → attempt to interact → assert that the expected side-effect (log message, counter increment) did NOT occur

## User Stories

### Harness Extension — Call Commands

1. As an agent, I want to call `texture_inspector.set_format` to change the texture format in the inspector, so that I can simulate what a user does when selecting a format from the dropdown.
2. As an agent, I want to call `texture_inspector.apply` to trigger the inspector's apply/save logic, so that I can simulate clicking the "Apply" button in the inspector UI.
3. As an agent, I want to query a widget's screen position via `system.query_widget`, so that I can compute click coordinates for input injection in windowed mode.

### Harness Extension — Assertions

4. As an agent, I want to assert that a specific key in a TOML file has a given value (`toml_value_equals`), so that I can verify file-based asset changes (like texture format in `.texture` files).

### Harness Extension — Widget Rects

5. As a test author, I want the engine to collect egui widget IDs and their screen-space rectangles during each frame, so that the harness can answer queries about widget positions.
6. As an agent, I want widget rect collection to have zero overhead when the `test-harness` feature is not active, so that release builds are unaffected.

### Test Case 1 — Texture Format Verification

7. As a test designer, I want to write a TOML test that sets a texture format via the inspector, applies the change, and asserts the `.texture` file was updated, so that I can verify the harness correctly validates a correct inspector implementation.
8. As a test designer, I want the same test to FAIL when the inspector implementation has a bug (e.g., missing wgpu feature for the format), so that I can verify the harness catches real bugs.

### Test Case 2 — Widget ID Conflict Detection

9. As a test designer, I want to write a TOML test that queries a widget's position, clicks it, and asserts a side effect occurred (log message or state change), so that I can verify correct widget interaction.
10. As a test designer, I want the same test to FAIL when the widget has a duplicate ID (causing it to be non-interactive), so that I can verify the harness detects rendering/input bugs.
11. As an agent, I want to read `kairos_crash.log` via the supervisor when a test fails, so that I can diagnose whether the failure was a crash or an assertion failure.

## Implementation Decisions

### Widget Rect Collection

- **Storage**: A `HashMap<String, egui::Rect>` field on `KairosEngine`, gated behind `#[cfg(feature = "test-harness")]`.
- **Write path**: In `KairosEditorRuntime::redraw()`, the egui `run_ui` callback calls `engine.record_widget_rect(id_source, response.rect)` for tracked widgets. Widgets register their rects by calling `ui.push_id("descriptive_id")` in the inspector code.
- **Read path**: A `pub(crate) fn widget_rect(&self, id: &str) -> Option<egui::Rect>` accessor on `KairosEngine`. Harness dispatch calls this to serve `system.query_widget`.
- **Lifecycle**: HashMap is cleared at the start of each frame's egui pass, then repopulated by widget draw calls.

### New Call Commands

`texture_inspector.set_format`:
- Receives `format` string arg (e.g., "BC7", "RGBA8")
- Looks up the `TextureInspector` instance via the editor's inspector window
- Sets the format selection on the inspector's model
- Available in windowed mode only (needs egui context)

`texture_inspector.apply`:
- Triggers `TextureInspector::apply()` on the active inspector instance
- Available in windowed mode only

`system.query_widget`:
- Receives `id` string arg
- Returns the widget's `Rect` (min/max in screen coordinates) or an error if not found
- Available in windowed mode only

### New Assertion

`toml_value_equals`:
- Receives `file` (path to TOML file) and `key` (dot-separated key path) and `value` (expected value as string)
- Reads the file, parses as TOML, traverses the dotted key path, compares the value
- `key = "format"` reads `file["format"]`; supports simple top-level keys only in v1
- Available in both modes

### Harness Dispatch Changes

- `dispatch_call` grows three new match arms
- `dispatch_assert` grows one new match arm
- `KairosEngine` gains `record_widget_rect()` and `widget_rect()` methods (cfg-gated)

### Inspector Code Changes (Texture)

- Key widgets (format dropdown, apply button) wrapped in `ui.push_id("descriptive_name")` so their rects are collected with meaningful IDs

## Testing Decisions

### What Makes a Good Test

- Tests validate the **harness's ability to detect bugs**, not the inspector's correctness itself.
- A good TOML test for this spec has a clear pass scenario (harness correctly reports success when code is correct) and a clear fail scenario (harness correctly reports failure when code is buggy).
- Assertions verify external, observable state: file contents, log messages, widget availability.

### Modules to Test

- The new `toml_value_equals` assertion should have unit tests (file exists, key exists, value matches, value mismatches).
- The `system.query_widget` command should have a unit test for the rect lookup logic.
- The two TOML test files are the integration-level validation of the harness itself.

### Prior Art

- Existing unit tests in `assertions.rs` (no_crash, resource_exists, etc.) for assertion logic.
- Existing TOML parsing tests in `test_runner.rs`.
- The `kairos-test` skill (`.agents/skills/kairos-test/SKILL.md`) provides the agent workflow for using the harness.

## Out of Scope

- **Full widget tree dumping**: v1 collects only explicitly tagged widgets via `push_id`. No automatic tree traversal.
- **egui widget state reading**: The harness can query position, but cannot read widget text, enabled state, or selected value.
- **Multi-key TOML path traversal**: `toml_value_equals` supports top-level keys only in v1.
- **Multiple inspector instances**: The harness assumes a single active `TextureInspector`.

## Further Notes

- The two TOML test files are the primary deliverable — they prove the harness works. Implementation order: extend harness → write tests → verify they detect intentional bugs.
- Windowed mode is required for Case 2 (widget interaction). The engine must be launched without `--headless` for widget rect collection to function.
- After implementing, regenerate docs with `cargo run --features test-harness -- --gen-docs`.
