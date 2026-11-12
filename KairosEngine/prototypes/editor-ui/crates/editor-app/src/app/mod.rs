//! Runnable editor host: composition root, explicit frame phases, effect
//! execution.
//!
//! # Read this first: the whole prototype in one page
//!
//! ```text
//! main.rs                    eframe::run_native -> EditorApp::new
//! app/mod.rs                 impl eframe::App for EditorApp  <-- frame entry
//!   logic()                    -> pipeline::render_viewports  (phase 1, once)
//!   ui()                       -> pipeline::build_ui          (phase 2, per pass)
//!                                 + pipeline::pump            (phases 3+4, last pass)
//! app/pipeline.rs            the phase implementations
//! app/effects.rs             Effect -> host IO/GPU; completions -> App::complete
//! app/shell.rs               top bar, keyboard shortcuts, modal overlay
//! app/seed.rs                demo state so the first frame has something to show
//! ```
//!
//! **Where is a panel drawn?** Not in this crate: PanelHost::draw (in
//! `editor-components/src/panel/host.rs`) is the single drawing entry. This crate
//! only calls it from `pipeline::build_ui`, then drains the resulting actions.
//! The module docs of `editor-components/src/panel/mod.rs` show that call chain.
//!
//! # Frame phases
//!
//! ```text
//! 1. RenderViewports   host renders every bound viewport from the host camera
//!                      (the previous frame's camera intents are already applied)
//!                      -- runs in eframe::App::logic, once per frame
//! 2. BuildUi           top bar + PanelHost::draw (tabs/slots/bodies) + modal;
//!                      panels read Frame, emit Actions into UiOut. egui may call
//!                      this more than once per frame; each pass starts from an
//!                      empty UiOut
//! 3. dispatch actions  reduce -> Effects (last pass only: !ctx.will_discard())
//! 4. execute effects   IO/GPU; async completions go back through App::complete
//! 5. Present           eframe renders egui; request repaint while a panel asks
//!                      for continuous repaint (the viewport panel does)
//! ```
//!
//! The one-frame viewport latency is deliberate and matches the real engine
//! (Engine::update runs before the UI there): a camera intent emitted in phase 3
//! is applied by the host in phase 4 and rendered in the *next* frame's phase 1.
//!
//! # Ownership
//!
//! EditorApp is the composition root (ADR 0008/0009): it owns the store (App), the
//! panel host, the host GPU/log resources and the frame action buffer. It owns no
//! panel-typed fields; panels are registered in `EditorApp::new` and created by the
//! host. It is the only crate member that knows both the UI layer and the wgpu
//! viewport host; `editor-components` sees only `HostView::viewport_texture`.

mod effects;
mod pipeline;
mod seed;
mod shell;

use editor_components_prototype::{DockSlot, PanelHost, PanelRegistry, UiOut, ViewportPanel};
use editor_core_prototype::{App, PanelId, ViewportId};
use ui_widgets_prototype::Theme;

use crate::gallery::{GALLERY_PANEL, GalleryPanel};
use crate::host::HostState;

/// The runnable prototype editor.
pub struct EditorApp {
    /// The single state tree + reducers + effect outbox (the real core).
    core: App,
    /// Panel instances + dock placement. No per-panel fields anywhere.
    panels: PanelHost,
    /// Host resources: wgpu viewport host + console log (the engine stand-in).
    host: HostState,
    /// Frame action buffer. Only the host drains it.
    ui_out: UiOut,
    frame_count: u64,
}

impl EditorApp {
    /// Build the editor. This is the only place that knows the concrete panel
    /// types; everything after this point goes through the `Panel` trait.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Theme::dark().install(&cc.egui_ctx);

        let mut core = App::new();
        seed::seed_demo_state(&mut core);

        // Store-independent panels come from the components crate's defaults;
        // the composition root only adds panels that need host resources.
        let mut registry = PanelRegistry::editor_defaults();
        registry.register(PanelId::VIEWPORT, DockSlot::Center, |_state| {
            Box::new(ViewportPanel::new(ViewportId::SCENE))
        });
        registry.register(GALLERY_PANEL, DockSlot::Center, |_state| {
            Box::new(GalleryPanel::new())
        });

        let mut host = HostState::new(cc);
        host.log("Prototype started: registry panels + wgpu offscreen viewport + real reducers.");

        Self {
            core,
            panels: PanelHost::new(registry),
            host,
            ui_out: UiOut::new(),
            frame_count: 0,
        }
    }
}

impl eframe::App for EditorApp {
    /// Phase 1 (once per frame, before any UI pass): render viewports.
    ///
    /// eframe calls `logic` exactly once per frame and outside egui's UI passes,
    /// which makes it the correct home for work that must not repeat on a
    /// discarded pass (the real editor's RenderViewports phase).
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_count += 1;
        self.render_viewports();
    }

    /// Phases 2-4: build the UI, then commit the collected actions on the last
    /// pass only. Repaint policy lives in the panel host aggregation (a panel that
    /// returns `wants_continuous_repaint` triggers the request in `build_ui`), so
    /// there is no second repaint decision here.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.build_ui(ui);
        if !ui.ctx().will_discard() {
            self.pump(ui.ctx());
        }
    }
}
