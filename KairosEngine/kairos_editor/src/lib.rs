use egui::Visuals;
use kairos_ecs::world::World;
use kairos_engine::{
    Engine, graphics::graphics_graph::GraphicsCommand, kairos_game::KairosGame, log::Log,
};

pub mod asset_registry;
pub mod camera;
mod color;
pub mod consts;
pub mod dialog;
pub mod editor_assets;
/// The editor's half of the engine's frame loop: egui state and the UI pass.
pub mod host;
pub mod project_path_tree;
pub mod project_watcher;
pub mod syntax;
pub mod ui;

/// Environment variable that starts a profiling capture for this run.
///
/// This is the development-build switch before the editor has a Profiler panel:
/// set `KAIROS_PROFILE=1`, run the editor, then read the capture written under
/// [`kairos_profile::DEFAULT_CAPTURE_DIR`] on exit. The engine owns the
/// recorder; this variable only decides when the host starts it.
const PROFILE_ENV: &str = "KAIROS_PROFILE";

/// The editor's authoring state: the panel tree, the UI context, and the log.
///
/// The engine is deliberately *not* held here. The host loop
/// (`kairos_engine::runtime::EngineRunner`) owns the engine and the frame
/// boundary; the editor only borrows the engine while it draws. That keeps the
/// editor a layer on the engine's loop rather than a second loop of its own.
pub struct Editor {
    ui_context: ui::Context,
    log: Log,
}

impl Editor {
    /// Builds the editor on an already-booted engine.
    ///
    /// This is where the editor's own install and the game assembly run:
    /// "bootstrap, then the host's additions, then assembly". The engine must
    /// be fully constructed — see [`install`] for the ordering precondition.
    pub fn new(
        egui_ctx: &egui::Context,
        engine: &mut Engine,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // The editor's own assets and camera go on immediately after the engine
        // rails are booted, and before the game is assembled on top.
        install(&mut engine.world);
        // The game is a stateless assembly entry: its `new` spawns the demo
        // scene and registers the drifting-listener system into the engine's
        // rails. The value it returns is not kept — per-frame work rides the
        // schedule now.
        let _ = KairosGame::new(engine);

        if std::env::var_os(PROFILE_ENV).is_some() {
            engine.profiler().set_recording(true);
            log::info!(
                "profiling this run; the capture is written to {}/ when the editor exits",
                kairos_profile::DEFAULT_CAPTURE_DIR
            );
        }

        Ok(Self {
            ui_context: ui::Context::new(egui_ctx)?,
            log: Log::new(),
        })
    }

    pub(crate) fn handle_ui(&mut self, engine: &mut Engine, ui: &mut egui::Ui) {
        self.ui_context.handle(engine, ui);
    }

    pub(crate) fn draw_ui(&mut self, engine: &Engine, ui: &mut egui::Ui) {
        let mut visuals = Visuals::dark();
        visuals.button_frame = true;
        ui.set_visuals(visuals);
        self.ui_context.darw(ui, engine, &mut self.log);
    }

    pub(crate) fn render_ui(&mut self, engine: &mut Engine) -> Vec<GraphicsCommand> {
        self.ui_context.render(engine)
    }

    /// Stop profiling and write the capture, if this run recorded anything.
    ///
    /// `flush` hands the last (still open) engine frame to the sink, because the
    /// next `Engine::update` — the normal flush point — never comes. The three
    /// files land under `Library/Profiler`; their exact paths are logged.
    pub(crate) fn on_exit(&self, engine: &Engine) {
        let profiler = engine.profiler();

        profiler.flush();
        profiler.set_recording(false);
        if profiler.session().is_empty() {
            return;
        }

        match profiler.save_capture(kairos_profile::DEFAULT_CAPTURE_DIR) {
            Ok(files) => log::info!("profiler capture saved: {}", files.json.display()),
            Err(error) => log::error!("saving the profiler capture failed: {error}"),
        }
    }
}

/// Installs the editor's own assets and camera into `world`.
///
/// This is the editor's single install entry point — the host-side counterpart
/// to the engine's private `build_world`. The engine boots its subsystems;
/// this adds what only the editor needs, in one explicit step: the
/// [`editor_assets::text`] / [`editor_assets::toml`] stores and loaders, the
/// [`syntax`] highlighting settings, the [`ui::inspector::texture`] store, and
/// the [`camera`] controller.
///
/// The order is a convenience grouping, not a precondition: none of the five
/// depends on another. It matches the order they were installed in while they
/// lived in the engine's `build_world`.
///
/// # Exactly once
///
/// Must be called exactly once, after [`Engine::new`] — the one caller is
/// `Editor::new`, which runs on the engine the host loop already built and
/// installs before `KairosGame::new`. There is deliberately no idempotency guard: a second call
/// would register a second loader and system over the first, silently
/// corrupting the world rather than panicking. The sole caller is known, so a
/// marker resource is not worth it.
///
/// # Panics
///
/// If the schedule rails and the asset core are not installed yet: the four
/// asset installs read the `AssetServer` and `AssetStages` the asset core
/// creates, and [`camera`] registers into the `PostUpdate` stage the rails
/// create. Calling this before an [`Engine`] exists is a bootstrap-order bug.
pub fn install(world: &mut World) {
    editor_assets::text::install(world);
    editor_assets::toml::install(world);
    syntax::install(world);
    ui::inspector::texture::install(world);
    camera::install(world);
}

#[cfg(test)]
mod test;
