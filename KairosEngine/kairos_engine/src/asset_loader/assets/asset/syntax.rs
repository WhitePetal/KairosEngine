use std::path::PathBuf;

use anyhow::Error;
use serde::Deserialize;

use crate::asset_loader::{
    assets::{
        DependencyLoadRequestEvent,
        asset::{self, AssetIndex, AssetLoader, Assets, AssetsHandler, AssetsSystem},
    },
    consts,
};

// ---------------------------------------------------------------------------
// SyntaxHighlightSettings — the asset type
// ---------------------------------------------------------------------------

/// Carries the syntect settings AND the language name so callers never need
/// to hardcode the language string (e.g. `"WGSL"`, `"Rust"`).
pub struct SyntaxHighlightSettings {
    /// The language name as declared in the TOML config, e.g. `"WGSL"`, `"Rust"`.
    pub language_name: String,
    /// The syntect settings (SyntaxSet + ThemeSet).
    pub settings: egui_extras::syntax_highlighting::SyntectSettings,
}

impl std::fmt::Debug for SyntaxHighlightSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyntaxHighlightSettings")
            .field("language_name", &self.language_name)
            .field("syntax_count", &self.settings.ps.syntaxes().len())
            .field("theme_count", &self.settings.ts.themes.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// SyntaxConfig — TOML-driven per-language syntax + theme config
// ---------------------------------------------------------------------------

/// Deserialized from a per-language TOML placed in
/// `Preferences/SublimeSyntax/<lang>_syntax.toml`.
///
/// # Example (WGSL with custom sublime-syntax)
///
/// ```toml
/// language_name = "WGSL"
/// sublime_syntax = "Preferences/SublimeSyntax/wgsl.sublime-syntax"
///
/// [theme]
/// name = "Kairos Dark"
///
/// [theme.colors]
/// foreground = "#D0D0D0"
/// background = "#1E1E1E"
/// keyword     = "#FF6464"
/// # ...
/// ```
///
/// # Example (Rust — built-in syntax, only customise theme)
///
/// ```toml
/// language_name = "Rust"
///
/// [theme]
/// name = "Kairos Dark"
///
/// [theme.colors]
/// keyword     = "#FF6464"
/// type        = "#57A5AB"
/// # ...
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct SyntaxConfig {
    /// The name passed to `highlight_with(…, language_name, …)`.
    ///
    /// This must match the `name` field in the `.sublime-syntax` file,
    /// or for built-in languages one of syntect's recognised names
    /// (e.g. `"Rust"`, `"C++"`, `"TOML"`).
    pub language_name: String,

    /// Optional path to a `.sublime-syntax` YAML file. When present, the
    /// syntax is loaded and added to the `SyntaxSet` so that custom
    /// languages (e.g. WGSL) are recognised by syntect.
    ///
    /// When absent the loader falls back to syntect's built-in syntaxes
    /// (suitable for Rust, TOML, C++, …).
    #[serde(default)]
    pub sublime_syntax: Option<String>,

    pub theme: SyntaxThemeSection,
}


#[derive(Debug, Clone, Deserialize)]
pub struct SyntaxThemeSection {
    /// Theme name, stored in the generated syntect theme.
    #[serde(default = "default_theme_name")]
    pub name: String,

    #[serde(default = "SyntaxThemeColorFields::default_values")]
    pub colors: SyntaxThemeColorFields,
}

fn default_theme_name() -> String {
    "Kairos Custom".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyntaxThemeColorFields {
    /// Default text color.
    #[serde(default = "default_foreground")]
    pub foreground: String,
    /// Editor background color.
    #[serde(default = "default_background")]
    pub background: String,
    /// Keywords: `fn`, `let`, `if`, `return`, …
    #[serde(default = "default_keyword")]
    pub keyword: String,
    /// Types: `f32`, `vec3`, `bool`, `texture_2d`, …
    #[serde(default = "default_type")]
    pub r#type: String,
    /// Built-in functions: `dot`, `normalize`, `textureSample`, …
    #[serde(default = "default_function")]
    pub function: String,
    /// String literals.
    #[serde(default = "default_string")]
    pub string: String,
    /// Comments.
    #[serde(default = "default_comment")]
    pub comment: String,
    /// Numeric literals.
    #[serde(default = "default_number")]
    pub number: String,
    /// Attributes / decorators: `@vertex`, `@group`, `#[derive(…)]`, …
    #[serde(default = "default_attribute")]
    pub attribute: String,
    /// Built-in variables: `position`, `vertex_index`, …
    #[serde(default = "default_builtin")]
    pub builtin_variable: String,
    /// User-defined variables / identifiers.
    #[serde(default = "default_variable")]
    pub variable: String,
    /// Operators: `+`, `-`, `&&`, `==`, …
    #[serde(default = "default_operator")]
    pub operator: String,
    /// Punctuation: `{ } ( ) [ ] ; , .`
    #[serde(default = "default_punctuation")]
    pub punctuation: String,
}

impl SyntaxThemeColorFields {
    fn default_values() -> Self {
        Self {
            foreground: default_foreground(),
            background: default_background(),
            keyword: default_keyword(),
            r#type: default_type(),
            function: default_function(),
            string: default_string(),
            comment: default_comment(),
            number: default_number(),
            attribute: default_attribute(),
            builtin_variable: default_builtin(),
            variable: default_variable(),
            operator: default_operator(),
            punctuation: default_punctuation(),
        }
    }
}

// ---- defaults (dark theme) ------------------------------------------------

fn default_foreground() -> String { "#D0D0D0".into() }
fn default_background() -> String { "#1E1E1E".into() }
fn default_keyword() -> String { "#FF6464".into() }
fn default_type() -> String { "#57A5AB".into() }
fn default_function() -> String { "#6D93E2".into() }
fn default_string() -> String { "#6D93E2".into() }
fn default_comment() -> String { "#787878".into() }
fn default_number() -> String { "#57A5AB".into() }
fn default_attribute() -> String { "#FFD700".into() }
fn default_builtin() -> String { "#DCDCAA".into() }
fn default_variable() -> String { "#D0D0D0".into() }
fn default_operator() -> String { "#D0D0D0".into() }
fn default_punctuation() -> String { "#C0C0C0".into() }

// ---------------------------------------------------------------------------
// TOML → syntect::highlighting::Theme
// ---------------------------------------------------------------------------

fn parse_hex_color(hex: &str) -> Option<syntect::highlighting::Color> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(syntect::highlighting::Color { r, g, b, a: 0xFF })
}

fn fg(hex: &str) -> Option<syntect::highlighting::Color> {
    parse_hex_color(hex)
}

fn scope_item(
    scope_str: &str,
    color: &str,
) -> syntect::highlighting::ThemeItem {
    syntect::highlighting::ThemeItem {
        scope: std::str::FromStr::from_str(scope_str).unwrap_or_default(),
        style: syntect::highlighting::StyleModifier {
            foreground: fg(color),
            background: None,
            font_style: None,
        },
    }
}

/// Build a syntect `Theme` from the TOML color configuration.
fn build_syntect_theme(cfg: &SyntaxConfig) -> syntect::highlighting::Theme {
    let c = &cfg.theme.colors;

    let scopes: Vec<syntect::highlighting::ThemeItem> = vec![
        // ---- keyword ----
        scope_item("keyword", &c.keyword),
        // ---- type ----
        scope_item("storage.type", &c.r#type),
        scope_item("entity.name.type", &c.r#type),
        scope_item("support.type", &c.r#type),
        // ---- function ----
        scope_item("support.function", &c.function),
        scope_item("entity.name.function", &c.function),
        // ---- string ----
        scope_item("string", &c.string),
        // ---- comment ----
        scope_item("comment", &c.comment),
        // ---- number ----
        scope_item("constant.numeric", &c.number),
        // ---- attribute / modifier ----
        scope_item("storage.modifier", &c.attribute),
        scope_item("meta.annotation", &c.attribute),
        scope_item("meta.attribute", &c.attribute),
        // ---- built-in variable ----
        scope_item("variable.language", &c.builtin_variable),
        scope_item("support.variable", &c.builtin_variable),
        // ---- user variable ----
        scope_item("variable.other", &c.variable),
        scope_item("variable", &c.variable),
        // ---- operator ----
        scope_item("keyword.operator", &c.operator),
        // ---- punctuation ----
        scope_item("punctuation", &c.punctuation),
    ];

    syntect::highlighting::Theme {
        name: Some(cfg.theme.name.clone()),
        settings: syntect::highlighting::ThemeSettings {
            foreground: fg(&c.foreground),
            background: fg(&c.background),
            ..Default::default()
        },
        scopes,
        ..Default::default()
    }
}

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
        if let Some(ref syntax_rel_path) = cfg.sublime_syntax {
            let syntax_path = std::path::PathBuf::from(syntax_rel_path);

            let yaml_str = tokio::fs::read_to_string(&syntax_path).await?;
            let syntax_def = syntect::parsing::SyntaxDefinition::load_from_str(
                &yaml_str,
                true,
                syntax_path.file_stem().and_then(|s| s.to_str()),
            )?;
            builder.add(syntax_def);
        }

        let ps = builder.build();

        // 3. Build the ThemeSet with the TOML theme.
        let custom_theme = build_syntect_theme(&cfg);
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
