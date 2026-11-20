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
//! Tests redirect that directory with [`set_style_root`] so a test run never
//! drops a `Preferences` tree into the crate it runs in.
//!
//! Colors in style structs are `kairos_color::Color` values: that crate's
//! serde impl writes displayable colors as `"#RRGGBB"` (or `"#RRGGBBAA"` when
//! translucent) and HDR/out-of-range colors as linear floats, keeping RON
//! human-editable.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Directory all component style RON files live in, relative to the working
/// directory (same convention as `Preferences/Fonts` and
/// `Preferences/Textures`).
pub const STYLE_DIR: &str = "Preferences/Styles";

thread_local! {
    /// Current thread's style root. Tests set it on their own thread, so
    /// parallel test binaries/threads cannot leak a scratch directory into each
    /// other; the editor never sets it.
    static STYLE_ROOT: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Redirect style files to `root` for the current thread.
///
/// Intended for tests and embedders that must not write into the process
/// working directory; the editor uses [`STYLE_DIR`].
#[doc(hidden)]
pub fn set_style_root(root: impl AsRef<Path>) {
    STYLE_ROOT.with(|cell| *cell.borrow_mut() = Some(root.as_ref().to_path_buf()));
}

/// The directory style files are resolved against: the redirected root in tests,
/// `Preferences/Styles` in the editor.
///
/// The one root for every style file: the component styles and the shell's
/// shared styles (`ToolBar.ron`, `ColorPicker.ron`, `EditorFonts.ron`), so one
/// redirect keeps a test run out of the working directory for all of them.
pub(crate) fn style_root() -> PathBuf {
    STYLE_ROOT
        .with(|cell| cell.borrow().clone())
        .unwrap_or_else(|| PathBuf::from(STYLE_DIR))
}

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

    /// `Preferences/Styles/<S::FILE>` (or the redirected root in tests).
    pub fn path() -> PathBuf {
        style_root().join(S::FILE)
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

    /// Load the style file, creating it from the code defaults when it does not
    /// exist yet.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct TestStyle {
        size: f32,
        label: String,
    }

    impl Style for TestStyle {
        const FILE: &'static str = "TestStyle.ron";
        fn dark() -> Self {
            Self {
                size: 1.0,
                label: "dark".to_owned(),
            }
        }
        fn light() -> Self {
            Self {
                size: 2.0,
                label: "light".to_owned(),
            }
        }
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

        let text = std::fs::read_to_string(&path).expect("read style");
        assert!(text.contains("dark:"), "{text}");
        assert!(text.contains("light:"), "{text}");

        let loaded = StyleFile::<TestStyle>::load_from(&path).expect("reload");
        assert_eq!(loaded.dark, TestStyle::dark());
        assert_eq!(loaded.light, TestStyle::light());

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The redirect is what keeps a test run from writing `Preferences/` into
    /// the crate directory; `load_or_default` still seeds the file there.
    #[test]
    fn load_or_default_seeds_the_redirected_root() {
        let root = std::env::temp_dir().join(format!(
            "kairos-style-root-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        set_style_root(&root);
        assert_eq!(StyleFile::<TestStyle>::path(), root.join("TestStyle.ron"));

        let seeded = StyleFile::<TestStyle>::load_or_default().expect("seed defaults");
        assert_eq!(seeded.dark, TestStyle::dark());
        assert_eq!(seeded.light, TestStyle::light());
        assert!(
            root.join("TestStyle.ron").is_file(),
            "load_or_default creates the RON file when it is missing"
        );

        let _ = std::fs::remove_dir_all(&root);
        set_style_root(STYLE_DIR);
    }
}
