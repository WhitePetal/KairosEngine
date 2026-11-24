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

use kairos_editor_core::{EditorState, LayoutState, PanelId};

use crate::{
    frame::{Frame, HostView, UiOut},
    shared_style::SharedStyles,
};

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

    /// One page's metadata per live panel that draws its own skin, in instance
    /// order.
    ///
    /// No style value is copied, so the app can rebuild this every frame and
    /// compare it with the snapshot it holds; it takes a copy only when the list
    /// changed or an owner was reset.
    ///
    /// A panel that is not open has no instance and therefore no editable skin:
    /// it owns its style, and the style only exists once `Panel::init` loaded
    /// it. `state` is only there for `Panel::title`, which names the page: the
    /// list is built outside the UI pass, so there are no host resources to show
    /// and the frame carries an empty host.
    pub fn page_metas(
        &self,
        state: &EditorState,
        shared_styles: &SharedStyles,
    ) -> Vec<crate::skin::PageMeta> {
        struct NoHost;

        impl HostView for NoHost {}

        let frame = Frame::new(state, &NoHost, shared_styles);
        self.instances
            .values()
            .filter_map(|panel| panel.skin_meta(&frame))
            .collect()
    }

    /// One live panel's page with its value copied, or `None` when the panel is
    /// not open or has no editable skin.
    ///
    /// This and [`Self::page_metas`] are the only two reads the Preferences
    /// snapshot is built from: metadata every frame, a value only when the page
    /// list changed or the owner was replaced/reset.
    pub fn page(&self, panel: PanelId) -> Option<Box<dyn crate::style_row::StylePage>> {
        self.instances
            .values()
            .find(|instance| instance.kind() == panel)
            .and_then(|instance| instance.skin_value())
            .map(crate::style_row::StylePage::clone_box)
    }

    /// Every live panel's page with its value copied.
    ///
    /// Built on [`Self::page_metas`] and [`Self::page`], so the list the app
    /// compares and the one it snapshots cannot disagree. Copying every open
    /// skin is what makes this a rebuild-only call, not a per-frame one.
    pub fn pages(
        &self,
        state: &EditorState,
        shared_styles: &SharedStyles,
    ) -> Vec<crate::skin::PageSkin> {
        self.page_metas(state, shared_styles)
            .into_iter()
            .filter_map(|meta| {
                let crate::skin::SkinId::Panel(panel) = meta.id else {
                    return None;
                };
                Some(crate::skin::PageSkin {
                    id: meta.id,
                    title: meta.title,
                    file: meta.file,
                    page: self.page(panel)?,
                })
            })
            .collect()
    }

    /// Replace one live panel's skin with the page's whole typed copy.
    /// `false` when the panel is not open or the type does not match.
    pub fn replace_skin(&mut self, panel: PanelId, skin: Box<dyn std::any::Any>) -> bool {
        self.instances
            .values_mut()
            .find(|instance| instance.kind() == panel)
            .is_some_and(|instance| instance.replace_skin(skin))
    }

    /// Replace one live panel's skin with its code defaults. `false` when the
    /// panel is not open or has no editable skin.
    pub fn reset_skin(&mut self, panel: PanelId, ctx: &egui::Context) -> bool {
        self.instances
            .values_mut()
            .find(|instance| instance.kind() == panel)
            .is_some_and(|instance| instance.reset_skin(ctx))
    }

    /// Persist one live panel's skin to its RON file.
    pub fn save_skin(&self, panel: PanelId, ctx: &egui::Context) -> Result<(), String> {
        let Some(instance) = self
            .instances
            .values()
            .find(|instance| instance.kind() == panel)
        else {
            return Err(format!("{panel} is not open"));
        };
        instance.save_skin(ctx)
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
#[cfg(test)]
mod tests {
    use super::*;

    use egui::{Ui, WidgetText};
    use kairos_editor_core::EditorState;

    use crate::frame::{Frame, UiOut};
    use crate::{EditorInfo, PanelDef, PanelId, PanelRegister};

    const FAKE: PanelId = PanelId("fake");

    fn test_info() -> EditorInfo {
        EditorInfo::new("KairosEngine", "9.9.9-test")
    }

    struct FakePanel {
        veto_close: bool,
    }

    // Sealed: outside this crate only `#[panel]` implements the seal. A unit
    // test is inside the crate, so it can stand in for a panel without
    // submitting a definition to the global link-section table.
    impl crate::__sealed::Sealed for FakePanel {}

    impl PanelRegister for FakePanel {
        fn panel_id(&self) -> PanelId {
            FAKE
        }
    }

    impl Panel for FakePanel {
        fn title(&self, _frame: &Frame) -> WidgetText {
            "Fake".into()
        }

        fn ui(&mut self, _ui: &mut Ui, _frame: &Frame, _out: &mut UiOut) {}

        fn on_close(&mut self, _frame: &Frame, _out: &mut UiOut) -> bool {
            !self.veto_close
        }
    }

    fn fake_allow(_context: &PanelContext) -> Box<dyn Panel> {
        Box::new(FakePanel { veto_close: false })
    }

    fn fake_veto(_context: &PanelContext) -> Box<dyn Panel> {
        Box::new(FakePanel { veto_close: true })
    }

    static FAKE_ALLOW: [PanelDef; 1] = [PanelDef::new(FAKE, fake_allow)];
    static FAKE_VETO: [PanelDef; 1] = [PanelDef::new(FAKE, fake_veto)];

    fn host_with_fake(veto_close: bool) -> PanelHost {
        let defs = if veto_close { &FAKE_VETO } else { &FAKE_ALLOW };
        PanelHost::new(PanelCatalog::new(defs), PanelContext::new(test_info()))
    }

    #[test]
    fn sync_creates_and_drops_instances_from_layout() {
        let mut host = host_with_fake(false);
        let mut state = EditorState::new();

        host.sync(&state.layout, &egui::Context::default());
        assert_eq!(host.ids().count(), 0);

        state.layout.open(FAKE);
        host.sync(&state.layout, &egui::Context::default());
        assert_eq!(host.ids().count(), 1);

        state.layout.close(FAKE);
        host.sync(&state.layout, &egui::Context::default());
        assert_eq!(host.ids().count(), 0);
    }

    /// The catalog is compile-time data, not a startup pass: a panel object
    /// exists only while its kind is open, and later frames reuse it.
    #[test]
    fn a_panel_is_constructed_only_while_it_is_open() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static BUILT: AtomicUsize = AtomicUsize::new(0);

        fn counting(_context: &PanelContext) -> Box<dyn Panel> {
            BUILT.fetch_add(1, Ordering::SeqCst);
            Box::new(FakePanel { veto_close: false })
        }

        static COUNTING: [PanelDef; 1] = [PanelDef::new(FAKE, counting)];

        let mut host = PanelHost::new(PanelCatalog::new(&COUNTING), PanelContext::new(test_info()));
        let mut state = EditorState::new();
        let ctx = egui::Context::default();

        host.sync(&state.layout, &ctx);
        assert_eq!(
            BUILT.load(Ordering::SeqCst),
            0,
            "a closed panel is never built"
        );

        state.layout.open(FAKE);
        host.sync(&state.layout, &ctx);
        host.sync(&state.layout, &ctx);
        assert_eq!(
            BUILT.load(Ordering::SeqCst),
            1,
            "opening builds one instance; later syncs reuse it"
        );
    }
}
