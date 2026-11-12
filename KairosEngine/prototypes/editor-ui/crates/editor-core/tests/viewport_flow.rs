//! End-to-end reducer tests for the viewport slice.
//!
//! These cover the contracts the app shell and the viewport panel rely on:
//! idempotent `Ensure`, bind/pick ticket validation, resize replay after bind,
//! camera intents only after binding, viewport destroy on panel close, and the
//! fact that panel identity is opaque data (a new panel kind needs no core edit).

use editor_core_prototype::{
    Action, App, CameraInput, Effect, EffectOutcome, LayoutAction, PanelId, SceneObjectId,
    SelectionAction, ViewportAction, ViewportId,
};

const SCENE: ViewportId = ViewportId::SCENE;

fn ensure(app: &mut App, width: u32, height: u32) -> Vec<Effect> {
    app.dispatch(Action::Viewport(ViewportAction::Ensure {
        viewport: SCENE,
        width,
        height,
    }))
}

fn create_ticket(effects: &[Effect]) -> editor_core_prototype::EffectTicket {
    effects
        .iter()
        .find_map(|effect| match effect {
            Effect::CreateViewport { ticket, .. } => Some(*ticket),
            _ => None,
        })
        .expect("expected a CreateViewport effect")
}

fn bind(app: &mut App, width: u32, height: u32) {
    let effects = ensure(app, width, height);
    let ticket = create_ticket(&effects);
    app.complete(ticket, EffectOutcome::ViewportBound);
    assert!(
        app.state
            .viewports
            .get(SCENE)
            .expect("bound viewport")
            .bound
    );
}

#[test]
fn ensure_creates_target_once_and_binding_is_ticket_validated() {
    let mut app = App::new();
    assert!(!app.state.viewports.contains(SCENE));

    let effects = ensure(&mut app, 800, 600);
    assert_eq!(effects.len(), 1);
    let ticket = create_ticket(&effects);
    let effect = &effects[0];
    assert_eq!(
        effect,
        &Effect::CreateViewport {
            ticket,
            viewport: SCENE,
            width: 800,
            height: 600,
        }
    );
    let info = app.state.viewports.get(SCENE).expect("viewport recorded");
    assert_eq!(info.size, (800, 600));
    assert!(!info.bound);
    assert_eq!(info.pending_bind, Some(ticket));

    // Same size while the bind is in flight: idempotent, no second request.
    assert!(ensure(&mut app, 800, 600).is_empty());

    // Completion is routed through the ticket recorded by the reducer.
    let effects = app.complete(ticket, EffectOutcome::ViewportBound);
    assert!(effects.is_empty());
    assert!(app.state.viewports.get(SCENE).expect("viewport").bound);
    assert_eq!(
        app.state
            .viewports
            .get(SCENE)
            .expect("viewport")
            .pending_bind,
        None
    );

    // Bound + same size: still a no-op.
    assert!(ensure(&mut app, 800, 600).is_empty());
}

#[test]
fn resize_while_binding_is_replayed_after_bind_completion() {
    let mut app = App::new();
    let effects = ensure(&mut app, 320, 200);
    let ticket = create_ticket(&effects);

    // The dock/DPI changes size before the host finished creating the target.
    assert!(ensure(&mut app, 640, 480).is_empty());
    assert_eq!(
        app.state.viewports.get(SCENE).expect("viewport").size,
        (640, 480)
    );

    let effects = app.complete(ticket, EffectOutcome::ViewportBound);
    assert_eq!(
        effects,
        vec![Effect::SyncViewportSize {
            viewport: SCENE,
            width: 640,
            height: 480,
        }]
    );
}

#[test]
fn resize_after_bind_emits_sync_effect() {
    let mut app = App::new();
    bind(&mut app, 800, 600);
    let effects = ensure(&mut app, 1280, 720);
    assert_eq!(
        effects,
        vec![Effect::SyncViewportSize {
            viewport: SCENE,
            width: 1280,
            height: 720,
        }]
    );
}

#[test]
fn zero_sized_viewport_is_clamped() {
    let mut app = App::new();
    let effects = ensure(&mut app, 0, 0);
    let _ = create_ticket(&effects);
    assert_eq!(
        app.state.viewports.get(SCENE).expect("viewport").size,
        (1, 1)
    );
}

#[test]
fn camera_intents_are_forwarded_only_after_binding() {
    let mut app = App::new();
    let effects = ensure(&mut app, 800, 600);
    let ticket = create_ticket(&effects);

    // Unbound: nothing to orbit yet.
    assert!(
        app.dispatch(Action::Viewport(ViewportAction::Orbit {
            viewport: SCENE,
            delta: [10.0, 4.0],
        }))
        .is_empty()
    );

    app.complete(ticket, EffectOutcome::ViewportBound);

    let effects = app.dispatch(Action::Viewport(ViewportAction::Orbit {
        viewport: SCENE,
        delta: [10.0, 4.0],
    }));
    assert_eq!(
        effects,
        vec![Effect::ViewportCameraInput {
            viewport: SCENE,
            input: CameraInput {
                orbit: [10.0, 4.0],
                ..CameraInput::default()
            },
        }]
    );

    // A zero intent must not reach the host (it would stamp camera change ticks
    // every frame in the real editor).
    assert!(
        app.dispatch(Action::Viewport(ViewportAction::Pan {
            viewport: SCENE,
            delta: [0.0, 0.0],
        }))
        .is_empty()
    );
}

