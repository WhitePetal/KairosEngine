//! App slice reducer: the editor's own lifecycle.
//!
//! There is no app *state* yet: quitting changes nothing the state tree
//! records, it produces host work. That is exactly why the reducer exists
//! rather than the view raising the effect directly — the decision ("is it
//! safe to quit?") belongs to the layer that can inspect every document.

use crate::{AppAction, AppEffect, EditorState, Effect};

pub(super) fn reduce(_state: &mut EditorState, action: AppAction) -> Vec<Effect> {
    match action {
        // Nothing to lose yet, so the request is answered immediately. Once
        // documents exist this arm checks their dirty flags and answers with a
        // confirmation modal instead; `Effect::App(Quit)` stays the "yes".
        AppAction::QuitRequested => vec![Effect::App(AppEffect::Quit)],
    }
}
