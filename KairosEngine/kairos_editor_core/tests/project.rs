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
    CreateKind, FsOutcome, IgnoreRules, ProjectError, ProjectView, RelPath, ScanEntry, ScanPolicy,
    ScanReason, ScanState, SelectMode,
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
    assert_eq!(
        state.project.policy.system_dirs,
        vec![String::from("Library")]
    );
    assert!(state.project.tree.is_empty());
    assert_eq!(state.project.tree.revision(), 0);
    assert!(state.project.view.expanded.is_empty());
    assert_eq!(state.project.view.active, None);
    assert!(state.project.view.selection.is_empty());
    assert_eq!(state.project.view.anchor, None);
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

    let effects = reduce(
        &mut state,
        completion(1, vec![dir("res")], false, Vec::new()),
    );

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

    reduce(
        &mut state,
        completion(0, vec![dir("res")], false, Vec::new()),
    );

    assert!(!state.project.scan.running);
    assert_eq!(state.project.scan.last_ms, 0);
    assert_eq!(state.project.tree.revision(), 0);

    request(&mut state);
    reduce(
        &mut state,
        completion(1, vec![dir("res")], false, Vec::new()),
    );
    let revision = state.project.tree.revision();

    reduce(
        &mut state,
        completion(1, vec![dir("other")], false, Vec::new()),
    );

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
    assert!(
        state
            .project
            .tree
            .find(&RelPath::new("res/a.png"))
            .is_some()
    );
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
            vec![dir("res"), dir("res/models"), file("res/models/new.png")],
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
        completion(
            1,
            vec![file("res/a.png"), file("res/a.png")],
            false,
            Vec::new(),
        ),
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
    state
        .project
        .view
        .selection
        .insert(RelPath::new("res/a.png"));

    let effects = view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/models/a.png")],
            mode: SelectMode::Replace,
        },
    );

    assert!(effects.is_empty(), "a selection is pure state");
    assert_eq!(
        state.project.view.selection,
        [RelPath::new("res/models/a.png")].into_iter().collect()
    );
    assert_eq!(
        state.project.view.active,
        Some(RelPath::new("res/models")),
        "a click moves the content pane to the picked item's directory"
    );
}

/// A click on a folder activates that folder: the content pane and the
/// breadcrumb follow a single click (M2 acceptance fix).
#[test]
fn selecting_a_folder_activates_it() {
    let mut state = scanned_state();

    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/models")],
            mode: SelectMode::Replace,
        },
    );

    assert_eq!(state.project.view.active, Some(RelPath::new("res/models")));
    assert_eq!(
        state.project.view.selection,
        [RelPath::new("res/models")].into_iter().collect()
    );
}

/// A file has no contents of its own: selecting it shows the folder that holds
/// it, which is what Unity does when an asset is picked in the tree.
#[test]
fn selecting_a_file_activates_its_parent() {
    let mut state = scanned_state();

    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/a.png")],
            mode: SelectMode::Replace,
        },
    );

    assert_eq!(state.project.view.active, Some(RelPath::new("res")));
    assert_eq!(
        state.project.view.selection,
        [RelPath::new("res/a.png")].into_iter().collect()
    );
}

/// A stale click empties the selection but must not move the panel off the
/// directory the user is browsing.
#[test]
fn selecting_a_missing_path_keeps_the_active_directory() {
    let mut state = scanned_state();
    state.project.view.active = Some(RelPath::new("res/models"));

    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/gone.png")],
            mode: SelectMode::Replace,
        },
    );

    assert!(state.project.view.selection.is_empty());
    assert_eq!(state.project.view.active, Some(RelPath::new("res/models")));
}

