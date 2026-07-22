use std::{any::type_name, cell::Cell, fs};

use egui::pos2;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::{
    asset_loader::assets::AssetsServer,
    graphics::{
        attachment::{Attachment, AttachmentLoadAction, AttachmentStoreAction},
        graphics_graph::{
            GraphicsCommand,
            graphics_node::{ColorAttachmentBind, DepthAttachmentBind},
        },
    },
    kairos_dialog,
    kairos_editor::{
        Engine,
        ui::{
            Drawer, Message, UIReader, paths,
            scene_camera::SceneCamera,
            scene_window::gizmos::{GizmosModel, GizmosRenderer},
            ui_style_fields::{
                FloatFieldEditViewType, FloatStyleField, RangeStyleField, StyleField,
                Vector3StyleField,
            },
        },
    },
    kairos_game::KairosGame,
    math::{self, float2, float3},
};

mod gizmos;

#[derive(Debug, Serialize, Deserialize)]
struct SceneWindowStyle {
    pub title: String,
    pub cam_default_position: float3,
    pub cam_default_target: float3,
    pub cam_default_fov: f32,
    pub cam_default_near: f32,
    pub cam_default_far: f32,
    pub cam_default_orbit_speed: f32,
    pub cam_default_zoom_speed: f32,
    pub cam_default_fly_acce_duration: f32,
    pub cam_default_fly_min_speed: f32,
    pub cam_default_fly_max_speed: f32,
    pub cam_default_min_distance: f32,
    pub cam_default_max_distance: f32,
}

struct SceneWindowModel {
    style: SceneWindowStyle,
    rt_id: Option<egui::TextureId>,
    width: u32,
    height: u32,
    egui_bind_tex_recever: Option<tokio::sync::oneshot::Receiver<egui::TextureId>>,
    drop_texture_id: Cell<Option<egui::TextureId>>,

    camera: SceneCamera,
    gizmos: GizmosModel,
}

pub struct SceneWindow {
    model: SceneWindowModel,
    gizmos_renderer: GizmosRenderer,
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
    pub fn new(assets_server: &mut AssetsServer) -> Result<Self, Box<dyn std::error::Error>> {
        let style = SceneWindowStyle::new()?;

        let scene_camera = SceneCamera::new(
            style.cam_default_position,
            style.cam_default_target,
            style.cam_default_fov,
            style.cam_default_near,
            style.cam_default_far,
            style.cam_default_orbit_speed,
            style.cam_default_zoom_speed,
            style.cam_default_fly_acce_duration,
            style.cam_default_fly_min_speed,
            style.cam_default_fly_max_speed,
            style.cam_default_min_distance,
            style.cam_default_max_distance,
        );
        let gizmos = GizmosModel::new(assets_server);

        Ok(Self {
            style,
            rt_id: None,
            width: 1,
            height: 1,
            egui_bind_tex_recever: None,
            drop_texture_id: Cell::new(None),
            camera: scene_camera,
            gizmos,
        })
    }
}

impl SceneWindow {
    #[inline(always)]
    pub fn new(assets_server: &mut AssetsServer) -> Result<Self, Box<dyn std::error::Error>> {
        let model = SceneWindowModel::new(assets_server)?;
        let gizmos_renderer = GizmosRenderer::new();
        Ok(Self {
            model,
            gizmos_renderer,
        })
    }
}

impl Drawer for SceneWindow {
    fn create(assets_server: &mut AssetsServer) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Self::new(assets_server)
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
        let available = ui.available_size_before_wrap();
        let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
        let pixels_per_point = ui.pixels_per_point();
        let width = (rect.width() * pixels_per_point).round().max(1.0) as u32;
        let height = (rect.height() * pixels_per_point).round().max(1.0) as u32;

        // --- Camera input (view: only sends messages, never mutates model) ---
        if response.hovered() {
            let dt = engine.time.delta_time().as_secs_f32();
            let delta = response.drag_delta();
            if response.dragged_by(egui::PointerButton::Secondary)
                || response.dragged_by(egui::PointerButton::Middle)
            {
                messager.send(Message::SceneCameraOrbit(-delta.x, -delta.y, dt));
            }
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            if scroll.y != 0.0 {
                messager.send(Message::CameraZoom(scroll.y, dt));
            }
            // WASD movement
            let w = ui.input(|i| i.key_down(egui::Key::W));
            let s = ui.input(|i| i.key_down(egui::Key::S));
            let a = ui.input(|i| i.key_down(egui::Key::A));
            let d = ui.input(|i| i.key_down(egui::Key::D));
            let forward = if w {
                1.0
            } else if s {
                -1.0
            } else {
                0.0
            };
            let right = if d {
                1.0
            } else if a {
                -1.0
            } else {
                0.0
            };
            if forward != 0.0 || right != 0.0 {
                messager.send(Message::CameraFly(right, forward, dt));
            }
        }

