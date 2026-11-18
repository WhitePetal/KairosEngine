//! Editor UI fonts loaded from `Preferences/Styles/EditorFonts.ron`.
//!
//! Fonts are global application resources (there is no per-panel font), but like
//! styles and icons they are **runtime files**, not `include_bytes!` data: the
//! binary stays small and a build can be re-themed by editing the RON file plus
//! the referenced font files without recompiling. The host installs them once at
//! startup, before the first frame.
//!
//! The RON shape mirrors the legacy `GlobalStyles.toml` font section, but with
//! the font list living in its own file:
//!
//! ```ron
//! (
//!     proportional: [
//!         (name: "Inter", path: "Preferences/Fonts/Inter-Regular.otf", priority: First),
//!         ...
//!     ],
//!     monospace: [],
//! )
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{Context, FontData, FontDefinitions, FontFamily};
use serde::{Deserialize, Serialize};

use crate::style;

/// Where a font sits in the fallback list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontPriority {
    First,
    Push,
}

/// One font file to load and register.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FontFile {
    /// Family name used by `FontDefinitions::font_data`.
    pub name: String,
    /// Path relative to the working directory.
    pub path: PathBuf,
    pub priority: FontPriority,
}

/// The two egui font families the editor configures.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FontsConfig {
    pub proportional: Vec<FontFile>,
    pub monospace: Vec<FontFile>,
}

impl Default for FontsConfig {
    fn default() -> Self {
        Self {
            proportional: vec![
                FontFile {
                    name: "Inter".to_owned(),
                    path: PathBuf::from("Preferences/Fonts/Inter-Regular.otf"),
                    priority: FontPriority::First,
                },
                FontFile {
                    name: "Inter Medium".to_owned(),
                    path: PathBuf::from("Preferences/Fonts/Inter-Medium.otf"),
                    priority: FontPriority::Push,
                },
            ],
            monospace: Vec::new(),
        }
    }
}

impl FontsConfig {
    /// File name under the style root (see `style::style_root`).
    pub const FILE: &'static str = "EditorFonts.ron";

    /// The font config lives beside the component style files, so it follows
    /// the same root: a test that redirects the root must not write
    /// `Preferences/Styles/EditorFonts.ron` into its working directory.
    pub fn path() -> PathBuf {
        style::style_root().join(Self::FILE)
    }

    pub fn load_from(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        ron::from_str(&text).map_err(|error| format!("parse {}: {error}", path.display()))
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|error| format!("serialize {}: {error}", path.display()))?;
        std::fs::write(path, text).map_err(|error| format!("write {}: {error}", path.display()))
    }

    /// Load the config; write code defaults on first run.
    pub fn load_or_create() -> Result<Self, String> {
        let path = Self::path();
        if path.is_file() {
            Self::load_from(&path)
        } else {
            let defaults = Self::default();
            defaults.save_to(&path)?;
            Ok(defaults)
        }
    }

    /// Read the configured font files and hand the definitions to egui.
    pub fn install(&self, ctx: &Context) -> Result<(), String> {
        let mut definitions = FontDefinitions::default();

        for (family, files) in [
            (FontFamily::Proportional, &self.proportional),
            (FontFamily::Monospace, &self.monospace),
        ] {
            for file in files {
                let bytes = std::fs::read(&file.path)
                    .map_err(|error| format!("read font {}: {error}", file.path.display()))?;
                definitions
                    .font_data
                    .insert(file.name.clone(), Arc::new(FontData::from_owned(bytes)));

                let entries = definitions.families.entry(family.clone()).or_default();
                match file.priority {
                    FontPriority::First => entries.insert(0, file.name.clone()),
                    FontPriority::Push => entries.push(file.name.clone()),
                }
            }
        }

        ctx.set_fonts(definitions);
        Ok(())
    }
}

/// Load `Preferences/Styles/EditorFonts.ron` (creating it on first run) and
/// install its fonts into the egui context.
pub fn install(ctx: &Context) -> Result<(), String> {
    FontsConfig::load_or_create()?.install(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_point_at_runtime_font_files() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let config = FontsConfig::default();
        assert!(!config.proportional.is_empty());
        for file in &config.proportional {
            assert!(
                workspace.join(&file.path).is_file(),
                "missing font {}",
                file.path.display()
            );
        }
    }

    #[test]
    fn install_registers_the_configured_font_families() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut config = FontsConfig::default();
        for file in &mut config.proportional {
            file.path = workspace.join(&file.path);
        }

        let ctx = Context::default();
        config.install(&ctx).expect("install fonts");
        let _ = ctx.run_ui(Default::default(), |ui| {
            let names: Vec<String> = ui
                .ctx()
                .fonts(|fonts| fonts.definitions().font_data.keys().cloned().collect());
            assert!(names.iter().any(|name| name == "Inter"), "{names:?}");
            assert!(names.iter().any(|name| name == "Inter Medium"), "{names:?}");
        });
    }

    #[test]
    fn config_round_trips_through_ron() {
        let config = FontsConfig::default();
        let text = ron::ser::to_string_pretty(&config, ron::ser::PrettyConfig::default()).unwrap();
        let loaded: FontsConfig = ron::from_str(&text).unwrap();
        assert_eq!(loaded.proportional.len(), config.proportional.len());
        assert_eq!(loaded.proportional[0].path, config.proportional[0].path);
    }
}
