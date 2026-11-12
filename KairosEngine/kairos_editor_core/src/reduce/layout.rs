//! Layout slice reducer: open/close/focus intent.
//!
//! Everything here is pure state. Making the dock and the panel host follow is
//! the app's job (`PanelHost::sync` + the dock tab reconcile); this reducer only
//! decides *what* the layout should be.

use crate::{EditorState, LayoutAction};

pub(super) fn reduce(state: &mut EditorState, action: LayoutAction) {
    match action {
        LayoutAction::TogglePanel { panel } => state.layout.toggle(panel),
        LayoutAction::ShowPanel { panel } => state.layout.open(panel),
        LayoutAction::ClosePanel { panel } => state.layout.close(panel),
        LayoutAction::FocusPanel { panel } => {
            // Focus is instance-keyed (`LayoutState::set_focus`): the app mirrors
            // the dock's focused instance into the state after every committed UI
            // pass, so the reducer only guarantees the panel is open.
            state.layout.open(panel);
        }
    }
}
