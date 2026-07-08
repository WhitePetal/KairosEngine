use std::{any::type_name, fs};

use crate::{
    kairos_editor::{
        Engine,
        ui::{Messager, egui_ext::UiExt, global_styles::GlobalStyles},
    },
    kairos_game::KairosGame,
    log::Log,
    math::{self, float2},
};
use egui::{self, Vec2};
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::ui::{
    Drawer, Message, paths,
    ui_style_fields::{FloatFieldEditViewType, FloatStyleField, StyleField, StylePage},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct PreferencesStyle {
    pub title: String,
    pub default_width: f32,
    pub default_height: f32,
    pub grid_space_x: f32,
    pub grid_space_y: f32,
}

impl PreferencesStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json =
            fs::read_to_string(paths::PATH_PREFERENCES_WINDOW_STYLE).map_err(|error| {
                format!(
                    "Load Preferences Window Style Json Failed, path: {}, error: {}",
                    paths::PATH_PREFERENCES_WINDOW_STYLE,
                    error
                )
            })?;
        let style = from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize Preferences Window Style Json Failed, error: {}",
                error
            )
        })?;

        Ok(style)
    }
}

pub struct PreferencesModel {
    style: PreferencesStyle,
    style_pages: Option<Vec<StylePage>>,
    selected_id: Option<usize>,
}

impl PreferencesModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = PreferencesStyle::new()?;

        Ok(Self {
            style,
            style_pages: None,
            selected_id: None,
        })
    }
}

pub struct PreferencesWindow {
    model: PreferencesModel,
}

impl PreferencesWindow {
    #[inline(always)]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = PreferencesModel::new()?;

        Ok(Self { model })
    }

    pub fn set_selected_id(&mut self, id: usize) {
        self.model.selected_id = Some(id);
    }
}

impl Drawer for PreferencesWindow {
    fn create(
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Self::new()
    }

    fn show_window(&self, state: Option<&mut super::docking_tab::window_state::WindowState>) {
        if let Some(state) = state {
            state.set_size(Vec2::new(
                self.model.style.default_width,
                self.model.style.default_height,
            ));
        }
    }

    fn ui(
        &self,
        ui: &mut egui::Ui,
        _global_styles: &GlobalStyles,
        messager: &mut super::Messager,
        _engine: &Engine,
        _log: &mut Log,
    ) {
        self.ui(ui, messager);
    }

    fn close(&self, messager: &mut super::Messager) {
        messager.send(super::Message::ClosePreferenceWindow);
    }

    fn get_style_fileds(&self) -> Vec<super::ui_style_fields::StyleField> {
        let mut fileds = Vec::new();
        let style = &self.model.style;

        fileds.push(StyleField::FloatStyleField(FloatStyleField::new(
            "default width",
            style.default_width,
            0.0,
            f32::MAX,
            FloatFieldEditViewType::Field,
        )));
        fileds.push(StyleField::FloatStyleField(FloatStyleField::new(
            "default height",
            style.default_height,
            0.0,
            f32::MAX,
            FloatFieldEditViewType::Field,
        )));
        fileds.push(StyleField::FloatStyleField(FloatStyleField::new(
            "grid space x",
            style.grid_space_x,
            0.0,
            f32::MAX,
            FloatFieldEditViewType::Field,
        )));
        fileds.push(StyleField::FloatStyleField(FloatStyleField::new(
            "grid space y",
            style.grid_space_y,
            0.0,
            f32::MAX,
            FloatFieldEditViewType::Field,
        )));

        fileds
    }

