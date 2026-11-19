//! Structured frames of a captured backtrace.
//!
//! A console renders a frame as `symbol (at file:line)` and colours the
//! parenthesised part as a source link, so a frame keeps its symbol and source
//! position as separate fields: the renderer never searches a string for a
//! marker to know what is a link.
//!
//! Only symbolization is textual. [`std::backtrace::Backtrace`] exposes just its
//! `Display` rendering (a frame iterator is still unstable), so
//! [`Record::stack_frames`](crate::Record::stack_frames) parses that text once
//! and hands every consumer typed frames.

use std::fmt;

/// A source position reported for one frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    /// Path as the toolchain printed it: `src/main.rs` on Unix,
    /// `src\main.rs` or `C:\...` on Windows.
    pub file: String,
    /// 1-based line number, when the toolchain reported one. Backtrace
    /// symbolization always does; a record's own call site may be file-only.
    pub line: Option<u32>,
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{line}", self.file),
            None => f.write_str(&self.file),
        }
    }
}

/// One caller frame of a captured backtrace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StackFrame {
    /// Demangled, module-qualified symbol.
    pub symbol: String,
    /// Source position the toolchain resolved for this frame, when it did.
    pub source: Option<SourceLocation>,
}
