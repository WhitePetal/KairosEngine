//! Headless tests for the panel framework: registry/sync lifecycle, tab and
//! close clicks mapped to core LayoutActions, focus-driven active tabs,
//! deterministic instance identity, panel-body action plumbing and repaint
//! aggregation.
//!
//! Everything runs through the real reducer vocabulary (editor-core Actions) but
//! uses fake panels for the behaviour assertions, so the test suite never needs
//! the app shell or GPU host. The final test renders the real built-in panels
//! through PanelHost as a smoke test of the production wiring.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use editor_components_prototype::{
    ConsolePanel, DockSlot, Frame, HostView, Panel, PanelHost, PanelHostOutput, PanelRegistry,
    UiOut,
};
use editor_core_prototype::{Action, EditorState, LayoutAction, PanelId, ViewportId};
use egui::{TextureId, Ui};
use ui_widgets_prototype::Theme;
use ui_widgets_prototype::testing::Harness;

const PANEL_A: PanelId = PanelId("a-panel");
const PANEL_B: PanelId = PanelId("b-panel");
const TEST_PANEL: PanelId = PanelId("test-panel");
const GHOST_PANEL: PanelId = PanelId("ghost-panel");

// ---------------------------------------------------------------------------
// test doubles
// ---------------------------------------------------------------------------

/// Minimal panel: records how often its body was drawn and can push a canned
/// Action, which is exactly the Panel contract (read Frame, write UiOut only).
struct FakePanel {
    kind: PanelId,
    title: &'static str,
    draws: Rc<Cell<usize>>,
    action: Option<Action>,
    closable: bool,
    repaint: bool,
}

impl FakePanel {
    fn new(kind: PanelId, title: &'static str, draws: Rc<Cell<usize>>) -> Self {
        Self {
            kind,
            title,
            draws,
            action: None,
            closable: true,
            repaint: false,
        }
    }

    fn with_action(mut self, action: Action) -> Self {
        self.action = Some(action);
        self
    }

    fn with_closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    fn with_repaint(mut self) -> Self {
        self.repaint = true;
        self
    }
}

impl Panel for FakePanel {
    fn kind(&self) -> PanelId {
        self.kind
    }

    fn title(&self, _frame: &Frame) -> String {
        self.title.to_owned()
    }

    fn draw(&mut self, _ui: &mut Ui, _frame: &Frame, out: &mut UiOut) {
        self.draws.set(self.draws.get() + 1);
        if let Some(action) = &self.action {
            out.push(action.clone());
        }
    }

    fn closable(&self) -> bool {
        self.closable
    }

    fn wants_continuous_repaint(&self) -> bool {
        self.repaint
    }
}

/// Host resources double for Frame::with_host and the console panel.
struct TestHost {
    lines: Vec<String>,
    texture: Option<TextureId>,
}

impl HostView for TestHost {
    fn viewport_texture(&self, _viewport: ViewportId) -> Option<TextureId> {
        self.texture
    }

