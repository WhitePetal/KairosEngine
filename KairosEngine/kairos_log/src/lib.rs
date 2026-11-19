//! # kairos_log
//!
//! Kairos' runtime log entry point: three levels, one process-wide logger, and
//! pluggable sinks.
//!
//! Any crate may emit a record through [`info!`], [`warn!`] or [`error!`] — no
//! logger instance is threaded through APIs. The host (the editor binary, a
//! game, a tool) calls [`init`] once at its entry point. By default every record
//! goes to **both** the terminal and a log file; extra sinks (the editor console)
//! attach with [`attach`].
//!
//! ## The `log` crate
//!
//! KairosLog is an implementation of the [`log`] facade, not a parallel macro
//! namespace. Existing `log::warn!` / `log::info!` call sites in the engine and
//! its crates keep working and reach the terminal and file sinks as soon as the
//! host calls [`init`]; no crate has to change its macros.
//!
//! ## Three levels
//!
//! [`Level`] has exactly three variants: `Info`, `Warning`, `Error`. The `log`
//! crate's `Debug` and `Trace` records are dropped on purpose — they are
//! development diagnostics, not part of the log vocabulary.
//!
//! ## Defaults and overrides
//!
//! [`Config::default`] logs at `Info` to stderr and to [`DEFAULT_LOG_FILE`]
//! (relative to the process working directory). [`init`] layers
//! `KAIROS_LOG_LEVEL` (`info` / `warning` / `error`), `KAIROS_LOG_FILE` (an empty
//! value disables the file sink) and `KAIROS_LOG_TERMINAL` (`0` disables stderr)
//! on top of those defaults; use [`init_with`] to configure in code.
//!
//! ```no_run
//! // Once, in the host entry point.
//! let _log = kairos_log::init().expect("logger installed once per process");
//!
//! kairos_log::info!(target: "kairos_editor", "editor booted");
//! kairos_log::warn!(target: "kairos_asset", "missing sidecar for {}", "hero.mesh");
//! ```
//!
//! Dropping the [`Guard`] does not uninstall the logger; it flushes the sinks,
//! so keep it alive in `main`.

#![forbid(unsafe_code)]

mod config;
mod frame;
mod level;
mod logger;
mod record;
mod sink;
mod sinks;

pub use config::{Config, DEFAULT_LOG_FILE};
pub use frame::{SourceLocation, StackFrame};
pub use level::Level;
pub use logger::{Guard, InitError, flush, init, init_with, set_level};
pub use record::Record;
pub use sink::{Sink, SinkHandle, attach};

// The three entry macros. Re-exported from `log` so call sites share the same
// facade the rest of the workspace already uses.
pub use log::{debug, error, info, warn};
