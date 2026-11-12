//! Layout reducers; layout persistence is an effect, the data stays here.
//!
//! Cross-slice rule owned here: closing the viewport *panel* also destroys the
//! viewport's render target. Any other slice closing a panel would have to know
//! about viewports, which is exactly the leakage this architecture removes.

use crate::action::LayoutAction;
use crate::effect::Effect;
use crate::state::{EditorState, PanelId};
use crate::viewport::ViewportId;

pub fn reduce(state: &mut EditorState, action: LayoutAction) -> Vec<Effect> {
    match action {
        LayoutAction::TogglePanel { panel } => {
            if state.layout.is_open(panel) {
                close_panel(state, panel)
            } else {
                open_panel(state, panel, true)
            }
        }
        LayoutAction::ShowPanel { panel } => open_panel(state, panel, false),
        LayoutAction::ClosePanel { panel } => close_panel(state, panel),
        LayoutAction::FocusPanel { panel } => open_panel(state, panel, true),
        LayoutAction::PersistLayout => vec![Effect::PersistLayout {
            layout: state.layout.clone(),
        }],
    }
}

fn open_panel(state: &mut EditorState, panel: PanelId, emit_focus_effect: bool) -> Vec<Effect> {
    state.layout.open(panel);
    state.layout.focus = panel;
    if emit_focus_effect {
        vec![Effect::FocusPanel { panel }]
    } else {
        Vec::new()
    }
}

fn close_panel(state: &mut EditorState, panel: PanelId) -> Vec<Effect> {
    if !state.layout.is_open(panel) {
        return Vec::new();
    }
    state.layout.close(panel);
    // The scene viewport's target is owned by the host; closing the panel must
    // free it instead of leaving a stale texture alive.
    if panel == PanelId::VIEWPORT && state.viewports.remove(ViewportId::SCENE).is_some() {
        return vec![Effect::DestroyViewport {
            viewport: ViewportId::SCENE,
        }];
    }
    Vec::new()
}