    fn console_lines(&self) -> &[String] {
        &self.lines
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn themed_harness() -> Harness {
    let harness = Harness::default();
    Theme::dark().install(&harness.ctx);
    harness
}

/// State whose layout opens exactly the given panels (the default state opens
/// four, so it is cleared first).
fn state_with(open: &[PanelId], focus: PanelId) -> EditorState {
    let mut state = EditorState::new();
    state.layout.open_panels.clear();
    for panel in open {
        state.layout.open(*panel);
    }
    state.layout.focus = focus;
    state
}

fn register_fake(
    registry: &mut PanelRegistry,
    kind: PanelId,
    title: &'static str,
    slot: DockSlot,
    draws: Rc<Cell<usize>>,
    configure: impl Fn(FakePanel) -> FakePanel + 'static,
) {
    registry.register(kind, slot, move |_state| {
        Box::new(configure(FakePanel::new(kind, title, draws.clone())))
    });
}

/// Run one UI pass through PanelHost and return its output plus the emitted
/// actions. Output slots are overwritten (not accumulated) so egui multi-pass
/// frames do not duplicate actions.
fn show_once(
    harness: &mut Harness,
    host: &mut PanelHost,
    state: &EditorState,
) -> (PanelHostOutput, Vec<Action>) {
    let output = Rc::new(RefCell::new(PanelHostOutput::default()));
    let actions = Rc::new(RefCell::new(Vec::<Action>::new()));
    {
        let output_cell = output.clone();
        let actions_cell = actions.clone();
        let mut draw = |ui: &mut Ui| {
            let frame = Frame::new(state);
            let mut out = UiOut::new();
            *output_cell.borrow_mut() = host.draw(ui, &frame, &mut out);
            *actions_cell.borrow_mut() = out.take();
        };
        harness.frame(Vec::new(), &mut draw);
    }
    let output = output.borrow().clone();
    let actions = actions.borrow().clone();
    (output, actions)
}

fn rect_for(output: &PanelHostOutput, kind: PanelId) -> egui::Rect {
    output
        .tab_rects
        .iter()
        .find(|(candidate, _rect)| *candidate == kind)
        .map(|(_kind, rect)| *rect)
        .unwrap_or_else(|| panic!("no tab rect for {kind:?}"))
}

fn close_rect_for(output: &PanelHostOutput, kind: PanelId) -> egui::Rect {
    output
        .close_rects
        .iter()
        .find(|(candidate, _rect)| *candidate == kind)
        .map(|(_kind, rect)| *rect)
        .unwrap_or_else(|| panic!("no close rect for {kind:?}"))
}

// ---------------------------------------------------------------------------
// registry + sync lifecycle
// ---------------------------------------------------------------------------

#[test]
fn sync_creates_one_instance_per_open_registered_kind_in_registry_order() {
    let draws = Rc::new(Cell::new(0));
    let mut registry = PanelRegistry::new();
    // Register in reverse PanelId order; instance creation must still be sorted.
    register_fake(
        &mut registry,
        PanelId::INSPECTOR,
        "Inspector",
        DockSlot::Right,
        draws.clone(),
        |p| p,
    );
    register_fake(
        &mut registry,
        PanelId::PROJECT,
        "Project",
        DockSlot::Left,
        draws.clone(),
        |p| p,
    );
    assert_eq!(
        registry.kinds().collect::<Vec<_>>(),
        vec![PanelId::INSPECTOR, PanelId::PROJECT],
        "kinds() must be deterministic (sorted by PanelId)"
    );
    assert_eq!(registry.slot(PanelId::PROJECT), Some(DockSlot::Left));
    assert_eq!(registry.slot(PanelId::CONSOLE), None);

    let state = state_with(&[PanelId::PROJECT, PanelId::INSPECTOR], PanelId::PROJECT);
    let mut host = PanelHost::new(registry);
    host.sync(&state);

    assert_eq!(host.instance_count(), 2);
    assert!(host.is_open(PanelId::PROJECT));
    assert!(host.is_open(PanelId::INSPECTOR));
    assert!(
        !host.is_open(PanelId::CONSOLE),
        "closed kinds have no instance"
    );
    let instances: Vec<_> = host.instances().collect();
    assert_eq!(
        instances
            .iter()
            .map(|instance| instance.kind)
            .collect::<Vec<_>>(),
        vec![PanelId::INSPECTOR, PanelId::PROJECT],
        "instance iteration is deterministic (PanelId order)"
    );
    let serials: Vec<u32> = instances.iter().map(|instance| instance.serial).collect();
    assert_eq!(
        serials,
        vec![0, 1],
        "serials are allocated in creation order"
    );
}

#[test]
fn sync_ignores_open_kinds_without_a_registration() {
    let mut registry = PanelRegistry::new();
    register_fake(
        &mut registry,
        TEST_PANEL,
        "Test",
        DockSlot::Left,
        Rc::new(Cell::new(0)),
        |p| p,
    );

    let state = state_with(&[TEST_PANEL, GHOST_PANEL], TEST_PANEL);
    let mut host = PanelHost::new(registry);
    host.sync(&state);

    assert_eq!(host.instance_count(), 1);
    assert!(host.is_open(TEST_PANEL));
    assert!(
        !host.is_open(GHOST_PANEL),
        "unregistered open kinds are ignored"
    );

    let mut harness = themed_harness();
    let (output, actions) = show_once(&mut harness, &mut host, &state);
    assert_eq!(output.active_panels, vec![TEST_PANEL]);
    assert!(actions.is_empty());
}

#[test]
fn sync_drops_instances_of_closed_panels_and_reopen_gets_a_new_serial() {
    let mut registry = PanelRegistry::new();
    register_fake(
        &mut registry,
        PANEL_A,
        "Alpha",
        DockSlot::Left,
        Rc::new(Cell::new(0)),
        |p| p,
    );
    let mut state = state_with(&[PANEL_A], PANEL_A);
    let mut host = PanelHost::new(registry);
    host.sync(&state);
    let first = host.instances().next().expect("instance created");
    assert_eq!(first.serial, 0);

    state.layout.close(PANEL_A);
    host.sync(&state);
    assert_eq!(
        host.instance_count(),
        0,
        "closing the panel drops its instance"
    );

    state.layout.open(PANEL_A);
    host.sync(&state);
    let reopened = host.instances().next().expect("instance recreated");
    assert_eq!(reopened.kind, PANEL_A);
    assert_eq!(
        reopened.serial, 1,
        "a reopened panel is a new instance identity"
    );
}

// ---------------------------------------------------------------------------
// show: custom panels, active tabs, tab bar
// ---------------------------------------------------------------------------

#[test]
fn brand_new_panel_kind_can_be_registered_and_shown_without_touching_the_store() {
    let draws = Rc::new(Cell::new(0));
    let mut registry = PanelRegistry::new();
    register_fake(
        &mut registry,
        TEST_PANEL,
        "Test panel",
        DockSlot::Right,
        draws.clone(),
        |p| p,
    );

    let state = state_with(&[TEST_PANEL], TEST_PANEL);
    let mut host = PanelHost::new(registry);
    host.sync(&state);

    let mut harness = themed_harness();
    let (output, actions) = show_once(&mut harness, &mut host, &state);

    assert_eq!(output.active_panels, vec![TEST_PANEL]);
    assert!(
        rect_for(&output, TEST_PANEL).width() > 0.0,
        "tab is laid out"
    );
    assert!(
        output
            .panel_rects
            .iter()
            .any(|(kind, rect)| *kind == TEST_PANEL && rect.is_positive()),
        "panel body rect is reported"
    );
    assert!(actions.is_empty());
    assert!(draws.get() >= 1, "active panel body must be drawn");
}

#[test]
fn host_lays_out_tab_and_close_rects_for_the_active_slot() {
    // FakePanel::title ignores Frame, but the host must call it every frame so a
    // real panel can derive a dynamic title (e.g. active document name).
    let draws = Rc::new(Cell::new(0));
    let mut registry = PanelRegistry::new();
    register_fake(
        &mut registry,
        PANEL_A,
        "Alpha",
        DockSlot::Left,
        draws,
        |p| p,
    );

    let state = state_with(&[PANEL_A], PANEL_A);
    let mut host = PanelHost::new(registry);
    host.sync(&state);
    let mut harness = themed_harness();
    let (output, _actions) = show_once(&mut harness, &mut host, &state);
    assert_eq!(output.tab_rects.len(), 1);
    assert_eq!(
        output.close_rects.len(),
        1,
        "closable panels get a close affordance"
    );
}

#[test]
fn active_panel_follows_layout_focus_and_only_it_is_drawn() {
    let draws_a = Rc::new(Cell::new(0));
    let draws_b = Rc::new(Cell::new(0));
    let mut registry = PanelRegistry::new();
    register_fake(
        &mut registry,
        PANEL_A,
        "Alpha",
        DockSlot::Left,
        draws_a.clone(),
        |p| p,
    );
    register_fake(
        &mut registry,
        PANEL_B,
        "Beta",
        DockSlot::Left,
        draws_b.clone(),
        |p| p,
    );

    let mut state = state_with(&[PANEL_A, PANEL_B], PANEL_B);
    let mut host = PanelHost::new(registry);
    host.sync(&state);
    let mut harness = themed_harness();

    let (output, _actions) = show_once(&mut harness, &mut host, &state);
    assert_eq!(
        output.active_panels,
        vec![PANEL_B],
        "focused panel wins its slot"
    );
    assert_eq!(draws_a.get(), 0, "inactive panel body must not be drawn");
    assert!(draws_b.get() >= 1, "active panel body is drawn");

    // Moving focus within the same slot switches the active tab.
    let draws_b_before = draws_b.get();
    state.layout.focus = PANEL_A;
    let (output, _actions) = show_once(&mut harness, &mut host, &state);
    assert_eq!(output.active_panels, vec![PANEL_A]);
    assert!(draws_a.get() >= 1);
    assert_eq!(
        draws_b.get(),
        draws_b_before,
        "the previously active panel stops drawing once focus moved"
    );
}

#[test]
fn non_closable_panel_has_no_close_affordance() {
    let mut registry = PanelRegistry::new();
    register_fake(
        &mut registry,
        PANEL_A,
        "Fixed",
        DockSlot::Left,
        Rc::new(Cell::new(0)),
        |p| p.with_closable(false),
    );
    let state = state_with(&[PANEL_A], PANEL_A);
    let mut host = PanelHost::new(registry);
    host.sync(&state);
    let mut harness = themed_harness();
    let (output, _actions) = show_once(&mut harness, &mut host, &state);
    assert_eq!(output.tab_rects.len(), 1);
    assert!(
        output.close_rects.is_empty(),
        "closable() == false hides the close button"
    );
}

// ---------------------------------------------------------------------------
// interaction -> Actions
// ---------------------------------------------------------------------------

#[test]
fn tab_click_emits_focus_panel_action() {
    let mut registry = PanelRegistry::new();
    register_fake(
        &mut registry,
        PANEL_A,
        "Alpha",
        DockSlot::Left,
        Rc::new(Cell::new(0)),
        |p| p,
    );
    register_fake(
        &mut registry,
        PANEL_B,
        "Beta",
        DockSlot::Left,
        Rc::new(Cell::new(0)),
        |p| p,
    );
    let state = state_with(&[PANEL_A, PANEL_B], PANEL_A);
    let mut host = PanelHost::new(registry);
    host.sync(&state);
    let mut harness = themed_harness();

    let output = Rc::new(RefCell::new(PanelHostOutput::default()));
    let actions = Rc::new(RefCell::new(Vec::<Action>::new()));
    {
        let output_cell = output.clone();
        let actions_cell = actions.clone();
        let mut draw = |ui: &mut Ui| {
            let frame = Frame::new(&state);
            let mut out = UiOut::new();
            *output_cell.borrow_mut() = host.draw(ui, &frame, &mut out);
            *actions_cell.borrow_mut() = out.take();
        };
        harness.frame(Vec::new(), &mut draw);
        let beta_tab = rect_for(&output.borrow(), PANEL_B);
        harness.click(beta_tab.center(), &mut draw);
    }

    let actions = actions.borrow();
    assert!(
        actions.iter().any(|action| matches!(
            action,
            Action::Layout(LayoutAction::FocusPanel { panel }) if *panel == PANEL_B
        )),
        "clicking an inactive tab must request FocusPanel, got {actions:?}"
    );
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::Layout(LayoutAction::ClosePanel { .. }))),
        "a tab click must not close the panel"
    );
}

