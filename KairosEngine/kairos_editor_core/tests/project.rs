//! Project slice: the request/completion protocol and what a completion does to
//! the state tree.
//!
//! M1 has no scanner here on purpose: the app's walk is tested in
//! `kairos_editor_app`. These cases pin the contract the two sides share — the
//! generation, the assembled tree, the pruned view intent, the pending reveal and
//! the visible error.
//!
//! The M2 panel's own intents (select, expand, navigate, open, split, collapse)
//! are pinned here too: they are plain state transitions over paths the tree
//! already has, so they need no scanner.

use std::path::{Path, PathBuf};

use kairos_editor_core::project::{
    IgnoreRules, ProjectError, ProjectView, RelPath, ScanEntry, ScanPolicy, ScanReason, ScanState,
};
use kairos_editor_core::{
    Action, EditorState, Effect, ProjectAction, ProjectEffect, ProjectState, reduce,
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

fn request(state: &mut EditorState) -> Vec<Effect> {
    reduce(
        state,
        Action::Project(ProjectAction::ScanRequested {
            reason: ScanReason::Startup,
            focus: None,
        }),
    )
}

fn scan_with(state: &mut EditorState, entries: Vec<ScanEntry>) -> Vec<Effect> {
    request(state);
    let generation = state.project.scan.generation;
    reduce(
        state,
        Action::Project(ProjectAction::Scanned {
            generation,
            entries,
            truncated: false,
            elapsed_ms: 7,
            library_nodes: 0,
            errors: Vec::new(),
        }),
    )
}

fn completion(
    generation: u64,
    entries: Vec<ScanEntry>,
    truncated: bool,
    errors: Vec<String>,
) -> Action {
    Action::Project(ProjectAction::Scanned {
        generation,
        entries,
        truncated,
        elapsed_ms: 7,
        library_nodes: 0,
        errors,
    })
}

/// Reduce one project action, without the aggregate `Action` wrapper.
fn view_action(state: &mut EditorState, action: ProjectAction) -> Vec<Effect> {
    reduce(state, Action::Project(action))
}

/// A state whose tree holds `res/models/a.png`, `res/a.png` and `Library`.
fn scanned_state() -> EditorState {
    let mut state = EditorState::new();
    scan_with(
        &mut state,
        vec![
            dir("res"),
            dir("res/models"),
            file("res/models/a.png"),
            file("res/a.png"),
            dir("Library"),
        ],
    );
    state
}

#[test]
fn the_state_tree_starts_with_an_empty_project() {
    let state = EditorState::new();

    assert_eq!(state.project, ProjectState::default());
    assert_eq!(state.project.name, "");
    assert!(state.project.root.as_os_str().is_empty());
    assert_eq!(state.project.policy, ScanPolicy::default());
    assert_eq!(state.project.policy.ignore, IgnoreRules::defaults());
    assert_eq!(state.project.policy.system_dirs, vec![String::from("Library")]);
    assert!(state.project.tree.is_empty());
    assert_eq!(state.project.tree.revision(), 0);
    assert!(state.project.view.expanded.is_empty());
    assert_eq!(state.project.view.active, None);
    assert!(state.project.view.selection.is_empty());
    assert!(!state.project.view.show_hidden);
    assert_eq!(state.project.view.tree_split, 0.28);
    assert_eq!(state.project.scan, ScanState::default());
    assert_eq!(state.project.error, None);
}

/// The root is data supplied by the app's IO boundary (D9); the last component
/// names the project for the panel title.
#[test]
fn a_project_root_supplies_the_name_for_the_panel_title() {
    let project = ProjectState::at(Path::new("D:/Projects/MyGame"));

    assert_eq!(project.name, "MyGame");
    assert_eq!(project.root, PathBuf::from("D:/Projects/MyGame"));
    assert_eq!(project.tree.revision(), 0);
    assert_eq!(project.view, ProjectView::default());
}

/// A root with no final component (a failed `current_dir` at boot) still names
/// the project, so the panel title never renders empty.
#[test]
fn an_empty_root_still_gets_a_name() {
    assert_eq!(ProjectState::at(PathBuf::new()).name, "project");
}

/// The app owns reading `.kairosignore`; the policy it produces is core data, and
/// the next request must be configured with it (R1 in the plan).
#[test]
fn a_loaded_policy_configures_the_next_scan() {
    let mut state = EditorState::new();
    let mut policy = ScanPolicy::default();
    policy.system_dirs = vec![String::from("Vault")];
    policy.max_nodes = 3;

    let effects = reduce(
        &mut state,
        Action::Project(ProjectAction::PolicyLoaded {
            policy: policy.clone(),
        }),
    );

    assert!(effects.is_empty(), "loading a policy only writes state");
    assert_eq!(state.project.policy, policy);

    let effects = request(&mut state);
    let Effect::Project(ProjectEffect::Scan {
        root: _,
        policy: carried,
        generation: _,
    }) = effects[0].clone()
    else {
        panic!("a scan request asks the host to walk");
    };
    assert_eq!(carried, policy);
}

#[test]
fn scan_requested_marks_the_scan_running_and_asks_the_host_to_walk() {
    let mut state = EditorState::new();
    state.project = ProjectState::at(Path::new("res/project"));

    let effects = request(&mut state);

    assert!(state.project.scan.running);
    assert_eq!(state.project.scan.generation, 1);
    assert_eq!(
        effects,
        vec![Effect::Project(ProjectEffect::Scan {
            root: PathBuf::from("res/project"),
            policy: ScanPolicy::default(),
            generation: 1,
        })]
    );
}

#[test]
fn a_request_keeps_the_path_to_reveal_when_the_result_lands() {
    let mut state = EditorState::new();

    reduce(
        &mut state,
        Action::Project(ProjectAction::ScanRequested {
            reason: ScanReason::Manual,
            focus: Some(RelPath::new("res/models")),
        }),
    );

    assert_eq!(
        state.project.scan.focus_after_scan,
        Some(RelPath::new("res/models"))
    );
}

/// A watcher rescan arriving while a rename's reveal is still pending must not
/// cancel it: the path belongs to the newest request that named one.
#[test]
fn a_request_without_a_focus_keeps_a_pending_reveal() {
    let mut state = EditorState::new();
    request(&mut state);
    reduce(
        &mut state,
        Action::Project(ProjectAction::ScanRequested {
            reason: ScanReason::Manual,
            focus: Some(RelPath::new("res/models")),
        }),
    );

    reduce(
        &mut state,
        Action::Project(ProjectAction::ScanRequested {
            reason: ScanReason::ExternalChange,
            focus: None,
        }),
    );

    assert_eq!(
        state.project.scan.focus_after_scan,
        Some(RelPath::new("res/models"))
    );
}

/// A slow scan finishing after a newer request must not overwrite the tree the
/// newer request is about to produce.
#[test]
fn a_stale_scan_completion_is_discarded() {
    let mut state = EditorState::new();
    request(&mut state);
    let effects = request(&mut state);
    assert_eq!(state.project.scan.generation, 2);
    assert_eq!(effects.len(), 1, "the newer request still asks for a walk");

    let effects = reduce(&mut state, completion(1, vec![dir("res")], false, Vec::new()));

    assert!(effects.is_empty(), "a completion never asks for host work");
    assert!(
        state.project.scan.running,
        "the newer scan is still in flight"
    );
    assert_eq!(state.project.scan.last_ms, 0);
    assert_eq!(state.project.tree.revision(), 0);
}

/// A completion for an idle scan (the zero generation, or a duplicate delivery
/// of the same completion) says nothing about the current tree.
#[test]
fn a_completion_for_an_idle_scan_is_discarded() {
    let mut state = EditorState::new();

    reduce(&mut state, completion(0, vec![dir("res")], false, Vec::new()));

    assert!(!state.project.scan.running);
    assert_eq!(state.project.scan.last_ms, 0);
    assert_eq!(state.project.tree.revision(), 0);

    request(&mut state);
    reduce(&mut state, completion(1, vec![dir("res")], false, Vec::new()));
    let revision = state.project.tree.revision();

    reduce(&mut state, completion(1, vec![dir("other")], false, Vec::new()));

    assert_eq!(
        state.project.tree.revision(),
        revision,
        "a second completion for the same generation is ignored"
    );
    assert!(state.project.tree.find(&RelPath::new("other")).is_none());
}

#[test]
fn the_current_completion_assembles_the_tree_and_stops_the_scan() {
    let mut state = EditorState::new();

    let effects = scan_with(
        &mut state,
        vec![
            dir("res"),
            file("res/a.png"),
            file("res/a.png.meta"),
            dir("Library"),
        ],
    );

    assert!(effects.is_empty());
    assert!(!state.project.scan.running);
    assert_eq!(state.project.scan.last_ms, 7);
    assert_eq!(state.project.tree.revision(), 1);
    assert_eq!(state.project.tree.len(), 5, "root plus four entries");
    assert!(state.project.tree.find(&RelPath::new("res/a.png")).is_some());
    assert_eq!(state.project.tree.system_nodes(), 1);
    assert_eq!(state.project.error, None);
}

/// Expansion and selection are user intent over paths; a path the new tree does
/// not have cannot stay expanded or selected.
#[test]
fn a_completion_prunes_intent_for_paths_that_vanished() {
    let mut state = EditorState::new();
    state.project.view.expanded.insert(RelPath::new("old"));
    state.project.view.expanded.insert(RelPath::new("res"));
    state
        .project
        .view
        .selection
        .insert(RelPath::new("res/gone.png"));

    scan_with(&mut state, vec![dir("res"), file("res/a.png")]);

    assert_eq!(
        state.project.view.expanded,
        [RelPath::new("res")].into_iter().collect()
    );
    assert!(state.project.view.selection.is_empty());
}

/// The active directory falls back to its nearest surviving ancestor, not to the
/// root: renaming a directory should not throw the breadcrumb away.
#[test]
fn the_active_directory_falls_back_to_its_nearest_surviving_ancestor() {
    let mut state = EditorState::new();
    state.project.view.active = Some(RelPath::new("res/models"));

    scan_with(&mut state, vec![dir("res")]);

    assert_eq!(state.project.view.active, Some(RelPath::new("res")));
}

#[test]
fn the_active_directory_falls_back_to_the_root_when_nothing_survives() {
    let mut state = EditorState::new();
    state.project.view.active = Some(RelPath::new("gone/deep"));

    scan_with(&mut state, vec![dir("res")]);

    assert_eq!(state.project.view.active, Some(RelPath::default()));
}

/// A rename or create asks the follow-up scan to reveal its result: expand the
/// ancestors, select the path, and show its directory in the breadcrumb.
#[test]
fn a_completion_reveals_the_pending_path_and_expands_its_ancestors() {
    let mut state = EditorState::new();
    request(&mut state);
    reduce(
        &mut state,
        Action::Project(ProjectAction::ScanRequested {
            reason: ScanReason::Manual,
            focus: Some(RelPath::new("res/models/new.png")),
        }),
    );
    let generation = state.project.scan.generation;

    reduce(
        &mut state,
        completion(
            generation,
            vec![
                dir("res"),
                dir("res/models"),
                file("res/models/new.png"),
            ],
            false,
            Vec::new(),
        ),
    );

    assert_eq!(
        state.project.view.expanded,
        [RelPath::new("res"), RelPath::new("res/models")]
            .into_iter()
            .collect()
    );
    assert_eq!(
        state.project.view.selection,
        [RelPath::new("res/models/new.png")].into_iter().collect()
    );
    assert_eq!(state.project.view.active, Some(RelPath::new("res/models")));
    assert_eq!(
        state.project.scan.focus_after_scan, None,
        "a reveal fires once"
    );
}

/// The walk's failures are state, not log lines: the status bar draws this.
#[test]
fn unreadable_directories_become_a_visible_error() {
    let mut state = EditorState::new();
    request(&mut state);

    reduce(
        &mut state,
        completion(
            1,
            vec![dir("res")],
            false,
            vec![String::from("C:/project/res: access denied")],
        ),
    );

    let error = state.project.error.expect("an error is state");
    assert!(error.message.contains("access denied"), "{}", error.message);
    assert!(error.message.contains("unreadable"), "{}", error.message);
}

/// Entries the tree had to drop (a lossy-name collision) are also surfaced.
#[test]
fn dropped_entries_become_a_visible_error() {
    let mut state = EditorState::new();
    request(&mut state);

    reduce(
        &mut state,
        completion(1, vec![file("res/a.png"), file("res/a.png")], false, Vec::new()),
    );

    let error = state.project.error.expect("a dropped entry is visible");
    assert!(error.message.contains("skipped"), "{}", error.message);
}

#[test]
fn a_clean_scan_clears_the_previous_error() {
    let mut state = EditorState::new();
    state.project.error = Some(kairos_editor_core::project::ProjectError::new("old"));

    scan_with(&mut state, vec![dir("res")]);

    assert_eq!(state.project.error, None);
}

#[test]
fn truncation_is_recorded_on_the_state() {
    let mut state = EditorState::new();
    request(&mut state);

    let generation = state.project.scan.generation;
    reduce(
        &mut state,
        completion(generation, vec![dir("res")], true, Vec::new()),
    );

    assert!(state.project.scan.truncated);
    assert!(state.project.tree.truncated());
}

// ---------------------------------------------------------------------------
// M2: the panel's own view intents
// ---------------------------------------------------------------------------

/// A click selects one row; it must not navigate (N1: click selects, double
/// click opens).
#[test]
fn selecting_a_row_replaces_the_selection() {
    let mut state = scanned_state();
    state.project.view.selection.insert(RelPath::new("res/a.png"));

    let effects = view_action(
        &mut state,
        ProjectAction::Select {
            path: Some(RelPath::new("res/models/a.png")),
        },
    );

    assert!(effects.is_empty(), "a selection is pure state");
    assert_eq!(
        state.project.view.selection,
        [RelPath::new("res/models/a.png")].into_iter().collect()
    );
    assert_eq!(
        state.project.view.active, None,
        "a click selects, it does not navigate"
    );
}

/// A path the tree does not have cannot be selected: a stale click clears the
/// selection instead of leaving a dangling path in the view.
#[test]
fn selecting_a_missing_path_leaves_the_selection_empty() {
    let mut state = scanned_state();
    state.project.view.selection.insert(RelPath::new("res/a.png"));

    let effects = view_action(
        &mut state,
        ProjectAction::Select {
            path: Some(RelPath::new("res/gone.png")),
        },
    );

    assert!(effects.is_empty());
    assert!(state.project.view.selection.is_empty());
}

#[test]
fn select_none_clears_the_selection() {
    let mut state = scanned_state();
    state.project.view.selection.insert(RelPath::new("res/a.png"));

    let effects = view_action(&mut state, ProjectAction::Select { path: None });

    assert!(effects.is_empty());
    assert!(state.project.view.selection.is_empty());
}

#[test]
fn toggle_folder_expands_and_collapses_a_folder() {
    let mut state = scanned_state();

    let effects = view_action(
        &mut state,
        ProjectAction::ToggleFolder {
            path: RelPath::new("res"),
        },
    );
    assert!(effects.is_empty());
    assert_eq!(
        state.project.view.expanded,
        [RelPath::new("res")].into_iter().collect()
    );

    let effects = view_action(
        &mut state,
        ProjectAction::ToggleFolder {
            path: RelPath::new("res"),
        },
    );
    assert!(effects.is_empty());
    assert!(state.project.view.expanded.is_empty());
}

/// Only a folder has a triangle; a file reaching the toggle is stale UI input.
#[test]
fn toggle_folder_ignores_a_file() {
    let mut state = scanned_state();

    let effects = view_action(
        &mut state,
        ProjectAction::ToggleFolder {
            path: RelPath::new("res/a.png"),
        },
    );

    assert!(effects.is_empty());
    assert!(state.project.view.expanded.is_empty());
}

#[test]
fn toggle_folder_ignores_a_missing_path() {
    let mut state = scanned_state();

    let effects = view_action(
        &mut state,
        ProjectAction::ToggleFolder {
            path: RelPath::new("gone"),
        },
    );

    assert!(effects.is_empty());
    assert!(state.project.view.expanded.is_empty());
}

/// Double-clicking a folder navigates: it becomes active and selected, and it
/// plus its ancestors expand so the row is visible.
#[test]
fn navigate_sets_active_selection_and_expands_ancestors() {
    let mut state = scanned_state();
    state.project.view.expanded.insert(RelPath::new("Library"));

    let effects = view_action(
        &mut state,
        ProjectAction::Navigate {
            path: RelPath::new("res/models"),
        },
    );

    assert!(effects.is_empty());
    assert_eq!(state.project.view.active, Some(RelPath::new("res/models")));
    assert_eq!(
        state.project.view.expanded,
        [
            RelPath::new("res"),
            RelPath::new("res/models"),
            RelPath::new("Library"),
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        state.project.view.selection,
        [RelPath::new("res/models")].into_iter().collect()
    );
}

#[test]
fn navigate_ignores_a_file() {
    let mut state = scanned_state();

    let effects = view_action(
        &mut state,
        ProjectAction::Navigate {
            path: RelPath::new("res/a.png"),
        },
    );

    assert!(effects.is_empty());
    assert_eq!(state.project.view.active, None);
    assert!(state.project.view.selection.is_empty());
    assert!(state.project.view.expanded.is_empty());
}

#[test]
fn navigate_ignores_a_missing_path() {
    let mut state = scanned_state();

    let effects = view_action(
        &mut state,
        ProjectAction::Navigate {
            path: RelPath::new("res/gone"),
        },
    );

    assert!(effects.is_empty());
    assert_eq!(state.project.view.active, None);
}

/// A request, not a completion: the reducer changes no state and asks the host
/// to open the file, carrying the project root the executor cannot read.
#[test]
fn open_requested_asks_the_host_to_open_the_path() {
    let mut state = EditorState::new();
    state.project = ProjectState::at(Path::new("res/project"));
    scan_with(&mut state, vec![dir("res"), file("res/a.png")]);
    let before = state.project.view.clone();

    let effects = view_action(
        &mut state,
        ProjectAction::OpenRequested {
            path: RelPath::new("res/a.png"),
        },
    );

    assert_eq!(
        effects,
        vec![Effect::Project(ProjectEffect::Open {
            root: PathBuf::from("res/project"),
            path: RelPath::new("res/a.png"),
        })]
    );
    assert_eq!(state.project.view, before, "opening changes no state");
}

#[test]
fn open_requested_ignores_a_missing_path() {
    let mut state = scanned_state();

    let effects = view_action(
        &mut state,
        ProjectAction::OpenRequested {
            path: RelPath::new("res/gone.png"),
        },
    );

    assert!(effects.is_empty(), "there is nothing to open");
}

#[test]
fn set_show_hidden_writes_the_view() {
    let mut state = scanned_state();

    let effects = view_action(&mut state, ProjectAction::SetShowHidden(true));
    assert!(effects.is_empty());
    assert!(state.project.view.show_hidden);

    let effects = view_action(&mut state, ProjectAction::SetShowHidden(false));
    assert!(effects.is_empty());
    assert!(!state.project.view.show_hidden);
}

/// The toolbar collapses rows, not the user's place: active and selection stay.
#[test]
fn collapse_all_keeps_active_and_selection() {
    let mut state = scanned_state();
    state.project.view.expanded.insert(RelPath::new("res"));
    state.project.view.expanded.insert(RelPath::new("res/models"));
    state.project.view.active = Some(RelPath::new("res/models"));
    state.project.view.selection.insert(RelPath::new("res/models/a.png"));

    let effects = view_action(&mut state, ProjectAction::CollapseAll);

    assert!(effects.is_empty());
    assert!(state.project.view.expanded.is_empty());
    assert_eq!(state.project.view.active, Some(RelPath::new("res/models")));
    assert_eq!(
        state.project.view.selection,
        [RelPath::new("res/models/a.png")].into_iter().collect()
    );
}

#[test]
fn tree_split_is_clamped_to_the_documented_range() {
    let mut state = scanned_state();

    for (requested, expected) in [(0.9, 0.6), (0.0, 0.15), (-2.0, 0.15), (0.45, 0.45)] {
        let effects = view_action(&mut state, ProjectAction::SetTreeSplit(requested));
        assert!(effects.is_empty());
        assert_eq!(
            state.project.view.tree_split, expected,
            "requested {requested}"
        );
    }
}

/// NaN would slip through clamp and poison the layout on every frame.
#[test]
fn tree_split_ignores_nan() {
    let mut state = scanned_state();
    state.project.view.tree_split = 0.4;

    let effects = view_action(&mut state, ProjectAction::SetTreeSplit(f32::NAN));

    assert!(effects.is_empty());
    assert_eq!(state.project.view.tree_split, 0.4);
}

/// A host failure is state the status bar renders, not a log line.
#[test]
fn a_host_failure_lands_in_the_visible_error() {
    let mut state = scanned_state();

    let effects = view_action(
        &mut state,
        ProjectAction::Failed {
            message: String::from("cannot open res/a.png"),
        },
    );

    assert!(effects.is_empty());
    assert_eq!(
        state.project.error,
        Some(ProjectError::new("cannot open res/a.png"))
    );
}