use std::{
    cell::Cell,
    fs,
    ops::DerefMut,
    path::PathBuf,
    sync::Arc,
};

use strum::{Display, EnumIter};

use egui::Vec2;
use egui_extras::{Column, TableBuilder};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, MaterialAssetsSystem, MeshAssetsSystem},
    graphics::{
        attachment::{Attachment, AttachmentFormat, AttachmentLoadAction, AttachmentStoreAction},
        graphics_graph::{
            GraphicsCommand,
            graphics_node::{ColorAttachmentBind, DepthAttachmentBind},
        },
        mesh::{Mesh, wireframe},
    },
    kairos_editor::ui::{
        Message, Messager, dialog::Dialog, inspector::Inspector, paths, scene_camera::SceneCamera
    },
    math::{Vector, float2, float3, float4, float4x4},
    spatial::AABB,
};

// ============================================================
// Preview mode
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Display, EnumIter)]
#[repr(usize)]
enum PreviewMode {
    Shaded,
    Normal,
    Tangent,
    VertexColor,
}

// ============================================================
// Style
// ============================================================

#[derive(Debug, Serialize, Deserialize)]
struct MeshInspectorStyle {
    row_height: f32,
    preview_min_height: f32,
    preview_default_size: u32,
    camera_fov: f32,
    camera_direction: float3,
    camera_orbit_speed: f32,
    camera_zoom_speed: f32,
    camera_min_distance: f32,
    camera_max_distance: f32,
}

impl MeshInspectorStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = fs::read(paths::PATH_MESH_INSPECTOR_STYLE).map_err(|error| {
            format!(
                "Load MeshInspector Style Failed, path: {}, error: {}",
                paths::PATH_MESH_INSPECTOR_STYLE,
                error
            )
        })?;
        let mut style = toml::from_slice::<Self>(&bytes)
            .map_err(|error| format!("Deserialize MeshInspector Style Failed, error: {}", error))?;
        style.preview_min_height = style.preview_min_height.max(1.0);
        Ok(style)
    }
}

// ============================================================
// Preview render state
// ============================================================

/// Tracks the egui texture handle, async bind channel, and camera rotation/zoom
/// for the 3D preview panel.
struct PreviewState {
    egui_texture_id: Option<egui::TextureId>,
    bind_receiver: Option<tokio::sync::oneshot::Receiver<egui::TextureId>>,
    pending_drop_id: Option<egui::TextureId>,
    size: (u32, u32),
    camera: SceneCamera,
}

impl PreviewState {
    fn new(aabb: AABB, style: &MeshInspectorStyle) -> Self {
        let center = (aabb.min + aabb.max) * 0.5;
        let size = aabb.max - aabb.min;
        let max_extent = size.x().max(size.y()).max(size.z()).max(0.001);
        let fov = style.camera_fov;
        let fov_rad = fov.to_radians();
        let distance = max_extent / (2.0 * (fov_rad * 0.5).tan()) * 1.5;
        let direction = style.camera_direction.normalize();
        let eye = center - direction * distance;

        let camera = SceneCamera::new(
            eye,
            center,
            style.camera_fov,
            0.03,
            3000.0,
            style.camera_orbit_speed,
            style.camera_zoom_speed,
            0.0,
            0.0,
            0.0,
            style.camera_min_distance,
            style.camera_max_distance,
        );

        let size = style.preview_default_size.max(1);
        Self {
            size: (size, size),
            egui_texture_id: None,
            bind_receiver: None,
            pending_drop_id: None,
            camera,
        }
    }
}
// ============================================================
// Model
// ============================================================

