//! Panel framework: stable ids, the `Panel` trait, the registry and the
//! host that owns live panel instances.
//!
//! `PanelHost` is the prototype's host adapted to the production app: it owns
//! `Box<dyn Panel>` instances and implements the docking crate's `TabViewer`,
//! so the dock tree stores only [`PanelInstanceId`]s. There is intentionally no
//! headless core crate: panels read/write the concrete [`EditorState`] through
//! [`PanelContext`].

use std::collections::BTreeMap;

use egui::{Ui, WidgetText};
use kairos_editor_docking::viewer::{CloseResponse, TabViewer};

use crate::state::EditorState;

/// Stable kind of a panel (independent of how many instances are open).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanelId(pub &'static str);

impl std::fmt::Display for PanelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Stable identity of one open panel instance. This is the id stored in the
/// dock: kind identifies the implementation, serial distinguishes instances.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PanelInstanceId {
    pub kind: PanelId,
    pub serial: u32,
}

impl PanelInstanceId {
    pub const fn new(kind: PanelId, serial: u32) -> Self {
        Self { kind, serial }
    }
}

/// What a panel gets while drawing: the concrete app state, mutable.
///
/// No `Frame`/`UiOut` indirection: panels can read and update editor state
/// directly (that is the intended model for this editor). When a panel needs a
/// command with side effects it will grow into an explicit result instead of a
/// generic headless boundary.
pub struct PanelContext<'a> {
    pub state: &'a mut EditorState,
}

/// One drawable panel body. The host owns the tab/title bar.
pub trait Panel {
    fn kind(&self) -> PanelId;

    /// Title shown in the dock tab bar; may read editor state (document name,
    /// selection, ...), unlike a title cached at construction time.
    fn title(&self, state: &EditorState) -> WidgetText;

    fn ui(&mut self, ui: &mut Ui, ctx: &mut PanelContext);

    fn context_menu(&mut self, _ui: &mut Ui, _ctx: &mut PanelContext) {}

    fn on_rect_changed(&mut self) {}

    /// Whether the dock offers a close entry for this tab.
    fn closeable(&self) -> bool {
        true
    }

    /// Close callback; returning `false` vetoes the close.
    fn on_close(&mut self, _ctx: &mut PanelContext) -> bool {
        true
    }

    fn allowed_in_windows(&self) -> bool {
        true
    }

    fn scroll_bars(&self) -> [bool; 2] {
        [false, true]
    }
}

pub type PanelFactory = Box<dyn Fn() -> Box<dyn Panel>>;

/// Panel kind -> factory. The app registers once at composition time; opening
/// a panel is then a layout/state change, not a new match arm.
#[derive(Default)]
pub struct PanelRegistry {
    factories: BTreeMap<PanelId, PanelFactory>,
}

impl PanelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, kind: PanelId, factory: impl Fn() -> Box<dyn Panel> + 'static) {
        self.factories.insert(kind, Box::new(factory));
    }

    pub fn create(&self, kind: PanelId) -> Option<Box<dyn Panel>> {
        self.factories.get(&kind).map(|factory| factory())
    }

    pub fn kinds(&self) -> impl Iterator<Item = PanelId> + '_ {
        self.factories.keys().copied()
    }
}

struct PanelInstance {
    panel: Box<dyn Panel>,
}

/// Owns live panel instances and reconciles them with `LayoutState`.
///
/// This is the prototype's `PanelHost` adapted to the production app: it does
/// not draw the dock itself (the docking crate does), but it is the single
/// owner of panel objects and the `TabViewer` implementation that resolves
/// dock tab ids back to panels.
pub struct PanelHost {
    registry: PanelRegistry,
    instances: BTreeMap<PanelInstanceId, PanelInstance>,
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
    pub fn sync(&mut self, state: &EditorState) {
        self.instances.retain(|id, _| state.layout.is_open(id.kind));

        for kind in self.registry.kinds() {
            if !state.layout.is_open(kind) {
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
            self.instances.insert(id, PanelInstance { panel });
        }
    }

    pub fn kinds(&self) -> impl Iterator<Item = PanelId> + '_ {
        self.registry.kinds()
    }

