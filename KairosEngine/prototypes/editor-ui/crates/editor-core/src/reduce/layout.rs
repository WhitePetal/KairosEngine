//! Layout reducers; layout persistence is an effect, the data stays here.

use crate::action::LayoutAction;
use crate::effect::Effect;
use crate::state::EditorState;

pub fn reduce(state: &mut EditorState, action: LayoutAction) -> Vec<Effect> {
    match action {
        LayoutAction::TogglePanel { panel } => {
            if state.layout.is_open(panel) {
                state.layout.close(panel);
                Vec::new()
            } else {
                state.layout.open(panel);
                state.layout.focus = panel;
                vec![Effect::FocusPanel { panel }]
            }
        }
        LayoutAction::ShowPanel { panel } => {
            state.layout.open(panel);
            state.layout.focus = panel;
            Vec::new()
        }
        LayoutAction::FocusPanel { panel } => {
            state.layout.open(panel);
            state.layout.focus = panel;
            vec![Effect::FocusPanel { panel }]
        }
        LayoutAction::PersistLayout => vec![Effect::PersistLayout {
            layout: state.layout.clone(),
        }],
    }
}
