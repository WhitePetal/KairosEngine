//! Marker categories: the vocabulary the Profiler chart stacks, and how a
//! captured marker is read back into it.
//!
//! Puffin has no concept of a category — a scope is registered with a name, a
//! function name and a source location, nothing else. Inferring the category
//! from those strings after the fact (a `Kairos.Editor.` prefix here, a crate
//! path there) is a convention nothing checks: a new module that forgot to
//! follow it lands in `Others` silently.
//!
//! So the category travels with the marker, prepended by the macro that opens
//! it:
//!
//! ```text
//! Editor/Kairos.Editor.Panel
//! ```
//!
//! The part before the `/` is the category tag, and it is written by
//! [`crate::scope!`] from a [`Category`] value, so a call site cannot produce a
//! marker without one. The executor's shared `Kairos.System` marker is the one
//! scope whose category varies per invocation; it carries the tag in its
//! `data` (next to the system name) instead:
//!
//! ```text
//! Kairos.System  [Physics/kairos_physics::physics_step_system]
//! ```
//!
//! The tag goes first because puffin truncates per-sample `data` to 127 bytes:
//! a tag appended to a long generic system name would be cut off.
//!
//! [`strip_tag`] removes the tag for display and [`Category::from_tagged`]
//! reads it back. The parser resolves the category once per marker into
//! `ScopeInfo::category`, so the panel never re-infers it. Untagged markers are
//! third-party scopes (wgpu, rapier, epaint), opened through the `profiling`
//! facade by their own crates; the only signal available for those is the source
//! location, so their fallback table lives here too, next to the parser that
//! decodes it.

use serde::{Deserialize, Serialize};

/// The buckets the Profiler's CPU chart stacks, in legend order.
///
/// `Rendering`, `Physics`, `Audio`, `IO` and `Editor` are the engine's
/// subsystems: their systems run under the subsystem's bucket instead of getting
/// a row of their own. `Scripts` is the rest of the ECS frame (Unity's
/// `Scripts` category, refined by the panel into one row per system), `Vsync`
/// is frame time spent waiting for the presentation engine, and `Others` is the
/// fallback: scheduler overhead, uninstrumented time, and anything no rule
/// claims.
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
    /// Asset loading and file IO (Unity's `Loading`): the `kairos_asset`
    /// server/loader/processor and file-watching scopes.
    IO,
    /// The editor host: egui/epaint and `Kairos.Editor.*` markers.
    Editor,
    /// Waiting for the presentation engine (surface acquire/present).
    Vsync,
    /// Scheduler overhead, uninstrumented time, and unclaimed markers.
    Others,
}

/// An unrecorded or unknown category falls back to `Others`.
impl Default for Category {
    fn default() -> Self {
        Self::Others
    }
}

impl Category {
    /// Every category, in legend order.
    pub const ALL: [Self; 8] = [
        Self::Rendering,
        Self::Scripts,
        Self::Physics,
        Self::Audio,
        Self::IO,
        Self::Editor,
        Self::Vsync,
        Self::Others,
    ];

    /// Number of categories (the length of a per-frame total array).
    pub const COUNT: usize = Self::ALL.len();

    /// The label shown in the legend.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rendering => "Rendering",
            Self::Scripts => "Scripts",
            Self::Physics => "Physics",
            Self::Audio => "Audio",
            Self::IO => "IO",
            Self::Editor => "Editor",
            Self::Vsync => "VSync",
            Self::Others => "Others",
        }
    }

    /// The tag appended to a marker name (the part after the `/`).
    pub const fn tag(self) -> &'static str {
        self.label()
    }

    /// The category a tag stands for, or `None` when it is not a category tag.
    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|category| category.tag() == tag)
    }

    /// The category carried by a tagged string (`"Tag/detail"`), if any.
    ///
    /// Used for marker names and for the ECS executor's per-sample data.
    pub fn from_tagged(text: &str) -> Option<Self> {
        let (tag, _) = text.split_once('/')?;
        Self::from_tag(tag)
    }
}

/// The marker the ECS executors open around every system run.
///
/// It is the one marker whose category varies per invocation, so the tag lives
/// in the sample's data (next to the system name) instead of the marker name.
pub const SYSTEM_MARKER: &str = "Kairos.System";

/// The marker name for `detail` in `category`: `Tag/detail`.
///
/// This is what [`crate::scope!`] passes to the backend; call it directly only
/// when a scope's name has to be built by hand.
pub fn tagged_name(detail: impl AsRef<str>, category: Category) -> String {
    format!("{}/{}", category.tag(), detail.as_ref())
}

/// The detail part of a tagged string (`"Tag/detail"`), or the string itself
/// when it carries no known category tag.
pub fn strip_tag(text: &str) -> &str {
    match text.split_once('/') {
        Some((tag, detail)) if Category::from_tag(tag).is_some() => detail,
        _ => text,
    }
}