/// The reducer's `Select` rule is `ProjectTree::content_dir`: a folder maps to
/// itself, a file to its parent, a missing path to nothing.
#[test]
fn content_dir_maps_a_path_to_the_folder_the_panel_shows() {
    let state = scanned_state();
    let tree = &state.project.tree;

    assert_eq!(
        tree.content_dir(&RelPath::new("res/models")),
        Some(RelPath::new("res/models"))
    );
    assert_eq!(
        tree.content_dir(&RelPath::new("res/models/a.png")),
        Some(RelPath::new("res/models"))
    );
    assert_eq!(
        tree.content_dir(&RelPath::new("res/a.png")),
        Some(RelPath::new("res"))
    );
    assert_eq!(tree.content_dir(&RelPath::new("res/gone.png")), None);
}

/// A path the tree does not have cannot be selected: a stale click clears the
/// selection instead of leaving a dangling path in the view.
#[test]
fn selecting_a_missing_path_leaves_the_selection_empty() {
    let mut state = scanned_state();
    state
        .project
        .view
        .selection
        .insert(RelPath::new("res/a.png"));

    let effects = view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/gone.png")],
            mode: SelectMode::Replace,
        },
    );

    assert!(effects.is_empty());
    assert!(state.project.view.selection.is_empty());
}

#[test]
fn select_none_clears_the_selection() {
    let mut state = scanned_state();
    state
        .project
        .view
        .selection
        .insert(RelPath::new("res/a.png"));
    state.project.view.active = Some(RelPath::new("res"));

    let effects = view_action(
        &mut state,
        ProjectAction::Select {
            paths: Vec::new(),
            mode: SelectMode::Replace,
        },
    );

    assert!(effects.is_empty());
    assert!(state.project.view.selection.is_empty());
    assert_eq!(
        state.project.view.active,
        Some(RelPath::new("res")),
        "clearing the selection leaves the content pane where it is"
    );
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
    state
        .project
        .view
        .expanded
        .insert(RelPath::new("res/models"));
    state.project.view.active = Some(RelPath::new("res/models"));
    state
        .project
        .view
        .selection
        .insert(RelPath::new("res/models/a.png"));

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

// ---------------------------------------------------------------------------
// M3: selection gestures and the file-operation protocol
// ---------------------------------------------------------------------------

use kairos_editor_core::project::{FileTypeRegistry, FsOpId, FsOpKind};

/// The file operation an effect asks for, if it asks for one.
fn fs_kind(effects: &[Effect]) -> Option<&FsOpKind> {
    effects.iter().find_map(|effect| match effect {
        Effect::Project(ProjectEffect::Fs(op)) => Some(&op.kind),
        _ => None,
    })
}

fn asks_for_a_scan(effects: &[Effect]) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, Effect::Project(ProjectEffect::Scan { .. })))
}

fn fs_completed(state: &mut EditorState, result: Result<FsOutcome, String>) -> Vec<Effect> {
    view_action(
        state,
        ProjectAction::FsCompleted {
            op: FsOpId(0),
            result,
        },
    )
}

/// Ctrl+click builds a set; it is not navigation, so the content pane stays put.
#[test]
fn toggling_a_row_adds_and_removes_it_without_moving_the_content_pane() {
    let mut state = scanned_state();
    state.project.view.active = Some(RelPath::new("res"));

    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/a.png")],
            mode: SelectMode::Toggle,
        },
    );
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/models/a.png")],
            mode: SelectMode::Toggle,
        },
    );
    assert_eq!(
        state.project.view.selection,
        [RelPath::new("res/a.png"), RelPath::new("res/models/a.png")]
            .into_iter()
            .collect()
    );
    assert_eq!(
        state.project.view.active,
        Some(RelPath::new("res")),
        "extending a selection is not navigating"
    );

    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/a.png")],
            mode: SelectMode::Toggle,
        },
    );
    assert_eq!(
        state.project.view.selection,
        [RelPath::new("res/models/a.png")].into_iter().collect()
    );
}

