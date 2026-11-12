//! Host-side console log: the ring buffer behind the Console panel.
//!
//! `kairos_log` owns capture and the default terminal/file sinks. The editor adds
//! this sink to mirror the same records into the UI: the app owns one
//! [`ConsoleLog`], attaches it with [`kairos_log::attach`], and exposes it to the
//! panel through [`HostView::console_lines`](crate::HostView::console_lines).
//!
//! The buffer is a host resource, not state: it is neither serializable nor
//! undoable, and the panel never owns or consumes it. Closing the console keeps
//! the history; reopening shows the same lines.
//!
//! There is no separate editor-log crate: the only consumer is the Console panel,
//! so the sink lives beside it. A second consumer (the legacy editor) can extract
//! it into `kairos_editor_log` when it migrates.

use std::{
    collections::VecDeque,
    sync::{Mutex, MutexGuard, PoisonError},
};

use kairos_log::{Record, Sink};

use crate::frame::ConsoleLine;

/// Lines kept before the oldest one is dropped.
pub const DEFAULT_CAPACITY: usize = 1000;

const TIME_FORMAT: &str = "%H:%M:%S";

/// Bounded, thread-safe console history.
pub struct ConsoleLog {
    lines: Mutex<VecDeque<ConsoleLine>>,
    capacity: usize,
}

impl ConsoleLog {
    /// Log with [`DEFAULT_CAPACITY`] lines.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Log with an explicit line cap (at least one line).
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            lines: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// Appends one record, formatted for display. Sinks call this via
    /// [`Sink::write`].
    pub fn push(&self, record: &Record) {
        let mut lines = self.lines();
        if lines.len() == self.capacity {
            lines.pop_front();
        }
        lines.push_back(ConsoleLine {
            level: record.level,
            text: record.line(TIME_FORMAT),
        });
    }

    /// Runs `f` over the current lines, oldest first.
    pub fn with_lines<R>(&self, f: impl FnOnce(&[ConsoleLine]) -> R) -> R {
        let mut lines = self.lines();
        f(lines.make_contiguous())
    }

    /// Number of lines currently kept.
    pub fn len(&self) -> usize {
        self.lines().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drops all history.
    pub fn clear(&self) {
        self.lines().clear();
    }

    fn lines(&self) -> MutexGuard<'_, VecDeque<ConsoleLine>> {
        self.lines.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for ConsoleLog {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ConsoleLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsoleLog")
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}

impl Sink for ConsoleLog {
    fn write(&self, record: &Record) {
        self.push(record);
    }
}
