use std::{any::type_name, fs, path::Path};

use crate::{
    kairos_editor::{
        Engine,
        project_path_tree::{ProjectPath, ProjectPathGraph},
        ui::Messager,
    },
    kairos_game::KairosGame,
    log::Log,
};
use egui::{Button, WidgetText};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::ui::{Drawer, Message, paths};

#[derive(Debug, Serialize, Deserialize)]
struct ProjectWindowStyle {
    pub title: String,
}

struct ProjectWindowModel {
    style: ProjectWindowStyle,
    project_path_graph: ProjectPathGraph,
}

pub struct ProjectWindow {
    model: ProjectWindowModel,
}

impl ProjectWindowStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_PROJECT_WINDOW_STYLE).map_err(|error| {
            format!(
                "Load ProjectWindow Style Json Failed, path: {}, error: {}",
                paths::PATH_PROJECT_WINDOW_STYLE,
                error
            )
        })?;
        let style = from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize ProjectWindow Style Json Failed, error: {}",
                error
            )
        })?;
        Ok(style)
    }
}

impl ProjectWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = ProjectWindowStyle::new()?;

        let project_path_graph = ProjectPathGraph::new();

        Ok(Self {
            style,
            project_path_graph,
        })
    }
}

impl ProjectWindow {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = ProjectWindowModel::new()?;
        Ok(Self { model })
    }
}

impl Drawer for ProjectWindow {
    fn show_window(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {}

    fn ui(&self, ui: &mut egui::Ui, _messager: &mut super::Messager, _log: &mut Log) {
        self.draw_dir(ui, self.model.project_path_graph.get_root_node());
    }

    fn close(&self, messager: &mut super::Messager) {
        messager.send(Message::CloseProjectTab);
    }

    fn get_name(&self) -> &'static str {
        type_name::<ProjectWindow>()
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
        _engine: &mut Engine,
        _game: &mut KairosGame,
        _messager: &mut Messager,
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        None
    }
}

impl ProjectWindow {
    fn draw_dir(&self, ui: &mut egui::Ui, node: petgraph::graph::NodeIndex) {
        let Some(pp) = self.model.project_path_graph.get_path(node) else {
            return;
        };

        match pp {
            ProjectPath::Dir(_) => {
                // println!("TODO Draw Path Dir: {:?}", path_buf)
            }
            ProjectPath::Texture(texture_path) => {
                let image_path = Path::new("file://");
                let mut image_path = image_path.join(&texture_path.path);
                if image_path.set_extension("png") {
                    let p = image_path.to_string_lossy();
                    let icon = egui::Image::new(egui::ImageSource::Uri(p));
                    let icon = icon.fit_to_exact_size(egui::Vec2 { x: 64.0, y: 64.0 });
                    let text = match texture_path.file_name.to_str() {
                        Some(str) => {
                            let rich_text = egui::RichText::new(str);
                            let rich_text = rich_text.size(16.0);
                            let text = WidgetText::from(rich_text);
                            Some(text)
                        }
                        None => None,
                    };
                    let bt = Button::opt_image_and_text(Some(icon), text);
                    let bt = ui.add(bt);
                    if bt.clicked() {}
                }
            }
            ProjectPath::Asset(_) => {
                // println!("TODO Draw Path Asset: {:?}", path_buf)
            }
        }

        let edges = self.model.project_path_graph.get_edges(node);
        edges.for_each(|edge| {
            let target = edge.target();
            self.draw_dir(ui, target);
        });
    }
}
