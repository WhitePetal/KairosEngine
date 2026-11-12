//! Frame phases 1-4: render viewports, build the UI, dispatch actions, execute
//! effects.
//!
//! These are `impl EditorApp` methods kept in their own file so the host in
//! `app/mod.rs` reads as a tiny `eframe::App` adapter and this file reads as the
//! frame pipeline. The drawing entry for panels is `PanelHost::draw` (called from
//! `build_ui`); everything below that is inside the components crate.

use std::time::Duration;

use editor_components_prototype::{Frame as UiFrame, UiOut};

use super::EditorApp;

impl EditorApp {
    /// Phase 1: render every bound viewport from the host camera into its
    /// offscreen target. The panel then samples those targets as `TextureId`.
    ///
    /// Driven by `ViewportState` data, not by panel identity: the app shell does
    /// not know which panel owns which viewport, so a second scene/game viewport
    /// is data, not a code change.
    pub(super) fn render_viewports(&mut self) {
        let selected = self
            .core
            .state
            .selection
            .selected_scene_object
            .map(|object| object.0);
        let mut errors = Vec::new();
        {
            let Some(viewport_host) = self.host.viewport.as_mut() else {
                return;
            };
            for (viewport, info) in self.core.state.viewports.iter() {
                if !info.bound {
                    continue;
                }
                viewport_host.set_selected(viewport.0, selected);
                if let Err(error) = viewport_host.render(viewport.0) {
                    errors.push((viewport.0, error.to_string()));
                }
            }
        }
        for (viewport, error) in errors {
            self.host
                .log(format!("viewport {viewport} render failed: {error}"));
        }
    }

    /// Phase 2: draw the shell. Panels only read `Frame` and push Actions.
    ///
    /// egui may run the UI pass more than once per frame (`max_passes = 2` when a
    /// layout requests a discard). Every pass starts from an empty `UiOut`, and
    /// the app shell only commits (dispatch + effects) when
    /// `Context::will_discard()` is false, so multi-pass cannot double-dispatch.
    pub(super) fn build_ui(&mut self, ui: &mut egui::Ui) {
        self.ui_out = UiOut::new();
        self.handle_shortcuts(ui.ctx());

        egui::Panel::top("proto_toolbar")
            .exact_size(44.0)
            .show(ui, |ui| self.draw_top_bar(ui));

        self.panels.sync(&self.core.state);

        let wants_continuous;
        {
            let frame = UiFrame::with_host(&self.core.state, &self.host);
            let output = self.panels.draw(ui, &frame, &mut self.ui_out);
            wants_continuous = output.wants_continuous_repaint;
        }

        self.draw_modal(ui.ctx());

        if wants_continuous {
            ui.ctx().request_repaint_after(Duration::from_millis(16));
        }
    }

    /// Phases 3+4: dispatch the actions collected while building the UI, then
    /// execute the effects (IO/GPU). Completions are routed back into the
    /// reducer immediately, so the visible state catches up in this frame.
    pub(super) fn pump(&mut self, ctx: &egui::Context) {
        let actions = self.ui_out.take();
        if actions.is_empty() {
            return;
        }
        let mut effects = Vec::new();
        for action in actions {
            effects.extend(self.core.dispatch(action));
        }

        let mut budget = 64;
        while !effects.is_empty() {
            if budget == 0 {
                self.host
                    .log("effect budget exhausted (possible effect cycle); dropping effects");
                break;
            }
            budget -= 1;
            let mut next = Vec::new();
            for effect in effects.drain(..) {
                next.extend(self.apply_effect(ctx, effect));
            }
            effects = next;
        }
    }
}
