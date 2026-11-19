# ADR 0010: Runtime logging — one `kairos_log` entry, three levels, terminal + file + sinks

- **Status**: Proposed
- **Date**: 2026-11-12
- **Context**: The workspace logs in three unrelated ways:
  1. `kairos_engine::log::Log` — an instance created by the host and threaded as `&mut Log`
     through every legacy editor `Drawer::ui`; it has Info/Warning/Error, a caller location,
     an optional backtrace, stderr-only output, and a `VecDeque` that the old Console window
     `pop_front`s (consuming history). It only exists where the host can pass it, so engine
     crates cannot use it at all.
  2. Direct `log::{info, warn, error, debug, trace}` calls in `kairos_ecs`, `kairos_physics`,
     `kairos_graphics`, `kairos_asset`, ... wired only by `env_logger` in the two binaries
     (`kairos_editor/src/main.rs`, `kairos_engine/src/bin/bake_assets.rs`): terminal-only,
     no file, and per-target `RUST_LOG` filters.
  3. `tracing` (spans) in a few crates (`kairos_transform`, `kairos_asset`), unconnected to
     the other two.

  There is no file output, no way for the new editor stack to display engine records, and the
  legacy instance-based `Log` cannot serve as the runtime entry. The owner asked for: a runtime
  log module any code can call; exactly three levels (Info, Warning, Error); terminal **and**
  file output by default; and the editor Console panel showing those same records.
- **Decision**:
  1. **`kairos_log` is the runtime log entry** — a foundation crate (leaf; depends only on
     `log` + `jiff`). Any crate calls `kairos_log::info!` / `warn!` / `error!`; no logger
     instance is threaded through APIs. It **implements `log::Log`**, so the existing
     `log::*` call sites keep working and start reaching the new sinks as soon as the host
     calls `init`; debug/trace records are intentionally dropped (three-level vocabulary).
  2. **One logger per process, installed at the host entry point.**
     `kairos_log::init()` installs the logger (idempotent detection returns
     `InitError::AlreadyInitialized`) and returns a `Guard` that flushes the sinks on drop.
     Configuration is `Config` (level, terminal, file) with `KAIROS_LOG_LEVEL` /
     `KAIROS_LOG_FILE` (empty disables the file) / `KAIROS_LOG_TERMINAL` overrides.
  3. **Default sinks: terminal + file.** stderr with ANSI colors when it is a terminal;
     `Library/Logs/KairosEngine.log` relative to the working directory, truncated per run,
     `Error` records flushed immediately. Additional sinks implement `Sink` and are added
     with `attach()`; dropping the returned `SinkHandle` detaches them.
  4. **The editor side is not a crate.** There is no `kairos_editor_log`: the only consumer
     is the Console panel, so its buffer lives beside it as
     `kairos_editor_component::ConsoleLog` — a bounded (1000-line), thread-safe `Sink` over
     `VecDeque`, alive across panel close (history is host-owned, not panel state). The app
     owns one `Arc<ConsoleLog>`, attaches it at construction, and exposes it to the panel
     through `HostView::console_lines(&mut dyn FnMut(&[ConsoleLine]))`; each `ConsoleLine`
     carries the level (the panel colors warnings/errors from the theme) and the formatted
     text. Panel code stays read-only; it never consumes history.
  5. **The legacy `kairos_engine::log::Log` stays for now.** It is only used by the legacy
     editor being retired (P5); new code uses `kairos_log`. The old Console window keeps
     working until that editor migrates.
- **Alternatives rejected**:
  - *tracing + tracing-subscriber*: better for spans and structured fields, but most call
    sites already use `log`; converting the workspace is churn for a three-level log. Can be
    revisited when spans/correlation ids are needed.
  - *env_logger plus a file target*: `env_logger` has one target per logger, so terminal +
    file still needs a custom `log::Log` composition; its per-target `RUST_LOG` syntax is
    more configuration than the owner asked for.
  - *A separate `kairos_editor_log` crate*: premature — one consumer, no second host yet.
    The sink is isolated enough to extract if the legacy editor or another window needs it.
  - *Console panel owns its history*: violates the host-resource rule (not state, not
    serializable, not panel-owned) and loses history when the panel closes.
