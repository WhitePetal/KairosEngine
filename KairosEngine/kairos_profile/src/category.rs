//! Which engine module a captured marker belongs to.
//!
//! Unity gives every `ProfilerMarker` a `ProfilerCategory` at the call site;
//! puffin has no such concept, so the mapping is a rule table over what the
//! capture does record: the marker's name, its source location, and — for the
//! ECS executor's single `Kairos.System` marker — the per-sample data.
//!
//! The table is deliberately coarse. It answers "which engine module spent
//! this time", not "which call site": marker-level detail belongs to the
//! hierarchy view, and the `Scripts` category is refined by the panel into one
//! row per ECS system.

use serde::{Deserialize, Serialize};

/// The buckets the Profiler's CPU chart stacks, in legend order.
///
/// The first five are the engine's core modules — their systems are folded
/// into the module's bucket instead of getting a row of their own. `Scripts`
/// is the rest of the ECS frame (Unity's `Scripts` category, refined by the
/// panel into one row per system), and `Others` is the fallback: scheduler
/// overhead, uninstrumented time, and anything no rule claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Graphics work: wgpu/naga, the egui renderer, `kairos_graphics`.
    Rendering,
    /// The game's own ECS systems (Unity's `Scripts`).
    Scripts,
    /// rapier, `kairos_physics`.
    Physics,
    /// `kairos_audio`.
    Audio,
    /// The editor host: egui/epaint and `Kairos.Editor.*` markers.
    Editor,
    /// Waiting for the presentation engine (surface acquire/present).
    Vsync,
    /// Scheduler overhead, uninstrumented time, and unclaimed markers.
    Others,
}

impl Category {
    /// Every category, in legend order.
    pub const ALL: [Self; 7] = [
        Self::Rendering,
        Self::Scripts,
        Self::Physics,
        Self::Audio,
        Self::Editor,
        Self::Vsync,
        Self::Others,
    ];

    /// Number of categories (the length of a per-frame total array).
    pub const COUNT: usize = Self::ALL.len();

    /// The label shown in the legend.
    pub fn label(self) -> &'static str {
        match self {
            Self::Rendering => "Rendering",
            Self::Scripts => "Scripts",
            Self::Physics => "Physics",
            Self::Audio => "Audio",
            Self::Editor => "Editor",
            Self::Vsync => "VSync",
            Self::Others => "Others",
        }
    }

    /// Classify one marker invocation.
    ///
    /// `name` and `location` are the marker's static metadata (the same for
    /// every sample of that marker); `data` is the per-invocation string, which
    /// is how the executor reports which system ran.
    pub fn of(name: &str, location: &str, data: Option<&str>) -> Self {
        // The executor's single marker: the system name is the data.
        if name == SYSTEM_MARKER {
            return Self::of_system(data.unwrap_or_default());
        }
        // Presentation waits are the one rendering-adjacent thing that is not
        // rendering: they are the frame budget being spent on the display.
        if matches!(name, "Surface::get_current_texture" | "Surface::present") {
            return Self::Vsync;
        }
        if name.starts_with("Kairos.Editor") {
            return Self::Editor;
        }

        let krate = crate_of_location(location);
        if krate.starts_with("wgpu") || krate.starts_with("naga") || krate.starts_with("egui-wgpu")
        {
            return Self::Rendering;
        }
        if krate.starts_with("egui") || krate.starts_with("epaint") {
            return Self::Editor;
        }
        if krate.starts_with("rapier") {
            return Self::Physics;
        }
        if krate.starts_with("kairos_graphics") {
            return Self::Rendering;
        }
        if krate.starts_with("kairos_audio") {
            return Self::Audio;
        }
        if krate.starts_with("kairos_editor") {
            return Self::Editor;
        }
        Self::Others
    }

    /// Classify a system by its registered name.
    ///
    /// `System::name` is `core::any::type_name`, so it carries the defining
    /// crate: `kairos_physics::physics_step_system`. Core-module systems fold
    /// into their module; everything else is the game's `Scripts`.
    pub fn of_system(system: &str) -> Self {
        let krate = system.split("::").next().unwrap_or_default();
        if krate.starts_with("kairos_graphics") {
            return Self::Rendering;
        }
        if krate.starts_with("kairos_physics") {
            return Self::Physics;
        }
        if krate.starts_with("kairos_audio") {
            return Self::Audio;
        }
        if krate.starts_with("kairos_editor") {
            return Self::Editor;
        }
        // The schedule driver and deferred-command application are framework
        // bookkeeping: they would otherwise top the Scripts ranking with the
        // whole frame wrapped inside them.
        if krate == "kairos_ecs" || system.starts_with("kairos_engine::schedule") {
            return Self::Others;
        }
        Self::Scripts
    }
}

