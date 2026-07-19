use std::{
    any::type_name, fs::{self}, path::PathBuf,
};

use parking_lot::Mutex;

use crate::{
    kairos_editor::{
        Engine,
        ui::{Messager, global_styles::GlobalStyles},
    },
    kairos_game::KairosGame,
    log::Log,
    math,
};
use egui::{self, Panel, containers::menu};
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::{
    kairos_dialog,
    kairos_editor::ui::{
        Drawer, Message, paths,
        ui_style_fields::{ColorStyleField, FloatFieldEditViewType, FloatStyleField, StyleField},
    },
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolBarStyle {
    pub height: f32,
    pub button_width: f32,
    pub corner_radius: f32,
    pub icon_size: u32,
    pub fill_color: math::Color32,
    pub button_text_color: math::Color32,
    pub about_icon_path: String,
}

pub struct ToolBarModel {
    style: ToolBarStyle,
}

impl ToolBarStyle {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json =
            fs::read_to_string(paths::PATH_EDITOR_WINDO_TOOL_BAR_STYLE).map_err(|error| {
                format!(
                    "Load MainWindow TitleBar Json Failed, path: {}, error: {}",
                    paths::PATH_EDITOR_WINDO_TOOL_BAR_STYLE,
                    error
                )
            })?;
        let style = from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize MainWindow TitleBar Json Failed, error: {}",
                error
            )
        })?;

        Ok(style)
    }
}

impl ToolBarModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = ToolBarStyle::new()?;

        Ok(Self { style })
    }
}

/// Pre-loaded icon data, scaled to toolbar height.
struct IconData {
    /// RGBA pixel data, already resized.
    rgba: Vec<u8>,
    /// Icon display size in pixels.
    width: usize,
    height: usize,
}

pub struct ToolBar {
    model: ToolBarModel,
    /// Pre-loaded RGBA icon data. None if the icon couldn't be loaded.
    icon_data: Option<IconData>,
    /// Cached egui texture handle for the icon (set on first frame).
    icon_texture: Mutex<Option<egui::TextureHandle>>,
}

impl ToolBar {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = ToolBarModel::new()?;

        // Load the icon once at startup — scale to toolbar height (~24 px).
        let icon_data = match std::fs::read(PathBuf::from(&model.style.about_icon_path)) {
            Ok(bytes) => match image::load_from_memory(&bytes) {
                Ok(img) => {
                    let (orig_w, orig_h) = (img.width(), img.height());
                    let icon_size = model.style.icon_size;
                    let (w, h) = if orig_h > icon_size {
                        (
                            icon_size,
                            icon_size,
                        )
                    } else {
                        (orig_w, orig_h)
                    };
                    let resized = img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
                    let rgba = resized.into_rgba8();
                    Some(IconData {
                        rgba: rgba.into_vec(),
                        width: w as usize,
                        height: h as usize,
                    })
                }
                Err(e) => {
                    log::warn!("Failed to decode engine icon: {e}");
                    None
                }
            },
            Err(e) => {
                log::warn!("Failed to read engine icon: {e}");
                None
            }
        };

        Ok(Self {
            model,
            icon_data,
            icon_texture: Mutex::new(None),
        })
    }
}

