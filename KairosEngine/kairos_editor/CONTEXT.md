# Editor

The host and authoring tool built on the Kairos engine.

## Language

**Editor**:
The authoring layer on the Kairos engine: the panels, the docking UI, the UI context, and the project browser. It is a [`Layer`] on the engine's frame loop (`kairos_engine::runtime`), not a host of its own: it borrows the engine while it draws and never drives the loop. The editor depends on the engine; the engine never depends on the editor in return — that one-way edge is the boundary between `kairos_editor` and `kairos_engine`.
*Avoid*: engine, app

**Engine**:
The runtime: ECS, rendering, physics, audio, input, and the window/frame loop (`EngineRunner`) behind `Engine`. It knows nothing of panels or the project browser.
*Avoid*: editor, app

**Layer**:
The host-specific half of the engine's frame loop: egui state and the UI pass for the editor, nothing at all for a game build. The loop owns the `Engine`; a layer only borrows it. The seam is four methods — `window_created`, `window_event`, `render`, `on_exit`.
*Avoid*: host, plugin, backend

**install**:
The editor's single entry point for mounting what only the editor needs — its own asset stores, syntax settings, texture-edit store, and camera controller — onto the engine's rails. Called exactly once, from `Editor::new`, after `Engine::new`.
*Avoid*: bootstrap, setup, init
