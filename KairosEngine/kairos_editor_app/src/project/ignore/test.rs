//! `.kairosignore` loading and seeding.

use std::fs;
use std::path::Path;

use kairos_editor_core::project::{DEFAULT_IGNORE_LINES, IgnoreRules};

use super::{IGNORE_FILE, load_or_seed, seed_text};

fn ignored(rules: &IgnoreRules, path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    rules.is_ignored(path, name)
}

#[test]
fn the_seed_text_and_the_built_in_fallback_agree() {
    let (rules, rejected) = IgnoreRules::parse(seed_text().lines().map(str::to_owned));

    assert!(rejected.is_empty());
    assert_eq!(rules, IgnoreRules::defaults());
    assert_eq!(rules.len(), DEFAULT_IGNORE_LINES.len());
    assert!(seed_text().starts_with('#'), "the seam is documented in the file");
}

/// A fresh checkout has no ignore file; opening the project writes one in place,
/// the same way missing RON skins are seeded.
#[test]
fn a_missing_file_is_seeded_with_the_defaults() {
    let dir = tempfile::TempDir::new().expect("a temp dir");

    let rules = load_or_seed(dir.path());

    let seeded = fs::read_to_string(dir.path().join(IGNORE_FILE)).expect("the seed file");
    assert!(seeded.starts_with('#'));
    assert_eq!(rules, IgnoreRules::defaults());
    assert!(ignored(&rules, "target"));
}

/// The user's file wins: the fallback is only for a missing file, so editing the
/// file is how a project changes its rules.
#[test]
fn an_existing_file_is_read_and_left_alone() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let path = dir.path().join(IGNORE_FILE);
    fs::write(&path, "# mine\nbuild\n").expect("write the ignore file");

    let rules = load_or_seed(dir.path());

    assert_eq!(fs::read_to_string(&path).expect("unchanged"), "# mine\nbuild\n");
    assert!(ignored(&rules, "build"));
    assert!(!ignored(&rules, "target"), "the defaults are not merged in");
}

/// An unsupported rule is named and skipped; the rest of the file still applies.
#[test]
fn an_unsupported_rule_does_not_disable_the_file() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    fs::write(dir.path().join(IGNORE_FILE), "!keep\ntarget\n").expect("write");

    let rules = load_or_seed(dir.path());

    assert_eq!(rules.len(), 1);
    assert!(ignored(&rules, "target"));
}

/// Reading must never block the editor: a `.kairosignore` that is not a file (a
/// directory, a broken link) falls back to the defaults.
#[test]
fn an_unreadable_file_falls_back_to_the_defaults() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    fs::create_dir(dir.path().join(IGNORE_FILE)).expect("a directory with the file's name");

    let rules = load_or_seed(dir.path());

    assert_eq!(rules, IgnoreRules::defaults());
}

/// The seeder never overwrites: opening a project twice leaves the user's file
/// byte-identical.
#[test]
fn seeding_is_idempotent() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let path: &Path = &dir.path().join(IGNORE_FILE);

    load_or_seed(dir.path());
    let first = fs::read_to_string(path).expect("the seed file");
    load_or_seed(dir.path());
    let second = fs::read_to_string(path).expect("the seed file");

    assert_eq!(first, second);
}