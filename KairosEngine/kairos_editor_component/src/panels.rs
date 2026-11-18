//! Built-in panels: one file per panel, registered by [`register_default`].
//!
//! Adding a panel is:
//!
//! 1. implement [`Panel`] in `panels/<name>.rs`;
//! 2. give it a [`PanelId`] const here;
//! 3. add one `registry.register` line to [`register_default`].
//!
//! The app shell does not change: it only toggles `PanelId`s in
//! `LayoutState` and builds the default dock out of the ids it knows (About is
//! never in that boot set: it opens from the menu, and `ShowPanel` floats it).
//!
//! `about` is the first panel ported from the legacy `kairos_editor` whose
//! body has real content (the others are still demo bodies that establish the
//! layer). The rest of the real panels are ported one at a time from
//! `prototypes/editor-ui/crates/editor-components` (project first, see
//! `docs/plan/editor-ui-architecture/panel-viewport-design.md` §6.2 P1).

mod about;
mod console;
mod hierarchy;
mod inspector;
mod profiler;
mod project;
mod scene;

use kairos_editor_core::PanelId;

use crate::panel::PanelRegistry;

pub use about::EditorInfo;

pub const SCENE: PanelId = PanelId("scene");
pub const INSPECTOR: PanelId = PanelId("inspector");
pub const CONSOLE: PanelId = PanelId("console");
pub const HIERARCHY: PanelId = PanelId("hierarchy");
pub const PROJECT: PanelId = PanelId("project");
pub const PROFILER: PanelId = PanelId("profiler");
/// The editor's identity card. Opened from the "Kairos" menu; floated by
/// `LayoutAction::ShowPanel` like any panel that has no dock tab yet.
pub const ABOUT: PanelId = PanelId("about");

/// Register the built-in panels. The app calls this once at composition time.
///
/// `info` is the product identity the About panel displays. It is a parameter
/// because this crate must not depend on `kairos_editor_app` (cargo enforces
/// the layering): the composition root owns what the editor calls itself, the
/// component only draws it. Capturing it in the factory keeps it out of
/// `EditorState` (a constant no action can change is not state) and out of
/// `HostView` (that seam is for per-frame host resources).
pub fn register_default(registry: &mut PanelRegistry, info: EditorInfo) {
    registry.register(SCENE, || Box::new(scene::ScenePanel));
    registry.register(INSPECTOR, || Box::new(inspector::InspectorPanel));
    registry.register(CONSOLE, || Box::new(console::ConsolePanel::default()));
    registry.register(HIERARCHY, || Box::new(hierarchy::HierarchyPanel));
    registry.register(PROJECT, || Box::new(project::ProjectPanel));
    registry.register(PROFILER, || Box::new(profiler::ProfilerPanel::default()));
    registry.register(ABOUT, move || Box::new(about::AboutPanel::new(info)));
}