struct MeshInspectorModel {
    mesh_path: PathBuf,
    wireframe_mesh_path: PathBuf,
    style: MeshInspectorStyle,
    mesh_handle: Arc<AssetHandle<MeshAssetsSystem>>,
    wireframe_material_handle: Arc<AssetHandle<MaterialAssetsSystem>>,
    wireframe_mesh_handle: Option<Arc<AssetHandle<MeshAssetsSystem>>>,
    mode_material_handles: [Arc<AssetHandle<MaterialAssetsSystem>>; 4],
    preview_mode: Cell<PreviewMode>,
    show_wireframe: Cell<bool>,
    preview: Mutex<Option<PreviewState>>,
}

// ============================================================
// Inspector
// ============================================================

pub struct MeshInspector {
    model: MeshInspectorModel,
}

impl MeshInspector {
    pub fn create_wireframe_mesh(&mut self, assets_server: &mut AssetsServer, mesh: Mesh) {
        self.model.wireframe_mesh_handle = Some(assets_server.insert::<MeshAssetsSystem>(mesh, &self.model.wireframe_mesh_path));
    }

    fn draw_preview(&self, ui: &mut egui::Ui, mesh: &Mesh, dt: f32) {
        let mut guard = self.model.preview.lock();
        let preview = guard.get_or_insert(PreviewState::new(mesh.compute_aabb(), &self.model.style));

        // Try to receive a new egui texture id from a completed bind.
        if let Some(receiver) = &mut preview.bind_receiver {
            if let Ok(texture_id) = receiver.try_recv() {
                // Free the old texture id on the next render_preview cycle.
                if let Some(old) = preview.egui_texture_id.replace(texture_id) {
                    preview.pending_drop_id = Some(old);
                }
                preview.bind_receiver = None;
            }
        }

        let Some(tex_id) = preview.egui_texture_id else {
            ui.centered_and_justified(|ui| {
                ui.label("Preview is loading...");
            });
            return;
        };

        // ---- Controls toolbar (flush top) ----
        self.draw_preview_toolbar(ui);

        // ---- 3D Preview image (remaining space) ----
        let available = ui.available_size_before_wrap();
        let min_h = self.model.style.preview_min_height;
        let size = Vec2::new(available.x, available.y.max(min_h));
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());

        // Update attachment dimensions for the next render_preview cycle.
        let pixels_per_point = ui.pixels_per_point();
        let width = (rect.width() * pixels_per_point).round().max(1.0) as u32;
        let height = (rect.height() * pixels_per_point).round().max(1.0) as u32;
        preview.size = (width, height);
        preview.camera.aspect = width as f32 / height as f32;

        // ---- Orbit: mouse drag ------
        if response.dragged() {
            let delta = -response.drag_delta();
            preview.camera.orbit(delta.x, delta.y, dt);
        }

        // ---- Zoom: scroll wheel ------
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
        if response.hovered() && scroll_delta.y != 0.0 {
            preview.camera.zoom(scroll_delta.y, dt);
        }

        // Draw the preview texture over the allocated rect.
        ui.painter().image(
            tex_id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }

    fn draw_preview_toolbar(&self, ui: &mut egui::Ui) {
        use strum::IntoEnumIterator;

        let current_mode = self.model.preview_mode.get();
        let current_wireframe = self.model.show_wireframe.get();

        let mut mode = current_mode;
        let mut wireframe = current_wireframe;

        ui.horizontal(|ui| {
            // Preview mode combo.
            egui::ComboBox::from_id_salt("preview_mode")
                .selected_text(current_mode.to_string())
                .width(120.0)
                .show_ui(ui, |ui| {
                    ui.style_mut().visuals.button_frame = true;
                    for variant in PreviewMode::iter() {
                        if ui
                            .selectable_value(&mut mode, variant, variant.to_string())
                            .changed()
                        {
                            self.model.preview_mode.set(mode);
                        }
                    }
                });

            ui.separator();

            // Wireframe toggle.
            if ui.checkbox(&mut wireframe, "Wireframe").changed() {
                self.model.show_wireframe.set(wireframe);
            }
        });
    }
}