/// A Replace that names several paths (Ctrl+A) is a selection gesture, not a
/// navigation: the content pane must stay on the directory the user was in.
#[test]
fn replace_with_several_paths_leaves_the_content_pane_alone() {
    let mut state = scanned_state();
    state.project.view.active = Some(RelPath::new("res/models"));
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![
                RelPath::new("res"),
                RelPath::new("res/a.png"),
                RelPath::new("Library"),
            ],
            mode: SelectMode::Replace,
        },
    );

    assert_eq!(state.project.view.selection.len(), 3);
    assert_eq!(
        state.project.view.active,
        Some(RelPath::new("res/models")),
        "selecting everything must not navigate the content pane"
    );
}

/// Shift+click selects the visible interval, inclusive, in row order.
#[test]
fn a_range_selects_the_visible_rows_between_anchor_and_focus() {
    let mut state = scanned_state();
    state.project.view.expanded.insert(RelPath::new("res"));
    state
        .project
        .view
        .expanded
        .insert(RelPath::new("res/models"));

    // Visible order: res, res/models, res/models/a.png, res/a.png, Library.
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/models"), RelPath::new("res/a.png")],
            mode: SelectMode::Range,
        },
    );

    assert_eq!(
        state.project.view.selection,
        [
            RelPath::new("res/models"),
            RelPath::new("res/models/a.png"),
            RelPath::new("res/a.png")
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        state.project.view.active,
        Some(RelPath::new("res")),
        "the content pane follows the focus end of the range"
    );
}

/// A plain click re-anchors on the row it picked; Shift+click has to extend
/// from the user's last click, not from a remembered panel-local variable.
#[test]
fn clicking_anchors_the_row_and_toggling_reanchors_on_the_kept_row() {
    let mut state = scanned_state();
    assert_eq!(state.project.view.anchor, None);

    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/a.png")],
            mode: SelectMode::Replace,
        },
    );
    assert_eq!(state.project.view.anchor, Some(RelPath::new("res/a.png")));

    // Ctrl+click keeps the clicked row, so the next Shift+click extends from it.
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/models/a.png")],
            mode: SelectMode::Toggle,
        },
    );
    assert_eq!(
        state.project.view.anchor,
        Some(RelPath::new("res/models/a.png"))
    );

    // Toggling the anchor off falls back to a row that is still selected: an
    // anchor outside the selection would make the next Shift+click start at a
    // row the user is not pointing at.
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/models/a.png")],
            mode: SelectMode::Toggle,
        },
    );
    assert_eq!(state.project.view.anchor, Some(RelPath::new("res/a.png")));
}

/// Escape clears the selection and with it the anchor: there is no row left for
/// a Shift+click to extend from.
#[test]
fn clearing_the_selection_clears_the_anchor() {
    let mut state = scanned_state();
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res")],
            mode: SelectMode::Replace,
        },
    );
    assert_eq!(state.project.view.anchor, Some(RelPath::new("res")));

    view_action(
        &mut state,
        ProjectAction::Select {
            paths: Vec::new(),
            mode: SelectMode::Replace,
        },
    );
    assert!(state.project.view.selection.is_empty());
    assert_eq!(state.project.view.anchor, None);
}

/// A range extends from its anchor without moving it, so the second Shift+click
/// grows from the same row the first one did.
#[test]
fn a_range_keeps_its_anchor_across_successive_shift_clicks() {
    let mut state = scanned_state();
    state.project.view.expanded.insert(RelPath::new("res"));

    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/models")],
            mode: SelectMode::Replace,
        },
    );
    let anchor = state.project.view.anchor.clone();
    assert_eq!(anchor, Some(RelPath::new("res/models")));

    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/models"), RelPath::new("res/a.png")],
            mode: SelectMode::Range,
        },
    );
    assert_eq!(
        state.project.view.selection.len(),
        2,
        "the first Shift+click selects the span"
    );
    assert_eq!(state.project.view.anchor, anchor);

    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/models"), RelPath::new("Library")],
            mode: SelectMode::Range,
        },
    );
    assert_eq!(
        state.project.view.selection.len(),
        3,
        "the second Shift+click grows from the same anchor"
    );
    assert_eq!(state.project.view.anchor, anchor);
}

