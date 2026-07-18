use std::sync::Arc;

use crate::asset_loader::assets::{AssetHandle, AudioAssetsSystem, asset::PcmAssetsSystem};

pub mod pcm;

#[derive(Debug, Default)]
pub struct AudioExt {
    pub audio: Option<Arc<AssetHandle<AudioAssetsSystem>>>,
    pub pcm: Option<Arc<AssetHandle<PcmAssetsSystem>>>,
}
