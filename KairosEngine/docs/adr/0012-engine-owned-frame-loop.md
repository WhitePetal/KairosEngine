# ADR 0012: The engine owns the frame loop; hosts plug in as `Layer`s

- **Status**: Accepted
- **Date**: 2026-11-16
- **Context**:
  - The eframe editor shell (`kairos_editor_app`) ticked the engine from
    `App::ui`, so the engine only advanced when egui decided to repaint. An
    idle window produced no engine frames at all; recording needed a
    `request_repaint()` hack to keep the loop turning, and a multi-pass egui
    frame needed a `current_pass_index() == 0` guard to tick once.
  - `kairos_editor` already owned a winit + wgpu + egui-winit loop
    (`src/runtime.rs`), which is the shape a game build needs too. Two hosts
    meant two loops, and a game build had no loop at all.
  - eframe was considered as the shared host and rejected: it does expose the
    window (`CreationContext::winit_window`) and the GPU device
    (`wgpu_render_state`), but not the winit event stream, and it owns the
    surface and the present — which is exactly the part the engine's render
    graph must own.
  - The alternative of extracting a `kairos_runtime` crate was considered and
    rejected: `kairos_engine` already depends on winit (input) and on wgpu
    (through `kairos_graphics`), so the window and the loop introduce no new
    dependency direction. A crate boundary would add a `pub use` seam, not a
    real one.
- **Decision**:
  1. **The engine is the runtime.** `kairos_engine::runtime` owns the
     `EventLoop`, the `Window`, the `RenderPipeline` (surface + present) and
     the `Engine`, and calls `Engine::update()` exactly once per
     `RedrawRequested`.
  2. **Hosts are layers.** `Layer` has four optional methods —
     `window_created`, `window_event`, `render`, `on_exit` — all defaulted, so
     a game build implements nothing. The editor implements them with egui
     state, the egui pass, and the profiler capture save.
  3. **The loop owns the engine; the layer borrows it.** `EngineRunner` keeps
     `Engine` and passes `&mut Engine` into `render`/`on_exit`; the editor no
     longer holds an engine.
  4. **Input is gated by the layer.** Window events reach the layer first;
     `EventOutcome::consumed` decides whether the engine's input also sees
     them. egui's text focus is what keeps a key press out of the game.
  5. **No dt parameter on the engine.** `Time` is advanced by the engine's own
     `First`-stage `time_system`, which samples the wall clock; the loop only
     has to keep the frames coming. Injecting a delta is a *resource* concern
     (bevy's `TimeUpdateStrategy`), not an `Engine::update` argument, and is
     not needed yet.
  6. **Pacing is continuous, and that is not an abstraction yet.** Every
     presented frame requests the next redraw, which `PresentMode::Fifo` caps
     at the display refresh rate. That is what keeps an idle editor ticking.
     A cap, a low-power idle mode, or a tick rate decoupled from the paint rate
     would be a policy inside this same loop later — not a second loop.
- **Consequences**:
  - `kairos_editor` loses `src/runtime.rs`; `main` becomes
    `runtime::run(engine, config, host::layer)`.
  - `kairos_editor_app` is off eframe as well: it is
    `runtime::run(engine, config, |_engine, proxy| host::layer(proxy))` and draws
    its panels through the same path. eframe left the workspace dependency graph
    entirely.
  - The egui half of a host — egui state, the pass, tessellation, and the egui
    render-graph node — lives **in the host that keeps it**. `kairos_editor_app`
    owns the maintained copy; the legacy `kairos_editor` keeps a private copy in
    its own `src/host.rs`. Deliberately not shared: a crate that is being retired
    must not shape the new host's architecture, and the duplicate disappears
    with the legacy crate. The engine still knows nothing about egui.
  - A game build is the same loop with a default `Layer`; it needs a non-egui
    frame-graph builder (camera pass → present), which does not exist yet.
  - `kairos_graphics` still depends on egui unconditionally (the `EguiDraw`
    graph node), so "no egui in the game build" remains a compile-weight
    question to be feature-gated there, independent of the loop.
  - FPS reporting moved from `println!` to `log::debug!`.
- **References**:
  - `kairos_engine/src/runtime.rs` (`EngineRunner`, `Layer`, `RuntimeConfig`)
  - `kairos_editor_egui/src/lib.rs` (`EguiLayer`, `UiRoot`)
  - `kairos_editor/src/host.rs` and `kairos_editor_app/src/host.rs` (the two
    hosts' `UiRoot` implementations)
  - bevy's `bevy_winit` runner + `WinitSettings`/`UpdateMode` as the pacing
    reference, and `TimeUpdateStrategy` as the manual-clock reference.
