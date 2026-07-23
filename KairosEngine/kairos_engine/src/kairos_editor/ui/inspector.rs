use std::any::Any;

use crate::{
    asset_loader::assets::AssetsServer,
    graphics::graphics_graph::GraphicsCommand,
    kairos_editor::{
        project_path_tree::ProjectPathGraph,
        ui::{Messager, UIReader, dialog::Dialog},
    },
};

pub mod audio;
pub mod code;
pub mod creater;
pub mod directory;
pub mod document;
pub mod font;
pub mod material;
pub mod mesh;
pub mod shader;
pub mod texture;
pub mod toml;
pub mod unknown;

pub trait InspectorFieldKey {
    fn get_key(&self) -> usize;
}
pub trait InspectorFieldValue {
    fn get_value<T>(&self) -> T;
}

pub trait InspectorField {
    fn get_key(&self) -> Box<dyn InspectorFieldKey>;
}

pub trait Inspector: Any {
    fn create(
        path: &std::path::Path,
        assets_server: &mut AssetsServer,
        _project_graph: &ProjectPathGraph,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized;

    fn draw(
        &self,
        ui: &mut egui::Ui,
        reader: &UIReader,
        messager: &mut Messager,
        assets_server: &AssetsServer,
        dt: f32,
    );

    fn on_exit(&mut self, ctx: &egui::Context) -> Option<Box<dyn Dialog>>;

    /// Optional: return preview render commands (e.g., 3D model preview).
    /// Called during `Context::render()` alongside other Drawer::render() calls.
    fn render(&self) -> Option<GraphicsCommand> {
        None
    }

    /// Inspector 被替换 / 关闭时调用：交出仍持有的预览 egui texture id，
    /// 由 Context 统一在下一帧 render() 释放（否则泄漏在 egui_wgpu renderer 中）。
    /// 默认实现：无预览纹理。
    fn take_preview_egui_textures(&mut self) -> Vec<egui::TextureId> {
        Vec::new()
    }
}
