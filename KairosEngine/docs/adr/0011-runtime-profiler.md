# ADR 0011: Runtime profiler — puffin-backed capture, structured frames, data-only Profiler panel

- **Status**: Proposed
- **Date**: 2026-11-13
- **Revised**: 2026-11-16 — the recorder is an owned value (no global), the
  engine drives the game frame, and captures are saved as files.
- **Revised (2026-11-17)** — categories are declared by the marker (tagged
  scopes and `.category(...)` systems), legend rows toggle chart bands, and the
  timeline shows registered thread lanes. See the revision notes below.
- **Context**:
  - The 0.2.0 editor (`kairos_editor_app`, eframe/egui) runs a single window: a
    toolbar, the dock, and six registered panels (five open by default). The owner wants a **Profiler Panel**
    consistent with Unity's Profiler, split into two phases:
    1. record the program's execution as **structured data**;
    2. draw that data (timeline, hierarchy bars, frame chart).
    This work delivers phase 1. The panel displays the structured capture
    (Hierarchy / Raw / JSON) and deliberately draws no charts yet. The capture
    layer must also serve the engine and game later, not just the editor UI.
  - Requirements: low overhead when recording is off; opt-in recording; a
    Unity-style marker hierarchy (the recorded call stack); per-frame and
    per-thread data; an in-process panel (no external GUI or network); a stable
    serializable schema; and a path to instrument engine/game code without
    rewriting call sites if the backend changes.
  - There is no profiler today. `kairos_log` records messages, not timings.
    The workspace already depends on the `profiling` facade transitively through
    egui, wgpu and rapier, but no backend feature is enabled, so those
    `profiling::scope!` calls currently compile to nothing.
- **Revision (2026-11-16)**:
  The first implementation bound the profiler to the eframe editor shell and
  made it a process global: `AppHost` held a `&'static Profiler` from
  `Profiler::global()`, `begin_frame`/`end_frame` were free functions over that
  global, and the frame boundary was an egui pass. Three things were wrong with
  that: `kairos_engine` could not profile a game build, the global contradicted
  the project's "no global static objects" rule, and an idle window produced no
  frames because eframe repaints on events. The revised decisions:
  1. `Profiler` is an **owned value** (`Profiler::new()`), registered with puffin
     on construction and unregistered in `Drop`. There is no `Profiler::global()`,
     no `OnceLock` and no `static` in `kairos_profile`. The only process-wide
     state left is inside puffin (its macro switch and frame collector), which
     the type encapsulates; the rule is one live recorder per process, created
     by the composition root.
  2. `kairos_engine` depends on `kairos_profile`: `Engine` owns the recorder
     (`Engine::profiler`), and `Engine::update` opens the frame
     (`begin_frame` → `Kairos.Engine.Frame` → `Kairos.Stage` per stage →
     `end_frame(true)`). A development build of a game is therefore profiled by
     its own loop; the engine no longer needs the editor.
  3. The **host loop owns the frame boundary**. The frame stays open between two
     `begin_frame` calls, so editor UI drawn after the engine tick belongs to
     the same frame. The eframe shell now hosts the engine too: it calls
     `Engine::update` once per redraw (guarded by egui's `current_pass_index`
     so a multi-pass layout cannot tick the game twice) and calls
     `request_repaint()` while recording, so an idle window keeps producing
     engine frames at the display rate instead of going silent.
  4. Captures are **saved as files** through `Profiler::save_capture`:
     `capacity` frames in three formats under `Library/Profiler` (JSON schema v1,
     Chrome trace, puffin's native `.puffin` stream readable by
     `puffin_viewer`). The panel's Save button shows the absolute paths written.
