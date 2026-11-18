//! Style slice: routing only — the value lands in the panel, not in state.

use kairos_editor_core::{
    Action, EditorState, Effect, PanelId, StyleAction, StyleEffect, StyleValue, reduce,
};

const CONSOLE: PanelId = PanelId("console");

#[test]
fn a_field_edit_becomes_the_panel_write_it_needs() {
    let mut state = EditorState::new();
    let action = Action::Style(StyleAction::SetField {
        panel: CONSOLE,
        path: "toolbar_height".to_owned(),
        value: StyleValue::Float(30.0),
    });

    assert_eq!(
        reduce(&mut state, action),
        vec![Effect::Style(StyleEffect::SetField {
            panel: CONSOLE,
            path: "toolbar_height".to_owned(),
            value: StyleValue::Float(30.0),
        })]
    );
}

#[test]
fn a_commit_becomes_a_persist() {
    let mut state = EditorState::new();

    assert_eq!(
        reduce(
            &mut state,
            Action::Style(StyleAction::Commit { panel: CONSOLE })
        ),
        vec![Effect::Style(StyleEffect::Persist { panel: CONSOLE })]
    );
}

/// A skin belongs to the panel instance, not to the state tree: editing one
/// must not change EditorState.
#[test]
fn skin_edits_leave_the_state_tree_alone() {
    let mut state = EditorState::new();
    reduce(
        &mut state,
        Action::Style(StyleAction::SetField {
            panel: CONSOLE,
            path: "toolbar_height".to_owned(),
            value: StyleValue::Float(30.0),
        }),
    );

    assert_eq!(state.project_name, "demo-project");
    assert!(!state.layout.is_open(CONSOLE));
}
