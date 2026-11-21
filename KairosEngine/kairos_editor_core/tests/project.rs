//! Project slice: the request/completion protocol around scanning.
//!
//! M0 has no scanner yet, so these tests pin what the app and the reducer
//! already agree on: a request gets a generation and a host effect, and a
//! completion is accepted only for the current, still-running generation.

use std::path::{Path, PathBuf};

use kairos_editor_core::project::{RelPath, ScanReason, ScanState};
use kairos_editor_core::{
    Action, EditorState, Effect, ProjectAction, ProjectEffect, ProjectState, reduce,
};

fn request(state: &mut EditorState) -> Vec<Effect> {
    reduce(
        state,
        Action::Project(ProjectAction::ScanRequested {
            reason: ScanReason::Startup,
            focus: None,
        }),
    )
}

fn scanned(generation: u64, truncated: bool, elapsed_ms: u64) -> Action {
    Action::Project(ProjectAction::Scanned {
        generation,
        entries: Vec::new(),
        truncated,
        elapsed_ms,
    })
}

#[test]
fn the_state_tree_starts_with_an_empty_project() {
    let state = EditorState::new();

    assert_eq!(state.project, ProjectState::default());
    assert_eq!(state.project.name, "");
    assert!(state.project.root.as_os_str().is_empty());
    assert!(state.project.view.expanded.is_empty());
    assert_eq!(state.project.view.active, None);
    assert!(state.project.view.selection.is_empty());
    assert!(!state.project.view.show_hidden);
    assert_eq!(state.project.view.tree_split, 0.28);
    assert_eq!(state.project.scan, ScanState::default());
    assert_eq!(state.project.tree.generation(), 0);
    assert_eq!(state.project.error, None);
}

/// The root is data supplied by the app's IO boundary (D9); the last component
/// names the project for the panel title.
#[test]
fn a_project_root_supplies_the_name_for_the_panel_title() {
    let project = ProjectState::at(Path::new("D:/Projects/MyGame"));

    assert_eq!(project.name, "MyGame");
    assert_eq!(project.root, PathBuf::from("D:/Projects/MyGame"));
    assert_eq!(project.tree.generation(), 0);
    assert_eq!(
        project.view,
        kairos_editor_core::project::ProjectView::default()
    );
}

/// A root with no final component (a failed `current_dir` at boot) still names
/// the project, so the panel title never renders empty.
#[test]
fn an_empty_root_still_gets_a_name() {
    assert_eq!(ProjectState::at(PathBuf::new()).name, "project");
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
    request(&mut state);
    assert_eq!(state.project.scan.generation, 2);

    let effects = reduce(&mut state, scanned(1, false, 7));

    assert!(effects.is_empty(), "a completion never asks for host work");
    assert!(
        state.project.scan.running,
        "the newer scan is still in flight"
    );
    assert_eq!(state.project.scan.last_ms, 0);
    assert_eq!(state.project.tree.generation(), 0);
}

/// A completion for an idle scan (the zero generation, or a duplicate delivery
/// of the same completion) says nothing about the current tree.
#[test]
fn a_completion_for_an_idle_scan_is_discarded() {
    let mut state = EditorState::new();

    reduce(&mut state, scanned(0, false, 7));

    assert!(!state.project.scan.running);
    assert_eq!(state.project.scan.last_ms, 0);
    assert_eq!(state.project.tree.generation(), 0);

    request(&mut state);
    reduce(&mut state, scanned(1, false, 7));
    let tree_generation = state.project.tree.generation();

    reduce(&mut state, scanned(1, false, 99));

    assert_eq!(
        state.project.tree.generation(),
        tree_generation,
        "a second completion for the same generation is ignored"
    );
    assert_eq!(state.project.scan.last_ms, 7);
}

#[test]
fn the_current_completion_stops_the_scan_and_records_its_metrics() {
    let mut state = EditorState::new();
    request(&mut state);

    let effects = reduce(&mut state, scanned(1, true, 42));

    assert!(effects.is_empty());
    assert!(!state.project.scan.running);
    assert_eq!(state.project.scan.last_ms, 42);
    assert!(state.project.scan.truncated);
    assert_eq!(
        state.project.tree.generation(),
        1,
        "assembling the result moves the tree's generation"
    );
}