- **Revision (2026-11-17)**:
  The panel became a Unity-style three-band view (toolbar, module area, timeline),
  which exposed three data-model gaps:
  1. **The category is declared, not inferred.** Puffin records no category, and
     the rule table over marker names and crates depended on a naming convention
     nothing checked. Markers now carry a `<Category>/` prefix written by
     `kairos_profile::scope!(Category::Physics, "Name")` (prefixing the marker with
     `Physics/`); the ECS executor tags
     its per-sample `data` the same way, and a system declares its module with
     `.category(Category::Physics)` at registration (`Scripts` — the game's own
     code — when it declares nothing). The vocabulary lives in the new
     `kairos_profile` crate, next to the recorder: the enum, the tag format,
     `scope!` and the fallback table for untagged third-party scopes (wgpu,
     rapier, epaint — the only component that sees their source locations is the
     parser). `kairos_ecs` therefore depends on the recorder to declare a
     system's category; that is accepted because a development build profiles
     itself and carries the recorder anyway. (A first cut put the vocabulary in
     a separate `kairos_profiling` crate; it was folded back in because the
     dependency direction it protected was not a real constraint here.)
  2. **Legend rows are toggles.** Clicking a category hides its band from the
     chart and dims its row, keeping the row and the time, like Unity's legend.
  3. **Timeline lanes are registered.** Rows come from the capture's lane
     registry — the frame thread plus every task-pool worker — not from the
     selected frame's threads, so a worker that runs nothing keeps its row. The
     engine installs a lane provider on the recorder at construction and names
     the pools (`Compute`, `Io`) so their lanes are distinguishable.
- **Decision**:
  1. **New foundation crate `kairos_profile`.** It owns the recorder, the
     plain-data capture model, hierarchy aggregation, and JSON / Chrome-trace
     export. It has no egui, ECS, engine, or editor dependency, so the engine
     and game can depend on it later. `kairos_editor_component` depends on it
     only for the model and the host handle.
  2. **Record with puffin 0.20 through the `profiling` 1.0.18 facade**
     (`profile-with-puffin`). Puffin is an instrumentation profiler built for
     games: scopes write to a thread-local byte stream, a thread reports when
     its outermost scope closes, and `GlobalProfiler::new_frame()` hands complete
     frames to sinks. Its advertised cost is ~1 ns when off (one atomic load)
     and ~60 ns per scope when on (M1). The facade is already in the dependency
     graph; enabling the feature **unifies** it for egui, wgpu and rapier, so
     their existing `profiling::scope!` calls are captured for free. `puffin` is
     reached through `profiling::puffin`, keeping facade and backend versions
     paired.
  3. **Instrumentation is marker based; the parent chain is the recorded call
     stack.** Call sites use `kairos_profile::scope!("Name")` (or
     `#[kairos_profile::function]`, importing `kairos_profile::profiling` for the
     attribute expansion); the recorder stores marker metadata,
     start/end timestamps and the parent span. There is no native stack walk in
     the hot path and no sampling. This matches Unity's `ProfilerMarker` /
     `Profiler.BeginSample` model. A future deep-profile module can add
     native stacks, but it does not belong on this path.
  4. **Frame contract.** The host loop calls `profiler.begin_frame()` once per
     frame and `profiler.end_frame(committed)` before the next boundary.
     `begin_frame` flushes the **previous** frame because puffin reports a
     thread's stream only after its outermost scope closes (after the frame
     returns). This gives a clean boundary at the cost of one frame of display
     latency. The frame stays open until the next call, so work the host draws
     after its tick belongs to the same frame. `Engine::update` owns the
     boundary for engine/game frames; a host whose frame is one discarding egui
     pass calls `end_frame(false)` and the sink drops that frame at the next
     flush. `set_recording(true)` only requests a capture; the macros are
     enabled at the next `begin_frame`, so the first captured frame is whole.
     Recording is off by default.
  5. **Structured model.** `ProfileSession` (revision + frame list + marker
     table + capture statistics) contains `FrameSnapshot`, `ThreadSnapshot`,
     `SampleSnapshot` and `ScopeInfo`. Samples are pre-order with `depth` and
     `parent`, times are integer nanoseconds, and a sample's start is relative
     to its frame while a frame's start is relative to the capture origin. This
     supports a tree view now and a timeline later without re-parsing puffin's
     byte stream. The ring keeps 300 frames by default and reports evicted
     (`dropped_frames`) and discarded passes.
  6. **Aggregation lives in the data layer.** `kairos_profile::hierarchy` folds a
     frame (optionally one thread) into Unity's Hierarchy rows: same marker +
     data under the same parent path are merged; total, self, calls and max are
     accumulated; siblings sort by total time. The panel only renders rows.
  7. **Exports are additive views, written as files.** Schema v1 pretty JSON
     (`to_json`, `write_json`, `frame_json`) is the panel's JSON tab and the
     capture loader format. Chrome Trace Event Format (`to_chrome_trace`,
     `write_chrome_trace`) opens in Perfetto/`chrome://tracing` with no Kairos
     tooling. Puffin's native capture (`write_puffin`) is the `PUF0` container
     that `puffin_viewer` reads, holding one `PFD4` record per frame; the sink
     retains the packed backend frames beside the parsed snapshots so this needs
     no second capture. `save_capture` writes
     all three under `Library/Profiler`, named `capture-YYYYMMDD-HHMMSSZ.*`, and
     returns the paths.
  8. **Panel wiring.** The app owns an `Engine` (no global, no `&'static`);
     the engine owns the `Profiler` and opens the frame in `Engine::update`,
     the UI pass only calls `Profiler::end_frame(committed)`, and
     `HostView::profiler` returns `engine.profiler()` to the panel. The panel reads `ProfileSession`, keeps only view
     state (tab, selected frame/thread, cached aggregate), and toggles
     record/clear/save through the host handle. It does no IO and knows no file
     format: `Profiler::save_capture` writes the files and returns the paths,
     which the panel displays. The Profiler is a dock tab next to Console by
     default. The app stops capture when that tab is closed (no invisible
     recording); already captured frames stay in the store for a reopen. While
     recording the app requests a repaint every pass, so an idle window records
     frames instead of waiting for an event.
  9. **Coverage in 0.2.0.** Editor shell: `Kairos.Editor.Frame`, `ToolBar`,
     `Sync`, `Dock`, `Panel` (with the panel kind as per-invocation data),
     `Pump`. Engine: `Kairos.Engine.Frame` around `Engine::update`, and
     `Kairos.Stage` per main-schedule stage (the stage label is the
     per-invocation data). Third-party `profiling::scope!` calls in egui/wgpu
     are captured by feature unification. The eframe shell hosts an engine and
     ticks it once per redraw, so its capture contains both the engine and
     editor scopes; a headless engine (`Engine::headless`) is used when no audio
     device is available.