- **Consequences**:
  - Positive: any crate logs with one import and no plumbing; terminal + file work by
    default; the same records appear in the editor console with level colors; sink fan-out is
    a small seam; `ConsoleLog` and the file sink are headless-testable; the old `log::*`
    calls are already on the new path.
  - Negative: one global logger, so a process cannot host two independent logs (tests use
    one init per test binary); `log::debug!`/`trace!` are silently dropped until a fourth
    level is wanted; records emitted before `init` are lost; the file sink writes on the
    emitting thread (flushing on errors), which is acceptable at current log volume.
  - Neutral: the default file path is working-directory-relative, like `Preferences/` and
    `imported_assets/`, not a per-user data directory.
- **Implementation note (2026-11-12, editor console)**:
  - `Record` also carries the macro call site (`file`/`line`) and, when the process
    enables backtraces (`RUST_BACKTRACE=1`, the repo default for `cargo run`/tests), a
    `Backtrace`. `Record::stack_frames()` trims the logger's own frames plus std/winit/eframe
    noise and returns structured frames (`StackFrame { symbol, source }`), so the console
    composes Unity-style `symbol (at file:line)` lines without parsing text; the
    platform-specific `at path:line[:col]` text is parsed once, here. It is what the editor
    console shows below a selected entry.
  - The Console panel is rebuilt against the real Unity/Tuanjie 2022.3 console: a 20 px
    toolbar (Clear / Collapse / Error Pause / Editor, search, per-level count toggles), 38 px
    two-line rows with alternating `#373737`/`#313131` backgrounds, 26 px severity icons, and
    a 130 px stack-trace detail pane for the selected entry, all in 12 px Inter. Geometry,
    palette and behavior notes live in
    [`docs/research/unity-console/`](../research/unity-console/README.md); the implemented
    result is `21_kairos_console_implemented_window.png` / `22_kairos_console_implemented_panel.png`.
  - Console style is component-owned: `kairos_editor_component::panels::ConsoleStyle`
    (`console_style.rs`) is loaded from `Preferences/Styles/ConsoleStyle.ron` in
    `Panel::init`. The RON file holds both the dark and light skins, colors are
    `kairos_color::Color32` values written as human-editable hex strings (`"#353535"`) by
    that crate's serde impls, and icon file paths are part of the style (including
    `tab_icon`, the panel icon shown in the dock tab header).
    `kairos_editor_widgets::style` provides the `Style` trait (`FILE`, `dark()`,
    `light()`), the generic `StyleFile<S>` (`load_from` / `save_to` /
    `load_or_default` / `load_or_create`) and a `export_styles` example in
    `kairos_editor_component` that re-seeds the RON files from the code defaults. The
    global `Theme` install was removed: each panel loads its own style, so editing the
    RON and reopening the panel restyles it without a rebuild.
  - The color type is shared: `kairos_color` (extracted from
    `kairos_engine::math::color`) owns `Color32`, hex parsing/formatting and serde,
    and stays UI-agnostic (`serde`/error code lives in `color.rs`; there is no
    conversions module). `kairos_engine::math::Color32` re-exports the same type for
    existing engine/editor code. Each consumer owns its conversion: the new editor
    uses `kairos_editor_widgets::color::Color32Ext::to_egui` at paint sites, and
    the legacy editor has `kairos_editor::color` helpers for egui/syntect.
  - Icons are runtime assets too (`Preferences/Textures/console/`, light artwork under
    `light/`). `Panel::init` preloads them through `kairos_editor_widgets::ui_image`;
    egui's `egui_extras` file/image loaders (installed once by the app) read and decode on
    worker threads and request a repaint, and drawing samples the texture cache. Neither
    `ConsoleStyle` nor the RON file contains GPU handles: textures are device resources and
    are not part of serializable style data.
  - The tab-header icon crosses the dock seam as an `egui::ImageSource`, not a file path.
    Each panel owns its tab identity: `Panel::icon` returns that panel's own
    editor-relative appearance path (the Console reads `ConsoleStyle::tab_icon`; the
    not-yet-styled demo panels keep a module-local const next to their title), and the
    panel's `init` preloads it with its other UI resources. `PanelHostViewer` (the only
    adapter that knows both layers) converts the path with `ui_image::file_uri` before
    handing the source to `TabViewer::icon`. The artwork itself is Tuanjie's window-type
    icon set extracted from the editor resources (`docs/research/unity-editor-chrome/`).
    `kairos_editor_docking` only lays out and paints the source inside the tab header;
    icon size, icon/title gap, horizontal padding and per-state tint are dock appearance
    tokens in `TabStyle` / `TabInteractionStyle`. The panel starts the load at the
    resource-lifecycle point; the first tab render then samples the cache instead of
    triggering file IO from the paint code.
  - Fonts are runtime files as well: `kairos_editor_widgets::fonts::FontsConfig` loads
    `Preferences/Styles/EditorFonts.ron` (Inter Regular/Medium from `Preferences/Fonts/`)
    and calls `Context::set_fonts` once when the app starts. The OFL licence moved next to
    the fonts (`Preferences/Fonts/Inter-LICENSE.txt`); no editor UI asset is embedded with
    `include_bytes!` anymore.
  - `PanelHost::sync(layout, ctx)` calls `Panel::init(ctx)` exactly once per instance,
    after construction and before the first draw. A panel loads its own UI resources
    there (the Console preloads its severity icons and tab icon; the demo panels preload
    their tab icon). No panel loads resources at process start or in its per-frame `ui`. The panel keeps only UI-only
    session state (search,
    collapse, filters, clear cursor, selection); `PanelHost` already keys the instance by
    `PanelInstanceId`, while retaining that state across close/reopen through the
    documented `ViewStates` map — and moving the hand-painted body into an L1
    `ConsoleView` widget — remain follow-ups.
  - Editor chrome images deliberately do **not** use `kairos_asset`: they are not project
    assets, have no import/meta/processing step, and must render before the engine asset
    server exists (the current editor shell is not engine-hosted yet). Project asset
    previews/thumbnails will take a separate path through `AssetServer` handles once the
    editor is hosted by the engine; the file-backed UI path is deliberately swappable.
  - The severity icons are the real `console.infoicon` / `warnicon` / `erroricon`
    (`ConsoleCountBadge` for collapse) extracted from the locally installed Tuanjie/Unity
    serialized resources; the editor UI fonts are Unity's bundled Inter (SIL OFL 1.1, licence
    in `Preferences/Fonts/Inter-LICENSE.txt`). These are local-reference assets: do not
    redistribute the Unity-extracted icon PNGs without checking Unity's terms.
  - Fix landed with this work: a split separator used to be painted on top of the right/bottom
    child by its full width, which hid the focused tab's blue headerline whenever the tab bar
    sat against a split boundary (the splitter and the `compute_rect_size` right border are
    corrected; regression test in `kairos_editor_docking/tests/docker_ui.rs`).
- **References**:
  - Implementation: `kairos_log/src/{lib,logger,level,record,config,sink,sinks}.rs`;
    console sink `kairos_editor_component/src/{console,frame}.rs`;
    wiring `kairos_editor_app/src/{main,app}.rs`.
  - Tests: `cargo test-crate kairos_log` (levels, config, file sink, public sink API),
    `cargo test-crate kairos_editor_component` (console ring, panel rendering),
    `cargo test -p kairos_editor_app --bins --tests` (host view + app wiring).
  - Related: [ADR 0008](./0008-editor-ui-architecture.md) (state tree / `Frame`/`UiOut`),
    [ADR 0009](./0009-editor-panel-host-and-viewport.md) (host-owned resources).