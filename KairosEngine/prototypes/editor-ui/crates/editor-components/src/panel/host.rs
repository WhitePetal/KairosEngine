//! Panel host: mirrors `LayoutState::open_panels` into live panel instances and
//! draws the dock.
//!
//! The whole "how is a panel drawn" path, collected once:
//!
//! * [`PanelHost::sync`] creates/drops instances so every open kind has one.
//! * [`PanelHost::draw`] is the **single drawing entry point** called once per UI
//!   pass from `EditorApp::build_ui`.
//! * [`PanelHost::draw_slot`] draws a slot's tab bar and its active panel body.
//! * [`PanelHost::draw_panel_body`] is the only place that calls [`Panel::draw`]
//!   (dynamic dispatch to the concrete panel implementation).
//!
//! Panels never get `&mut` state or host resources: `draw` receives the immutable
//! [`Frame`] and may only push `Action`s into [`UiOut`].

use std::collections::BTreeMap;

use editor_core_prototype::{Action, EditorState, LayoutAction, PanelId};
use egui::{Rect, Ui};
use ui_widgets_prototype::{EmptyState, TabBar, TabBarItem};

use super::{DockSlot, Panel, PanelInstanceId, PanelRegistry};
use crate::frame::{Frame, UiOut};

struct PanelInstance {
    id: PanelInstanceId,
    panel: Box<dyn Panel>,
    slot: DockSlot,
}

/// Per-slot egui panel sizing. The prototype keeps fixed sensible defaults; a
/// production host would read them from the persisted dock/layout state.
const LEFT_DEFAULT_WIDTH: f32 = 300.0;
const LEFT_MIN_WIDTH: f32 = 180.0;
const LEFT_MAX_WIDTH: f32 = 620.0;
const RIGHT_DEFAULT_WIDTH: f32 = 340.0;
const RIGHT_MIN_WIDTH: f32 = 200.0;
const RIGHT_MAX_WIDTH: f32 = 640.0;
const BOTTOM_DEFAULT_HEIGHT: f32 = 180.0;
const BOTTOM_MIN_HEIGHT: f32 = 80.0;
const BOTTOM_MAX_HEIGHT: f32 = 480.0;

fn slot_panel_id(slot: DockSlot) -> egui::Id {
    egui::Id::new(("kairos.panel-host.slot", slot.id_str()))
}

/// Docked host for the open panels.
///
/// The host mirrors LayoutState::open_panels into owned Panel instances:
/// PanelHost::sync creates instances for newly open kinds and drops instances
/// for kinds that were closed. PanelHost::show then draws, for each non-empty
/// slot, a tab bar plus the active panel body. Because every click is reported
/// as UiOut actions, the host stays a view: the reducer remains the only writer
/// of layout state.
///
/// Usage per frame (the app shell BuildUi phase):
///
/// ```ignore
/// host.sync(state);
/// let frame = Frame::with_host(state, self.host_resources());
/// let output = host.draw(ui, &frame, &mut ui_out);
/// if output.wants_continuous_repaint {
///     ui.ctx().request_repaint();
/// }
/// ```
pub struct PanelHost {
    registry: PanelRegistry,
    /// Open instances keyed by kind. A BTreeMap keeps iteration (and therefore
    /// tab order, active fallback and output rect order) deterministic.
    instances: BTreeMap<PanelId, PanelInstance>,
    /// Last active panel per slot. Updated in PanelHost::show from
    /// LayoutState::focus; kept so a slot whose focus moved elsewhere keeps
    /// showing the panel the user last had there (docking behaviour).
    active: BTreeMap<DockSlot, PanelId>,
    next_serial: u32,
}

impl PanelHost {
    /// Create a host; instances appear after the first PanelHost::sync.
    pub fn new(registry: PanelRegistry) -> Self {
        Self {
            registry,
            instances: BTreeMap::new(),
            active: BTreeMap::new(),
            next_serial: 0,
        }
    }

    /// Reconcile live instances with the layout open panels.
    ///
    /// Call once per frame before [`PanelHost::draw`]. Instances are created for
    /// registered kinds that became open and dropped for kinds that are no
    /// longer open. Open kinds without a registration are ignored (registration
    /// is how a feature module contributes a panel; the store knows nothing
    /// about implementations).
    pub fn sync(&mut self, state: &EditorState) {
        self.instances
            .retain(|kind, _instance| state.layout.is_open(*kind));

        // registry.kinds() is sorted, so serial assignment is deterministic even
        // though open_panels is a set.
        for kind in self.registry.kinds() {
            if !state.layout.is_open(kind) || self.instances.contains_key(&kind) {
                continue;
            }
            let Some(slot) = self.registry.slot(kind) else {
                continue;
            };
            let Some(panel) = self.registry.create(kind, state) else {
                continue;
            };
            let serial = self.next_serial;
            self.next_serial = self.next_serial.wrapping_add(1);
            self.instances.insert(
                kind,
                PanelInstance {
                    id: PanelInstanceId::new(kind, serial),
                    panel,
                    slot,
                },
            );
        }
    }

