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
//! * [`row`] draws a texture from the texture itself (the host's thumbnail
//!   cache); every other family uses the skin's type icon.
//! * [`display_name`] is what the row shows (and what the rename editor edits):
//!   the extension is not part of a file's name there, it is the right-aligned
//!   [`suffix`] column, so renaming cannot silently change a file's type.
//! * [`suffix`] is the file type label. M5 replaces it with the
//!   `FileTypeRegistry`'s `label`; until then the extension is the best answer and
//!   the only one that cannot disagree with a registry that does not exist.

use std::collections::BTreeSet;

use kairos_editor_core::project::{FileTypeRegistry, NodeId, ProjectTree, ProjectView, RelPath};
use kairos_editor_widgets::tree::{Expander, TreeRow};

/// Icon file paths for one skin, borrowed from the panel's style.
///
/// The generic three are fixed files; the type icons are resolved by the panel
/// from the skin's type-icon root plus the type table's file name (M3/T16), so
/// this struct carries the already-resolved paths keyed by type id.
#[derive(Clone, Copy)]
pub(crate) struct Icons<'a> {
    pub folder: &'a str,
    pub folder_open: &'a str,
    pub file: &'a str,
    pub types: &'a [(&'static str, &'a str)],
    /// The host's thumbnail cache, when there is one. A resolver rather than a
    /// handle: the panel passes its host call, the widget gets only the resulting
    /// texture id, and tests/headless embedders pass [`None`] to keep the type
    /// icon for textures.
    pub thumbnails: Option<&'a dyn Fn(&str, &str) -> Option<egui::TextureId>>,
}

