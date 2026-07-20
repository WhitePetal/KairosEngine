## Parent

Part of #23 (Spec: Harness Extension)

## What to build

Three capabilities delivered together because they share the engine→inspector reference path:

1. **`texture_inspector.set_format`** — dispatch command that sets the texture format on the active TextureInspector. Receives a `format` string (e.g., "BC7").
2. **`texture_inspector.apply`** — dispatch command that triggers the inspector's apply/save logic.
3. **`toml_value_equals`** — new assertion that reads a TOML file, traverses a top-level key, and asserts its value equals an expected string. Used to verify `.texture` file format field after apply.

Also wraps key inspector widgets (format dropdown, apply button) in `ui.push_id("descriptive_id")` so T1's rect collection can find them by name.

## Acceptance criteria

- [ ] `texture_inspector.set_format` call command registered in dispatch, sets format on active inspector
- [ ] `texture_inspector.apply` call command registered in dispatch, triggers apply on active inspector
- [ ] `toml_value_equals` assertion registered, reads file + key, compares value
- [ ] `toml_value_equals` has unit tests: exists, key matches, key mismatches, file missing
- [ ] Key inspector widgets tagged with `push_id` for rect collection (format dropdown, apply button)
- [ ] All dispatch commands return clear errors when inspector is not active
- [ ] `cargo check` passes both with and without `--features test-harness`
- [ ] Docs regenerated

## Blocked by

None — can start immediately.