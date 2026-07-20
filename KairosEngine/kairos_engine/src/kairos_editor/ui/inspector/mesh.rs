use std::{fs, ops::DerefMut, path::PathBuf, sync::Arc};

use egui::Vec2;
use egui_extras::{Column, TableBuilder};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, MaterialAssetsSystem, MeshAssetsSystem},
    graphics::{
        attachment::{Attachment, AttachmentFormat, AttachmentLoadAction, AttachmentStoreAction},
        camera::Camera,
        graphics_graph::{
            GraphicsCommand,
            graphics_node::{ColorAttachmentBind, DepthAttachmentBind},
        },
    },
    graphics::mesh::Mesh,
    kairos_editor::ui::{
        Messager,
        dialog::Dialog,
        inspector::Inspector,
        paths,
    },
    math::{float3, float4x4},
    spatial::Transform,
};

// ============================================================
// Style
// ============================================================

#[derive(Debug, Serialize, Deserialize)]
struct MeshInspectorStyle {
    row_height: f32,
    preview_min_height: f32,
}

impl MeshInspectorStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = fs::read(paths::PATH_MESH_INSPECTOR_STYLE).map_err(|error| {
            format!("Load MeshInspector Style Failed, path: {}, error: {}", paths::PATH_MESH_INSPECTOR_STYLE, error)
        })?;
        let style: Self = toml::from_slice(&bytes).map_err(|error| {
            format!("Deserialize MeshInspector Style Failed, error: {}", error)
        })?;
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
    /// Horizontal orbit angle (radians).
    yaw: f32,
    /// Vertical orbit angle (radians), clamped to (-π/2, π/2).
    pitch: f32,
    /// Distance multiplier: 1.0 = auto-framing, <1 = closer, >1 = farther.
    zoom: f32,
}

impl PreviewState {
    const DEFAULT_SIZE: (u32, u32) = (512, 512);
    /// Nice 3/4 default view: ~40° yaw, ~29° pitch.
    const DEFAULT_YAW: f32 = 0.7;
    const DEFAULT_PITCH: f32 = 0.5;
    const DEFAULT_ZOOM: f32 = 1.0;

    const ORBIT_SPEED: f32 = 0.008;
    const ZOOM_SPEED: f32 = 0.01;
    const ZOOM_MIN: f32 = 0.1;
    const ZOOM_MAX: f32 = 10.0;

    fn new() -> Self {
        Self {
            size: Self::DEFAULT_SIZE,
            yaw: Self::DEFAULT_YAW,
            pitch: Self::DEFAULT_PITCH,
            zoom: Self::DEFAULT_ZOOM,
            egui_texture_id: None,
            bind_receiver: None,
            pending_drop_id: None,
        }
    }
}

// ============================================================
// Bounding box helper
// ============================================================

struct BoundingBox {
    min: float3,
    max: float3,
}

fn compute_bounding_box(mesh: &Mesh) -> BoundingBox {
    let mut min = float3::new(f32::MAX, f32::MAX, f32::MAX);
    let mut max = float3::new(f32::MIN, f32::MIN, f32::MIN);
    for v in &mesh.vertices {
        let p = v.position.xyz();
        min = float3::new(
            min.x().min(p.x()),
            min.y().min(p.y()),
            min.z().min(p.z()),
        );
        max = float3::new(
            max.x().max(p.x()),
            max.y().max(p.y()),
            max.z().max(p.z()),
        );
    }
    BoundingBox { min, max }
}

fn compute_preview_vp(
    bbox: &BoundingBox,
    width: u32,
    height: u32,
    yaw: f32,
    pitch: f32,
    zoom: f32,
) -> float4x4 {
    let center = (bbox.min + bbox.max) * 0.5;
    let size = bbox.max - bbox.min;
    let max_extent = size.x().max(size.y()).max(size.z()).max(0.001);
    let fov_deg: f32 = 60.0;
    let fov_rad = fov_deg.to_radians();
    let distance = max_extent / (2.0 * (fov_rad * 0.5).tan()) * 1.5 * zoom;

    let aspect = width as f32 / height as f32;
    let camera = Camera::new(fov_deg, aspect, 0.01, distance * 3.0);

    // Spherical → Cartesian: camera orbits around `center`.
    let cp = pitch.cos();
    let sp = pitch.sin();
    let cy = yaw.cos();
    let sy = yaw.sin();
    let eye = center + float3::new(cp * sy, sp, -cp * cy) * distance;
    let transform = Transform::look_at(eye, center, float3::UP);

    camera.get_view_projection_matrix(transform)
}

