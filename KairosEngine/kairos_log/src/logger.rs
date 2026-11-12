use std::sync::Arc;

use crate::{Config, Level, Record, sinks};

/// The process logger. The `log` facade keeps a `&'static` reference to it.
static LOGGER: Logger = Logger;

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::max_level() && Level::from_log(metadata.level()).is_some()
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let Some(level) = Level::from_log(record.level()) else {
            return;
        };
        let captured = Record::new(level, record.target(), record.args().to_string());
        sinks::write(&captured);
    }

    fn flush(&self) {
        sinks::flush();
    }
}

/// Installs the process logger with defaults overridden by `KAIROS_LOG_*`.
///
/// See [`Config::from_env`] for the variables; use [`init_with`] to configure
/// in code.
pub fn init() -> Result<Guard, InitError> {
    init_with(Config::from_env())
}

/// Installs the process logger with an explicit config.
///
/// Only one `log` implementation can be installed per process; a second call
/// returns [`InitError::AlreadyInitialized`]. A file that cannot be opened
/// degrades to terminal-only logging instead of failing startup.
pub fn init_with(config: Config) -> Result<Guard, InitError> {
    log::set_logger(&LOGGER).map_err(|_| InitError::AlreadyInitialized)?;
    log::set_max_level(config.level.filter());

    if config.terminal {
        let _ = sinks::attach(Arc::new(sinks::TerminalSink::new()));
    }

    if let Some(path) = &config.file {
        match sinks::FileSink::open(path) {
            Ok(sink) => {
                let _ = sinks::attach(Arc::new(sink));
            }
            Err(error) => eprintln!(
                "kairos_log: cannot open log file {}: {error}; logging to the terminal only",
                path.display()
            ),
        }
    }

    Ok(Guard)
}

/// Changes the minimum level of the process logger at runtime.
pub fn set_level(level: Level) {
    log::set_max_level(level.filter());
}

/// Flushes every attached sink.
pub fn flush() {
    log::logger().flush();
}

/// Keeps the logger's sinks alive and flushes them when dropped.
///
/// Dropping the guard does **not** uninstall the logger: records emitted
/// afterwards still reach the attached sinks.
#[must_use = "keep the guard alive for the process lifetime; it flushes the sinks on drop"]
#[derive(Debug, Default)]
pub struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        flush();
    }
}

/// Errors returned by [`init`] / [`init_with`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitError {
    /// A `log` implementation (this one, `env_logger`, ...) is already installed.
    AlreadyInitialized,
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyInitialized => f.write_str("a log implementation is already installed"),
        }
    }
}

impl std::error::Error for InitError {}
