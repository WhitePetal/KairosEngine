use std::fmt;

use strum::{EnumCount, EnumIter};

/// Severity of a log record.
///
/// KairosLog has exactly three levels. The `log` crate's `Debug` and `Trace`
/// are not part of the vocabulary; see the crate docs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, EnumIter, EnumCount)]
pub enum Level {
    /// Normal operation.
    #[default]
    Info,
    /// Something unexpected that the code recovered from.
    Warning,
    /// An operation failed.
    Error,
}

impl Level {
    /// Short uppercase label used by every sink: `INFO`, `WARN`, `ERROR`.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warning => "WARN",
            Self::Error => "ERROR",
        }
    }

    /// Smallest `log` filter that admits this level and everything above it.
    pub const fn filter(self) -> log::LevelFilter {
        match self {
            Self::Info => log::LevelFilter::Info,
            Self::Warning => log::LevelFilter::Warn,
            Self::Error => log::LevelFilter::Error,
        }
    }

    /// `log` level mapped onto the Kairos vocabulary. `Debug`/`Trace` are dropped.
    pub(crate) const fn from_log(level: log::Level) -> Option<Self> {
        match level {
            log::Level::Error => Some(Self::Error),
            log::Level::Warn => Some(Self::Warning),
            log::Level::Info => Some(Self::Info),
            log::Level::Debug | log::Level::Trace => None,
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Level as LogLevel;
use strum::IntoEnumIterator;

    #[test]
    fn levels_are_ordered_by_severity() {
        assert!(Level::Info < Level::Warning);
        assert!(Level::Warning < Level::Error);
        assert_eq!(Level::iter().collect::<Vec<Level>>(), vec![Level::Info, Level::Warning, Level::Error]);
    }

    #[test]
    fn labels_match_the_vocabulary() {
        assert_eq!(Level::Info.label(), "INFO");
        assert_eq!(Level::Warning.label(), "WARN");
        assert_eq!(Level::Error.label(), "ERROR");
    }

    #[test]
    fn log_filters_follow_the_level() {
        assert_eq!(Level::Info.filter(), log::LevelFilter::Info);
        assert_eq!(Level::Warning.filter(), log::LevelFilter::Warn);
        assert_eq!(Level::Error.filter(), log::LevelFilter::Error);
    }

    #[test]
    fn from_log_maps_three_levels_and_drops_debug_and_trace() {
        assert_eq!(Level::from_log(LogLevel::Info), Some(Level::Info));
        assert_eq!(Level::from_log(LogLevel::Warn), Some(Level::Warning));
        assert_eq!(Level::from_log(LogLevel::Error), Some(Level::Error));
        assert_eq!(Level::from_log(LogLevel::Debug), None);
        assert_eq!(Level::from_log(LogLevel::Trace), None);
    }
}
