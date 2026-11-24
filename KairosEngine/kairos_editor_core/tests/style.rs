//! Style slice: routing only — the value lands in the panel, not in state.

use kairos_editor_core::style::{Skin, StyleBlob};
use kairos_editor_core::{
    Action, EditorState, Effect, PanelId, ProjectState, StyleAction, StyleEffect, reduce,
};

const CONSOLE: PanelId = PanelId("console");

/// Reset is its own effect, not a pile of field writes: the host knows how
/// to restore its own type's defaults, and the click also persists them.
#[test]
fn a_reset_becomes_a_reset() {
    let mut state = EditorState::new();

    assert_eq!(
        reduce(
            &mut state,
            Action::Style(StyleAction::Reset { panel: CONSOLE })
        ),
        vec![Effect::Style(StyleEffect::Reset { panel: CONSOLE })]
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

/// A whole value crosses the boundary in one piece: the page drew its own copy,
/// so the reducer only routes the box to the owner.
#[derive(Clone, Debug, PartialEq)]
struct TestStyle {
    height: f32,
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
fn a_whole_skin_is_routed_to_its_owner() {
    let mut state = EditorState::new();
    let skin = Skin::new(TestStyle { height: 30.0 });

    assert_eq!(
        reduce(
            &mut state,
            Action::Style(StyleAction::Replace {
                panel: CONSOLE,
                skin: skin.clone(),
            })
        ),
        vec![Effect::Style(StyleEffect::Replace {
            panel: CONSOLE,
            skin,
        })],
        "the box reaches the owner unchanged; equality compares the styles"
    );
}

/// A skin belongs to the panel instance, not to the state tree: replacing one
/// must not change EditorState.
#[test]
fn skin_edits_leave_the_state_tree_alone() {
    let mut state = EditorState::new();
    reduce(
        &mut state,
        Action::Style(StyleAction::Replace {
            panel: CONSOLE,
            skin: Skin::new(TestStyle { height: 30.0 }),
        }),
    );

    assert_eq!(state.project, ProjectState::default());
    assert!(!state.layout.is_open(CONSOLE));
}
