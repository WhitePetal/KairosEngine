//! Shared-style slice: routing only, exactly like the panel-skin slice.

use kairos_editor_core::{
    Action, EditorState, Effect, ListEdit, SharedStyleAction, SharedStyleEffect, SharedStyleId,
    StyleValue, reduce,
};

const COLOR_PICKER: SharedStyleId = SharedStyleId("color_picker");

#[test]
fn a_shared_field_edit_becomes_a_registry_write() {
    let mut state = EditorState::new();
    let action = Action::SharedStyle(SharedStyleAction::SetField {
        style: COLOR_PICKER,
        path: "accent".to_owned(),
        value: StyleValue::Color(kairos_color::Color::from_srgb8([1, 2, 3, 255])),
    });

    assert_eq!(
        reduce(&mut state, action),
        vec![Effect::SharedStyle(SharedStyleEffect::SetField {
            style: COLOR_PICKER,
            path: "accent".to_owned(),
            value: StyleValue::Color(kairos_color::Color::from_srgb8([1, 2, 3, 255])),
        })]
    );
}

/// A shared list edit routes to the registry's list write, like a shared field
/// edit routes to its field write.
#[test]
fn a_shared_list_edit_becomes_a_registry_list_write() {
    let mut state = EditorState::new();
    let edit = ListEdit::Insert { index: 0 };

    assert_eq!(
        reduce(
            &mut state,
            Action::SharedStyle(SharedStyleAction::EditList {
                style: COLOR_PICKER,
                path: "proportional".to_owned(),
                edit,
            })
        ),
        vec![Effect::SharedStyle(SharedStyleEffect::EditList {
            style: COLOR_PICKER,
            path: "proportional".to_owned(),
            edit,
        })]
    );
}

#[test]
fn a_shared_commit_becomes_a_persist() {
    let mut state = EditorState::new();

    assert_eq!(
        reduce(
            &mut state,
            Action::SharedStyle(SharedStyleAction::Commit {
                style: COLOR_PICKER
            })
        ),
        vec![Effect::SharedStyle(SharedStyleEffect::Persist {
            style: COLOR_PICKER
        })]
    );
}

#[test]
fn a_shared_reload_becomes_a_reload() {
    let mut state = EditorState::new();

    assert_eq!(
        reduce(
            &mut state,
            Action::SharedStyle(SharedStyleAction::Reload {
                style: COLOR_PICKER
            })
        ),
        vec![Effect::SharedStyle(SharedStyleEffect::Reload {
            style: COLOR_PICKER
        })]
    );
}

/// Shared styles live in the app's registry, not in the state tree.
#[test]
fn shared_style_edits_leave_the_state_tree_alone() {
    let mut state = EditorState::new();
    reduce(
        &mut state,
        Action::SharedStyle(SharedStyleAction::SetField {
            style: COLOR_PICKER,
            path: "accent".to_owned(),
            value: StyleValue::Float(1.0),
        }),
    );

    assert_eq!(state.project_name, "demo-project");
}
