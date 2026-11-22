//! The tree model: assembly, ordering, row projection and identity.
//!
//! Every case builds `ScanEntry` values by hand — no tempdir, no file system —
//! which is the point of keeping the walk in the app: the tree's rules are pure
//! and test in milliseconds.

use kairos_editor_core::project::{
    FileTypeRegistry, Node, NodeKind, ProjectTree, ProjectView, RelPath, ScanEntry,
};

fn dir(path: &str) -> ScanEntry {
    ScanEntry {
        path: RelPath::new(path),
        is_dir: true,
    }
}

fn file(path: &str) -> ScanEntry {
    ScanEntry {
        path: RelPath::new(path),
        is_dir: false,
    }
}

/// A small project: two ordinary folders (one nested), a system dir with its own
/// subtree, plus the file kinds the tree treats specially (a dot entry and a
/// `.meta` companion).
fn sample() -> Vec<ScanEntry> {
    vec![
        dir("res"),
        dir("res/models"),
        file("res/models/Suzanne.png"),
        file("res/models/Suzanne.png.meta"),
        file("res/readme.md"),
        dir("scripts"),
        file("AGENTS.md"),
        file(".gitignore"),
        dir("Library"),
        dir("Library/Logs"),
        file("Library/Logs/run.log"),
    ]
}

fn assemble(entries: &[ScanEntry]) -> ProjectTree {
    let mut tree = ProjectTree::default();
    tree.assemble(entries, &[String::from("Library")], false);
    tree
}

fn names(tree: &ProjectTree, rows: &[kairos_editor_core::project::NodeId]) -> Vec<String> {
    rows.iter()
        .map(|&id| tree.node(id).expect("row node").path.to_string())
        .collect()
}

#[test]
fn a_new_tree_is_a_bare_root() {
    let tree = ProjectTree::default();

    assert_eq!(tree.len(), 1);
    assert!(tree.is_empty());
    assert_eq!(tree.revision(), 0);
    assert!(!tree.truncated());
    let root: &Node = tree.node(tree.root()).expect("root node");
    assert!(root.path.is_root());
    assert_eq!(root.parent, None);
    assert_eq!(root.kind, NodeKind::Folder);
    assert!(root.children.is_empty());
    assert_eq!(tree.find(&RelPath::default()), Some(tree.root()));
}

/// The ordering rule the panel relies on: folders before files at every level,
/// names compared case-insensitively, and the root is the only level where system
/// directories sink below everything.
#[test]
fn assemble_orders_folders_first_then_system_dirs_last() {
    let tree = assemble(&sample());

    let root = tree.children(tree.root());
    let root_names: Vec<&str> = root
        .iter()
        .map(|&id| tree.node(id).expect("node").name())
        .collect();
    assert_eq!(
        root_names,
        vec!["res", "scripts", ".gitignore", "AGENTS.md", "Library"]
    );

    let res = tree.get(&RelPath::new("res")).expect("res");
    let res_names: Vec<&str> = res
        .children
        .iter()
        .map(|&id| tree.node(id).expect("node").name())
        .collect();
    assert_eq!(res_names, vec!["models", "readme.md"]);
}

#[test]
fn names_sort_case_insensitively_with_a_stable_tie_break() {
    let entries = vec![dir("zebra"), dir("Alpha"), dir("alpha"), dir("Beta")];
    let tree = assemble(&entries);
    let names: Vec<&str> = tree
        .children(tree.root())
        .iter()
        .map(|&id| tree.node(id).expect("node").name())
        .collect();

    assert_eq!(names, vec!["Alpha", "alpha", "Beta", "zebra"]);
}

/// A system directory is scanned normally: it is in the tree, its children are
/// there, and only its *position* and its accounting are special.
#[test]
fn system_dirs_are_ordinary_nodes_with_system_positions_and_counts() {
    let tree = assemble(&sample());
    let library = RelPath::new("Library");

    assert!(tree.is_system_dir(&library));
    assert!(!tree.is_system_dir(&RelPath::new("res")));
    assert!(tree.is_under_system(&RelPath::new("Library/Logs/run.log")));
    assert!(!tree.is_under_system(&RelPath::new("res/models")));
    assert_eq!(tree.system_nodes(), 3, "Library, Logs and the log file");
    assert!(tree.get(&RelPath::new("Library/Logs")).is_some());
}

#[test]
fn truncation_is_carried_by_the_snapshot() {
    let mut tree = ProjectTree::default();
    tree.assemble(&sample(), &[String::from("Library")], true);
    assert!(tree.truncated());
}

/// Pre-order, and a collapsed folder hides its whole subtree: the row list is the
/// only thing the panel iterates, so it must never be the whole tree.
#[test]
fn visible_rows_follow_expansion_in_pre_order() {
    let tree = assemble(&sample());
    let mut view = ProjectView::default();

    assert_eq!(
        names(
            &tree,
            &tree.visible_rows(&view, &FileTypeRegistry::builtin())
        ),
        vec!["res", "scripts", "AGENTS.md", "Library"]
    );

    view.expanded.insert(RelPath::new("res"));
    assert_eq!(
        names(
            &tree,
            &tree.visible_rows(&view, &FileTypeRegistry::builtin())
        ),
        vec![
            "res",
            "res/models",
            "res/readme.md",
            "scripts",
            "AGENTS.md",
            "Library"
        ]
    );

    view.expanded.insert(RelPath::new("res/models"));
    assert_eq!(
        names(
            &tree,
            &tree.visible_rows(&view, &FileTypeRegistry::builtin())
        ),
        vec![
            "res",
            "res/models",
            "res/models/Suzanne.png",
            "res/readme.md",
            "scripts",
            "AGENTS.md",
            "Library"
        ]
    );
}