/// Classify one marker that is not the executor's system scope.
///
/// A tagged marker answers from its tag. The fallback table below only exists
/// for untagged third-party scopes.
pub fn classify_marker(name: &str, location: &str) -> Category {
    if let Some(category) = Category::from_tagged(name) {
        return category;
    }
    // Presentation waits are the one rendering-adjacent thing that is not
    // rendering: they are the frame budget being spent on the display.
    if matches!(name, "Surface::get_current_texture" | "Surface::present") {
        return Category::Vsync;
    }
    // Asset-server markers are the loading budget even when they are recorded
    // as untagged function scopes: the caller's category would otherwise win.
    if name.starts_with("AssetServer::")
        || name.starts_with("AssetLoader::")
        || name.starts_with("AssetProcessor::")
    {
        return Category::IO;
    }

    match crate_of_location(location) {
        krate
            if krate.starts_with("wgpu")
                || krate.starts_with("naga")
                || krate.starts_with("egui-wgpu") =>
        {
            Category::Rendering
        }
        krate if krate.starts_with("egui") || krate.starts_with("epaint") => Category::Editor,
        krate if krate.starts_with("rapier") => Category::Physics,
        krate if krate.starts_with("kairos_asset") => Category::IO,
        // The debounced filesystem watcher behind asset hot reload: a source
        // scan is asset IO, not editor or rendering work.
        krate if krate.starts_with("notify") => Category::IO,
        krate if krate.starts_with("kairos_graphics") => Category::Rendering,
        krate if krate.starts_with("kairos_audio") => Category::Audio,
        krate if krate.starts_with("kairos_editor") => Category::Editor,
        _ => Category::Others,
    }
}

/// Classify the executor's per-sample data, `"Tag/<system name>"`.
///
/// A capture recorded before the tag existed carries a bare system name; treat
/// it as the game's own code rather than `Others`, because that is what the
/// executor defaulted to when it had no declaration to send.
pub fn classify_system(data: Option<&str>) -> Category {
    data.and_then(Category::from_tagged)
        .unwrap_or(Category::Scripts)
}

/// Markers that only group children and time the scheduler itself.
///
/// Their self time is not attributed to a category: it is either the executor
/// waiting for off-thread work (counted where that work ran) or its own
/// bookkeeping. The panel folds the remainder into `Others`.
pub fn is_frame_wrapper(name: &str) -> bool {
    matches!(strip_tag(name), "Kairos.Engine.Frame" | "Kairos.Stage")
}

/// Systems that only drive a schedule: `run_main`, `run_fixed_main_loop`.
///
/// They are wrappers like the markers [`is_frame_wrapper`] recognises: what they
/// measure is their children's work, already counted where it ran. Their
/// registrations stay `Category::Others` (they are engine rails), so category
/// time drops them by name — the per-invocation data is the only place that
/// name appears, because every system shares the executor's marker.
pub fn is_scheduler_driver(system_name: &str) -> bool {
    matches!(
        strip_tag(system_name),
        "kairos_engine::schedule::run_main" | "kairos_engine::schedule::run_fixed_main_loop"
    )
}

