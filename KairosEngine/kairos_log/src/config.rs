use std::{env, path::PathBuf};

use crate::Level;

/// Default relative path of the run log, Unity-style under the project cache.
pub const DEFAULT_LOG_FILE: &str = "Library/Logs/KairosEngine.log";

/// Logger configuration understood by [`crate::init_with`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Records below this level are filtered out.
    pub level: Level,
    /// Write every record to stderr.
    pub terminal: bool,
    /// Write every record to this file (created/truncated at init).
    /// `None` disables the file sink.
    pub file: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            level: Level::Info,
            terminal: true,
            file: Some(PathBuf::from(DEFAULT_LOG_FILE)),
        }
    }
}

impl Config {
    /// Defaults overridden by the `KAIROS_LOG_*` environment variables.
    ///
    /// - `KAIROS_LOG_LEVEL`: `info` (default), `warning`/`warn`, `error`.
    /// - `KAIROS_LOG_FILE`: log file path; an empty value disables the file sink.
    /// - `KAIROS_LOG_TERMINAL`: `0`/`false` disables stderr.
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Some(level) = env::var("KAIROS_LOG_LEVEL")
            .ok()
            .as_deref()
            .and_then(parse_level)
        {
            config.level = level;
        }

        match env::var("KAIROS_LOG_FILE") {
            Ok(path) if path.is_empty() => config.file = None,
            Ok(path) => config.file = Some(PathBuf::from(path)),
            Err(_) => {}
        }

        if let Ok(value) = env::var("KAIROS_LOG_TERMINAL") {
            config.terminal = !matches!(value.trim().to_ascii_lowercase().as_str(), "0" | "false");
        }

        config
    }
}

/// Parses the three vocabulary names (plus `warning`), ignoring case.
fn parse_level(value: &str) -> Option<Level> {
    match value.trim().to_ascii_lowercase().as_str() {
        "info" => Some(Level::Info),
        "warning" | "warn" => Some(Level::Warning),
        "error" => Some(Level::Error),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_level_accepts_the_three_names_and_rejects_the_rest() {
        assert_eq!(parse_level("info"), Some(Level::Info));
        assert_eq!(parse_level(" INFO "), Some(Level::Info));
        assert_eq!(parse_level("warning"), Some(Level::Warning));
        assert_eq!(parse_level("warn"), Some(Level::Warning));
        assert_eq!(parse_level("error"), Some(Level::Error));
        assert_eq!(parse_level("debug"), None);
        assert_eq!(parse_level("trace"), None);
    }

    #[test]
    fn defaults_log_to_terminal_and_file_at_info() {
        let config = Config::default();
        assert_eq!(config.level, Level::Info);
        assert!(config.terminal);
        assert_eq!(
            config.file.as_deref(),
            Some(std::path::Path::new(DEFAULT_LOG_FILE))
        );
    }
}
