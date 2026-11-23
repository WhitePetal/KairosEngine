use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use kairos_editor_component::{
    ConsoleLine, ConsoleLog, EditorInfo, Frame, HostView, PanelContext, PanelHost, PanelInstanceId,
    PanelSkin, UiOut, panels, shared_style::SharedStyles, shared_style::StyleCatalog,
    toolbar::ToolBar,
};
use kairos_editor_core::project::{ScanReason, SelectMode};
use kairos_editor_core::{
    Action, AppAction, EditorState, Effect, LayoutAction, ProjectAction, ProjectState,
    SharedStyleAction, StyleAction, reduce,
};
use kairos_editor_docking::{
    docker::{DockState, Docker},
    split::Fraction,
    tree::NodeIndex,
};
use kairos_engine::Engine;
use kairos_log::SinkHandle;
use kairos_profile::{Category, Profiler};

use crate::consts;
use crate::effects::EffectExecutor;
use crate::project::ProjectService;
use crate::project::thumbnail::Thumbnails;

/// Host-owned resources a panel may display but never own (the prototype's
/// `HostState`). The console history is a `kairos_log` sink: the process logger
/// writes into it, the Console panel reads it through `HostView`, and neither
/// owns it. It is not in `EditorState` because it is a host stream, not
/// serializable state.
///
/// The engine is the other, but the app does not own it: the engine's loop
/// (`kairos_engine::runtime`) does, and borrows it into each UI pass. A frame
/// sees it through [`HostScope`], which is the `HostView` handed to the panels.
/// `kairos_profile` has no process-global recorder, so the engine owns the one
/// this process uses; the app never creates a second.
struct AppHost {
    console: Arc<ConsoleLog>,
    /// Decoded previews for the Project panel's texture rows.
    ///
    /// Host-owned because a preview is a GPU texture and a file read, neither
    /// of which belongs to the UI state tree. Shared with each frame's
    /// `HostScope` by Arc so the UI can fill it while the app's other fields are
    /// borrowed.
    thumbnails: Arc<Thumbnails>,
    /// Keeps the console attached to the process logger; dropping the app
    /// detaches the sink.
    _console_sink: SinkHandle,
}

impl AppHost {
    fn new() -> Self {
        let console = Arc::new(ConsoleLog::new());
        let console_sink = kairos_log::attach(console.clone());
        Self {
            console,
            thumbnails: Arc::new(Thumbnails::default()),
            _console_sink: console_sink,
        }
    }
}

/// The `HostView` one UI pass hands to the panels: the host's console history
/// plus the loop's engine recorder.
///
/// A separate short-lived value rather than a field on the app, because the
/// engine belongs to the loop, not to the app — this is where that borrow is
/// bounded.
struct HostScope<'a> {
    console: Arc<ConsoleLog>,
    thumbnails: Arc<Thumbnails>,
    profiler: Option<&'a Profiler>,
}

impl HostView for HostScope<'_> {
    fn console_lines(&self, f: &mut dyn FnMut(&[ConsoleLine])) {
        self.console.with_lines(f);
    }

    fn profiler(&self) -> Option<&Profiler> {
        self.profiler
    }

    fn project_thumbnail(
        &self,
        ctx: &egui::Context,
        root: &str,
        relative: &str,
    ) -> Option<egui::TextureId> {
        self.thumbnails.get(ctx, root, relative)
    }
}

/// The panels' construction context, built from this app's product constants.
///
/// The app owns what the editor calls itself; `kairos_editor_component` cannot
/// reach these constants without depending on this crate, so the identity is
/// injected through the `PanelContext` the host hands to every factory (see
/// `panels::catalog`). That keeps one source of truth for the version: the
/// title bar and About read the same constant.
fn editor_info() -> EditorInfo {
    EditorInfo {
        name: consts::APP_NAME,
        version: consts::VERSION,
        description: consts::DESCRIPTION,
        license: consts::LICENSE,
        repository: consts::REPOSITORY_URL,
        discussions: consts::DISCUSSIONS_URL,
        credits: consts::CREDITS_URL,
    }
}

/// The first-run open set: the panels [`default_dock`] builds tabs for.
///
/// Bootstrapping is the only code allowed to call `LayoutState::open` directly;
/// runtime opens go through `LayoutAction` so the reducer can return a
/// placement effect. This is app policy, not panel metadata: which panels a
/// fresh editor shows is a product decision, so it does not live in the
/// catalog.
///
/// This is also where the project slice learns its root. The working directory
/// is read here, at the one IO boundary the app owns, and stored as data: core
/// never calls `current_dir` (D9 in `docs/plan/project-panel`), so pointing the
/// editor at another project later means replacing this value, not the tree.
fn boot_state(root: PathBuf) -> EditorState {
    let mut state = EditorState::new();
    state.project = ProjectState::at(root);
    for kind in [
        panels::SCENE,
        panels::INSPECTOR,
        panels::PROJECT,
        panels::CONSOLE,
        panels::PROFILER,
        panels::HIERARCHY,
    ] {
        state.layout.open(kind);
    }
    state
}

/// Build the first-run layout from the ids the host created for the default
/// panels. User drags/splits after this are pure dock-state changes.
fn default_dock(host: &PanelHost) -> DockState<PanelInstanceId> {
    let scene = host.id_of(panels::SCENE).expect("scene instance");
    let inspector = host.id_of(panels::INSPECTOR).expect("inspector instance");
    let project = host.id_of(panels::PROJECT).expect("project instance");
    let console = host.id_of(panels::CONSOLE).expect("console instance");
    let profiler = host.id_of(panels::PROFILER).expect("profiler instance");
    let hierarchy = host.id_of(panels::HIERARCHY).expect("hierarchy instance");

    let mut dock: DockState<PanelInstanceId> = DockState::new();
    let [scene_leaf, _inspector_leaf] = dock.main_surface_mut().split_right(
        NodeIndex::root(),
        Fraction::new(0.78),
        vec![inspector],
    );
    dock.main_surface_mut()[scene_leaf].append_tab(scene);
    let [top, _bottom] = dock.main_surface_mut().split_bottom(
        scene_leaf,
        Fraction::new(0.7),
        vec![project, console, profiler],
    );
    dock.main_surface_mut()
        .split_left(top, Fraction::new(0.22), vec![hierarchy]);
    dock
}

pub struct EditorApp {
    state: EditorState,
    host: PanelHost,
    dock: DockState<PanelInstanceId>,
    host_view: AppHost,
    tool_bar: ToolBar,
    /// Host work returned by the last reduce. Executed in the next frame's host
    /// phase, after the panel host has synced its instances.
    pending_effects: Vec<Effect>,
    /// Actions waiting to be applied to the state tree at the next frame's host
    /// phase (`begin_frame`). The reducer is the only writer, so host work
    /// (scan results, watcher rescans, file-operation completions) enters
    /// through here like any other action.
    inbox: VecDeque<Action>,
    /// Host side of the project slice: scanning, watching and file operations.
    /// It owns the file system so the state tree does not have to.
    project: ProjectService,
    /// Live panels' skin field tables, rebuilt each frame while the Preferences
    /// editor is open (and empty otherwise).
    skins: Vec<PanelSkin>,
    /// Appearance owned by the editor shell (toolbar, color picker). Loaded
    /// once at boot, edited live from the Preferences panel's shared pages.
    shared: SharedStyles,
}

