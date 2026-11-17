//! The capture recorder: puffin sink, recording state, frame parsing.
//!
//! A [`Profiler`] is a value. The composition root creates exactly one, owns it
//! for the life of the process, and hands out `&Profiler` to whoever displays
//! the capture. There is no global `Profiler`, no `OnceLock`, and no `static`
//! recorder in this crate: dropping the value removes its puffin sink, so a
//! test or a second host never inherits a recorder it did not create.
//!
//! The only process-wide state left is *inside* puffin: the `set_scopes_on`
//! macro switch and the `GlobalProfiler` frame collector. Puffin is a game
//! profiler whose scope macros take no handle, so the backend has to be
//! process-global; that constraint is encapsulated here instead of leaking out
//! as a static of ours. The practical rule is one live `Profiler` per process
//! (a second live one would receive the same frames); see the crate docs.
//!
//! The sink runs on the thread that calls [`Profiler::begin_frame`] (the host's
//! frame thread), never on a worker thread: worker threads only report their
//! thread-local stream to puffin's global collector when their outermost scope
//! closes.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs::File,
    io::{BufWriter, Write},
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

use profiling::puffin::{
    Error as PuffinError, FrameData, FrameSinkId, GlobalProfiler, NanoSecond, Reader,
    ScopeCollection, ScopeDetails, ScopeId, ScopeType, Stream, ThreadInfo, now_ns, set_scopes_on,
};

use crate::category::{
    Category, SYSTEM_MARKER, classify_marker, classify_system, is_frame_wrapper,
};
use crate::model::{
    FrameSnapshot, ProfileSession, SampleSnapshot, ScopeInfo, ScopeKind, ThreadSnapshot,
};

/// Default directory a host saves captures into (Unity-style project cache).
///
/// Relative to the process working directory, like `kairos_log`'s
/// `Library/Logs/KairosEngine.log`. A host may pass any directory to
/// [`Profiler::save_capture`].
pub const DEFAULT_CAPTURE_DIR: &str = "Library/Profiler";

/// Per-thread guard against runaway captures: a frame with more spans than this
/// is truncated (and the thread reports a warning) instead of exhausting RAM.
const MAX_SAMPLES_PER_THREAD: usize = 500_000;

/// Deepest span nesting parsed. Deeper children are skipped and flagged.
const MAX_DEPTH: u32 = 512;

/// Where a [`Profiler::save_capture`] wrote one capture.
///
/// The three files share `stem`: the structured JSON (schema v1), the Chrome
/// Trace Event Format view, and puffin's native stream. Absolute or relative
/// depends on the directory the caller passed in; the panel shows the paths it
/// gets back, so "where did it go" is never a guess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureFiles {
    /// File-name stem shared by the three files (e.g. `capture-20261116-091500Z`).
    pub stem: String,
    /// Structured capture, schema v1 ([crate::ProfileSession::to_json]).
    pub json: PathBuf,
    /// Chrome Trace Event Format ([crate::ProfileSession::to_chrome_trace]).
    pub chrome_trace: PathBuf,
    /// Puffin's native capture (a `PUF0` container), readable by `puffin_viewer`.
    pub puffin: PathBuf,
}

/// A capture recorder: one puffin sink plus the bounded structured store.
///
/// Obtain one with [`Profiler::new`]. The recorder is installed lazily on
/// construction and removed on drop; see the module docs for why this is a
/// value rather than a global.
pub struct Profiler {
    inner: Arc<Inner>,
    /// Puffin sink registration, removed in `Drop` so the recorder cannot
    /// outlive its owner inside the backend.
    sink: FrameSinkId,
}

impl std::fmt::Debug for Profiler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Profiler")
            .field("recording", &self.is_recording())
            .finish_non_exhaustive()
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Profiler {
    fn drop(&mut self) {
        // Leave the backend as we found it: stop the macros if this recorder
        // turned them on, and unregister the sink before the shared state goes
        // away (the sink holds only a `Weak`, so this is the last reference).
        if self.inner.recording.load(Ordering::Relaxed)
            || self.inner.wants_recording.load(Ordering::Relaxed)
        {
            set_scopes_on(false);
        }
        GlobalProfiler::lock().remove_sink(self.sink);
    }
}

impl Profiler {
    /// Create a recorder and register its frame sink with puffin.
    ///
    /// Call this once, at the composition root: the engine creates one for the
    /// game frame and every host (game build, editor shell) borrows it. Puffin's
    /// collector is process-wide, so two *live* recorders would receive the same
    /// frames and fight over the record switch.
    pub fn new() -> Self {
        let inner = Arc::new(Inner::default());
        let sink_inner = Arc::downgrade(&inner);
        let mut global = GlobalProfiler::lock();
        let sink = global.add_sink(Box::new(move |frame| {
            if let Some(inner) = sink_inner.upgrade() {
                inner.on_frame(frame);
            }
        }));
        // A sink added after scopes were registered must receive a full scope
        // delta once, or the first frames cannot resolve names.
        global.emit_scope_snapshot();
        Self { inner, sink }
    }

