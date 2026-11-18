//! Panel catalog: the compile-time table of panel kinds and their factories.
//!
//! The set of panels is fixed when the binary is linked, so it is data, not
//! startup work: a [`PanelCatalog`] holds a `&'static [PanelDef]` and the host
//! consults it when a kind becomes open. There is no registration call, no map
//! built at boot and no closure to capture the environment: a definition is a
//! `const`, and everything a factory needs arrives as a [`PanelContext`].
//!
//! Binding the id to the factory in one value also removes the "registered
//! under the wrong kind" failure mode: the host keys instances by
//! [`PanelDef::id`], and `Panel::kind` is only a consistency check.

use kairos_editor_core::PanelId;

use super::{Panel, PanelContext};

/// Builds one panel instance.
///
/// A plain `fn` pointer, not a boxed closure: definitions are `const` values,
/// so nothing may be captured. State a factory needs comes from its
/// [`PanelContext`] argument.
pub type PanelFactory = fn(&PanelContext) -> Box<dyn Panel>;

/// One panel kind: its stable id and the only factory that builds it.
#[derive(Clone, Copy)]
pub struct PanelDef {
    /// Stable id: the key in `LayoutState` and in persisted layouts.
    pub id: PanelId,
    /// The only constructor for this panel.
    pub factory: PanelFactory,
}

impl PanelDef {
    pub const fn new(id: PanelId, factory: PanelFactory) -> Self {
        Self { id, factory }
    }
}

/// The panel kinds a host knows, in declaration order.
///
/// Declaration order is part of the contract: `PanelHost::sync` walks it when
/// it allocates instance serials, so the same open set yields the same ids on
/// every run.
#[derive(Clone, Copy)]
pub struct PanelCatalog {
    defs: &'static [PanelDef],
}

impl PanelCatalog {
    pub const fn new(defs: &'static [PanelDef]) -> Self {
        Self { defs }
    }

    /// The definition for `id`, or `None` for a kind this host does not ship
    /// (a stale layout entry, a command naming a panel from another build).
    pub fn get(&self, id: PanelId) -> Option<&'static PanelDef> {
        self.defs.iter().find(|def| def.id == id)
    }

    /// Build one instance of `id`, or `None` when the kind is unknown.
    ///
    /// The only caller in production is `PanelHost::sync`, when the panel
    /// kind becomes open; nothing here runs at startup.
    pub fn create(&self, id: PanelId, context: &PanelContext) -> Option<Box<dyn Panel>> {
        self.get(id).map(|def| (def.factory)(context))
    }

    /// Declared kinds in declaration order.
    pub fn kinds(&self) -> impl Iterator<Item = PanelId> + '_ {
        self.defs.iter().map(|def| def.id)
    }

    /// Number of declared kinds.
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    /// Whether no kind is declared.
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}