impl<'a> Icons<'a> {
    /// The icon of a registered type family, if this skin has artwork for it.
    ///
    /// The returned path borrows the skin, not the temporary copy of `self`: the
    /// row tuple stores the resolved `&'a str`, so the lifetime has to come from
    /// the slice element rather than from the borrow of this struct.
    fn type_icon(&self, id: &str) -> Option<&'a str> {
        self.types
            .iter()
            .find(|(key, _)| *key == id)
            .map(|(_, path)| *path)
    }
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
    pub(crate) fn update(
        &mut self,
        tree: &ProjectTree,
        view: &ProjectView,
        types: &FileTypeRegistry,
    ) -> &[NodeId] {
        let stale = !self.built
            || self.revision != tree.revision()
            || self.show_hidden != view.show_hidden
            || self.expanded != view.expanded;
        if stale {
            self.rows = tree.visible_rows(view, types);
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
    types: &'a FileTypeRegistry,
    root: &'a str,
    icons: Icons<'a>,
    id: NodeId,
) -> Option<TreeRow<'a>> {
    let node = tree.node(id)?;
    let path = &node.path;
    let name = node.name();
    let is_folder = node.is_folder();
    let expanded = view.expanded.contains(path);
    // A texture row draws the file itself; every other row uses the family icon
    // (or the generic file icon for an unregistered extension).
    let mut thumbnail = None;
    let mut icon = Some(if is_folder {
        if expanded {
            icons.folder_open
        } else {
            icons.folder
        }
    } else {
        let mut type_icon = None;
        if let Some(kind) = types.lookup(name) {
            if kind.thumbnail
                && let Some(resolve) = icons.thumbnails
            {
                thumbnail = resolve(root, path.as_str());
            }
            if thumbnail.is_none() {
                type_icon = icons.type_icon(kind.id);
            }
        }
        // A family without artwork (or one still decoding) falls back to the
        // generic file icon: the table adds families, not panel code.
        type_icon.unwrap_or(icons.file)
    });
    if thumbnail.is_some() {
        icon = None;
    }
    Some(TreeRow {
        depth: depth(path),
        label: display_name(name, is_folder),
        suffix: if is_folder { None } else { suffix(name) },
        icon,
        thumbnail,
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
        thumbnail: None,
        expander: None,
        selected: false,
        separator_above: false,
    }
}

/// Nesting depth of a relative path: the number of `/` separators.
pub(crate) fn depth(path: &RelPath) -> u16 {
    u16::try_from(path.as_str().bytes().filter(|byte| *byte == b'/').count()).unwrap_or(u16::MAX)
}

/// What a row shows and the rename editor edits: folders keep their whole name,
/// files keep their stem.
///
/// The extension of a file is the right-aligned [`suffix`] column, not part of
/// its name. That is the Unity/Tuanjie read, and it is also why a rename cannot
/// change a file's type by accident.
pub(crate) fn display_name(name: &str, is_folder: bool) -> &str {
    if is_folder { name } else { stem(name) }
}

/// The name without its extension: everything before the last interior dot.
///
/// A leading dot is part of the name, not an extension (`.gitignore` has none),
/// and so is a trailing one (`"name."` is the whole name, not an empty
/// extension): both keep the full text.
fn stem(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.is_empty() => stem,
        _ => name,
    }
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
    use kairos_editor_core::project::{FileTypeRegistry, ProjectState, ScanEntry};

    const ROOT: &str = "D:/project";

    const ICONS: Icons<'static> = Icons {
        folder: "folder.png",
        folder_open: "folder_open.png",
        file: "file.png",
        types: &[],
        thumbnails: None,
    };

    /// The same three generic files plus one resolved type icon, for the tests
    /// that care which family drawn.
    const TYPE_ICONS: Icons<'static> = Icons {
        types: &[("texture", "types/texture.png")],
        ..ICONS
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
        project
            .tree
            .visible_rows(&project.view, &project.file_types)
    }

    #[test]
    fn rows_are_rebuilt_when_the_expanded_set_changes() {
        let project = project();
        let mut cache = ProjectRows::default();
        let collapsed = cache
            .update(&project.tree, &project.view, &project.file_types)
            .len();
        assert!(collapsed > 0);
        assert!(
            !cache
                .update(&project.tree, &project.view, &project.file_types)
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
        let expanded = cache
            .update(&project.tree, &view, &project.file_types)
            .len();
        assert!(expanded > collapsed, "expanding must reveal rows");
        assert!(
            cache
                .update(&project.tree, &view, &project.file_types)
                .iter()
                .any(|id| project
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
        let hidden = cache
            .update(&project.tree, &project.view, &project.file_types)
            .len();

        let mut view = project.view.clone();
        view.show_hidden = true;
        let shown = cache
            .update(&project.tree, &view, &project.file_types)
            .len();
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
        let collapsed = row(
            &project.tree,
            &project.view,
            &project.file_types,
            ROOT,
            ICONS,
            id,
        )
        .expect("row");
        let opened = row(&project.tree, &view, &project.file_types, ROOT, ICONS, id).expect("row");
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
        let file = row(
            &project.tree,
            &project.view,
            &project.file_types,
            ROOT,
            ICONS,
            id,
        )
        .expect("row");
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

    /// A file row shows its stem; the extension is the suffix column. A folder
    /// keeps its whole name, dots included ("v1.2" is not a document).
    #[test]
    fn display_names_hide_the_extension_and_only_for_files() {
        assert_eq!(display_name("readme.md", false), "readme");
        assert_eq!(display_name("archive.tar.gz", false), "archive.tar");
        assert_eq!(display_name(".gitignore", false), ".gitignore");
        assert_eq!(display_name("no-extension", false), "no-extension");
        assert_eq!(display_name("name.", false), "name.");
        assert_eq!(display_name("v1.2", true), "v1.2");
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
            row(
                &project.tree,
                &project.view,
                &project.file_types,
                ROOT,
                ICONS,
                library
            )
            .expect("row")
            .separator_above
        );
        assert!(
            !row(
                &project.tree,
                &project.view,
                &project.file_types,
                ROOT,
                ICONS,
                res
            )
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
        assert!(
            row(&project.tree, &view, &project.file_types, ROOT, ICONS, id)
                .expect("row")
                .selected
        );
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
        assert!(
            row(
                &empty,
                &project.view,
                &FileTypeRegistry::builtin(),
                ROOT,
                ICONS,
                stale
            )
            .is_none()
        );
    }

    /// A registered family draws its resolved icon; a family without artwork in
    /// this fixture keeps the generic file icon (M5 adds families, not rows).
    #[test]
    fn registered_types_draw_their_own_icon() {
        let project = project();
        let image = project
            .tree
            .find(&RelPath::new("res/models/Suzanne.png"))
            .expect("Suzanne");
        let textured = row(
            &project.tree,
            &project.view,
            &project.file_types,
            ROOT,
            TYPE_ICONS,
            image,
        )
        .expect("row");
        assert_eq!(textured.icon, Some("types/texture.png"));

        let readme = project
            .tree
            .find(&RelPath::new("res/readme.md"))
            .expect("readme");
        let document = row(
            &project.tree,
            &project.view,
            &project.file_types,
            ROOT,
            TYPE_ICONS,
            readme,
        )
        .expect("row");
        assert_eq!(
            document.icon,
            Some("file.png"),
            "a family without artwork falls back to the generic file icon"
        );
    }

    /// A texture row draws the texture itself: the panel hands the row projection
    /// the host's resolver, and a resolved preview replaces the family glyph. While
    /// the host is still decoding (or has no preview), the family icon stands in.
    #[test]
    fn a_texture_row_draws_its_own_preview_when_the_host_has_one() {
        let project = project();
        let image = project
            .tree
            .find(&RelPath::new("res/models/Suzanne.png"))
            .expect("Suzanne");
        let texture = egui::TextureId::Managed(7);
        let ready = |root: &str, relative: &str| {
            (root == ROOT && relative == "res/models/Suzanne.png").then_some(texture)
        };
        let icons = Icons {
            thumbnails: Some(&ready),
            ..TYPE_ICONS
        };
        let drawn = row(
            &project.tree,
            &project.view,
            &project.file_types,
            ROOT,
            icons,
            image,
        )
        .expect("row");
        assert_eq!(drawn.thumbnail, Some(texture));
        assert_eq!(
            drawn.icon, None,
            "the preview replaces the family glyph instead of painting both"
        );

        let pending = |_root: &str, _relative: &str| None;
        let icons = Icons {
            thumbnails: Some(&pending),
            ..TYPE_ICONS
        };
        let fallback = row(
            &project.tree,
            &project.view,
            &project.file_types,
            ROOT,
            icons,
            image,
        )
        .expect("row");
        assert_eq!(fallback.thumbnail, None);
        assert_eq!(fallback.icon, Some("types/texture.png"));
    }

    #[test]
    fn the_row_count_matches_visible_rows() {
        let project = project();
        let expected = ids(&project);
        let mut cache = ProjectRows::default();
        assert_eq!(
            cache.update(&project.tree, &project.view, &project.file_types),
            expected.as_slice()
        );
    }
}
