use std::{any::type_name, collections::HashSet, fs};

use eframe::egui::{self, Color32, RichText};
use egui_dock::{DockArea, DockState, NodeIndex, SurfaceIndex, TabViewer};
use kairos_engine::math;
use serde::{Deserialize, Serialize};
use sonic_rs::from_str;

use crate::kairos_editor::{UIDrawer, paths, ui_style_fields::{ColorStyleField, StyleField}};


#[derive(Debug, Serialize, Deserialize)]
pub struct MainContentStyle {
    pub background_color: math::Color32,
    pub central_panel_color: math::Color32
}

pub struct MainContentModel {
    style: MainContentStyle,
}

impl MainContentStyle {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_MAIN_CONTENT_STYLE)
            .map_err(|e| format!("Loader EditorWindowStyle.json Failed: {}, Path: {}", e, paths::PATH_MAIN_CONTENT_STYLE))?;

        let style = from_str(&style_json)
            .map_err(|e| format!("Desierialize Json Failed: {}", e))?;

        Ok(style)
    }
}

impl MainContentModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = MainContentStyle::new()?;

        Ok(Self { 
            style: style,
        })
    }
}

struct DocTabViewer {
    pub title: String,
}

impl TabViewer for DocTabViewer {
    type Tab = String;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.as_str().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab.as_str() {
            _ => {
                ui.label(tab.as_str());
            }
        }
    }

    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        true
    }
}

pub struct MainContent {
    model: MainContentModel,
    doc_tab_viewer: DocTabViewer,
    doc_tree: DockState<String>,
}

impl MainContent {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = MainContentModel::new()?;
        let mut doc_tree = DockState::new(vec!["Default".to_string()]);
        let [r_root, _] = doc_tree.main_surface_mut().split_right(
            NodeIndex::root(), 
            0.7,
            vec!["Inspector".to_string()]
        );
        let [r_root, _] = doc_tree.main_surface_mut().split_below(
            r_root, 
            0.7,
            vec!["Project".to_string(), "Console".to_string()] 
        );
        let [_, _] = doc_tree.main_surface_mut().split_left(
            r_root, 
            0.3,
            vec!["Hierarchy".to_string()]
        );
        let mut open_tabs = HashSet::new();
        for node in doc_tree[SurfaceIndex::main()].iter() {
            if let Some(tabs) = node.tabs() {
                for tab in tabs {
                    open_tabs.insert(tab);
                }
            }
        }
        let doc_tab_viewer = DocTabViewer {
            title: "SimpleDoc".to_string()
        };
        Ok(
            Self {  
                model,
                doc_tab_viewer,
                doc_tree,
            }   
        )
    }
}

impl UIDrawer for MainContent {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame, _messager: &mut super::UIMessager) {
        let model = &self.model;
        // 设置整体背景色
        ctx.style_mut(|style| {
            style.visuals.window_fill = model.style.background_color.into();
            style.visuals.panel_fill = model.style.background_color.into();
        });

        // 中央区域显示内容
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(model.style.central_panel_color.into()))
            .show(ctx, |ui| {
                // ui.vertical_centered(|ui| {
                //     ui.label(RichText::new("Main Content Area").size(24.0).color(Color32::LIGHT_GRAY));
                //     ui.label(RichText::new("Custom titlebar demo").size(14.0).color(Color32::GRAY));
                // }

                DockArea::new(&mut self.doc_tree)
                    .show_inside(ui, &mut self.doc_tab_viewer);
            }
        );
    }
    
    fn get_style_fileds(&self) -> Vec<super::ui_style_fields::StyleField> {
        let mut fields = Vec::new();
        let style = &self.model.style;

        fields.push(StyleField::ColorStyleField(ColorStyleField::new("background_color", style.background_color)));
        fields.push(StyleField::ColorStyleField(ColorStyleField::new("central_panel_color", style.central_panel_color)));

        fields
    }

    fn update_style(&mut self, style_fields: &Vec<StyleField>) {
        if let StyleField::ColorStyleField(field) = &style_fields[0] {
            self.model.style.background_color = field.color;
        }
        if let StyleField::ColorStyleField(field) = &style_fields[1] {
            self.model.style.central_panel_color = field.color;
        }
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    
    fn get_name(&self) -> &'static str {
        type_name::<MainContent>()
    }
}