#[test]
fn close_click_emits_close_panel_action() {
    let mut registry = PanelRegistry::new();
    register_fake(
        &mut registry,
        PANEL_A,
        "Alpha",
        DockSlot::Left,
        Rc::new(Cell::new(0)),
        |p| p,
    );
    let state = state_with(&[PANEL_A], PANEL_A);
    let mut host = PanelHost::new(registry);
    host.sync(&state);
    let mut harness = themed_harness();

    let output = Rc::new(RefCell::new(PanelHostOutput::default()));
    let actions = Rc::new(RefCell::new(Vec::<Action>::new()));
    {
        let output_cell = output.clone();
        let actions_cell = actions.clone();
        let mut draw = |ui: &mut Ui| {
            let frame = Frame::new(&state);
            let mut out = UiOut::new();
            *output_cell.borrow_mut() = host.draw(ui, &frame, &mut out);
            *actions_cell.borrow_mut() = out.take();
        };
        harness.frame(Vec::new(), &mut draw);
        let close = close_rect_for(&output.borrow(), PANEL_A);
        harness.click(close.center(), &mut draw);
    }

    let actions = actions.borrow();
    assert!(
        actions.iter().any(|action| matches!(
            action,
            Action::Layout(LayoutAction::ClosePanel { panel }) if *panel == PANEL_A
        )),
        "clicking the close affordance must request ClosePanel, got {actions:?}"
    );
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::Layout(LayoutAction::FocusPanel { .. }))),
        "a close click must not also focus the panel"
    );
}