impl Inspector for MeshInspector {
    fn create(
        path: &std::path::Path,
        assets_server: &mut AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        let style = MeshInspectorStyle::new()?;
        let mesh_path = path.to_path_buf();
        let mesh_handle = assets_server.load::<MeshAssetsSystem>(&mesh_path);
        let wireframe_material_handle =
            assets_server.load::<MaterialAssetsSystem>(&PathBuf::from(
                paths::PATH_MESH_INSPECTOR_PREVIEW_WIREFRAME_MATERIAL,
            ));

        let wireframe_mesh_path = mesh_path.with_added_extension(".wireframe_mesh");

        let mode_material_handles = [
            assets_server.load::<MaterialAssetsSystem>(&PathBuf::from(
                paths::PATH_MESH_INSPECTOR_PREVIEW_SHADED_MATERIAL,
            )),
            assets_server.load::<MaterialAssetsSystem>(&PathBuf::from(
                paths::PATH_MESH_INSPECTOR_PREVIEW_NORMAL_MATERIAL,
            )),
            assets_server.load::<MaterialAssetsSystem>(&PathBuf::from(
                paths::PATH_MESH_INSPECTOR_PREVIEW_TANGENT_MATERIAL,
            )),
            assets_server.load::<MaterialAssetsSystem>(&PathBuf::from(
                paths::PATH_MESH_INSPECTOR_PREVIEW_VERTEX_COLOR_MATERIAL,
            )),
        ];

        let model = MeshInspectorModel {
            style,
            mesh_path,
            wireframe_mesh_path,
            mesh_handle,
            wireframe_material_handle,
            wireframe_mesh_handle: None,
            mode_material_handles,
            preview_mode: Cell::new(PreviewMode::Shaded),
            show_wireframe: Cell::new(true),
            preview: Mutex::new(None),
        };

        Ok(Self { model })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        messager: &mut Messager,
        assets_server: &AssetsServer,
        dt: f32,
    ) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            let Some(mesh) = assets_server.get(&self.model.mesh_handle) else {
                ui.label("Mesh is Loading...");
                return;
            };
            if self.model.wireframe_mesh_handle.is_none() {
                messager.send(Message::ModelInspectorCreateWireframeMesh(wireframe::create_wireframe_mesh(mesh)));
            }

            // ---- Source ----
            ui.label(format!("Source: {}", self.model.mesh_path.display()));
            // ---- Properties table ----
            let row_h = self.model.style.row_height;
            let total_bytes =
                mesh.vertices.len() * std::mem::size_of::<crate::graphics::vertex::Vertex>();