/// The marker the ECS executors open around every system run.
pub const SYSTEM_MARKER: &str = "Kairos.System";

/// Markers that only group children and time the scheduler itself.
///
/// Their self time is not attributed to a category: it is either the executor
/// waiting for off-thread work (counted where that work ran) or its own
/// bookkeeping. The panel folds the remainder into `Others`.
pub fn is_frame_wrapper(name: &str) -> bool {
    matches!(name, "Kairos.Engine.Frame" | "Kairos.Stage")
}

/// The crate a source location belongs to: puffin stores `crate/file.rs:line`,
/// with the registry path shortened to `crate-version`.
fn crate_of_location(location: &str) -> &str {
    location.split(['/', '\\']).next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::Category;

    #[test]
    fn presentation_waits_are_vsync() {
        assert_eq!(
            Category::of(
                "Surface::get_current_texture",
                "wgpu-core-29.0.3/src/present.rs:156",
                None
            ),
            Category::Vsync
        );
        assert_eq!(
            Category::of(
                "Surface::present",
                "wgpu-core-29.0.3/src/present.rs:277",
                None
            ),
            Category::Vsync
        );
    }

    #[test]
    fn third_party_markers_follow_their_crate() {
        assert_eq!(
            Category::of(
                "CommandEncoder::finish",
                "wgpu-core-29.0.3/src/command/mod.rs:1698",
                None
            ),
            Category::Rendering
        );
        assert_eq!(
            Category::of("tessellate", "epaint-0.35.0/src/tessellator.rs:2229", None),
            Category::Editor
        );
        assert_eq!(
            Category::of(
                "text_layout::layout",
                "epaint-0.35.0/src/text/text_layout.rs:102",
                None
            ),
            Category::Editor
        );
        assert_eq!(
            Category::of(
                "NarrowPhase::compute_contacts",
                "rapier3d-0.33.0/…/narrow_phase.rs:808",
                None
            ),
            Category::Physics
        );
        assert_eq!(
            Category::of(
                "Kairos.Editor.Panel",
                "kairos_editor_component/src/panel/viewer.rs:61",
                None
            ),
            Category::Editor
        );
    }

    #[test]
    fn systems_fold_into_their_module_or_stay_scripts() {
        let system = super::SYSTEM_MARKER;
        assert_eq!(
            Category::of(
                system,
                "kairos_ecs/src/schedule/executor/multi_threaded.rs:660",
                Some("kairos_physics::physics_step_system")
            ),
            Category::Physics
        );
        assert_eq!(
            Category::of(system, "", Some("kairos_audio::mix_system")),
            Category::Audio
        );
        assert_eq!(
            Category::of(system, "", Some("kairos_graphics::extract_meshes")),
            Category::Rendering
        );
        assert_eq!(
            Category::of(system, "", Some("kairos_editor_app::sync_game_view")),
            Category::Editor
        );
        // A game system is the refinement of Unity's `Scripts`.
        assert_eq!(
            Category::of(system, "", Some("kairos_engine::kairos_game::spin_system")),
            Category::Scripts
        );
        // Framework bookkeeping is not a game system, even though it is
        // scheduled like one.
        assert_eq!(
            Category::of(system, "", Some("kairos_engine::schedule::run_main")),
            Category::Others
        );
        assert_eq!(
            Category::of(system, "", Some("kairos_ecs::apply_deferred")),
            Category::Others
        );
        // The `debug` feature can be off, which blanks every system name.
        assert_eq!(
            Category::of(
                system,
                "",
                Some("<Enable the debug feature to see the name>")
            ),
            Category::Scripts
        );
    }

    #[test]
    fn scheduler_wrappers_and_unknowns_are_others() {
        assert_eq!(
            Category::of("Kairos.Stage", "kairos_engine/src/schedule.rs:223", None),
            Category::Others
        );
        assert_eq!(
            Category::of(
                "Kairos.Engine.Frame",
                "kairos_engine/src/engine.rs:92",
                None
            ),
            Category::Others
        );
        assert!(super::is_frame_wrapper("Kairos.Stage"));
        assert!(super::is_frame_wrapper("Kairos.Engine.Frame"));
        assert!(!super::is_frame_wrapper("Kairos.Editor.Panel"));
        assert_eq!(Category::of("something", "", None), Category::Others);
        assert_eq!(
            Category::of("something", "unknown/src/lib.rs:1", None),
            Category::Others
        );
    }

    #[test]
    fn the_category_order_is_the_legend_order() {
        assert_eq!(Category::ALL.len(), Category::COUNT);
        assert_eq!(Category::ALL[1], Category::Scripts);
        assert_eq!(Category::ALL[5], Category::Vsync);
        assert_eq!(Category::ALL[Category::COUNT - 1], Category::Others);
        for category in Category::ALL {
            assert!(!category.label().is_empty());
        }
    }
}