// ---------------------------------------------------------------------------
// panel body -> UiOut
// ---------------------------------------------------------------------------

#[test]
fn panel_body_draw_pushes_actions_into_ui_out() {
    let draws = Rc::new(Cell::new(0));
    let mut registry = PanelRegistry::new();
    register_fake(
        &mut registry,
        TEST_PANEL,
        "Test",
        DockSlot::Center,
        draws.clone(),
        |panel| panel.with_action(Action::Layout(LayoutAction::PersistLayout)),
    );
    let state = state_with(&[TEST_PANEL], TEST_PANEL);
    let mut host = PanelHost::new(registry);
    host.sync(&state);
    let mut harness = themed_harness();

    let (output, actions) = show_once(&mut harness, &mut host, &state);
    assert_eq!(
        output.active_panels,
        vec![TEST_PANEL],
        "custom kind works in the center slot"
    );
    assert!(
        actions.contains(&Action::Layout(LayoutAction::PersistLayout)),
        "actions pushed by Panel::draw reach the frame UiOut, got {actions:?}"
    );
    assert!(draws.get() >= 1);
}

#[test]
fn wants_continuous_repaint_aggregates_open_panels() {
    let mut registry = PanelRegistry::new();
    register_fake(
        &mut registry,
        PANEL_A,
        "Animated",
        DockSlot::Left,
        Rc::new(Cell::new(0)),
        |panel| panel.with_repaint(),
    );
    register_fake(
        &mut registry,
        PANEL_B,
        "Static",
        DockSlot::Center,
        Rc::new(Cell::new(0)),
        |panel| panel,
    );
    let state = state_with(&[PANEL_A, PANEL_B], PANEL_A);
    let mut host = PanelHost::new(registry);
    host.sync(&state);
    assert!(host.wants_continuous_repaint());

    let mut harness = themed_harness();
    let (output, _actions) = show_once(&mut harness, &mut host, &state);
    assert!(output.wants_continuous_repaint);

    // Once the animated panel is closed only the static one remains.
    let state = state_with(&[PANEL_B], PANEL_B);
    let mut registry = PanelRegistry::new();
    register_fake(
        &mut registry,
        PANEL_B,
        "Static",
        DockSlot::Center,
        Rc::new(Cell::new(0)),
        |panel| panel,
    );
    let mut host = PanelHost::new(registry);
    host.sync(&state);
    let mut harness = themed_harness();
    let (output, _actions) = show_once(&mut harness, &mut host, &state);
    assert!(!host.wants_continuous_repaint());
    assert!(!output.wants_continuous_repaint);
}

