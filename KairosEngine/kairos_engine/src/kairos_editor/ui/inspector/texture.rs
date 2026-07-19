use std::{cell::Cell, fs, ops::DerefMut, path::PathBuf, sync::Arc};

use egui::{ComboBox, Vec2};
use egui_extras::{Column, TableBuilder};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, TextureAssetsSystem}, graphics::{
        texture::Texture,
        texture_format::{EngineSettings, TextureCompressionConfig, TextureFormat},
    }, kairos_editor::{
        editor_assets::{TextureExt, TextureExtAssetsSystem},
        texture_compression,
        ui::{
            Message, Messager,
            dialog::{ConfirmDialogWindow, Dialog},
            inspector::Inspector,
            paths,
        },
    },
};

// ============================================================
// Style
// ============================================================

#[derive(Debug, Serialize, Deserialize)]
struct TextureInspectorStyle {
    row_height: f32,
    apply_button_height: f32,
    preview_min_height: f32,
}

impl TextureInspectorStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = fs::read(paths::PATH_TEXTURE_INSPECTOR_STYLE).map_err(|error| {
            format!(
                "Load TextureInspector Style Failed, path: {}, error: {}",
                paths::PATH_TEXTURE_INSPECTOR_STYLE,
                error
            )
        })?;
        let style: Self = toml::from_slice(&bytes).map_err(|error| {
            format!(
                "Deserialize TextureInspector Style Failed, error: {}",
                error
            )
        })?;
        Ok(style)
    }
}

// ============================================================
// Model
// ============================================================

struct TextureInspectorModel {
    style: TextureInspectorStyle,
    /// Path to the `.texture` asset file (for Apply writes).
    texture_path: PathBuf,
    /// Handle to the editor runtime resource (loaded asynchronously).
    handle: Arc<AssetHandle<TextureExtAssetsSystem>>,
    texture_ext: Arc<Mutex<Option<TextureExt>>>,
    /// Compression feature flags from `Preferences/texture_compression.toml`.
    compression_config: TextureCompressionConfig,
}

// ============================================================
// Inspector
// ============================================================

pub struct TextureInspector {
    model: TextureInspectorModel,
    /// Cached egui texture handle for the preview panel.
    preview_texture: parking_lot::Mutex<Option<egui::TextureHandle>>,
    /// Whether the user has changed settings that haven't been applied.
    dirty: Cell<bool>,
}

