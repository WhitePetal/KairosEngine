//! Style slice: routing for panel skins.
//!
//! Unlike the layout and app slices, there is no state to mutate here. A skin
//! belongs to the panel instance that draws from it (it is loaded in
//! `Panel::init` and persisted per component), so core cannot read or write
//! it; the reducer's job is to turn the panel's intent into the host work that
//! applies it.
//!
//! The `Replace` / `Commit` split is the same save boundary the settings
//! editor used: the page hands the value over as it changes (live preview), the
//! file is written once when the interaction ends.

use crate::{EditorState, Effect, StyleAction, StyleEffect};

pub(super) fn reduce(_state: &mut EditorState, action: StyleAction) -> Vec<Effect> {
    match action {
        StyleAction::Replace { panel, skin } => {
            vec![Effect::Style(StyleEffect::Replace { panel, skin })]
        }
        StyleAction::Reset { panel } => vec![Effect::Style(StyleEffect::Reset { panel })],
        StyleAction::Commit { panel } => vec![Effect::Style(StyleEffect::Persist { panel })],
    }
}