- **Data model**:
  ~~~json
  {
    "schemaVersion": 1,
    "originNs": 0,
    "droppedFrames": 0,
    "discardedPasses": 0,
    "scopes": [
      { "id": 12, "name": "Kairos.Editor.Frame", "location": "src/app.rs:198", "kind": "named" }
    ],
    "frames": [
      {
        "index": 0,
        "sourceIndex": 41,
        "startNs": 0,
        "endNs": 16000000,
        "threads": [
          {
            "id": 0,
            "name": "main",
            "samples": [
              { "scope": 12, "startNs": 0, "durationNs": 16000000, "depth": 0, "parent": null }
            ]
          }
        ]
      }
    ]
  }
  ~~~
  `sourceIndex` is puffin's process-global frame counter; `index` is the session
  ordinal. `parent` indexes the same thread's `samples`. `data` is omitted when
  empty. Readers ignore unknown fields; removing or changing a field's meaning
  bumps `schemaVersion`.
- **Frame lifecycle**:
  ~~~text
  frame N:  profiler.begin_frame()  -> flush N-1 (puffin reported it after N-1 returned)
            Engine::update:
              scope!("Kairos.Engine.Frame")
              scope!("Kairos.Stage", stage)  per main-schedule stage
            ... the host draws UI / presents inside the same frame ...
            profiler.end_frame(committed)
  frame N+1: profiler.begin_frame() -> sink parses N, session revision++
  ~~~
  The eframe shell ticks the engine on the first egui pass and puts its
  `Kairos.Editor.Frame` around the UI pass in the same slot; while recording it
  requests a repaint after each pass, so an idle window still produces frames. The one-frame lag is visible in the panel
  as "frame N-1"; it is the price of not calling puffin's `new_frame` while a
  frame's scopes are still open.
