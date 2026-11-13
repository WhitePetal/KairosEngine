//! Serializable per-component style files.
//!
//! Every UI component owns its style (colors, metrics, icon file paths). The
//! style is a plain serde type with dark/light defaults authored in code; it is
//! persisted as **one RON file per component** under `Preferences/Styles/`:
//!
//! ```ron
//! (
//!     dark: (toolbar_bg: "#353535", ...),
//!     light: (toolbar_bg: "#C8C8C8", ...),
//! )
//! ```
//!
//! The component loads the file when it is instantiated (`Panel::init`), not at
//! program startup: editing `Preferences/Styles/ConsoleStyle.ron` and reopening
//! the panel restyles it without a rebuild. If the file does not exist the
//! component writes the code defaults once, so the file is also the documented
//! starting point for a custom skin.
//!
//! Colors are serialized through [`hex_color`] as `"#RRGGBB"` (or
//! `"#RRGGBBAA"` when translucent), keeping RON human-editable.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Directory all component style RON files live in, relative to the working
/// directory (same convention as `Preferences/Fonts` and
/// `Preferences/Textures`).
pub const STYLE_DIR: &str = "Preferences/Styles";

/// One component's style: a dark/light pair plus the file it persists to.
///
/// Implementations live next to their component (e.g. the Console panel), not
/// in this crate: the style trait/helpers are shared, the values are not.
pub trait Style: Clone + Serialize + DeserializeOwned {
    /// File name under [`STYLE_DIR`], e.g. `"ConsoleStyle.ron"`.
    const FILE: &'static str;

    /// Code-authored dark skin.
    fn dark() -> Self;

    /// Code-authored light skin.
    fn light() -> Self;
}

/// The on-disk shape: one file holds both skins.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "S: Serialize", deserialize = "S: DeserializeOwned"))]
pub struct StyleFile<S> {
    pub dark: S,
    pub light: S,
}

impl<S: Style> StyleFile<S> {
    /// Code defaults for both skins (what a fresh file is seeded with).
    pub fn defaults() -> Self {
        Self {
            dark: S::dark(),
            light: S::light(),
        }
    }

    /// `Preferences/Styles/<S::FILE>`.
    pub fn path() -> PathBuf {
        Path::new(STYLE_DIR).join(S::FILE)
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

    /// Load `path`, writing the code defaults first when it does not exist.
    pub fn load_or_create_at(path: &Path) -> Result<Self, String> {
        if path.is_file() {
            Self::load_from(path)
        } else {
            let defaults = Self::defaults();
            defaults.save_to(path)?;
            Ok(defaults)
        }
    }

    /// Load `Preferences/Styles/<S::FILE>`, creating it from code defaults on
    /// first run.
    pub fn load_or_create() -> Result<Self, String> {
        Self::load_or_create_at(&Self::path())
    }

    /// Load the style file, falling back to code defaults when it does not exist
    /// yet. Used by components at init; use [`StyleFile::save_to`] (or the
    /// `export_styles` example) to materialize the defaults on disk.
    pub fn load_or_default() -> Result<Self, String> {
        let path = Self::path();
        if path.is_file() {
            Self::load_from(&path)
        } else {
            let dft = Self::defaults();
            let _ = dft.save_to(&path);
            Ok(dft)
        }
    }
}

/// Serde adapter for `Color32` as a hex string.
///
/// Use it field-by-field:
///
/// ```ignore
/// #[serde(with = "kairos_editor_widgets::style::hex_color")]
/// pub background: Color32,
/// ```
pub mod hex_color {
    use egui::Color32;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(color: &Color32, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&to_hex(*color))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Color32, D::Error> {
        let text = String::deserialize(deserializer)?;
        from_hex(&text).map_err(serde::de::Error::custom)
    }

    /// `#RRGGBB` for opaque colors, `#RRGGBBAA` for translucent ones.
    pub fn to_hex(color: Color32) -> String {
        // Color32 stores premultiplied sRGBA; serialize the unmultiplied form
        // so a translucent hex string round-trips to what a human authored.
        let [r, g, b, a] = color.to_srgba_unmultiplied();
        if a == 255 {
            format!("#{r:02X}{g:02X}{b:02X}")
        } else {
            format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
        }
    }

    /// Parse `#RRGGBB` / `#RRGGBBAA` with an optional leading `#`.
    pub fn from_hex(text: &str) -> Result<Color32, String> {
        let text = text.trim();
        let hex = text.strip_prefix('#').unwrap_or(text);
        let channel = |range: std::ops::Range<usize>| {
            u8::from_str_radix(&hex[range], 16).map_err(|_| format!("invalid hex color {text:?}"))
        };
        match hex.len() {
            6 => Ok(Color32::from_rgb(
                channel(0..2)?,
                channel(2..4)?,
                channel(4..6)?,
            )),
            8 => Ok(Color32::from_rgba_unmultiplied(
                channel(0..2)?,
                channel(2..4)?,
                channel(4..6)?,
                channel(6..8)?,
            )),
            _ => Err(format!(
                "invalid hex color {text:?}: expected #RRGGBB or #RRGGBBAA"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Color32;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TestStyle {
        #[serde(with = "hex_color")]
        color: Color32,
        size: f32,
    }

    impl Style for TestStyle {
        const FILE: &'static str = "TestStyle.ron";
        fn dark() -> Self {
            Self {
                color: Color32::from_rgb(0x12, 0x34, 0x56),
                size: 1.0,
            }
        }
        fn light() -> Self {
            Self {
                color: Color32::from_rgba_unmultiplied(0xAB, 0xCD, 0xEF, 0x80),
                size: 2.0,
            }
        }
    }

    #[test]
    fn colors_serialize_as_hex_strings() {
        let file = StyleFile::<TestStyle>::defaults();
        let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default()).unwrap();
        assert!(text.contains("\"#123456\""), "{text}");
        assert!(text.contains("\"#ABCDEF80\""), "{text}");
        assert!(!text.contains("Color32"), "{text}");
    }

    #[test]
    fn style_file_round_trips_and_seeds_defaults() {
        let dir = std::env::temp_dir().join(format!(
            "kairos-style-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let path = dir.join("TestStyle.ron");
        let _ = std::fs::remove_dir_all(&dir);

        let created = StyleFile::<TestStyle>::load_or_create_at(&path).expect("create defaults");
        assert_eq!(created.dark, TestStyle::dark());
        assert!(path.is_file());

        let loaded = StyleFile::<TestStyle>::load_from(&path).expect("reload");
        assert_eq!(loaded.light.color, TestStyle::light().color);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hex_parser_accepts_short_and_alpha_forms() {
        assert_eq!(
            hex_color::from_hex("#123456").unwrap(),
            Color32::from_rgb(0x12, 0x34, 0x56)
        );
        assert_eq!(
            hex_color::from_hex("ABCDEF80").unwrap(),
            Color32::from_rgba_unmultiplied(0xAB, 0xCD, 0xEF, 0x80)
        );
        assert!(hex_color::from_hex("#FFF").is_err());
    }
}
