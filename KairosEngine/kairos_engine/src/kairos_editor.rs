use crate::{Engine, graphics::graphics_graph::GraphicsCommand, kairos_game::KairosGame, log::Log};
use egui::Visuals;
use kairos_ecs::world::World;
use winit::event::KeyEvent;

pub mod asset_registry;
pub mod camera;
pub mod consts;
pub mod editor_assets;
pub mod project_path_tree;
pub mod project_watcher;
pub mod runtime;
pub mod syntax;
pub mod ui;

pub struct KairosEngine {
    engine: Engine,
    ui_context: ui::Context,
    log: Log,
}

impl KairosEngine {
    pub fn new(egui_ctx: &egui::Context) -> Result<Self, Box<dyn std::error::Error>> {
        let mut engine = Engine::new()?;
        // The editor's own assets and camera go on immediately after the engine
        // rails are booted, and before the game is assembled on top. The order
        // reads "bootstrap, then the host's additions, then assembly" — see
        // [`install`] for why the editor owns this step rather than `Engine`.
        install(&mut engine.world);
        // The game is a stateless assembly entry: its `new` spawns the demo
        // scene and registers the drifting-listener system into the engine's
        // rails. The value it returns is not kept — per-frame work rides the
        // schedule now.
        let _ = KairosGame::new(&mut engine);
        let ui_context = ui::Context::new(egui_ctx)?;
        let log = Log::new();
        Ok(Self {
            engine,
            ui_context,
            log,
        })
    }

    fn update_keyboard_input(&mut self, event: KeyEvent) {
        self.engine.input_engine.update_keyboard_input(event)
    }

    fn update(&mut self) {
        // Frame start: the game's per-frame work (the drifting listener, the
        // audio driver) is scheduled into the rails, so driving the rails *is*
        // the frame — there is no second hand-driven step.
        self.engine.update();
    }

    fn handle_ui(&mut self, ui: &mut egui::Ui) {
        self.ui_context.handle(&mut self.engine, ui);
    }

    fn draw_ui(&mut self, ui: &mut egui::Ui) {
        let mut visuals = Visuals::dark();
        visuals.button_frame = true;
        ui.set_visuals(visuals);
        self.ui_context.darw(ui, &self.engine, &mut self.log);
    }

    fn render_ui(&mut self) -> Vec<GraphicsCommand> {
        self.ui_context.render(&mut self.engine)
    }

    fn on_exit(&mut self) {
        // TODO!
        // self.engine.world.clear();
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
/// `KairosEngine::new`, which installs after `Engine::new()?` and before
/// `KairosGame::new`. There is deliberately no idempotency guard: a second call
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
