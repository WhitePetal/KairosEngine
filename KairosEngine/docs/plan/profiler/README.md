# Profiler roadmap

Status: phase 1 (capture + structured data) implemented; phase 2 (drawing) not
started. The recorder is an **owned value** (no global), the engine drives the
game frame, and a capture saves as JSON / Chrome trace / puffin-native files.

## Product target

A Unity-like **Profiler Panel** inside the Kairos editor:

- record CPU work as a hierarchical marker stream (the recorded call stack);
- show it per frame and per thread;
- draw a frame chart, hierarchy bars and a timeline;
- later cover the engine and game, not only the editor UI.

The work is split at the data boundary: `kairos_profile` records and exposes
structured frames; the panel only renders them. That boundary lets the same
capture feed the in-editor view, a JSON export and a Perfetto trace.

## Phase 1 — capture + structured data (this change)

Delivered:

- **`kairos_profile` crate**: puffin-backed recorder (through the `profiling`
  facade), frame sink, pre-order samples, scope table, bounded frame ring,
  hierarchy aggregation, JSON / Chrome Trace / puffin-native exports. The
  recorder is a value (`Profiler::new`), registered with puffin on construction
  and removed on drop; there is no global profiler.
- **Frame lifecycle**: the host loop owns the boundary. `Engine::update` opens
  the game frame (`begin_frame` → `Kairos.Engine.Frame` → `Kairos.Stage` per
  schedule stage), so a development build profiles itself with no editor. Both
  hosts do this: `kairos_editor` (winit) and `kairos_editor_app` (eframe), the
  latter ticking the engine once per redraw on egui's first pass. The UI pass
  only marks the frame committed (`end_frame`), and while recording the host
  requests a repaint every pass so an idle window still produces engine frames.
  Recording is requested through `set_recording`; the macros turn on at the next
  frame so the first captured frame is whole.
- **Scopes**: engine frame and stages; editor frame, toolbar, sync, dock, panel
  body (with panel kind data), pump; third-party egui/wgpu scopes through feature
  unification.
- **`Profiler` panel**: Hierarchy / Raw / JSON views, record/clear/save, frame
  and thread selectors; docked next to Console by default.
- **Capture files**: `Profiler::save_capture` writes `capture-YYYYMMDD-HHMMSSZ`
  under `Library/Profiler/` as `.json` (schema v1), `.trace.json` (Perfetto) and
  `.puffin` (`puffin_viewer`); the panel shows the absolute paths it wrote.
- **Tests**: capture lifecycle, discarded pass, worker thread, capacity, byte
  stream parser, hierarchy self-time, JSON/trace/puffin export, saved paths,
  engine frame + stage scopes, panel render with and without a capture,
  end-to-end editor frame capture.

Not in phase 1: any drawing (charts, bars, timeline), GPU timings, allocation
tracking, native call stacks, async task propagation, loading a saved capture
back into the panel.

## Structured data

| Type | Contents |
| --- | --- |
| `ProfileSession` | revision, origin, frames (Arc), marker table, dropped/discarded counts, recording segments, origin wall clock |
| `FrameSnapshot` | session index, puffin source index, start/end ns, threads |
| `ThreadSnapshot` | session thread id, name, pre-order samples, optional warning |
| `SampleSnapshot` | marker id, optional data, frame-relative start ns, duration ns, depth, parent index |
| `ScopeInfo` | marker id, name, file:line, named/function |
| `HierarchyRow` | marker + data, depth, total/self/max ns, calls (derived view) |

Samples are pre-order: a parent always precedes its children and `parent` points
into the same thread's `samples`. A frame's times are relative to the capture
origin; samples are relative to their frame. This is enough for a tree now and a
timeline later.

JSON schema v1 is written by `ProfileSession::to_json` / `write_json`; the
panel's JSON tab shows one frame plus the scope table. Additive fields are
backwards compatible; a semantic change bumps `schemaVersion`.

`Profiler::save_capture` writes the same data in three files that share a
`capture-YYYYMMDD-HHMMSSZ` stem under `Library/Profiler/`:

