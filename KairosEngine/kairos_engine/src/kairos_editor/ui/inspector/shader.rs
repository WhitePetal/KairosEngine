use std::{cell::Cell, fs, ops::DerefMut, path::PathBuf, sync::Arc};

use egui::Vec2;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::{
    asset_loader::assets::{
        AssetHandle, AssetsServer, SyntaxAssetsSystem, asset::TextAssetsSystem,
    },
    kairos_editor::ui::{
        self, Message, Messager,
        dialog::{ConfirmDialogWindow, Dialog},
        inspector::Inspector,
        paths,
    },
    math,
};

#[derive(Debug, Serialize, Deserialize)]
struct ShaderStyle {
    save_button_height: f32,
    desired_rows: usize,
    dark_background_color: math::Color32,
    bright_background_color: math::Color32,
}
impl ShaderStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = fs::read(paths::PATH_SHADER_INSPECTOR_STYLE)?;
        let style = toml::from_slice(&bytes)?;
        Ok(style)
    }
}

struct ShaderModel {
    style: ShaderStyle,
    path: PathBuf,
    handle: Arc<AssetHandle<TextAssetsSystem>>,
    content: Arc<Mutex<Option<String>>>,
    dirty: Cell<bool>,
    /// WGSL syntax highlighting settings, loaded via the asset system.
    syntax_handle: Arc<AssetHandle<SyntaxAssetsSystem>>,
}

pub struct ShaderInspector {
    model: ShaderModel,
}

impl Inspector for ShaderInspector {
    fn create(
        path: &std::path::Path,
        assets_server: &mut AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        let style = ShaderStyle::new()?;
        let path = path.to_path_buf();
        let handle = assets_server.load(&path);

        // Load the per-language syntax+theme config through the asset system.
        // This is async — the inspector will show a "Loading…" label
        // until the SyntaxSet is built (first frame).
        let syntax_path: PathBuf = paths::PATH_WGSL_SYNTAX_CONFIG.into();
        let syntax_handle = assets_server.load::<SyntaxAssetsSystem>(&syntax_path);

        let content = Arc::new(Mutex::new(None));
        let model = ShaderModel {
            style,
            path,
            handle,
            content,
            dirty: Cell::new(false),
            syntax_handle,
        };

        Ok(Self { model })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        messager: &mut Messager,
        assets_server: &AssetsServer,
        _dt: f32,
    ) {
        {
            let mut content_mut = self.model.content.lock();
            let content_mut = content_mut.deref_mut();
            if content_mut.is_none() {
                if let Some(content) = assets_server.get(&self.model.handle) {
                    *content_mut = Some(content.clone());
                }
                ui.label("Shader File is Loading...");
                return;
            }
        }

        // Wait until the syntax settings are loaded.
        let Some(syntax_settings) =
            assets_server.get::<SyntaxAssetsSystem>(&self.model.syntax_handle)
        else {
            ui.label("WGSL syntax highlighting is loading...");
            return;
        };

        let mut changed = false;
        {
            let mut content_mut = self.model.content.lock();
            let Some(content) = content_mut.deref_mut() else {
                return;
            };
            let theme =
                egui_extras::syntax_highlighting::CodeTheme::from_memory(ui.ctx(), ui.style());

            // `highlight_with` memos by reference address, so passing a stable
            // `&SyntectSettings` from the asset system avoids recomputation.
            let settings = &syntax_settings.settings;
            let language_name: &str = &syntax_settings.language_name;

            let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
                let mut layout_job = egui_extras::syntax_highlighting::highlight_with(
                    ui.ctx(),
                    ui.style(),
                    &theme,
                    buf.as_str(),
                    language_name,
                    settings,
                );
                layout_job.wrap.max_width = wrap_width;
                ui.fonts_mut(|f| f.layout_job(layout_job))
            };

            egui::ScrollArea::vertical()
                .max_height(
                    ui.available_height()
                        - self.model.style.save_button_height
                        - ui::DEFAULT_SPEATOR_HEIGHT
                        - ui::DEFAULT_LABEL_HEIGHT,
                )
                .show(ui, |ui| {
                    let editor = egui::TextEdit::multiline(content)
                        .font(egui::TextStyle::Monospace) // for cursor height
                        .code_editor()
                        .desired_rows(self.model.style.desired_rows)
                        .lock_focus(true)
                        .desired_width(f32::INFINITY)
                        .layouter(&mut layouter);
                    let editor = {
                        let background_color = if theme.is_dark() {
                            self.model.style.dark_background_color
                        } else {
                            self.model.style.bright_background_color
                        };
                        editor.background_color(background_color.into())
                    };
                    let edit = ui.add(editor);
                    if edit.changed() {
                        changed = true;
                    }
                });
        }

        ui.separator();

        if changed {
            self.model.dirty.replace(true);
        }

        ui.vertical_centered(|ui| {
            let save_btn =
                egui::Button::new("Save").min_size(Vec2::new(ui.available_width(), 20.0));
            if ui.add_enabled(self.model.dirty.get(), save_btn).clicked() {
                messager.send(Message::ShaderInspectorSave(
                    self.model.path.clone(),
                    self.model.handle.clone(),
                    self.model.content.clone(),
                ));
                self.model.dirty.replace(false);
            }
            if self.model.dirty.get() {
                ui.label("* unsaved changes");
            }
        });
    }

    fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
        if self.model.dirty.get() {
            let handle = self.model.handle.clone();
            let content = self.model.content.clone();
            let path = self.model.path.clone();
            let dialog = ConfirmDialogWindow::new(
                "have modify not save".into(),
                "save the modify?".into(),
                "save".into(),
                "cancel".into(),
                Some(Message::CodeInspectorSave(path, handle, content)),
                None,
                None::<fn()>,
                None::<fn()>,
            );
            Some(Box::new(dialog))
        } else {
            None
        }
    }
}

impl ShaderInspector {
    pub fn save_shader(
        assets_server: &mut AssetsServer,
        path: &PathBuf,
        handle: Arc<AssetHandle<TextAssetsSystem>>,
        content: Arc<Mutex<Option<String>>>,
    ) {
        let mut content = content.lock();
        if let Some(content) = content.deref_mut().take() {
            if let Err(e) = fs::write(path, &content) {
                log::warn!("Failed to write Document '{}': {e}", path.display());
            }
            if let Some(doc_res) = assets_server.get_mut(&handle) {
                *doc_res = content;
            }
        }
    }
}
