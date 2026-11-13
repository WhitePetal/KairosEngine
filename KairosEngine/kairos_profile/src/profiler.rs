//! The process-global recorder: puffin sink, recording state, frame parsing.
//!
//! One [`Profiler`] exists per process ([`Profiler::global`]). It installs a
//! puffin frame sink once, keeps the bounded capture in [`Store`], and exposes
//! the plain-data [`ProfileSession`]. The sink runs on the thread that calls
//! [`Profiler::begin_frame`] (the host's UI/frame thread), never on a worker
//! thread: worker threads only report their thread-local stream to puffin's
//! global collector when their outermost scope closes.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    num::NonZeroU32,
    path::Path,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use profiling::puffin::{
    Error as PuffinError, FrameData, GlobalProfiler, NanoSecond, Reader, ScopeCollection,
    ScopeDetails, ScopeId, ScopeType, Stream, ThreadInfo, now_ns, set_scopes_on,
};

use crate::model::{
    FrameSnapshot, ProfileSession, SampleSnapshot, ScopeInfo, ScopeKind, ThreadSnapshot,
};

/// Frames kept by default (about five seconds at 60 Hz).
pub const DEFAULT_CAPACITY: usize = 300;

/// Per-thread guard against runaway captures: a frame with more spans than this
/// is truncated (and the thread reports a warning) instead of exhausting RAM.
const MAX_SAMPLES_PER_THREAD: usize = 500_000;

/// Deepest span nesting parsed. Deeper children are skipped and flagged.
const MAX_DEPTH: u32 = 512;

/// Process-global profiling entry point.
///
/// Obtain it with [`Profiler::global`]. The recorder is installed lazily on the
/// first use; the sink lives for the process, matching `kairos_log`'s
/// one-logger-per-process model.
pub struct Profiler {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Profiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Profiler")
            .field("recording", &self.is_recording())
            .field("capacity", &self.capacity())
            .finish_non_exhaustive()
    }
}

