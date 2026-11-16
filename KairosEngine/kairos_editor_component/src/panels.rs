//! Built-in panels: one file per panel, registered by [`register_default`].
//!
//! Adding a panel is:
//!
//! 1. implement [`Panel`] in `panels/<name>.rs`;
//! 2. give it a [`PanelId`] const here;
//! 3. add one `registry.register` line to [`register_default`].
//!
//! The app shell does not change: it only toggles `PanelId`s in
//! `LayoutState` and builds the default dock out of the ids it knows.
//!
//! These are the demo bodies ported from `kairos_editor_app` to establish the
//! layer. Real content is ported panel by panel from
//! `prototypes/editor-ui/crates/editor-components` (project first, see
//! `docs/plan/editor-ui-architecture/panel-viewport-design.md` §6.2 P1).

mod console;
mod hierarchy;
mod inspector;
mod profiler;
mod project;
mod scene;

use kairos_editor_core::PanelId;

use crate::panel::PanelRegistry;

pub const SCENE: PanelId = PanelId("scene");
pub const INSPECTOR: PanelId = PanelId("inspector");
pub const CONSOLE: PanelId = PanelId("console");
pub const HIERARCHY: PanelId = PanelId("hierarchy");
pub const PROJECT: PanelId = PanelId("project");
pub const PROFILER: PanelId = PanelId("profiler");

/// Register the built-in panels. The app calls this once at composition time.
pub fn register_default(registry: &mut PanelRegistry) {
    registry.register(SCENE, || Box::new(scene::ScenePanel));
    registry.register(INSPECTOR, || Box::new(inspector::InspectorPanel));
    registry.register(CONSOLE, || Box::new(console::ConsolePanel::default()));
    registry.register(HIERARCHY, || Box::new(hierarchy::HierarchyPanel));
    registry.register(PROJECT, || Box::new(project::ProjectPanel));
    registry.register(PROFILER, || Box::new(profiler::ProfilerPanel::default()));
}
