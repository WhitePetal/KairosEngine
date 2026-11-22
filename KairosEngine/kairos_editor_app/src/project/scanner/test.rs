//! The walk against real directory trees.
//!
//! Every case builds a temp directory instead of mocking `std::fs`: the walk is
//! the one piece whose contract *is* the file system, and the rules it applies are
//! pure data from `ScanPolicy`, so a fixture is the honest way to test both.

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use kairos_editor_core::project::ScanPolicy;

use super::scan;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("the fixture directory");
    }
    fs::write(path, text).expect("the fixture file");
}

fn walk(root: &Path) -> super::ScanResult {
    scan(root, &ScanPolicy::default(), 7, &AtomicBool::new(false))
}

fn paths(result: &super::ScanResult) -> Vec<String> {
    result
        .entries
        .iter()
        .map(|entry| entry.path.to_string())
        .collect()
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    write(&dir.path().join("res/models/Suzanne.glb"), "mesh");
    write(&dir.path().join("res/readme.md"), "# readme");
    write(&dir.path().join("scripts/tool.rs"), "fn main() {}");
    write(&dir.path().join("Library/Logs/run.log"), "log");
    dir
}

/// The walk reports folders and files, sorted by path bytes, and a folder always
/// precedes its children — that is what makes `ProjectTree::assemble` a single
/// pass.
#[test]
fn a_nested_tree_is_walked_into_sorted_entries() {
    let dir = project();
    let result = walk(dir.path());

    assert_eq!(
        paths(&result),
        vec![
            "Library",
            "Library/Logs",
            "Library/Logs/run.log",
            "res",
            "res/models",
            "res/models/Suzanne.glb",
            "res/readme.md",
            "scripts",
            "scripts/tool.rs",
        ]
    );
    assert!(!result.truncated);
    assert!(result.errors.is_empty());
    assert_eq!(result.library_nodes, 3, "Library, Logs, run.log");
    assert_eq!(result.generation, 7);
}

/// Ignored directories never reach the snapshot at all — the rule is about the
/// walk, not about drawing.
#[test]
fn an_ignored_directory_is_not_in_the_snapshot() {
    let dir = project();
    write(&dir.path().join("target/debug/junk.rlib"), "junk");
    write(&dir.path().join(".git/config"), "[core]");

    let result = walk(dir.path());
    let paths = paths(&result);

    assert!(!paths.iter().any(|path| path.starts_with("target")));
    assert!(!paths.iter().any(|path| path.starts_with(".git")));
    assert!(paths.contains(&String::from("res")));
}

/// System directories are exempt from the ignore rules: a rule that matches
/// everything must not make the editor's own product directory disappear.
#[test]
fn system_dirs_survive_a_rule_that_matches_everything() {
    let dir = project();
    let mut policy = ScanPolicy::default();
    policy.ignore = kairos_editor_core::project::IgnoreRules::parse(
        [String::from("*")],
    )
    .0;

    let result = scan(dir.path(), &policy, 1, &AtomicBool::new(false));
    let paths = paths(&result);

    assert!(paths.contains(&String::from("Library")));
    assert!(paths.contains(&String::from("Library/Logs/run.log")));
    assert!(!paths.contains(&String::from("res")), "ordinary dirs are ignored");
}

#[test]
fn the_system_dir_budget_truncates_instead_of_flooding_the_walk() {
    let dir = project();
    write(&dir.path().join("Library/Logs/more.log"), "log");
    let mut policy = ScanPolicy::default();
    policy.system_dir_budget = 2;

    let result = scan(dir.path(), &policy, 1, &AtomicBool::new(false));

    assert!(result.truncated);
    assert_eq!(result.library_nodes, 2);
    assert!(paths(&result).contains(&String::from("res/models/Suzanne.glb")));
}

#[test]
fn the_main_node_budget_stops_the_walk() {
    let dir = project();
    let mut policy = ScanPolicy::default();
    // No system dirs: every entry eats the same budget, so the count is exact
    // regardless of the order `read_dir` reports.
    policy.system_dirs.clear();
    policy.max_nodes = 2;

    let result = scan(dir.path(), &policy, 1, &AtomicBool::new(false));

    assert!(result.truncated);
    assert_eq!(result.entries.len(), 2);
}