impl Profiler {
    /// The process-wide profiler, installing the puffin sink on first use.
    pub fn global() -> &'static Profiler {
        static GLOBAL: OnceLock<Profiler> = OnceLock::new();
        GLOBAL.get_or_init(Profiler::new)
    }

    fn new() -> Self {
        let inner = Arc::new(Inner::default());
        let sink_inner = Arc::clone(&inner);
        {
            let mut global = GlobalProfiler::lock();
            global.add_sink(Box::new(move |frame| sink_inner.on_frame(frame)));
            // A sink added after scopes were registered must receive a full
            // scope delta once, or the first frames cannot resolve names.
            global.emit_scope_snapshot();
        }
        Self { inner }
    }

    /// Whether a capture is requested.
    ///
    /// A start request takes effect at the next [`Profiler::begin_frame`]; the
    /// UI can show the button as active immediately.
    pub fn is_recording(&self) -> bool {
        self.inner.wants_recording.load(Ordering::Relaxed)
    }

    /// Request a capture (`true`) or stop the current one (`false`).
    ///
    /// Starting clears the previous capture and defers enabling the macros to
    /// the next [`Profiler::begin_frame`] so the first captured frame is whole.
    /// Stopping takes effect immediately; the pass being drawn when the user
    /// pressed Stop is dropped, while already stored frames stay visible.
    pub fn set_recording(&self, recording: bool) {
        if recording {
            if self.inner.wants_recording.swap(true, Ordering::Relaxed) {
                return;
            }
            self.inner.recording.store(false, Ordering::Relaxed);
            set_scopes_on(false);
            self.lock_store().clear_frames();
        } else {
            self.inner.wants_recording.store(false, Ordering::Relaxed);
            self.inner.recording.store(false, Ordering::Relaxed);
            set_scopes_on(false);
        }
    }

    /// Begin a host frame: flush the previous pass, then honour a start request.
    ///
    /// Puffin reports a thread's stream only after the pass's outermost scope
    /// closes, so the previous pass is flushed here (one pass later) and the
    /// panel reads frame N-1 while frame N is drawn. See the crate docs.
    pub fn begin_frame(&self) {
        let recording = self.inner.recording.load(Ordering::Relaxed);
        let wants = self.inner.wants_recording.load(Ordering::Relaxed);

        if recording || wants {
            GlobalProfiler::lock().new_frame();
        }

        if wants && !recording {
            self.start_capture();
        }
    }

    /// End a host pass and record whether it committed.
    ///
    /// Pass `false` for a pass the host discards; the sink drops that frame at
    /// the next flush. Does not flush.
    pub fn end_frame(&self, committed: bool) {
        self.inner
            .pending_committed
            .store(committed, Ordering::Relaxed);
    }

    /// Flush the pending puffin frame without beginning a new host pass.
    ///
    /// Best effort at shutdown: an outermost scope that never closed cannot be
    /// flushed.
    pub fn flush(&self) {
        if self.inner.recording.load(Ordering::Relaxed)
            || self.inner.wants_recording.load(Ordering::Relaxed)
        {
            GlobalProfiler::lock().new_frame();
        }
    }

    /// Drop every captured frame while keeping the marker table and thread ids.
    ///
    /// If a capture is active its origin is reset, so the next frame starts near
    /// zero again.
    pub fn clear(&self) {
        let mut store = self.lock_store();
        store.clear_frames();
        if self.inner.recording.load(Ordering::Relaxed) {
            store.origin_ns = Some(now_ns());
        }
    }

    /// Snapshot of the current capture. Cheap: frames are reference counted.
    pub fn session(&self) -> ProfileSession {
        let store = self.lock_store();
        ProfileSession {
            revision: store.revision,
            origin_ns: store.origin_ns,
            frames: store.frames.iter().cloned().collect(),
            scopes: store.scope_table(),
            dropped_frames: store.dropped_frames,
            discarded_passes: store.discarded_passes,
        }
    }

    /// One captured frame by session ordinal.
    pub fn frame(&self, index: usize) -> Option<Arc<FrameSnapshot>> {
        self.lock_store().frames.get(index).cloned()
    }

    /// Pretty JSON for one frame plus the full marker table (the panel's JSON
    /// tab). `None` when `index` is out of range.
    pub fn frame_json(&self, index: usize) -> Option<String> {
        self.session().frame_json(index)
    }

    /// Pretty JSON for the whole capture.
    pub fn to_json(&self) -> String {
        self.session().to_json()
    }

    /// Write the whole capture as pretty JSON.
    pub fn write_json(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        self.session().write_json(path)
    }

    /// Chrome Trace Event Format JSON for every kept frame.
    pub fn chrome_trace(&self) -> String {
        self.session().to_chrome_trace()
    }

    /// Write a Chrome Trace Event Format JSON file.
    pub fn write_chrome_trace(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        self.session().write_chrome_trace(path)
    }

    /// Maximum number of frames kept before the oldest is evicted.
    pub fn capacity(&self) -> usize {
        self.inner.capacity.load(Ordering::Relaxed)
    }

    /// Set the frame ring capacity (clamped to at least one) and evict now if
    /// the new capacity is smaller.
    pub fn set_capacity(&self, frames: usize) {
        let capacity = frames.max(1);
        self.inner.capacity.store(capacity, Ordering::Relaxed);
        let mut store = self.lock_store();
        while store.frames.len() > capacity {
            store.frames.pop_front();
            store.dropped_frames += 1;
        }
    }

    fn start_capture(&self) {
        self.inner.recording.store(true, Ordering::Relaxed);
        set_scopes_on(true);
        GlobalProfiler::lock().emit_scope_snapshot();
        let mut store = self.lock_store();
        if store.origin_ns.is_none() {
            store.origin_ns = Some(now_ns());
        }
    }

    fn lock_store(&self) -> MutexGuard<'_, Store> {
        self.inner.lock_store()
    }
}

struct Inner {
    store: Mutex<Store>,
    wants_recording: AtomicBool,
    recording: AtomicBool,
    /// Whether the pass whose data puffin will report next committed.
    pending_committed: AtomicBool,
    capacity: AtomicUsize,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            store: Mutex::new(Store::default()),
            wants_recording: AtomicBool::new(false),
            recording: AtomicBool::new(false),
            pending_committed: AtomicBool::new(true),
            capacity: AtomicUsize::new(DEFAULT_CAPACITY),
        }
    }
}

impl Inner {
    fn lock_store(&self) -> MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Puffin frame sink: runs on the host frame thread at `new_frame`.
    fn on_frame(&self, frame: Arc<FrameData>) {
        if !self.recording.load(Ordering::Relaxed) {
            return;
        }
        if !self.pending_committed.load(Ordering::Relaxed) {
            self.lock_store().discarded_passes += 1;
            return;
        }

        let mut store = self.lock_store();
        store.ingest(&frame);

        let Some(snapshot) = store.build_snapshot(&frame) else {
            return;
        };

        let capacity = self.capacity.load(Ordering::Relaxed).max(1);
        while store.frames.len() >= capacity {
            store.frames.pop_front();
            store.dropped_frames += 1;
        }
        store.frames.push_back(Arc::new(snapshot));
        store.revision = store.revision.wrapping_add(1);
    }
}

