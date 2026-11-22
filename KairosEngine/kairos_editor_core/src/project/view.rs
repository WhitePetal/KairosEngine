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

/// How a [`ProjectAction::Select`](super::ProjectAction::Select) changes the
/// selection.
///
/// The three are the desktop list conventions: a click replaces, Ctrl+click
/// toggles one entry, Shift+click extends from an anchor over the rows the user
/// can actually see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectMode {
    /// The clicked paths become the selection.
    Replace,
    /// Each path flips its own membership; the rest of the selection survives.
    Toggle,
    /// `[anchor, focus]`: every visible row between the two, inclusive.
    Range,
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
    /// Selected rows (M3: a real multi-selection).
    pub selection: BTreeSet<RelPath>,
    /// The row a selection gesture extends from: the last row clicked or
    /// toggled, or the row a reveal selected.
    ///
    /// Shift+click and Shift+arrow select the visible span between this path and
    /// the focus, so the anchor belongs next to the selection, not to the panel:
    /// a panel that reopens (or a scan that rebuilds every row) must extend from
    /// the row the user last touched. `settle` prunes it with the selection and
    /// `rewrite_prefixes` remaps it across a rename or a move.
    pub anchor: Option<RelPath>,
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
            anchor: None,
            sort: SortKey::Name,
            show_hidden: false,
            tree_split: 0.28,
        }
    }
}
