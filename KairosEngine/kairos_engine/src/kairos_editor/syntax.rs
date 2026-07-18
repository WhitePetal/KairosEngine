// ---------------------------------------------------------------------------
// SyntaxHighlightSettings — the asset type
// ---------------------------------------------------------------------------

use std::path::PathBuf;

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