/// Collapsing the folder the anchor lives in hides the row without deleting it.
/// The next Shift+click is still a selection: it selects the clicked row and
/// re-anchors there.
#[test]
fn a_range_from_a_hidden_anchor_selects_the_focus_and_reanchors() {
    let mut state = scanned_state();
    state.project.view.expanded.insert(RelPath::new("res"));
    state
        .project
        .view
        .expanded
        .insert(RelPath::new("res/models"));
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/models/a.png")],
            mode: SelectMode::Replace,
        },
    );
    // Collapse the folder that held the anchor: its row is gone from the
    // visible rows, but the path is still in the tree and in no selection once
    // the range runs.
    state
        .project
        .view
        .expanded
        .remove(&RelPath::new("res/models"));

    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/models/a.png"), RelPath::new("res/a.png")],
            mode: SelectMode::Range,
        },
    );

    assert_eq!(
        state.project.view.selection,
        [RelPath::new("res/a.png")].into_iter().collect()
    );
    assert_eq!(
        state.project.view.anchor,
        Some(RelPath::new("res/a.png")),
        "the gesture re-anchors on the row it could actually select"
    );
}

/// A create reveals its new row, which becomes the selection and the anchor: a
/// Shift+click right after creating a folder must extend from that folder.
#[test]
fn a_create_reveal_moves_the_selection_and_the_anchor_to_the_new_row() {
    let mut state = scanned_state();
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res")],
            mode: SelectMode::Replace,
        },
    );

    let effects = fs_completed(
        &mut state,
        Ok(FsOutcome::created(RelPath::new("res/New Folder"))),
    );
    assert!(asks_for_a_scan(&effects));
    let generation = state.project.scan.generation;
    reduce(
        &mut state,
        completion(
            generation,
            vec![
                dir("res"),
                dir("res/New Folder"),
                dir("res/models"),
                file("res/models/a.png"),
                file("res/a.png"),
                dir("Library"),
            ],
            false,
            Vec::new(),
        ),
    );

    assert_eq!(
        state.project.view.selection,
        [RelPath::new("res/New Folder")].into_iter().collect()
    );
    assert_eq!(
        state.project.view.anchor,
        Some(RelPath::new("res/New Folder"))
    );
}

/// The reported bug: create `A`, then create `B` inside it, then Shift+click
/// `A`. The range must run from the last revealed row (`B`) to `A`, so the
/// selection is `{A, B}` — not the parent/siblings a stale anchor produces.
#[test]
fn shift_clicking_after_creating_a_nested_folder_selects_the_two_folders() {
    let mut state = EditorState::new();
    scan_with(
        &mut state,
        vec![dir("parent"), dir("parent/A"), dir("parent/sibling")],
    );
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("parent/A")],
            mode: SelectMode::Replace,
        },
    );

    // Create `parent/A/B`; the create's reveal selects and anchors it.
    fs_completed(
        &mut state,
        Ok(FsOutcome::created(RelPath::new("parent/A/B"))),
    );
    let generation = state.project.scan.generation;
    reduce(
        &mut state,
        completion(
            generation,
            vec![
                dir("parent"),
                dir("parent/A"),
                dir("parent/A/B"),
                dir("parent/sibling"),
            ],
            false,
            Vec::new(),
        ),
    );
    assert_eq!(
        state.project.view.anchor,
        Some(RelPath::new("parent/A/B")),
        "the created row is where the next Shift+click extends from"
    );

    // Visible order after the reveal: parent, parent/A, parent/A/B, parent/sibling.
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("parent/A/B"), RelPath::new("parent/A")],
            mode: SelectMode::Range,
        },
    );
    assert_eq!(
        state.project.view.selection,
        [RelPath::new("parent/A"), RelPath::new("parent/A/B")]
            .into_iter()
            .collect()
    );
}