    fn get_name(&self) -> &'static str {
        type_name::<PreferencesWindow>()
    }

    fn update_style(&mut self, style_fields: &Vec<super::ui_style_fields::StyleField>) {
        if let StyleField::FloatStyleField(field) = &style_fields[0] {
            self.model.style.default_width = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[1] {
            self.model.style.default_height = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[2] {
            self.model.style.grid_space_x = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[3] {
            self.model.style.grid_space_y = field.value;
        }
    }

    fn get_title(&self) -> egui::WidgetText {
        self.model.style.title.to_owned().into()
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

impl PreferencesWindow {
    pub fn registe_ui_styles(&mut self, style_pages: Vec<StylePage>) {
        self.model.style_pages = Some(style_pages);
    }

    pub fn update_style_page(&mut self, style_page: &StylePage) {
        if let Some(pages) = &mut self.model.style_pages {
            let page = &mut pages[style_page.id];
            page.fields.copy_from_slice(&style_page.fields);
        }
    }

    fn ui(&self, ui: &mut egui::Ui, messager: &mut super::Messager) {
        let model = &self.model;

        let selected_id = &model.selected_id;

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                if let Some(style_pages) = &model.style_pages {
                    for page in style_pages {
                        if ui
                            .selectable_label(*selected_id == Some(page.id), page.name)
                            .clicked()
                        {
                            messager.send(Message::SetPreferenceWindowSelectedId(page.id));
                            break;
                        }
                    }
                }
            });

            ui.add(egui::Separator::default().vertical());

            ui.vertical_centered(|ui| {
                if let Some(id) = selected_id {
                    if let Some(style_pages) = &model.style_pages {
                        let mut page = style_pages[*id].clone();

                        ui.horizontal_top(|ui| {
                            ui.heading(page.name);
                        });

                        ui.add(egui::Separator::default().horizontal());

                        let ui_builder = egui::UiBuilder::new();
                        ui.scope_builder(ui_builder, |ui| {
                            egui::Grid::new("PreferenceWindow_Fields_Grid")
                                .num_columns(2)
                                .spacing([model.style.grid_space_x, model.style.grid_space_y])
                                .striped(true)
                                .show(ui, |ui| {
                                    let fields = &mut page.fields;
                                    for field in fields {
                                        match field {
                                            super::ui_style_fields::StyleField::FloatStyleField(
                                                float_style_field,
                                            ) => {
                                                ui.label(float_style_field.name);
                                                match float_style_field.view_type {
                                                    FloatFieldEditViewType::Field => {
                                                        ui.add(
                                                            egui::DragValue::new(
                                                                &mut float_style_field.value,
                                                            )
                                                            .range(
                                                                float_style_field.min
                                                                    ..=float_style_field.max,
                                                            ),
                                                        );
                                                    }
                                                    FloatFieldEditViewType::Slider => {
                                                        ui.add(egui::Slider::new(
                                                            &mut float_style_field.value,
                                                            float_style_field.min
                                                                ..=float_style_field.max,
                                                        ));
                                                    }
                                                }
                                                ui.end_row();
                                            }
                                            super::ui_style_fields::StyleField::ColorStyleField(
                                                color_style_field,
                                            ) => {
                                                ui.label(color_style_field.name);
                                                let color = &color_style_field.color;
                                                let mut color_data: [u8; 4] =
                                                    [color.r, color.g, color.b, color.a];
                                                ui.color_edit_button_srgba_premultiplied(
                                                    &mut color_data,
                                                );
                                                color_style_field.color = math::Color32::new(
                                                    color_data[0],
                                                    color_data[1],
                                                    color_data[2],
                                                    color_data[3],
                                                );
                                                ui.end_row();
                                            }
                                            super::ui_style_fields::StyleField::Vector2StyleField(
                                                field,
                                            ) => {
                                                ui.label(field.name);
                                                let mut arr = field.value.to_array();
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("X")
                                                            .color(egui::Color32::RED),
                                                    );
                                                    ui.add(
                                                        egui::DragValue::new(&mut arr[0])
                                                            .range(field.min..=field.max),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new("Y")
                                                            .color(egui::Color32::GREEN),
                                                    );
                                                    ui.add(
                                                        egui::DragValue::new(&mut arr[1])
                                                            .range(field.min..=field.max),
                                                    );
                                                });
                                                field.value = crate::math::float2::from_array(arr);
                                                ui.end_row();
                                            }
                                            super::ui_style_fields::StyleField::Vector3StyleField(
                                                field,
                                            ) => {
                                                ui.label(field.name);
                                                let mut arr = field.value.to_array();
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("X")
                                                            .color(egui::Color32::RED),
                                                    );
                                                    ui.add(
                                                        egui::DragValue::new(&mut arr[0])
                                                            .range(field.min..=field.max),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new("Y")
                                                            .color(egui::Color32::GREEN),
                                                    );
                                                    ui.add(
                                                        egui::DragValue::new(&mut arr[1])
                                                            .range(field.min..=field.max),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new("Z")
                                                            .color(egui::Color32::BLUE),
                                                    );
                                                    ui.add(
                                                        egui::DragValue::new(&mut arr[2])
                                                            .range(field.min..=field.max),
                                                    );
                                                });
                                                field.value =
                                                    crate::math::float3::from_array_4(arr);
                                                ui.end_row();
                                            }
                                            super::ui_style_fields::StyleField::Vector4StyleField(
                                                field,
                                            ) => {
                                                ui.label(field.name);
                                                let mut arr = [
                                                    field.value.x(),
                                                    field.value.y(),
                                                    field.value.z(),
                                                    field.value.w(),
                                                ];
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("X")
                                                            .color(egui::Color32::RED),
                                                    );
                                                    ui.add(
                                                        egui::DragValue::new(&mut arr[0])
                                                            .range(field.min..=field.max),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new("Y")
                                                            .color(egui::Color32::GREEN),
                                                    );
                                                    ui.add(
                                                        egui::DragValue::new(&mut arr[1])
                                                            .range(field.min..=field.max),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new("Z")
                                                            .color(egui::Color32::BLUE),
                                                    );
                                                    ui.add(
                                                        egui::DragValue::new(&mut arr[2])
                                                            .range(field.min..=field.max),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new("W")
                                                            .color(egui::Color32::GRAY),
                                                    );
                                                    ui.add(
                                                        egui::DragValue::new(&mut arr[3])
                                                            .range(field.min..=field.max),
                                                    );
                                                });
                                                field.value = crate::math::float4::new(
                                                    arr[0], arr[1], arr[2], arr[3],
                                                );
                                                ui.end_row();
                                            }
                                            StyleField::RangeStyleField(field) => {
                                                ui.label(field.name);
                                                let mut low = field.range.x();
                                                let mut high = field.range.y();
                                                ui.range_slider(&mut low, &mut high, field.min..=field.max);
                                                field.range = float2::new(low, high);
                                                ui.end_row();
                                            },
                                        }
                                    }
                                })
                        });

                        messager.send(Message::UpdateUIStyle(page));
                    }
                } else {
                    ui.horizontal_top(|ui| {
                        ui.label("Select a Setting...");
                    });
                }
            });
        });
    }
}
