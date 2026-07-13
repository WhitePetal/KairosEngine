use crate::{asset_loader::assets::AssetsServer, kairos_editor::ui::Messager};

pub mod creater;
pub mod directory;
pub mod text;
pub mod toml;

pub trait Inspector {
    fn create(path: &std::path::Path, assets_server: &mut AssetsServer) -> Self
    where
        Self: Sized;

    fn draw(&self, ui: &mut egui::Ui, messager: &mut Messager, assets_server: &AssetsServer);

    fn dirty(&self) -> bool;

    fn set_dirty(&mut self, dirty: bool);
}
