//! Style slice: routing for panel skins.
//!
//! Unlike the layout and app slices, there is no state to mutate here. A skin
//! belongs to the panel instance that draws from it (it is loaded in
//! `Panel::init` and persisted per component), so core cannot read or write
//! it; the reducer's job is to turn the panel's intent into the host work that
//! applies it. Validation — does the path exist, does the value match the field
//! kind — happens where the value lands, in `Panel::set_skin_field`, which
//! returns whether it wrote anything.
//!
//! The `SetField` / `Commit` split is the same save boundary the settings
//! editor used: a drag previews every frame, the file is written once when the
//! interaction ends. The legacy window pushed a whole page back every frame and
//! wrote TOML about 60 times a second.

use crate::{EditorState, Effect, StyleAction, StyleEffect};

pub(super) fn reduce(_state: &mut EditorState, action: StyleAction) -> Vec<Effect> {
    match action {
        StyleAction::SetField { panel, path, value } => {
            vec![Effect::Style(StyleEffect::SetField { panel, path, value })]
        }
        StyleAction::EditList { panel, path, edit } => {
            vec![Effect::Style(StyleEffect::EditList { panel, path, edit })]
        }
        StyleAction::Commit { panel } => vec![Effect::Style(StyleEffect::Persist { panel })],
    }
}
