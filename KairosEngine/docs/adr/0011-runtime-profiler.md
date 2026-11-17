# ADR 0011: Runtime profiler — puffin-backed capture, structured frames, data-only Profiler panel

- **Status**: Proposed
- **Date**: 2026-11-13
- **Revised**: 2026-11-16 — the recorder is an owned value (no global), the
  engine drives the game frame, and captures are saved as files.
- **Revised (2026-11-17)** — categories are declared by the marker (tagged
  scopes and `.category(...)` systems), legend rows toggle chart bands, the
  timeline shows registered thread lanes, blocks report their duration, clicking
  a block opens Unity's details box, the timeline zooms and scrolls with Unity's
  gestures, and the module area and timeline split with a draggable rule. See
  the revision notes below.
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
  which exposed four data-model gaps:
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
  4. **Timeline blocks carry their category.** The parser stores each marker's
     category in `ScopeInfo` (resolved once from its tag, or from the fallback
     table for third-party scopes), and the timeline paints a block in its
     category's colour and labels it `Category · marker`. A system block is
     named by the short system name in its per-sample data rather than by the
     shared `Kairos.System` marker. The category is deliberately **per block,
     not a timeline region**: ECS systems from one module can run in parallel or
     interleave with other work on a lane, so a contiguous "Physics band" is not
     a meaningful shape. The tag/colour still makes `Surface::get_current_texture`
     read as VSync wherever it lands.
  5. **Blocks report their duration and a click selects one.** A timeline label
     is `Category · marker (duration)` — two decimals, Unity's block format — so
     a block says what it is and what it cost without a hover. Clicking a block
     selects it: a 1 px outline over its category colour, and Unity's details
     box opens under it. Selecting pins the displayed frame (Live off) because
     the box describes exactly one frame, and a capture that is still recording
     would replace that frame on the next tick.
  6. **The details box is a popup, not a pane.** It reports the block's marker
     and time, then — only when that adds information — Unity's "Current frame
     accumulated time:" lines: the marker's invocations and accumulated time on
     the clicked block's thread, and across the frame's threads. The conditions
     and number formats are copied from `ProfilerTimelineGUI.DoSelectionTooltip`
     in Unity's editor source: the section needs more than one instance in the
     frame, the thread line needs repeats on the block's thread, the frame line
     needs instances on other threads, and times print `f2` at or above 1 ms
     and `f3` below. Most blocks in this project's captures ran once on one
     thread, so most boxes are just the marker and its time — without these
     conditions every box would repeat the block's own time. Matching is by call
     site **and per-call data**, so the executor's shared `Kairos.System`
     marker is split per system. `Copy` writes that text to the clipboard
     (Unity also copies the call stack; the capture records none). `Show` is a
     placeholder until the Hierarchy / Raw views come back: Unity's menu lists
     the block there, opens its call stack and jumps to the source line.
     Clicking the same block again, or empty track, dismisses the box.
  7. **The timeline navigates like Unity's.** The three-band view exposed that
     the timeline could only show the whole frame: the row list scrolled the
     header and ruler with it, and the only horizontal scrollbar was egui's
     `ScrollArea` bar, whose thumb cannot resize. The revised timeline copies
     `ProfilerTimelineGUI` / `ZoomableArea`:
     - **Pinned chrome.** The header (view selector, Live, CPU/GPU summary) and
       the time ruler are laid out once and never scroll; the vertical bar takes
       the right edge and the horizontal bar the bottom edge, with the thread
       names outside the zoomable time area so panning leaves them put.
     - **The view is a frame fraction.** `TimelineView { pan, span }` stores the
       shown window as fractions of the frame duration, so stepping frames,
       resizing the panel or recording again keeps the zoom instead of jumping to
       an absolute time range. The zoom stops at Unity's `ZoomableArea`
       `minWidth` (0.05 ms) and at the whole frame: the capture has no
       neighbouring frames to reveal in the margins yet.
     - **Unity's gestures.** Wheel over the time area zooms around the pointer
       (`SetScaleFocused`); wheel over the thread names or the vertical bar
       scrolls the rows; wheel over the horizontal bar pans; middle-drag or
       alt-drag pans; alt+right-drag zooms; `A` fits the frame again
       (`PerformFrameAll`). Ticks are absolute frame times, so panning slides
       them and their grid lines past the pointer, and their label decimals
       follow the tick period (`FormatTickLabel`).
     - **Unity's min-max scroll bar.** egui's scroll bar is a plain thumb owned
       by a `ScrollArea` and cannot grow the resize ends, so
       `kairos_editor_widgets::min_max_scroll::MinMaxScroll` draws Unity's
       `EditorGUIExt.MinMaxScroller`: the thumb pans, a grip at each end resizes
       the window (the timeline's second zoom gesture), and clicking the trough
       snaps the thumb under the pointer. The vertical bar is its non-resizable
       form, because the timeline's vertical scale is locked like Unity's
       `vZoomLockedByDefault`.
  8. **The module area and the timeline split like Unity's.** The last piece of
     Unity's window layout was the draggable rule between the two bands.
     Unity 2022's `ProfilerWindow` hosts the charts pane and the details view in
     a UI Toolkit `TwoPaneSplitView` (`MainSplitView`): the charts pane is the
     *fixed* pane, its height persisted as
     `ProfilerWindow.MainSplitView.FixedPaneSize`. The panel therefore keeps the
     **module pane's** height, not the timeline's: `ProfilerPanel::module_height`
     starts at the skin's `module_area_height`, the splitter drags it, and the
     timeline takes the rest. The pane is a window onto a stack of **fixed-height
     module cards** — CPU Usage (legend + frame chart) and an empty GPU Usage
     card — so dragging the splitter down reveals the modules below instead of
     stretching the CPU chart, and the cards scroll (wheel or the pane's own
     vertical bar) when they overflow. The splitter stops at the card stack's
     height, so there is never dead space under the last module. Unity reaches
     the same shape whenever its modules do not fit the pane; its
     `requiresChartHeightExpansion` path only spreads leftover height when the
     pane is taller than the sum of every module's minimum height.
     Both panes keep a minimum (the module pane: its first title row and two
     legend rows; the timeline: header, ruler, one thread row and the horizontal
     bar); when the panel is shorter than both, the module pane keeps its
     minimum and the timeline gives way first, like Unity's `UpdateLayout`.
     Double-clicking the rule restores the skin's height, as the dock's
     separators and the Console already reset. The idle rule has its own token
     (`splitter_idle`) rather than the section-rule colour, so it reads as a
     handle instead of another separator.
     The rule is the new shared widget
     `kairos_editor_widgets::splitter::Splitter`: a press-anchored drag (the
     edge follows the pointer from the press, not from egui's drag threshold),
     clamping that re-anchors at a bound so dragging back moves immediately,
     hover/active tints and a resize cursor. It has two forms because the
     editor has two kinds of measured chrome: `show` allocates the strip (the
     profiler, matching Unity's split view, which reserves the drag anchor's
     space in the pane layout) and `show_in` overlays a caller-laid-out band
     (the Console's stack-trace edge, which must not move the panes it
     separates). The Console's hand-rolled splitter from the detail-pane commit
     was migrated to it, so the two panels share one drag implementation. The
     height is session state: Unity writes `FixedPaneSize` to `EditorPrefs`,
     while this editor has no per-panel layout settings file yet.
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
      { "id": 12, "name": "Kairos.Editor.Frame", "location": "src/app.rs:198", "kind": "named", "category": "editor" }
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
    no GPU timings, allocation tracking, per-marker colour overrides or native
    stacks.
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
  - Marker metadata: category tags and colours are in place; per-marker colour
    overrides and optional user-registered scopes for dynamic names remain.
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
