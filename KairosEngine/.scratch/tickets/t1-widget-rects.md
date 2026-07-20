## Parent

Part of #23 (Spec: Harness Extension)

## What to build

Add widget rect collection to the engine so the test harness can answer "where is this widget on screen?" and an agent can compute click coordinates for input injection. An agent runs a TOML test that calls `system.query_widget` with a widget ID, receives the screen-space rectangle, and uses those coordinates to click the widget.

The widget rects are stored in `KairosEngine` (gated behind `#[cfg]`), written by the egui draw callback each frame, and read by the harness dispatch.

## Acceptance criteria

- [ ] `KairosEngine` gains a `pub(crate)` method `record_widget_rect(id: &str, rect: egui::Rect)` (cfg-gated)
- [ ] `KairosEngine` gains a `pub(crate)` method `widget_rect(id: &str) -> Option<egui::Rect>` (cfg-gated)
- [ ] `KairosEditorRuntime::redraw()` calls `record_widget_rect` in the egui callback for widgets with `push_id`
- [ ] HashMap cleared at the start of each frame's egui pass
- [ ] `system.query_widget` registered in dispatch, returns rect as JSON or error if not found
- [ ] TOML test verifies query_widget finds a widget with a known ID when the engine is running in windowed mode
- [ ] `cargo check` passes both with and without `--features test-harness`
- [ ] Docs regenerated

## Blocked by

None — can start immediately.