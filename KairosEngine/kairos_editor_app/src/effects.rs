//! Executes reducer effects against host resources.
//!
//! This module is the only place that turns [`Effect`] data into mutations of
//! the dock. `Effect` is a closed, domain-partitioned enum: adding an effect
//! adds a variant and makes the dispatch match fail to compile until the arm is
//! written. Effect-specific policy belongs in the handler; never add shared
//! preconditions to `apply` (the old single `focus_tab` preamble made every
//! future effect depend on tab focus).

use egui::{Context, Rect};
use kairos_editor_component::{PanelHost, PanelInstanceId};
use kairos_editor_core::{Effect, LayoutEffect, PanelId};
use kairos_editor_docking::docker::DockState;

/// Runs effects in order against the live panel host and dock tree.
pub(crate) struct EffectExecutor<'a> {
    host: &'a PanelHost,
    dock: &'a mut DockState<PanelInstanceId>,
}

impl<'a> EffectExecutor<'a> {
    pub(crate) fn new(host: &'a PanelHost, dock: &'a mut DockState<PanelInstanceId>) -> Self {
        Self { host, dock }
    }

    /// Dispatch one effect. Compiler-enforced extension point: a new `Effect`
    /// variant fails this match until it has a handler.
    pub(crate) fn apply(&mut self, effect: Effect, ctx: &Context) {
        match effect {
            Effect::Layout(effect) => self.apply_layout(effect, ctx),
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
