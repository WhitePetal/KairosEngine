//! Shared-style slice: routing for styles the editor shell owns.
//!
//! The toolbar and the color picker are loaded once by the app, not by a panel
//! instance, so there is no state to mutate here either. The reducer only turns
//! the Preferences panel's intent into the host work that applies or persists
//! it; the value is the page's typed copy, so there is no path to validate.

use crate::{EditorState, Effect, SharedStyleAction, SharedStyleEffect};

pub(super) fn reduce(_state: &mut EditorState, action: SharedStyleAction) -> Vec<Effect> {
    match action {
        SharedStyleAction::Replace { style, skin } => {
            vec![Effect::SharedStyle(SharedStyleEffect::Replace {
                style,
                skin,
            })]
        }
        SharedStyleAction::Reset { style } => {
            vec![Effect::SharedStyle(SharedStyleEffect::Reset { style })]
        }
        SharedStyleAction::Commit { style } => {
            vec![Effect::SharedStyle(SharedStyleEffect::Persist { style })]
        }
    }
}
