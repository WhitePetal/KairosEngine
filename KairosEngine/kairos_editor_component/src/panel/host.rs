//! Owns the live panel instances and reconciles them with `LayoutState`.
//!
//! The host is the single owner of `Box<dyn Panel>` objects. It does not draw:
//! the dock does that through [`PanelHostViewer`]. Every frame the app calls
//! [`PanelHost::sync`] and then hands `host.viewer(&mut out, frame)` to the dock.
//!
//! Closing a panel is not this type's decision: the reducer closes the layout
//! (through an action), and the next `sync` drops the instance.

use std::collections::BTreeMap;

use kairos_editor_core::{LayoutState, PanelId};

use crate::frame::{Frame, UiOut};

use super::{Panel, PanelHostViewer, PanelInstanceId, PanelRegistry};

/// Owns live panel instances and reconciles them with the open-panel intent.
pub struct PanelHost {
    registry: PanelRegistry,
    instances: BTreeMap<PanelInstanceId, Box<dyn Panel>>,
    next_serial: u32,
}

impl PanelHost {
    pub fn new(registry: PanelRegistry) -> Self {
        Self {
            registry,
            instances: BTreeMap::new(),
            next_serial: 0,
        }
    }

    /// Create instances for newly opened panels and drop instances for closed
    /// ones. Called once per frame before drawing.
    pub fn sync(&mut self, layout: &LayoutState) {
        self.instances.retain(|id, _| layout.is_open(id.kind));

        for kind in self.registry.kinds() {
            if !layout.is_open(kind) {
                continue;
            }
            if self.instances.keys().any(|id| id.kind == kind) {
                continue;
            }
            let Some(panel) = self.registry.create(kind) else {
                continue;
            };
            debug_assert_eq!(
                panel.kind(),
                kind,
                "factory registered under the wrong kind"
            );
            let id = PanelInstanceId::new(kind, self.next_serial);
            self.next_serial = self.next_serial.wrapping_add(1);
            self.instances.insert(id, panel);
        }
    }

    /// Registered kinds, for toolbars/menus that toggle panels by kind.
    pub fn kinds(&self) -> impl Iterator<Item = PanelId> + '_ {
        self.registry.kinds()
    }

    /// Live instance ids, for reconciling the dock tree with the host.
    pub fn ids(&self) -> impl Iterator<Item = PanelInstanceId> + '_ {
        self.instances.keys().copied()
    }

    /// The first live instance of `kind`, if that panel is open.
    pub fn id_of(&self, kind: PanelId) -> Option<PanelInstanceId> {
        self.instances.keys().copied().find(|id| id.kind == kind)
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
