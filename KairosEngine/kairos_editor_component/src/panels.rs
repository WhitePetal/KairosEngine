//! Built-in panels: one file per panel, declared in one static catalog.
//!
//! Adding a panel is:
//!
//! 1. implement [`Panel`](crate::Panel) in `panels/<name>.rs`, with a
//!    `pub(super) const DEF` that names its id and its factory;
//! 2. add one entry to the `panel_catalog!` table below.
//!
//! That entry is the only declaration: the macro emits the `mod` statement, the
//! [`PanelId`] constant and the [`CATALOG`] row from the same line, so none of
//! the three can exist without the others. `scene::DEF` is written against
//! `super::SCENE`, which only exists because the table generated it.
//!
//! One line per panel is the floor, not an oversight: Rust requires every file
//! to be reachable through a `mod` declaration, so even a link-section registry
//! (`inventory`) would still need this statement somewhere. The table makes it
//! the *only* one.
//!
//! The table also takes over the compiler's usual "declared but not compiled"
//! check, so a test covers the other direction: `every_panel_file_is_declared`
//! fails when `src/panels/` holds a panel file the table never mentions. Every
//! top-level file in `src/panels/` is a panel; helper modules live in
//! subdirectories (`about/`, `profiler/`).
//!
//! Keep the macro in this file: the `mod` it emits resolves relative to the
//! file that defines the macro, not the one that invokes it.
//!
//! There is no registration step and no startup pass. [`CATALOG`] is a
//! compile-time table; `PanelHost` consults it when a kind becomes open and
//! builds the instance then, so a panel nobody opens is never constructed and
//! never runs `init`. The app shell does not change: it toggles `PanelId`s in
//! `LayoutState` and builds the default dock out of the ids it knows (About is
//! never in that boot set: it opens from the menu, and `ShowPanel` floats it).
//!
//! `about` is the first panel ported from the legacy `kairos_editor` whose
//! body has real content (the others are still demo bodies that establish the
//! layer). `preferences` is the second: it edits `EditorState::settings` and
//! offers the "Reload" bridge to the per-component skins, replacing the legacy
//! window's reflection over every drawer's style fields. The rest of the real
//! panels are ported one at a time from
//! `prototypes/editor-ui/crates/editor-components` (project first, see
//! `docs/plan/editor-ui-architecture/panel-viewport-design.md` §6.2 P1).

use kairos_editor_core::PanelId;

use crate::panel::PanelCatalog;

/// Declares a built-in panel once: file, id and catalog row.
///
/// Each entry expands to `mod <module>;`, the `PanelId` constant and the
/// [`PanelCatalog`] row that points at `<module>::DEF`. A `DEF` cannot name a
/// constant the table did not generate, and an entry whose module lacks a
/// `DEF` fails to resolve it. A *file* with no entry is invisible to the
/// compiler (nothing declares it), so `every_panel_file_is_declared` compares
/// the table with `src/panels/` on disk.
macro_rules! panel_catalog {
    ($(
        $(#[$attr:meta])*
        $konst:ident = $id:literal => $module:ident
    ),+ $(,)?) => {
        $(
            $(#[$attr])*
            mod $module;

            $(#[$attr])*
            pub const $konst: PanelId = PanelId($id);
        )+

        /// Every built-in panel, declared once.
        ///
        /// Each entry's id and constructor live together in its module's `DEF`,
        /// and this table is generated from the same declaration as the ids and
        /// the `mod` statements, so the three cannot drift apart. It is a
        /// `static`, not a value filled at boot: the host reads it when a panel
        /// opens, and nothing in it runs before that.
        ///
        /// `PanelHost::sync` walks it in declaration order when it allocates
        /// instance serials, so the same open set yields the same ids on every
        /// run.
        pub static CATALOG: PanelCatalog = PanelCatalog::new(&[$($module::DEF),+]);

        /// Module names in table order. Test-only: it is how
        /// `every_panel_file_is_declared` compares the table against the files
        /// on disk.
        #[cfg(test)]
        const PANEL_MODULES: &[&str] = &[$(stringify!($module)),+];
    };
}

panel_catalog! {
    SCENE = "scene" => scene,
    INSPECTOR = "inspector" => inspector,
    CONSOLE = "console" => console,
    HIERARCHY = "hierarchy" => hierarchy,
    PROJECT = "project" => project,
    PROFILER = "profiler" => profiler,
    /// The editor's identity card. Opened from the "Kairos" menu; floated by
    /// `LayoutAction::ShowPanel` like any panel that has no dock tab yet.
    ABOUT = "about" => about,
    /// User preferences. Opened from the "Edit" menu; floats like About.
    PREFERENCES = "preferences" => preferences,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::*;

    /// The table generates the `mod` statements, so an undeclared panel file
    /// would simply never be compiled and nothing would report it. Compare the
    /// table with the directory instead.
    #[test]
    fn every_panel_file_is_declared() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/panels");
        let on_disk: BTreeSet<String> = std::fs::read_dir(&dir)
            .expect("src/panels")
            .map(|entry| entry.expect("directory entry").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .filter_map(|path| Some(path.file_stem()?.to_str()?.to_owned()))
            .collect();
        let declared: BTreeSet<String> = PANEL_MODULES
            .iter()
            .map(|name| (*name).to_owned())
            .collect();

        assert_eq!(
            on_disk, declared,
            "every src/panels/*.rs needs a panel_catalog! entry, and vice versa"
        );
    }

    /// A panel disappearing (file and entry both deleted) leaves the table and
    /// the directory in agreement, which the test above accepts. This number is
    /// the deliberate change signal: update it when the set changes on purpose.
    #[test]
    fn the_table_declares_eight_panels() {
        assert_eq!(CATALOG.len(), 8);
        assert_eq!(PANEL_MODULES.len(), 8);
    }
}
