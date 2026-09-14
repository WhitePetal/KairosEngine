# Editor

The host and authoring tool built on the Kairos engine.

## Language

**Editor**:
The host application: it owns the window, the UI, and the project browser, and drives the engine through its schedule. The editor depends on the engine; the engine never depends on the editor in return — that one-way edge is the boundary between `kairos_editor` and `kairos_engine`.
*Avoid*: engine, app

**Engine**:
The host-agnostic runtime the editor drives: ECS, rendering, physics, and audio behind `Engine`. It knows nothing of windows, panels, or the project browser.
*Avoid*: editor, host, app

**install**:
The editor's single entry point for mounting what only the editor needs — its own asset stores, syntax settings, texture-edit store, and camera controller — onto the engine's rails. Called exactly once, from `Editor::new`, after `Engine::new`.
*Avoid*: bootstrap, setup, init