// ---------------------------------------------------------------------------
// Frame host resources + real panels
// ---------------------------------------------------------------------------

#[test]
fn frame_with_host_exposes_host_resources_and_frame_new_uses_empty_host() {
    let state = EditorState::new();
    let host = TestHost {
        lines: vec!["hello".to_owned(), "world".to_owned()],
        texture: Some(TextureId::User(7)),
    };
    let frame = Frame::with_host(&state, &host);
    assert_eq!(
        frame.host.console_lines(),
        ["hello".to_owned(), "world".to_owned()].as_slice()
    );
    assert_eq!(
        frame.host.viewport_texture(ViewportId::SCENE),
        Some(TextureId::User(7))
    );

    let empty = Frame::new(&state);
    assert!(empty.host.console_lines().is_empty());
    assert_eq!(empty.host.viewport_texture(ViewportId::SCENE), None);
}

#[test]
fn console_panel_renders_the_host_log_snapshot_without_emitting_actions() {
    let state = state_with(&[PanelId::CONSOLE], PanelId::CONSOLE);
    let host = TestHost {
        lines: vec!["line one".to_owned(), "line two".to_owned()],
        texture: None,
    };
    let mut panel = ConsolePanel::new();
    let mut harness = themed_harness();

    let output = Rc::new(RefCell::new(
        editor_components_prototype::ConsolePanelOutput::default(),
    ));
    let actions = Rc::new(RefCell::new(Vec::<Action>::new()));
    {
        let output_cell = output.clone();
        let actions_cell = actions.clone();
        let mut draw = |ui: &mut Ui| {
            let frame = Frame::with_host(&state, &host);
            let mut out = UiOut::new();
            *output_cell.borrow_mut() = panel.show(ui, &frame, &mut out);
            *actions_cell.borrow_mut() = out.take();
        };
        harness.frame(Vec::new(), &mut draw);
    }

    let output = output.borrow();
    assert_eq!(
        output.line_count, 2,
        "console reads the host snapshot each frame"
    );
    assert!(output.log_rect.is_positive(), "log area is laid out");
    assert!(actions.borrow().is_empty(), "the console is read-only");
}

