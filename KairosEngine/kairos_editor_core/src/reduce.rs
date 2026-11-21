//! Root reducer: routes the aggregate [`Action`] to per-domain reducers and owns
//! the cross-slice rules no single slice can decide alone.
//!
//! Reducers are pure functions over the state tree: they may read and mutate
//! anything in [`EditorState`], but they never do IO, never sleep and never touch
//! egui or the dock. The UI pass only collects actions; the app calls this after
//! the last pass (`!Context::will_discard()`) so a discarded pass cannot commit
//! half a frame twice.
//!
//! Like the prototype, a reducer returns `Vec<Effect>`: host work as data. The
//! first effects are panel placement/focus (the dock tree lives in the app, not
//! in core), skin edits (a panel's style lives in the panel, not in the state
//! tree) and `PersistLayout`, which will join them when the layout file format
//! lands.

mod app;
mod layout;
mod project;
mod shared_style;
mod style;

use crate::{Action, EditorState, Effect};

pub fn reduce(state: &mut EditorState, action: Action) -> Vec<Effect> {
    match action {
        Action::Layout(action) => layout::reduce(state, action),
        Action::App(action) => app::reduce(state, action),
        Action::Project(action) => project::reduce(state, action),
        Action::Style(action) => style::reduce(state, action),
        Action::SharedStyle(action) => shared_style::reduce(state, action),
    }
}