#[derive(Default)]
struct Store {
    frames: VecDeque<Arc<FrameSnapshot>>,
    origin_ns: Option<i64>,
    revision: u64,
    dropped_frames: u64,
    discarded_passes: u64,
    next_frame_index: u64,
    next_thread_id: u32,
    thread_ids: HashMap<ThreadInfo, u32>,
    scopes: ScopeCollection,
    fallback_scopes: BTreeMap<u32, ScopeInfo>,
}

impl Store {
    fn clear_frames(&mut self) {
        self.frames.clear();
        self.origin_ns = None;
        self.dropped_frames = 0;
        self.discarded_passes = 0;
        self.next_frame_index = 0;
        self.revision = self.revision.wrapping_add(1);
    }

    /// Merge a frame's newly registered scope metadata.
    fn ingest(&mut self, frame: &FrameData) {
        for details in &frame.scope_delta {
            self.scopes.insert(Arc::clone(details));
        }
    }

    fn scope_table(&self) -> Vec<ScopeInfo> {
        let mut table: BTreeMap<u32, ScopeInfo> = self
            .scopes
            .scopes_by_id()
            .iter()
            .map(|(id, details)| {
                let id = id.0.get();
                (
                    id,
                    ScopeInfo {
                        id,
                        name: details.name().to_string(),
                        location: details.location(),
                        kind: scope_kind(details.scope_type()),
                    },
                )
            })
            .collect();
        for (id, info) in &self.fallback_scopes {
            table.entry(*id).or_insert_with(|| info.clone());
        }
        table.into_values().collect()
    }

    fn build_snapshot(&mut self, frame: &FrameData) -> Option<FrameSnapshot> {
        let unpacked = frame.unpacked().ok()?;

        let (frame_start, frame_end) = frame.meta().range_ns;
        let origin = *self.origin_ns.get_or_insert(frame_start);

        let mut parsed: Vec<(ThreadInfo, Result<ParsedThread, PuffinError>)> = unpacked
            .thread_streams
            .iter()
            .map(|(info, stream_info)| (info.clone(), parse_stream(&stream_info.stream)))
            .collect();
        if parsed.is_empty() {
            return None;
        }

        // Names for scopes that were not in a delta (normally only possible if
        // the capture started between a scope's registration and its report).
        let mut unknown: BTreeSet<u32> = BTreeSet::new();
        for (_, result) in &parsed {
            if let Ok(thread) = result {
                for sample in &thread.samples {
                    if self.scope_details(sample.scope).is_none() {
                        unknown.insert(sample.scope);
                    }
                }
            }
        }
        for id in unknown {
            self.fallback_scopes
                .entry(id)
                .or_insert_with(|| fallback_scope(id));
        }

        let mut threads = Vec::with_capacity(parsed.len());
        for (info, result) in parsed.drain(..) {
            let id = self.thread_id(&info);
            match result {
                Ok(thread) => threads.push(ThreadSnapshot {
                    id,
                    name: info.name,
                    samples: thread
                        .samples
                        .into_iter()
                        .map(|sample| SampleSnapshot {
                            scope: sample.scope,
                            data: sample.data,
                            start_ns: (sample.start_ns - frame_start).max(0) as u64,
                            duration_ns: sample.duration_ns.max(0) as u64,
                            depth: sample.depth,
                            parent: sample.parent,
                        })
                        .collect(),
                    warning: thread.warning,
                }),
                Err(error) => threads.push(ThreadSnapshot {
                    id,
                    name: info.name,
                    samples: Vec::new(),
                    warning: Some(format!("{error:?}")),
                }),
            }
        }

        threads.sort_by_key(|thread| thread.id);
        let index = self.next_frame_index;
        self.next_frame_index = self.next_frame_index.saturating_add(1);
        let start_ns = (frame_start - origin).max(0) as u64;
        let end_ns = (frame_end - origin).max(0) as u64;

        Some(FrameSnapshot {
            index,
            source_index: frame.meta().frame_index,
            start_ns,
            end_ns: end_ns.max(start_ns),
            threads,
        })
    }

