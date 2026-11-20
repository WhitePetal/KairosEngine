//! Editor UI fonts: the runtime list and the install step.
//!
//! Fonts are global application resources (there is no per-panel font): a list
//! of files registered once into the egui context. The list a build uses is a
//! shared style the shell owns
//! (`kairos_editor_component::shared_style::EditorFonts`); that value persists
//! `Preferences/Styles/EditorFonts.ron`, is edited from the Preferences panel,
//! and projects onto [`FontsConfig`] here, which only knows how to hand the
//! files to egui.
//!
//! Like styles and icons the files are **runtime files**, not `include_bytes!`
//! data: the binary stays small and a build can be re-themed by editing the RON
//! file plus the referenced font files without recompiling.

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Context, FontData, FontDefinitions, FontFamily};
use serde::{Deserialize, Serialize};

/// Where a font sits in the fallback list.
///
/// The shell's font skin stores this alongside each entry, so it keeps the
/// serde derives even though the install structure itself is a runtime value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontPriority {
    /// Goes in front of its family, so it draws the editor's text.
    First,
    /// Appends to its family, so it is a fallback for glyphs the front lacks.
    Push,
}

/// One font file to load and register.
#[derive(Clone, Debug, PartialEq)]
pub struct FontFile {
    /// Family name used by `FontDefinitions::font_data`.
    pub name: String,
    /// Path relative to the working directory.
    pub path: PathBuf,
    pub priority: FontPriority,
}

/// The two egui font families the editor configures.
///
/// A runtime value, built from the shell's `EditorFonts` skin at install
/// time; it is not itself persisted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontsConfig {
    pub proportional: Vec<FontFile>,
    pub monospace: Vec<FontFile>,
}

impl FontsConfig {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_font(name: &str, file: &str) -> FontFile {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        FontFile {
            name: name.to_owned(),
            path: workspace.join("Preferences/Fonts").join(file),
            priority: FontPriority::First,
        }
    }

    #[test]
    fn install_registers_the_configured_font_families() {
        let config = FontsConfig {
            proportional: vec![
                workspace_font("Inter", "Inter-Regular.otf"),
                FontFile {
                    priority: FontPriority::Push,
                    ..workspace_font("Inter Medium", "Inter-Medium.otf")
                },
            ],
            monospace: Vec::new(),
        };

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
}
