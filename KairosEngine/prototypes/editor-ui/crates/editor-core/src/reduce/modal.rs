//! Modal reducers: modals store plain `Action`s; Confirm/Cancel simply dispatch
//! the stored action through the root reducer, so modal flows reuse all normal
//! reduction rules (including nested cross-slice rules).

use crate::action::ModalAction;
use crate::effect::Effect;
use crate::modal::Modal;
use crate::state::EditorState;

pub fn reduce(state: &mut EditorState, action: ModalAction) -> Vec<Effect> {
    match action {
        ModalAction::ShowConfirm(dialog) => {
            state.modal.active = Some(Modal::Confirm(*dialog));
            Vec::new()
        }
        ModalAction::Dismiss => {
            state.modal.active = None;
            Vec::new()
        }
        ModalAction::Confirm => {
            let Some(Modal::Confirm(dialog)) = state.modal.active.take() else {
                return Vec::new();
            };
            super::reduce(state, dialog.confirm)
        }
        ModalAction::Cancel => {
            let Some(Modal::Confirm(dialog)) = state.modal.active.take() else {
                return Vec::new();
            };
            super::reduce(state, dialog.cancel)
        }
    }
}