| File | Reader |
| --- | --- |
| `.json` | Kairos (schema v1) — the structured capture |
| `.trace.json` | Perfetto / `chrome://tracing` |
| `.puffin` | `puffin_viewer` (puffin's native `PUF0` container of `PFD4` frame records) |

## Instrumentation rules

- Put a scope in a block: `{ kairos_profile::scope!("Load mesh"); ... }`. The
  guard drops at the end of the block.
- Keep the name static per call site. The changing part goes in the second
  argument: `kairos_profile::scope!("Load asset", path)`. Puffin registers the
  first name it sees per call site and truncates data to 127 bytes.
- Prefer subsystem boundaries first, then add fine scopes where the hierarchy
  shows a cost worth explaining.
- `#[kairos_profile::function]` instruments a whole function; `#[all_functions]`
  covers an impl block. Their expansion names `profiling::function_scope!`, so
  import `kairos_profile::profiling` once in that module.
- Do not put scopes in code that runs thousands of times per frame unless the
  cost is understood; a scope is ~60 ns when recording and ~1 ns when off.

## Phase 2 — drawing

All views consume the existing `ProfileSession` / `HierarchyRow`. The module
area's time axis needed only additive capture fields (recording segments and the
origin's wall clock), recorded on start/stop rather than per frame.

1. **Frame chart** (top strip): one CPU bar per kept frame, height ∝
   `FrameSnapshot::duration_ns`, placed on a real-clock time axis; selected frame
   highlighted; the tallest visible bar fills 80% of the chart with Unity's
   four ms/FPS guide tiers. Recording takes are laid end to end with each pause
   compressed into a labelled break band (done, see the module axis below).
   This is the Unity "CPU Usage" module plus this editor's capture-gap axis.
2. **Hierarchy**: the current rows plus a total/self bar per row (stacked or
   overlaid), sort by total/self/calls/name, search, expand/collapse. Bar widths
   are normalized to the selected frame's duration, not to the root row.
3. **Timeline**: one row per thread, samples laid out by `start_ns` and
   `duration_ns`; click selects a sample and opens its details box (below);
   colour by category (each block is labelled `Category · marker (duration)`); a
   category is per block, not a contiguous region, because ECS work can be
   parallel or interleaved. The header and ruler are pinned and the tracks
   scroll; the time axis zooms and pans with Unity's gestures (wheel around the
   pointer, middle/alt drag, `A` to fit). The module pane above the tracks is a
   scrolling list of fixed-height module cards (CPU Usage, plus an empty GPU
   Usage card) with a pinned clock ruler on top and a min-max time bar at the
   bottom: dragging the rule between the two bands reveals the modules below
   instead of stretching one chart, and a double-click restores the skin's
   height. The min-max bar zooms and pans the whole capture (the thumb pans, the
   ends zoom, the wheel zooms around the pointer), the default window is the
   most recent 300 frames and Live pins it to the newest frame, clicking a chart
   column selects the frame recorded then, dragging the chart or ruler scrubs the
   selection (the cursor line runs from the clock row through the cards, centred
   on the frame's bar), and frame transport pans the window to the selected
   frame. Hover details, Unity's click-drag range selection, `F` to
   frame a block and the ghost frames outside the selected frame are still open.
4. **Details**: Unity's block popup reports the marker, its time and the frame's
   accumulated time per marker, with `Copy` (done; `Show` is a TODO for the
   Hierarchy / Raw views) — the wider pane (location, calls, total/self/max,
   parent chain) is still open, and the JSON escape hatch stays.
5. **Interactions**: record/clear, frame slider, thread filter, follow-latest,
   capture stats; no new dependencies — egui `Painter` + `Shape::rect_filled` +
   `Sense::click_and_drag` are enough. Unity's min-max scroll bar (a drag thumb
   with a resize grip at each end) cannot be built from `ScrollArea`, so the
   shared `kairos_editor_widgets::min_max_scroll` widget draws it. The rule
   between the module area and the timeline is the shared
   `kairos_editor_widgets::splitter` widget; the Console's stack-trace edge uses
   its overlay form.
6. **Performance**: compute layout once per `(revision, frame, thread, zoom)`;
   virtualize rows; never rebuild the capture inside the paint pass.

Unity parity checklist for phase 2: Hierarchy, Raw Hierarchy, Timeline, CPU
Usage chart, frame selector, thread selector, marker details. GPU modules and
memory modules are separate phases.

## Phase 3 — coverage and depth

- **Engine**: done — `Engine::update` opens the frame and records
  `Kairos.Engine.Frame`; `run_main` records one `Kairos.Stage` scope per
  schedule stage. Systems still instrument themselves with `#[function]`.
- **Game**: same crate and the same `Profiler`; a standalone game host drives
  `Engine::update` per loop iteration, so it is profiled without the editor.
- **Async**: a `tracing` or explicit task-id bridge so spans survive task
  migration; puffin scopes are thread-local and cannot nest across an await.
- **GPU**: wgpu timestamp queries as their own track; marker scopes measure CPU
  time only.
- **Allocations**: a tracking allocator or a sampler; not part of the marker
  path.
- **Deep profile / native stacks**: opt-in native backtraces or sampling, with
  symbolization; expensive and explicitly not on the default path.
- **Capture lifecycle**: saving is done (JSON, Chrome trace, `.puffin`); loading
  a capture back into the panel, naming captures and recording from app start
  are open.
- **Marker metadata**: categories/colors (Unity module colors), user-registered
  dynamic markers.

## Inspecting a capture

1. Run the editor and open the **Profiler** tab (bottom dock, next to Console).
2. Press **● Record**. The host keeps ticking while recording, so frames keep
   arriving even if you leave the window alone. Press **■ Stop**.
3. Use **Latest** / the frame field and the thread combo; switch between
   **Hierarchy**, **Raw**, **JSON**.
4. Press **Save**. The panel shows the three absolute paths it wrote under
   `Library/Profiler/`. Open the `.trace.json` in <https://ui.perfetto.dev> or
   `chrome://tracing`, the `.puffin` in `puffin_viewer`, and the `.json` with
   any JSON reader (schema v1 is documented above).
5. A host can also call `Profiler::save_capture`, `write_json`,
   `write_chrome_trace` or `write_puffin` directly with its own directory.

The production winit editor (`kairos_editor`, engine + UI, no Profiler panel yet)
uses the development-build switch instead: run it with `KAIROS_PROFILE=1`, and
the engine records every tick; the editor writes the capture under
`Library/Profiler` and logs the JSON path on exit. This is the same engine
recorder a game build uses — no editor required for the capture itself.

## Non-goals for now

- Replacing `kairos_log`: logs are events, profiles are timings.
- A second capture implementation beside puffin: the `profiling` facade is the
  swap point if the backend changes.
- Drawing before the data model is stable: phase 1 exists to make phase 2 a
  pure renderer.
