//! Executes reducer effects against host resources.
//!
//! This module is the only place that turns [`Effect`] data into host work:
//! mutations of the dock, and the app-level requests only the backend can
//! fulfil. `Effect` is a closed, domain-partitioned enum: adding an effect adds
//! a variant and makes the dispatch match fail to compile until the arm is
//! written. Effect-specific policy belongs in the handler; never add shared
//! preconditions to `apply` (the old single `focus_tab` preamble made every
//! future effect depend on tab focus).

use std::time::Instant;

use egui::{Context, Rect};
use kairos_editor_component::skin::SkinId;
use kairos_editor_component::{PageSkin, PanelHost, PanelInstanceId, shared_style::SharedStyles};
use kairos_editor_core::{
    Action, AppEffect, Effect, LayoutEffect, PanelId, ProjectEffect, SharedStyleEffect, StyleEffect,
};
use kairos_editor_docking::docker::DockState;

use crate::project::ProjectService;

/// Runs effects in order against the live panel host, dock tree, shared style
/// registry and the project service.
pub(crate) struct EffectExecutor<'a> {
    host: &'a mut PanelHost,
    dock: &'a mut DockState<PanelInstanceId>,
    shared: &'a mut SharedStyles,
    /// The file system's host side: scanning (M1), watching (M1), file
    /// operations (M3).
    project: &'a mut ProjectService,
    /// The app's Preferences page snapshot. A skin edit writes the owner, so
    /// the snapshot's copy of that page is refreshed here: the page only reads
    /// it when the user switches to it, and a switch back must show the edit.
    pages: &'a mut Vec<PageSkin>,
    /// The instant the effects run. Timing policy that depends on wall-clock
    /// time (M4's view-file debounce, scan elapsed time) is computed here, not
    /// in the reducer, which must stay a pure function.
    now: Instant,
}

impl<'a> EffectExecutor<'a> {
    pub(crate) fn new(
        host: &'a mut PanelHost,
        dock: &'a mut DockState<PanelInstanceId>,
        shared: &'a mut SharedStyles,
        project: &'a mut ProjectService,
        pages: &'a mut Vec<PageSkin>,
        now: Instant,
    ) -> Self {
        Self {
            host,
            dock,
            shared,
            project,
            pages,
            now,
        }
    }

    /// Dispatch one effect.
    ///
    /// Returns the actions the host work produced. Effects never write
    /// `EditorState`; completions leave here as actions and the app queues them
    /// for the next frame's host phase. A completion is therefore a new frame's
    /// input, never a re-entry into the reducer in the frame that ran the effect,
    /// so action and effect cannot cycle.
    ///
    /// Compiler-enforced extension point: a new `Effect` variant fails this
    /// match until it has a handler.
    pub(crate) fn apply(&mut self, effect: Effect, ctx: &Context) -> Vec<Action> {
        match effect {
            Effect::Layout(effect) => {
                self.apply_layout(effect, ctx);
                Vec::new()
            }
            Effect::App(effect) => {
                self.apply_app(effect, ctx);
                Vec::new()
            }
            Effect::Project(effect) => self.apply_project(effect),
            Effect::Style(effect) => {
                self.apply_style(effect, ctx);
                Vec::new()
            }
            Effect::SharedStyle(effect) => {
                self.apply_shared_style(effect, ctx);
                Vec::new()
            }
        }
    }

    /// Project work: the service owns the file system, so an effect becomes a
    /// service call and whatever the service completed becomes actions.
    fn apply_project(&mut self, effect: ProjectEffect) -> Vec<Action> {
        let now = self.now;
        self.project.execute(effect, now)
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

    /// Skin edits: write the page's value into the owning panel's live style
    /// (the next pass draws with it), and persist a panel's file on commit.
    ///
    /// The two are separate effects on purpose. `Replace` runs on every changed
    /// value, including every frame of a drag; `Persist` runs once when the
    /// interaction ends. Writing the file per frame is exactly the legacy defect
    /// this editor replaces (a selected style page wrote its TOML about 60 times
    /// a second).
    fn apply_style(&mut self, effect: StyleEffect, ctx: &Context) {
        match effect {
            StyleEffect::Replace { panel, skin } => {
                if self.host.replace_skin(panel, skin.into_any()) {
                    self.refresh_page(SkinId::Panel(panel));
                } else {
                    kairos_log::warn!(
                        target: "kairos_editor",
                        "skin {panel}: the page's value is not this panel's style type"
                    );
                }
            }
            StyleEffect::Reset { panel } => {
                if !self.host.reset_skin(panel, ctx) {
                    kairos_log::warn!(
                        target: "kairos_editor",
                        "skin {panel}: no editable style to reset"
                    );
                    return;
                }
                self.refresh_page(SkinId::Panel(panel));
                if let Err(error) = self.host.save_skin(panel, ctx) {
                    kairos_log::warn!(target: "kairos_editor", "skin {panel}: {error}");
                }
            }
            StyleEffect::Persist { panel } => {
                if let Err(error) = self.host.save_skin(panel, ctx) {
                    kairos_log::warn!(target: "kairos_editor", "skin {panel}: {error}");
                }
            }
        }
    }

    /// Shared-style edits: live write, then reset/persist on the app-owned
    /// registry. Same gesture boundary as a panel skin, different owner.
    fn apply_shared_style(&mut self, effect: SharedStyleEffect, ctx: &Context) {
        match effect {
            SharedStyleEffect::Replace { style, skin } => {
                if self.shared.replace(style, skin.into_any()) {
                    self.refresh_page(SkinId::Shared(style));
                } else {
                    kairos_log::warn!(
                        target: "kairos_editor",
                        "shared skin {style}: the page's value is not this style's type"
                    );
                }
            }
            SharedStyleEffect::Reset { style } => {
                if !self.shared.reset(style, ctx.theme()) {
                    kairos_log::warn!(
                        target: "kairos_editor",
                        "shared skin {style}: not registered"
                    );
                    return;
                }
                self.refresh_page(SkinId::Shared(style));
                if let Err(error) = self.shared.save(style, ctx.theme()) {
                    kairos_log::warn!(target: "kairos_editor", "shared skin {style}: {error}");
                }
                self.shared.preload(ctx);
            }
            SharedStyleEffect::Persist { style } => {
                if let Err(error) = self.shared.save(style, ctx.theme()) {
                    kairos_log::warn!(target: "kairos_editor", "shared skin {style}: {error}");
                }
                self.shared.preload(ctx);
            }
        }
    }

    /// Keep the app's Preferences page snapshot in step with an owner an effect
    /// just wrote.
    ///
    /// The page draws its own copy, so the snapshot's copy is read only when the
    /// user switches back to that page; without this refresh a switch back would
    /// show the value from before the edit or the reset. One page is copied, not
    /// the whole list, and only when a replace or reset landed.
    fn refresh_page(&mut self, id: SkinId) {
        let page = match id {
            SkinId::Panel(panel) => self.host.page(panel),
            SkinId::Shared(style) => self.shared.page(style),
        };
        let Some(page) = page else {
            // Not in the snapshot (Preferences is closed, or the page is no
            // longer editable): the next snapshot rebuild picks the owner up.
            return;
        };
        if let Some(slot) = self.pages.iter_mut().find(|slot| slot.id == id) {
            slot.page = page;
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