    /// Whether a capture is requested.
    ///
    /// A start request takes effect at the next [`Profiler::begin_frame`]; the
    /// UI can show the button as active immediately.
    pub fn is_recording(&self) -> bool {
        self.inner.wants_recording.load(Ordering::Relaxed)
    }

    /// Install the callback that lists the thread lanes a new capture starts
    /// with.
    ///
    /// The engine is the only component that knows its task pools: the main
    /// thread plus every worker thread's name, whether or not the worker runs
    /// during the capture. Without a provider the timeline shows only threads
    /// that reported samples.
    pub fn set_thread_lane_provider(&self, provider: fn() -> Vec<String>) {
        *self
            .inner
            .lane_provider
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(provider);
    }

    /// Register one thread lane for the current capture.
    ///
    /// Idempotent; lanes survive `clear` (they describe threads, not frames)
    /// and are replaced wholesale by the provider at the next capture start.
    pub fn register_thread_lane(&self, name: impl Into<String>) {
        let name = name.into();
        let mut store = self.lock_store();
        if !store.thread_lanes.contains(&name) {
            store.thread_lanes.push(name);
            // Lanes are part of the session; a view caching on the revision
            // (the panel) must rebuild.
            store.revision = store.revision.wrapping_add(1);
        }
    }

    /// Request a capture (`true`) or stop the current one (`false`).
    ///
    /// Starting clears the previous capture and defers enabling the macros to
    /// the next [`Profiler::begin_frame`] so the first captured frame is whole.
    /// Stopping takes effect immediately; the frame being drawn when the user
    /// pressed Stop is dropped, while already stored frames stay visible.
    pub fn set_recording(&self, recording: bool) {
        if recording {
            if self.inner.wants_recording.swap(true, Ordering::Relaxed) {
                return;
            }
        } else {
            self.inner.wants_recording.store(false, Ordering::Relaxed);
        }
        self.inner.recording.store(false, Ordering::Relaxed);
        set_scopes_on(false);
    }

    /// Begin a host frame: flush the previous frame, then honour a start request.
    ///
    /// The host loop calls this once per frame — an engine tick, a game loop
    /// iteration, a UI pass. Puffin reports a thread's stream only after the
    /// outermost scope on that thread closes, so the frame that just ended is
    /// flushed here (one frame later) and the UI reads frame N-1 while frame N
    /// is drawn. See the crate docs.
    ///
    /// The frame stays open until the next call: work the host performs after
    /// its tick (drawing the editor, presenting the frame) belongs to the same
    /// recorded frame. [`Profiler::end_frame`] only records the commit
    /// decision; it does not flush.
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

    /// Record whether the frame being drawn will be kept.
    ///
    /// Pass `false` for a frame the host will discard (egui's multi-pass
    /// layout can run a pass twice); the sink drops that frame at the next
    /// flush and counts it as a discarded pass. A host whose frame contains
    /// work it cannot discard (an engine tick followed by a discarded UI pass)
    /// must pass `true`: the frame is one unit.
    ///
    /// Does not flush by itself — that is [`Profiler::begin_frame`]'s job.
    pub fn end_frame(&self, committed: bool) {
        self.inner
            .pending_committed
            .store(committed, Ordering::Relaxed);
    }

    /// Flush the pending puffin frame without beginning a new host frame.
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
        self.lock_store().session()
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

    /// Write puffin's native capture (`.puffin`), the format `puffin_viewer` opens.
    ///
    /// The file is a `PUF0` container holding one `PFD4` record per kept frame,
    /// in capture order. The frames also appear in [`Profiler::to_json`]; this
    /// is the same capture in the backend's own container, so an external puffin
    /// tool can read it without going through Kairos.
    pub fn write_puffin(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        self.lock_store().write_puffin(path.as_ref())
    }

    /// Save the capture into `dir` as JSON, Chrome trace and `.puffin`.
    ///
    /// Creates `dir` if needed and names the three files
    /// `capture-YYYYMMDD-HHMMSSZ.*` (UTC, so the name sorts and is unambiguous).
    /// Returns the exact paths written, which is what the Profiler panel shows
    /// after a save.
    pub fn save_capture(&self, dir: impl AsRef<Path>) -> std::io::Result<CaptureFiles> {
        self.save_capture_as(dir, capture_stem())
    }