impl TextureInspector {
    /// Available size options for the max-size ComboBox.
    const SIZE_OPTIONS: &[u32] = &[2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

    /// Compute the target (width, height) after proportional scaling
    /// so that the larger side is ≤ `max_size`.
    fn compute_target_size(orig_w: u32, orig_h: u32, max_size: u32) -> (u32, u32) {
        let max_side = orig_w.max(orig_h) as f32;
        if max_side <= max_size as f32 {
            return (orig_w, orig_h);
        }
        let scale = max_size as f32 / max_side;
        let new_w = (orig_w as f32 * scale).round() as u32;
        let new_h = (orig_h as f32 * scale).round() as u32;
        (new_w.max(1), new_h.max(1))
    }

    /// Draw the texture preview panel.
    /// Build and cache an egui texture from the current asset's RGBA8 data.
    fn ensure_preview(&self, ui: &mut egui::Ui, texture: &Texture) {
        {
            let guard = self.preview_texture.lock();
            if guard.is_some() {
                return;
            }
        }

        let w = texture.width as usize;
        let h = texture.height as usize;
        let expected_len = w * h * 4;

        if texture.data.len() != expected_len {
            return;
        }

        let color_image =
            egui::ColorImage::from_rgba_unmultiplied([w, h], &texture.data);
        let texture_handle = ui.ctx().load_texture(
            "texture_inspector_preview",
            color_image,
            egui::TextureOptions::LINEAR,
        );
        *self.preview_texture.lock() = Some(texture_handle);
    }

    /// Draw the texture preview panel.
    fn draw_preview(&self, ui: &mut egui::Ui) {
        let guard = self.preview_texture.lock();
        let Some(texture_id) = guard.as_ref().map(|h| h.id()) else {
            ui.label("Preview not available");
            return;
        };

        let available_size =
            Vec2::new(ui.available_width(), self.model.style.preview_min_height);

        let tex_mgr = ui.ctx().tex_manager();
        let tex_mgr = tex_mgr.read();
        let Some(meta) = tex_mgr.meta(texture_id) else {
            return;
        };
        let tex_w = meta.size[0] as f32;
        let tex_h = meta.size[1] as f32;

        let mut display_size = available_size;
        let aspect = tex_w / tex_h;
        if display_size.x / display_size.y > aspect {
            display_size.x = display_size.y * aspect;
        } else {
            display_size.y = display_size.x / aspect;
        }

        ui.centered_and_justified(|ui| {
            ui.image(egui::ImageSource::Texture(
                egui::load::SizedTexture::new(texture_id, display_size),
            ));
        });
    }

    // ============================================================
    // Public API — called from Context::handle()
    // ============================================================

    pub fn apply(&mut self) {
        self.dirty.set(false);
        self.preview_texture.lock().take();
    }

    pub fn save_texture(assets_server: &mut AssetsServer, path: &PathBuf, handle: Arc<AssetHandle<TextureExtAssetsSystem>>, ext: Arc<Mutex<Option<TextureExt>>>) {
        let mut ext_guard = ext.lock();
        let Some(ext) = ext_guard.deref_mut().take() else {
            return;
        };

        let (new_w, new_h) = (ext.serialized.width, ext.serialized.height);
        let (orig_w, orig_h) = (ext.original_width, ext.original_height);
        let original_rgba = ext.original_rgba.clone();

        // 1. Resize from cached original RGBA (always RGBA8 intermediate)
        let rgba_data = if new_w == orig_w && new_h == orig_h {
            original_rgba
        } else {
            match image::RgbaImage::from_raw(orig_w, orig_h, original_rgba) {
                Some(source_image) => {
                    let filtered = image::imageops::resize(
                        &source_image,
                        new_w,
                        new_h,
                        image::imageops::FilterType::Lanczos3,
                    );
                    filtered.into_vec()
                },
                None => {
                    log::error!("Failed to reconstruct source image from cached data, texture_path: {:?}", path);
                    Vec::new()
                },
            }
        };

        // 2. Encode to target GPU format
        let encoded = match texture_compression::encode_rgba(
            &rgba_data,
            new_w,
            new_h,
            ext.serialized.format,
        ) {
            Some(data) => data,
            None => {
                log::error!(
                    "Unsupported texture format for encoding: {:?}, texture_path: {:?}",
                    ext.serialized.format,
                    path
                );
                return;
            }
        };

        // 3. Save to file
        match ext.serialized.save_to_file(&encoded) {
            Ok(_) => {},
            Err(err) => {
                log::error!("Failed to save texture, error: {}, texture_path: {:?}", err, path);
                return;
            },
        }

        // 4. Update in-memory asset
        let texture_asset = Texture {
            width: new_w,
            height: new_h,
            format: ext.serialized.format,
            data: encoded,
        };
        if let Some(asset) = assets_server.get_mut(&ext.texture) {
            *asset = texture_asset;
        }
        if let Some(ext_source) = assets_server.get_mut(&handle) {
            *ext_source = ext
        }
    }
}

impl Inspector for TextureInspector {
    fn create(
        path: &std::path::Path,
        assets_server: &mut AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        let style = TextureInspectorStyle::new()?;
        let texture_path = path.to_path_buf();

        // Load the editor runtime resource asynchronously.
        // `TextureExtAssetsSystem` will auto-register on first use.
        let handle =
            assets_server.load::<TextureExtAssetsSystem>(&texture_path);

        let compression_config = load_compression_config();

        let model = TextureInspectorModel {
            style,
            texture_path,
            handle,
            texture_ext: Arc::new(Mutex::new(None)),
            compression_config,
        };

        Ok(Self {
            model,
            preview_texture: parking_lot::Mutex::new(None),
            dirty: Cell::new(false),
        })
    }