- **Alternatives rejected**:
  - *tracing + tracing-chrome / tracing-perfetto*: a full subscriber stack and
    string/field formatting per span, no frame sink or per-thread frame model,
    and a global subscriber would collide with the existing unconnected
    `tracing` uses. A tracing bridge is the future answer for async task
    propagation; it is not the frame recorder.
  - *Tracy / Superluminal / Optick through `profiling`*: excellent external
    tools, but they need an external GUI (and sometimes a network port); the
    requirement is an in-editor panel. The `profiling` facade keeps the option
    open for a developer build, but it cannot be combined with puffin in one
    binary (one backend feature at a time).
  - *Write our own TLS recorder*: full control over marker/category metadata,
    but reimplements the thread stream, scope registration, frame sink and
    merge logic. Puffin is mature and the model/export/panel are ours either
    way; if puffin becomes a constraint the facade isolates the swap.
  - *Use `puffin_egui`'s profiler window*: it draws a flamegraph/inspector, not
    Unity's Hierarchy/Raw/Timeline views, and it owns its frame view. We use
    puffin's recorder, not its UI, and build the Unity-shaped data views.
  - *`GlobalFrameView` as the store*: it keeps recent/slowest frames and merges
    scopes, but it does not expose the commit/discard decision or raw per-thread
    pre-order samples. A small custom sink keeps those semantics explicit.
  - *Native backtraces / sampling now*: expensive per event, not frame-aligned,
    and needs debug info/symbolization. Instrumentation already answers "what
    called what" for the code we own; deep profile is a separate phase.
- **Consequences**:
  - Positive: recording is off by default and cheap when off; the marker
    hierarchy is the call stack Unity users expect; third-party egui/wgpu spans
    are captured without patching them; the structured model is backend-neutral
    and already exports to JSON and Perfetto; the panel is a pure view; the
    engine/game can adopt the same crate.
  - Negative: one frame of display latency; a scope's name is fixed at its first
    call site (per-invocation detail must go through `data`); puffin's scopes are
    thread-local, so async tasks that move threads are not nested yet; parsing
    happens in the sink at flush (holding puffin's global profiler lock), which
    is fine at editor scale but should move to a worker for very large captures;
    the profiler panel's own drawing is part of the capture (an observer cost);
    no GPU timings, allocation tracking, categories/colors or native stacks.
  - Neutral: one live recorder per process is a puffin constraint (its macro
    switch and frame collector are process-wide), so the crate exposes an owned
    value rather than a global and the composition root creates exactly one;
    adding `puffin` through `profiling` also adds its small dependency set to
    binaries that enable the new crates; enabling puffin's `serialization` adds
    `bincode` + `lz4` for the native capture format; the editor gain is
    automatic coverage of egui/wgpu.
- **Follow-ups**:
  - Phase 2 (drawing): CPU/frame chart, hierarchy bar overlays, timeline with
    per-thread tracks, selection sync, frame comparison; reuse `hierarchy` and
    `FrameSnapshot`, no capture changes.
  - Engine/game coverage: done for `Engine::update` and the schedule stages;
    game systems still instrument themselves with `#[function]` / `scope!`.
  - Async: a `tracing` (or explicit task-id) bridge so spans survive task
    migration.
  - GPU: wgpu timestamp queries as a separate track; markers cannot measure GPU
    time.
  - Capture lifecycle: saving exists (`save_capture`); loading a capture back
    into the panel, naming captures and recording from app start are open.
  - Marker metadata: categories/colors (Unity's Profiler module colors), and
    optional user-registered scopes for dynamic names.
- **References**:
  - Implementation: `kairos_profile/src/{lib,model,profiler,analysis,export}.rs`;
    engine ownership and game frame `kairos_engine/src/engine.rs`, stage scopes
    `kairos_engine/src/schedule.rs`; panel
    `kairos_editor_component/src/panels/profiler.rs`; host seam
    `kairos_editor_component/src/frame.rs`; editor frame boundary and capture
    loop `kairos_editor_app/src/app.rs`; panel-body scope
    `kairos_editor_component/src/panel/viewer.rs`.
  - Tests: `cargo test-crate kairos_profile` (capture lifecycle, discard,
    worker threads, capacity, JSON/Chrome, readable `.puffin`, saved paths,
    hierarchy, parser), `cargo test-crate kairos_engine` (game frame + stage
    scopes), `cargo test-crate kairos_editor_component` (panel render
    with/without a capture, registration/icon),
    `cargo test -p kairos_editor_app --bins --tests` (end-to-end editor frame
    capture).
  - Related: [ADR 0008](./0008-editor-ui-architecture.md) (state tree / Frame /
    UiOut), [ADR 0009](./0009-editor-panel-host-and-viewport.md) (host-owned
    resources), [ADR 0010](./0010-runtime-logging.md) (the other process-global
    host stream).
  - Roadmap and data-schema notes: [docs/plan/profiler/README.md](../plan/profiler/README.md).