/// The depth limit keeps the folder row (so the user sees it) and stops below it,
/// marking the snapshot truncated.
#[test]
fn the_depth_limit_stops_descending() {
    let dir = project();
    let mut policy = ScanPolicy::default();
    policy.max_depth = 1;

    let result = scan(dir.path(), &policy, 1, &AtomicBool::new(false));
    let paths = paths(&result);

    assert!(result.truncated);
    assert!(paths.contains(&String::from("res")));
    assert!(paths.contains(&String::from("res/models")), "the folder is still a row");
    assert!(!paths.contains(&String::from("res/models/Suzanne.glb")));
}

/// A directory the walk cannot read is reported, not silently skipped: the old
/// editor's `read_dir` swallowed the error and the folder looked empty.
#[test]
fn an_unreadable_root_is_reported() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let missing = dir.path().join("does-not-exist");

    let result = walk(&missing);

    assert!(result.entries.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].contains("does-not-exist"));
}

#[test]
fn a_cancelled_walk_stops_before_it_starts() {
    let dir = project();
    let cancel = AtomicBool::new(true);

    let result = scan(dir.path(), &ScanPolicy::default(), 1, &cancel);

    assert!(result.entries.is_empty());
}

/// Names that cannot round-trip through UTF-8 are kept with a lossy name; the
/// tree detects the resulting collision. This pins that the walk itself never
/// panics on such a name.
#[cfg(unix)]
#[test]
fn a_non_utf8_name_does_not_break_the_walk() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let dir = tempfile::TempDir::new().expect("a temp dir");
    let name = OsString::from_vec(vec![b'b', 0xff, b'd']);
    fs::create_dir_all(dir.path().join("res")).expect("res");
    fs::write(dir.path().join("res").join(&name), "x").expect("the odd name");

    let result = walk(dir.path());

    assert!(
        paths(&result).iter().any(|path| path.contains('\u{fffd}')),
        "the name is kept lossily: {:?}",
        paths(&result)
    );
    // Exercise the tree with it too: a lossy name must assemble, not panic.
    let mut tree = kairos_editor_core::project::ProjectTree::default();
    let report = tree.assemble(&result.entries, &[], false);
    assert!(report.is_clean());
    let Resolved = tree
        .find(&kairos_editor_core::project::RelPath::new("res"))
        .expect("res assembled");
    assert_eq!(tree.children(Resolved).len(), 1);
}

/// The plan's performance acceptance (§5): 50k nodes scan inside the budget.
///
/// Not a normal test: creating 50k files on NTFS costs an order of magnitude more
/// than scanning them, so this is `#[ignore]` and run once per milestone:
///
/// ```text
/// cargo test -p kairos_editor_app --bins -- --ignored fifty_thousand --nocapture
/// ```
///
/// The measured number belongs in the plan's §5 table; the assertion is a wide
/// debug-build guard, not the release target.
#[test]
#[ignore = "builds 50k files; run manually (see the doc comment)"]
fn fifty_thousand_nodes_scan_inside_the_budget() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let root = dir.path();
    let directories = 250;
    let files_per_directory = 200;
    for directory in 0..directories {
        let path = root.join(format!("res/d{directory:03}"));
        fs::create_dir_all(&path).expect("the fixture directory");
        for file in 0..files_per_directory {
            fs::write(path.join(format!("f{file:04}.txt")), "x").expect("the fixture file");
        }
    }

    let result = walk(root);
    eprintln!(
        "project scan performance: {} entries in {} ms (truncated: {})",
        result.entries.len(),
        result.elapsed_ms,
        result.truncated
    );

    assert!(
        result.entries.len() >= directories * files_per_directory,
        "every fixture file must be in the snapshot"
    );
    assert!(!result.truncated);
    assert!(
        result.elapsed_ms < 1_500,
        "debug-build guard; measured {} ms",
        result.elapsed_ms
    );
}