# ADR 0008: Editor UI architecture — single state tree, reducers, effects, commands

- **Status**: Proposed
- **Date**: 2026-10-30
- **Implementation note (2026-11-12)**: the L2 crate landed as `kairos_editor_component`
  (not `kairos_editor_ui`). It currently carries the panel framework
  (`Panel`/`PanelRegistry`/`PanelHost`/`TabViewer` adapter) plus demo panels;
  `kairos_editor_core` currently carries only the state tree (`EditorState`/`LayoutState`/panel ids) —
  reducers/effects/commands are still pending (only the layout slice has `Action`/`reduce`).
  The prototype's `Frame`/`UiOut` are ported: panels read a read-only `Frame` and push
  `Action`s into `UiOut`; the app drains it after the last UI pass and calls `reduce`, the
  only writer of `EditorState`. The console history is a `kairos_log` sink
  (`kairos_editor_component::ConsoleLog`) exposed through `HostView::console_lines`; runtime
  logging is [ADR 0010](./0010-runtime-logging.md).
- **Context**: `kairos_editor` UI layer (~17.6k LOC / 54 files) built on a global `Messager`
  queue (57 `Message` variants, one 365-line central match) and on window structs that own
  both model and view (`Drawer`). The design blocks multi-instance views, forces interior
  mutability (`Cell`, `Arc<Mutex<_>>`, `oneshot::Receiver` inside messages), mixes UI
  lifecycle with business logic and `World` mutation, and leaves the editor with zero
  interaction tests. The editor owner has explicitly accepted a full rewrite.
- **Decision**: Rewrite the editor UI as a **unidirectional data flow** architecture, keeping
  egui as the view technology:
  1. `kairos_editor_core` (new crate, no egui/winit/wgpu/World/IO): `EditorState` as a single
     owned state tree split into domain slices; per-domain `Action` enums; pure
     `reduce(&mut EditorState, Action) -> Vec<Effect>`; `Effect` as plain data executed by the
     host; `CommandRegistry` (id/label/shortcut/enabled/action) as the single entry point for
     toolbar, menus, shortcuts, command palette and MCP; `Document` + `Selection` as shared
     services with `EditTransaction` undo history.
  2. `kairos_editor_ui` (new crate): stateless egui panels taking `&Frame` (read-only state +
     `&World` + host resources) and `&mut UiOut` (action collector); dock layout keyed by
     `PanelInstanceId`; per-panel `ViewState` for ephemeral UI state only.
  3. `kairos_editor` becomes the host: frame loop (pump async completions → reduce → run
     effects → render viewports → draw UI → dispatch UI actions), `EffectExecutor` (disk,
     tokio, native dialogs, engine mutations), and the `install` bootstrap.
  4. Delete `Messager`, `Message`, `Drawer`, `UIReader`, the god `Context::handle`, the
     fake layout drawers, and the callback-style `Dialog`.
  5. Sequencing rules validated by the merged prototype (`prototypes/editor-ui/`, crates `editor-core` + `editor-components`):
     the reducer owns `RequestSlot<T>`s and validates stale completions itself — the host
     only dispatches `Action::Effect(EffectOutcome)` and holds no ticket→semantics mapping;
     `Action::Batch` is for **synchronous** actions only (async sequencing such as
     save-then-close must be modelled as explicit pending state, e.g. `close_after_save`,
     because `Batch([Save, Close])` is racy); `Command.action` is a function
     `fn(&EditorState) -> Action`, not a constant, so parameterised commands do not spawn
     `*Active` variants.
  6. View layer is split into three layers (see `docs/plan/editor-ui-architecture/widget-layer.md`):
     **Theme tokens → Widgets (pure appearance) → Components (feature views)**.
     Widgets live in their own crate `kairos_editor_widgets` that depends only on egui and
     the UI-agnostic leaf value crate `kairos_color` (style colors), so a widget cannot name
     a domain type (`Guid`/`Entity`/`Document`/`Action`) — the boundary is enforced by cargo,
     not by convention. Components in `kairos_editor_ui` read
     `&EditorState`, compose widgets and emit `Action`; the domain↔widget mapping lives in
     pure adapter functions (`state -> widget props`, `widget output -> Action`) that are
     unit-testable without egui. Ephemeral widget state stays in egui `Memory`; session state
     in `ViewState`; domain state only in the core store.
- **Consequences**:
  - Positive: logic unit-testable without GPU/window; features become "one domain module +
    one draw function"; multi-instance panels possible; MCP/commands/queries share one
    typed API; async results become `Action::Effect(EffectOutcome)` instead of channel
    receivers smuggled through a queue; layers are enforced by crate boundaries.
  - Negative: two new crates and a ~7-9 week single-dev migration (see rewrite plan); the
    team must resist re-introducing mutable/direct state access in views (enforced by
    signatures + lint/review rules); Action/Effect enums must be partitioned per domain or
    they will grow into a new god enum.
  - Neutral: egui, the forked dock widget, style TOMLs, fonts, assets and `camera.rs`
    systems are kept; the engine's own ECS `Message` system remains the mechanism for
    engine-internal events, not the editor UI bus.
- **Alternatives considered**:
  - ECS-resident editor state (World resources + systems): rejected — forces `Send + Sync` UI
    state, still needs read snapshots during draw, mixes editor UI with game data.
  - Fine-grained reactive signals (Floem/Dioxus/Leptos style): rejected for egui's immediate
    mode; only derived-value caching may borrow the idea later.
  - Incremental repair of `Drawer`/`Messager` (e.g. the `QueryHandler` plan in
    `docs/plan/kairos-editor-mcp`): rejected as the end state — it keeps the root causes
    ("window = model", global writable state) and only adds per-window boilerplate.
- **References**:
  - `docs/adr/0009-editor-panel-host-and-viewport.md` (panel registry/host and egui↔wgpu viewport seam)
  - `docs/plan/editor-ui-architecture/widget-layer.md` (widget/component layering)
  - `docs/research/ui-widget-component-layer-survey.md` / `docs/research/editor-ui-widget-catalog.md` (design-system prior art + current-UI widget catalog)
- `prototypes/editor-ui/` (one merged, runnable prototype: `editor-core` reducers/effects/undo + viewport slice, pure `ui-widgets` kit, `editor-components` Panel trait/registry/host + panels, `editor-viewport` wgpu RTT/camera/pick host, `editor-app` frame phases; 126 tests = 125 headless + 1 doctest, incl. 7 full-stack, 14 panel-host and 3 viewport-panel interaction tests; see ADR 0009)
