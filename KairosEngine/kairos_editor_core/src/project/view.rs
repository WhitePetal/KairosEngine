//! The user's intent over the tree: what is expanded, which directory is
//! active, what is selected, and how rows are ordered.
//!
//! This is the half of the panel's view state that survives a restart (M4
//! persists it in `Library/project_view.toml`). Per-frame view state — search
//! text, rename buffer, drag paths, scroll position — belongs to the panel
//! instance, because `PanelHost::sync` drops a closed panel and everything it
//! holds.

use std::collections::BTreeSet;

use super::RelPath;

/// Row ordering. v1 only sorts by name (directories first is the tree's own
/// rule, not a sort key).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortKey {
    #[default]
    Name,
}

/// Persistable view intent over the project tree.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectView {
    /// Folders the user opened. Files never appear here.
    pub expanded: BTreeSet<RelPath>,
    /// The directory the breadcrumb and the right-hand pane show: the selected
    /// folder, or the parent of the selected file. `Navigate` sets it to the
    /// folder it opens; `Select` sets it to the clicked item's directory, so a
    /// single click moves the content pane (M2 acceptance fix).
    pub active: Option<RelPath>,
    /// Selected rows. v1 holds at most one entry; M3 grows it to a real
    /// multi-selection.
    pub selection: BTreeSet<RelPath>,
    pub sort: SortKey,
    /// Draw dot files and companion sidecars (`.meta`). They are in the tree;
    /// this only decides whether they are drawn (M2/M5).
    pub show_hidden: bool,
    /// Left column's fraction of the panel's width (D1: the splitter exists
    /// from the first day because the right pane is part of the layout). The
    /// splitter clamps it to `0.15..=0.6`; the default matches the design.
    pub tree_split: f32,
}

impl Default for ProjectView {
    fn default() -> Self {
        Self {
            expanded: BTreeSet::new(),
            active: None,
            selection: BTreeSet::new(),
            sort: SortKey::Name,
            show_hidden: false,
            tree_split: 0.28,
        }
    }
}
