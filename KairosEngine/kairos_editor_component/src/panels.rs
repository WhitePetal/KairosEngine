//! Built-in panels: one file per panel, each registering itself.
//!
//! Every panel struct carries the `#[panel]` attribute, which declares its
//! id, implements `PanelRegister` and submits its `PanelDef`; `catalog()`
//! collects the submissions from the link-section table. Nothing here names a
//! panel's factory, and there is no table to keep in sync with the files.
//!
//! What remains in this module is the module plumbing Rust requires: the `mod`
//! declaration makes the file part of the crate, and the `pub use` keeps the
//! public vocabulary at `panels::SCENE`. A file that nothing declares is never
//! compiled, so its `#[panel]` submission never runs.
//!
//! Panels are defined here and nowhere else: `PanelRegister` is sealed, so only
//! `#[panel]` can implement it, and `Panel` requires it.
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

pub use about::ABOUT;
pub use console::CONSOLE;
pub use hierarchy::HIERARCHY;
pub use inspector::INSPECTOR;
pub use profiler::PROFILER;
pub use project::PROJECT;
pub use scene::SCENE;

use crate::panel::PanelCatalog;

/// Every built-in panel, collected from the panel files themselves.
///
/// The submissions share a link-section table; `PanelCatalog::builtin` reads
/// it once and sorts by id, because link order is not source order. The app
/// passes this to `PanelHost::new`, and nothing is constructed until a panel
/// opens.
pub fn catalog() -> PanelCatalog {
    PanelCatalog::builtin()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::*;
    use crate::PanelId;

    /// `mod` is the one list inventory cannot replace: a file nothing declares
    /// is never compiled, so its `#[panel]` submission never runs. Compare the
    /// declarations with `src/panels/` so a forgotten module is a test failure
    /// rather than a panel that silently does not exist.
    #[test]
    fn every_panel_file_is_declared() {
        let declared: BTreeSet<String> = include_str!("panels.rs")
            .lines()
            .filter_map(|line| line.trim().strip_prefix("mod "))
            .filter_map(|line| line.strip_suffix(';'))
            .map(str::to_owned)
            .collect();
        let on_disk: BTreeSet<String> =
            std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/panels"))
                .expect("src/panels")
                .map(|entry| entry.expect("directory entry").path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
                .filter_map(|path| Some(path.file_stem()?.to_str()?.to_owned()))
                .collect();

        assert_eq!(
            on_disk, declared,
            "every src/panels/*.rs needs a `mod` line, and vice versa"
        );
    }

    #[test]
    fn every_builtin_panel_is_collected_once() {
        let catalog = catalog();
        assert_eq!(catalog.len(), 7);

        let mut kinds: Vec<PanelId> = catalog.kinds().collect();
        kinds.sort_unstable();
        let collected = kinds.len();
        kinds.dedup();
        assert_eq!(
            kinds.len(),
            collected,
            "two panel modules submitted the same PanelId"
        );
    }

    #[test]
    fn the_collection_is_sorted_by_id() {
        let kinds: Vec<PanelId> = catalog().kinds().collect();
        let mut sorted = kinds.clone();
        sorted.sort_unstable();
        assert_eq!(
            kinds, sorted,
            "link order is unspecified; the catalog must sort"
        );
    }
}
