use std::{cell::Cell, fs, ops::DerefMut, path::PathBuf, sync::Arc};

use egui::{ComboBox, Vec2, Widget};
use egui_extras::{Column, TableBuilder};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, TextureAssetsSystem},
    graphics::{
        texture::Texture,
        texture::{
            format::{TextureCompressionConfig, TextureFormat},
            sampler::{AddressMode, FilterMode, MipmapFilter},
        },
    },
    kairos_editor::{
        editor_assets::{TextureExt, TextureExtAssetsSystem},
        ui::{
            Message, Messager,
            dialog::{ConfirmDialogWindow, Dialog},
            inspector::Inspector,
            paths,
        },
    },
    kairos_paths,
    kairos_settings::EngineSettings,
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
    /// so that the larger side is <= `max_size`.
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

        // Decode compressed data to RGBA8 for the egui preview.
        let rgba = crate::graphics::texture::format::decode_to_rgba8(
            &texture.data,
            texture.width,
            texture.height,
            texture.format,
        );

        let w = texture.width as usize;
        let h = texture.height as usize;
        let color_image = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
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

        let available_size = Vec2::new(ui.available_width(), self.model.style.preview_min_height);

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
            ui.image(egui::ImageSource::Texture(egui::load::SizedTexture::new(
                texture_id,
                display_size,
            )));
        });
    }

    // ============================================================
    // Public API -- called from Context::handle()
    // ============================================================

    pub fn apply(&mut self) {
        self.dirty.set(false);
        self.preview_texture.lock().take();
    }

    pub fn save_texture(
        assets_server: &mut AssetsServer,
        path: &PathBuf,
        handle: Arc<AssetHandle<TextureExtAssetsSystem>>,
        ext: Arc<Mutex<Option<TextureExt>>>,
    ) {
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
                }
                None => {
                    log::error!(
                        "Failed to reconstruct source image from cached data, texture_path: {:?}",
                        path
                    );
                    Vec::new()
                }
            }
        };

        // 2. Encode to target GPU format
        let encoded = crate::graphics::texture::format::encode_rgba(
            &rgba_data,
            new_w,
            new_h,
            ext.serialized.format,
        );

        // 3. Save to file
        match ext.serialized.save_to_file(&encoded) {
            Ok(_) => {}
            Err(err) => {
                log::error!(
                    "Failed to save texture, error: {}, texture_path: {:?}",
                    err,
                    path
                );
                return;
            }
        }

        // 4. Update in-memory asset
        let texture_asset = Texture {
            width: new_w,
            height: new_h,
            format: ext.serialized.format,
            data: encoded,
            sampler: ext.serialized.sampler.clone(),
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
        let handle = assets_server.load::<TextureExtAssetsSystem>(&texture_path);

        let compression_config = load_compression_config()?;

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
                            ui.label(format!("{} x {}", ext.original_width, ext.original_height));
                        });
                    });

                    // Current Size
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Current Size:");
                        });
                        row.col(|ui| {
                            ui.label(format!("{} x {}", texture.width, texture.height));
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
                                            .selectable_value(
                                                &mut selected_size,
                                                size,
                                                size.to_string(),
                                            )
                                            .changed()
                                        {
                                            (ext.serialized.width, ext.serialized.height) =
                                                Self::compute_target_size(
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
                                    for format in <TextureFormat as strum::IntoEnumIterator>::iter()
                                    {
                                        ui.add_enabled_ui(
                                            format.is_available(&self.model.compression_config),
                                            |ui| {
                                                if ui
                                                    .selectable_value(
                                                        &mut ext.serialized.format,
                                                        format,
                                                        format!("{format:?}"),
                                                        )
                                                        .changed()
                                                    {
                                                        // #2: auto-adjust sampler for non-filterable formats.
                                                        if !format.is_filterable() {
                                                            ext.serialized.sampler.filter_mode = FilterMode::Nearest;
                                                            if let Some(ref mut mip) = ext.serialized.sampler.mipmap {
                                                                mip.filter = MipmapFilter::Nearest;
                                                            }
                                                        }
                                                        self.dirty.set(true);
                                                    }
                                                },
                                        );
                                    }
                                });
                        });
                    });

                    // ---- Sampler settings ----

                    // Filter Mode
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Filter Mode:");
                        });
                        row.col(|ui| {
                            sampler_combo(
                                ui,
                                "texture_filter_mode",
                                &["Nearest", "Linear"],
                                ext.serialized.sampler.filter_mode as usize,
                                |idx| {
                                    ext.serialized.sampler.filter_mode =
                                        if idx == 0 { FilterMode::Nearest } else { FilterMode::Linear };
                                    self.dirty.set(true);
                                },
                            );
                        });
                    });

                    // Address Mode (with per-axis support)
                    draw_address_mode_rows(&mut body, row_h, &mut ext.serialized, &self.dirty);

                    // Mipmap toggle
                    body.row(row_h, |mut row| {
                        row.col(|ui| {
                            ui.label("Enable Mipmap:");
                        });
                        row.col(|ui| {
                            let mut enabled = ext.serialized.sampler.mipmap.is_some();
                            if ui.checkbox(&mut enabled, "").changed() {
                                if enabled {
                                    let max_dim = ext.serialized.width.max(ext.serialized.height);
                                    let max_level = (max_dim as f32).log2().floor();
                                    ext.serialized.sampler.mipmap = Some(
                                        crate::graphics::texture::sampler::MipmapConfig {
                                            filter: MipmapFilter::Linear,
                                            anisotropy_clamp: 2,
                                            lod_min_clamp: 0.0,
                                            lod_max_clamp: max_level,
                                        },
                                    );
                                } else {
                                    ext.serialized.sampler.mipmap = None;
                                }
                                self.dirty.set(true);
                            }
                        });
                    });

                    // Pre-compute LOD bounds before mutably borrowing sampler.
                    let max_dim = ext.serialized.width.max(ext.serialized.height);
                    let max_level = (max_dim as f32).log2().floor();

                    if let Some(ref mut mip) = ext.serialized.sampler.mipmap {
                        // Mipmap Filter
                        body.row(row_h, |mut row| {
                            row.col(|ui| {
                                ui.label("  Mipmap Filter:");
                            });
                            row.col(|ui| {
                                sampler_combo(
                                    ui,
                                    "texture_mipmap_filter",
                                    &["Nearest", "Linear"],
                                    mip.filter as usize,
                                    |idx| {
                                        mip.filter = if idx == 0 {
                                            MipmapFilter::Nearest
                                        } else {
                                            MipmapFilter::Linear
                                        };
                                        self.dirty.set(true);
                                    },
                                );
                            });
                        });

                        // Anisotropic
                        body.row(row_h, |mut row| {
                            row.col(|ui| {
                                ui.label("  Anisotropic:");
                            });
                            row.col(|ui| {
                                let mut aniso_on = mip.anisotropy_clamp > 1;
                                if ui.checkbox(&mut aniso_on, "").changed() {
                                    mip.anisotropy_clamp = if aniso_on { 4 } else { 1 };
                                    self.dirty.set(true);
                                }
                                if aniso_on {
                                    egui::ComboBox::from_id_salt("texture_anisotropy")
                                        .width(80.0)
                                        .selected_text(mip.anisotropy_clamp.to_string())
                                        .show_ui(ui, |ui| {
                                            for &level in &[2u16, 4, 8, 16] {
                                                if ui
                                                    .selectable_value(
                                                        &mut mip.anisotropy_clamp,
                                                        level,
                                                        level.to_string(),
                                                    )
                                                    .changed()
                                                {
                                                    self.dirty.set(true);
                                                }
                                            }
                                        });
                                }
                            });
                        });

                        // LOD Min
                        body.row(row_h, |mut row| {
                            row.col(|ui| {
                                ui.label("  LOD Min:");
                            });
                            row.col(|ui| {
                                let mut val = mip.lod_min_clamp;
                                if egui::Slider::new(&mut val, 0.0f32..=max_level)
                                    .text("lod_min")
                                    .ui(ui)
                                    .changed()
                                {
                                    mip.lod_min_clamp = val.min(mip.lod_max_clamp);
                                    self.dirty.set(true);
                                }
                            });
                        });

                        // LOD Max
                        body.row(row_h, |mut row| {
                            row.col(|ui| {
                                ui.label("  LOD Max:");
                            });
                            row.col(|ui| {
                                let mut val = mip.lod_max_clamp;
                                if egui::Slider::new(&mut val, 0.0f32..=max_level)
                                    .text("lod_max")
                                    .ui(ui)
                                    .changed()
                                {
                                    mip.lod_max_clamp = val.max(mip.lod_min_clamp);
                                    self.dirty.set(true);
                                }
                            });
                        });
                    }

                    // Compare
                    draw_compare_row(&mut body, row_h, &mut ext.serialized, &self.dirty);
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
                messager.send(Message::TextureInspectorApply(
                    self.model.texture_path.clone(),
                    self.model.handle.clone(),
                    self.model.texture_ext.clone(),
                ));
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

        // Read source_path from the ext_handle -- the handler resolves it.
        // We use a placeholder; the handler reads from TextureExt in asset system.
        let dialog = ConfirmDialogWindow::new(
            "Unsaved texture changes".into(),
            "Apply the changes before leaving?".into(),
            "Apply".into(),
            "Discard".into(),
            Some(Message::TextureInspectorApply(
                self.model.texture_path.clone(),
                self.model.handle.clone(),
                self.model.texture_ext.clone(),
            )),
            None,
            None::<fn()>,
            None::<fn()>,
        );
        Some(Box::new(dialog))
    }
}

