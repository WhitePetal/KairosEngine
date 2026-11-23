//! Shared-style slice: routing for styles the editor shell owns.
//!
//! The toolbar and the color picker are loaded once by the app, not by a panel
//! instance, so there is no state to mutate here either. The reducer only turns
//! the Preferences panel's intent into the host work that applies or persists
//! it; validation (does the path exist, does the value kind match) happens in
//! the registry where the value lands.

use crate::{EditorState, Effect, SharedStyleAction, SharedStyleEffect};

pub(super) fn reduce(_state: &mut EditorState, action: SharedStyleAction) -> Vec<Effect> {
    match action {
        SharedStyleAction::SetField { style, path, value } => {
            vec![Effect::SharedStyle(SharedStyleEffect::SetField {
                style,
                path,
                value,
            })]
        }
        SharedStyleAction::EditList { style, path, edit } => {
            vec![Effect::SharedStyle(SharedStyleEffect::EditList {
                style,
                path,
                edit,
            })]
        }
        SharedStyleAction::Reset { style } => {
            vec![Effect::SharedStyle(SharedStyleEffect::Reset { style })]
        }
        SharedStyleAction::Commit { style } => {
            vec![Effect::SharedStyle(SharedStyleEffect::Persist { style })]
        }
        SharedStyleAction::Reload { style } => {
            vec![Effect::SharedStyle(SharedStyleEffect::Reload { style })]
        }
    }
}
