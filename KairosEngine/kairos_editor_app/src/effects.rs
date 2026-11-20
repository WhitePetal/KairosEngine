//! Executes reducer effects against host resources.
//!
//! This module is the only place that turns [`Effect`] data into host work:
//! mutations of the dock, and the app-level requests only the backend can
//! fulfil. `Effect` is a closed, domain-partitioned enum: adding an effect adds
//! a variant and makes the dispatch match fail to compile until the arm is
//! written. Effect-specific policy belongs in the handler; never add shared
//! preconditions to `apply` (the old single `focus_tab` preamble made every
//! future effect depend on tab focus).

use egui::{Context, Rect};
use kairos_editor_component::{PanelHost, PanelInstanceId, shared_style::SharedStyles};
use kairos_editor_core::{
    AppEffect, Effect, LayoutEffect, PanelId, SharedStyleEffect, StyleEffect,
};
use kairos_editor_docking::docker::DockState;

/// Runs effects in order against the live panel host, dock tree and shared
/// style registry.
pub(crate) struct EffectExecutor<'a> {
    host: &'a mut PanelHost,
    dock: &'a mut DockState<PanelInstanceId>,
    shared: &'a mut SharedStyles,
}

impl<'a> EffectExecutor<'a> {
    pub(crate) fn new(
        host: &'a mut PanelHost,
        dock: &'a mut DockState<PanelInstanceId>,
        shared: &'a mut SharedStyles,
    ) -> Self {
        Self { host, dock, shared }
    }

    /// Dispatch one effect. Compiler-enforced extension point: a new `Effect`
    /// variant fails this match until it has a handler.
    pub(crate) fn apply(&mut self, effect: Effect, ctx: &Context) {
        match effect {
            Effect::Layout(effect) => self.apply_layout(effect, ctx),
            Effect::App(effect) => self.apply_app(effect, ctx),
            Effect::Style(effect) => self.apply_style(effect, ctx),
            Effect::SharedStyle(effect) => self.apply_shared_style(effect, ctx),
        }
    }

    /// App lifecycle: the app does not own the window (the engine's loop does),
    /// so quitting is expressed as the request the platform layer already
    /// understands. `EditorLayer::render` reads it back as
    /// `FrameOutput::close_requested`, and the loop runs `Layer::on_exit` and
    /// exits — the same teardown a host-initiated exit gets.
    fn apply_app(&mut self, effect: AppEffect, ctx: &Context) {
        match effect {
            AppEffect::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    /// Skin edits: write the value into the owning panel's live style (the
    /// next pass draws with it), and persist a panel's file on commit.
    ///
    /// The two are separate effects on purpose. `SetField` runs on every
    /// changed value, including every frame of a drag; `Persist` runs once when
    /// the interaction ends. Writing the file per frame is exactly the legacy
    /// defect this editor replaces (a selected style page wrote its TOML about
    /// 60 times a second).
    fn apply_style(&mut self, effect: StyleEffect, ctx: &Context) {
        match effect {
            StyleEffect::SetField { panel, path, value } => {
                if !self.host.set_skin_field(panel, &path, &value) {
                    kairos_log::warn!(
                        target: "kairos_editor",
                        "skin {panel}: no editable field {path}"
                    );
                }
            }
            StyleEffect::EditList { panel, path, edit } => {
                if !self.host.edit_skin_list(panel, &path, edit) {
                    kairos_log::warn!(
                        target: "kairos_editor",
                        "skin {panel}: no list {path}"
                    );
                }
            }
            StyleEffect::Persist { panel } => {
                if let Err(error) = self.host.save_skin(panel, ctx) {
                    kairos_log::warn!(target: "kairos_editor", "skin {panel}: {error}");
                }
            }
        }
    }

    /// Shared-style edits: live write, then persist/reload on the app-owned
    /// registry. Same gesture boundary as a panel skin, different owner.
    fn apply_shared_style(&mut self, effect: SharedStyleEffect, ctx: &Context) {
        match effect {
            SharedStyleEffect::SetField { style, path, value } => {
                if !self.shared.set_field(style, &path, &value) {
                    kairos_log::warn!(
                        target: "kairos_editor",
                        "shared skin {style}: no editable field {path}"
                    );
                }
            }
            SharedStyleEffect::EditList { style, path, edit } => {
                if !self.shared.edit_list(style, &path, edit) {
                    kairos_log::warn!(
                        target: "kairos_editor",
                        "shared skin {style}: no list {path}"
                    );
                }
            }
            SharedStyleEffect::Persist { style } => {
                if let Err(error) = self.shared.save(style, ctx.theme()) {
                    kairos_log::warn!(target: "kairos_editor", "shared skin {style}: {error}");
                }
                self.shared.preload(ctx);
            }
            SharedStyleEffect::Reload { style } => {
                if !self.shared.reload(style, ctx.theme()) {
                    kairos_log::warn!(
                        target: "kairos_editor",
                        "shared skin {style}: not registered"
                    );
                }
                self.shared.preload(ctx);
            }
        }
    }

    fn apply_layout(&mut self, effect: LayoutEffect, ctx: &Context) {
        match effect {
            LayoutEffect::ShowPanel { panel } => self.show_panel(panel, ctx),
            LayoutEffect::FocusPanel { panel } => self.focus_panel(panel),
        }
    }

    /// Show: focus an existing tab; otherwise float the panel in a new window
    /// (unless the panel forbids floating, in which case dock it).
    fn show_panel(&mut self, panel: PanelId, ctx: &Context) {
        let Some(id) = self.host.id_of(panel) else {
            return;
        };
        if self.dock.focus_tab(id) {
            return;
        }

        if self.host.allowed_in_windows(id) {
            let rect = floating_rect(ctx, self.dock);
            self.dock.open_in_window(id, rect);
        } else {
            self.dock_panel(id);
        }
    }

    /// Focus: focus an existing tab; otherwise dock the panel in the focused
    /// leaf. Callers that mean "bring it to me" do not get a new window.
    fn focus_panel(&mut self, panel: PanelId) {
        let Some(id) = self.host.id_of(panel) else {
            return;
        };
        if self.dock.focus_tab(id) {
            return;
        }
        self.dock_panel(id);
    }

    /// Add a panel with no tab yet to the dock's focused leaf and focus it.
    fn dock_panel(&mut self, id: PanelInstanceId) {
        self.dock.append_tab(id);
        self.dock.focus_tab(id);
    }
}

/// Default rect for a panel opened from the Window menu when its tab is not in
/// the dock: centred on the editor and cascaded down-right by one step per
/// existing floating window so repeated opens do not stack exactly.
fn floating_rect(ctx: &Context, dock: &DockState<PanelInstanceId>) -> Rect {
    let bounds = ctx.content_rect();
    let size = egui::vec2(bounds.width() * 0.42, bounds.height() * 0.42)
        .clamp(egui::vec2(360.0, 260.0), egui::vec2(640.0, 480.0));
    let windows = dock
        .valid_surface_indices()
        .iter()
        .filter(|surface| !surface.is_main())
        .count() as f32;
    let origin = (bounds.center() - size * 0.5 + egui::vec2(24.0, 24.0) * windows)
        .clamp(bounds.min, (bounds.max - size).max(bounds.min));
    Rect::from_min_size(origin, size)
}