/// Pick a sensible default max-size: the smallest SIZE_OPTIONS entry
/// that is >= the original's larger side.
fn find_size_level(width: u32, height: u32) -> u32 {
    let max_side = width.max(height);
    for &size in TextureInspector::SIZE_OPTIONS {
        if size >= max_side {
            return size;
        }
    }
    max_side
}

fn load_compression_config() -> Result<TextureCompressionConfig, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(kairos_paths::PATH_KAIROS_SETTINGS)?;
    let engine_settings = toml::from_slice::<EngineSettings>(&bytes)?;
    Ok(engine_settings.texture_compression)
}

// ============================================================
// Sampler UI helpers
// ============================================================

/// Simple two-option ComboBox (used for FilterMode, MipmapFilter).
fn sampler_combo(
    ui: &mut egui::Ui,
    id: &str,
    labels: &[&str],
    current: usize,
    mut on_change: impl FnMut(usize),
) {
    let mut selected = current;
    egui::ComboBox::from_id_salt(id)
        .width(120.0)
        .selected_text(labels[selected])
        .show_ui(ui, |ui| {
            for (i, label) in labels.iter().enumerate() {
                if ui.selectable_value(&mut selected, i, *label).changed() {
                    on_change(i);
                }
            }
        });
}

/// Draw the address-mode row: quick-select ComboBox + optional per-axis sub-rows.
fn draw_address_mode_rows(
    body: &mut egui_extras::TableBody,
    row_h: f32,
    serialized: &mut crate::graphics::texture::SerializedTexture,
    dirty: &Cell<bool>,
) {
    let s = &mut serialized.sampler;
    let modes_equal =
        s.address_mode_u == s.address_mode_v && s.address_mode_v == s.address_mode_w;

    const MODE_VALUES: &[AddressMode] = &[
        AddressMode::ClampToEdge,
        AddressMode::Repeat,
        AddressMode::MirrorRepeat,
        AddressMode::ClampToBorder,
    ];

    let current_label = if modes_equal {
        match s.address_mode_u {
            AddressMode::ClampToEdge => "ClampToEdge",
            AddressMode::Repeat => "Repeat",
            AddressMode::MirrorRepeat => "MirrorRepeat",
            AddressMode::ClampToBorder => "ClampToBorder",
        }
    } else {
        "Per Axis"
    };

    body.row(row_h, |mut row| {
        row.col(|ui| {
            ui.label("Address Mode:");
        });
        row.col(|ui| {
            egui::ComboBox::from_id_salt("texture_address_mode")
                .width(140.0)
                .selected_text(current_label)
                .show_ui(ui, |ui| {
                    for &mode in MODE_VALUES {
                        let label = match mode {
                            AddressMode::ClampToEdge => "ClampToEdge",
                            AddressMode::Repeat => "Repeat",
                            AddressMode::MirrorRepeat => "MirrorRepeat",
                            AddressMode::ClampToBorder => "ClampToBorder",
                        };
                        if ui
                            .selectable_label(s.address_mode_u == mode && modes_equal, label)
                            .clicked()
                        {
                            s.address_mode_u = mode;
                            s.address_mode_v = mode;
                            s.address_mode_w = mode;
                            dirty.set(true);
                        }
                    }
                    if ui.selectable_label(!modes_equal, "Per Axis").clicked() {
                        // Force per-axis mode by making the axes differ.
                        if modes_equal {
                            let alt = match s.address_mode_u {
                                AddressMode::Repeat => AddressMode::ClampToEdge,
                                _ => AddressMode::Repeat,
                            };
                            s.address_mode_v = alt;
                            dirty.set(true);
                        }
                    }
                });
        });
    });

    // Per-axis sub-rows
    if !modes_equal {
        for (label, ptr) in [
            ("  U:", &mut s.address_mode_u),
            ("  V:", &mut s.address_mode_v),
            ("  W:", &mut s.address_mode_w),
        ] {
            body.row(row_h, |mut row| {
                row.col(|ui| {
                    ui.label(label);
                });
                row.col(|ui| {
                    egui::ComboBox::from_id_salt(format!("addr_{}", label))
                        .width(140.0)
                        .selected_text(format!("{:?}", *ptr))
                        .show_ui(ui, |ui| {
                            for &mode in MODE_VALUES {
                                let mode_label = match mode {
                                    AddressMode::ClampToEdge => "ClampToEdge",
                                    AddressMode::Repeat => "Repeat",
                                    AddressMode::MirrorRepeat => "MirrorRepeat",
                                    AddressMode::ClampToBorder => "ClampToBorder",
                                };
                                if ui
                                    .selectable_label(*ptr == mode, mode_label)
                                    .clicked()
                                {
                                    *ptr = mode;
                                    dirty.set(true);
                                }
                            }
                        });
                });
            });
        }
    }

    // BorderColor -- only when any axis is ClampToBorder
    let needs_border = s.address_mode_u == AddressMode::ClampToBorder
        || s.address_mode_v == AddressMode::ClampToBorder
        || s.address_mode_w == AddressMode::ClampToBorder;

    if needs_border {
        const BORDER_LABELS: &[&str] =
            &["None", "TransparentBlack", "OpaqueBlack", "OpaqueWhite", "Zero"];
        const BORDER_VALUES: &[Option<wgpu::SamplerBorderColor>] = &[
            None,
            Some(wgpu::SamplerBorderColor::TransparentBlack),
            Some(wgpu::SamplerBorderColor::OpaqueBlack),
            Some(wgpu::SamplerBorderColor::OpaqueWhite),
            Some(wgpu::SamplerBorderColor::Zero),
        ];

        body.row(row_h, |mut row| {
            row.col(|ui| {
                ui.label("  Border Color:");
            });
            row.col(|ui| {
                let current_idx = BORDER_VALUES
                    .iter()
                    .position(|v| *v == s.border_color)
                    .unwrap_or(0);
                egui::ComboBox::from_id_salt("texture_border_color")
                    .width(140.0)
                    .selected_text(BORDER_LABELS[current_idx])
                    .show_ui(ui, |ui| {
                        for (i, label) in BORDER_LABELS.iter().enumerate() {
                            if ui.selectable_label(i == current_idx, *label).clicked() {
                                s.border_color = BORDER_VALUES[i];
                                dirty.set(true);
                            }
                        }
                    });
            });
        });
    }
}

