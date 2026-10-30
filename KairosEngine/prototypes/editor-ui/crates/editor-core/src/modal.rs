//! Modal state as data. A modal carries the `Action`s it will dispatch, so
//! the host view only draws buttons and sends `ModalAction::Confirm/Cancel`.

use crate::action::Action;

#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    pub confirm: Action,
    pub cancel: Action,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Modal {
    Confirm(ConfirmDialog),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModalState {
    pub active: Option<Modal>,
}

impl ModalState {
    pub fn is_open(&self) -> bool {
        self.active.is_some()
    }

    pub fn confirm_dialog(&self) -> Option<&ConfirmDialog> {
        match &self.active {
            Some(Modal::Confirm(dialog)) => Some(dialog),
            None => None,
        }
    }
}
