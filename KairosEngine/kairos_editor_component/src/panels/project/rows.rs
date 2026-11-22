//! Pure projection from project state to the tree widget's rows.
//!
//! Nothing here touches egui, so the panel's mapping is unit-testable without a
//! context. Three pieces:
//!
//! * [`ProjectRows`] is the panel's visible-row cache: the result of
//!   `ProjectTree::visible_rows` plus the key it was built for. The cache lives in
//!   the panel, not in core (M2 定稿 N2): M6 adds the search query to the same key,
//!   and the query is panel view state, so core could not serve it.
//! * [`row`] maps one node to a [`TreeRow`]: label, kind, icon, suffix, selection
//!   and the system-directory separator.
//! * [`suffix`] is the file type label. M5 replaces it with the
//!   `FileTypeRegistry`'s `label`; until then the extension is the best answer and
//!   the only one that cannot disagree with a registry that does not exist.

use std::collections::BTreeSet;

use kairos_editor_core::project::{NodeId, ProjectTree, ProjectView, RelPath};
use kairos_editor_widgets::tree::{Expander, TreeRow};

/// Icon file paths for one skin, borrowed from the panel's style.
#[derive(Clone, Copy)]
pub(crate) struct Icons<'a> {
    pub folder: &'a str,
    pub folder_open: &'a str,
    pub file: &'a str,
}

/// The panel's visible-row cache.
///
/// The key is exactly what `ProjectTree::visible_rows` reads: the tree's content
/// revision (a rescan rebuilds the table even when nothing is visible changed),
/// `show_hidden`, and the expanded set. M6 appends the search query and scope.
#[derive(Default)]
pub(crate) struct ProjectRows {
    rows: Vec<NodeId>,
    revision: u64,
    show_hidden: bool,
    expanded: BTreeSet<RelPath>,
    /// Distinguishes "not built yet" from "built, currently empty": a tree with
    /// only the root has revision 0 and no rows, which must not look like a miss.
    built: bool,
}

impl ProjectRows {
    /// This frame's rows, rebuilt only when the key changed.
    pub(crate) fn update(&mut self, tree: &ProjectTree, view: &ProjectView) -> &[NodeId] {
        let stale = !self.built
            || self.revision != tree.revision()
            || self.show_hidden != view.show_hidden
            || self.expanded != view.expanded;
        if stale {
            self.rows = tree.visible_rows(view);
            self.revision = tree.revision();
            self.show_hidden = view.show_hidden;
            self.expanded = view.expanded.clone();
            self.built = true;
        }
        &self.rows
    }
}

/// Widget props for one row, or `None` when the id is no longer in the tree.
pub(crate) fn row<'a>(
    tree: &'a ProjectTree,
    view: &ProjectView,
    icons: Icons<'a>,
    id: NodeId,
) -> Option<TreeRow<'a>> {
    let node = tree.node(id)?;
    let path = &node.path;
    let name = node.name();
    let is_folder = node.is_folder();
    let expanded = view.expanded.contains(path);
    Some(TreeRow {
        depth: depth(path),
        label: name,
        suffix: if is_folder { None } else { suffix(name) },
        icon: Some(if is_folder {
            if expanded {
                icons.folder_open
            } else {
                icons.folder
            }
        } else {
            icons.file
        }),
        // A folder with nothing under it has nothing to expand; drawing a
        // triangle anyway would promise rows that never appear (Unity does the
        // same: only non-empty folders show an arrow).
        expander: if is_folder && !node.children.is_empty() {
            Some(if expanded {
                Expander::Expanded
            } else {
                Expander::Collapsed
            })
        } else {
            None
        },
        selected: view.selection.contains(path),
        separator_above: tree.is_system_dir(path),
    })
}

/// A blank row, for a node id the tree no longer has.
///
/// The widget's callback must return a row for every id it is handed; the rows
/// come from the same snapshot, so this is unreachable in practice and exists so
/// a stale id degrades to an empty row instead of a panic.
pub(crate) fn missing_row<'a>() -> TreeRow<'a> {
    TreeRow {
        depth: 0,
        label: "",
        suffix: None,
        icon: None,
        expander: None,
        selected: false,
        separator_above: false,
    }
}

/// Nesting depth of a relative path: the number of `/` separators.
pub(crate) fn depth(path: &RelPath) -> u16 {
    u16::try_from(path.as_str().bytes().filter(|byte| *byte == b'/').count()).unwrap_or(u16::MAX)
}

