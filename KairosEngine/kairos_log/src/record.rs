use std::{
    backtrace::{Backtrace, BacktraceStatus},
    sync::{Arc, OnceLock},
};

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
#[derive(Clone, Debug)]
pub struct Record {
    /// Wall-clock time at which the record was emitted.
    pub time: Timestamp,
    /// Severity.
    pub level: Level,
    /// Emitting module, or the macro's explicit `target:`.
    pub target: String,
    /// Formatted message.
    pub message: String,
    /// Source file of the macro call, when the `log` facade provides it.
    pub file: Option<String>,
    /// Source line of the macro call.
    pub line: Option<u32>,
    /// Captured backtrace. Only populated when the process enables backtraces
    /// (`RUST_BACKTRACE=1`); [`Record::stack_trace`] renders and trims it.
    pub stack: Option<Arc<Backtrace>>,
}

impl Record {
    /// Captures a record stamped with the current wall-clock time.
    ///
    /// No caller location or backtrace: use [`Record::capture`] (through the
    /// logger) for records that should show a stack trace in the console.
    pub fn new(level: Level, target: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            time: Timestamp::now(),
            level,
            target: target.into(),
            message: message.into(),
            file: None,
            line: None,
            stack: None,
        }
    }

    /// Record produced by the process logger: call site plus backtrace (the
    /// backtrace is only captured when `RUST_BACKTRACE` enables it).
    pub(crate) fn capture(
        level: Level,
        target: impl Into<String>,
        message: impl Into<String>,
        file: Option<&str>,
        line: Option<u32>,
    ) -> Self {
        Self {
            file: file.map(str::to_owned),
            line,
            stack: capture_stack(),
            ..Self::new(level, target, message)
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

    /// Caller location as `file:line`, when known.
    pub fn location(&self) -> Option<String> {
        let file = self.file.as_deref()?;
        Some(match self.line {
            Some(line) => format!("{file}:{line}"),
            None => file.to_owned(),
        })
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

    /// The captured backtrace as text, with the logger's own frames and the
    /// runtime entry-point tail removed: what remains is the caller's stack,
    /// which is what Unity's console shows under a log entry.
    ///
    /// Returns `None` when backtraces are disabled or nothing was captured.
    pub fn stack_trace(&self) -> Option<String> {
        let stack = self.stack.as_ref()?;
        let text = stack.to_string();
        let frames = split_frames(&text);

        // Keep the caller frames only: the logger's own frames, the `log`
        // facade, std/core/alloc and the host framework (winit/eframe/egui)
        // are noise in a console entry, exactly like Unity hides its native
        // event loop and shows the managed frames. Once the caller stack ends
        // (we hit the first trampoline/harness frame after a caller frame)
        // the rest of the runtime is dropped.
        let mut output = String::new();
        let mut started = false;
        for frame in &frames {
            if frame_is_internal(frame) {
                continue;
            }

            let symbol = strip_frame_index(frame.first().copied().unwrap_or_default());
            if symbol.is_empty() || frame_is_noise(frame) || is_trampoline(symbol) {
                if started {
                    break;
                }
                continue;
            }
            started = true;
            output.push_str(symbol);

            // Frames carry `at file:line:col` on the next line; Unity puts the
            // source link on the same line as `(at file:line)`.
            if let Some(location) = frame
                .get(1)
                .and_then(|line| line.trim().strip_prefix("at "))
                .and_then(source_location)
            {
                output.push_str(" (at ");
                output.push_str(&location);
                output.push(')');
            }
            output.push('\n');
        }

        (!output.is_empty()).then_some(output)
    }
}

fn capture_stack() -> Option<Arc<Backtrace>> {
    let backtrace = Backtrace::capture();
    (backtrace.status() == BacktraceStatus::Captured).then(|| Arc::new(backtrace))
}

/// Splits the `Display` output of a [`Backtrace`] into frames (`NN: symbol` plus
/// its continuation lines such as `at file:line`).
fn split_frames(text: &str) -> Vec<Vec<&str>> {
    let mut frames: Vec<Vec<&str>> = Vec::new();
    for line in text.lines() {
        if is_frame_header(line) {
            frames.push(vec![line]);
        } else if let Some(frame) = frames.last_mut() {
            frame.push(line);
        }
    }
    frames
}

fn is_frame_header(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some((index, _)) = trimmed.split_once(':') else {
        return false;
    };
    !index.is_empty() && index.chars().all(|c| c.is_ascii_digit())
}

fn frame_is_internal(frame: &[&str]) -> bool {
    let symbol = frame.first().copied().unwrap_or_default();
    symbol.contains("kairos_log::")
        || symbol.contains("std::backtrace")
        || symbol.contains("<std::backtrace")
        || symbol.contains("log::logger")
        || symbol.contains("log::__private_api")
}

/// Drops the `NN:` index the std `Backtrace` display prefixes to each frame.
fn strip_frame_index(line: &str) -> &str {
    let trimmed = line.trim_start();
    let Some((index, symbol)) = trimmed.split_once(':') else {
        return trimmed;
    };
    if !index.is_empty() && index.chars().all(|c| c.is_ascii_digit()) {
        symbol.trim()
    } else {
        trimmed
    }
}

/// `path:line:col` → `path:line`; the column is noise in the console.
fn source_location(location: &str) -> Option<String> {
    let mut parts = location.rsplitn(3, ':');
    let column = parts.next()?;
    let line = parts.next()?;
    let path = parts.next()?;
    if column.parse::<u32>().is_err() || line.parse::<u32>().is_err() {
        return None;
    }
    Some(format!("{}:{line}", path.trim_start_matches("./")))
}

/// Host/standard-library frames that are not useful in a console entry.
fn frame_is_noise(frame: &[&str]) -> bool {
    let symbol = strip_frame_index(frame.first().copied().unwrap_or_default());
    if symbol.starts_with("<unknown>") {
        return true;
    }
    let symbol = symbol.trim_start_matches(['<', '&']);
    const NOISE: &[&str] = &[
        "<unknown>",
        "fn(",
        "std::",
        "core::",
        "alloc::",
        "__rustc::",
        "__pthread",
        "winit::",
        "eframe::",
        "egui::",
        "epaint::",
        "wgpu::",
        "pollster::",
        "tokio::",
    ];
    NOISE.iter().any(|prefix| symbol.starts_with(prefix)) || symbol.contains("{{shim:")
}

/// Generic `FnOnce`/vtable trampolines and test-harness frames that close the
/// caller stack and lead into the runtime.
fn is_trampoline(symbol: &str) -> bool {
    symbol.contains(" as core::ops::function::Fn")
        || symbol.starts_with("test::")
        || symbol.contains("__rust_begin_short_backtrace")
        || symbol.contains("lang_start")
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

    #[test]
    fn captured_record_keeps_the_call_site() {
        let record = Record::capture(Level::Error, "test", "boom", Some("src/lib.rs"), Some(7));
        assert_eq!(record.location().as_deref(), Some("src/lib.rs:7"));
    }

    #[test]
    fn frame_splitter_starts_a_frame_only_on_indexed_symbols() {
        let text = "   0: foo::bar\n             at src/x.rs:1:2\n   1: baz\n";
        let frames = split_frames(text);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].len(), 2);
        assert_eq!(frames[1], ["   1: baz"]);
        assert!(frame_is_internal(&[
            "   0: kairos_log::logger::Logger::log"
        ]));
    }
}