    pub fn ids(&self) -> impl Iterator<Item = PanelInstanceId> + '_ {
        self.instances.keys().copied()
    }

    pub fn id_of(&self, kind: PanelId) -> Option<PanelInstanceId> {
        self.instances.keys().find(|id| id.kind == kind).copied()
    }

    pub fn viewer<'a>(&'a mut self, state: &'a mut EditorState) -> PanelHostViewer<'a> {
        PanelHostViewer { host: self, state }
    }
}

/// `TabViewer` adapter: the docking crate asks for a title/body/close policy,
/// the viewer resolves the id against the host's instances.
pub struct PanelHostViewer<'a> {
    host: &'a mut PanelHost,
    state: &'a mut EditorState,
}

impl TabViewer for PanelHostViewer<'_> {
    type Id = PanelInstanceId;

    fn title(&mut self, id: Self::Id) -> WidgetText {
        let PanelHostViewer { host, state } = self;
        host.instances
            .get(&id)
            .map(|instance| instance.panel.title(state))
            .unwrap_or_else(|| format!("{}#{}", id.kind, id.serial).into())
    }

    fn ui(&mut self, ui: &mut Ui, id: Self::Id) {
        let PanelHostViewer { host, state } = self;
        if let Some(instance) = host.instances.get_mut(&id) {
            let mut ctx = PanelContext {
                state: &mut **state,
            };
            instance.panel.ui(ui, &mut ctx);
        }
    }

    fn context_menu(&mut self, ui: &mut Ui, id: Self::Id) {
        let PanelHostViewer { host, state } = self;
        if let Some(instance) = host.instances.get_mut(&id) {
            let mut ctx = PanelContext {
                state: &mut **state,
            };
            instance.panel.context_menu(ui, &mut ctx);
        }
    }

    fn closeable(&mut self, id: Self::Id) -> bool {
        self.host
            .instances
            .get(&id)
            .is_some_and(|instance| instance.panel.closeable())
    }

    fn on_close(&mut self, id: Self::Id) -> CloseResponse {
        let PanelHostViewer { host, state } = self;
        let Some(instance) = host.instances.get_mut(&id) else {
            return CloseResponse::Ignore;
        };
        let mut ctx = PanelContext {
            state: &mut **state,
        };
        if !instance.panel.on_close(&mut ctx) {
            return CloseResponse::Ignore;
        }
        state.layout.close(id.kind);
        host.instances.remove(&id);
        CloseResponse::Close
    }

    fn allowed_in_windows(&mut self, id: Self::Id) -> bool {
        self.host
            .instances
            .get(&id)
            .is_some_and(|instance| instance.panel.allowed_in_windows())
    }

    fn on_rect_changed(&mut self, id: Self::Id) {
        if let Some(instance) = self.host.instances.get_mut(&id) {
            instance.panel.on_rect_changed();
        }
    }

    fn scroll_bars(&mut self, id: Self::Id) -> [bool; 2] {
        self.host
            .instances
            .get(&id)
            .map_or([false, true], |instance| instance.panel.scroll_bars())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakePanel;

    impl Panel for FakePanel {
        fn kind(&self) -> PanelId {
            PanelId("fake")
        }

        fn title(&self, _state: &EditorState) -> WidgetText {
            "Fake".into()
        }

        fn ui(&mut self, _ui: &mut Ui, _ctx: &mut PanelContext) {}
    }

    fn host_with_fake() -> PanelHost {
        let mut registry = PanelRegistry::new();
        registry.register(PanelId("fake"), || Box::new(FakePanel));
        PanelHost::new(registry)
    }

    #[test]
    fn sync_creates_and_drops_instances_from_layout() {
        let mut host = host_with_fake();
        let mut state = EditorState::new();

        host.sync(&state);
        assert_eq!(host.ids().count(), 0);

        state.layout.open(PanelId("fake"));
        host.sync(&state);
        assert_eq!(host.ids().count(), 1);

        state.layout.close(PanelId("fake"));
        host.sync(&state);
        assert_eq!(host.ids().count(), 0);
    }

    #[test]
    fn viewer_close_updates_layout_and_drops_instance() {
        let mut host = host_with_fake();
        let mut state = EditorState::new();
        state.layout.open(PanelId("fake"));
        host.sync(&state);
        let id = host.id_of(PanelId("fake")).expect("instance");

        {
            let mut viewer = host.viewer(&mut state);
            assert_eq!(viewer.on_close(id), CloseResponse::Close);
        }

        assert!(!state.layout.is_open(PanelId("fake")));
        assert_eq!(host.ids().count(), 0);
    }
}
