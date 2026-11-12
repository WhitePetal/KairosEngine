//! Panel kind -> factory registry.
//!
//! The app (or a feature module) registers its factories once at composition
//! time; the host calls them when a kind becomes open. The registry never
//! creates instances during a frame and never holds live panels.

use std::collections::BTreeMap;

use kairos_editor_core::PanelId;

use super::Panel;

/// Factory invoked by the host when a panel kind becomes open.
pub type PanelFactory = Box<dyn Fn() -> Box<dyn Panel>>;

/// Panel kind -> factory. The app registers once at composition time; opening
/// a panel is then a layout/state change, not a new match arm.
///
/// Kept in a `BTreeMap` so `kinds()` is deterministic (sorted by `PanelId`)
/// regardless of registration order.
#[derive(Default)]
pub struct PanelRegistry {
    factories: BTreeMap<PanelId, PanelFactory>,
}

impl PanelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or replace) the factory for a panel kind.
    ///
    /// Re-registering a kind takes effect the next time an instance is created;
    /// already-open instances keep running the old implementation until closed.
    pub fn register(&mut self, kind: PanelId, factory: impl Fn() -> Box<dyn Panel> + 'static) {
        self.factories.insert(kind, Box::new(factory));
    }

    /// Create one panel instance for `kind`, or `None` when unregistered.
    pub fn create(&self, kind: PanelId) -> Option<Box<dyn Panel>> {
        self.factories.get(&kind).map(|factory| factory())
    }

    /// Registered kinds in deterministic (sorted) order.
    pub fn kinds(&self) -> impl Iterator<Item = PanelId> + '_ {
        self.factories.keys().copied()
    }

    /// Number of registered kinds.
    pub fn len(&self) -> usize {
        self.factories.len()
    }

    /// Whether no kind is registered.
    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }
}