/// The type label at the end of a file row: the extension, when the name has one.
///
/// A leading dot is part of the name, not an extension (`.gitignore` has no
/// suffix), which is also how Unity labels hidden files.
pub(crate) fn suffix(name: &str) -> Option<&str> {
    let (stem, extension) = name.rsplit_once('.')?;
    if stem.is_empty() || extension.is_empty() {
        return None;
    }
    Some(extension)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_editor_core::project::{ProjectState, ScanEntry};

    const ICONS: Icons<'static> = Icons {
        folder: "folder.png",
        folder_open: "folder_open.png",
        file: "file.png",
    };

    fn entry(path: &str, is_dir: bool) -> ScanEntry {
        ScanEntry {
            path: RelPath::new(path),
            is_dir,
        }
    }

    fn project() -> ProjectState {
        let mut project = ProjectState::at("D:/project");
        project.tree.assemble(
            &[
                entry("res", true),
                entry("res/models", true),
                entry("res/models/Suzanne.png", false),
                entry("res/readme.md", false),
                entry("Library", true),
                entry("Library/Logs", true),
                entry(".git", true),
                entry(".git/HEAD", false),
                entry("target", true),
                entry("res/.hidden.png", false),
            ],
            &[String::from("Library")],
            false,
        );
        project
    }

    fn ids(project: &ProjectState) -> Vec<NodeId> {
        project.tree.visible_rows(&project.view)
    }

    #[test]
    fn rows_are_rebuilt_when_the_expanded_set_changes() {
        let project = project();
        let mut cache = ProjectRows::default();
        let collapsed = cache.update(&project.tree, &project.view).len();
        assert!(collapsed > 0);
        assert!(
            !cache
                .update(&project.tree, &project.view)
                .iter()
                .any(|id| project
                    .tree
                    .node(*id)
                    .is_some_and(|node| node.path.as_str() == "res/models/Suzanne.png")),
            "a collapsed res hides its children"
        );

        let mut view = project.view.clone();
        view.expanded.insert(RelPath::new("res"));
        view.expanded.insert(RelPath::new("res/models"));
        let expanded = cache.update(&project.tree, &view).len();
        assert!(expanded > collapsed, "expanding must reveal rows");
        assert!(
            cache.update(&project.tree, &view).iter().any(|id| project
                .tree
                .node(*id)
                .is_some_and(|node| node.path.as_str() == "res/models/Suzanne.png")),
            "the expanded set must reach the cache key"
        );
    }

    #[test]
    fn the_cache_key_includes_show_hidden() {
        let project = project();
        let mut cache = ProjectRows::default();
        let hidden = cache.update(&project.tree, &project.view).len();

        let mut view = project.view.clone();
        view.show_hidden = true;
        let shown = cache.update(&project.tree, &view).len();
        assert!(shown > hidden, "showing hidden names must reveal rows");
    }

    #[test]
    fn folders_use_the_open_icon_when_expanded() {
        let project = project();
        let mut view = project.view.clone();
        view.expanded.insert(RelPath::new("res"));
        let id = project
            .tree
            .find(&RelPath::new("res"))
            .expect("res is in the snapshot");
        let collapsed = row(&project.tree, &project.view, ICONS, id).expect("row");
        let opened = row(&project.tree, &view, ICONS, id).expect("row");
        assert_eq!(collapsed.icon, Some("folder.png"));
        assert_eq!(opened.icon, Some("folder_open.png"));
        assert_eq!(collapsed.expander, Some(Expander::Collapsed));
        assert_eq!(opened.expander, Some(Expander::Expanded));
        assert_eq!(collapsed.label, "res");
    }

    #[test]
    fn files_carry_their_extension_as_suffix() {
        let project = project();
        let id = project
            .tree
            .find(&RelPath::new("res/readme.md"))
            .expect("readme is in the snapshot");
        let file = row(&project.tree, &project.view, ICONS, id).expect("row");
        assert_eq!(file.suffix, Some("md"));
        assert_eq!(file.icon, Some("file.png"));
        assert_eq!(file.expander, None);
    }

    #[test]
    fn dot_names_have_no_suffix() {
        assert_eq!(suffix("readme.md"), Some("md"));
        assert_eq!(suffix(".gitignore"), None);
        assert_eq!(suffix("no-extension"), None);
        assert_eq!(suffix("archive.tar.gz"), Some("gz"));
    }

    #[test]
    fn system_directories_get_a_separator_above_them() {
        let project = project();
        let library = project
            .tree
            .find(&RelPath::new("Library"))
            .expect("Library is in the snapshot");
        let res = project
            .tree
            .find(&RelPath::new("res"))
            .expect("res is in the snapshot");
        assert!(
            row(&project.tree, &project.view, ICONS, library)
                .expect("row")
                .separator_above
        );
        assert!(
            !row(&project.tree, &project.view, ICONS, res)
                .expect("row")
                .separator_above
        );
    }

    #[test]
    fn depth_counts_separators() {
        assert_eq!(depth(&RelPath::new("res")), 0);
        assert_eq!(depth(&RelPath::new("res/models")), 1);
        assert_eq!(depth(&RelPath::new("res/models/Suzanne.png")), 2);
    }

    #[test]
    fn selected_rows_are_flagged() {
        let project = project();
        let mut view = project.view.clone();
        view.selection.insert(RelPath::new("res/readme.md"));
        let id = project
            .tree
            .find(&RelPath::new("res/readme.md"))
            .expect("readme");
        assert!(row(&project.tree, &view, ICONS, id).expect("row").selected);
    }

    #[test]
    fn a_vanished_id_has_no_row() {
        // Ids are table indices, so a valid id from another tree is an id the
        // empty tree does not have — exactly the stale-id case.
        let project = project();
        let stale = project
            .tree
            .find(&RelPath::new("res/readme.md"))
            .expect("readme");
        let empty = ProjectTree::default();
        assert!(row(&empty, &project.view, ICONS, stale).is_none());
    }

    #[test]
    fn the_row_count_matches_visible_rows() {
        let project = project();
        let expected = ids(&project);
        let mut cache = ProjectRows::default();
        assert_eq!(
            cache.update(&project.tree, &project.view),
            expected.as_slice()
        );
    }
}