/// A multi-entry move keeps the whole selection: the focus after the scan is one
/// of the moved rows, so the reveal must not replace the selection with it.
#[test]
fn a_move_of_a_multi_selection_keeps_every_entry_selected() {
    let mut state = EditorState::new();
    scan_with(
        &mut state,
        vec![
            dir("res"),
            file("res/a.png"),
            file("res/b.png"),
            dir("Library"),
        ],
    );
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/a.png"), RelPath::new("res/b.png")],
            mode: SelectMode::Replace,
        },
    );
    assert_eq!(state.project.view.selection.len(), 2);

    let effects = fs_completed(
        &mut state,
        Ok(FsOutcome::moved(vec![
            (RelPath::new("res/a.png"), RelPath::new("Library/a.png")),
            (RelPath::new("res/b.png"), RelPath::new("Library/b.png")),
        ])),
    );
    assert!(asks_for_a_scan(&effects));
    let generation = state.project.scan.generation;
    reduce(
        &mut state,
        completion(
            generation,
            vec![
                dir("res"),
                dir("Library"),
                file("Library/a.png"),
                file("Library/b.png"),
            ],
            false,
            Vec::new(),
        ),
    );

    assert_eq!(
        state.project.view.selection,
        [RelPath::new("Library/a.png"), RelPath::new("Library/b.png")]
            .into_iter()
            .collect(),
        "a multi-entry move stays multi-selected after the rescan"
    );
    assert_eq!(
        state.project.view.anchor,
        Some(RelPath::new("Library/b.png")),
        "the focus of the reveal is the last moved row, not an unrelated one"
    );
    assert_eq!(state.project.view.active, Some(RelPath::new("Library")));
}

/// A trashed anchor cannot stay in the view: the scan prunes it together with
/// the selection and falls back to a row that survived.
#[test]
fn a_trashed_anchor_falls_back_to_the_surviving_selection() {
    let mut state = scanned_state();
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("Library")],
            mode: SelectMode::Replace,
        },
    );
    view_action(
        &mut state,
        ProjectAction::Select {
            paths: vec![RelPath::new("res/a.png")],
            mode: SelectMode::Toggle,
        },
    );
    assert_eq!(state.project.view.anchor, Some(RelPath::new("res/a.png")));

    fs_completed(
        &mut state,
        Ok(FsOutcome::trashed(vec![RelPath::new("res/a.png")])),
    );
    let generation = state.project.scan.generation;
    reduce(
        &mut state,
        completion(
            generation,
            vec![
                dir("res"),
                dir("res/models"),
                file("res/models/a.png"),
                dir("Library"),
            ],
            false,
            Vec::new(),
        ),
    );

    assert_eq!(
        state.project.view.selection,
        [RelPath::new("Library")].into_iter().collect()
    );
    assert_eq!(
        state.project.view.anchor,
        Some(RelPath::new("Library")),
        "the anchor falls back to a row that is still there"
    );
}

/// A create passes the type table's template through and asks the host, with the
/// extension appended exactly once.
#[test]
fn a_create_asks_for_host_work_with_the_template_extension() {
    let mut state = scanned_state();

    let effects = view_action(
        &mut state,
        ProjectAction::CreateRequested {
            parent: RelPath::new("res"),
            name: String::from("New Document"),
            kind: CreateKind::File {
                extension: "md",
                contents: "# ",
            },
        },
    );

    assert_eq!(
        fs_kind(&effects),
        Some(&FsOpKind::Create {
            parent: RelPath::new("res"),
            name: String::from("New Document.md"),
            kind: CreateKind::File {
                extension: "md",
                contents: "# "
            },
        })
    );
    assert_eq!(
        state.project.next_fs_op,
        FsOpId(1),
        "the reducer owns request ids"
    );
}