impl EditorApp {
    /// Builds the app on an existing egui context.
    ///
    /// The context comes from the engine's egui layer
    /// (`kairos_editor_egui::EguiLayer`), which owns it, installs the image
    /// loaders and wires the repaint callback. The engine belongs to the loop
    /// and is not passed here; it arrives with every UI pass.
    pub fn new(egui_ctx: &egui::Context) -> Self {
        Self::with_root(egui_ctx, std::env::current_dir().unwrap_or_default())
    }

    /// The editor for an explicit project root.
    ///
    /// `new` passes the working directory. Tests pass a temp directory: the boot
    /// scan and the watcher are real here, and they must not read the checkout
    /// the test binary happens to run from.
    pub(crate) fn with_root(egui_ctx: &egui::Context, root: PathBuf) -> Self {
        // The editor chrome is authored dark-only: the dock's `Style`, the
        // toolbar skin and the widget defaults are all dark, but each panel picks
        // its skin from `ctx.theme()`. Left to follow the desktop, a light system
        // theme paints a white panel inside the dark window (About is where that
        // showed up). Pin the theme before `PanelHost::sync` runs `Panel::init`,
        // which is where every component reads it. The per-component light skins
        // stay in their RON files, dormant until the dock gains one too.
        egui_ctx.set_theme(egui::Theme::Dark);

        // Shared styles are editor chrome, not panel instances: one load at
        // boot, before the first frame. `preload` installs what they point at —
        // the font list, the toolbar logo, the eyedropper icon — so the fonts no
        // longer need a boot call of their own. A missing file is seeded in
        // place, so the editor works on a fresh checkout without a build step.
        let shared = SharedStyles::load(&StyleCatalog::builtin(), egui_ctx.theme());
        shared.preload(egui_ctx);

        // The swatch book is a context-wide widget resource: the store only
        // records the project-relative path, and the first picker that opens
        // loads it. Installing it here keeps the path out of the widget and out
        // of the working directory.
        kairos_editor_widgets::color_picker::install_swatches(
            egui_ctx,
            root.join("Library/ColorSwatches.ron"),
        );

        let state = boot_state(root.clone());
        let mut host = PanelHost::new(panels::catalog(), PanelContext::new(editor_info()));
        host.sync(&state.layout, egui_ctx);
        let dock = default_dock(&host);

        let host_view = AppHost::new();
        let tool_bar = ToolBar::new();

        // The project's host side, booted after the shared styles: seeding a
        // missing style file writes into the project, and the watcher must not
        // be running yet to report that as an external change. The two actions
        // it returns are the order the slice requires — the policy first, then
        // the first scan — so they enter the inbox as a pair.
        let (project, boot_actions) = ProjectService::boot(root);
        let inbox: VecDeque<Action> = boot_actions.into_iter().collect();

        // Reaches stderr, the run log and the console sink attached above.
        kairos_log::info!(target: "kairos_editor", "editor booted");

        Self {
            state,
            host,
            dock,
            host_view,
            tool_bar,
            pending_effects: Vec::new(),
            inbox,
            project,
            skins: Vec::new(),
            shared,
        }
    }

    /// Start one engine frame: settle host work, then make the live instances
    /// match the layout, before the UI pass draws.
    ///
    /// Called exactly once per engine frame by the host layer
    /// (`EditorLayer::render`), *outside* `Context::run_ui`: egui may run a UI
    /// pass more than once (a layout can request a discard), and host work must
    /// not depend on how many passes a frame happens to run. The frame boundary
    /// is the engine's, not egui's.
    ///
    /// The phase order is fixed:
    ///
    /// 1. host events that completed since the last frame (scan results, the
    ///    watcher, file operations) enter the inbox;
    /// 2. the inbox is applied through the reducer — the one entry point, shared
    ///    with the UI's own actions;
    /// 3. live instances and dock tabs are reconciled with `LayoutState`;
    /// 4. the effects the reducer queued are executed, after the instances
    ///    exist (a placement effect needs its panel to exist first).
    ///
    /// Actions an effect produces are *completions*. They are queued for the
    /// **next** frame's host phase instead of re-entering the reducer here, so an
    /// action/effect pair cannot cycle within a frame and the host phase needs no
    /// settle budget. The cost is one frame of latency for a completion the host
    /// produced synchronously; a completion delivered through `poll` (a scan
    /// thread finishing) is applied in the first frame that polls it.
    pub(crate) fn begin_frame(&mut self, ctx: &egui::Context) {
        kairos_profile::scope!(Category::Editor, "Kairos.Editor.BeginFrame");
        let now = Instant::now();

        // 1. Host work that completed since the last frame. It joins the
        // completions an effect produced last frame: both are actions waiting
        // for the reducer. An external change also invalidates every decoded
        // preview: the image behind a path may be a different image now.
        let events = self.project.poll(now);
        if events.iter().any(|action| {
            matches!(
                action,
                Action::Project(ProjectAction::ScanRequested {
                    reason: ScanReason::ExternalChange,
                    ..
                })
            )
        }) {
            self.host_view.thumbnails.clear();
        }
        self.inbox.extend(events);

        // 2. Apply everything queued; the reducer returns this frame's effects.
        let batch: Vec<Action> = self.inbox.drain(..).collect();
        self.apply_actions(batch);

        // 3. Instances and tabs must match the layout before effects run: a
        // placement effect for a panel whose instance does not exist yet would
        // otherwise find nothing to focus.
        self.sync_instances(ctx);

        // 4. Run this frame's effects. Their completions wait for the next
        // frame; see the doc comment above.
        let produced = self.execute_pending_effects(ctx, now);
        self.inbox.extend(produced);

        // The Preferences editor draws the live panels' field tables. Building
        // them walks every token of every open skin, so snapshots exist only
        // while that panel is open; after the effects, so an edit applied last
        // frame is already visible in the table.
        self.skins = if self.state.layout.is_open(panels::PREFERENCES) {
            self.host.skins(&self.state)
        } else {
            Vec::new()
        };

        debug_assert!(
            self.host.ids().all(|id| self.dock.contains_tab(id)),
            "a live panel instance has no dock tab; opening a panel must return a layout effect"
        );
    }

    /// Make the live panel instances and the dock tabs match `LayoutState`.
    ///
    /// Both are derived data: the state tree holds the intent, this is the
    /// reconciliation. It is cheap and idempotent, so the host phase can call it
    /// every frame and the dock cannot drift from the layout.
    fn sync_instances(&mut self, ctx: &egui::Context) {
        self.host.sync(&self.state.layout, ctx);

        let ids: Vec<PanelInstanceId> = self.host.ids().collect();
        for stale in self.dock.tabs().into_iter().filter(|id| !ids.contains(id)) {
            self.dock.remove_tab_by_id(stale);
        }
    }

