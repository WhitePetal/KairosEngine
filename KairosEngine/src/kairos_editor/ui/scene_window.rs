use std::{any::type_name, fs};

use egui::pos2;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::{
    graphics::{
        attachment::Attachment, camera::Camera, graphics_graph::GraphicsCommand, mesh::Mesh,
        vertex::Vertex,
    },
    kairos_editor::ui::{Drawer, Message, paths},
    math::{self, float2, float3, float4, float4x4, quaternion},
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

fn node_transform_matrix(node: &gltf::Node<'_>) -> float4x4 {
    let (translation, rotation, scale) = node.transform().decomposed();

    float4x4::trs(
        float3::from(translation),
        quaternion::new(rotation[0], rotation[1], rotation[2], rotation[3]),
        float3::from(scale),
    )
}

fn load_mesh_from_primitive(
    primitive: gltf::Primitive<'_>,
    node_to_world: float4x4,
    buffers: &[gltf::buffer::Data],
) -> Option<Mesh> {
    if primitive.mode() != gltf::mesh::Mode::Triangles {
        return None;
    }

    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()].0));
    let positions = reader.read_positions()?;
    let vertex_count = positions.len();
    let mut colors = reader.read_colors(0).map(|colors| colors.into_rgba_f32());
    let mut texcoords = reader
        .read_tex_coords(0)
        .map(|texcoords| texcoords.into_f32());
    let mut normals = reader.read_normals();
    let mut tangents = reader.read_tangents();

    let mut vertices = Vec::with_capacity(vertex_count);
    for position in positions {
        let color = colors
            .as_mut()
            .and_then(|colors| colors.next())
            .map(float4::from)
            .unwrap_or(float4::new(1.0, 1.0, 1.0, 1.0));
        let texcoord = texcoords
            .as_mut()
            .and_then(|texcoords| texcoords.next())
            .map(float2::from_array)
            .unwrap_or(float2::new(0.0, 0.0));
        let normal = normals
            .as_mut()
            .and_then(|normals| normals.next())
            .map(float3::from)
            .unwrap_or(float3::new(0.0, 0.0, 1.0));
        let tangent = tangents
            .as_mut()
            .and_then(|tangents| tangents.next())
            .unwrap_or([1.0, 0.0, 0.0, 1.0]);

        let position = (node_to_world * float4::from((float3::from(position), 1.0))).xyz();
        let normal = math::normalize((node_to_world * float4::from((normal, 0.0))).xyz());
        let tangent_xyz = math::normalize(
            (node_to_world * float4::from((float3::new(tangent[0], tangent[1], tangent[2]), 0.0)))
                .xyz(),
        );

        vertices.push(Vertex {
            position: float4::from((position, 1.0)),
            color,
            texcoord,
            normal,
            tangent: float4::from((tangent_xyz, tangent[3])),
        });
    }

    let mut indices = reader
        .read_indices()
        .map(|indices| {
            indices
                .into_u32()
                .map(u16::try_from)
                .collect::<Result<Vec<_>, _>>()
                .ok()
        })
        .unwrap_or_else(|| {
            (0..vertices.len())
                .map(u16::try_from)
                .collect::<Result<Vec<_>, _>>()
                .ok()
        })?;
    
    // for triangle in indices.chunks_exact_mut(3) {
    //     triangle.swap(1, 2);
    // }

    Some(Mesh::new(0, vertices, indices))
}

fn load_mesh_from_node(
    node: gltf::Node<'_>,
    parent_to_world: float4x4,
    buffers: &[gltf::buffer::Data],
) -> Option<Mesh> {
    let node_to_world = parent_to_world * node_transform_matrix(&node);

    if let Some(gltf_mesh) = node.mesh() {
        for primitive in gltf_mesh.primitives() {
            if let Some(mesh) = load_mesh_from_primitive(primitive, node_to_world, buffers) {
                return Some(mesh);
            }
        }
    }

    for child in node.children() {
        if let Some(mesh) = load_mesh_from_node(child, node_to_world, buffers) {
            return Some(mesh);
        }
    }

    None
}

fn load_first_scene_mesh(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
) -> Option<Mesh> {
    for scene in document.scenes() {
        for node in scene.nodes() {
            if let Some(mesh) = load_mesh_from_node(node, float4x4::idenity(), buffers) {
                return Some(mesh);
            }
        }
    }

    None
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
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        let mut graphics_command = GraphicsCommand::new(16, 2, 4, 16);
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
        let scene_depth_stencil = Attachment::new(
            Some("SceneWindow DepthStencil"),
            width,
            height,
            crate::graphics::attachment::AttachmentFormat::D24S8,
        );
        let scene_view_id = graphics_command.create_color_attachment(scene_view);
        let scene_depth_id = graphics_command.create_depth_attachment(scene_depth_stencil);
        let cam_pos = float3::new(0.0, 1.0, -2.0);
        let cam_taget = float3::new(0.0, 0.0, 0.0);
        let cam_forward = math::normalize(cam_taget - cam_pos);
        let world_up = float3::new(0.0, 1.0, 0.0);
        let cam_right = math::cross(world_up, cam_forward) * -1.0;
        let camera = Camera::new(
            float3::new(0.0, 1.0, -2.0),
            cam_forward,
            cam_right,
            45.,
            width as f32 / height as f32,
            0.3,
            100.,
        );

        let (document, buffers, _images) = gltf::import("res/models/Suzanne.glb").unwrap();
        let mesh = load_first_scene_mesh(&document, &buffers);

        let Some(mesh) = mesh else {
            return None;
        };

        let vp_id =
            graphics_command.set_view_projection_matrix(camera.get_view_projection_matrix());
        graphics_command.begin_render_pass(
            Some("SceneWindow Render Pass"),
            vec![scene_view_id],
            Some(scene_depth_id),
            vp_id,
            4,
            true,
        );

        const NUM_INSTANCES_PER_ROW: i32 = 5;

        for z in -NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW {
            for x in -NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW {
                let position = float3::new(x as f32, 0.0, z as f32);
                let rotation = quaternion::identity();
                let scale = float3::new(1.0, 1.0, 1.0);

                let local_to_world = float4x4::trs(position, rotation, scale);
                graphics_command.draw(mesh.clone(), local_to_world);
            }
        }

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