// ============================================================
// Model
// ============================================================

struct MeshInspectorModel {
    style: MeshInspectorStyle,
    mesh_path: PathBuf,
    handle: Arc<AssetHandle<MeshAssetsSystem>>,
    material: Arc<AssetHandle<MaterialAssetsSystem>>,
    mesh: Arc<Mutex<Option<Mesh>>>,
    preview: Mutex<PreviewState>,
}

// ============================================================
// Inspector
// ============================================================

pub struct MeshInspector {
    model: MeshInspectorModel,
}

/// Vertex attribute descriptor.
struct AttrInfo {
    name: &'static str,
    ty: &'static str,
    bytes: usize,
}

const ATTRIBUTES: &[AttrInfo] = &[
    AttrInfo { name: "position", ty: "float4", bytes: 16 },
    AttrInfo { name: "color",    ty: "float4", bytes: 16 },
    AttrInfo { name: "texcoord", ty: "float2", bytes: 8  },
    AttrInfo { name: "normal",   ty: "float3", bytes: 12 },
    AttrInfo { name: "tangent",  ty: "float4", bytes: 16 },
];

impl MeshInspector {
    fn draw_preview(&self, ui: &mut egui::Ui) {
        let mut guard = self.model.preview.lock();

        // Try to receive a new egui texture id from a completed bind.
        if let Some(receiver) = guard.bind_receiver.as_mut() {
            if let Ok(texture_id) = receiver.try_recv() {
                // Free the old texture id on the next render_preview cycle.
                if let Some(old) = guard.egui_texture_id.replace(texture_id) {
                    guard.pending_drop_id = Some(old);
                }
                guard.bind_receiver = None;
            }
        }

        let Some(tex_id) = guard.egui_texture_id else {
            ui.centered_and_justified(|ui| {
                ui.label("Preview loading...");
            });
            return;
        };

        // Take all remaining space (like SceneWindow), but enforce a minimum height.
        let available = ui.available_size_before_wrap();
        let min_h = self.model.style.preview_min_height;
        let size = Vec2::new(available.x, available.y.max(min_h));
        let (rect, response) =
            ui.allocate_exact_size(size, egui::Sense::click_and_drag());

        // Update attachment dimensions for the next render_preview cycle.
        let pixels_per_point = ui.pixels_per_point();
        let width = (rect.width() * pixels_per_point).round().max(1.0) as u32;
        let height = (rect.height() * pixels_per_point).round().max(1.0) as u32;
        guard.size = (width, height);

        // ---- Orbit: mouse drag ------
        // SceneWindow negates the delta before sending to orbit(); combined
        // with yaw -= (-dx) that gives yaw += dx. We follow the same convention.
        if response.dragged() {
            let delta = response.drag_delta();
            guard.yaw += delta.x * PreviewState::ORBIT_SPEED;
            guard.pitch += delta.y * PreviewState::ORBIT_SPEED;
            let limit = std::f32::consts::FRAC_PI_2 - 0.01;
            guard.pitch = guard.pitch.clamp(-limit, limit);
        }

        // ---- Zoom: scroll wheel ------
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
        if response.hovered() && scroll_delta.y != 0.0 {
            guard.zoom = (guard.zoom - scroll_delta.y * PreviewState::ZOOM_SPEED)
                .clamp(PreviewState::ZOOM_MIN, PreviewState::ZOOM_MAX);
        }

        // Draw the preview texture over the allocated rect.
        ui.painter().image(
            tex_id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
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
        let handle = assets_server.load::<MeshAssetsSystem>(&mesh_path);
        let material = assets_server
            .load::<MaterialAssetsSystem>(&PathBuf::from("res/materials/preview.mat"));

        let model = MeshInspectorModel {
            style,
            mesh_path,
            handle,
            material,
            mesh: Arc::new(Mutex::new(None)),
            preview: Mutex::new(PreviewState::new()),
        };

        Ok(Self { model })
    }

    fn draw(&self, ui: &mut egui::Ui, _messager: &mut Messager, assets_server: &AssetsServer) {
        // Wait for the mesh asset to load asynchronously.
        let mut mesh_guard = self.model.mesh.lock();
        let Some(mesh) = mesh_guard.deref_mut() else {
            if let Some(loaded) = assets_server.get(&self.model.handle) {
                *mesh_guard = Some(loaded.clone());
            }
            ui.label("Mesh is Loading...");
            return;
        };

        // ---- Source ----
        ui.label(format!("Source: {}", self.model.mesh_path.display()));

        // ---- Properties table ----
        let row_h = self.model.style.row_height;
        let total_bytes = mesh.vertices.len() * std::mem::size_of::<crate::graphics::vertex::Vertex>();

        TableBuilder::new(ui)
            .id_salt("mesh_properties")
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::remainder())
            .body(|mut body| {
                body.row(row_h, |mut row| {
                    row.col(|ui| { ui.label("Vertices:"); });
                    row.col(|ui| { ui.label(mesh.vertices.len().to_string()); });
                });
                body.row(row_h, |mut row| {
                    row.col(|ui| { ui.label("Indices:"); });
                    row.col(|ui| { ui.label(mesh.indices.len().to_string()); });
                });
                body.row(row_h, |mut row| {
                    row.col(|ui| { ui.label("Triangles:"); });
                    row.col(|ui| { ui.label((mesh.indices.len() / 3).to_string()); });
                });
                body.row(row_h, |mut row| {
                    row.col(|ui| { ui.label("Total Size:"); });
                    row.col(|ui| { ui.label(format!("{} bytes", total_bytes)); });
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
                header.col(|ui| { ui.label("Name"); });
                header.col(|ui| { ui.label("Type"); });
                header.col(|ui| { ui.label("Bytes"); });
            })
            .body(|mut body| {
                for attr in ATTRIBUTES {
                    body.row(row_h, |mut row| {
                        row.col(|ui| { ui.label(attr.name); });
                        row.col(|ui| { ui.label(attr.ty); });
                        row.col(|ui| { ui.label(attr.bytes.to_string()); });
                    });
                }
            });

        ui.separator();

        // ---- Preview ----
        self.draw_preview(ui);
    }

    fn render_preview(&self) -> Vec<GraphicsCommand> {
        let mut guard = self.model.preview.lock();

        // Free the old egui texture id if one was marked for drop.
        let drop_id = guard.pending_drop_id.take();

        // Only bind a new attachment if no receiver is pending.
        let bind_ready = guard.bind_receiver.is_none();
        let (egui_bind_sender, egui_bind_receiver) = if bind_ready {
            let (tx, rx) = tokio::sync::oneshot::channel();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        let (width, height) = guard.size;
        drop(guard);

        // Compute view-projection matrix from mesh bounding box and camera state.
        let (yaw, pitch, zoom) = {
            let g = self.model.preview.lock();
            (g.yaw, g.pitch, g.zoom)
        };
        let vp = self
            .model
            .mesh
            .lock()
            .as_ref()
            .map(|mesh| compute_preview_vp(&compute_bounding_box(mesh), width, height, yaw, pitch, zoom))
            .unwrap_or(float4x4::IDENTITY);

        let mut command = GraphicsCommand::new(3, 2, 1, 5);

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
            1,
        );

        // Draw the mesh (handle resolves asynchronously via the render pipeline).
        command.draw(
            self.model.handle.clone(),
            self.model.material.clone(),
            float4x4::IDENTITY,
        );

        command.end_render_pass();

        // Bind attachment to egui so it can be displayed in the inspector panel.
        if let (Some(sender), Some(receiver)) = (egui_bind_sender, egui_bind_receiver) {
            command.bind_attachment_to_egui(preview_attachment_id, sender);
            self.model.preview.lock().bind_receiver = Some(receiver);
        }

        vec![command]
    }

    fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
        None
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::vertex::Vertex;
    use crate::math::float4;

    fn make_mesh(vertices: Vec<[f32; 3]>) -> Mesh {
        Mesh {
            vertices: vertices
                .into_iter()
                .map(|p| Vertex {
                    position: float4::new(p[0], p[1], p[2], 1.0),
                    color: float4::new(1.0, 1.0, 1.0, 1.0),
                    texcoord: crate::math::float2::new(0.0, 0.0),
                    normal: crate::math::float3::new(0.0, 1.0, 0.0),
                    tangent: float4::new(1.0, 0.0, 0.0, 1.0),
                })
                .collect(),
            indices: vec![0, 1, 2],
        }
    }

    #[test]
    fn bounding_box_unit_cube() {
        let mesh = make_mesh(vec![
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [0.5, 0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [0.5, 0.5, 0.5],
        ]);
        let bbox = compute_bounding_box(&mesh);
        assert!((bbox.min.x() + 0.5).abs() < 0.001);
        assert!((bbox.min.y() + 0.5).abs() < 0.001);
        assert!((bbox.min.z() + 0.5).abs() < 0.001);
        assert!((bbox.max.x() - 0.5).abs() < 0.001);
        assert!((bbox.max.y() - 0.5).abs() < 0.001);
        assert!((bbox.max.z() - 0.5).abs() < 0.001);
    }

    #[test]
    fn preview_vp_unit_cube_fov_60() {
        let bbox = BoundingBox {
            min: float3::new(-0.5, -0.5, -0.5),
            max: float3::new(0.5, 0.5, 0.5),
        };
        let vp = compute_preview_vp(&bbox, 512, 512, 0.7, 0.5, 1.0);

        // The VP matrix should NOT be identity (camera positioned away from origin).
        assert_ne!(vp, float4x4::IDENTITY);

        // Project a point at the center — should be visible.
        let center = float4::new(0.0, 0.0, 0.0, 1.0);
        let projected = vp * center;
        let w = projected.w();
        assert!(w > 0.0, "Center of bounding box should be in front of camera, w={w}");
        let ndc_x = projected.x() / w;
        let ndc_y = projected.y() / w;
        assert!(ndc_x.abs() <= 1.1, "Center NDC x should be on screen, got {ndc_x}");
        assert!(ndc_y.abs() <= 1.1, "Center NDC y should be on screen, got {ndc_y}");
    }

    #[test]
    fn preview_vp_responds_to_yaw() {
        let bbox = BoundingBox {
            min: float3::new(-0.5, -0.5, -0.5),
            max: float3::new(0.5, 0.5, 0.5),
        };
        let vp0 = compute_preview_vp(&bbox, 512, 512, 0.0, 0.5, 1.0);
        let vp90 = compute_preview_vp(&bbox, 512, 512, std::f32::consts::FRAC_PI_2, 0.5, 1.0);

        // Different yaw should produce different VP matrices.
        assert_ne!(vp0, vp90);

        // Both should still keep center visible.
        let c = float4::new(0.0, 0.0, 0.0, 1.0);
        assert!((vp0 * c).w() > 0.0);
        assert!((vp90 * c).w() > 0.0);
    }

    #[test]
    fn preview_vp_camera_distance_scales_with_bbox() {
        let small = BoundingBox {
            min: float3::new(-0.5, -0.5, -0.5),
            max: float3::new(0.5, 0.5, 0.5),
        };
        let large = BoundingBox {
            min: float3::new(-5.0, -5.0, -5.0),
            max: float3::new(5.0, 5.0, 5.0),
        };
        let vp_small = compute_preview_vp(&small, 512, 512, 0.7, 0.5, 1.0);
        let vp_large = compute_preview_vp(&large, 512, 512, 0.7, 0.5, 1.0);

        // Project both centers — both should be visible.
        let c = float4::new(0.0, 0.0, 0.0, 1.0);
        let ps = vp_small * c;
        let pl = vp_large * c;
        assert!(ps.w() > 0.0);
        assert!(pl.w() > 0.0);

        // Both centers should map near NDC origin (since camera looks at center).
        assert!((ps.x() / ps.w()).abs() < 0.1);
        assert!((pl.x() / pl.w()).abs() < 0.1);
    }

    #[test]
    fn preview_vp_zoom_changes_distance() {
        let bbox = BoundingBox {
            min: float3::new(-0.5, -0.5, -0.5),
            max: float3::new(0.5, 0.5, 0.5),
        };
        // Zoom 2.0 → camera farther away → corner of bbox maps to smaller NDC.
        let vp_near = compute_preview_vp(&bbox, 512, 512, 0.7, 0.5, 0.5);
        let vp_far = compute_preview_vp(&bbox, 512, 512, 0.7, 0.5, 2.0);

        let corner = float4::new(0.5, 0.5, 0.5, 1.0);
        let p_near = vp_near * corner;
        let p_far = vp_far * corner;

        let ndc_near = (p_near.x() / p_near.w()).abs();
        let ndc_far = (p_far.x() / p_far.w()).abs();
        // With camera farther away (zoom in), the corner should be smaller on screen.
        assert!(ndc_far < ndc_near,
            "zoom=2.0 should push camera farther; near ndc={ndc_near}, far ndc={ndc_far}");
    }
}