/// Hidden names stay in the snapshot — revealing them must not need a rescan —
/// but the row projection leaves them out until asked.
#[test]
fn hidden_names_are_snapshotted_but_not_drawn_by_default() {
    let tree = assemble(&sample());
    let mut view = ProjectView::default();
    view.expanded.insert(RelPath::new("res"));
    view.expanded.insert(RelPath::new("res/models"));

    assert!(tree.get(&RelPath::new(".gitignore")).is_some());
    assert!(
        tree.get(&RelPath::new("res/models/Suzanne.png.meta"))
            .is_some()
    );
    assert!(
        !names(
            &tree,
            &tree.visible_rows(&view, &FileTypeRegistry::builtin())
        )
        .iter()
        .any(|name| FileTypeRegistry::builtin().is_hidden(name.rsplit('/').next().unwrap_or(name)))
    );

    view.show_hidden = true;
    let rows = names(
        &tree,
        &tree.visible_rows(&view, &FileTypeRegistry::builtin()),
    );
    assert!(rows.contains(&String::from(".gitignore")));
    assert!(rows.contains(&String::from("res/models/Suzanne.png.meta")));
}

#[test]
fn a_file_is_not_a_folder_for_the_expansion_rule() {
    let tree = assemble(&sample());
    let mut view = ProjectView::default();
    view.show_hidden = true;
    view.expanded.insert(RelPath::new("AGENTS.md"));

    assert_eq!(
        names(
            &tree,
            &tree.visible_rows(&view, &FileTypeRegistry::builtin())
        ),
        vec!["res", "scripts", ".gitignore", "AGENTS.md", "Library"]
    );
}

/// The scanner sorts, but `assemble` must not depend on it: a hand-built fixture
/// (a test, a future incremental path) gets the same tree.
#[test]
fn assemble_accepts_unsorted_entries() {
    let mut entries = sample();
    entries.reverse();

    let tree = assemble(&entries);

    assert_eq!(tree.len(), 1 + entries.len());
    assert!(tree.get(&RelPath::new("Library/Logs/run.log")).is_some());
    assert_eq!(
        tree.node(tree.children(tree.root())[0])
            .expect("first root child")
            .name(),
        "res"
    );
}

#[test]
fn every_assemble_moves_the_revision_but_not_on_a_dropped_completion() {
    let mut tree = ProjectTree::default();
    assert_eq!(tree.revision(), 0);
    tree.assemble(&[], &[], false);
    assert_eq!(tree.revision(), 1);
    tree.assemble(&[], &[], false);
    assert_eq!(tree.revision(), 2);
}

/// Lossy UTF-8 conversion can map two distinct names onto one path. The tree must
/// keep the first and report the rest instead of panicking or silently merging.
#[test]
fn a_duplicate_path_is_skipped_and_reported() {
    let entries = vec![dir("res"), file("res/a.png"), file("res/a.png")];
    let mut tree = ProjectTree::default();
    let report = tree.assemble(&entries, &[], false);

    assert_eq!(report.skipped, 1);
    assert!(!report.is_clean());
    assert_eq!(tree.len(), 3, "root, res, one a.png");
}

/// An entry whose parent is not in the snapshot (it was ignored, truncated, or
/// the list came from somewhere other than this walk) cannot be placed.
#[test]
fn an_entry_without_its_parent_is_skipped_and_reported() {
    let entries = vec![file("res/models/a.png")];
    let mut tree = ProjectTree::default();
    let report = tree.assemble(&entries, &[], false);

    assert_eq!(
        report.skipped, 1,
        "its parent folder is not in the snapshot"
    );
    assert!(tree.is_empty());
}

#[test]
fn find_and_get_agree_for_every_snapshotted_path() {
    let tree = assemble(&sample());

    for path in tree.paths() {
        assert!(tree.find(path).is_some());
        assert_eq!(tree.get(path).map(|node| &node.path), Some(path));
    }
    assert_eq!(tree.find(&RelPath::new("res/nope")), None);
}

#[test]
fn rel_paths_split_and_join_without_allocation_surprises() {
    let root = RelPath::default();
    assert!(root.is_root());
    assert_eq!(root.name(), "");
    assert_eq!(root.parent(), None);

    let deep = RelPath::new("res/models/Suzanne.png");
    assert_eq!(deep.name(), "Suzanne.png");
    assert_eq!(deep.parent(), Some(RelPath::new("res/models")));
    assert_eq!(RelPath::new("res").parent(), Some(root.clone()));
    assert_eq!(
        root.child("res").child("models"),
        RelPath::new("res/models")
    );

    assert!(deep.is_under(&RelPath::new("res")));
    assert!(deep.is_under(&deep));
    assert!(!RelPath::new("res").is_under(&deep));
    assert!(!deep.is_under(&root), "the root is not a parent prefix");
    assert!(!RelPath::new("res2").is_under(&RelPath::new("res")));
}

#[test]
fn a_deep_chain_keeps_every_parent_link() {
    let mut entries = Vec::new();
    let mut path = String::new();
    for depth in 0..32 {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(&format!("d{depth}"));
        entries.push(dir(&path));
    }
    let tree = assemble(&entries);

    assert_eq!(tree.len(), 33);
    let leaf = tree.get(&RelPath::new(&path)).expect("leaf");
    assert_eq!(leaf.children.len(), 0);
    let parent = leaf.parent.expect("parent");
    assert_eq!(
        tree.node(parent).expect("parent node").path,
        RelPath::new(
            "d0/d1/d2/d3/d4/d5/d6/d7/d8/d9/d10/d11/d12/d13/d14/d15/d16/d17/d18/d19/d20/d21/d22/d23/d24/d25/d26/d27/d28/d29/d30"
        )
    );
}