    /// Run the effects queued by the reducer and return the actions they
    /// produced.
    ///
    /// Effects are the only source of new dock tabs: every action that opens a
    /// panel returns a placement effect (see `LayoutAction::reduce`). This
    /// replaces the old "append every live instance missing a tab" fallback,
    /// which silently invented a placement in the focused leaf.
    ///
    /// The returned actions are the host's answers (scan completions; later,
    /// file-operation results). The host phase queues them for the *next*
    /// frame's `begin_frame`, never re-entering the reducer in the frame that
    /// ran the effect: a completion is a new frame's input, which is what keeps
    /// action and effect from cycling.
    fn execute_pending_effects(&mut self, ctx: &egui::Context, now: Instant) -> Vec<Action> {
        let effects = std::mem::take(&mut self.pending_effects);
        if effects.is_empty() {
            return Vec::new();
        }
        let mut produced = Vec::new();
        {
            let Self {
                host,
                dock,
                shared,
                project,
                ..
            } = self;
            let mut executor = EffectExecutor::new(host, dock, shared, project, now);
            for effect in effects {
                produced.extend(executor.apply(effect, ctx));
            }
        }
        produced
    }

    /// Apply one batch of actions to the state tree — the only write path.
    ///
    /// Logs the intent (except for gestures that fire every frame), calls the
    /// reducer and queues the effects it returns for the host phase.
    fn apply_actions(&mut self, actions: Vec<Action>) {
        for action in actions {
            if !is_quiet(&action) {
                kairos_log::info!(target: "kairos_editor", "{}", describe(&action));
            }
            let effects = reduce(&mut self.state, action);
            self.pending_effects.extend(effects);
        }
    }

    /// dispatch the actions collected during the UI pass, then let the host-side resources record what happened.
    ///
    /// Called only for a committed pass (`!Context::will_discard()`): egui may
    /// run the UI pass twice, and a discarded pass must not apply its actions.
    fn pump(&mut self, mut out: UiOut) {
        kairos_profile::scope!(Category::Editor, "Kairos.Editor.Pump");
        self.apply_actions(out.take());
    }
}

/// Human-readable form of an action, for the editor log (`kairos_log`).
fn describe(action: &Action) -> String {
    match action {
        Action::Layout(action) => match action {
            LayoutAction::TogglePanel { panel } => format!("toggle panel: {panel}"),
            LayoutAction::ShowPanel { panel } => format!("show panel: {panel}"),
            LayoutAction::ClosePanel { panel } => format!("close panel: {panel}"),
            LayoutAction::FocusPanel { panel } => format!("focus panel: {panel}"),
        },
        Action::App(action) => match action {
            AppAction::QuitRequested => "quit editor".to_owned(),
        },
        Action::Project(action) => match action {
            ProjectAction::PolicyLoaded { policy } => format!(
                "project policy: {} system dirs, {} ignore rules",
                policy.system_dirs.len(),
                policy.ignore.len()
            ),
            ProjectAction::ScanRequested { reason, focus } => {
                let focus = focus
                    .as_ref()
                    .map(|path| format!(" -> {path}"))
                    .unwrap_or_default();
                format!("project scan requested ({reason:?}){focus}")
            }
            ProjectAction::Scanned {
                generation,
                entries,
                truncated,
                elapsed_ms,
                library_nodes,
                errors,
            } => {
                let truncated = if *truncated { " (truncated)" } else { "" };
                let errors = if errors.is_empty() {
                    String::new()
                } else {
                    format!(", {} unreadable", errors.len())
                };
                format!(
                    "project scan #{generation}: {} nodes (library {library_nodes}) in {elapsed_ms} ms{truncated}{errors}",
                    entries.len()
                )
            }
            ProjectAction::Select { paths, mode } => match (mode, paths.len()) {
                (_, 0) => "project select: none".to_owned(),
                (SelectMode::Range, 2) => {
                    format!("project select range: {} .. {}", paths[0], paths[1])
                }
                (SelectMode::Toggle, count) => format!("project toggle selection: {count} paths"),
                (_, 1) => format!("project select: {}", paths[0]),
                (_, count) => format!("project select: {count} paths"),
            },
            ProjectAction::ToggleFolder { path } => format!("project toggle folder: {path}"),
            ProjectAction::Navigate { path } => format!("project navigate: {path}"),
            ProjectAction::OpenRequested { path } => format!("project open: {path}"),
            ProjectAction::SetShowHidden(show_hidden) => {
                format!("project show hidden: {show_hidden}")
            }
            ProjectAction::CollapseAll => "project collapse all".to_owned(),
            ProjectAction::SetTreeSplit(tree_split) => format!("project tree split: {tree_split}"),
            ProjectAction::Failed { message } => format!("project failed: {message}"),
            ProjectAction::CreateRequested { parent, name, kind } => {
                format!("project create {kind:?}: {parent}/{name}")
            }
            ProjectAction::RenameRequested { path, name } => {
                format!("project rename: {path} -> {name}")
            }
            ProjectAction::MoveRequested { paths, target } => {
                format!("project move: {} entries -> {target}", paths.len())
            }
            ProjectAction::DeleteRequested { paths } => {
                format!("project delete: {} entries", paths.len())
            }
            ProjectAction::FsCompleted { op, result } => match result {
                Ok(outcome) => format!("project {op} done: {outcome:?}"),
                Err(error) => format!("project {op} failed: {error}"),
            },
            ProjectAction::RevealRequested { path } => format!("project reveal: {path}"),
        },
        Action::Style(action) => match action {
            StyleAction::SetField { panel, path, .. } => {
                format!("set {panel} skin: {path}")
            }
            StyleAction::EditList { panel, path, edit } => {
                format!("edit {panel} skin list: {path} {edit:?}")
            }
            StyleAction::Reset { panel } => format!("reset {panel} skin to defaults"),
            StyleAction::Commit { panel } => format!("save {panel} skin"),
        },
        Action::SharedStyle(action) => match action {
            SharedStyleAction::SetField { style, path, .. } => {
                format!("set {style} shared skin: {path}")
            }
            SharedStyleAction::EditList { style, path, edit } => {
                format!("edit {style} shared skin list: {path} {edit:?}")
            }
            SharedStyleAction::Reset { style } => {
                format!("reset {style} shared skin to defaults")
            }
            SharedStyleAction::Commit { style } => format!("save {style} shared skin"),
            SharedStyleAction::Reload { style } => format!("reload {style} shared skin"),
        },
    }
}

/// Whether an action is too frequent to log.
///
/// A skin drag emits one `SetField` per frame; logging every frame of a gesture
/// would flood the console the way the legacy window did with its per-frame
/// style messages. The commit (the write) is logged; the value itself is
/// visible in the field.
///
/// An explicit function rather than a `matches!` inside the pump: adding the
/// next high-frequency action means editing this list, with a reason, instead of
/// scattering exceptions through the dispatch path.
fn is_quiet(action: &Action) -> bool {
    matches!(
        action,
        Action::Style(StyleAction::SetField { .. })
            | Action::SharedStyle(SharedStyleAction::SetField { .. })
    )
}

