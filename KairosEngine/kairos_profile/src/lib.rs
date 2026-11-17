//! # kairos_profile
//!
//! Kairos' runtime profiling entry point: low-overhead marker capture, a
//! structured frame model, and JSON / Chrome-trace / puffin-native export.
//!
//! The crate is host-agnostic: the engine, a game build and both editor shells
//! (winit and eframe) use the same [`Profiler`]. The engine holds the recorder
//! for its game frame; a shell borrows it through `Engine::profiler` rather
//! than creating a second one. Nothing here knows about egui, the ECS, or the
//! editor.
//!
//! ## Ownership
//!
//! A [`Profiler`] is a **value owned by the composition root**, not a global.
//! There is no `Profiler::global()`, no `static` recorder, and no `OnceLock`
//! in this crate: whoever creates the recorder owns it, hands out `&Profiler`
//! to the views that display the capture, and drops it at shutdown (which
//! removes its puffin sink). A test or a second host can therefore never
//! inherit a recorder it did not create. (A `scope!` call site does get a
//! per-site `OnceLock<ScopeId>` from puffin's macro expansion — that is marker
//! identity, not a recorder.)
//!
//! Puffin itself is process-global — its scope macros take no handle, and the
//! `GlobalProfiler` collector is a singleton — so the backend still has one
//! frame collector and one macro switch per process. That is an implementation
//! detail of the backend, encapsulated by `Profiler`: create **one live
//! recorder per process** and let everything else borrow it.
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
//! The **host loop** owns the frame boundary — not egui's repaint schedule. One
//! iteration of the loop (an engine tick, a game loop update, a UI pass) is one
//! frame:
//!
//! ```text
//! frame N
//!   begin_frame()          // flush frame N-1 (puffin reports after it ended)
//!   scope!("Kairos.Engine.Frame")
//!   ... engine work, UI, present ...
//!   end_frame(committed)   // mark whether the host will keep this frame
//! ```
//!
//! [`Profiler::begin_frame`] flushes the **previous** frame because puffin
//! reports a thread's stream only after its outermost scope closes, which
//! happens after the frame returns. The panel therefore shows frame N-1 while
//! frame N is being drawn; that one-frame latency is the price of a clean frame
//! boundary.
//!
//! The frame stays open until the next `begin_frame`, so anything the host does
//! after its tick (drawing the editor, presenting the swapchain) lands in the
//! same recorded frame. `kairos_engine::Engine` opens the frame in
//! `Engine::update` for exactly this reason: a game frame is the engine tick
//! plus whatever the host draws around it.
//!
//! A host whose frame is one discarding pass (egui multi-pass layout) calls
//! `end_frame(false)`; the sink drops that frame at the next flush and counts
//! it as a discarded pass. A host whose frame also contains an engine tick must
//! pass `true`: the frame is one unit.
//!
//! Recording is off by default. [`Profiler::set_recording`] requests a capture;
//! the actual scopes turn on at the next `begin_frame`, so the first captured
//! frame is complete.
//!
//! ## Structured data
//!
//! Captured frames are parsed into plain data ([`ProfileSession`], [`FrameSnapshot`],
//! [`ThreadSnapshot`], [`SampleSnapshot`], [`ScopeInfo`]) as soon as puffin
//! submits them. Samples are pre-order (parent before children) and carry
//! `depth` + `parent`, so a tree can be rebuilt without re-parsing the byte
//! stream. [`analysis::hierarchy`] turns a frame into the aggregated rows the
//! Profiler panel shows.
//!
//! The same model exports to:
//!
//! * pretty JSON ([`ProfileSession::to_json`], `write_json`) — the panel's JSON tab;
//! * Chrome Trace Event Format ([`ProfileSession::to_chrome_trace`],
//!   `write_chrome_trace`) — open it in <https://ui.perfetto.dev> or
//!   `chrome://tracing`;
//! * puffin's native frame stream (`Profiler::write_puffin`) — open it with
//!   `puffin_viewer`.
//!
//! ## Capture files
//!
//! [`Profiler::save_capture`] writes all three formats into one directory and
//! returns the exact paths. The default directory is [`DEFAULT_CAPTURE_DIR`]
//! (`Library/Profiler`), and the files share a `capture-YYYYMMDD-HHMMSSZ` stem:
//!
//! ```text
//! Library/Profiler/
//!   capture-20261116-091500Z.json         # structured schema v1
//!   capture-20261116-091500Z.trace.json   # Perfetto / chrome://tracing
//!   capture-20261116-091500Z.puffin       # puffin_viewer
//! ```
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
mod category;
mod export;
mod model;
mod profiler;

#[cfg(test)]
mod tests;

pub use analysis::{HierarchyRow, hierarchy};
pub use category::{Category, SYSTEM_MARKER, is_frame_wrapper};
pub use model::{
    FrameSnapshot, ProfileSession, SampleSnapshot, ScopeInfo, ScopeKind, ThreadSnapshot,
};
pub use profiler::{CaptureFiles, DEFAULT_CAPTURE_DIR, Profiler};

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
