mod edit;

use std::{
    cell::Cell,
    fs,
    sync::Arc,
};

use egui::{ComboBox, Vec2, Widget};
use egui_extras::{Column, TableBuilder};
use kairos_tasks::{IoTaskPool, TaskPool};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use edit::{load_texture_edit, write_settings_meta};
pub use edit::TextureEdit;

use crate::{
    graphics::{
        compare_function::CompareFunction,
        texture::{
            Texture, TextureMaxSize, TextureSettings, find_texture_max_size,
            format::{TextureCompressionConfig, TextureFormat},
            sampler::{AddressMode, AnisotropyLevel, BorderColor, FilterMode, MipmapFilter},
        },
    },
    kairos_editor::ui::{
        Message, Messager, UIReader,
        dialog::{ConfirmDialogWindow, Dialog},
        inspector::Inspector,
        paths,
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
    combo_width_default: f32,
    combo_width_narrow: f32,
    combo_width_anisotropy: f32,
    combo_width_format: f32,
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
    /// The loaded edit, filled by the background task started in `create`.
    edit: Arc<Mutex<Option<TextureEdit>>>,
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
    /// Whether the address-mode UI is in per-axis editing mode.
    per_axis_mode: Cell<bool>,
}

impl TextureInspector {
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

        // Decode to RGBA bytes for the egui preview (converts F16/F32 if needed).
        let rgba_bytes = crate::graphics::texture::format::decode(
            &texture.data[0],
            texture.width,
            texture.height,
            texture.format,
        )
        .convert_to_u8_bytes();

        let w = texture.width as usize;
        let h = texture.height as usize;
        let color_image = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba_bytes);
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

    pub fn save_texture(edit: &Arc<Mutex<Option<TextureEdit>>>) {
        let mut guard = edit.lock();
        let Some(edit) = guard.as_mut() else {
            return;
        };

        let (new_w, new_h) = (edit.settings.width, edit.settings.height);
        let (orig_w, orig_h) = (edit.original_width, edit.original_height);
        let original_rgba = edit.original_rgba.clone();

        // 1. Resize from cached original RGBA
        let rgba_data = if new_w == orig_w && new_h == orig_h {
            original_rgba.clone()
        } else {
            match image::RgbaImage::from_raw(orig_w, orig_h, original_rgba.clone()) {
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
                        "Failed to reconstruct source image from cached data, source_path: {:?}",
                        edit.source_path
                    );
                    Vec::new()
                }
            }
        };

        // 2. Encode base level + generate cascading mip-chain.
        let mip_data: Vec<crate::graphics::texture::PixelDatas> =
            if let Some(ref mip) = edit.settings.sampler.mipmap {
                let max_possible = (new_w.max(new_h) as f32).log2().floor() as u32;
                let end_level = (mip.lod_max_clamp.floor() as u32).min(max_possible);
                let total_levels = end_level + 1;
                let (block_w, block_h) = edit.settings.format.block_dimensions();
                let mut levels = Vec::with_capacity(total_levels as usize);
                let mut current_w = new_w;
                let mut current_h = new_h;
                let mut current_rgba = rgba_data;

                for _ in 0..total_levels {
                    if current_w < block_w || current_h < block_h {
                        break;
                    }
                    let pixels = crate::graphics::texture::PixelDatas::U8(current_rgba.clone());
                    let encoded = crate::graphics::texture::format::encode(
                        &pixels,
                        current_w,
                        current_h,
                        edit.settings.format,
                    );
                    levels.push(encoded);
                    let (pw, ph) = (current_w, current_h);
                    current_w = (current_w / 2).max(1);
                    current_h = (current_h / 2).max(1);
                    if let Some(source) = image::RgbaImage::from_raw(pw, ph, current_rgba) {
                        current_rgba = image::imageops::resize(
                            &source,
                            current_w,
                            current_h,
                            image::imageops::FilterType::Lanczos3,
                        )
                        .into_vec();
                    } else {
                        break;
                    }
                }
                levels
            } else {
                let pixels = crate::graphics::texture::PixelDatas::U8(rgba_data);
                let encoded = crate::graphics::texture::format::encode(
                    &pixels,
                    new_w,
                    new_h,
                    edit.settings.format,
                );
                vec![encoded]
            };

        // 3. Persist the processor settings to the source's `.meta` rather than
        //    the product: the processor — the only writer of the product —
        //    regenerates the processed bytes from them.
        if let Err(err) = write_settings_meta(&edit.source_path, &edit.settings) {
            log::error!(
                "Failed to write texture `.meta`, error: {}, source_path: {:?}",
                err,
                edit.source_path
            );
            return;
        }

        // 4. Update the in-memory preview the inspector shows.
        edit.texture = Texture {
            width: new_w,
            height: new_h,
            format: edit.settings.format,
            data: mip_data,
            sampler: edit.settings.sampler.clone(),
        };
    }
}