impl Drawer for ToolBar {
    fn create(
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Self::new()
    }
    fn show_window(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {}

    fn ui(
        &self,
        ui: &mut egui::Ui,
        _global_styles: &GlobalStyles,
        messager: &mut super::Messager,
        _engine: &Engine,
        _log: &mut Log,
    ) {
        let model = &self.model;
        Panel::top("toolbar")
            .default_size(model.style.height)
            .show(ui, |ui| {
                ui.visuals_mut().override_text_color = Some(model.style.button_text_color.into());
                menu::MenuBar::new().ui(ui, |ui| {
                    ui.set_height(model.style.height);
                    // 工具栏背景
                    ui.painter().rect_filled(
                        ui.available_rect_before_wrap(),
                        model.style.corner_radius,
                        model.style.fill_color,
                    );

                    // Shared menu content — only one branch executes, so FnMut is fine.
                    let menu_content = |ui: &mut egui::Ui| {
                        if ui.button("About Kairos").clicked() {
                            messager.send(Message::OpenAboutWindow);
                        }
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            messager.send(Message::QuitEngine);
                        }
                    };

                    if let Some(icon) = &self.icon_data {
                        // Build egui texture lazily on first frame — keep the
                        // TextureHandle alive so egui doesn't free the texture.
                        {
                            let mut guard = self.icon_texture.lock();
                            if guard.is_none() {
                                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                    [icon.width, icon.height],
                                    &icon.rgba,
                                );
                                let handle = ui.ctx().load_texture(
                                    "toolbar_engine_icon",
                                    color_image,
                                    egui::TextureOptions::LINEAR,
                                );
                                *guard = Some(handle);
                            }
                        }

                        let tex_id = {
                            let guard = self.icon_texture.lock();
                            guard.as_ref().map(|h| h.id())
                        };
                        if let Some(tex_id) = tex_id {
                            ui.menu_image_button(
                                egui::Image::from_texture(egui::load::SizedTexture::new(
                                    tex_id,
                                    [icon.width as f32, icon.height as f32],
                                )),
                                menu_content,
                            );
                        }
                    } else {
                        ui.menu_button("Kairos", menu_content);
                    }

                    // Scene
                    ui.menu_button("Scene", |ui| {
                        if ui.button("New Scene").clicked() {
                            todo!()
                        }
                    });

                    // Editor
                    ui.menu_button("Edit", |ui| {
                        if ui.button("Preferences").clicked() {
                            messager.send(Message::OpenPreferenceWindow);
                        }
                    });

                    // Window
                    ui.menu_button("Window", |ui| {
                        // General
                        ui.menu_button("General", |ui| {
                            if ui.button("Inspector").clicked() {
                                messager.send(Message::OpenInspectorTab);
                            }
                            if ui.button("Hierarchy").clicked() {
                                messager.send(Message::OpenHierarchyTab);
                            }
                            if ui.button("Project").clicked() {
                                messager.send(Message::OpenProjectTab);
                            }
                            if ui.button("Console").clicked() {
                                messager.send(Message::OpenConsoleTab);
                            }
                        })
                    })
                });
            });
    }

    fn close(&self, _messager: &mut super::Messager) {}

    fn get_style_fileds(&self) -> Vec<StyleField> {
        let mut fields = Vec::new();
        let style = &self.model.style;

        fields.push(StyleField::FloatStyleField(FloatStyleField::new(
            "height",
            style.height,
            0.0,
            f32::MAX,
            FloatFieldEditViewType::Field,
        )));
        fields.push(StyleField::FloatStyleField(FloatStyleField::new(
            "button_width",
            style.button_width,
            0.0,
            f32::MAX,
            FloatFieldEditViewType::Field,
        )));
        fields.push(StyleField::FloatStyleField(FloatStyleField::new(
            "corrner_radius",
            style.corner_radius,
            0.0,
            f32::MAX,
            FloatFieldEditViewType::Field,
        )));
        fields.push(StyleField::ColorStyleField(ColorStyleField::new(
            "fill_color",
            style.fill_color,
        )));
        fields.push(StyleField::ColorStyleField(ColorStyleField::new(
            "button_text_color",
            style.button_text_color,
        )));

        fields
    }

    fn get_name(&self) -> &'static str {
        type_name::<ToolBar>()
    }

    fn update_style(&mut self, style_fields: &Vec<StyleField>) {
        if let StyleField::FloatStyleField(field) = &style_fields[0] {
            self.model.style.height = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[1] {
            self.model.style.button_width = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[2] {
            self.model.style.corner_radius = field.value;
        }
        if let StyleField::ColorStyleField(field) = &style_fields[3] {
            self.model.style.fill_color = field.color;
        }
        if let StyleField::ColorStyleField(field) = &style_fields[4] {
            self.model.style.button_text_color = field.color;
        }

        match toml::to_string(&self.model.style) {
            Ok(toml) => match std::fs::write(paths::PATH_EDITOR_WINDO_TOOL_BAR_STYLE, toml) {
                Ok(_) => (),
                Err(error) => {
                    kairos_dialog::error_message_window(
                        "Write File Falied",
                        &format!("Write the ToolBarStyle toml file Failed, Error: {}", error),
                    );
                }
            },
            Err(error) => {
                kairos_dialog::error_message_window(
                    "Serialize Data Failed",
                    &format!(
                        "Serialize the ToolBarStyle toml file Failed, Erro: {}",
                        error
                    ),
                );
            }
        }
    }

    fn get_title(&self) -> egui::WidgetText {
        "ToolBar".into()
    }

    fn render(
        &self,
        _engine: &mut Engine,
        _game: &mut KairosGame,
        _messager: &mut Messager,
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        None
    }
}
