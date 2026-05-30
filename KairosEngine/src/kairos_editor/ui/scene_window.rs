use std::{any::type_name, fs};

use egui::pos2;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::{
    graphics::{
        attachment::Attachment, camera::Camera, graphics_graph::GraphicsCommand, mesh::Mesh,
        render_pipeline::RenderPipeline, vertex::Vertex,
    },
    kairos_editor::ui::{Drawer, Message, paths},
    math::{self, float2, float3, float4},
};

#[derive(Debug, Serialize, Deserialize)]
struct SceneWindowStyle {
    pub title: String,
}

struct SceneWindowModel {
    style: SceneWindowStyle,
    rt_id: Option<egui::TextureId>,
    width: u32,
    height: u32,
    recever: Option<tokio::sync::oneshot::Receiver<egui::TextureId>>,
    drop_texture_id: Option<egui::TextureId>,
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
        Ok(Self {
            style,
            rt_id: None,
            width: 1,
            height: 1,
            recever: None,
            drop_texture_id: None,
        })
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

    fn ui(&self, ui: &mut egui::Ui, messager: &mut super::Messager, _log: &mut crate::log::Log) {
        let available = ui.available_size_before_wrap();
        let (rect, _) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        let pixels_per_point = ui.pixels_per_point();
        let width = (rect.width() * pixels_per_point).round().max(1.0) as u32;
        let height = (rect.height() * pixels_per_point).round().max(1.0) as u32;

        if width != self.model.width || height != self.model.height {
            messager.send(Message::UpdateSceneWindowSize(width, height));
        }

        if self.model.recever.is_some() {
            messager.send(Message::SceneWindowTryReceTextureId);
        }

        if let Some(rt_id) = self.model.rt_id {
            ui.painter().image(
                rt_id,
                rect,
                egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
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

    fn render(
        &self,
        messager: &mut super::Messager,
        render_pipeline: &RenderPipeline,
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        let mut graphics_command = GraphicsCommand::new(16, 4, 16);
        // clear last rt_id
        if let Some(drop_texture_id) = self.model.drop_texture_id {
            graphics_command.free_egui_texture_id(drop_texture_id);
        }

        // draw
        let width = self.model.width;
        let height = self.model.height;
        let scene_view = Attachment::new(
            Some("SceneWindow Attachment"),
            width,
            height,
            crate::graphics::attachment::AttachmentFormat::RGBA8UNorm,
        );
        let scene_view_id = graphics_command.create_attachment(scene_view);
        let cam_pos = float3::new(0.0, 1.0, -2.0);
        let cam_taget = float3::new(0.0, 0.0, 0.0);
        let cam_forward = math::normalize(cam_taget - cam_pos);
        let world_up = float3::new(0.0, 1.0, 0.0);
        let cam_right = math::cross(world_up, cam_forward);
        let camera = Camera::new(
            float3::new(0.0, 1.0, -2.0),
            cam_forward,
            cam_right,
            45.,
            width as f32 / height as f32,
            0.3,
            100.,
        );
        let vp_id =
            graphics_command.set_view_projection_matrix(camera.get_view_projection_matrix());
        graphics_command.begin_render_pass(
            Some("SceneWindow Render Pass"),
            vec![scene_view_id],
            vp_id,
            4,
            true,
        );

        let vertices = vec![
            Vertex {
                position: float4::new(-0.0868241, 0.49240386, 0.0, 1.0),
                color: float4::new(0.5, 0.0, 0.5, 1.0),
                texcoord: float2::new(0.4131759, 0.00759614),
            }, // A
            Vertex {
                position: float4::new(-0.49513406, 0.06958647, 0.0, 1.0),
                color: float4::new(0.5, 0.0, 0.5, 1.0),
                texcoord: float2::new(0.0048659444, 0.43041354),
            }, // B
            Vertex {
                position: float4::new(-0.21918549, -0.44939706, 0.0, 1.0),
                color: float4::new(0.5, 0.0, 0.5, 1.0),
                texcoord: float2::new(0.28081453, 0.949397),
            }, // C
            Vertex {
                position: float4::new(0.35966998, -0.3473291, 0.0, 1.0),
                color: float4::new(0.5, 0.0, 0.5, 1.0),
                texcoord: float2::new(0.85967, 0.84732914),
            }, // D
            Vertex {
                position: float4::new(0.44147372, 0.2347359, 0.0, 1.0),
                color: float4::new(0.5, 0.0, 0.5, 1.0),
                texcoord: float2::new(0.9414737, 0.2652641),
            }, // E
        ];
        let indices = vec![0, 1, 4, 1, 2, 4, 2, 3, 4];

        let mesh = Mesh::new(vertices, indices);
        graphics_command.draw(mesh);
        graphics_command.end_render_pass();
        let (egui_bind_tex_sender, egui_bind_tex_recever) = tokio::sync::oneshot::channel();
        messager.send(Message::RegesiterSceneWindowViewBind(egui_bind_tex_recever));
        graphics_command.bind_attachment_to_egui(scene_view_id, egui_bind_tex_sender);

        Some(graphics_command)
    }
}

impl SceneWindow {
    pub fn set_rt_id(&mut self, rt_id: egui::TextureId) {
        self.model.rt_id = Some(rt_id)
    }

    pub fn update_size(&mut self, width: u32, height: u32) {
        self.model.width = width;
        self.model.height = height;
    }

    pub fn register_view_bind(&mut self, recever: tokio::sync::oneshot::Receiver<egui::TextureId>) {
        self.model.recever = Some(recever);
        // self.try_rece_texture_id();
    }

    pub fn try_rece_texture_id(&mut self) {
        let received = {
            match &mut self.model.recever {
                Some(recever) => match recever.try_recv() {
                    Ok(texuter_id) => {
                        self.model.drop_texture_id = self.model.rt_id.take();
                        self.model.rt_id = Some(texuter_id);
                        true
                    }
                    Err(_) => false,
                },
                None => false,
            }
        };
        if received {
            self.model.recever.take();
        }
    }
}
