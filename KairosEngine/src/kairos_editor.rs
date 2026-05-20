use egui::Visuals;
use kairos_engine::log::Log;


pub mod consts;
pub mod ui;
pub mod runtime;

pub struct KairosEngine {
    ui_context: ui::Context,
    log: Log,
}

impl KairosEngine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let ui_context = ui::Context::new();
        let log = Log::new();

        Ok(Self{
            ui_context,
            log,
        })
    }

    fn update(&mut self, ctx: &egui::Context) {
        let mut visuals = Visuals::dark();
        visuals.button_frame = true;
        ctx.set_visuals(visuals);

        self.ui_context.handle(ctx, &mut self.log);
        self.ui_context.darw(ctx, &mut self.log);
    }

    fn on_exit(&mut self) {
        
    }
}