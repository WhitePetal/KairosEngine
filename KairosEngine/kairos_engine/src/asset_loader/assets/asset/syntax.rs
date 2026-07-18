use crate::kairos_editor::syntax::{SyntaxConfig, SyntaxHighlightSettings};
use std::path::PathBuf;

use anyhow::Error;

use crate::asset_loader::{
    assets::{
        DependencyLoadRequestEvent,
        asset::{self, AssetIndex, AssetLoader, Assets, AssetsHandler, AssetsSystem},
    },
    consts,
};

// ---------------------------------------------------------------------------
// LoadedEvent / DropEvent
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct LoadedEvent {
    index: AssetIndex,
    settings: SyntaxHighlightSettings,
}
impl asset::LoadedEvent<SyntaxHighlightSettings> for LoadedEvent {
    fn get_index(&self) -> AssetIndex {
        self.index
    }

    fn get_asset(self) -> SyntaxHighlightSettings {
        self.settings
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

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Loader {}
impl Loader {
    async fn load(
        path: PathBuf,
        asset_index: AssetIndex,
        sender: tokio::sync::mpsc::Sender<LoadedEvent>,
        _denpendency_request_sender: tokio::sync::mpsc::Sender<DependencyLoadRequestEvent>,
    ) -> Result<(), Error> {
        // 1. Read and parse the per-language TOML config.
        let toml_str = tokio::fs::read_to_string(&path).await?;
        let cfg: SyntaxConfig = toml::from_str(&toml_str)?;

        // 2. Build the SyntaxSet: always start with defaults.
        let default_ss = syntect::parsing::SyntaxSet::load_defaults_newlines();
        let mut builder = default_ss.into_builder();

        // If a custom sublime-syntax is declared, load and add it.
        // The path is relative to the engine root (working directory),
        // consistent with all other asset paths in the engine.
        if let Some(syntax_path) = &cfg.sublime_syntax {
            let yaml_str = tokio::fs::read_to_string(syntax_path).await?;
            let syntax_def = syntect::parsing::SyntaxDefinition::load_from_str(
                &yaml_str,
                true,
                syntax_path.file_stem().and_then(|s| s.to_str()),
            )?;
            builder.add(syntax_def);
        }

        let ps = builder.build();

        // 3. Build the ThemeSet with the TOML theme.
        let custom_theme = cfg.build_syntect_theme();
        let mut ts = syntect::highlighting::ThemeSet::load_defaults();

        // Override all preset slots so the custom theme takes effect
        // regardless of which theme `egui_extras::CodeTheme` selects.
        for key in [
            "base16-eighties.dark",
            "base16-mocha.dark",
            "base16-ocean.dark",
            "base16-ocean.light",
            "InspiredGitHub",
            "Solarized (dark)",
            "Solarized (light)",
        ] {
            ts.themes.insert(key.into(), custom_theme.clone());
        }

        let language_name = cfg.language_name.clone();

        let settings = SyntaxHighlightSettings {
            language_name,
            settings: egui_extras::syntax_highlighting::SyntectSettings { ps, ts },
        };

        sender
            .send(LoadedEvent {
                index: asset_index,
                settings,
            })
            .await?;
        Ok(())
    }
}
impl AssetLoader<LoadedEvent, SyntaxHighlightSettings> for Loader {
    fn load_asset(
        &self,
        path: PathBuf,
        asset_index: AssetIndex,
        loaded_sender: tokio::sync::mpsc::Sender<LoadedEvent>,
        denpendency_request_sender: tokio::sync::mpsc::Sender<DependencyLoadRequestEvent>,
    ) {
        tokio::spawn(Self::load(
            path,
            asset_index,
            loaded_sender,
            denpendency_request_sender,
        ));
    }
}

// ---------------------------------------------------------------------------
// SyntaxAssetsSystem
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct SyntaxAssetsSystem {
    assets: Assets<Self>,
}
impl SyntaxAssetsSystem {
    pub fn new() -> Self {
        let loader = Loader {};
        let assets = Assets::<Self>::new(
            loader,
            consts::SYNTAX_ASSETS_CAPACITY,
            consts::SYNTAX_ASSETS_LOADED_CHANNEL_BUFFER_SIZE,
            consts::SYNTAX_ASSETS_DROP_CHANNEL_BUFFER_SIZE,
        );
        Self { assets }
    }
}

impl AssetsHandler for SyntaxAssetsSystem {
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

impl Default for SyntaxAssetsSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetsSystem for SyntaxAssetsSystem {
    type AssetType = SyntaxHighlightSettings;

    type LoadedEvent = LoadedEvent;

    type DropEvent = DropEvent;

    type Loader = Loader;

    fn get_assets(&self) -> &Assets<Self>
    where
        Self: Sized,
    {
        &self.assets
    }

    fn get_assets_mut(&mut self) -> &mut Assets<Self>
    where
        Self: Sized,
    {
        &mut self.assets
    }
}
