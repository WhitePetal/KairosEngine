use crate::{
    asset_loader::assets::AssetsServer,
    kairos_editor::ui::{Messager, dialog::Dialog},
};

pub mod audio;
pub mod creater;
pub mod directory;
pub mod document;
pub mod font;
pub mod text;
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

pub trait Inspector {
    fn create(
        path: &std::path::Path,
        assets_server: &mut AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized;

    fn draw(&self, ui: &mut egui::Ui, messager: &mut Messager, assets_server: &AssetsServer);

    fn on_exit(&self, ctx: &egui::Context, assets_server: &AssetsServer)
    -> Option<Box<dyn Dialog>>;
}
