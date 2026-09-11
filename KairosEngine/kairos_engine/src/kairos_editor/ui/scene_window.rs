use std::{any::type_name, fs};

use egui::pos2;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::{
    asset_loader::assets::AssetsServer,
    graphics::{
        attachment::{Attachment, AttachmentLoadAction, AttachmentStoreAction},
        camera::{Camera, CameraView},
        drawer::DrawCommand,
        egui_texture_handle::EguiTextureHandle,
        graphics_graph::{
            GraphicsCommand,
            graphics_node::{ColorAttachmentBind, DepthAttachmentBind},
        },
        view_port::SceneView,
    },
    kairos_dialog,
    kairos_editor::{
        Engine,
        camera::{EditorCameraController, OrbitState, OrbitTuning},
        ui::{
            Drawer, Message, UIReader, paths,
            scene_window::gizmos::{GizmosModel, GizmosRenderer},
            ui_style_fields::{
                FloatFieldEditViewType, FloatStyleField, RangeStyleField, StyleField,
                Vector3StyleField,
            },
        },
    },
    kairos_game::KairosGame,
    math::{self, float2, float3, float4x4},
};
use kairos_ecs::{entity::Entity, world::World};

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
    rt_handle: Option<EguiTextureHandle>,
    egui_bind_tex_recever: Option<tokio::sync::oneshot::Receiver<EguiTextureHandle>>,

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

    /// The orbit knobs this style maps to. The style DTO stays private to the
    /// window (toml keys unchanged); this is where it reaches the component.
    fn orbit_tuning(&self) -> OrbitTuning {
        OrbitTuning {
            orbit_speed: self.cam_default_orbit_speed,
            zoom_speed: self.cam_default_zoom_speed,
            fly_acce_duration: self.cam_default_fly_acce_duration,
            fly_min_speed: self.cam_default_fly_min_speed,
            fly_max_speed: self.cam_default_fly_max_speed,
            min_distance: self.cam_default_min_distance,
            max_distance: self.cam_default_max_distance,
        }
    }
}

impl SceneWindowModel {
    pub fn new(
        world: &kairos_ecs::world::World,
        assets_server: &mut AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let style = SceneWindowStyle::new()?;
        let gizmos = GizmosModel::new(world, assets_server);

        Ok(Self {
            style,
            rt_handle: None,
            egui_bind_tex_recever: None,
            gizmos,
        })
    }
}

impl SceneWindow {
    #[inline(always)]
    pub fn new(
        world: &kairos_ecs::world::World,
        assets_server: &mut AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let model = SceneWindowModel::new(world, assets_server)?;
        let gizmos_renderer = GizmosRenderer::new();
        Ok(Self {
            model,
            gizmos_renderer,
        })
    }
}

impl Drawer for SceneWindow {
    fn create(
        world: &kairos_ecs::world::World,
        assets_server: &mut AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Self::new(world, assets_server)
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
            let delta = response.drag_delta();
            if response.dragged_by(egui::PointerButton::Secondary)
                || response.dragged_by(egui::PointerButton::Middle)
            {
                messager.send(Message::SceneViewOrbit(-delta.x, -delta.y));
            }
            let scroll = ui.input(|i| i.smooth_scroll_delta);
            if scroll.y != 0.0 {
                messager.send(Message::SceneViewZoom(scroll.y));
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
                messager.send(Message::SceneViewFly(right, forward));
            }
        }

        // The view resource is the only place the size is stored; write back
        // only on a real change (the message handler owns the write).
        let size = engine.world.resource::<SceneView>().size;
        if size.width != width || size.height != height {
            messager.send(Message::UpdateSceneWindowSize(width, height));
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
        _game: &mut KairosGame,
        messager: &mut super::Messager,
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        let mut graphics_command = GraphicsCommand::new(16, 2, 4, 16);

        if self.model.egui_bind_tex_recever.is_some() {
            messager.send(Message::SceneWindowTryReceTextureId);
        }

        // Thin play layer: everything comes from the view resource, nothing is
        // queried out of the world. A closed window (`size == 0`) renders nothing
        // at all — no attachment, no stale frame.
        let view = engine.world.resource::<SceneView>();
        let size = view.size;
        if !size.is_renderable() {
            return None;
        }

        let scene_view = Attachment::new(
            Some("SceneWindow Attachment"),
            size.width,
            size.height,
            crate::graphics::attachment::AttachmentFormat::RGBA8UNorm,
        );
        let scene_depth_stencil = Attachment::new(
            Some("SceneWindow DepthStencil"),
            size.width,
            size.height,
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

        // The view projection lives on the bound camera entity, not on the view
        // resource. `None` means no camera is bound yet, or the extract stage
        // derived nothing for it this frame. `draws` is gated on size alone, so it
        // can be non-empty with no projection — drain it only when there is one,
        // and fall back to an identity VP for the clear-only pass.
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
            Some("SceneWindow Render Pass"),
            vec![scene_view_bind],
            Some(scene_depth_bind),
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
    // --- Editor camera entity (the window owns its style DTO, so it owns the
    // --- only code that can map that style onto the camera components) ---

    /// Spawns the editor camera entity on first open and binds it to the Scene
    /// view.
    ///
    /// Idempotent: if the view already has a camera (the tab was reopened), the
    /// existing entity is returned untouched. The entity is never despawned when
    /// the tab closes — it lives as long as the world — so reopening preserves
    /// the view.
    pub fn spawn_editor_camera(&self, world: &mut World) -> Entity {
        if let Some(existing) = world.resource::<SceneView>().camera {
            return existing;
        }

        let style = &self.model.style;
        let orbit = OrbitState::from_eye_pivot(style.cam_default_position, style.cam_default_target);
        let camera = Camera::new(
            style.cam_default_fov,
            style.cam_default_near,
            style.cam_default_far,
        );

        let entity = world
            .spawn((
                orbit.transform(),
                camera,
                EditorCameraController {
                    orbit,
                    tuning: style.orbit_tuning(),
                },
            ))
            .id();
        world.resource_mut::<SceneView>().camera = Some(entity);
        entity
    }

    /// Hot-updates the live editor camera from this window's style.
    ///
    /// Intrinsics and the seven orbit knobs take effect on the already-orbitable
    /// entity. The default position/target are deliberately **not** touched: they
    /// are the view's framing start point, and the user has since orbited
    /// somewhere — dragging a slider must not yank the view back.
    pub fn apply_camera_style(&self, world: &mut World) {
        let Some(entity) = world.resource::<SceneView>().camera else {
            return;
        };
        let style = &self.model.style;

        let mut entity_mut = world.entity_mut(entity);
        if let Some(mut camera) = entity_mut.get_mut::<Camera>() {
            camera.fov = style.cam_default_fov;
            camera.near = style.cam_default_near;
            camera.far = style.cam_default_far;
        }
        if let Some(mut controller) = entity_mut.get_mut::<EditorCameraController>() {
            controller.tuning = style.orbit_tuning();
        }
    }

    pub fn register_view_bind(
        &mut self,
        recever: tokio::sync::oneshot::Receiver<EguiTextureHandle>,
    ) {
        self.model.egui_bind_tex_recever = Some(recever);
        // self.try_rece_texture_id();
    }

    pub fn try_rece_texture_id(&mut self) {
        let received = {
            match &mut self.model.egui_bind_tex_recever {
                Some(recever) => match recever.try_recv() {
                    Ok(texuter_id) => {
                        self.model.rt_handle.replace(texuter_id);
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