    /// [`Profiler::save_capture`] with an explicit file-name stem.
    pub fn save_capture_as(
        &self,
        dir: impl AsRef<Path>,
        stem: impl Into<String>,
    ) -> std::io::Result<CaptureFiles> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;

        let stem = stem.into();
        let files = CaptureFiles {
            json: dir.join(format!("{stem}.json")),
            chrome_trace: dir.join(format!("{stem}.trace.json")),
            puffin: dir.join(format!("{stem}.puffin")),
            stem,
        };

        // One snapshot behind all three files: a frame ingested between two
        // separate `session()` calls would make the JSON and the native stream
        // describe different frame sets. The store lock is held for the write,
        // so a concurrent frame boundary waits for this save instead of tearing
        // the capture.
        let store = self.lock_store();
        let session = store.session();
        session.write_json(&files.json)?;
        session.write_chrome_trace(&files.chrome_trace)?;
        store.write_puffin(&files.puffin)?;
        Ok(files)
    }

    fn start_capture(&self) {
        self.inner.recording.store(true, Ordering::Relaxed);
        set_scopes_on(true);
        GlobalProfiler::lock().emit_scope_snapshot();

        // Collect the lanes the host expects before taking the store lock: the
        // provider may ask subsystems about their threads.
        let lanes = self
            .inner
            .lane_provider
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|provider| provider());

        let mut store = self.lock_store();
        if let Some(lanes) = lanes {
            store.set_thread_lanes(lanes);
        }
        if store.origin_ns.is_none() {
            store.origin_ns = Some(now_ns());
        }
    }

    fn lock_store(&self) -> MutexGuard<'_, Store> {
        self.inner.lock_store()
    }
}

/// `capture-YYYYMMDD-HHMMSSZ`, the default stem for [`Profiler::save_capture`].
fn capture_stem() -> String {
    let now = jiff::Timestamp::now();
    format!("capture-{}", now.strftime("%Y%m%d-%H%M%SZ"))
}

struct Inner {
    store: Mutex<Store>,
    wants_recording: AtomicBool,
    recording: AtomicBool,
    /// Whether the frame whose data puffin will report next committed.
    pending_committed: AtomicBool,
    /// Supplies the thread lanes a capture starts with.
    ///
    /// The engine installs this: only it knows the task pools, whose threads
    /// are named and may be idle for the whole capture.
    lane_provider: Mutex<Option<fn() -> Vec<String>>>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            store: Mutex::new(Store::default()),
            wants_recording: AtomicBool::new(false),
            recording: AtomicBool::new(false),
            pending_committed: AtomicBool::new(true),
            lane_provider: Mutex::new(None),
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

        // Keep the backend's own encoding too, so `.puffin` export needs no
        // second capture. The packing feature lets the sink store it compressed.
        frame.pack();

        store.frames.push_back(Arc::new(snapshot));
        store.raw_frames.push_back(frame);
        store.revision = store.revision.wrapping_add(1);
    }
}

#[derive(Default)]
struct Store {
    frames: VecDeque<Arc<FrameSnapshot>>,
    /// The puffin frames behind `frames`, same order; kept for `.puffin`.
    raw_frames: VecDeque<Arc<FrameData>>,
    origin_ns: Option<i64>,
    revision: u64,
    dropped_frames: u64,
    discarded_passes: u64,
    next_frame_index: u64,
    next_thread_id: u32,
    thread_ids: HashMap<ThreadInfo, u32>,
    /// Thread lanes registered for the capture, in display order.
    thread_lanes: Vec<String>,
    scopes: ScopeCollection,
    fallback_scopes: BTreeMap<u32, ScopeInfo>,
}

impl Store {
    /// Clone the current capture into the plain-data view.
    fn session(&self) -> ProfileSession {
        ProfileSession {
            revision: self.revision,
            origin_ns: self.origin_ns,
            frames: self.frames.iter().cloned().collect(),
            scopes: self.scope_table(),
            dropped_frames: self.dropped_frames,
            discarded_passes: self.discarded_passes,
            thread_lanes: self.thread_lanes.clone(),
        }
    }

    /// Replace the registered lanes, keeping the first mention of each name.
    fn set_thread_lanes(&mut self, lanes: Vec<String>) {
        let mut seen = BTreeSet::new();
        self.thread_lanes = lanes
            .into_iter()
            .filter(|lane| seen.insert(lane.clone()))
            .collect();
        // Lanes are part of the session view; drop a cache keyed on the
        // revision (the panel's) even when the previous capture's frames remain.
        self.revision = self.revision.wrapping_add(1);
    }

