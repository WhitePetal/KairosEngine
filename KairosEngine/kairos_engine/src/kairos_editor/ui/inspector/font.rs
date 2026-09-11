use std::{cell::Cell, fs};

use egui::{FontData, FontFamily, FontId, RichText};
use crate::asset::{AssetServer, Assets, Handle};
use kairos_ecs::world::World;
use serde::{Deserialize, Serialize};

use crate::{
    kairos_editor::ui::{Messager, UIReader, dialog::Dialog, inspector::Inspector, paths},
    kairos_ui::font::Font,
};

#[derive(Debug, Serialize, Deserialize)]
struct FontInspectorStyle {
    default_font_size: f32,
    example_content: String,
}
impl FontInspectorStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_FONT_INSPECTOR_STYLE).map_err(|error| {
            format!(
                "Load FontInspector Style Toml Failed, path: {}, error: {}",
                paths::PATH_FONT_INSPECTOR_STYLE,
                error
            )
        })?;
        let style: Self = toml::from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize FontInspector Style Toml Failed, error: {}",
                error
            )
        })?;
        Ok(style)
    }
}

struct FontInspectorModel {
    style: FontInspectorStyle,
    font_handle: Handle<Font>,
    family_name: std::sync::Arc<str>,
}
impl FontInspectorModel {
    fn new(
        font_handle: Handle<Font>,
        family_name: std::sync::Arc<str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let style = FontInspectorStyle::new()?;
        Ok(Self {
            style,
            font_handle,
            family_name,
        })
    }
}

pub struct FontInspector {
    model: FontInspectorModel,
    font_size: Cell<f32>,
}

impl FontInspector {
    fn family(&self) -> FontFamily {
        FontFamily::Name(self.model.family_name.clone())
    }

    fn is_registered(&self, ctx: &egui::Context) -> bool {
        ctx.fonts(|f| f.definitions().families.contains_key(&self.family()))
    }

    fn register(&self, ctx: &egui::Context, font_data: &[u8]) {
        let mut defs = ctx.fonts(|f| f.definitions().clone());
        defs.font_data.insert(
            self.model.family_name.to_string(),
            FontData::from_owned(font_data.to_vec()).into(),
        );
        defs.families
            .insert(self.family(), vec![self.model.family_name.to_string()]);
        ctx.set_fonts(defs);
    }

    fn unregister(&self, ctx: &egui::Context) {
        let mut defs = ctx.fonts(|f| f.definitions().clone());
        defs.font_data.remove(self.model.family_name.as_ref());
        defs.families.remove(&self.family());
        ctx.set_fonts(defs);
    }
}

impl Inspector for FontInspector {
    fn create(
        path: &std::path::Path,
        world: &World,
        _project_graph: &crate::kairos_editor::project_path_tree::ProjectPathGraph,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // The font rides the new core: the `AssetServer` World resource starts
        // the load (driven by `PreUpdate`) and hands back a lightweight
        // `Handle<Font>`, whose value lands in the `Assets<Font>` store.
        let handle = world
            .resource::<AssetServer>()
            .load::<Font>(path.to_path_buf());
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("font_inspector");
        let family_name = format!("__inspector_font_{name}");
        let model = FontInspectorModel::new(handle, family_name.into())?;
        let default_font_size = model.style.default_font_size;
        Ok(Self {
            model,
            font_size: Cell::new(default_font_size),
        })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        _reader: &UIReader,
        _messager: &mut Messager,
        world: &World,
        _dt: f32,
    ) {
        if !self.is_registered(ui.ctx()) {
            if let Some(font) = world
                .resource::<Assets<Font>>()
                .get(self.model.font_handle.id())
            {
                self.register(ui.ctx(), &font.bytes);
                ui.label("Loading font preview...");
            } else {
                // The load runs on the io task pool, so keep repainting until
                // the value reaches the store.
                ui.label("Font data not loaded yet...");
            }
            ui.ctx().request_repaint();
            return;
        }

        ui.horizontal(|ui| {
            ui.label("Size:");
            let mut size = self.font_size.get();
            if ui.add(egui::Slider::new(&mut size, 8.0..=72.0)).changed() {
                self.font_size.set(size);
            }
        });

        ui.separator();

        let font_id = FontId::new(self.font_size.get(), self.family());
        ui.label(
            RichText::new(&self.model.style.example_content)
                .font(font_id)
                .size(self.font_size.get()),
        );
    }

    fn on_exit(&mut self, ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
        self.unregister(ctx);
        None
    }
}
