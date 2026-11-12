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
    `Backtrace`. `Record::stack_trace()` trims the logger's own frames plus std/winit/eframe
    noise and renders Unity-style `symbol (at file:line)` frames; it is what the editor
    console shows below a selected entry.
  - The Console panel is rebuilt against the real Unity/Tuanjie 2022.3 console: a 20 px
    toolbar (Clear / Collapse / Error Pause / Editor, search, per-level count toggles), 38 px
    two-line rows with alternating `#373737`/`#313131` backgrounds, 26 px severity icons, and
    a 130 px stack-trace detail pane for the selected entry, all in 12 px Inter. Geometry,
    palette and behavior notes live in
    [`docs/research/unity-console/`](../research/unity-console/README.md); the implemented
    result is `21_kairos_console_implemented_window.png` / `22_kairos_console_implemented_panel.png`.
  - The severity icons are the real `console.infoicon` / `warnicon` / `erroricon`
    (`ConsoleCountBadge` for collapse) extracted from the locally installed Tuanjie/Unity
    serialized resources; the editor UI font is Unity's bundled Inter (SIL OFL 1.1, licence
    in `kairos_editor_widgets/assets/fonts/Inter-LICENSE.txt`). These are local-reference
    assets: do not redistribute the Unity-extracted icon PNGs without checking Unity's terms.
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
