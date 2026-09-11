// ---------------------------------------------------------------------------
// SyntaxHighlightSettings — the asset type
// ---------------------------------------------------------------------------

use std::path::PathBuf;

use kairos_asset::next::{
    Asset, AssetLoader, AssetWorldExt, LoadContext, Reader, VisitAssetDependencies,
};
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;
use serde::Deserialize;

use crate::math;

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
    pub sublime_syntax: Option<PathBuf>,

    pub theme: SyntaxThemeSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyntaxThemeSection {
    /// Theme name, stored in the generated syntect theme.
    pub name: String,

    #[serde(default = "SyntaxThemeColorFields::default_values")]
    pub colors: SyntaxThemeColorFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyntaxThemeColorFields {
    /// Default text color.
    #[serde(default = "default_foreground")]
    pub foreground: math::Color32,
    /// Editor background color.
    #[serde(default = "default_background")]
    pub background: math::Color32,
    /// Keywords: `fn`, `let`, `if`, `return`, …
    #[serde(default = "default_keyword")]
    pub keyword: math::Color32,
    /// Types: `f32`, `vec3`, `bool`, `texture_2d`, …
    #[serde(default = "default_type")]
    pub r#type: math::Color32,
    /// Built-in functions: `dot`, `normalize`, `textureSample`, …
    #[serde(default = "default_function")]
    pub function: math::Color32,
    /// String literals.
    #[serde(default = "default_string")]
    pub string: math::Color32,
    /// Comments.
    #[serde(default = "default_comment")]
    pub comment: math::Color32,
    /// Numeric literals.
    #[serde(default = "default_number")]
    pub number: math::Color32,
    /// Attributes / decorators: `@vertex`, `@group`, `#[derive(…)]`, …
    #[serde(default = "default_attribute")]
    pub attribute: math::Color32,
    /// Built-in variables: `position`, `vertex_index`, …
    #[serde(default = "default_builtin")]
    pub builtin_variable: math::Color32,
    /// User-defined variables / identifiers.
    #[serde(default = "default_variable")]
    pub variable: math::Color32,
    /// Operators: `+`, `-`, `&&`, `==`, …
    #[serde(default = "default_operator")]
    pub operator: math::Color32,
    /// Punctuation: `{ } ( ) [ ] ; , .`
    #[serde(default = "default_punctuation")]
    pub punctuation: math::Color32,
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

fn default_foreground() -> math::Color32 {
    math::Color32::from_hex("#D0D0D0").unwrap_or_default()
}
fn default_background() -> math::Color32 {
    math::Color32::from_hex("#1E1E1E").unwrap_or_default()
}
fn default_keyword() -> math::Color32 {
    math::Color32::from_hex("#FF6464").unwrap_or_default()
}
fn default_type() -> math::Color32 {
    math::Color32::from_hex("#57A5AB").unwrap_or_default()
}
fn default_function() -> math::Color32 {
    math::Color32::from_hex("#6D93E2").unwrap_or_default()
}
fn default_string() -> math::Color32 {
    math::Color32::from_hex("#6D93E2").unwrap_or_default()
}
fn default_comment() -> math::Color32 {
    math::Color32::from_hex("#787878").unwrap_or_default()
}
fn default_number() -> math::Color32 {
    math::Color32::from_hex("#1E1E1E").unwrap_or_default()
}
fn default_attribute() -> math::Color32 {
    math::Color32::from_hex("#FFD700").unwrap_or_default()
}
fn default_builtin() -> math::Color32 {
    math::Color32::from_hex("#DCDCAA").unwrap_or_default()
}
fn default_variable() -> math::Color32 {
    math::Color32::from_hex("#909090").unwrap_or_default()
}
fn default_operator() -> math::Color32 {
    math::Color32::from_hex("#D0D0D0").unwrap_or_default()
}
fn default_punctuation() -> math::Color32 {
    math::Color32::from_hex("#C0C0C0").unwrap_or_default()
}

// ---------------------------------------------------------------------------
// TOML → syntect::highlighting::Theme
// ---------------------------------------------------------------------------

fn scope_item(scope_str: &str, color: math::Color32) -> syntect::highlighting::ThemeItem {
    syntect::highlighting::ThemeItem {
        scope: std::str::FromStr::from_str(scope_str).unwrap_or_default(),
        style: syntect::highlighting::StyleModifier {
            foreground: Some(color.into()),
            background: None,
            font_style: None,
        },
    }
}

impl SyntaxConfig {
    pub fn build_syntect_theme(&self) -> syntect::highlighting::Theme {
        let c = &self.theme.colors;

        let scopes: Vec<syntect::highlighting::ThemeItem> = vec![
            // ---- keyword ----
            scope_item("keyword", c.keyword),
            // ---- type ----
            scope_item("storage.type", c.r#type),
            scope_item("entity.name.type", c.r#type),
            scope_item("support.type", c.r#type),
            // ---- function ----
            scope_item("support.function", c.function),
            scope_item("entity.name.function", c.function),
            // ---- string ----
            scope_item("string", c.string),
            // ---- comment ----
            scope_item("comment", c.comment),
            // ---- number ----
            scope_item("constant.numeric", c.number),
            // ---- attribute / modifier ----
            scope_item("storage.modifier", c.attribute),
            scope_item("meta.annotation", c.attribute),
            scope_item("meta.attribute", c.attribute),
            // ---- built-in variable ----
            scope_item("variable.language", c.builtin_variable),
            scope_item("support.variable", c.builtin_variable),
            // ---- user variable ----
            scope_item("variable.other", c.variable),
            scope_item("variable", c.variable),
            // ---- operator ----
            scope_item("keyword.operator", c.operator),
            // ---- punctuation ----
            scope_item("punctuation", c.punctuation),
        ];

        syntect::highlighting::Theme {
            name: Some(self.theme.name.clone()),
            settings: syntect::highlighting::ThemeSettings {
                foreground: Some(c.foreground.into()),
                background: Some(c.background.into()),
                ..Default::default()
            },
            scopes,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// The `SyntaxHighlightSettings` asset (next-generation core)
// ---------------------------------------------------------------------------

/// How many syntax slots
/// [`Assets<SyntaxHighlightSettings>`](kairos_asset::next::Assets) preallocates.
///
/// Syntax settings are few and long-lived — one per language — so the store is
/// sized up front. The constant lands with the type it belongs to.
pub const SYNTAX_ASSETS_CAPACITY: usize = 8;

impl Asset for SyntaxHighlightSettings {}
impl VisitAssetDependencies for SyntaxHighlightSettings {}

/// Builds a [`SyntaxHighlightSettings`] from a per-language TOML config.
#[derive(Debug)]
pub struct SyntaxHighlightSettingsLoader;

impl AssetLoader for SyntaxHighlightSettingsLoader {
    type Asset = SyntaxHighlightSettings;
    type Settings = ();
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<SyntaxHighlightSettings, KairosError>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let toml_str = String::from_utf8(bytes)?;
            let cfg: SyntaxConfig = toml::from_str(&toml_str)?;

            // Build the SyntaxSet: always start with the built-in syntaxes, then
            // add the custom sublime-syntax the config names, if any. The path is
            // relative to the engine root (the process cwd), like every other
            // asset path in the engine.
            let default_ss = syntect::parsing::SyntaxSet::load_defaults_newlines();
            let mut builder = default_ss.into_builder();
            if let Some(syntax_path) = &cfg.sublime_syntax {
                let yaml_str = async_fs::read_to_string(syntax_path).await?;
                let syntax_def = syntect::parsing::SyntaxDefinition::load_from_str(
                    &yaml_str,
                    true,
                    syntax_path.file_stem().and_then(|s| s.to_str()),
                )?;
                builder.add(syntax_def);
            }
            let ps = builder.build();

            // Build the ThemeSet with the TOML theme, overriding every preset
            // slot so the custom theme wins regardless of which theme
            // `egui_extras::CodeTheme` selects.
            let custom_theme = cfg.build_syntect_theme();
            let mut ts = syntect::highlighting::ThemeSet::load_defaults();
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

            Ok(SyntaxHighlightSettings {
                language_name: cfg.language_name.clone(),
                settings: egui_extras::syntax_highlighting::SyntectSettings { ps, ts },
            })
        }
    }

    /// The syntax loader is resolved by asset type, never by extension: the
    /// config is a `.toml` file, which the [`Toml`](crate::kairos_editor::editor_assets::Toml)
    /// loader already claims. Claiming it here too would leave which loader an
    /// untyped `.toml` load picks up to registration order.
    fn extensions(&self) -> &[&str] {
        &[]
    }
}

/// Registers the [`SyntaxHighlightSettings`] asset and its loader with the core.
///
/// Must run after [`kairos_asset::next::install`], which creates the
/// `AssetServer` and the `AssetStages` this reads.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<SyntaxHighlightSettings>(SYNTAX_ASSETS_CAPACITY);
    world.register_asset_loader(SyntaxHighlightSettingsLoader);
}

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use kairos_asset::next::{AssetServer, Assets, install};
    use kairos_ecs::schedule::ScheduleLabel;
    use kairos_ecs::world::World;

    use super::{SyntaxHighlightSettings, install as install_syntax};

    /// The two ad-hoc stages the asset drivers are installed into.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Tracking;

    impl ScheduleLabel for Tracking {
        fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
            Box::new(*self)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Events;

    impl ScheduleLabel for Events {
        fn dyn_clone(&self) -> Box<dyn ScheduleLabel> {
            Box::new(*self)
        }
    }

    /// A syntax config load through the new core lands its settings in the store.
    #[test]
    fn syntax_loads_through_the_core() {
        // The default source is rooted at the cwd and `UnapprovedPathMode::Forbid`
        // rejects paths outside it, so the load uses a path relative to the cwd.
        let dir = tempfile::Builder::new()
            .tempdir_in(".")
            .expect("a temp dir in the cwd");
        let full_path = dir.path().join("probe_syntax.toml");
        std::fs::write(
            &full_path,
            b"language_name = \"Rust\"\n[theme]\nname = \"Probe\"\n",
        )
        .expect("write the syntax config");
        let cwd = std::env::current_dir().expect("the cwd");
        let rel_path = full_path
            .strip_prefix(&cwd)
            .expect("the temp dir is under the cwd")
            .to_path_buf();

        let mut world = World::new();
        install(&mut world, Tracking, Events);
        install_syntax(&mut world);

        let handle = world
            .resource::<AssetServer>()
            .load::<SyntaxHighlightSettings>(rel_path);

        // The loader runs on the io task pool, so pump the tracking stage until
        // its result reaches the store.
        let mut language = None;
        for _ in 0..200 {
            world.run_schedule(Tracking);
            if let Some(settings) = world
                .resource::<Assets<SyntaxHighlightSettings>>()
                .get(handle.id())
            {
                language = Some(settings.language_name.clone());
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(language.as_deref(), Some("Rust"));
    }
}
