//! The editor shell around the dock: top bar, keyboard shortcuts and the modal
//! overlay. Shell widgets push Actions into `UiOut` exactly like panels do; they
//! never mutate state directly.

use editor_core_prototype::{Action, AppAction, CommandId, DocumentAction, LayoutAction, PanelId};
use egui::{Align, Layout};
use ui_widgets_prototype::{
    Badge, BadgeTone, Button, ButtonVariant, ConfirmDialog, Heading, IconId, Label, Separator,
    Toolbar, ToolbarItem,
};

use super::EditorApp;
use crate::gallery::GALLERY_PANEL;

impl EditorApp {
    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let (save, undo, redo) = ctx.input(|input| {
            (
                input.modifiers.command && input.key_pressed(egui::Key::S),
                input.modifiers.command
                    && !input.modifiers.shift
                    && input.key_pressed(egui::Key::Z),
                input.modifiers.command && input.modifiers.shift && input.key_pressed(egui::Key::Z),
            )
        });
        if save {
            self.ui_out
                .push(Action::Document(DocumentAction::SaveActive));
        }
        if undo {
            self.ui_out
                .push(Action::Document(DocumentAction::UndoActive));
        }
        if redo {
            self.ui_out
                .push(Action::Document(DocumentAction::RedoActive));
        }
        let toggle_gallery =
            ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::G));
        if toggle_gallery {
            self.ui_out.push(Action::Layout(LayoutAction::TogglePanel {
                panel: GALLERY_PANEL,
            }));
        }
    }

    pub(super) fn draw_top_bar(&mut self, ui: &mut egui::Ui) {
        let (dirty, saving) = self
            .core
            .state
            .active_document()
            .map(|doc| (doc.dirty, doc.is_saving()))
            .unwrap_or((false, false));
        let save_enabled = self
            .core
            .commands
            .action(CommandId::SAVE_DOCUMENT, &self.core.state)
            .is_some();
        let undo_enabled = self
            .core
            .commands
            .action(CommandId::UNDO, &self.core.state)
            .is_some();
        let redo_enabled = self
            .core
            .commands
            .action(CommandId::REDO, &self.core.state)
            .is_some();

        ui.horizontal(|ui| {
            ui.add(Heading::new("Kairos Editor"));
            ui.add(Label::new("panel registry + egui↔wgpu viewport").muted());
            ui.add(Separator);

            let mut toolbar = Toolbar::new(vec![
                ToolbarItem::button("save")
                    .icon(IconId::Save)
                    .label("Save")
                    .tooltip("Save active document (Cmd/Ctrl+S)")
                    .enabled(save_enabled),
                ToolbarItem::button("undo")
                    .icon(IconId::Revert)
                    .label("Undo")
                    .tooltip("Undo (Cmd/Ctrl+Z)")
                    .enabled(undo_enabled),
                ToolbarItem::button("redo")
                    .icon(IconId::Check)
                    .label("Redo")
                    .tooltip("Redo (Cmd/Ctrl+Shift+Z)")
                    .enabled(redo_enabled),
            ]);
            if let Some(id) = toolbar.show(ui).clicked {
                let command = match id {
                    "save" => Some(CommandId::SAVE_DOCUMENT),
                    "undo" => Some(CommandId::UNDO),
                    "redo" => Some(CommandId::REDO),
                    _ => None,
                };
                if let Some(action) =
                    command.and_then(|id| self.core.commands.action(id, &self.core.state))
                {
                    self.ui_out.push(action);
                }
            }

            ui.add(Separator);
            if ui
                .add(Button::new("Widget gallery").variant(ButtonVariant::Secondary))
                .clicked()
            {
                // The host knows the gallery's PanelId; the store/core does not
                // need a new enum variant for it.
                self.ui_out.push(Action::Layout(LayoutAction::TogglePanel {
                    panel: GALLERY_PANEL,
                }));
            }
            for (label, panel) in [
                ("Project panel", PanelId::PROJECT),
                ("Inspector panel", PanelId::INSPECTOR),
                ("Console panel", PanelId::CONSOLE),
                ("Viewport panel", PanelId::VIEWPORT),
            ] {
                if ui.add(Button::new(label)).clicked() {
                    self.ui_out
                        .push(Action::Layout(LayoutAction::TogglePanel { panel }));
                }
            }
            if ui
                .add(Button::new("Quit").variant(ButtonVariant::Danger))
                .clicked()
            {
                self.ui_out.push(Action::App(AppAction::QuitRequested));
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if saving {
                    ui.add(Badge::new("saving").tone(BadgeTone::Accent));
                }
                if dirty {
                    ui.add(Badge::new("unsaved").tone(BadgeTone::Error));
                }
                let document_count = format!("documents: {}", self.core.state.documents.len());
                ui.add(Label::new(&document_count));
                let frame_label = format!("frame {}", self.frame_count);
                ui.add(Label::new(&frame_label).muted());
            });
        });
    }

    pub(super) fn draw_modal(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.core.state.modal.confirm_dialog().cloned() else {
            return;
        };
        let mut confirm = false;
        let mut cancel = false;
        egui::Area::new(egui::Id::new("proto_modal"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(420.0);
                    let mut widget = ConfirmDialog::new(&dialog.title, &dialog.message)
                        .confirm_label("Confirm")
                        .cancel_label("Cancel")
                        .danger(true);
                    let output = widget.show(ui);
                    confirm = output.confirm;
                    cancel = output.cancel;
                });
            });
        if confirm {
            self.ui_out.push(dialog.confirm);
        }
        if cancel {
            self.ui_out.push(dialog.cancel);
        }
    }
}
