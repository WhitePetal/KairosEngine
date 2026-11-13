//! # kairos_profile
//!
//! Kairos' runtime profiling entry point: low-overhead marker capture, a
//! structured frame model, and JSON / Chrome-trace export.
//!
//! ## What is recorded
//!
//! Profiling is **instrumentation based**, like Unity's `ProfilerMarker` /
//! `Profiler.BeginSample`. A scope records a start timestamp, an end timestamp,
//! the marker (a static name plus source location) and its parent scope. The
//! parent chain *is* the recorded call stack: there is no native stack walk in
//! the hot path, and no sampling. A future deep-profile module can add native
//! stacks, but it is deliberately not on this path (it is too expensive for a
//! frame loop).
//!
//! The recorder is [`puffin`](https://github.com/EmbarkStudios/puffin) behind
//! the [`profiling`](https://crates.io/crates/profiling) facade:
//!
//! * scopes write into a thread-local byte stream (no global lock per scope);
//! * a thread's stream is reported when its outermost scope closes;
//! * `GlobalProfiler::new_frame()` drains the reported streams and hands
//!   complete frames to sinks;
//! * a disabled scope costs one relaxed atomic load (the macros are off until
//!   recording starts).
//!
//! The `profiling` facade is already a dependency of egui, wgpu and rapier.
//! Enabling `profile-with-puffin` here therefore captures *their* instrumented
//! scopes too (layout, tessellation, render passes, physics), without patching
//! those crates.
//!
//! ## Frame lifecycle
//!
//! A host drives two calls around each UI pass:
//!
//! ```text
//! frame N
//!   begin_frame()          // flush frame N-1 (puffin needs the pass to have ended)
//!   scope!("Editor.Frame")
//!   ...
//!   end_frame(committed)   // mark whether egui committed this pass
//! ```
//!
//! `begin_frame` flushes the **previous** pass because puffin reports a thread's
//! stream only after its outermost scope closes, which happens after the pass
//! returns. The panel therefore shows frame N-1 while frame N is being drawn;
//! that one-frame latency is the price of a clean frame boundary.
//!
//! egui may run a pass twice and discard the first one. `end_frame(false)`
//! marks that pass; the sink drops it at the next flush and counts it as a
//! discarded pass. Nothing from a discarded pass reaches the structured data.
//!
//! Recording is off by default. `Profiler::set_recording(true)` requests a
//! capture; the actual scopes turn on at the next `begin_frame`, so the first
//! captured frame is complete.
//!
//! ## Structured data
//!
//! Captured frames are parsed into plain data ([`ProfileSession`], [`FrameSnapshot`],
//! [`ThreadSnapshot`], [`SampleSnapshot`], [`ScopeInfo`]) as soon as puffin
//! submits them. Samples are pre-order (parent before children) and carry
//! `depth` + `parent`, so a tree can be rebuilt without re-parsing the byte
//! stream. `analysis::hierarchy` turns a frame into the aggregated rows the
//! Profiler panel shows.
//!
//! The same model exports to:
//!
//! * pretty JSON (`ProfileSession::to_json`, `write_json`) — the panel's JSON tab;
//! * Chrome Trace Event Format (`to_chrome_trace`, `write_chrome_trace`) — open
//!   it in <https://ui.perfetto.dev> or `chrome://tracing`.
//!
//! ## Instrumentation guidelines
//!
//! * Put `scope!` in a block and let the guard drop at the end of the block:
//!   `{ kairos_profile::scope!("Load mesh"); ... }`.
//! * Keep the name static per call site. Puffin registers the first name it sees
//!   for a call site; put per-invocation detail in the second `data` argument
//!   (e.g. an asset path).
//! * Prefer coarse scopes at subsystem boundaries, then add finer ones where the
//!   hierarchy shows a cost worth explaining.
//! * The `#[function]` attribute (re-exported from `profiling`) instruments a
//!   whole function; `#[all_functions]` covers an impl block. Their expansion
//!   names `profiling::function_scope!`, so import the re-export once in that
//!   module: `use kairos_profile::profiling;`.
//!
//! ## Backend boundary
//!
//! Nothing outside this crate knows that puffin exists. Call sites use
//! `kairos_profile::scope!`; third-party crates use `profiling::scope!` directly;
//! the panel reads [`ProfileSession`]. If a different backend (tracing, Tracy,
//! Perfetto) is chosen later, the facade and the model stay.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod analysis;
mod export;
mod model;
mod profiler;

