use std::io::IsTerminal;

use crate::{Level, Record, Sink};

const TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Writes every record to stderr, colored when stderr is a terminal.
///
/// stderr (not stdout) keeps log output out of piped program output, matching
/// the convention the workspace used with `env_logger`.
pub(crate) struct TerminalSink {
    color: bool,
}

impl TerminalSink {
    pub(crate) fn new() -> Self {
        Self {
            color: std::io::stderr().is_terminal(),
        }
    }
}

impl Sink for TerminalSink {
    fn write(&self, record: &Record) {
        let line = record.line(TIME_FORMAT);
        match (self.color, ansi_color(record.level)) {
            (true, Some(color)) => eprintln!("\x1b[{color}m{line}\x1b[0m"),
            _ => eprintln!("{line}"),
        }
    }
}

fn ansi_color(level: Level) -> Option<u8> {
    match level {
        Level::Info => None,
        Level::Warning => Some(33),
        Level::Error => Some(31),
    }
}
