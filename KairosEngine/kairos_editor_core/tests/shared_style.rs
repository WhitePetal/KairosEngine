//! Shared-style slice: routing only, exactly like the panel-skin slice.

use kairos_editor_core::style::{Skin, StyleBlob};
use kairos_editor_core::{
    Action, EditorState, Effect, ProjectState, SharedStyleAction, SharedStyleEffect, SharedStyleId,
    reduce,
};

const COLOR_PICKER: SharedStyleId = SharedStyleId("color_picker");

#[derive(Clone, Debug, PartialEq)]
struct TestStyle {
    accent: f32,
}

impl StyleBlob for TestStyle {
    fn clone_blob(&self) -> Box<dyn StyleBlob> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn equals(&self, other: &dyn StyleBlob) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }

    fn debug_blob(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TestStyle")
    }
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
fn a_shared_reset_becomes_a_reset() {
    let mut state = EditorState::new();

    assert_eq!(
        reduce(
            &mut state,
            Action::SharedStyle(SharedStyleAction::Reset {
                style: COLOR_PICKER
            })
        ),
        vec![Effect::SharedStyle(SharedStyleEffect::Reset {
            style: COLOR_PICKER
        })]
    );
}

#[test]
fn a_whole_shared_skin_is_routed_to_the_registry() {
    let mut state = EditorState::new();
    let skin = Skin::new(TestStyle { accent: 1.0 });

    assert_eq!(
        reduce(
            &mut state,
            Action::SharedStyle(SharedStyleAction::Replace {
                style: COLOR_PICKER,
                skin: skin.clone(),
            })
        ),
        vec![Effect::SharedStyle(SharedStyleEffect::Replace {
            style: COLOR_PICKER,
            skin,
        })]
    );
}

/// Shared styles live in the app's registry, not in the state tree.
#[test]
fn shared_style_edits_leave_the_state_tree_alone() {
    let mut state = EditorState::new();
    reduce(
        &mut state,
        Action::SharedStyle(SharedStyleAction::Replace {
            style: COLOR_PICKER,
            skin: Skin::new(TestStyle { accent: 1.0 }),
        }),
    );

    assert_eq!(state.project, ProjectState::default());
}