/// Creating a folder asks for a folder, not a file.
#[test]
fn a_create_without_a_template_makes_a_folder() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::CreateRequested {
            parent: RelPath::new("res"),
            name: String::from("New Folder"),
            kind: CreateKind::Folder,
        },
    );
    assert_eq!(
        fs_kind(&effects),
        Some(&FsOpKind::Create {
            parent: RelPath::new("res"),
            name: String::from("New Folder"),
            kind: CreateKind::Folder
        })
    );
}

/// A name the table rejects never reaches the host.
#[test]
fn an_invalid_create_name_is_a_visible_error_and_no_host_work() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::CreateRequested {
            parent: RelPath::new("res"),
            name: String::from("bad/name"),
            kind: CreateKind::Folder,
        },
    );

    assert!(effects.is_empty());
    assert!(
        fs_kind(&effects).is_none(),
        "the file system is never asked to do something already known to be wrong"
    );
    let error = state.project.error.expect("the failure is visible");
    // A rejected entry request is a one-off notice: no host work follows, so no
    // rescan either, and a later clean scan may supersede it.
    assert!(error.clears_on_scan);
    assert!(error.message.contains("cannot contain"));
}

/// A sister with the same name (case-insensitively) is a visible error.
#[test]
fn a_taken_create_name_is_rejected() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::CreateRequested {
            parent: RelPath::new("res"),
            name: String::from("A.PNG"),
            kind: CreateKind::File {
                extension: "png",
                contents: "",
            },
        },
    );

    assert!(effects.is_empty());
    assert!(
        state
            .project
            .error
            .is_some_and(|error| error.message.contains("already exists")),
        "the message names the clash"
    );
}

/// A rename of a path the tree does not have is stale input: no effect, no
/// error, nothing for the user to act on.
#[test]
fn renaming_a_missing_path_does_nothing() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::RenameRequested {
            path: RelPath::new("res/gone.png"),
            name: String::from("x.png"),
        },
    );
    assert!(effects.is_empty());
    assert_eq!(state.project.error, None);
}

/// Renaming to the name the entry already has is a no-op, not an error: the
/// inline editor is pre-filled, so Enter on it lands here.
#[test]
fn renaming_to_the_same_name_is_a_no_op() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::RenameRequested {
            path: RelPath::new("res/a.png"),
            name: String::from("a.png"),
        },
    );
    assert!(effects.is_empty());
    assert_eq!(state.project.error, None);
}

/// A case-only rename is legal: the entry being renamed is excluded from the
/// sister set, so it does not collide with itself.
#[test]
fn a_case_only_rename_asks_for_host_work() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::RenameRequested {
            path: RelPath::new("res/a.png"),
            name: String::from("A.PNG"),
        },
    );
    assert_eq!(
        fs_kind(&effects),
        Some(&FsOpKind::Rename {
            from: RelPath::new("res/a.png"),
            to: RelPath::new("res/A.PNG"),
        })
    );
}

/// A sibling that already uses the new name stops the rename before it starts.
#[test]
fn renaming_onto_a_sister_is_rejected() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::RenameRequested {
            path: RelPath::new("res/a.png"),
            name: String::from("models"),
        },
    );
    assert!(effects.is_empty());
    assert!(
        state
            .project
            .error
            .is_some_and(|error| error.message.contains("already exists"))
    );
}

/// Moving a folder into its own subtree would destroy it; the reducer refuses
/// before any host work exists.
#[test]
fn moving_a_folder_into_itself_is_rejected() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::MoveRequested {
            paths: vec![RelPath::new("res")],
            target: RelPath::new("res/models"),
        },
    );
    assert!(effects.is_empty());
    assert!(
        state
            .project
            .error
            .is_some_and(|error| error.message.contains("into itself"))
    );
}

