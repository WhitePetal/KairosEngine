//! Plain-data capture model.
//!
//! Times are integer nanoseconds. A frame's `start_ns` is relative to the
//! capture origin; a sample's `start_ns` is relative to its frame's start. That
//! keeps the panel's numbers small while still allowing absolute placement in
//! exports (frame start + sample start).
//!
//! Samples are stored in pre-order: a parent always precedes its children, and
//! `parent` indexes into the same thread's `samples` vector. This is the shape a
//! tree view and a timeline both want, and it can be produced without sorting.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::category::Category;

/// Version of the JSON schema produced by [`crate::ProfileSession::to_json`].
///
/// Bump when a field changes meaning or a required field is removed. Additive
/// fields do not need a bump: readers ignore unknown fields.
pub const SCHEMA_VERSION: u32 = 1;

/// What kind of call site produced a scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    /// A named `scope!` / `profiling::scope!` marker.
    Named,
    /// A function scope from `#[function]` / `function_scope!`.
    Function,
}

/// Static metadata for one marker (one syntactic call site).
///
/// Many samples share a `ScopeInfo`; frames reference it by [`SampleSnapshot::scope`].
/// Look it up in a [`ProfileSession::scopes`] table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeInfo {
    /// Puffin's scope id (non-zero, stable for the process).
    pub id: u32,
    /// Marker name. For function scopes this is the function name.
    pub name: String,
    /// `file:line` of the call site, or an empty string for user-registered
    /// scopes that did not provide a location.
    pub location: String,
    /// Whether the marker came from a named scope or a function scope.
    pub kind: ScopeKind,
    /// The module the marker declared at its call site, or the fallback the
    /// parser applied for an untagged third-party scope.
    ///
    /// The executor's shared `Kairos.System` marker is `Scripts` here; its
    /// per-invocation category is in [`SampleSnapshot::data`] and must be read
    /// with [`crate::classify_system`].
    #[serde(default)]
    pub category: Category,
}

/// One recorded span on one thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleSnapshot {
    /// Id of the marker, resolvable in [`ProfileSession::scopes`].
    pub scope: u32,
    /// Per-invocation data passed as `scope!("name", data)`, when non-empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// Start relative to the frame start, in nanoseconds.
    pub start_ns: u64,
    /// Duration in nanoseconds. Zero for spans below the clock resolution.
    pub duration_ns: u64,
    /// Nesting depth; `0` is a top-level scope on this thread.
    pub depth: u32,
    /// Index of the parent sample in this thread's `samples`, or `None` for a
    /// top-level scope. Always smaller than this sample's index.
    pub parent: Option<u32>,
}

/// The samples recorded by one thread during one frame.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSnapshot {
    /// Session-local thread id. Stable across frames for the same thread.
    pub id: u32,
    /// OS thread name, or an empty string for an unnamed thread.
    pub name: String,
    /// Pre-order samples (parent before children).
    pub samples: Vec<SampleSnapshot>,
    /// Non-fatal capture warning (malformed stream, depth/sample cap). The
    /// samples collected before the stop are still valid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

impl ThreadSnapshot {
    /// Deepest nesting level in this thread, or `0` when it has no samples.
    pub fn depth(&self) -> u32 {
        self.samples
            .iter()
            .map(|sample| sample.depth + 1)
            .max()
            .unwrap_or(0)
    }
}

