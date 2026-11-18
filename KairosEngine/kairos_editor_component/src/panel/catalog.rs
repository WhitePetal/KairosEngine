//! Panel catalog: the set of panel kinds and their factories.
//!
//! Production fills it from the **link-section table**: every panel module
//! submits a [`PanelDef`] through the `panel!` macro, the linker concatenates
//! the entries, and [`PanelCatalog::builtin`] collects them once. The same
//! mechanism Bevy 0.17 uses for `Reflect` auto-registration; see
//! `docs/plan/editor-ui-architecture/panel-viewport-design.md`.
//!
//! The collection order is link order, which is not source order, so
//! `builtin` sorts by [`PanelId`]: instance serials and tests must not depend
//! on how the linker laid the object files out. Tests and embedders that need
//! a different set pass an explicit slice to [`PanelCatalog::new`].

use std::sync::OnceLock;

use kairos_editor_core::PanelId;

use super::{Panel, PanelContext};

/// Builds one panel instance.
///
/// A plain `fn` pointer, not a boxed closure: definitions are `static` values,
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

// Bare type: `panel!` submits `PanelDef` values from any panel module.
inventory::collect!(PanelDef);

/// The panel kinds a host knows, in declaration order.
///
/// [`PanelCatalog::builtin`] returns them sorted by id; a catalog built with
/// [`PanelCatalog::new`] keeps the slice order.
#[derive(Clone, Copy)]
pub struct PanelCatalog {
    defs: &'static [PanelDef],
}

impl PanelCatalog {
    pub const fn new(defs: &'static [PanelDef]) -> Self {
        Self { defs }
    }

    /// Every panel that submitted a [`PanelDef`] through `panel!`.
    ///
    /// Collected once per process and sorted by id, because inventory
    /// iteration follows link order, which is not the order panels are
    /// written in. Two submissions with the same id are a bug; debug builds
    /// assert it.
    pub fn builtin() -> Self {
        static COLLECTED: OnceLock<Vec<PanelDef>> = OnceLock::new();
        let defs = COLLECTED.get_or_init(|| {
            let mut defs: Vec<PanelDef> =
                inventory::iter::<PanelDef>.into_iter().copied().collect();
            defs.sort_by(|left, right| left.id.cmp(&right.id));
            debug_assert!(
                defs.windows(2).all(|pair| pair[0].id != pair[1].id),
                "two panels submitted the same PanelId"
            );
            defs
        });
        // A `OnceLock` in a static gives the vector a `'static` lifetime.
        Self::new(defs.as_slice())
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