/// Draw the CompareFunction row.
fn draw_compare_row(
    body: &mut egui_extras::TableBody,
    row_h: f32,
    serialized: &mut crate::graphics::texture::SerializedTexture,
    dirty: &Cell<bool>,
) {
    use wgpu::CompareFunction;
    const COMPARE_LABELS: &[&str] = &[
        "None", "Never", "Less", "Equal", "LessEqual", "Greater", "NotEqual", "GreaterEqual",
        "Always",
    ];
    const COMPARE_VALUES: &[Option<CompareFunction>] = &[
        None,
        Some(CompareFunction::Never),
        Some(CompareFunction::Less),
        Some(CompareFunction::Equal),
        Some(CompareFunction::LessEqual),
        Some(CompareFunction::Greater),
        Some(CompareFunction::NotEqual),
        Some(CompareFunction::GreaterEqual),
        Some(CompareFunction::Always),
    ];

    body.row(row_h, |mut row| {
        row.col(|ui| {
            ui.label("Compare:");
        });
        row.col(|ui| {
            let current_idx = COMPARE_VALUES
                .iter()
                .position(|v| *v == serialized.sampler.compare)
                .unwrap_or(0);
            egui::ComboBox::from_id_salt("texture_compare")
                .width(140.0)
                .selected_text(COMPARE_LABELS[current_idx])
                .show_ui(ui, |ui| {
                    for (i, label) in COMPARE_LABELS.iter().enumerate() {
                        if ui.selectable_label(i == current_idx, *label).clicked() {
                            serialized.sampler.compare = COMPARE_VALUES[i];
                            dirty.set(true);
                        }
                    }
                });
        });
    });
}