impl Inspector for TextureInspector {
    fn create(
        path: &std::path::Path,
        _world: &kairos_ecs::world::World,
        project_graph: &crate::kairos_editor::project_path_tree::ProjectPathGraph,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        let style = TextureInspectorStyle::new()?;

        // The node's engine path is the processed product; the composite needs
        // the paired source image and its `.meta`. Fail rather than fall back to
        // the product path, whose `.meta` is a `Load` sidecar the editor must
        // never rewrite.
        let source_path = project_graph.source_path_for(path).ok_or_else(|| {
            format!(
                "texture inspector: '{}' is not paired with a source node",
                path.display()
            )
        })?;
        // Read the source image and its `.meta` and build the preview off the UI
        // thread; the model polls the slot the task fills.
        let edit = Arc::new(Mutex::new(None));
        let task_edit = edit.clone();
        IoTaskPool::get_or_init(TaskPool::default)
            .spawn(async move {
                *task_edit.lock() = Some(load_texture_edit(source_path).await);
            })
            .detach();

        let compression_config = load_compression_config()?;

        let model = TextureInspectorModel {
            style,
            edit,
            compression_config,
        };

        Ok(Self {
            model,
            preview_texture: parking_lot::Mutex::new(None),
            dirty: Cell::new(false),
            per_axis_mode: Cell::new(false),
        })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        _reader: &UIReader,
        messager: &mut Messager,
        _world: &kairos_ecs::world::World,
        _dt: f32,
    ) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            {
                // The edit is built by the background task `create` started; poll it.
                let mut edit_guard = self.model.edit.lock();
                let Some(edit) = edit_guard.as_mut() else {
                    ui.label("Texture is Loading...");
                    return;
                };

                // ---- Source ----
                ui.label(format!("Source: {}", edit.source_path.display()));

                // ---- Properties table ----
                let row_h = self.model.style.row_height;
                let w_default = self.model.style.combo_width_default;
                let w_narrow = self.model.style.combo_width_narrow;
                let w_aniso = self.model.style.combo_width_anisotropy;
                let w_format = self.model.style.combo_width_format;
                let original_max = edit.original_width.max(edit.original_height);
                let mut selected_size =
                    find_texture_max_size(edit.settings.width, edit.settings.height);

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
                                ui.label(format!(
                                    "{} x {}",
                                    edit.original_width, edit.original_height
                                ));
                            });
                        });

                        // Current Size
                        body.row(row_h, |mut row| {
                            row.col(|ui| {
                                ui.label("Current Size:");
                            });
                            row.col(|ui| {
                                ui.label(format!("{} x {}", edit.texture.width, edit.texture.height));
                            });
                        });

                        // Max Size (ComboBox)
                        body.row(row_h, |mut row| {
                            row.col(|ui| {
                                ui.label("Max Size:");
                            });
                            row.col(|ui| {
                                ComboBox::from_id_salt("texture_max_size")
                                    .width(w_narrow)
                                    .selected_text(selected_size.as_u32().to_string())
                                    .show_ui(ui, |ui| {
                                        for size in TextureMaxSize::iter() {
                                            if size.as_u32() > original_max {
                                                continue;
                                            }
                                            if ui
                                                .selectable_value(
                                                    &mut selected_size,
                                                    size,
                                                    size.as_u32().to_string(),
                                                )
                                                .changed()
                                            {
                                                (edit.settings.width, edit.settings.height) =
                                                    Self::compute_target_size(
                                                        edit.original_width,
                                                        edit.original_height,
                                                        selected_size.as_u32(),
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
                                // Tag for test harness
                                ui.push_id("format_dropdown", |ui| {
                                    let _combo_resp = ComboBox::from_id_salt("texture_format")
                                        .width(w_format)
                                        .selected_text(format!("{:?}", edit.settings.format))
                                        .show_ui(ui, |ui| {
                                            for format in TextureFormat::iter() {
                                                ui.add_enabled_ui(
                                                    format.is_available(
                                                        &self.model.compression_config,
                                                    ),
                                                    |ui| {
                                                        let resp = ui.selectable_value(
                                                            &mut edit.settings.format,
                                                            format,
                                                            format!("{format:?}"),
                                                        );
                                                        if resp.changed() {
                                                            // #2: auto-adjust sampler for non-filterable formats.
                                                            if !format.is_filterable() {
                                                                edit.settings
                                                                    .sampler
                                                                    .filter_mode =
                                                                    FilterMode::Nearest;
                                                                if let Some(ref mut mip) =
                                                                    edit.settings.sampler.mipmap
                                                                {
                                                                    mip.filter =
                                                                        MipmapFilter::Nearest;
                                                                    mip.anisotropy_clamp = 1;
                                                                }
                                                            }
                                                            self.dirty.set(true);
                                                        }
                                                    },
                                                );
                                            }
                                        });
                                }); // push_id("format_dropdown")
                            });
                        });

                        // ---- Sampler settings ----

                        // Filter Mode
                        body.row(row_h, |mut row| {
                            row.col(|ui| {
                                ui.label("Filter Mode:");
                            });
                            row.col(|ui| {
                                let mut current = edit.settings.sampler.filter_mode;
                                egui::ComboBox::from_id_salt("texture_filter_mode")
                                    .width(w_narrow)
                                    .selected_text(current.label())
                                    .show_ui(ui, |ui| {
                                        for mode in FilterMode::iter() {
                                            if ui
                                                .selectable_value(&mut current, mode, mode.label())
                                                .changed()
                                            {
                                                edit.settings.sampler.filter_mode = current;
                                                // Anisotropy requires Linear filtering.
                                                if current == FilterMode::Nearest {
                                                    if let Some(ref mut mip) =
                                                        edit.settings.sampler.mipmap
                                                    {
                                                        mip.anisotropy_clamp = 1;
                                                    }
                                                }
                                                self.dirty.set(true);
                                            }
                                        }
                                    });
                            });
                        });

                        // Address Mode (with per-axis support)
                        draw_address_mode_rows(
                            &mut body,
                            row_h,
                            w_default,
                            &mut edit.settings,
                            &self.dirty,
                            &self.per_axis_mode,
                        );

                        // Mipmap toggle
                        body.row(row_h, |mut row| {
                            row.col(|ui| {
                                ui.label("Enable Mipmap:");
                            });
                            row.col(|ui| {
                                let mut enabled = edit.settings.sampler.mipmap.is_some();
                                if ui.checkbox(&mut enabled, "").changed() {
                                    if enabled {
                                        let max_dim =
                                            edit.settings.width.max(edit.settings.height);
                                        let max_level = (max_dim as f32).log2().floor();
                                        edit.settings.sampler.mipmap =
                                            Some(crate::graphics::texture::sampler::MipmapConfig {
                                                filter: MipmapFilter::Linear,
                                                anisotropy_clamp: AnisotropyLevel::Level2.as_u16(),
                                                lod_min_clamp: 0.0,
                                                lod_max_clamp: max_level,
                                            });
                                    } else {
                                        edit.settings.sampler.mipmap = None;
                                    }
                                    self.dirty.set(true);
                                }
                            });
                        });

                        // Pre-compute LOD bounds before mutably borrowing sampler.
                        let max_dim = edit.settings.width.max(edit.settings.height);
                        let max_level = (max_dim as f32).log2().floor();

                        if let Some(ref mut mip) = edit.settings.sampler.mipmap {
                            // Mipmap Filter
                            body.row(row_h, |mut row| {
                                row.col(|ui| {
                                    ui.label("  Mipmap Filter:");
                                });
                                row.col(|ui| {
                                    let mut current = mip.filter;
                                    egui::ComboBox::from_id_salt("texture_mipmap_filter")
                                        .width(w_narrow)
                                        .selected_text(current.label())
                                        .show_ui(ui, |ui| {
                                            for mode in MipmapFilter::iter() {
                                                if ui
                                                    .selectable_value(
                                                        &mut current,
                                                        mode,
                                                        mode.label(),
                                                    )
                                                    .changed()
                                                {
                                                    mip.filter = current;
                                                    if current == MipmapFilter::Nearest {
                                                        mip.anisotropy_clamp = 1;
                                                    }
                                                    self.dirty.set(true);
                                                }
                                            }
                                        });
                                });
                            });

                            // Anisotropic
                            body.row(row_h, |mut row| {
                                row.col(|ui| {
                                    ui.label("  Anisotropic:");
                                });
                                row.col(|ui| {
                                    let can_aniso = edit.settings.sampler.filter_mode
                                        == FilterMode::Linear
                                        && mip.filter == MipmapFilter::Linear;
                                    let mut aniso_on = mip.anisotropy_clamp > 1 && can_aniso;
                                    ui.add_enabled_ui(can_aniso, |ui| {
                                        if ui.checkbox(&mut aniso_on, "").changed() {
                                            mip.anisotropy_clamp = if aniso_on {
                                                AnisotropyLevel::Level4.as_u16()
                                            } else {
                                                1
                                            };
                                            self.dirty.set(true);
                                        }
                                    });
                                    if aniso_on {
                                        let mut level =
                                            AnisotropyLevel::from_u16(mip.anisotropy_clamp)
                                                .unwrap_or(AnisotropyLevel::Level4);
                                        egui::ComboBox::from_id_salt("texture_anisotropy")
                                            .width(w_aniso)
                                            .selected_text(level.as_u16().to_string())
                                            .show_ui(ui, |ui| {
                                                for l in AnisotropyLevel::iter() {
                                                    if ui
                                                        .selectable_value(
                                                            &mut level,
                                                            l,
                                                            l.as_u16().to_string(),
                                                        )
                                                        .changed()
                                                    {
                                                        mip.anisotropy_clamp = level.as_u16();
                                                        self.dirty.set(true);
                                                    }
                                                }
                                            });
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
                                        mip.lod_max_clamp = val.max(0.0);
                                        self.dirty.set(true);
                                    }
                                });
                            });
                        }

                        // Compare
                        draw_compare_row(
                            &mut body,
                            row_h,
                            w_default,
                            &mut edit.settings,
                            &self.dirty,
                        );
                    });

                // Build the egui preview from the current pixels (cached until
                // Apply clears it).
                self.ensure_preview(ui, &edit.texture);
            }

            // ---- Apply button ----
            let changed = self.dirty.get();
            ui.vertical_centered(|ui| {
                // Tag for test harness: widget rect collection
                ui.push_id("apply_button", |ui| {
                    let apply_btn = egui::Button::new("Apply").min_size(Vec2::new(
                        ui.available_width(),
                        self.model.style.apply_button_height,
                    ));

                    let resp = ui.add_enabled(changed, apply_btn);

                    if resp.clicked() {
                        messager.send(Message::TextureInspectorApply(self.model.edit.clone()));
                    }
                    if changed {
                        ui.label("* unsaved changes");
                    }
                }); // push_id("apply_button")
            });

            ui.separator();

            // ---- Preview panel ----
            self.draw_preview(ui);
        });
    }

    fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
        if !self.dirty.get() {
            return None;
        }

        let dialog = ConfirmDialogWindow::new(
            "Unsaved texture changes".into(),
            "Apply the changes before leaving?".into(),
            "Apply".into(),
            "Discard".into(),
            Some(Message::TextureInspectorApply(self.model.edit.clone())),
            None,
            None::<fn()>,
            None::<fn()>,
        );
        Some(Box::new(dialog))
    }
}

