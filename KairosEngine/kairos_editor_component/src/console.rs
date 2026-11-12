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
    sync::{
        Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};

use kairos_log::{Record, Sink};

use crate::frame::ConsoleLine;

/// Lines kept before the oldest one is dropped.
pub const DEFAULT_CAPACITY: usize = 1000;

/// Stack frames kept per entry; Unity shows the full trace, but a runaway
/// recursive stack should not make the console unusable.
const MAX_STACK_LINES: usize = 40;

const TIME_FORMAT: &str = "%H:%M:%S";

/// Bounded, thread-safe console history.
pub struct ConsoleLog {
    lines: Mutex<VecDeque<ConsoleLine>>,
    capacity: usize,
    next_id: AtomicU64,
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
            // Ids start at 1 so the panel's default clear cursor (0) keeps the
            // very first entry visible.
            next_id: AtomicU64::new(1),
        }
    }

    /// Appends one record. Sinks call this via [`Sink::write`].
    pub fn push(&self, record: &Record) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut stack: Vec<String> = record
            .stack_trace()
            .map(|trace| {
                trace
                    .lines()
                    .map(str::trim_end)
                    .filter(|line| !line.trim().is_empty())
                    .take(MAX_STACK_LINES)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        if stack.is_empty() {
            // No backtrace: keep the source of the entry visible so the panel
            // still answers "where did this come from".
            if let Some(location) = record.location() {
                stack.push(format!("{} (at {location})", record.target));
            }
        }

        let line = ConsoleLine {
            id,
            level: record.level,
            time: record.timestamp(TIME_FORMAT),
            message: record.message.clone(),
            target: record.target.clone(),
            location: record.location(),
            stack,
        };

        let mut lines = self.lines();
        if lines.len() == self.capacity {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    /// Runs `f` over the current lines, oldest first.
    pub fn with_lines<R>(&self, f: impl FnOnce(&[ConsoleLine]) -> R) -> R {
        let mut lines = self.lines();
        f(lines.make_contiguous())
    }

    /// Id of the most recent entry, or `0` when empty.
    pub fn last_id(&self) -> Option<u64> {
        self.lines().back().map(|line| line.id)
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
