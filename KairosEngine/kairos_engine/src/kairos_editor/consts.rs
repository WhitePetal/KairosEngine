pub const APP_NAME: &'static str = "KairosEngine";
pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

// TextureExt (editor runtime composite) capacities
pub const TEXTURE_EXT_ASSETS_CAPACITY: usize = 512;
pub const TEXTURE_EXT_ASSETS_LOADED_CHANNEL_BUFFER_SIZE: usize = 64;
pub const TEXTURE_EXT_ASSETS_DROP_CHANNEL_BUFFER_SIZE: usize = 16;