impl EditorApp {
    /// One UI pass, independent of the window backend (also used by smoke tests).
    ///
    /// **The frame boundary is not here.** `kairos_engine::runtime::EngineRunner`
    /// ticks the engine once per redraw, before calling the layer; the pass this
    /// function draws belongs to the frame the loop just opened. So multi-pass
    /// egui layouts need no guard: the engine never ticks twice.
    ///
    /// Host work is settled by [`EditorApp::begin_frame`] before this function
    /// runs, once per engine frame. This function only draws the current state
    /// and pumps the committed pass's actions into the reducer; a discarded pass
    /// commits nothing.
    ///
    /// `Profiler::end_frame` carries egui's discard decision — a discarded pass
    /// must not store the frame, and for a second pass the committed one wins.
    ///
    /// The app asks egui for the next repaint immediately: an editor left alone
    /// keeps producing frames at the display rate. That is also what keeps the
    /// engine's wall-clock `Time` in step with the loop — the engine samples its
    /// own delta in `First`, so it only needs the frames to keep coming, never a
    /// timestamp handed in by the host.
    pub(crate) fn ui(&mut self, engine: &Engine, ui: &mut egui::Ui) {
        let committed = {
            kairos_profile::scope!(Category::Editor, "Kairos.Editor.Frame");

            // Editor chrome is not a text document; dragging tabs must not select
            // labels in the panel bodies.
            ui.style_mut().interaction.selectable_labels = false;

            let mut out = UiOut::new();

            // Capture is host policy: keep it tied to the visible Profiler tab
            // so closing the tab cannot leave the recorder running with no
            // control surface. Frames already captured stay in the store.
            if !self.state.layout.is_open(panels::PROFILER) && engine.profiler().is_recording() {
                engine.profiler().set_recording(false);
            }

            // Built before the destructure below: it borrows the engine, not the
            // app, so the dock pass can still take `self` apart by field.
            let host_scope = HostScope {
                console: self.host_view.console.clone(),
                thumbnails: self.host_view.thumbnails.clone(),
                profiler: Some(engine.profiler()),
            };

            {
                kairos_profile::scope!(Category::Editor, "Kairos.Editor.Dock");
                let Self {
                    state,
                    host,
                    dock,
                    tool_bar,
                    skins,
                    shared,
                    ..
                } = self;
                let frame = Frame::with_host(state, &host_scope)
                    .with_skins(skins)
                    .with_shared(shared);
                tool_bar.ui(ui, &frame, &mut out);
                let mut viewer = host.viewer(&mut out, frame);
                Docker::new("kairos_editor_docker", dock).show(ui, &mut viewer);
            }

            if !ui.ctx().will_discard() {
                self.pump(out);
                // Focus is instance-keyed in `LayoutState`; the dock owns the live
                // instance ids, so the committed frame mirrors them back.
                self.state.layout.set_focus(self.dock.focused_tab());
            }

            !ui.ctx().will_discard()
        };

        engine.profiler().end_frame(committed);

        // Keep the host loop turning whether or not the window receives input:
        // an idle editor must still produce frames (and, when the Profiler tab
        // is recording, record). This is the whole of the frame-pacing policy
        // for now — repaint every frame, capped by vsync.
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use kairos_editor_component::shared_style::{COLOR_PICKER, TOOLBAR};
    use kairos_editor_core::StyleValue;
    use kairos_editor_core::project::ScanReason;
    use kairos_editor_docking::viewer::TabViewer;

    /// Puffin's frame collector is process-global even though each app owns its
    /// recorder, so only one capture test may run a UI pass while recording.
    static UI_LOCK: Mutex<()> = Mutex::new(());

    /// Skin-file tests share the process-wide style root; serialize the writes
    /// so one test cannot read another's edited value.
    static SKIN_FILE_LOCK: Mutex<()> = Mutex::new(());

    fn raw_input() -> egui::RawInput {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(1280.0, 820.0),
        ));
        input
    }

    /// `PanelHost::sync` and `ToolBar::new` seed missing style files; keep them
    /// in a scratch root instead of the crate directory.
    fn scratch_style_root() -> &'static std::path::Path {
        static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
        let root = DIR.get_or_init(|| {
            let root = std::env::temp_dir()
                .join(format!("kairos-editor-app-styles-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let _ = std::fs::create_dir_all(&root);
            root
        });
        kairos_editor_widgets::style::set_style_root(root);
        root
    }

    /// Apply one action the way a committed UI pass does, then run the next
    /// frame's host phase so its effects execute.
    fn apply(app: &mut EditorApp, action: Action, ctx: &egui::Context) {
        app.pump({
            let mut out = UiOut::new();
            out.push(action);
            out
        });
        app.begin_frame(ctx);
    }

    /// A project root the tests own: the boot scan and the watcher are real,
    /// and they must not read the checkout the test binary happens to run from.
    fn tmp_project_root() -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("kairos-editor-app-project-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        root
    }

    fn build() -> EditorApp {
        build_with_root(tmp_project_root())
    }

    fn build_with_root(root: std::path::PathBuf) -> EditorApp {
        scratch_style_root();

        let ctx = egui::Context::default();
        kairos_editor_widgets::color_picker::install_swatches(
            &ctx,
            root.join("Library/ColorSwatches.ron"),
        );
        let state = boot_state(root);
        let mut host = PanelHost::new(panels::catalog(), PanelContext::new(editor_info()));
        host.sync(&state.layout, &ctx);
        let dock = default_dock(&host);
        let tool_bar = ToolBar::new();
        EditorApp {
            state,
            host,
            dock,
            host_view: AppHost::new(),
            tool_bar,
            pending_effects: Vec::new(),
            inbox: VecDeque::new(),
            // Scans run on the calling thread here: the protocol is what these
            // tests assert, and a scheduler would only add timing to it.
            project: ProjectService::inline(),
            skins: Vec::new(),
            shared: SharedStyles::default(),
        }
    }

    /// How many panels the layout has open. Count-based tests derive the number
    /// from app policy (`boot_state`) instead of pinning a literal, so the product
    /// opening another panel by default does not mean rewriting the suite.
    fn open_panel_count(app: &EditorApp) -> usize {
        app.host
            .kinds()
            .filter(|kind| app.state.layout.is_open(*kind))
            .count()
    }

    /// A light desktop must not paint a white panel inside the dark chrome: the
    /// app pins the egui theme before `PanelHost::sync` runs `Panel::init`, which
    /// is where each panel reads `ctx.theme()` to pick its skin.
    #[test]
    fn a_light_desktop_still_gets_the_dark_editor() {
        scratch_style_root();

        let ctx = egui::Context::default();
        ctx.set_theme(egui::Theme::Light);
        let _app = EditorApp::with_root(&ctx, tmp_project_root());

        assert_eq!(ctx.theme(), egui::Theme::Dark);
    }

    #[test]
    fn default_layout_opens_a_tab_for_every_boot_panel() {
        let app = build();
        let expected = open_panel_count(&app);

        assert!(expected > 0, "boot opens at least one panel");
        assert_eq!(app.host.ids().count(), expected);
        assert_eq!(app.dock.tabs().len(), expected);
    }

    /// The project root is data the app reads at boot: the working directory
    /// today (D9). Core never calls `current_dir`; this is that boundary, so the
    /// test pins it here instead of inside the state tree.
    #[test]
    fn the_project_slice_starts_at_the_supplied_root() {
        let root = tmp_project_root();
        let app = build_with_root(root.clone());

        assert_eq!(app.state.project.root, root);
        assert_eq!(
            app.state.project.name,
            root.file_name()
                .expect("the project root has a name")
                .to_string_lossy()
                .into_owned()
        );
    }

    /// The boot path as the product runs it: read/seed `.kairosignore`, load the
    /// policy, then walk the project — all through the real background scanner.
    /// The loop is the test's frame loop, so this covers the same seam the editor
    /// uses (R1): policy before request, completion through `poll`.
    #[test]
    fn boot_loads_the_policy_and_completes_the_first_scan() {
        let root = tmp_project_root().join("boot-case");
        std::fs::create_dir_all(&root).expect("the boot case directory");
        std::fs::write(root.join("res-scan-test.md"), "hello").expect("a project file");
        scratch_style_root();
        let ctx = egui::Context::default();
        let mut app = EditorApp::with_root(&ctx, root.clone());

        app.begin_frame(&ctx);

        assert!(
            app.state.project.scan.running,
            "the boot actions land on the first host phase"
        );
        assert_eq!(
            app.state.project.policy.system_dirs,
            vec![String::from("Library")],
            "the policy reached the state tree before the walk"
        );
        assert!(
            root.join(".kairosignore").is_file(),
            "boot seeds the ignore file in place"
        );

        for _ in 0..500 {
            app.begin_frame(&ctx);
            if !app.state.project.scan.running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        assert!(
            !app.state.project.scan.running,
            "the background scan completes through poll"
        );
        assert!(
            app.state
                .project
                .tree
                .find(&kairos_editor_core::project::RelPath::new(
                    "res-scan-test.md"
                ))
                .is_some(),
            "the tree holds what the walk found"
        );
    }

    /// The About entry is an ordinary `ShowPanel`: it floats where there is no
    /// dock tab, focuses the same instance when there is one, and closing it
    /// takes the floating surface away again.
    #[test]
    fn about_opens_as_a_floating_window_and_comes_back() {
        let mut app = build();
        let ctx = egui::Context::default();
        let workspace_tabs = open_panel_count(&app);

        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::ShowPanel {
            panel: panels::ABOUT,
        }));
        app.pump(out);
        app.begin_frame(&ctx);

        let about = app.host.id_of(panels::ABOUT).expect("about instance");
        assert!(app.dock.contains_tab(about));
        assert_eq!(
            app.dock.valid_surface_indices().len(),
            2,
            "About floats instead of splitting the dock"
        );
        assert_eq!(
            app.dock.main_surface().tabs_count(),
            workspace_tabs,
            "the workspace layout is untouched"
        );
        assert_eq!(app.dock.focused_tab(), Some(about));

        // A second Show focuses the open instance instead of stacking windows.
        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::ShowPanel {
            panel: panels::ABOUT,
        }));
        app.pump(out);
        app.begin_frame(&ctx);
        assert_eq!(app.dock.valid_surface_indices().len(), 2);
        assert_eq!(app.host.id_of(panels::ABOUT), Some(about));

        // Closing drops the instance's tab; the empty window is removed with it.
        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::ClosePanel {
            panel: panels::ABOUT,
        }));
        app.pump(out);
        app.begin_frame(&ctx);
        assert_eq!(app.host.id_of(panels::ABOUT), None);
        assert_eq!(app.dock.valid_surface_indices().len(), 1);
    }

    #[test]
    fn draw_pipeline_runs_end_to_end() {
        let _guard = UI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let engine = Engine::headless();
        let ctx = egui::Context::default();

        let expected = open_panel_count(&app);
        app.begin_frame(&ctx);
        let _ = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));

        assert_eq!(app.host.ids().count(), expected);
        assert_eq!(app.dock.tabs().len(), expected);
    }

    #[test]
    fn editor_frames_are_captured_when_recording() {
        let _guard = UI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let mut engine = Engine::headless();
        engine.profiler().set_recording(true);

        let ctx = egui::Context::default();
        // The loop ticks the engine and then draws: `update` opens the frame,
        // the UI pass below belongs to it, and the next `update` flushes it
        // into the store.
        engine.update();
        app.begin_frame(&ctx);
        let _ = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));
        engine.update();

        let profiler = engine.profiler();
        let session = profiler.session();
        assert!(
            !session.frames.is_empty(),
            "a committed editor pass is flushed at the next frame boundary"
        );
        let has_scope = |name: &str| {
            session.frames.iter().any(|frame| {
                frame.threads.iter().any(|thread| {
                    thread
                        .samples
                        .iter()
                        .any(|sample| session.scope_name(sample.scope) == name)
                })
            })
        };
        assert!(
            has_scope("Kairos.Engine.Frame"),
            "the shell ticks the engine inside the recorded frame"
        );
        assert!(
            has_scope("Kairos.Stage"),
            "the engine's schedule stages are captured too"
        );
        assert!(
            has_scope("Kairos.Editor.Frame"),
            "the UI pass belongs to the same engine frame"
        );

        engine.profiler().set_recording(false);
        engine.profiler().clear();
    }

    #[test]
    fn pump_applies_layout_actions() {
        let mut app = build();

        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::TogglePanel {
            panel: panels::CONSOLE,
        }));
        app.pump(out);

        assert!(!app.state.layout.is_open(panels::CONSOLE));

        app.begin_frame(&egui::Context::default());
        let expected = open_panel_count(&app);
        assert_eq!(app.host.ids().count(), expected);
        assert_eq!(app.dock.tabs().len(), expected);
    }

    #[test]
    fn actions_have_human_readable_log_lines() {
        assert_eq!(
            describe(&Action::Layout(LayoutAction::TogglePanel {
                panel: panels::CONSOLE
            })),
            "toggle panel: console"
        );
        assert_eq!(
            describe(&Action::Layout(LayoutAction::ClosePanel {
                panel: panels::SCENE
            })),
            "close panel: scene"
        );
        assert_eq!(
            describe(&Action::App(AppAction::QuitRequested)),
            "quit editor"
        );
        assert_eq!(
            describe(&Action::Style(StyleAction::SetField {
                panel: panels::CONSOLE,
                path: "toolbar_height".to_owned(),
                value: StyleValue::F32(30.0),
            })),
            "set console skin: toolbar_height"
        );
        assert_eq!(
            describe(&Action::Style(StyleAction::Reset {
                panel: panels::CONSOLE
            })),
            "reset console skin to defaults"
        );
        assert_eq!(
            describe(&Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE
            })),
            "save console skin"
        );
        assert_eq!(
            describe(&Action::SharedStyle(SharedStyleAction::SetField {
                style: COLOR_PICKER,
                path: "border_width".to_owned(),
                value: StyleValue::F32(0.9),
            })),
            "set color_picker shared skin: border_width"
        );
        assert_eq!(
            describe(&Action::SharedStyle(SharedStyleAction::Reset {
                style: COLOR_PICKER
            })),
            "reset color_picker shared skin to defaults"
        );
        assert_eq!(
            describe(&Action::SharedStyle(SharedStyleAction::Commit {
                style: COLOR_PICKER
            })),
            "save color_picker shared skin"
        );
        assert_eq!(
            describe(&Action::SharedStyle(SharedStyleAction::Reload {
                style: COLOR_PICKER
            })),
            "reload color_picker shared skin"
        );
        assert_eq!(
            describe(&Action::Project(ProjectAction::ScanRequested {
                reason: ScanReason::Manual,
                focus: None,
            })),
            "project scan requested (Manual)"
        );
        assert_eq!(
            describe(&Action::Project(ProjectAction::ScanRequested {
                reason: ScanReason::Manual,
                focus: Some(kairos_editor_core::project::RelPath::new("res/models")),
            })),
            "project scan requested (Manual) -> res/models"
        );
        assert_eq!(
            describe(&Action::Project(ProjectAction::Scanned {
                generation: 3,
                entries: Vec::new(),
                truncated: true,
                elapsed_ms: 12,
                library_nodes: 0,
                errors: Vec::new(),
            })),
            "project scan #3: 0 nodes (library 0) in 12 ms (truncated)"
        );
        assert_eq!(
            describe(&Action::Project(ProjectAction::PolicyLoaded {
                policy: kairos_editor_core::project::ScanPolicy::default(),
            })),
            "project policy: 1 system dirs, 2 ignore rules"
        );
    }

    /// A shared style lives in the app registry, so the effect executor writes
    /// it there rather than on a panel instance.
    #[test]
    fn a_shared_skin_edit_reaches_the_registry() {
        let mut app = build();
        let ctx = egui::Context::default();

        apply(
            &mut app,
            Action::SharedStyle(SharedStyleAction::SetField {
                style: COLOR_PICKER,
                path: "border_width".to_owned(),
                value: StyleValue::F32(0.9),
            }),
            &ctx,
        );

        assert_eq!(
            app.shared
                .color_picker()
                .expect("the color picker skin is registered")
                .border_width,
            0.9
        );
    }

    /// The separate Commit path writes the shared file, not a panel's.
    #[test]
    fn a_shared_skin_commit_writes_its_file() {
        let _guard = SKIN_FILE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = scratch_style_root();
        let mut app = build();
        let ctx = egui::Context::default();
        let path = root.join("ColorPicker.ron");
        let _ = std::fs::remove_file(&path);

        apply(
            &mut app,
            Action::SharedStyle(SharedStyleAction::SetField {
                style: COLOR_PICKER,
                path: "border_width".to_owned(),
                value: StyleValue::F32(0.9),
            }),
            &ctx,
        );
        apply(
            &mut app,
            Action::SharedStyle(SharedStyleAction::Commit {
                style: COLOR_PICKER,
            }),
            &ctx,
        );

        let text = std::fs::read_to_string(&path).expect("the shared style file was written");
        assert!(text.contains("border_width: 0.9"), "{text}");
    }

    /// Quit is host work: the toolbar only *requests* it, the reducer answers
    /// with an app effect, and the effect executor turns that into the close
    /// command the backend reads as `FrameOutput::close_requested`.
    #[test]
    fn quitting_asks_the_window_to_close() {
        let _guard = UI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let engine = Engine::headless();

        let mut out = UiOut::new();
        out.push(Action::App(AppAction::QuitRequested));
        app.pump(out);

        // Effects run in the next frame's host phase, before the pass; the
        // close request therefore belongs to that frame's viewport output.
        let ctx = egui::Context::default();
        app.begin_frame(&ctx);
        let output = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));

        let close_requested = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|viewport| {
                viewport
                    .commands
                    .iter()
                    .any(|command| matches!(command, egui::ViewportCommand::Close))
            });
        assert!(
            close_requested,
            "the quit action must reach the backend as a close request"
        );
    }

    #[test]
    fn host_view_exposes_the_engine_profiler() {
        let engine = Engine::headless();
        let host = AppHost::new();
        let scope = HostScope {
            console: host.console.clone(),
            thumbnails: host.thumbnails.clone(),
            profiler: Some(engine.profiler()),
        };
        assert!(
            scope.profiler().is_some(),
            "the pass borrows the loop engine's recorder"
        );
    }

    #[test]
    fn host_view_exposes_the_console_history() {
        let host = AppHost::new();
        let scope = HostScope {
            console: host.console.clone(),
            thumbnails: host.thumbnails.clone(),
            profiler: None,
        };
        host.console.push(&kairos_log::Record::new(
            kairos_log::Level::Info,
            "test",
            "hello console",
        ));

        let mut seen = Vec::new();
        scope.console_lines(&mut |lines| seen = lines.to_vec());

        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].message, "hello console");
        assert_eq!(seen[0].target, "test");
    }

    #[test]
    fn closing_the_profiler_tab_stops_recording() {
        let _guard = UI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let engine = Engine::headless();
        engine.profiler().set_recording(true);

        let ctx = egui::Context::default();
        app.begin_frame(&ctx);
        let _ = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));
        assert!(
            engine.profiler().is_recording(),
            "the first pass starts the capture"
        );

        app.state.layout.close(panels::PROFILER);
        app.begin_frame(&ctx);
        let _ = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));

        assert!(
            !engine.profiler().is_recording(),
            "closing the Profiler tab stops the host capture"
        );
        engine.profiler().clear();
    }

    #[test]
    fn scene_panel_can_be_detached_into_a_window() {
        let mut app = build();
        let id = app.host.id_of(panels::SCENE).expect("scene instance");
        let engine = Engine::headless();
        let host_scope = HostScope {
            console: app.host_view.console.clone(),
            thumbnails: app.host_view.thumbnails.clone(),
            profiler: Some(engine.profiler()),
        };

        let mut out = UiOut::new();
        {
            let EditorApp { state, host, .. } = &mut app;
            let frame = Frame::with_host(state, &host_scope);
            let mut viewer = host.viewer(&mut out, frame);
            assert!(
                viewer.allowed_in_windows(id),
                "Scene must be draggable into a floating window"
            );
        }
        assert!(out.is_empty());
    }

    #[test]
    fn showing_a_docked_panel_focuses_its_tab() {
        let mut app = build();
        let ctx = egui::Context::default();
        let scene = app.host.id_of(panels::SCENE).expect("scene instance");
        let console = app.host.id_of(panels::CONSOLE).expect("console instance");

        assert!(app.dock.focus_tab(scene));
        assert_eq!(app.dock.focused_tab(), Some(scene));

        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::ShowPanel {
            panel: panels::CONSOLE,
        }));
        app.pump(out);
        app.begin_frame(&ctx);

        assert_eq!(app.dock.focused_tab(), Some(console));
        assert_eq!(
            app.dock.valid_surface_indices().len(),
            1,
            "a panel that already has a tab must not become a second window"
        );
    }

    #[test]
    fn showing_a_panel_without_a_dock_tab_floats_it() {
        let mut app = build();
        let ctx = egui::Context::default();
        let profiler = app.host.id_of(panels::PROFILER).expect("profiler instance");

        app.state.layout.close(panels::PROFILER);
        app.begin_frame(&ctx);
        let closed_tabs = open_panel_count(&app);
        assert_eq!(app.dock.tabs().len(), closed_tabs);
        assert_eq!(app.dock.valid_surface_indices().len(), 1);

        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::ShowPanel {
            panel: panels::PROFILER,
        }));
        app.pump(out);
        app.begin_frame(&ctx);

        let reopened = app.host.id_of(panels::PROFILER).expect("profiler reopened");
        assert_ne!(reopened, profiler, "reopening creates a new instance");
        assert_eq!(app.dock.tabs().len(), closed_tabs + 1);
        assert_eq!(app.dock.valid_surface_indices().len(), 2);
        assert_eq!(app.dock.focused_tab(), Some(reopened));
        assert_eq!(
            app.dock.main_surface().tabs_count(),
            closed_tabs,
            "the reopened panel is a floating window, not a tab in the main surface"
        );
    }

    #[test]
    fn focusing_a_panel_without_a_dock_tab_docks_it() {
        let mut app = build();
        let ctx = egui::Context::default();

        app.state.layout.close(panels::PROFILER);
        app.begin_frame(&ctx);

        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::FocusPanel {
            panel: panels::PROFILER,
        }));
        app.pump(out);
        app.begin_frame(&ctx);

        let reopened = app.host.id_of(panels::PROFILER).expect("profiler reopened");
        let expected = open_panel_count(&app);
        assert_eq!(app.dock.tabs().len(), expected);
        assert_eq!(
            app.dock.valid_surface_indices().len(),
            1,
            "Focus docks a missing panel instead of opening a window"
        );
        assert_eq!(app.dock.focused_tab(), Some(reopened));
        assert_eq!(app.dock.main_surface().tabs_count(), expected);
    }

    #[test]
    fn close_and_reopen_a_panel_converges_instance_and_tab() {
        let mut app = build();
        let ctx = egui::Context::default();

        app.state.layout.close(panels::CONSOLE);
        app.begin_frame(&ctx);
        let closed = open_panel_count(&app);
        assert_eq!(app.host.ids().count(), closed);
        assert_eq!(app.dock.tabs().len(), closed);

        let mut out = UiOut::new();
        out.push(Action::Layout(LayoutAction::TogglePanel {
            panel: panels::CONSOLE,
        }));
        app.pump(out);
        app.begin_frame(&ctx);

        let expected = open_panel_count(&app);
        assert_eq!(app.host.ids().count(), expected);
        assert_eq!(app.dock.tabs().len(), expected);
        assert_eq!(
            app.dock.focused_tab(),
            app.host.id_of(panels::CONSOLE),
            "opening via Toggle docks and focuses the panel"
        );
    }

    /// Preferences is an ordinary panel: the layout reducer opens it and
    /// ShowPanel floats it, exactly like About.
    #[test]
    fn preferences_opens_as_a_floating_window() {
        let mut app = build();
        let ctx = egui::Context::default();

        apply(
            &mut app,
            Action::Layout(LayoutAction::ShowPanel {
                panel: panels::PREFERENCES,
            }),
            &ctx,
        );

        let id = app
            .host
            .id_of(panels::PREFERENCES)
            .expect("preferences instance");
        assert!(app.dock.contains_tab(id));
        assert_eq!(app.dock.valid_surface_indices().len(), 2);
        assert_eq!(app.dock.focused_tab(), Some(id));
    }

    /// Opening Preferences makes the app snapshot the live panels' field tables;
    /// they are the rows the editor draws.
    #[test]
    fn opening_preferences_snapshots_the_live_skins() {
        let mut app = build();
        let ctx = egui::Context::default();
        assert!(app.skins.is_empty(), "nothing to draw while closed");

        apply(
            &mut app,
            Action::Layout(LayoutAction::ShowPanel {
                panel: panels::PREFERENCES,
            }),
            &ctx,
        );

        assert!(
            app.skins.iter().any(|skin| skin.panel == panels::CONSOLE),
            "the open console is editable"
        );
        assert!(
            app.skins
                .iter()
                .any(|skin| skin.panel == panels::PREFERENCES),
            "the editor can edit its own skin"
        );
        assert!(
            !app.skins.iter().any(|skin| skin.panel == panels::ABOUT),
            "About is not open, so it has no live style to edit"
        );
    }

    /// A field edit is host work: the executor writes it into the owning
    /// panel's live style, and the next snapshot shows the new value.
    #[test]
    fn a_skin_edit_reaches_the_owning_panel() {
        let mut app = build();
        let ctx = egui::Context::default();

        apply(
            &mut app,
            Action::Style(StyleAction::SetField {
                panel: panels::CONSOLE,
                path: "toolbar_height".to_owned(),
                value: StyleValue::F32(30.0),
            }),
            &ctx,
        );

        let console = app
            .host
            .skins(&app.state)
            .into_iter()
            .find(|skin| skin.panel == panels::CONSOLE)
            .expect("console skin");
        let height = console
            .fields
            .iter()
            .find(|field| field.path == "toolbar_height")
            .expect("toolbar_height");
        assert_eq!(height.value, StyleValue::F32(30.0));
    }

    /// The save boundary: an edit alone writes nothing; the commit writes the
    /// panel's RON file with the new value.
    #[test]
    fn a_skin_commit_writes_the_panel_file() {
        let _guard = SKIN_FILE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let dir = scratch_style_root().to_path_buf();
        let ctx = egui::Context::default();
        let file = dir.join("ConsoleStyle.ron");

        apply(
            &mut app,
            Action::Style(StyleAction::SetField {
                panel: panels::CONSOLE,
                path: "toolbar_height".to_owned(),
                value: StyleValue::F32(30.0),
            }),
            &ctx,
        );
        let before = std::fs::read_to_string(&file).unwrap_or_default();
        assert!(
            !before.contains("toolbar_height: 30.0"),
            "a field edit must not write the file: {before}"
        );

        apply(
            &mut app,
            Action::Style(StyleAction::Commit {
                panel: panels::CONSOLE,
            }),
            &ctx,
        );

        let text = std::fs::read_to_string(&file).expect("the commit writes the skin");
        assert!(text.contains("toolbar_height: 30.0"), "{text}");
    }

    /// Reset is a complete action: it restores the code default and writes it
    /// in the same click, with no separate Commit.
    #[test]
    fn a_skin_reset_restores_defaults_and_writes_the_file() {
        let _guard = SKIN_FILE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let dir = scratch_style_root().to_path_buf();
        let ctx = egui::Context::default();

        // The scratch root is shared by the suite and may hold an earlier
        // test's edit; reset once so the captured value is the code default.
        apply(
            &mut app,
            Action::Style(StyleAction::Reset {
                panel: panels::CONSOLE,
            }),
            &ctx,
        );
        let default = console_height(&app);

        apply(
            &mut app,
            Action::Style(StyleAction::SetField {
                panel: panels::CONSOLE,
                path: "toolbar_height".to_owned(),
                value: StyleValue::F32(30.0),
            }),
            &ctx,
        );
        assert_eq!(console_height(&app), StyleValue::F32(30.0));

        apply(
            &mut app,
            Action::Style(StyleAction::Reset {
                panel: panels::CONSOLE,
            }),
            &ctx,
        );

        assert_eq!(
            console_height(&app),
            default,
            "the live style is back to its code default"
        );
        let text = std::fs::read_to_string(dir.join("ConsoleStyle.ron"))
            .expect("reset writes the panel skin");
        assert!(
            !text.contains("toolbar_height: 30.0"),
            "reset persists the default, not the discarded edit: {text}"
        );
    }

    /// Shared styles reset through the registry, and the same click persists
    /// the code default.
    #[test]
    fn a_shared_skin_reset_restores_defaults_and_writes_the_file() {
        let _guard = SKIN_FILE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let dir = scratch_style_root().to_path_buf();
        let ctx = egui::Context::default();
        let default = app.shared.toolbar().expect("toolbar").height;

        apply(
            &mut app,
            Action::SharedStyle(SharedStyleAction::SetField {
                style: TOOLBAR,
                path: "height".to_owned(),
                value: StyleValue::F32(99.0),
            }),
            &ctx,
        );
        assert_eq!(app.shared.toolbar().expect("toolbar").height, 99.0);

        apply(
            &mut app,
            Action::SharedStyle(SharedStyleAction::Reset { style: TOOLBAR }),
            &ctx,
        );

        assert_eq!(app.shared.toolbar().expect("toolbar").height, default);
        let text =
            std::fs::read_to_string(dir.join("ToolBar.ron")).expect("reset writes the shared skin");
        assert!(
            !text.contains("height: 99.0"),
            "reset persists the default, not the discarded edit: {text}"
        );
    }

    fn console_height(app: &EditorApp) -> StyleValue {
        app.host
            .skins(&app.state)
            .into_iter()
            .find(|skin| skin.panel == panels::CONSOLE)
            .expect("console skin")
            .fields
            .into_iter()
            .find(|field| field.path == "toolbar_height")
            .expect("toolbar_height")
            .value
    }

    /// The field cursors survive the real dock: a numeric row publishes the
    /// scrub cursor over its name and the text cursor over its box. The rows
    /// live below the fold of the Console skin, so the pass scrolls first and
    /// lets the smooth scroll settle before reading the painted rects.
    #[test]
    fn preferences_publish_the_scrub_and_text_cursors() {
        let _guard = UI_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut app = build();
        let engine = Engine::headless();
        let ctx = egui::Context::default();

        app.begin_frame(&ctx);
        let _ = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));
        apply(
            &mut app,
            Action::Layout(LayoutAction::ShowPanel {
                panel: panels::PREFERENCES,
            }),
            &ctx,
        );
        app.begin_frame(&ctx);
        let mut output = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));

        for _ in 0..4 {
            let mut input = raw_input();
            input.events = vec![
                egui::Event::PointerMoved(egui::pos2(800.0, 450.0)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0.0, -12.0),
                    modifiers: egui::Modifiers::NONE,
                    phase: egui::TouchPhase::Move,
                },
            ];
            app.begin_frame(&ctx);
            output = ctx.run_ui(input, |ui| app.ui(&engine, ui));
        }
        for _ in 0..90 {
            app.begin_frame(&ctx);
            output = ctx.run_ui(raw_input(), |ui| app.ui(&engine, ui));
        }

        let mut name = None;
        for clipped in &output.shapes {
            if let egui::Shape::Text(text) = &clipped.shape
                && text.galley.text() == "Search radius"
            {
                name = Some(text.pos);
            }
        }
        let name = name.expect("the Console skin keeps a Search radius token");

        let mut hover = |offset: egui::Vec2| {
            let mut input = raw_input();
            input.events = vec![egui::Event::PointerMoved(name + offset)];
            app.begin_frame(&ctx);
            ctx.run_ui(input, |ui| app.ui(&engine, ui))
                .platform_output
                .cursor_icon
        };

        assert_eq!(
            hover(egui::vec2(4.0, 5.0)),
            egui::CursorIcon::ResizeHorizontal
        );
        assert_eq!(hover(egui::vec2(200.0, 5.0)), egui::CursorIcon::Text);
    }

    /// Host work enters through the inbox and is applied through the same
    /// reducer as a UI action. Order is preserved: every action sees what the
    /// ones before it produced.
    #[test]
    fn inbox_actions_are_applied_in_order() {
        let mut app = build();
        let ctx = egui::Context::default();

        app.inbox.push_back(Action::Layout(LayoutAction::ShowPanel {
            panel: panels::ABOUT,
        }));
        app.inbox
            .push_back(Action::Layout(LayoutAction::ClosePanel {
                panel: panels::ABOUT,
            }));

        app.begin_frame(&ctx);

        assert!(
            !app.state.layout.is_open(panels::ABOUT),
            "the last action wins: Close ran after Show"
        );
        assert_eq!(
            app.host.id_of(panels::ABOUT),
            None,
            "the panel was closed before its placement effect could run"
        );
        assert!(app.inbox.is_empty(), "the host phase drained the inbox");
    }

    /// An action produced by executing an effect is a *completion*: it is
    /// applied at the next frame's host phase, never in the frame that ran the
    /// effect. That one-frame boundary is what keeps action and effect from
    /// cycling, so the host phase needs no settle budget. This is the channel
    /// every later async feature lands on (scan results first).
    #[test]
    fn an_effects_completion_is_applied_on_the_next_frame() {
        let mut app = build();
        let ctx = egui::Context::default();
        assert_eq!(app.state.project.tree.revision(), 0);

        app.inbox
            .push_back(Action::Project(ProjectAction::ScanRequested {
                reason: ScanReason::Manual,
                focus: None,
            }));

        app.begin_frame(&ctx);

        assert_eq!(app.state.project.scan.generation, 1, "the request ran");
        assert!(
            app.state.project.scan.running,
            "the frame that ran the effect has not seen its completion"
        );
        assert_eq!(
            app.state.project.tree.revision(),
            0,
            "no completion is applied in the frame that ran the effect"
        );
        assert!(
            !app.inbox.is_empty(),
            "the completion waits in the inbox for the next frame"
        );

        app.begin_frame(&ctx);

        assert!(
            !app.state.project.scan.running,
            "the completion landed on the next frame"
        );
        assert_eq!(app.state.project.tree.revision(), 1);
        assert!(
            app.inbox.is_empty(),
            "nothing is left over after the completion was applied"
        );
    }
}