/// The crate a source location belongs to: puffin stores `crate/file.rs:line`,
/// with the registry path shortened to `crate-version`.
fn crate_of_location(location: &str) -> &str {
    location.split(['/', '\\']).next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        Category, SYSTEM_MARKER, classify_marker, classify_system, is_frame_wrapper,
        is_scheduler_driver, strip_tag, tagged_name,
    };

    #[test]
    fn a_typed_marker_names_its_category() {
        assert_eq!(
            tagged_name("Kairos.Editor.Panel", Category::Editor),
            "Editor/Kairos.Editor.Panel"
        );
        assert_eq!(
            strip_tag("Editor/Kairos.Editor.Panel"),
            "Kairos.Editor.Panel"
        );
        assert_eq!(strip_tag("Kairos.Editor.Panel"), "Kairos.Editor.Panel");
        // A '/' after an unknown prefix is part of the name.
        assert_eq!(strip_tag("src/main.rs"), "src/main.rs");
        assert_eq!(
            Category::from_tagged("Physics/kairos_physics::step"),
            Some(Category::Physics)
        );
        assert_eq!(
            tagged_name("Load mesh", Category::IO),
            "IO/Load mesh",
            "asset loading has its own tag"
        );
        assert_eq!(
            Category::from_tagged("IO/kairos_asset::server::load"),
            Some(Category::IO)
        );
        assert_eq!(Category::from_tagged("kairos_physics::step"), None);
    }

    #[test]
    fn the_category_order_is_the_legend_order() {
        assert_eq!(Category::ALL.len(), Category::COUNT);
        assert_eq!(Category::ALL[1], Category::Scripts);
        assert_eq!(Category::ALL[4], Category::IO, "IO sits after Audio");
        assert_eq!(Category::ALL[6], Category::Vsync);
        assert_eq!(Category::ALL[Category::COUNT - 1], Category::Others);
        for category in Category::ALL {
            assert_eq!(Category::from_tag(category.tag()), Some(category));
            assert!(!category.label().is_empty());
        }
        assert_eq!(Category::IO.tag(), "IO");
    }

    #[test]
    fn scheduler_drivers_are_wrappers_not_category_owners() {
        assert!(is_scheduler_driver(
            "Others/kairos_engine::schedule::run_main"
        ));
        assert!(is_scheduler_driver(
            "kairos_engine::schedule::run_fixed_main_loop"
        ));
        assert!(!is_scheduler_driver(
            "Others/kairos_ecs::message::update::message_update_system"
        ));
        assert!(!is_scheduler_driver("Scripts/kairos_game::spin_system"));
        assert!(!is_scheduler_driver(""));
    }

    #[test]
    fn wrappers_are_recognised_with_and_without_their_tag() {
        assert!(is_frame_wrapper("Others/Kairos.Stage"));
        assert!(is_frame_wrapper("Kairos.Stage"));
        assert!(is_frame_wrapper("Others/Kairos.Engine.Frame"));
        assert!(!is_frame_wrapper("Editor/Kairos.Editor.Panel"));
        assert!(!is_frame_wrapper("something"));
    }

    #[test]
    fn tagged_markers_classify_by_tag_not_by_name() {
        assert_eq!(
            classify_marker("Editor/Kairos.Editor.Panel", "epaint-0.35.0/src/lib.rs:1"),
            Category::Editor
        );
        // The tag wins over the location fallback.
        assert_eq!(
            classify_marker("Physics/Anything", "epaint-0.35.0/src/lib.rs:1"),
            Category::Physics
        );
        // Our own naming convention no longer classifies anything.
        assert_eq!(
            classify_marker("Kairos.Editor.Panel", "kairos_editor_component/src/x.rs:1"),
            Category::Editor
        );
        assert_eq!(
            classify_marker("Kairos.Editor.Loose", ""),
            Category::Others,
            "an untagged Kairos marker has no category"
        );
    }

    #[test]
    fn third_party_markers_follow_their_crate() {
        assert_eq!(
            classify_marker(
                "Surface::get_current_texture",
                "wgpu-core-29.0.3/src/present.rs:156"
            ),
            Category::Vsync
        );
        assert_eq!(
            classify_marker("Surface::present", "wgpu-core-29.0.3/src/present.rs:277"),
            Category::Vsync
        );
        assert_eq!(
            classify_marker(
                "CommandEncoder::finish",
                "wgpu-core-29.0.3/src/command/mod.rs:1698"
            ),
            Category::Rendering
        );
        assert_eq!(
            classify_marker("tessellate", "epaint-0.35.0/src/tessellator.rs:2229"),
            Category::Editor
        );
        assert_eq!(
            classify_marker(
                "NarrowPhase::compute_contacts",
                "rapier3d-0.33.0/…/narrow_phase.rs:808"
            ),
            Category::Physics
        );
        // Asset work keeps its own budget even when the scope is untagged.
        assert_eq!(
            classify_marker(
                "load_internal",
                "kairos_asset-0.1.0/src/server/loaders.rs:42"
            ),
            Category::IO
        );
        assert_eq!(
            classify_marker("handle_event", "notify-8.2.0/src/inotify.rs:1"),
            Category::IO
        );
        assert_eq!(
            classify_marker("AssetServer::load", ""),
            Category::IO,
            "an asset-server marker is loading, not Others"
        );
        assert_eq!(classify_marker("something", ""), Category::Others);
    }

    #[test]
    fn systems_take_their_category_from_the_data() {
        assert_eq!(
            classify_system(Some("Physics/kairos_physics::physics_step_system")),
            Category::Physics
        );
        assert_eq!(
            classify_system(Some("Scripts/kairos_game::spin_system")),
            Category::Scripts
        );
        assert_eq!(
            classify_system(Some("Others/kairos_engine::schedule::run_main")),
            Category::Others
        );
        assert_eq!(
            classify_system(Some(
                "IO/kairos_asset::server::handle_internal_asset_events"
            )),
            Category::IO
        );
        // A capture recorded before the tag existed: game code by assumption.
        assert_eq!(
            classify_system(Some("kairos_game::spin_system")),
            Category::Scripts
        );
        assert_eq!(classify_system(None), Category::Scripts);
        assert_eq!(
            classify_system(Some(SYSTEM_MARKER)),
            Category::Scripts,
            "the marker name alone is not data"
        );
    }
}
