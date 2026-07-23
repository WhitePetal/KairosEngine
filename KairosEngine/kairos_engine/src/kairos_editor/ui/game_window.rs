use std::{any::type_name, fs};

use egui::pos2;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::{
    graphics::{
        attachment::{Attachment, AttachmentLoadAction, AttachmentStoreAction}, camera::Camera, egui_texture_handle::EguiTextureHandle, graphics_graph::{
            GraphicsCommand,
            graphics_node::{ColorAttachmentBind, DepthAttachmentBind},
        }
    },
    kairos_editor::{
        Engine,
        ui::{Drawer, Message, UIReader, paths},
    },
    kairos_game::KairosGame,
    spatial::Transform,
};

#[derive(Debug, Serialize, Deserialize)]
struct GameWindowStyle {
    pub title: String,
}

struct GameWindowModel {
    style: GameWindowStyle,
    rt_handle: Option<EguiTextureHandle>,
    width: u32,
    height: u32,
    egui_bind_tex_recever: Option<tokio::sync::oneshot::Receiver<EguiTextureHandle>>,
}

pub struct GameWindow {
    model: GameWindowModel,
}

impl GameWindowStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_GAME_WINDOW_STYLE).map_err(|error| {
            format!(
                "Load GameWindow Style Json Failed, path: {}, error: {}",
                paths::PATH_GAME_WINDOW_STYLE,
                error
            )
        })?;
        let style = from_str(&style_json).map_err(|error| {
            format!("Deserialize GameWindow Style Json Failed, error: {}", error)
        })?;
        Ok(style)
    }
}

impl GameWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = GameWindowStyle::new()?;
        Ok(Self {
            style,
            rt_handle: None,
            width: 1,
            height: 1,
            egui_bind_tex_recever: None,
        })
    }
}

impl GameWindow {
    #[inline(always)]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = GameWindowModel::new()?;
        Ok(Self { model })
    }
}

impl Drawer for GameWindow {
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
        _reader: &UIReader,
        messager: &mut super::Messager,
        _engine: &Engine,
        _log: &mut crate::log::Log,
    ) {
        egui::Frame::NONE
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Display label
                    ui.label("Display 1");
                    ui.separator();

                    // Aspect ratio dropdown (placeholder)
                    egui::ComboBox::from_id_salt("game_aspect_ratio")
                        .width(80.0)
                        .selected_text("Free Aspect")
                        .show_ui(ui, |ui| {
                            ui.label("Free Aspect");
                            ui.label("16:9");
                            ui.label("4:3");
                        });

                    // Scale slider (placeholder)
                    ui.add(
                        egui::Slider::new(&mut 1.0_f32, 0.25..=2.0)
                            .text("Scale")
                            .fixed_decimals(1),
                    );

                    // Maximize on play toggle (placeholder)
                    ui.checkbox(&mut false, "Maximize On Play");

                    // Stats toggle (placeholder)
                    ui.checkbox(&mut false, "Stats");

                    // Gizmos toggle (placeholder)
                    ui.checkbox(&mut true, "Gizmos");
                });
            });

        // --- Render target area ---
        let available = ui.available_size_before_wrap();
        let (rect, _) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        let pixels_per_point = ui.pixels_per_point();
        let width = (rect.width() * pixels_per_point).round().max(1.0) as u32;
        let height = (rect.height() * pixels_per_point).round().max(1.0) as u32;

        if width != self.model.width || height != self.model.height {
            messager.send(Message::UpdateGameWindowSize(width, height));
        }

        if let Some(rt_handle) = &self.model.rt_handle {
            ui.painter().image(
                rt_handle.id(),
                rect,
                egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    }

    fn close(&self, messager: &mut super::Messager) {
        messager.send(Message::CloseGameTab);
    }

    fn scroll_bars(&self) -> [bool; 2] {
        [false, false]
    }

    fn get_name(&self) -> &'static str {
        type_name::<GameWindow>()
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
        engine: &mut Engine,
        game: &mut KairosGame,
        messager: &mut super::Messager,
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        let mut graphics_command = GraphicsCommand::new(16, 2, 4, 16);

        if self.model.egui_bind_tex_recever.is_some() {
            messager.send(Message::GameWindowTryReceTextureId);
        }

        // draw
        let width = self.model.width;
        let height = self.model.height;
        let game_view = Attachment::new(
            Some("GameWindow Attachment"),
            width,
            height,
            crate::graphics::attachment::AttachmentFormat::RGBA8UNorm,
        );
        let game_depth_stencil = Attachment::new(
            Some("GameWindow DepthStencil"),
            width,
            height,
            crate::graphics::attachment::AttachmentFormat::D24S8,
        );
        let game_view_id = graphics_command.create_color_attachment(game_view);
        let game_view_bind = ColorAttachmentBind::new(
            game_view_id,
            AttachmentLoadAction::LoadClear,
            AttachmentStoreAction::Store,
        );
        let game_depth_id = graphics_command.create_depth_attachment(game_depth_stencil);
        let game_depth_bind = DepthAttachmentBind::new(
            game_depth_id,
            Some((
                AttachmentLoadAction::LoadClear,
                AttachmentStoreAction::Store,
            )),
            Some((
                AttachmentLoadAction::LoadClear,
                AttachmentStoreAction::Store,
            )),
        );

        if let Some((transform, camera)) = engine
            .world
            .query_mut::<(&Transform, &mut Camera)>()
            .into_iter()
            .next()
        {
            camera.aspect = width as f32 / height as f32;
            let vp_id = graphics_command
                .set_view_projection_matrix(camera.get_view_projection_matrix(*transform));
            graphics_command.begin_render_pass(
                Some("GameWindow Render Pass"),
                vec![game_view_bind],
                Some(game_depth_bind),
                vp_id,
                4,
            );

            game.render(engine, &mut graphics_command);

            graphics_command.end_render_pass();

            let (egui_bind_tex_sender, egui_bind_tex_recever) = tokio::sync::oneshot::channel();
            messager.send(Message::RegisteGameWindowViewBind(egui_bind_tex_recever));
            graphics_command.bind_attachment_to_egui(game_view_id, egui_bind_tex_sender);
            return Some(graphics_command);
        };

        None
    }
}

impl GameWindow {
    pub fn update_size(&mut self, width: u32, height: u32) {
        self.model.width = width;
        self.model.height = height;
    }

    pub fn register_view_bind(&mut self, recever: tokio::sync::oneshot::Receiver<EguiTextureHandle>) {
        self.model.egui_bind_tex_recever = Some(recever);
    }

    pub fn try_rece_texture_id(&mut self) {
        let received = {
            match &mut self.model.egui_bind_tex_recever {
                Some(recever) => match recever.try_recv() {
                    Ok(texture_handle) => {
                        self.model.rt_handle.replace(texture_handle);
                        true
                    }
                    Err(_) => false,
                },
                None => false,
            }
        };
        if received {
            self.model.egui_bind_tex_recever.take();
        }
    }
}
