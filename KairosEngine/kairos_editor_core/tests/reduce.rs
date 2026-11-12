//! Layout slice: actions are the only way the layout changes.

use kairos_editor_core::{Action, EditorState, LayoutAction, PanelId, PanelInstanceId, reduce};

const SCENE: PanelId = PanelId("scene");
const CONSOLE: PanelId = PanelId("console");

fn show(panel: PanelId) -> Action {
    Action::Layout(LayoutAction::ShowPanel { panel })
}

fn close(panel: PanelId) -> Action {
    Action::Layout(LayoutAction::ClosePanel { panel })
}

fn toggle(panel: PanelId) -> Action {
    Action::Layout(LayoutAction::TogglePanel { panel })
}

#[test]
fn show_and_close_are_idempotent() {
    let mut state = EditorState::new();

    reduce(&mut state, show(SCENE));
    reduce(&mut state, show(SCENE));
    assert!(state.layout.is_open(SCENE));

    reduce(&mut state, close(SCENE));
    reduce(&mut state, close(SCENE));
    assert!(!state.layout.is_open(SCENE));
}

#[test]
fn toggle_flips_the_open_flag() {
    let mut state = EditorState::new();

    reduce(&mut state, toggle(CONSOLE));
    assert!(state.layout.is_open(CONSOLE));

    reduce(&mut state, toggle(CONSOLE));
    assert!(!state.layout.is_open(CONSOLE));
}

#[test]
fn focus_ensures_the_panel_is_open() {
    let mut state = EditorState::new();

    reduce(
        &mut state,
        Action::Layout(LayoutAction::FocusPanel { panel: SCENE }),
    );

    assert!(state.layout.is_open(SCENE));
}

#[test]
fn closing_the_focused_panel_clears_focus() {
    let mut state = EditorState::new();
    reduce(&mut state, show(SCENE));
    state.layout.set_focus(Some(PanelInstanceId::new(SCENE, 3)));

    reduce(&mut state, close(SCENE));

    assert_eq!(state.layout.focus(), None);
}