        if width != self.model.width || height != self.model.height {
            messager.send(Message::UpdateSceneWindowSize(width, height));
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
        let mut fields = Vec::new();
        let style = &self.model.style;

        fields.push(StyleField::Vector3StyleField(Vector3StyleField::new(
            "camera default position",
            style.cam_default_position,
            f32::MIN,
            f32::MAX,
        )));
        fields.push(StyleField::Vector3StyleField(Vector3StyleField::new(
            "camera default target",
            style.cam_default_target,
            f32::MIN,
            f32::MAX,
        )));
        fields.push(StyleField::FloatStyleField(FloatStyleField::new(
            "camera default fov",
            style.cam_default_fov,
            0.0,
            180.0,
            FloatFieldEditViewType::Field,
        )));
        fields.push(StyleField::FloatStyleField(FloatStyleField::new(
            "camera default near",
            style.cam_default_near,
            0.0001,
            math::min(style.cam_default_far, 100000.0),
            FloatFieldEditViewType::Field,
        )));
        fields.push(StyleField::FloatStyleField(FloatStyleField::new(
            "camera default far",
            style.cam_default_far,
            math::max(style.cam_default_near, 0.0001),
            100000.0,
            FloatFieldEditViewType::Field,
        )));
        fields.push(StyleField::FloatStyleField(FloatStyleField::new(
            "camera default orbit speed",
            style.cam_default_orbit_speed,
            0.0,
            0.1,
            FloatFieldEditViewType::Slider,
        )));
        fields.push(StyleField::FloatStyleField(FloatStyleField::new(
            "camera default zoom speed",
            style.cam_default_zoom_speed,
            0.0,
            1.0,
            FloatFieldEditViewType::Slider,
        )));
        fields.push(StyleField::FloatStyleField(FloatStyleField::new(
            "camera default fly acceleration duration",
            style.cam_default_fly_acce_duration,
            0.01,
            4.0,
            FloatFieldEditViewType::Slider,
        )));
        fields.push(StyleField::RangeStyleField(RangeStyleField::new(
            "camera default fly speed range",
            float2::new(
                style.cam_default_fly_min_speed,
                style.cam_default_fly_max_speed,
            ),
            0.01,
            40.0,
        )));
        fields.push(StyleField::RangeStyleField(RangeStyleField::new(
            "camera distance range",
            float2::new(
                style.cam_default_min_distance,
                style.cam_default_max_distance,
            ),
            math::max(style.cam_default_near, 0.0001),
            math::min(style.cam_default_far, 100000.0),
        )));

        fields
    }