#[test]
fn editor_defaults_registry_renders_builtin_panels_through_the_host() {
    let mut state = state_with(
        &[PanelId::PROJECT, PanelId::INSPECTOR, PanelId::CONSOLE],
        PanelId::PROJECT,
    );
    let mut host = PanelHost::new(PanelRegistry::editor_defaults());
    host.sync(&state);
    assert_eq!(host.instance_count(), 3);
    assert!(host.is_open(PanelId::PROJECT));
    assert!(host.is_open(PanelId::INSPECTOR));
    assert!(host.is_open(PanelId::CONSOLE));

    let mut harness = themed_harness();
    let (output, _actions) = show_once(&mut harness, &mut host, &state);
    assert_eq!(
        output.active_panels,
        vec![PanelId::PROJECT, PanelId::INSPECTOR, PanelId::CONSOLE],
        "draw order is left, right, bottom"
    );
    assert!(
        output
            .tab_rects
            .iter()
            .any(|(kind, _rect)| *kind == PanelId::CONSOLE)
    );
    assert!(
        output
            .panel_rects
            .iter()
            .all(|(_kind, rect)| rect.is_positive())
    );

    // Closing a built-in panel through the layout drops its instance on sync.
    state.layout.close(PanelId::PROJECT);
    host.sync(&state);
    assert_eq!(host.instance_count(), 2);
    assert!(!host.is_open(PanelId::PROJECT));
}