    fn clear_frames(&mut self) {
        self.frames.clear();
        self.raw_frames.clear();
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
                        // The category tag is marker metadata, not part of the
                        // name the panel shows.
                        name: crate::strip_tag(details.name()).to_string(),
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

        // One classification per marker: its name and location are static. The
        // executor's shared `Kairos.System` marker is the exception — which
        // system ran is per-sample data.
        let scope_classes: HashMap<u32, ScopeClass> = self
            .scopes
            .scopes_by_id()
            .iter()
            .map(|(id, details)| {
                let name = details.name();
                let class = if crate::strip_tag(name) == SYSTEM_MARKER {
                    ScopeClass::System
                } else if is_frame_wrapper(name) {
                    ScopeClass::FrameWrapper
                } else {
                    ScopeClass::Fixed(classify_marker(name, &details.location()))
                };
                (id.0.get(), class)
            })
            .collect();

        let mut categories = [0u64; Category::COUNT];
        let mut threads = Vec::with_capacity(parsed.len());
        for (info, result) in parsed.drain(..) {
            let id = self.thread_id(&info);
            match result {
                Ok(thread) => threads.push(thread_snapshot(
                    id,
                    info.name,
                    thread,
                    frame_start,
                    &scope_classes,
                    &mut categories,
                )),
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
            categories,
        })
    }

    /// Write the retained puffin frames as one native capture file.
    ///
    /// The container is what `puffin_viewer` and `FrameView::read` expect: a
    /// `PUF0` magic followed by a sequence of `FrameData` records (each with its
    /// own `PFD4` header). Writing the bare `PFD4` records was the bug — the
    /// viewer rejects a file that does not start with `PUF0`.
    ///
    /// Every record is written with the full scope table rather than only the
    /// frame's delta, so the file stays readable even when the first captured
    /// frame was a discarded pass (whose scope snapshot never reached the store).
    fn write_puffin(&self, path: &Path) -> std::io::Result<()> {
        let mut writer = BufWriter::new(File::create(path)?);
        writer.write_all(b"PUF0")?;
        for frame in &self.raw_frames {
            frame
                .write_into(Some(&self.scopes), &mut writer)
                .map_err(std::io::Error::other)?;
        }
        writer.flush()
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

/// How a marker's time is attributed to a category.
#[derive(Clone, Copy, Debug)]
enum ScopeClass {
    /// The marker belongs to one category for its whole life.
    Fixed(Category),
    /// The executor's shared marker: the owner comes from the sample's data.
    System,
    /// A scheduler wrapper. It measures its children, so its own time is not
    /// attributed — it is either waiting for off-thread work (counted where
    /// that work ran) or its own bookkeeping.
    FrameWrapper,
}

/// Build one thread's snapshot, adding every sample's exclusive time to
/// `categories`.
///
/// Exclusive time is what the category stack needs: a sample's own duration
/// minus its children's, so nested markers are counted once, at the innermost
/// marker that was running.
fn thread_snapshot(
    id: u32,
    name: String,
    thread: ParsedThread,
    frame_start: NanoSecond,
    scope_classes: &HashMap<u32, ScopeClass>,
    categories: &mut [u64; Category::COUNT],
) -> ThreadSnapshot {
    // Children may appear after their parent but never before it, so one pass
    // over the pre-order samples collects every parent's child total.
    let mut children = vec![0u64; thread.samples.len()];
    for sample in &thread.samples {
        if let Some(parent) = sample.parent {
            if let Some(slot) = children.get_mut(parent as usize) {
                *slot = slot.saturating_add(sample.duration_ns.max(0) as u64);
            }
        }
    }

    // One lookup per distinct system name, not per sample.
    let mut systems: HashMap<String, Category> = HashMap::new();

    let mut samples = Vec::with_capacity(thread.samples.len());
    for (index, sample) in thread.samples.into_iter().enumerate() {
        let duration_ns = sample.duration_ns.max(0) as u64;
        let self_ns = duration_ns.saturating_sub(children[index]);
        let class = scope_classes
            .get(&sample.scope)
            .copied()
            .unwrap_or(ScopeClass::Fixed(Category::Others));
        let category = match class {
            ScopeClass::FrameWrapper => None,
            ScopeClass::Fixed(category) => Some(category),
            ScopeClass::System => {
                let system = sample.data.as_deref().unwrap_or_default();
                Some(if let Some(category) = systems.get(system) {
                    *category
                } else {
                    let category = classify_system(Some(system));
                    systems.insert(system.to_owned(), category);
                    category
                })
            }
        };
        if let Some(category) = category {
            let total = &mut categories[category as usize];
            *total = total.saturating_add(self_ns);
        }

        samples.push(SampleSnapshot {
            scope: sample.scope,
            data: sample.data,
            start_ns: (sample.start_ns - frame_start).max(0) as u64,
            duration_ns,
            depth: sample.depth,
            parent: sample.parent,
        });
    }

    ThreadSnapshot {
        id,
        name,
        samples,
        warning: thread.warning,
    }
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