    /// Draw every non-empty slot and the central area.
    ///
    /// **This is the single drawing entry point for panels**: the app shell
    /// calls it once per UI pass and never calls a concrete panel itself.
    ///
    /// Tab clicks emit LayoutAction::FocusPanel, close clicks emit
    /// LayoutAction::ClosePanel; only the active panel body of each slot is
    /// drawn. Deterministic order: left, right, bottom, center.
    pub fn draw(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut) -> PanelHostOutput {
        let mut output = PanelHostOutput::default();
        self.resolve_active(frame.state.layout.focus);

        for slot in DockSlot::SIDES {
            if self.panel_ids(slot).is_empty() {
                continue;
            }
            match slot {
                DockSlot::Left => {
                    egui::Panel::left(slot_panel_id(slot))
                        .resizable(true)
                        .default_size(LEFT_DEFAULT_WIDTH)
                        .min_size(LEFT_MIN_WIDTH)
                        .max_size(LEFT_MAX_WIDTH)
                        .show(ui, |ui| self.draw_slot(ui, frame, out, slot, &mut output));
                }
                DockSlot::Right => {
                    egui::Panel::right(slot_panel_id(slot))
                        .resizable(true)
                        .default_size(RIGHT_DEFAULT_WIDTH)
                        .min_size(RIGHT_MIN_WIDTH)
                        .max_size(RIGHT_MAX_WIDTH)
                        .show(ui, |ui| self.draw_slot(ui, frame, out, slot, &mut output));
                }
                DockSlot::Bottom => {
                    egui::Panel::bottom(slot_panel_id(slot))
                        .resizable(true)
                        .default_size(BOTTOM_DEFAULT_HEIGHT)
                        .min_size(BOTTOM_MIN_HEIGHT)
                        .max_size(BOTTOM_MAX_HEIGHT)
                        .show(ui, |ui| self.draw_slot(ui, frame, out, slot, &mut output));
                }
                // SIDES cannot contain Center; kept total for exhaustiveness.
                DockSlot::Center => {}
            }
        }

        egui::CentralPanel::default_margins()
            .show(ui, |ui| self.draw_center_slot(ui, frame, out, &mut output));

        output.wants_continuous_repaint = self.wants_continuous_repaint();
        output
    }

    /// Aggregate repaint request of every open panel.
    ///
    /// The app shell calls ui.ctx().request_repaint() when this is true (or
    /// after a fixed interval, as the prototype does for its event log).
    pub fn wants_continuous_repaint(&self) -> bool {
        self.instances
            .values()
            .any(|instance| instance.panel.wants_continuous_repaint())
    }

    /// Whether a live instance exists for kind (created by the last sync).
    pub fn is_open(&self, kind: PanelId) -> bool {
        self.instances.contains_key(&kind)
    }