/// Selecting a folder and something inside it moves only the folder: otherwise
/// the child would be moved twice.
#[test]
fn a_move_of_a_folder_and_its_child_moves_only_the_folder() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::MoveRequested {
            paths: vec![
                RelPath::new("res/a.png"),
                RelPath::new("res/models"),
                RelPath::new("res/models/a.png"),
            ],
            target: RelPath::new("Library"),
        },
    );
    assert_eq!(
        fs_kind(&effects),
        Some(&FsOpKind::Move {
            from: vec![RelPath::new("res/a.png"), RelPath::new("res/models")],
            to: RelPath::new("Library"),
        })
    );
}

/// Dropping a selection into the directory it already lives in changes nothing.
#[test]
fn moving_entries_to_their_own_parent_is_a_no_op() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::MoveRequested {
            paths: vec![RelPath::new("res/a.png")],
            target: RelPath::new("res"),
        },
    );
    assert!(effects.is_empty());
}

/// A delete asks for one operation over the top-level entries: a folder and a
/// child selected together would otherwise be sent to the host twice, and a path
/// the tree no longer has is not intent.
#[test]
fn a_delete_asks_for_host_work_over_the_top_level_entries() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::DeleteRequested {
            paths: vec![
                RelPath::new("res/models"),
                RelPath::new("res/models/a.png"),
                RelPath::new("res/a.png"),
                RelPath::new("res/gone.png"),
            ],
        },
    );
    assert_eq!(
        fs_kind(&effects),
        Some(&FsOpKind::Trash {
            paths: vec![RelPath::new("res/models"), RelPath::new("res/a.png")],
        })
    );
}

/// Deleting nothing is stale input: no host work and nothing to report.
#[test]
fn deleting_nothing_asks_for_no_host_work() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::DeleteRequested { paths: Vec::new() },
    );
    assert!(effects.is_empty());
    assert_eq!(state.project.error, None);
}

/// The root is the project itself, not a row: it is never sent to the trash.
#[test]
fn deleting_the_project_root_is_stale_input() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::DeleteRequested {
            paths: vec![RelPath::default()],
        },
    );
    assert!(effects.is_empty());
    assert_eq!(state.project.error, None);
}

/// A finished delete asks for a rescan but no reveal: the entry is gone, so
/// pointing the scan at its parent would select a folder the user did not ask
/// for.
#[test]
fn a_successful_delete_rescans_without_revealing_anything() {
    let mut state = scanned_state();
    state
        .project
        .view
        .selection
        .insert(RelPath::new("res/a.png"));
    state.project.view.active = Some(RelPath::new("res"));

    let effects = fs_completed(
        &mut state,
        Ok(FsOutcome::trashed(vec![RelPath::new("res/a.png")])),
    );

    assert!(asks_for_a_scan(&effects));
    assert_eq!(
        state.project.scan.focus_after_scan, None,
        "a deleted path cannot be revealed"
    );

    let generation = state.project.scan.generation;
    reduce(
        &mut state,
        completion(
            generation,
            vec![dir("res"), dir("res/models"), dir("Library")],
            false,
            Vec::new(),
        ),
    );

    assert!(
        state.project.view.selection.is_empty(),
        "the scan prunes the deleted path out of the selection"
    );
    assert_eq!(
        state.project.view.active,
        Some(RelPath::new("res")),
        "the content pane stays where the deleted file lived"
    );
}

/// Deleting the active directory falls the breadcrumb back to its nearest
/// surviving ancestor instead of resetting to the project root.
#[test]
fn deleting_the_active_directory_falls_back_to_its_parent() {
    let mut state = scanned_state();
    state
        .project
        .view
        .expanded
        .insert(RelPath::new("res/models"));
    state.project.view.active = Some(RelPath::new("res/models"));
    state
        .project
        .view
        .selection
        .insert(RelPath::new("res/models"));

    fs_completed(
        &mut state,
        Ok(FsOutcome::trashed(vec![RelPath::new("res/models")])),
    );
    let generation = state.project.scan.generation;
    reduce(
        &mut state,
        completion(
            generation,
            vec![dir("res"), file("res/a.png"), dir("Library")],
            false,
            Vec::new(),
        ),
    );

    assert_eq!(state.project.view.active, Some(RelPath::new("res")));
    assert!(state.project.view.selection.is_empty());
    assert!(state.project.view.expanded.is_empty());
}

