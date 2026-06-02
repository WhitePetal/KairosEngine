
use crate::asset_loader::{consts, texture::TextureAssets};


pub struct Assets {
    texture: TextureAssets,
}

impl Assets {
    pub fn new() -> Self {
        // let (on_asset_loaded_sender, on_asset_loaded_recever) = tokio::sync::mpsc::channel(consts::ASSETS_LOADED_CHANNEL_BUFFER_SIZE);
        let texture = TextureAssets::new(consts::TEXTURE_ASSETS_CAPACITY);

        Self { texture}
    }

    pub fn handle(&mut self) {
        self.texture.handle_recves();
    }
}