#[test]
fn pick_round_trip_updates_scene_selection_and_ignores_stale_tickets() {
    let mut app = App::new();
    bind(&mut app, 800, 600);

    let effects = app.dispatch(Action::Viewport(ViewportAction::Pick {
        viewport: SCENE,
        ndc: [0.0, 0.0],
    }));
    let ticket = match effects.as_slice() {
        [Effect::PickViewport { ticket, ndc, .. }] => {
            assert_eq!(*ndc, [0.0, 0.0]);
            *ticket
        }
        other => panic!("expected one PickViewport effect, got {other:?}"),
    };

    // Second pick while the first is in flight is dropped, not queued.
    assert!(
        app.dispatch(Action::Viewport(ViewportAction::Pick {
            viewport: SCENE,
            ndc: [0.5, 0.5],
        }))
        .is_empty()
    );

    app.complete(
        ticket,
        EffectOutcome::SceneObjectPicked {
            object: Some(SceneObjectId(1)),
        },
    );
    assert_eq!(
        app.state.selection.selected_scene_object,
        Some(SceneObjectId(1))
    );

    // Stale completion for the same ticket is ignored after the slot cleared.
    app.complete(
        ticket,
        EffectOutcome::SceneObjectPicked {
            object: Some(SceneObjectId(9)),
        },
    );
    assert_eq!(
        app.state.selection.selected_scene_object,
        Some(SceneObjectId(1))
    );

    // Clicking empty space clears the scene selection.
    let effects = app.dispatch(Action::Viewport(ViewportAction::Pick {
        viewport: SCENE,
        ndc: [-1.0, -1.0],
    }));
    let ticket = match effects.as_slice() {
        [Effect::PickViewport { ticket, .. }] => *ticket,
        other => panic!("expected one PickViewport effect, got {other:?}"),
    };
    app.complete(ticket, EffectOutcome::SceneObjectPicked { object: None });
    assert_eq!(app.state.selection.selected_scene_object, None);
}

#[test]
fn bind_failure_is_recorded_without_marking_the_viewport_bound() {
    let mut app = App::new();
    let effects = ensure(&mut app, 800, 600);
    let ticket = create_ticket(&effects);
    app.complete(
        ticket,
        EffectOutcome::ViewportBindFailed {
            error: "device lost".to_string(),
        },
    );
    let info = app.state.viewports.get(SCENE).expect("viewport");
    assert!(!info.bound);
    assert_eq!(info.last_error.as_deref(), Some("device lost"));
}

#[test]
fn closing_the_viewport_panel_destroys_the_target_and_reopen_recreates() {
    let mut app = App::new();
    assert!(app.state.layout.is_open(PanelId::VIEWPORT));
    bind(&mut app, 800, 600);

    let effects = app.dispatch(Action::Layout(LayoutAction::ClosePanel {
        panel: PanelId::VIEWPORT,
    }));
    assert_eq!(effects, vec![Effect::DestroyViewport { viewport: SCENE }]);
    assert!(!app.state.layout.is_open(PanelId::VIEWPORT));
    assert!(!app.state.viewports.contains(SCENE));

    // Closing again is idempotent.
    assert!(
        app.dispatch(Action::Layout(LayoutAction::ClosePanel {
            panel: PanelId::VIEWPORT,
        }))
        .is_empty()
    );

    // Re-opening is a layout concern; the panel's first Ensure creates a fresh
    // target and a fresh ticket.
    app.dispatch(Action::Layout(LayoutAction::TogglePanel {
        panel: PanelId::VIEWPORT,
    }));
    assert!(app.state.layout.is_open(PanelId::VIEWPORT));
    let effects = ensure(&mut app, 800, 600);
    let ticket = create_ticket(&effects);
    assert_eq!(
        app.state
            .viewports
            .get(SCENE)
            .expect("viewport")
            .pending_bind,
        Some(ticket)
    );

    app.complete(ticket, EffectOutcome::ViewportBound);
    assert!(app.state.viewports.get(SCENE).expect("viewport").bound);
}

#[test]
fn completion_for_a_destroyed_viewport_is_ignored() {
    let mut app = App::new();
    let effects = ensure(&mut app, 800, 600);
    let old_ticket = create_ticket(&effects);
    app.dispatch(Action::Layout(LayoutAction::ClosePanel {
        panel: PanelId::VIEWPORT,
    }));

    // Re-open and create a new target; the old completion must not bind it.
    app.dispatch(Action::Layout(LayoutAction::TogglePanel {
        panel: PanelId::VIEWPORT,
    }));
    let effects = ensure(&mut app, 800, 600);
    let new_ticket = create_ticket(&effects);
    assert_ne!(old_ticket, new_ticket);
    app.complete(old_ticket, EffectOutcome::ViewportBound);
    let info = app.state.viewports.get(SCENE).expect("viewport");
    assert!(!info.bound);
    assert_eq!(info.pending_bind, Some(new_ticket));
}

#[test]
fn panel_identity_is_opaque_data_so_new_panels_need_no_core_edit() {
    let gallery = PanelId("gallery");
    let mut app = App::new();
    app.dispatch(Action::Layout(LayoutAction::ShowPanel { panel: gallery }));
    assert!(app.state.layout.is_open(gallery));
    assert_eq!(app.state.layout.focus, gallery);
    assert_eq!(gallery.0, "gallery");
}

#[test]
fn clearing_asset_selection_does_not_touch_scene_selection() {
    let mut app = App::new();
    app.state.selection.selected_scene_object = Some(SceneObjectId(1));
    app.dispatch(Action::Selection(SelectionAction::ClearAssetSelection));
    assert_eq!(
        app.state.selection.selected_scene_object,
        Some(SceneObjectId(1))
    );
}
