//! App slice: quitting is an intent, and the reducer answers it with host work.

use kairos_editor_core::{
    Action, AppAction, AppEffect, EditorState, Effect, LayoutAction, PanelId, ProjectState, reduce,
};

const SCENE: PanelId = PanelId("scene");
const CONSOLE: PanelId = PanelId("console");

#[test]
fn quit_is_answered_with_the_host_close_effect() {
    let mut state = EditorState::new();

    assert_eq!(
        reduce(&mut state, Action::App(AppAction::QuitRequested)),
        vec![Effect::App(AppEffect::Quit)]
    );
}

/// Quit is not a state transition: today it changes nothing in the state tree,
/// so the future "unsaved work?" arm can be added without migrating state.
#[test]
fn quitting_leaves_the_layout_alone() {
    let mut state = EditorState::new();
    reduce(
        &mut state,
        Action::Layout(LayoutAction::ShowPanel { panel: SCENE }),
    );

    reduce(&mut state, Action::App(AppAction::QuitRequested));

    assert!(state.layout.is_open(SCENE));
    assert!(!state.layout.is_open(CONSOLE));
    assert_eq!(
        state.project,
        ProjectState::default(),
        "quitting must not touch the project slice"
    );
}
