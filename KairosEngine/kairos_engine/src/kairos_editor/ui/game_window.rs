use std::{any::type_name, fs};

use egui::pos2;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::{
    graphics::{
        attachment::{Attachment, AttachmentLoadAction, AttachmentStoreAction},
        camera::CameraView,
        drawer::DrawCommand,
        egui_texture_handle::EguiTextureHandle,
        graphics_graph::{
            GraphicsCommand,
            graphics_node::{ColorAttachmentBind, DepthAttachmentBind},
        },
        view_port::GameView,
    },
    kairos_editor::{
        Engine,
        ui::{Drawer, Message, UIReader, paths},
    },
    kairos_game::KairosGame,
    math::float4x4,
};

#[derive(Debug, Serialize, Deserialize)]
struct GameWindowStyle {
    pub title: String,
}

struct GameWindowModel {
    style: GameWindowStyle,
    rt_handle: Option<EguiTextureHandle>,
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
        _world: &kairos_ecs::world::World,
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
        engine: &Engine,
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

        // The view resource is the only place the size is stored; write back
        // only on a real change (the message handler owns the write).
        let size = engine.world.resource::<GameView>().size;
        if size.width != width || size.height != height {
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
        _game: &mut KairosGame,
        messager: &mut super::Messager,
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        let mut graphics_command = GraphicsCommand::new(16, 2, 4, 16);

        if self.model.egui_bind_tex_recever.is_some() {
            messager.send(Message::GameWindowTryReceTextureId);
        }

        // Thin play layer: everything comes from the view resource, nothing is
        // queried out of the world. A closed window (`size == 0`) renders
        // nothing at all — no attachment, no stale frame.
        let view = engine.world.resource::<GameView>();
        let size = view.size;
        if !size.is_renderable() {
            return None;
        }

        let game_view = Attachment::new(
            Some("GameWindow Attachment"),
            size.width,
            size.height,
            crate::graphics::attachment::AttachmentFormat::RGBA8UNorm,
        );
        let game_depth_stencil = Attachment::new(
            Some("GameWindow DepthStencil"),
            size.width,
            size.height,
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

        // The view projection lives on the bound camera entity, not on the view
        // resource. `None` means no camera is bound (or the bound entity is
        // gone / derived nothing this frame). `draws` is gated on size alone, so
        // it can be non-empty with no projection — drain it only when there is
        // one, and fall back to an identity VP for the clear-only pass.
        let view_projection = view
            .camera
            .and_then(|camera| engine.world.entity(camera).get::<CameraView>())
            .and_then(|derived| derived.view_projection);
        let (view_projection, draws): (float4x4, &[DrawCommand]) = match view_projection {
            Some(view_projection) => (view_projection, &view.draws),
            None => (float4x4::IDENTITY, &[]),
        };
        let vp_id = graphics_command.set_view_projection_matrix(view_projection);

        graphics_command.begin_render_pass(
            Some("GameWindow Render Pass"),
            vec![game_view_bind],
            Some(game_depth_bind),
            vp_id,
            draws.len(),
        );

        for draw in draws {
            graphics_command.draw(
                draw.mesh.clone(),
                draw.material.clone(),
                draw.local_to_world,
            );
        }

        graphics_command.end_render_pass();

        // Only create a new egui bind if the previous one has been consumed.
        if self.model.egui_bind_tex_recever.is_none() {
            let (egui_bind_tex_sender, egui_bind_tex_recever) = tokio::sync::oneshot::channel();
            messager.send(Message::RegisteGameWindowViewBind(egui_bind_tex_recever));
            graphics_command.bind_attachment_to_egui(game_view_id, egui_bind_tex_sender);
        }

        Some(graphics_command)
    }
}

impl GameWindow {
    pub fn register_view_bind(
        &mut self,
        recever: tokio::sync::oneshot::Receiver<EguiTextureHandle>,
    ) {
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
