use std::{any::type_name, fs};

use egui::pos2;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::{
    graphics::render_pipeline::RenderPipeline,
    kairos_editor::ui::{Drawer, Message, paths},
};

#[derive(Debug, Serialize, Deserialize)]
struct SceneWindowStyle {
    pub title: String,
}

struct SceneWindowModel {
    style: SceneWindowStyle,
    rt_id: Option<egui::TextureId>,
}

pub struct SceneWindow {
    model: SceneWindowModel,
}

impl SceneWindowStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_SCENE_WINDOW_STYLE).map_err(|error| {
            format!(
                "Load SceneWindow Style Json Failed, path: {}, error: {}",
                paths::PATH_SCENE_WINDOW_STYLE,
                error
            )
        })?;
        let style = from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize SceneWindow Style Json Failed, error: {}",
                error
            )
        })?;
        Ok(style)
    }
}

impl SceneWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = SceneWindowStyle::new()?;
        Ok(Self { style, rt_id: None })
    }
}

impl SceneWindow {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = SceneWindowModel::new()?;
        Ok(Self { model })
    }
}

impl Drawer for SceneWindow {
    fn show_window(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {}

    fn ui(
        &self,
        ui: &mut egui::Ui,
        render_pipeline: &mut RenderPipeline,
        render_command_encoder: &mut wgpu::CommandEncoder,
        egui_renderer: &mut egui_wgpu::Renderer,
        messager: &mut super::Messager,
        _log: &mut crate::log::Log,
    ) {
        let available = ui.available_size_before_wrap();
        let (rect, _) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        let pixels_per_point = ui.pixels_per_point();
        let width = (rect.width() * pixels_per_point).round().max(1.0) as u32;
        let height = (rect.height() * pixels_per_point).round().max(1.0) as u32;

        // for fixed width / height
        // let mut width = rect.width();
        // let mut height = width * 0.45;
        // if height > rect.height() {
        //     height = rect.height();
        //     width = height * 2.2222;
        // }
        // let rect = egui::Rect::from_center_size(rect.center(), egui::Vec2 { x: width, y: height });
        // let width = (rect.width() * pixels_per_point).round().max(1.0) as u32;
        // let height = (rect.height() * pixels_per_point).round().max(1.0) as u32;

        let rt_view = render_pipeline.create_render_target("SceneView", width, height);
        render_pipeline.render(render_command_encoder, &rt_view);

        match self.model.rt_id {
            Some(rt_id) => {
                egui_renderer.update_egui_texture_from_wgpu_texture(
                    &render_pipeline.device,
                    &rt_view,
                    wgpu::FilterMode::Linear,
                    rt_id,
                );
                ui.painter().image(
                    rt_id,
                    rect,
                    egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            None => {
                let rt_id = egui_renderer.register_native_texture(
                    &render_pipeline.device,
                    &rt_view,
                    wgpu::FilterMode::Linear,
                );
                messager.send(Message::CreateSceneTabRt(rt_id));
            }
        }
    }

    fn close(&self, messager: &mut super::Messager) {
        messager.send(Message::CloseSceneTab);
    }

    fn scroll_bars(&self) -> [bool; 2] {
        [false, false]
    }

    fn get_name(&self) -> &'static str {
        type_name::<SceneWindow>()
    }

    fn get_title(&self) -> egui::WidgetText {
        self.model.style.title.to_owned().into()
    }

    fn get_style_fileds(&self) -> Vec<super::ui_style_fields::StyleField> {
        Vec::new()
    }

    fn update_style(&mut self, _style_fields: &Vec<super::ui_style_fields::StyleField>) {}
}

impl SceneWindow {
    pub fn set_rt_id(&mut self, rt_id: egui::TextureId) {
        self.model.rt_id = Some(rt_id)
    }
}
