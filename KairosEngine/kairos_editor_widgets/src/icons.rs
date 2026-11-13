//! Icon *locations* for the editor chrome.
//!
//! Icons are not compiled into the binary: they are ordinary files under
//! `Preferences/Textures/` (the Console set is documented in
//! `docs/research/unity-console/README.md`), so replacing artwork or shipping a
//! different skin does not require a rebuild. A component preloads its icon set
//! in `Panel::init`; drawing goes through [`crate::ui_image`], which uses egui's
//! async image loaders.
//!
//! Icon paths are appearance data and live in the component's style struct next
//! to the palette, so the RON style file can point at a different icon set.

use serde::{Deserialize, Serialize};

/// Severity artwork the Console needs. Kept here (rather than using
/// `kairos_log::Level`) so the widget layer stays domain-agnostic; the Console
/// component maps the log level onto this enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleSeverity {
    Info,
    Warning,
    Error,
}

/// Runtime icon file paths for the Console panel, relative to the editor's
/// working directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsoleIconPaths {
    pub info: String,
    pub warning: String,
    pub error: String,
    pub info_off: String,
    pub warning_off: String,
    pub error_off: String,
    pub badge: String,
}

impl ConsoleIconPaths {
    /// The measured Unity/Tuanjie 2022.3 dark-skin icon set.
    pub fn dark() -> Self {
        Self {
            info: "Preferences/Textures/console/info.png".to_owned(),
            warning: "Preferences/Textures/console/warning.png".to_owned(),
            error: "Preferences/Textures/console/error.png".to_owned(),
            info_off: "Preferences/Textures/console/info_off.png".to_owned(),
            warning_off: "Preferences/Textures/console/warning_off.png".to_owned(),
            error_off: "Preferences/Textures/console/error_off.png".to_owned(),
            badge: "Preferences/Textures/console/count_badge.png".to_owned(),
        }
    }

    /// Light-skin active icons. Unity ships separate light artwork; the dimmed
    /// variants are grey enough to work on both skins, so they are shared.
    pub fn light() -> Self {
        Self {
            info: "Preferences/Textures/console/light/info.png".to_owned(),
            warning: "Preferences/Textures/console/light/warning.png".to_owned(),
            error: "Preferences/Textures/console/light/error.png".to_owned(),
            ..Self::dark()
        }
    }

    /// The icon for one severity, active or dimmed (filtered out).
    pub fn severity(&self, severity: ConsoleSeverity, visible: bool) -> &str {
        match (severity, visible) {
            (ConsoleSeverity::Info, true) => &self.info,
            (ConsoleSeverity::Info, false) => &self.info_off,
            (ConsoleSeverity::Warning, true) => &self.warning,
            (ConsoleSeverity::Warning, false) => &self.warning_off,
            (ConsoleSeverity::Error, true) => &self.error,
            (ConsoleSeverity::Error, false) => &self.error_off,
        }
    }

    /// The collapse repeat-count badge.
    pub fn badge(&self) -> &str {
        &self.badge
    }

    /// All paths in deterministic order, for preloading in `Panel::init`.
    pub fn all(&self) -> [&str; 7] {
        [
            &self.info,
            &self.warning,
            &self.error,
            &self.info_off,
            &self.warning_off,
            &self.error_off,
            &self.badge,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_exist(paths: ConsoleIconPaths) {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        for path in paths.all() {
            assert!(
                workspace.join(path).is_file(),
                "missing console icon {path}"
            );
        }
    }

    #[test]
    fn console_icons_are_external_files() {
        // Icons are runtime files under `Preferences/Textures`, not `include_bytes!`.
        assert_exist(ConsoleIconPaths::dark());
    }

    #[test]
    fn light_console_icons_are_external_files() {
        assert_exist(ConsoleIconPaths::light());
    }
}
