//! Root reducer: routes the aggregate [`Action`] to per-domain reducers and owns
//! the cross-slice rules no single slice can decide alone.
//!
//! Reducers are pure functions over the state tree: they may read and mutate
//! anything in [`EditorState`], but they never do IO, never sleep and never touch
//! egui or the dock. The UI pass only collects actions; the app calls this after
//! the last pass (`!Context::will_discard()`) so a discarded pass cannot commit
//! half a frame twice.
//!
//! The prototype's reducer also returns `Vec<Effect>` (host work as data). We do
//! not need effects yet: the first one will be `PersistLayout`, which lands here
//! together with the layout file format.

mod layout;

use crate::{Action, EditorState};

pub fn reduce(state: &mut EditorState, action: Action) {
    match action {
        Action::Layout(action) => layout::reduce(state, action),
    }
}