/// A finished rename rewrites the view's prefixes before the rescan lands, so a
/// renamed folder stays expanded and its selection survives, and the scan is
/// asked to reveal the new path.
#[test]
fn a_successful_rename_rewrites_the_view_and_asks_for_a_scan() {
    let mut state = scanned_state();
    state
        .project
        .view
        .expanded
        .insert(RelPath::new("res/models"));
    state
        .project
        .view
        .selection
        .insert(RelPath::new("res/models/a.png"));
    state.project.view.active = Some(RelPath::new("res/models"));

    let effects = fs_completed(
        &mut state,
        Ok(FsOutcome::renamed(
            RelPath::new("res/models"),
            RelPath::new("res/renamed"),
        )),
    );

    assert!(asks_for_a_scan(&effects));
    assert_eq!(
        state.project.view.expanded,
        [RelPath::new("res/renamed")].into_iter().collect()
    );
    assert_eq!(
        state.project.view.selection,
        [RelPath::new("res/renamed/a.png")].into_iter().collect()
    );
    assert_eq!(state.project.view.active, Some(RelPath::new("res/renamed")));
    assert_eq!(
        state.project.scan.focus_after_scan,
        Some(RelPath::new("res/renamed")),
        "the completion asks the scan to reveal what it produced"
    );
}

/// A failed operation is visible and still resyncs the tree: a multi-entry move
/// can fail halfway.
#[test]
fn a_failed_operation_reports_and_rescans() {
    let mut state = scanned_state();
    let effects = fs_completed(&mut state, Err(String::from("access denied")));

    assert!(asks_for_a_scan(&effects));
    let error = state.project.error.expect("visible");
    assert!(!error.clears_on_scan);
    assert_eq!(error.message, "access denied");
}

/// The clean scan a failed operation requests must not wipe its message — the
/// user never gets to see a two-frame flash.
#[test]
fn a_host_error_survives_the_clean_scan_that_follows_it() {
    let mut state = scanned_state();
    fs_completed(&mut state, Err(String::from("access denied")));

    let generation = state.project.scan.generation;
    reduce(
        &mut state,
        completion(
            generation,
            vec![dir("res"), file("res/a.png"), dir("Library")],
            false,
            Vec::new(),
        ),
    );

    assert!(
        state
            .project
            .error
            .is_some_and(|error| !error.clears_on_scan),
        "a scan error is replaced by the next scan; an operation error is not"
    );
}

/// The next successful host operation clears the host error.
#[test]
fn a_successful_operation_clears_the_host_error() {
    let mut state = scanned_state();
    fs_completed(&mut state, Err(String::from("access denied")));
    fs_completed(
        &mut state,
        Ok(FsOutcome::created(RelPath::new("res/b.png"))),
    );
    assert_eq!(state.project.error, None);
}

/// Revealing a path the tree no longer has is stale input.
#[test]
fn revealing_a_missing_path_does_nothing() {
    let mut state = scanned_state();
    let effects = view_action(
        &mut state,
        ProjectAction::RevealRequested {
            path: RelPath::new("res/gone.png"),
        },
    );
    assert!(effects.is_empty());
}

/// The empty state carries the built-in type table and a fresh operation id.
#[test]
fn a_new_project_has_the_builtin_type_table() {
    let state = EditorState::new();
    assert_eq!(state.project.file_types, FileTypeRegistry::builtin());
    assert_eq!(state.project.next_fs_op, FsOpId(0));
}