fn load_compression_config() -> Result<TextureCompressionConfig, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(kairos_paths::PATH_KAIROS_SETTINGS)?;
    let engine_settings = toml::from_slice::<EngineSettings>(&bytes)?;
    Ok(engine_settings.texture_compression)
}

// ============================================================
// Sampler UI helpers
// ============================================================

/// Draw the address-mode row: quick-select ComboBox + optional per-axis sub-rows.
fn draw_address_mode_rows(
    body: &mut egui_extras::TableBody,
    row_h: f32,
    combo_width: f32,
    settings: &mut TextureSettings,
    dirty: &Cell<bool>,
    per_axis_mode: &Cell<bool>,
) {
    use strum::IntoEnumIterator;

    let s = &mut settings.sampler;
    let modes_equal = s.address_mode_u == s.address_mode_v && s.address_mode_v == s.address_mode_w;
    let is_per_axis = per_axis_mode.get();

    let current_label = if is_per_axis || !modes_equal {
        "Per Axis"
    } else {
        s.address_mode_u.label()
    };

    body.row(row_h, |mut row| {
        row.col(|ui| {
            ui.label("Address Mode:");
        });
        row.col(|ui| {
            egui::ComboBox::from_id_salt("texture_address_mode")
                .width(combo_width)
                .selected_text(current_label)
                .show_ui(ui, |ui| {
                    for mode in AddressMode::iter() {
                        let selected = !is_per_axis && modes_equal && s.address_mode_u == mode;
                        if ui.selectable_label(selected, mode.label()).clicked() {
                            s.address_mode_u = mode;
                            s.address_mode_v = mode;
                            s.address_mode_w = mode;
                            per_axis_mode.set(false);
                            dirty.set(true);
                        }
                    }
                    if ui.selectable_label(is_per_axis, "Per Axis").clicked() {
                        per_axis_mode.set(true);
                    }
                });
        });
    });

    // Per-axis sub-rows — shown only in per-axis mode.
    if is_per_axis {
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
                        .width(combo_width)
                        .selected_text(ptr.label())
                        .show_ui(ui, |ui| {
                            for mode in AddressMode::iter() {
                                if ui.selectable_label(*ptr == mode, mode.label()).clicked() {
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
        body.row(row_h, |mut row| {
            row.col(|ui| {
                ui.label("  Border Color:");
            });
            row.col(|ui| {
                let mut current = s.border_color;
                let label = current.map_or("None", |c| c.label());
                egui::ComboBox::from_id_salt("texture_border_color")
                    .width(combo_width)
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(current.is_none(), "None").clicked() {
                            s.border_color = None;
                            dirty.set(true);
                        }
                        for color in BorderColor::iter() {
                            if ui
                                .selectable_value(&mut current, Some(color), color.label())
                                .changed()
                            {
                                s.border_color = Some(color);
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
    combo_width: f32,
    settings: &mut TextureSettings,
    dirty: &Cell<bool>,
) {
    body.row(row_h, |mut row| {
        row.col(|ui| {
            ui.label("Compare:");
        });
        row.col(|ui| {
            let mut current = settings.sampler.compare;
            let label = current.map_or("None", |c| c.label());
            egui::ComboBox::from_id_salt("texture_compare")
                .width(combo_width)
                .selected_text(label)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(current.is_none(), "None").clicked() {
                        settings.sampler.compare = None;
                        dirty.set(true);
                    }
                    for func in CompareFunction::iter() {
                        if ui
                            .selectable_value(&mut current, Some(func), func.label())
                            .changed()
                        {
                            settings.sampler.compare = Some(func);
                            dirty.set(true);
                        }
                    }
                });
        });
    });
}
