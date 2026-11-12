//! Panel registry: maps a stable `PanelId` kind to its dock slot and factory.
//!
//! Feature modules (or the app composition root) register their factories once at
//! startup; the host calls them when a kind becomes open. The registry never
//! creates instances during a frame and never reads domain state beyond the
//! factory argument.

use std::collections::BTreeMap;

use editor_core_prototype::{EditorState, PanelId};

use super::{DockSlot, Panel};
use crate::console_panel::ConsolePanel;
use crate::inspector_panel::InspectorPanel;
use crate::project_panel::ProjectPanel;

/// Factory invoked by the host when a panel kind becomes open.
pub type PanelFactory = Box<dyn Fn(&EditorState) -> Box<dyn Panel>>;

struct PanelRegistration {
    slot: DockSlot,
    factory: PanelFactory,
}

/// Maps PanelId kinds to their dock slot and factory.
///
/// The registry is the composition root for panels: feature modules (or the app
/// shell) register their factories once at startup, then the host creates and
/// drops instances as the layout reducer opens/closes panels. Kept in a
/// BTreeMap so kinds() is deterministic (sorted by PanelId) regardless of
/// registration order.
#[derive(Default)]
pub struct PanelRegistry {
    registrations: BTreeMap<PanelId, PanelRegistration>,
}

impl PanelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registry with the prototype built-in panels: project (left), inspector
    /// (right), console (bottom). The central slot is left to the host, which
    /// registers its viewport panel there (it needs host textures).
    pub fn editor_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(PanelId::PROJECT, DockSlot::Left, |state| {
            Box::new(ProjectPanel::expanded(&state.project))
        });
        registry.register(PanelId::INSPECTOR, DockSlot::Right, |_state| {
            Box::new(InspectorPanel::new())
        });
        registry.register(PanelId::CONSOLE, DockSlot::Bottom, |_state| {
            Box::new(ConsolePanel::new())
        });
        registry
    }

    /// Register (or replace) the factory for a panel kind.
    ///
    /// Re-registering a kind takes effect the next time an instance is created;
    /// already-open instances keep running the old implementation until closed.
    pub fn register(
        &mut self,
        kind: PanelId,
        slot: DockSlot,
        factory: impl Fn(&EditorState) -> Box<dyn Panel> + 'static,
    ) -> &mut Self {
        self.registrations.insert(
            kind,
            PanelRegistration {
                slot,
                factory: Box::new(factory),
            },
        );
        self
    }

    /// Create one panel instance for kind, or None when unregistered.
    pub fn create(&self, kind: PanelId, state: &EditorState) -> Option<Box<dyn Panel>> {
        let registration = self.registrations.get(&kind)?;
        let panel = (registration.factory)(state);
        debug_assert_eq!(
            panel.kind(),
            kind,
            "panel factory registered for {kind:?} must return a panel of that kind"
        );
        Some(panel)
    }

    /// Dock slot of a registered kind, if any.
    pub fn slot(&self, kind: PanelId) -> Option<DockSlot> {
        self.registrations
            .get(&kind)
            .map(|registration| registration.slot)
    }

    /// Registered kinds in deterministic (sorted) order.
    pub fn kinds(&self) -> impl Iterator<Item = PanelId> + '_ {
        self.registrations.keys().copied()
    }

    /// Number of registered kinds.
    pub fn len(&self) -> usize {
        self.registrations.len()
    }

    /// Whether no kind is registered.
    pub fn is_empty(&self) -> bool {
        self.registrations.is_empty()
    }
}