#[cfg(test)]
mod tests;

pub use analysis::{HierarchyRow, hierarchy};
pub use model::{
    FrameSnapshot, ProfileSession, SampleSnapshot, ScopeInfo, ScopeKind, ThreadSnapshot,
};
pub use profiler::{DEFAULT_CAPACITY, Profiler};

/// The `profiling` proc-macro attribute: instrument a function with a scope
/// named after the function.
pub use profiling::{all_functions, function};

/// The underlying [`profiling`] facade.
///
/// Re-exported because the `#[function]` / `#[all_functions]` expansions name
/// `profiling::function_scope!()`; importing this module once in the file that
/// uses the attributes makes that path resolve without adding `profiling` as a
/// second direct dependency. It is also the direct `profiling::scope!` entry
/// point for crates that already use the facade.
pub use profiling;

/// Implementation detail of `kairos_profile::function_scope!`.
#[doc(hidden)]
pub use profiling::function_scope as __function_scope;
/// Implementation detail of `kairos_profile::scope!`.
#[doc(hidden)]
pub use profiling::scope as __scope;

/// Open a profiling scope until the end of the enclosing block.
///
/// The one-argument form uses the name as the marker. The two-argument form
/// adds per-invocation `data` (e.g. an asset path) that is stored with each
/// sample; puffin truncates it to 127 bytes.
///
/// ```text
/// {
///     kairos_profile::scope!("Load mesh", path);
///     load(path);
/// }
/// ```
///
/// The name is registered once per call site, so it must be effectively static;
/// pass the changing part through `data`. When recording is off the macro costs
/// one atomic load and no timestamp.
///
/// The guard is hidden inside the expansion and lives to the end of the block,
/// so use a block for every nested scope.
#[macro_export]
macro_rules! scope {
    ($name:expr) => {
        $crate::__scope!($name);
    };
    ($name:expr, $data:expr) => {
        $crate::__scope!($name, $data);
    };
}

/// Open a scope named after the current function until the end of the block.
///
/// This is the macro form of `#[kairos_profile::function]` and is useful when
/// the attribute cannot be applied (closures, generated code, ...). It accepts
/// the same optional `data` argument as [`scope!`].
#[macro_export]
macro_rules! function_scope {
    () => {
        $crate::__function_scope!();
    };
    ($data:expr) => {
        $crate::__function_scope!($data);
    };
}

/// Begin a host frame and flush the previously recorded pass.
///
/// Call at the top of every UI pass / engine frame, before any `scope!`. The
/// flush is what makes the previous pass visible to the Profiler panel. When
/// recording is off the call is a no-op apart from the state checks.
pub fn begin_frame() {
    Profiler::global().begin_frame();
}

/// End a host pass and record whether it committed.
///
/// Pass `false` for passes the host will discard (egui's multi-pass layout can
/// run a pass twice); the sink drops that pass at the next flush. Does not
/// flush by itself — that is [`begin_frame`]'s job, after the pass has ended.
pub fn end_frame(committed: bool) {
    Profiler::global().end_frame(committed);
}

/// Flush the current puffin frame without starting a host pass (best effort).
///
/// Useful at shutdown: if the last pass is still pending, this hands it to the
/// sinks. An open scope is not closed, so a frame that never ended cannot be
/// flushed completely.
pub fn flush() {
    Profiler::global().flush();
}
