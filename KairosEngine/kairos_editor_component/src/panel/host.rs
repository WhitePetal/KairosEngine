//! Owns the live panel instances and reconciles them with `LayoutState`.
//!
//! The host is the single owner of `Box<dyn Panel>` objects. It does not draw:
//! the dock does that through [`PanelHostViewer`]. Every frame the app calls
//! [`PanelHost::sync`] and then hands `host.viewer(&mut out, frame)` to the dock.
//!
//! Instances exist only while their kind is open: the catalog is collected once
//! from the link-section table, so a panel nobody opens is never built and its
//! [`Panel::init`] hook never runs.
//!
//! Closing a panel is not this type's decision: the reducer closes the layout
//! (through an action), and the next `sync` drops the instance.

use std::collections::BTreeMap;

use kairos_editor_core::{LayoutState, PanelId};

use crate::frame::{Frame, UiOut};

use super::{Panel, PanelCatalog, PanelContext, PanelHostViewer, PanelInstanceId};

/// Owns live panel instances and reconciles them with the open-panel intent.
pub struct PanelHost {
    catalog: PanelCatalog,
    /// Injected into every factory: the composition root's construction-time
    /// input for panels (`EditorInfo` today).
    context: PanelContext,
    instances: BTreeMap<PanelInstanceId, Box<dyn Panel>>,
    next_serial: u32,
}

impl PanelHost {
    pub fn new(catalog: PanelCatalog, context: PanelContext) -> Self {
        Self {
            catalog,
            context,
            instances: BTreeMap::new(),
            next_serial: 0,
        }
    }

    /// Create instances for newly opened panels and drop instances for closed
    /// ones. Called once per frame before drawing.
    ///
    /// This is the only place a panel is constructed: the loop visits the
    /// declared kinds and builds only the ones the layout has open, so a closed
    /// panel costs one set lookup and nothing else. New instances get their
    /// [`Panel::init`] hook here, with the egui context they will draw into, so
    /// a panel can load its own UI resources on open.
    pub fn sync(&mut self, layout: &LayoutState, ctx: &egui::Context) {
        self.instances.retain(|id, _| layout.is_open(id.kind));

        // A copy, so the loop does not hold a borrow of `self` while instances
        // are inserted.
        let catalog = self.catalog;
        for kind in catalog.kinds() {
            if !layout.is_open(kind) {
                continue;
            }
            if self.instances.keys().any(|id| id.kind == kind) {
                continue;
            }
            let Some(mut panel) = catalog.create(kind, &self.context) else {
                continue;
            };
            debug_assert_eq!(
                panel.kind(),
                kind,
                "a panel definition builds a panel of another kind"
            );
            panel.init(ctx);
            let id = PanelInstanceId::new(kind, self.next_serial);
            self.next_serial = self.next_serial.wrapping_add(1);
            self.instances.insert(id, panel);
        }
    }

    /// Declared kinds, for toolbars/menus that toggle panels by kind.
    pub fn kinds(&self) -> impl Iterator<Item = PanelId> + '_ {
        self.catalog.kinds()
    }

    /// Live instance ids, for reconciling the dock tree with the host.
    pub fn ids(&self) -> impl Iterator<Item = PanelInstanceId> + '_ {
        self.instances.keys().copied()
    }

    /// The first live instance of `kind`, if that panel is open.
    pub fn id_of(&self, kind: PanelId) -> Option<PanelInstanceId> {
        self.instances.keys().copied().find(|id| id.kind == kind)
    }

    /// Whether the dock may detach this panel into a floating window.
    pub fn allowed_in_windows(&self, id: PanelInstanceId) -> bool {
        self.panel(id)
            .is_some_and(|panel| panel.allowed_in_windows())
    }

    /// The dock's view of the panels: titles, bodies and close policy.
    pub fn viewer<'host, 'frame>(
        &'host mut self,
        out: &'host mut UiOut,
        frame: Frame<'frame>,
    ) -> PanelHostViewer<'host, 'frame> {
        PanelHostViewer::new(self, out, frame)
    }

    pub(crate) fn panel(&self, id: PanelInstanceId) -> Option<&dyn Panel> {
        self.instances.get(&id).map(|panel| &**panel)
    }

    pub(crate) fn panel_mut(&mut self, id: PanelInstanceId) -> Option<&mut (dyn Panel + 'static)> {
        self.instances.get_mut(&id).map(|panel| &mut **panel)
    }
}