/// One captured frame: every thread's samples between two frame boundaries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameSnapshot {
    /// Session-local frame ordinal, `0` for the oldest frame kept.
    pub index: u64,
    /// Puffin's process-global frame counter at capture time. Keeps a capture
    /// traceable when frames are evicted; do not use it as an array index.
    pub source_index: u64,
    /// Frame start relative to the capture origin, in nanoseconds.
    pub start_ns: u64,
    /// Frame end relative to the capture origin, in nanoseconds.
    pub end_ns: u64,
    /// Threads that reported work in this frame, ordered by [`ThreadSnapshot::id`].
    pub threads: Vec<ThreadSnapshot>,
    /// Exclusive time per category, indexed by [`Category::ALL`].
    ///
    /// Derived from `threads` once, when the frame is parsed, so the chart can
    /// read a frame's stack in O(1). Scheduler wrappers (`Kairos.Stage`,
    /// `Kairos.Engine.Frame`) contribute no time of their own — what they
    /// measure is their children's work, which is counted where it ran.
    ///
    /// Not serialized: it is a reduction of `threads`, and the JSON already
    /// carries the samples it came from.
    #[serde(default, skip_serializing)]
    pub categories: [u64; Category::COUNT],
}

impl FrameSnapshot {
    /// Wall time covered by the frame, in nanoseconds.
    pub fn duration_ns(&self) -> u64 {
        self.end_ns.saturating_sub(self.start_ns)
    }

    /// Total number of samples across every thread.
    pub fn sample_count(&self) -> usize {
        self.threads.iter().map(|thread| thread.samples.len()).sum()
    }

    /// The thread with session id `id`, if it reported this frame.
    pub fn thread(&self, id: u32) -> Option<&ThreadSnapshot> {
        self.threads.iter().find(|thread| thread.id == id)
    }

    /// Exclusive time attributed to one category, in nanoseconds.
    pub fn category_ns(&self, category: Category) -> u64 {
        self.categories[category as usize]
    }
}

/// One capture session: the frames kept plus the marker metadata they reference.
///
/// Cloning a session is cheap: frames are `Arc`s. The session is produced by
/// [`crate::Profiler::session`] and is the only type the UI needs to display
/// data. It is not serializable directly; use [`ProfileSession::to_json`] or
/// [`ProfileSession::frame_json`] (which include the scope table and the schema
/// version).
#[derive(Clone, Debug, Default)]
pub struct ProfileSession {
    /// Monotonic revision. Increment on every stored frame and on clear; a view
    /// can cache derived trees by this number.
    pub revision: u64,
    /// Timestamp origin of the capture (puffin's epoch-based clock). Frame
    /// times are relative to it.
    pub origin_ns: Option<i64>,
    /// Captured frames, oldest first.
    pub frames: Vec<Arc<FrameSnapshot>>,
    /// Marker table, sorted by [`ScopeInfo::id`].
    pub scopes: Vec<ScopeInfo>,
    /// Frames evicted because the capture capacity was reached.
    pub dropped_frames: u64,
    /// Passes the host marked as discarded (egui multi-pass) and the sink
    /// dropped before parsing.
    pub discarded_passes: u64,
    /// Thread lanes registered for this capture, in display order.
    ///
    /// A lane is a thread the host expects to see — the main thread and every
    /// worker of a task pool — registered when the capture starts rather than
    /// discovered from the frames. A pool thread that ran no task therefore
    /// keeps its row in the timeline instead of the tracks shifting.
    pub thread_lanes: Vec<String>,
}

impl ProfileSession {
    /// Whether no frame has been captured.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Number of captured frames.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Frame by session ordinal.
    pub fn frame(&self, index: usize) -> Option<&FrameSnapshot> {
        self.frames.get(index).map(Arc::as_ref)
    }

    /// Marker metadata by id. The table is sorted, so this is a binary search.
    pub fn scope(&self, id: u32) -> Option<&ScopeInfo> {
        self.scopes
            .binary_search_by_key(&id, |scope| scope.id)
            .ok()
            .map(|index| &self.scopes[index])
    }

    /// Display name for a marker id, with a readable fallback.
    pub fn scope_name(&self, id: u32) -> &str {
        self.scope(id)
            .map(|scope| scope.name.as_str())
            .unwrap_or("<unknown scope>")
    }

    /// Total samples across every kept frame.
    pub fn sample_count(&self) -> usize {
        self.frames.iter().map(|frame| frame.sample_count()).sum()
    }
}
