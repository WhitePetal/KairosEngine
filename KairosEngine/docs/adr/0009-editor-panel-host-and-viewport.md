# ADR 0009: Editor panels are a registry of trait objects; 3D viewports are host-owned render targets bound to egui

- **Status**: Proposed
- **Date**: 2026-11-09
- **Context**: The runnable UI prototype (`prototypes/editor-ui/`, ADR 0008) was written
  when the UI was two panels, so `EditorApp` owned them directly
  (`prototypes/editor-ui/crates/editor-app/src/app.rs:34-35`: `project: ProjectPanel,
  inspector: InspectorPanel`) and the central "viewport" was an empty-state placeholder.
  The final editor has many panels (project, hierarchy, inspector, console, scene, game,
  material/texture/mesh previews, preferences, plugin panels, ...) and a wgpu-rendered 3D
  scene view embedded in egui. If the prototype keeps per-panel fields and never
  demonstrates the egui/wgpu seam, it stops being a guide for the real editor: every new
  panel would edit a central struct, and the engine/UI render boundary would be designed
  for the first time during the migration.
- **Decision**:
  1. **Panels are trait objects created by a registry, never fields on `EditorApp`.**
     `EditorApp` is the *composition root*: it registers panel factories once and owns no
     panel-typed fields. `PanelHost` owns the live instances and the dock placement;
     panels read an immutable `Frame` (state + host view) and push intents into `UiOut`.
     ```rust
     pub trait Panel {
         fn kind(&self) -> PanelId;
         fn title(&self, frame: &Frame) -> String;
         fn draw(&mut self, ui: &mut egui::Ui, frame: &Frame, out: &mut UiOut);
         fn closable(&self) -> bool { true }
         fn wants_continuous_repaint(&self) -> bool { false }
     }
     ```
  2. **Panel identity is data, not a closed enum.** `PanelId(&'static str)` is opaque to
     the core (open/focus sets only) and lives in `kairos_editor_core`; panel *kinds* are
     contributed by feature modules. `PanelInstanceId { kind, serial }` exists from day one
     so multi-instance panels (two Inspectors, lock-to-selection, previews) do not require
     a layout redesign. **Adding a panel is: implement `Panel`, register a factory, done**
     — no edit to `editor-core`, no edit to `EditorApp`, no new `Message` variant.
  3. **Layout ownership split.** Which panels are open and which has focus is domain data
     (`core::LayoutState`) because it is persisted and queryable (MCP); the dock *tree*,
     tab order, split ratios and drag state are host/UI view state. The prototype's
     `PanelHost` keeps a four-slot layout (left / center / right / bottom) with tab bars;
     the split/floating dock widget from today's `kairos_editor` is a drop-in replacement
     behind the same `PanelHost` interface, not a reason to change the panel API.
  4. **3D viewport = host-owned offscreen render target bound to egui.** The engine render
     graph renders the scene into a viewport attachment; the host registers that texture
     with egui (`GraphicsCommand::bind_attachment_to_egui` in
     `kairos_graphics/src/graphics_graph/graphics_command.rs`,
     `kairos_graphics/src/render_pipeline.rs`), and the panel only draws an
     `egui::TextureId`. UI code never sees wgpu types (crate boundary, not convention).
     Multiple viewports (scene/game/material preview) are a registry keyed by
     `core::ViewportId`; each has its own attachment/camera binding, and the same
     `bind_attachment_to_egui` node type serves all of them.
  5. **Viewport interaction travels the same unidirectional path as every other panel.**
     The panel converts egui pointer/keyboard state into `ViewportAction` intents
     (`Ensure/Orbit/Pan/Zoom/Pick`); the reducer validates tickets and size, and emits
     `Effect::{CreateViewport, SyncViewportSize, DestroyViewport, ViewportCameraInput,
     PickViewport}`; the host applies them to the engine camera / render target and routes
     completions back through `EffectOutcome`. The **camera pose stays in the engine
     `World`** (`EditorCameraController`), never duplicated in the store; the UI only
     renders the texture it is handed. In the real frame loop, camera intents are applied
     in the `BeforeEngineUpdate` phase, so the visible latency is one frame — modelled
     explicitly, not inherited by accident.
  6. **Picking is host/engine work with a UI-only contract.** The prototype's CPU ray cast
     proves the request/completion path; production uses the engine (GPU id-buffer
     readback or a scene query), keeping `PickViewport -> SceneObjectPicked` unchanged.
     Ray casting in a panel is forbidden — panels have no camera.
  7. **Explicit frame phases** replace the old implicit `render_ui -> handle_ui -> draw_ui`
     ordering: `BeginFrame -> ApplyIntents -> EngineUpdate -> RenderViewports -> BuildUi
     -> dispatch UI actions -> execute effects -> Present`. `BuildUi` is multi-pass safe:
     only the last pass commits `UiOut`.
  8. **Alternatives rejected for the scene viewport**: (a) `egui_wgpu::CallbackTrait` paint
     callbacks draw inside egui's render pass — fine for cheap previews, but it cannot
     express the engine render graph's passes, and it makes attachment/render ordering a
     property of where the egui panel happened to be drawn; (b) a native child window
     `Surface` per viewport — compositing, DPI, input routing and multi-viewport/docking
     become platform work and the UI would hold GPU resources; (c) UI-owned wgpu
     resources/camera state — violates the single UI→GPU path and creates two camera
     truths.
- **Consequences**:
  - Positive: adding panels is additive; the same panel can be hosted by the current
    four-slot host or the final dock widget; viewport rendering/render-target lifetime has
    exactly one owner (host/engine); panels remain headless-testable; the UI/renderer
    boundary is identical in the prototype and the final editor.
  - Negative: one more indirection (trait object + host) versus a struct field; panel hosts
    must explicitly sync with `LayoutState`; a one-frame viewport latency exists unless the
    engine update is moved after UI (deliberately rejected for now).
  - Neutral: egui remains the view technology; the final dock widget, camera systems and
    render graph are kept.
- **References**:
  - Prototype implementation: `prototypes/editor-ui/` — `editor-components/src/panel/`
    (`mod.rs` trait/ids, `registry.rs` factories, `host.rs` docking/draw entry),
    `editor-components/src/viewport_panel.rs`, `editor-core/src/viewport.rs`
    and `editor-core/src/reduce/viewport.rs` (domain slice), `crates/editor-viewport`
    (host renderer + camera + picking), `editor-app/src/app/` (`mod.rs` frame entry /
    composition root, `pipeline.rs` phases, `effects.rs` executor).
  - Design detail: [`docs/plan/editor-ui-architecture/panel-viewport-design.md`](../plan/editor-ui-architecture/panel-viewport-design.md)
  - Engine integration facts: [`docs/research/editor-ui-engine-integration.md`](../research/editor-ui-engine-integration.md)
  - Architecture baseline: [ADR 0008](./0008-editor-ui-architecture.md),
    [`docs/plan/editor-ui-architecture/README.md`](../plan/editor-ui-architecture/README.md) §3.3-§3.5,
    [`rewrite-plan.md`](../plan/editor-ui-architecture/rewrite-plan.md) Phase 1-2.
