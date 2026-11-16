//! Layout slice reducer: open/close/focus intent.
//!
//! Everything here is pure state. Making the dock and the panel host follow is
//! the app's job (`PanelHost::sync` + the dock tab reconcile); this reducer only
//! decides *what* the layout should be.

use crate::{EditorState, Effect, LayoutAction, LayoutEffect};

pub(super) fn reduce(state: &mut EditorState, action: LayoutAction) -> Vec<Effect> {
    match action {
        // Opening always asks the app for a placement; the app never invents a
        // tab for an open panel. Toggle keeps its historical "open docked"
        // behavior by using the Focus effect.
        LayoutAction::TogglePanel { panel } => {
            let opening = !state.layout.is_open(panel);
            state.layout.toggle(panel);
            if opening {
                vec![Effect::Layout(LayoutEffect::FocusPanel { panel })]
            } else {
                Vec::new()
            }
        }
        LayoutAction::ShowPanel { panel } => {
            state.layout.open(panel);
            vec![Effect::Layout(LayoutEffect::ShowPanel { panel })]
        }
        LayoutAction::ClosePanel { panel } => {
            state.layout.close(panel);
            Vec::new()
        }
        LayoutAction::FocusPanel { panel } => {
            // Focus is instance-keyed (`LayoutState::set_focus`): the app mirrors
            // the dock's focused instance into the state after every committed UI
            // pass. The effect carries the kind to the app, which resolves it to
            // the live instance and focuses its dock tab.
            state.layout.open(panel);
            vec![Effect::Layout(LayoutEffect::FocusPanel { panel })]
        }
    }
}