    /// Live instance ids in deterministic (PanelId) order.
    ///
    /// Introspection for the host and tests; drawing goes through show.
    pub fn instances(&self) -> impl Iterator<Item = PanelInstanceId> + '_ {
        self.instances.values().map(|instance| instance.id)
    }

    /// Number of live instances.
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    // ------------------------------------------------------------------
    // internals
    // ------------------------------------------------------------------

    /// Kind of every live instance docked in slot, sorted by PanelId.
    fn panel_ids(&self, slot: DockSlot) -> Vec<PanelId> {
        self.instances
            .iter()
            .filter(|(_kind, instance)| instance.slot == slot)
            .map(|(kind, _instance)| *kind)
            .collect()
    }

    /// Resolve one active panel per non-empty slot.
    ///
    /// The focused panel wins when it is docked in that slot; otherwise the slot
    /// keeps its last active panel, or falls back to the first (lowest PanelId,
    /// deterministic).
    fn resolve_active(&mut self, focus: PanelId) {
        for slot in DockSlot::ALL {
            let candidates = self.panel_ids(slot);
            if candidates.is_empty() {
                self.active.remove(&slot);
                continue;
            }
            let next = if candidates.contains(&focus) {
                focus
            } else if let Some(last) = self
                .active
                .get(&slot)
                .copied()
                .filter(|kind| candidates.contains(kind))
            {
                last
            } else if let Some(first) = candidates.first() {
                *first
            } else {
                continue;
            };
            self.active.insert(slot, next);
        }
    }

    /// Active panel of a slot as resolved by the last show.
    fn active_kind(&self, slot: DockSlot) -> Option<PanelId> {
        self.active.get(&slot).copied()
    }

    /// Draw the tab bar and active body of one slot.
    fn draw_slot(
        &mut self,
        ui: &mut Ui,
        frame: &Frame,
        out: &mut UiOut,
        slot: DockSlot,
        output: &mut PanelHostOutput,
    ) {
        let Some(active) = self.active_kind(slot) else {
            return;
        };
        let panel_ids = self.panel_ids(slot);
        let tabs: Vec<(PanelId, TabBarItem)> = panel_ids
            .iter()
            .filter_map(|kind| {
                let instance = self.instances.get(kind)?;
                Some((
                    *kind,
                    TabBarItem::new(instance.panel.title(frame))
                        .closable(instance.panel.closable()),
                ))
            })
            .collect();
        let Some(active_index) = tabs.iter().position(|(kind, _item)| *kind == active) else {
            return;
        };

        let mut tab_bar = TabBar::new(tabs.iter().map(|(_kind, item)| item.clone()).collect())
            .active(active_index);
        let bar_output = tab_bar.show(ui);

        for (index, rect) in &bar_output.tab_rects {
            if let Some((kind, _item)) = tabs.get(*index) {
                output.tab_rects.push((*kind, *rect));
            }
        }
        for (index, rect) in &bar_output.close_rects {
            if let Some((kind, _item)) = tabs.get(*index) {
                output.close_rects.push((*kind, *rect));
            }
        }
        if let Some(index) = bar_output.activated
            && let Some((kind, _item)) = tabs.get(index)
        {
            out.push(Action::Layout(LayoutAction::FocusPanel { panel: *kind }));
        }
        if let Some(index) = bar_output.close_clicked
            && let Some((kind, _item)) = tabs.get(index)
        {
            out.push(Action::Layout(LayoutAction::ClosePanel { panel: *kind }));
        }

        self.draw_panel_body(ui, frame, out, active, output);
    }

    /// Central slot: a normal docked slot, with a placeholder when the host did
    /// not register a panel for it (headless/widget tests).
    fn draw_center_slot(
        &mut self,
        ui: &mut Ui,
        frame: &Frame,
        out: &mut UiOut,
        output: &mut PanelHostOutput,
    ) {
        if self.panel_ids(DockSlot::Center).is_empty() {
            let mut empty = EmptyState::new(
                "No central panel",
                "The host can register a panel for DockSlot::Center (the app shell uses it for the 3D viewport).",
            );
            let _ = empty.show(ui);
            return;
        }
        self.draw_slot(ui, frame, out, DockSlot::Center, output);
    }

    /// Draw one panel body. The host owns the tab/title bar, so the panel only
    /// renders its content here.
    fn draw_panel_body(
        &mut self,
        ui: &mut Ui,
        frame: &Frame,
        out: &mut UiOut,
        kind: PanelId,
        output: &mut PanelHostOutput,
    ) {
        // Capture the area handed to the panel before it draws: overlay anchoring
        // (context menus, drop targets, viewport texture blits) needs it, and the
        // panel may draw less than the full area.
        let body_rect = ui.available_rect_before_wrap();
        if let Some(instance) = self.instances.get_mut(&kind) {
            instance.panel.draw(ui, frame, out);
        }
        output.active_panels.push(kind);
        output.panel_rects.push((kind, body_rect));
    }
}

/// Data returned by one PanelHost::show frame.
///
/// The rects exist for tests and for host-side overlays (context menus anchored
/// to a tab, drop targets, viewport texture space); production drawing never
/// needs to read them back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelHostOutput {
    /// Active panel per non-empty slot, in draw order: left, right, bottom, center.
    pub active_panels: Vec<PanelId>,
    /// Rect handed to each active panel body (below its tab bar).
    pub panel_rects: Vec<(PanelId, Rect)>,
    /// Tab hit rects, keyed by panel kind.
    pub tab_rects: Vec<(PanelId, Rect)>,
    /// Close-affordance hit rects, keyed by panel kind (closable panels only).
    pub close_rects: Vec<(PanelId, Rect)>,
    /// Whether any open panel wants a repaint every frame (see
    /// PanelHost::wants_continuous_repaint).
    pub wants_continuous_repaint: bool,
}
