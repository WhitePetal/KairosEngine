use std::{borrow::Cow, cell::Cell};

use crate::kairos_editor::ui::{Message, Messager};

pub enum DialogState {
    Opening,
    Closed,
}

pub trait Dialog {
    fn draw(&self, ui: &egui::Ui, messager: &mut Messager) -> DialogState;
}

struct ConfirmDialogWindowModel<'a, T, V>
where
    T: FnOnce() -> (),
    V: FnOnce() -> (),
{
    title: Cow<'a, str>,
    content: Cow<'a, str>,
    confirm_name: Cow<'a, str>,
    cancel_name: Cow<'a, str>,
    on_confirm_message: Cell<Option<Message>>,
    on_cancel_message: Cell<Option<Message>>,
    on_confirm: Cell<Option<T>>,
    on_cancel: Cell<Option<V>>,
}

pub struct ConfirmDialogWindow<'a, T, V>
where
    T: FnOnce() -> (),
    V: FnOnce() -> (),
{
    model: ConfirmDialogWindowModel<'a, T, V>,
}

impl<'a, T, V> Dialog for ConfirmDialogWindow<'a, T, V>
where
    T: FnOnce() -> (),
    V: FnOnce() -> (),
{
    fn draw(&self, ui: &egui::Ui, messager: &mut Messager) -> DialogState {
        egui::Modal::new(ui.id().with(&self.model.title))
            .show(ui.ctx(), |ui| {
                ui.label(self.model.content.as_ref());
                ui.horizontal(|ui| {
                    if ui.button(self.model.confirm_name.as_ref()).clicked() {
                        if let Some(on_confirm) = self.model.on_confirm.take() {
                            on_confirm();
                            if let Some(msg) = self.model.on_confirm_message.take() {
                                messager.send(msg);
                            }
                        }
                        return DialogState::Closed;
                    }
                    if ui.button(self.model.cancel_name.as_ref()).clicked() {
                        if let Some(on_cancle) = self.model.on_cancel.take() {
                            on_cancle();
                            if let Some(msg) = self.model.on_cancel_message.take() {
                                messager.send(msg);
                            }
                        }
                        return DialogState::Closed;
                    }
                    DialogState::Opening
                })
                .inner
            })
            .inner
    }
}

impl<'a, T, V> ConfirmDialogWindow<'a, T, V>
where
    T: FnOnce() -> (),
    V: FnOnce() -> (),
{
    pub fn new(
        title: Cow<'a, str>,
        content: Cow<'a, str>,
        confirm_name: Cow<'a, str>,
        cancel_name: Cow<'a, str>,
        on_confirm_message: Option<Message>,
        on_cancel_message: Option<Message>,
        on_confirm: Option<T>,
        on_cacel: Option<V>,
    ) -> Self {
        Self {
            model: ConfirmDialogWindowModel {
                title,
                content,
                confirm_name,
                cancel_name,
                on_confirm_message: Cell::new(on_confirm_message),
                on_cancel_message: Cell::new(on_cancel_message),
                on_confirm: Cell::new(on_confirm),
                on_cancel: Cell::new(on_cacel),
            },
        }
    }
}
