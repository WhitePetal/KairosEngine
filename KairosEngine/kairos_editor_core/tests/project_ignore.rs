//! The ignore matcher: the pure half of `.kairosignore`.
//!
//! Table-driven because the grammar is small and each row is a rule a user can
//! actually write; the two consumers (the scanner and the watcher) are pinned
//! separately because they ask different questions of the same rules.

use kairos_editor_core::project::{DEFAULT_IGNORE_LINES, IgnoreRules, RelPath};

fn rules(lines: &[&str]) -> IgnoreRules {
    IgnoreRules::parse(lines.iter().map(|line| (*line).to_owned())).0
}

fn ignored(rules: &IgnoreRules, path: &str) -> bool {
    let path = RelPath::new(path);
    rules.is_ignored(&path, path.name())
}

#[test]
fn the_built_in_defaults_ignore_dot_entries_and_the_build_directory() {
    let rules = IgnoreRules::defaults();

    assert!(ignored(&rules, ".git"));
    assert!(ignored(&rules, ".codegraph"));
    assert!(ignored(&rules, "target"));
    assert!(!ignored(&rules, "res"));
    assert!(!ignored(&rules, "Library"), "system dirs are exempt by the walker, not by the rules");
    assert_eq!(rules.len(), DEFAULT_IGNORE_LINES.len());
}

#[test]
fn a_bare_name_matches_at_any_depth() {
    let rules = rules(&["generated"]);

    assert!(ignored(&rules, "generated"));
    assert!(ignored(&rules, "res/generated"));
    assert!(ignored(&rules, "a/b/c/generated"));
    assert!(!ignored(&rules, "generated_res"));
}

#[test]
fn a_pattern_with_a_slash_is_a_prefix_from_the_project_root() {
    let rules = rules(&["res/generated"]);

    assert!(ignored(&rules, "res/generated"));
    assert!(!ignored(&rules, "res/generated2"));
    assert!(!ignored(&rules, "other/res/generated"));
    assert!(!ignored(&rules, "res"));
}

#[test]
fn a_leading_slash_means_the_same_prefix() {
    let rules = rules(&["/res/generated"]);

    assert!(ignored(&rules, "res/generated"));
    assert!(!ignored(&rules, "other/res/generated"));
}

#[test]
fn a_trailing_slash_is_absorbed() {
    let rules = rules(&["build/", "parts/"]);

    assert!(ignored(&rules, "build"));
    assert!(ignored(&rules, "a/parts"));
}

#[test]
fn segments_support_star_and_question_mark() {
    let rules = rules(&["*.tmp", "cache_?", "res/*/bin"]);

    assert!(ignored(&rules, "work.tmp"));
    assert!(ignored(&rules, "a/work.tmp"));
    assert!(!ignored(&rules, "work.tmp2"));
    assert!(ignored(&rules, "cache_1"));
    assert!(!ignored(&rules, "cache_12"));
    assert!(ignored(&rules, "res/models/bin"));
    assert!(!ignored(&rules, "res/models/target"));
}

#[test]
fn matching_is_case_insensitive_in_both_directions() {
    assert!(ignored(&rules(&["Target"]), "target"));
    assert!(ignored(&rules(&["target"]), "TARGET"));
    assert!(ignored(&rules(&["Res/Generated"]), "res/generated"));
}

#[test]
fn comments_and_blank_lines_are_skipped_and_nothing_is_rejected() {
    let (rules, rejected) = IgnoreRules::parse(
        [
            "# a comment",
            "",
            "   ",
            "target",
            "  # an indented comment",
        ]
        .iter()
        .map(|line| (*line).to_owned()),
    );

    assert!(rejected.is_empty());
    assert_eq!(rules.len(), 1);
    assert!(ignored(&rules, "target"));
}

/// Negation and `**` are not implemented: accepting the line with a different
/// meaning would be worse than reporting it, because the project tree would then
/// disagree with the file the user wrote.
#[test]
fn unsupported_rules_are_reported_not_dropped() {
    let (rules, rejected) = IgnoreRules::parse(
        ["!keep", "res/**/generated", "target"]
            .iter()
            .map(|line| (*line).to_owned()),
    );

    assert_eq!(rejected, vec![String::from("!keep"), String::from("res/**/generated")]);
    assert_eq!(rules.len(), 1);
    assert!(ignored(&rules, "target"));
}

#[test]
fn an_empty_rule_set_ignores_nothing() {
    let rules = IgnoreRules::default();

    assert!(rules.is_empty());
    assert!(!ignored(&rules, "target"));
    assert!(!rules.is_ignored_path(&RelPath::default()));
}

/// The watcher's question: an event only names the changed file, so the rule must
/// match an ancestor directory. This is what stops `target/` churn (57k files in
/// this repo) from triggering a rescan every 300 ms during a build.
#[test]
fn an_event_path_is_ignored_through_any_ancestor() {
    let rules = IgnoreRules::defaults();

    assert!(rules.is_ignored_path(&RelPath::new("target/debug/deps/libfoo.rlib")));
    assert!(rules.is_ignored_path(&RelPath::new(".git/objects/ab/cdef")));
    assert!(!rules.is_ignored_path(&RelPath::new("res/models/Suzanne.png")));
    assert!(!rules.is_ignored_path(&RelPath::new("targeted/file.rs")));
    assert!(!rules.is_ignored_path(&RelPath::default()));
}

#[test]
fn a_prefix_rule_ignores_a_changed_file_below_it() {
    let rules = rules(&["res/generated"]);

    assert!(rules.is_ignored_path(&RelPath::new("res/generated/tex/a.png")));
    assert!(!rules.is_ignored_path(&RelPath::new("res/generatedish/a.png")));
}