    fn scope_details(&self, id: u32) -> Option<&Arc<ScopeDetails>> {
        self.scopes.fetch_by_id(&ScopeId(NonZeroU32::new(id)?))
    }

    fn thread_id(&mut self, info: &ThreadInfo) -> u32 {
        if let Some(&id) = self.thread_ids.get(info) {
            return id;
        }
        let id = self.next_thread_id;
        self.next_thread_id = self.next_thread_id.saturating_add(1);
        self.thread_ids.insert(info.clone(), id);
        id
    }
}

#[derive(Debug)]
struct RawSample {
    scope: u32,
    data: Option<String>,
    start_ns: NanoSecond,
    duration_ns: NanoSecond,
    depth: u32,
    parent: Option<u32>,
}

struct ParsedThread {
    samples: Vec<RawSample>,
    warning: Option<String>,
}

/// Parse one thread's puffin byte stream into pre-order samples.
fn parse_stream(stream: &Stream) -> Result<ParsedThread, PuffinError> {
    let mut samples = Vec::new();
    let mut truncated = false;
    parse_range(
        stream,
        0,
        stream.len() as u64,
        0,
        None,
        &mut samples,
        &mut truncated,
    )?;
    let warning = truncated
        .then(|| format!("truncated at {MAX_SAMPLES_PER_THREAD} samples or depth {MAX_DEPTH}"));
    Ok(ParsedThread { samples, warning })
}

fn parse_range(
    stream: &Stream,
    begin: u64,
    end: u64,
    depth: u32,
    parent: Option<u32>,
    out: &mut Vec<RawSample>,
    truncated: &mut bool,
) -> Result<(), PuffinError> {
    let mut position = begin;
    while position < end {
        if out.len() >= MAX_SAMPLES_PER_THREAD {
            *truncated = true;
            break;
        }

        let mut reader = Reader::with_offset(stream, position)?;
        let Some(scope) = reader.next() else {
            break;
        };
        let scope = scope?;

        let index = out.len() as u32;
        let data = if scope.record.data.is_empty() {
            None
        } else {
            Some(scope.record.data.to_owned())
        };
        out.push(RawSample {
            scope: scope.id.0.get(),
            data,
            start_ns: scope.record.start_ns,
            duration_ns: scope.record.duration_ns,
            depth,
            parent,
        });

        if depth < MAX_DEPTH {
            parse_range(
                stream,
                scope.child_begin_position,
                scope.child_end_position,
                depth + 1,
                Some(index),
                out,
                truncated,
            )?;
        } else if scope.child_end_position > scope.child_begin_position {
            *truncated = true;
        }

        let next = scope.next_sibling_position;
        if next <= position {
            break;
        }
        position = next;
    }
    Ok(())
}

fn scope_kind(kind: ScopeType) -> ScopeKind {
    match kind {
        ScopeType::Function => ScopeKind::Function,
        ScopeType::Named => ScopeKind::Named,
    }
}

fn fallback_scope(id: u32) -> ScopeInfo {
    ScopeInfo {
        id,
        name: format!("scope #{id}"),
        location: String::new(),
        kind: ScopeKind::Named,
    }
}
#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use profiling::puffin::{ScopeId, Stream};

    use super::parse_stream;

    fn scope_id(value: u32) -> ScopeId {
        ScopeId(NonZeroU32::new(value).expect("non-zero scope id"))
    }

    #[test]
    fn parser_rebuilds_depth_parent_data_and_timing() {
        let mut stream = Stream::default();
        let (outer, _) = stream.begin_scope(|| 100, scope_id(1), "");
        let (inner, _) = stream.begin_scope(|| 110, scope_id(2), "mesh");
        stream.end_scope(inner, 140);
        stream.end_scope(outer, 200);

        let parsed = parse_stream(&stream).expect("parse");
        assert_eq!(parsed.samples.len(), 2);
        assert!(parsed.warning.is_none());

        let outer = &parsed.samples[0];
        assert_eq!(outer.scope, 1);
        assert_eq!(outer.depth, 0);
        assert_eq!(outer.parent, None);
        assert_eq!(outer.start_ns, 100);
        assert_eq!(outer.duration_ns, 100);

        let inner = &parsed.samples[1];
        assert_eq!(inner.scope, 2);
        assert_eq!(inner.depth, 1);
        assert_eq!(inner.parent, Some(0));
        assert_eq!(inner.data.as_deref(), Some("mesh"));
        assert_eq!(inner.start_ns, 110);
        assert_eq!(inner.duration_ns, 30);
    }
}