            TableBuilder::new(ui)
                .id_salt("mesh_properties")
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::auto())
                .column(Column::remainder())
                .body(|mut body| {
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Vertices:");
                        });
                        row.col(|ui| {
                            ui.label(mesh.vertices.len().to_string());
                        });
                    });
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Indices:");
                        });
                        row.col(|ui| {
                            ui.label(mesh.indices.len().to_string());
                        });
                    });
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Triangles:");
                        });
                        row.col(|ui| {
                            ui.label((mesh.indices.len() / 3).to_string());
                        });
                    });
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Total Size:");
                        });
                        row.col(|ui| {
                            ui.label(format!("{} bytes", total_bytes));
                        });
                    });
                });

            ui.separator();

            // ---- Attribute table ----
            ui.label("Vertex Attributes:");
            TableBuilder::new(ui)
                .id_salt("mesh_attributes")
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::remainder())
                .header(row_h, |mut header| {
                    header.col(|ui| {
                        ui.label("Name");
                    });
                    header.col(|ui| {
                        ui.label("Type");
                    });
                    header.col(|ui| {
                        ui.label("Bytes");
                    });
                })
                .body(|mut body| {
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Position");
                        });
                        row.col(|ui| {
                            ui.label("float4");
                        });
                        row.col(|ui| {
                            ui.label(std::mem::size_of::<float4>().to_string());
                        });
                    });
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Color");
                        });
                        row.col(|ui| {
                            ui.label("float4");
                        });
                        row.col(|ui| {
                            ui.label(std::mem::size_of::<float4>().to_string());
                        });
                    });
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Texcoord");
                        });
                        row.col(|ui| {
                            ui.label("float2");
                        });
                        row.col(|ui| {
                            ui.label(std::mem::size_of::<float2>().to_string());
                        });
                    });
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Normal");
                        });
                        row.col(|ui| {
                            ui.label("float3");
                        });
                        row.col(|ui| {
                            ui.label(std::mem::size_of::<float3>().to_string());
                        });
                    });
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Tangent");
                        });
                        row.col(|ui| {
                            ui.label("float4");
                        });
                        row.col(|ui| {
                            ui.label(std::mem::size_of::<float4>().to_string());
                        });
                    });
                });

            ui.separator();

            // ---- Preview ----
            self.draw_preview(ui, mesh, dt);
        });
    }

    fn render(&self) -> Option<GraphicsCommand> {
        let mut guard = self.model.preview.lock();
        let Some(preview) = guard.deref_mut() else {
            return None;
        };

        // Free the old egui texture id if one was marked for drop.
        let drop_id = preview.pending_drop_id.take();

        // Only bind a new attachment if no receiver is pending.
        let bind_ready = preview.bind_receiver.is_none();
        let (egui_bind_sender, egui_bind_receiver) = if bind_ready {
            let (tx, rx) = tokio::sync::oneshot::channel();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        let (width, height) = preview.size;

        let vp = preview.camera.view_projection();

        let mut command = GraphicsCommand::new(3, 2, 1, 6);

        // Free old texture.
        if let Some(id) = drop_id {
            command.free_egui_texture_id(id);
        }

        // Create preview color attachment.
        let preview_attachment = Attachment::new(
            Some("MeshPreview Color"),
            width,
            height,
            AttachmentFormat::RGBA8UNorm,
        );
        let preview_attachment_id = command.create_color_attachment(preview_attachment);
        let preview_bind = ColorAttachmentBind::new(
            preview_attachment_id,
            AttachmentLoadAction::LoadClear,
            AttachmentStoreAction::Store,
        );

        // Create preview depth attachment.
        let preview_depth = Attachment::new(
            Some("MeshPreview Depth"),
            width,
            height,
            AttachmentFormat::D24S8,
        );
        let preview_depth_id = command.create_depth_attachment(preview_depth);
        let preview_depth_bind = DepthAttachmentBind::new(
            preview_depth_id,
            Some((
                AttachmentLoadAction::LoadClear,
                AttachmentStoreAction::Store,
            )),
            None,
        );

        let vp_id = command.set_view_projection_matrix(vp);
        command.begin_render_pass(
            Some("MeshPreview Render Pass"),
            vec![preview_bind],
            Some(preview_depth_bind),
            vp_id,
            2,
        );

        // Select the material for the current preview mode.
        let mode_material = {
            let idx = self.model.preview_mode.get() as usize;
            self.model.mode_material_handles[idx].clone()
        };

        // Draw solid preview mesh with the selected mode material.
        command.draw(
            self.model.mesh_handle.clone(),
            mode_material,
            float4x4::IDENTITY,
        );

        // Draw wireframe overlay on top (line-list, no depth write).
        if self.model.show_wireframe.get() {
            if let Some(wireframe_mesh_handle) = self.model.wireframe_mesh_handle.clone() {
                command.draw(
                    wireframe_mesh_handle,
                    self.model.wireframe_material_handle.clone(),
                    float4x4::IDENTITY,
                );
            }
        }

        command.end_render_pass();

        // Bind attachment to egui so it can be displayed in the inspector panel.
        if let (Some(sender), Some(receiver)) = (egui_bind_sender, egui_bind_receiver) {
            command.bind_attachment_to_egui(preview_attachment_id, sender);
            preview.bind_receiver = Some(receiver);
        }

        Some(command)
    }

    fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
        None
    }
}