    fn draw(&self, ui: &mut egui::Ui, messager: &mut Messager, assets_server: &AssetsServer) {
        let texture;
        {
            // Wait for the TextureExt resource to load asynchronously.
            let mut ext_guard = self.model.texture_ext.lock();
            let Some(ext) = ext_guard.deref_mut() else {
                if let Some(ext_source) = assets_server.get(&self.model.handle) {
                    *ext_guard = Some(ext_source.clone());
                }
                ui.label("Texture is Loading...");
                return;
            };

            // Also wait for the runtime Texture (pixel data) to be ready.
            let Some(texture_inner) = assets_server.get::<TextureAssetsSystem>(&ext.texture) else {
                ui.label("Texture data is Loading...");
                return;
            };
            texture = texture_inner;

            // ---- Source ----
            ui.label(format!("Source: {}", ext.serialized.source_path.display()));

            // ---- Properties table ----
            let row_h = self.model.style.row_height;
            let original_max = ext.original_width.max(ext.original_height);
            let mut selected_size = find_size_level(ext.serialized.width, ext.serialized.height);

            TableBuilder::new(ui)
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::auto())
                .column(Column::remainder())
                .body(|mut body| {
                    // Original Size
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Original Size:");
                        });
                        row.col(|ui| {
                            ui.label(format!("{} × {}", ext.original_width, ext.original_height));
                        });
                    });

                    // Current Size
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Current Size:");
                        });
                        row.col(|ui| {
                            ui.label(format!("{} × {}", texture.width, texture.height));
                        });
                    });

                    // Max Size (ComboBox)
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Max Size:");
                        });
                        row.col(|ui| {
                            ComboBox::from_id_salt("texture_max_size")
                                .width(120.0)
                                .selected_text(selected_size.to_string())
                                .show_ui(ui, |ui| {
                                    for &size in Self::SIZE_OPTIONS {
                                        if size > original_max {
                                            continue;
                                        }
                                        if ui
                                            .selectable_value(&mut selected_size, size, size.to_string())
                                            .changed()
                                        {
                                            (ext.serialized.width, ext.serialized.height) = Self::compute_target_size(
                                                ext.original_width,
                                                ext.original_height,
                                                selected_size,
                                            );
                                            self.dirty.set(true);
                                        }
                                    }
                                });
                        });
                    });

                    // Texture Format
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Texture Format:");
                        });
                        row.col(|ui| {
                            ComboBox::from_id_salt("texture_format")
                                .width(180.0)
                                .selected_text(format!("{:?}", ext.serialized.format))
                                .show_ui(ui, |ui| {
                                    for format in <TextureFormat as strum::IntoEnumIterator>::iter() {
                                        ui.add_enabled_ui(format.is_available(&self.model.compression_config), |ui| {
                                            if ui
                                                .selectable_value(
                                                    &mut ext.serialized.format,
                                                    format,
                                                    format!("{format:?}"),
                                                )
                                                .changed()
                                            {
                                                self.dirty.set(true);
                                            }
                                        });
                                    }
                                });
                        });
                    });
                });
        }

        // ---- Apply button ----
        let changed = self.dirty.get();
        ui.vertical_centered(|ui| {
            let apply_btn = egui::Button::new("Apply").min_size(Vec2::new(
                ui.available_width(),
                self.model.style.apply_button_height,
            ));

            if ui.add_enabled(changed, apply_btn).clicked() {
                messager.send(Message::TextureInspectorApply(self.model.texture_path.clone(), self.model.handle.clone(), self.model.texture_ext.clone()));
            }
            if changed {
                ui.label("* unsaved changes");
            }
        });

        ui.separator();

        // ---- Preview panel ----
        self.ensure_preview(ui, texture);
        self.draw_preview(ui);
    }

    fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
        if !self.dirty.get() {
            return None;
        }

        // Read source_path from the ext_handle — the handler resolves it.
        // We use a placeholder; the handler reads from TextureExt in asset system.
        let dialog = ConfirmDialogWindow::new(
            "Unsaved texture changes".into(),
            "Apply the changes before leaving?".into(),
            "Apply".into(),
            "Discard".into(),
            Some(Message::TextureInspectorApply(self.model.texture_path.clone(), self.model.handle.clone(), self.model.texture_ext.clone())),
            None,
            None::<fn()>,
            None::<fn()>,
        );
        Some(Box::new(dialog))
    }
}

/// Pick a sensible default max-size: the smallest SIZE_OPTIONS entry
/// that is ≥ the original's larger side.
fn find_size_level(width: u32, height: u32) -> u32 {
    let max_side = width.max(height);
    for &size in TextureInspector::SIZE_OPTIONS {
        if size >= max_side {
            return size;
        }
    }
    max_side
}

fn load_compression_config() -> TextureCompressionConfig {
    let path = "Preferences/Engine/engine.toml";
    match std::fs::read_to_string(path).ok().and_then(|s| toml::from_str::<EngineSettings>(&s).ok()) {
        Some(settings) => settings.texture_compression,
        None => TextureCompressionConfig::default(),
    }
}
