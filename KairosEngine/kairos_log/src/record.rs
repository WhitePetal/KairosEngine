use std::sync::OnceLock;

use jiff::Timestamp;

use crate::Level;

/// The host's local time zone, resolved once per process (it is read for every
/// record by every sink).
fn local_time_zone() -> &'static jiff::tz::TimeZone {
    static ZONE: OnceLock<jiff::tz::TimeZone> = OnceLock::new();
    ZONE.get_or_init(jiff::tz::TimeZone::system)
}

/// One captured log record, shared by every sink.
///
/// Formatting lives here so the terminal, the log file and the editor console
/// agree on what a record looks like; only the timestamp precision differs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// Wall-clock time at which the record was emitted.
    pub time: Timestamp,
    /// Severity.
    pub level: Level,
    /// Emitting module, or the macro's explicit `target:`.
    pub target: String,
    /// Formatted message.
    pub message: String,
}

impl Record {
    /// Captures a record stamped with the current wall-clock time.
    pub fn new(level: Level, target: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            time: Timestamp::now(),
            level,
            target: target.into(),
            message: message.into(),
        }
    }

    /// Renders the capture time in the host's local time zone using a
    /// `strftime` format, e.g. `"%Y-%m-%d %H:%M:%S"`.
    pub fn timestamp(&self, format: &str) -> String {
        self.time
            .to_zoned(local_time_zone().clone())
            .strftime(format)
            .to_string()
    }

    /// `"{time} {level} {target}: {message}"`, with `time` rendered by
    /// `time_format`.
    pub fn line(&self, time_format: &str) -> String {
        format!(
            "{} {:<5} {}: {}",
            self.timestamp(time_format),
            self.level.label(),
            self.target,
            self.message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_carries_time_level_target_and_message() {
        let record = Record::new(Level::Warning, "kairos_asset", "missing sidecar");
        let line = record.line("%Y-%m-%d");
        assert!(
            line.contains(" WARN  kairos_asset: missing sidecar"),
            "{line}"
        );
        assert_eq!(line.len(), 10 + 1 + 5 + 1 + 12 + 2 + 15);
    }

    #[test]
    fn timestamp_format_is_honoured() {
        let record = Record::new(Level::Info, "test", "hello");
        assert_eq!(record.timestamp("%Y").len(), 4);
        assert_eq!(record.timestamp("%H:%M:%S").len(), 8);
    }
}
