use std::{path::PathBuf, sync::Arc};

use anyhow::Error;
use tokio::sync::{
    mpsc::{self},
    oneshot,
};

use crate::{
    asset_loader::assets::{
        AssetHandle, DependencyLoadRequest, DependencyLoadRequestEvent, TextureAssetsSystem,
        asset::{self, AssetIndex, Assets, AssetsHandler, AssetsSystem},
    },
    graphics::texture::SerializedTexture,
    kairos_editor::consts,
};

// ============================================================
// TextureExt — editor runtime resource (cf. AudioExt)
// ============================================================

/// Editor-specific composite that bundles the texture's serialized
/// settings, its runtime pixel data, and cached original image info.
///
/// Loaded asynchronously by `TextureExtAssetsSystem`.
#[derive(Debug, Clone)]
pub struct TextureExt {
    /// Canonical settings — modifiable by the inspector.
    pub serialized: SerializedTexture,
    /// Handle to the runtime `Texture` (RGBA pixel data for preview).
    pub texture: Arc<AssetHandle<TextureAssetsSystem>>,
    /// Original image width from the source PNG.
    pub original_width: u32,
    /// Original image height from the source PNG.
    pub original_height: u32,
    /// Cached original RGBA pixel data (for resizing on Apply).
    pub original_rgba: Vec<u8>,
}

// ============================================================
// Asset system boilerplate
// ============================================================

#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    asset: TextureExt,
}
impl asset::LoadedEvent<TextureExt> for LoadedEvent {
    fn get_index(&self) -> AssetIndex {
        self.index
    }
    fn get_asset(self) -> TextureExt {
        self.asset
    }
}

#[derive(Debug)]
pub struct DropEvent {
    index: AssetIndex,
}
impl asset::DropEvent for DropEvent {
    fn new(index: AssetIndex) -> Self {
        Self { index }
    }
    fn get_index(&self) -> AssetIndex {
        self.index
    }
}

#[derive(Debug)]
pub struct Loader {}
impl Loader {
    async fn load(
        path: PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
        denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) -> Result<(), Error> {
        // 1. Read .texture TOML → SerializedTexture
        let toml_bytes = tokio::fs::read(&path).await?;
        let serialized: SerializedTexture =
            tokio::task::spawn_blocking(move || toml::from_slice(&toml_bytes)).await??;

        // 2. Request TextureAssetsSystem dependency (loads .texture_bin)
        let (texture_setback_sender, texture_setback_receiver) =
            oneshot::channel::<Arc<AssetHandle<TextureAssetsSystem>>>();
        denpendency_request_sender
            .send(Box::new(DependencyLoadRequest::<TextureAssetsSystem> {
                dependency_path: path.clone(),
                setback_sender: texture_setback_sender,
            }))
            .await?;
        let texture = texture_setback_receiver.await?;

        // 3. Read original PNG to cache dimensions + RGBA data
        let source_path = serialized.source_path.clone();
        let (original_width, original_height, original_rgba) =
            tokio::task::spawn_blocking(move || match image::open(&source_path) {
                Ok(img) => {
                    let (w, h) = (img.width(), img.height());
                    (w, h, img.into_rgba8().into_vec())
                }
                Err(e) => {
                    log::warn!(
                        "TextureExt: failed to open source PNG '{}': {e}",
                        source_path.display()
                    );
                    (0, 0, Vec::new())
                }
            })
            .await?;

        let asset = TextureExt {
            serialized,
            texture,
            original_width,
            original_height,
            original_rgba,
        };

        sender
            .send(LoadedEvent {
                index: asset_index,
                asset,
            })
            .await?;
        Ok(())
    }
}

impl asset::AssetLoader<LoadedEvent, TextureExt> for Loader {
    fn load_asset(
        &self,
        path: PathBuf,
        asset_index: AssetIndex,
        sender: mpsc::Sender<LoadedEvent>,
        denpendency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) {
        tokio::spawn(Self::load(
            path,
            asset_index,
            sender,
            denpendency_request_sender,
        ));
    }
}

#[derive(Debug)]
pub struct TextureExtAssetsSystem {
    assets: Assets<Self>,
}

impl TextureExtAssetsSystem {
    pub fn new() -> Self {
        let loader = Loader {};
        let assets = Assets::<Self>::new(
            loader,
            consts::TEXTURE_EXT_ASSETS_CAPACITY,
            consts::TEXTURE_EXT_ASSETS_LOADED_CHANNEL_BUFFER_SIZE,
            consts::TEXTURE_EXT_ASSETS_DROP_CHANNEL_BUFFER_SIZE,
        );
        Self { assets }
    }
}

impl AssetsHandler for TextureExtAssetsSystem {
    fn handle_receves(&mut self) {
        self.assets.handle_receves();
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Default for TextureExtAssetsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetsSystem for TextureExtAssetsSystem {
    type AssetType = TextureExt;
    type LoadedEvent = LoadedEvent;
    type DropEvent = DropEvent;
    type Loader = Loader;

    fn get_assets(&self) -> &Assets<Self> {
        &self.assets
    }
    fn get_assets_mut(&mut self) -> &mut Assets<Self> {
        &mut self.assets
    }
}