    fn update_style(&mut self, style_fields: &Vec<super::ui_style_fields::StyleField>) {
        if let StyleField::Vector3StyleField(field) = &style_fields[0] {
            self.model.style.cam_default_position = field.value;
        }
        if let StyleField::Vector3StyleField(field) = &style_fields[1] {
            self.model.style.cam_default_target = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[2] {
            self.model.style.cam_default_fov = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[3] {
            self.model.style.cam_default_near = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[4] {
            self.model.style.cam_default_far = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[5] {
            self.model.style.cam_default_orbit_speed = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[6] {
            self.model.style.cam_default_zoom_speed = field.value;
        }
        if let StyleField::FloatStyleField(field) = &style_fields[7] {
            self.model.style.cam_default_fly_acce_duration = field.value;
        }
        if let StyleField::RangeStyleField(field) = &style_fields[8] {
            self.model.style.cam_default_fly_min_speed = field.range.x();
            self.model.style.cam_default_fly_max_speed = field.range.y();
        }
        if let StyleField::RangeStyleField(field) = &style_fields[9] {
            self.model.style.cam_default_min_distance = field.range.x();
            self.model.style.cam_default_max_distance = field.range.y();
        }

        match toml::to_string(&self.model.style) {
            Ok(toml) => match std::fs::write(paths::PATH_SCENE_WINDOW_STYLE, toml) {
                Ok(_) => (),
                Err(error) => {
                    kairos_dialog::error_message_window(
                        "Write File Falied",
                        &format!(
                            "Write the SceneWindowStyle toml file Failed, Error: {}",
                            error
                        ),
                    );
                }
            },
            Err(error) => {
                kairos_dialog::error_message_window(
                    "Serialize Data Failed",
                    &format!(
                        "Serialize the SceneWindowStyle toml file Failed, Erro: {}",
                        error
                    ),
                );
            }
        }
    }

    fn render(
        &self,
        engine: &mut Engine,
        game: &mut KairosGame,
        messager: &mut super::Messager,
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        let mut graphics_command = GraphicsCommand::new(16, 2, 4, 16);

        if self.model.egui_bind_tex_recever.is_some() {
            messager.send(Message::SceneWindowTryReceTextureId);
        }

        // Free previous texture exactly once, then clear.
        if let Some(drop_texture_id) = self.model.drop_texture_id.take() {
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
        let scene_depth_stencil = Attachment::new(
            Some("SceneWindow DepthStencil"),
            width,
            height,
            crate::graphics::attachment::AttachmentFormat::D24S8,
        );
        let scene_view_id = graphics_command.create_color_attachment(scene_view);
        let scene_view_bind = ColorAttachmentBind::new(
            scene_view_id,
            AttachmentLoadAction::LoadClear,
            AttachmentStoreAction::Store,
        );
        let scene_depth_id = graphics_command.create_depth_attachment(scene_depth_stencil);
        let scene_depth_bind = DepthAttachmentBind::new(
            scene_depth_id,
            Some((
                AttachmentLoadAction::LoadClear,
                AttachmentStoreAction::Store,
            )),
            Some((
                AttachmentLoadAction::LoadClear,
                AttachmentStoreAction::Store,
            )),
        );

        let vp_id =
            graphics_command.set_view_projection_matrix(self.model.camera.view_projection());
        graphics_command.begin_render_pass(
            Some("SceneWindow Render Pass"),
            vec![scene_view_bind],
            Some(scene_depth_bind),
            vp_id,
            4,
        );

        game.render(engine, &mut graphics_command);

        graphics_command.end_render_pass();

        let scene_view_gizmos_bind = ColorAttachmentBind::new(
            scene_view_id,
            AttachmentLoadAction::Load,
            AttachmentStoreAction::Store,
        );
        let scene_depth_gizmos_bind = DepthAttachmentBind::new(
            scene_depth_id,
            Some((
                AttachmentLoadAction::LoadClear,
                AttachmentStoreAction::Store,
            )),
            Some((
                AttachmentLoadAction::LoadClear,
                AttachmentStoreAction::Store,
            )),
        );

        graphics_command.begin_render_pass(
            Some("SceneWindow Gizmos Render Pass"),
            vec![scene_view_gizmos_bind],
            Some(scene_depth_gizmos_bind),
            vp_id,
            4,
        );

        self.gizmos_renderer
            .render_gizmos(&self.model.gizmos, &mut graphics_command);

        graphics_command.end_render_pass();

        // Only create a new egui bind if the previous one has been consumed.
        if self.model.egui_bind_tex_recever.is_none() {
            let (egui_bind_tex_sender, egui_bind_tex_recever) = tokio::sync::oneshot::channel();
            messager.send(Message::RegisteSceneWindowViewBind(egui_bind_tex_recever));
            graphics_command.bind_attachment_to_egui(scene_view_id, egui_bind_tex_sender);
        }

        Some(graphics_command)
    }
}

impl SceneWindow {
    // --- Camera controller (mutates model, called from Context::handle) ---

    pub fn on_camera_orbit(&mut self, dx: f32, dy: f32, dt: f32) {
        self.model.camera.orbit(dx, dy, dt);
    }

    pub fn on_camera_zoom(&mut self, delta: f32, dt: f32) {
        self.model.camera.zoom(delta, dt);
    }

    pub fn on_camera_fly(&mut self, right: f32, forward: f32, dt: f32) {
        self.model.camera.fly(right, forward, dt);
    }

    pub fn update_size(&mut self, width: u32, height: u32) {
        self.model.width = width;
        self.model.height = height;
        self.model.camera.aspect = width as f32 / height as f32;
    }

    pub fn register_view_bind(&mut self, recever: tokio::sync::oneshot::Receiver<egui::TextureId>) {
        self.model.egui_bind_tex_recever = Some(recever);
        // self.try_rece_texture_id();
    }

    pub fn try_rece_texture_id(&mut self) {
        let received = {
            match &mut self.model.egui_bind_tex_recever {
                Some(recever) => match recever.try_recv() {
                    Ok(texuter_id) => {
                        self.model.drop_texture_id.set(self.model.rt_id.take());
                        self.model.rt_id = Some(texuter_id);
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
