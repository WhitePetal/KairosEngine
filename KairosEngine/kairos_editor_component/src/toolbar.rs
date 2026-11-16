use egui::{Panel, RichText, Ui, menu};
use kairos_editor_widgets::{color::Color32Ext, style::{Style, StyleFile}, ui_image};

use crate::{Frame, UiOut, toolbar::style::ToolBarStyle};

pub mod style;


pub struct ToolBar {
    style: ToolBarStyle,
}


impl ToolBar {
    pub fn new(ctx: &egui::Context) -> Self {
        let style = match StyleFile::<ToolBarStyle>::load_or_default() {
            Ok(file) => file.dark,
            Err(error) => {
                kairos_log::warn!(target: "kairos_editor", "toolbar style: {error}");
                ToolBarStyle::dark()
            }
        };

        for path in style.icons.all() {
            ui_image::preload(ctx, path);
        }

        Self {
            style
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, _frame: &Frame, _out: &mut UiOut) {
        Panel::top("toolbar")
            .default_size(self.style.height)
            .show(ui, |ui| {
                menu::MenuBar::new().ui(ui, |ui| {
                    ui.set_height(self.style.height);
                    ui.painter().rect_filled(ui.available_rect_before_wrap(), self.style.corner_radius, self.style.background_color.to_egui());

                    let menu_content = |ui: &mut egui::Ui| {
                        if ui.button("About Kairos").clicked() {
                            todo!("需要实现 UiOut 发送 About 窗口创建逻辑")
                        }
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            todo!("需要实现 UiOut 发送 Editor程序退出逻辑")
                        }
                    };

                    ui.menu_image_button(ui_image::get_image(&self.style.icons.about), menu_content);

                    ui.menu_button(RichText::new("Edit").color(self.style.text_color.to_egui()), |ui| {
                        if ui.button("Preferences").clicked() {
                            todo!("需要实现 UiOut 发送 创建Preferences窗口逻辑")
                        }
                    });

                    ui.menu_button(RichText::new("World").color(self.style.text_color.to_egui()), |ui| {
                        if ui.button("New World").clicked() {
                            todo!("需要实现 UiOut 发送 创建新世界/场景逻辑")
                        }
                    });

                    ui.menu_button(RichText::new("Window").color(self.style.text_color.to_egui()), |ui| {
                        if ui.button("Inspector").clicked() {
                            todo!("需要实现 UiOut 发送 创建Inspector窗口逻辑")
                        }
                        if ui.button("Hierarchy").clicked() {
                            todo!("需要实现 UiOut 发送 创建Hierarchy窗口逻辑")
                        }
                        if ui.button("Project").clicked() {
                            todo!("需要实现 UiOut 发送 创建Project窗口逻辑")
                        }
                        if ui.button("Console").clicked() {
                            todo!("需要实现 UiOut 发送 创建 Console窗口逻辑")
                        }
                        if ui.button("Scene").clicked() {
                            todo!("需要实现 UiOut 发送 创建Scene窗口逻辑")
                        }
                        if ui.button("Game").clicked() {
                            todo!("需要实现 UiOut 发送 创建 Game窗口逻辑")
                        }
                    })
                });
            });
    }
}
