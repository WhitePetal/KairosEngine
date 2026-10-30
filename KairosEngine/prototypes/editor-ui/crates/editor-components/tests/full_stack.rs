//! Full-stack round-trip tests: this is the reason the two prototypes were
//! merged into one workspace.
//!
//! Chain under test (no fakes anywhere):
//!
//!   App::new() -> dispatch(AssetsDiscovered)
//!     -> render ProjectPanel/InspectorPanel headlessly through ui-widgets
//!     -> synthesize a pointer gesture with the widgets test harness
//!     -> collect UiOut actions (real editor-core Action values)
//!     -> app.dispatch every action through the real reducer
//!     -> assert state/effects, then App::complete async tickets
//!
//! A failure here means the component/widget layer can no longer drive the real
//! core reducer, which is exactly the risk the merged prototype exists to catch.

use std::cell::RefCell;
use std::rc::Rc;

use editor_components_prototype::{row_id, Frame, InspectorPanel, ProjectPanel, UiOut};
use editor_core_prototype::{
    Action, App, AssetId, AssetKind, DocumentAction, DocumentId, DocumentKind, EditData, Effect,
    EffectOutcome, MaterialData, PanelId, ProjectAction, ProjectState,
};
use egui::{Ui, pos2, Rect};
use ui_widgets_prototype::Theme;
use ui_widgets_prototype::testing::Harness;

fn themed_harness() -> Harness {
    let harness = Harness::default();
    Theme::dark().install(&harness.ctx);
    harness
}

fn demo_app() -> App {
    let mut app = App::new();
    app.dispatch(Action::Project(ProjectAction::AssetsDiscovered {
        assets: ProjectState::demo().assets,
    }));
    app
}

fn open_material(app: &mut App) -> DocumentId {
    app.dispatch(Action::Document(DocumentAction::Open {
        path: "assets/models/crate.material".to_string(),
        kind: DocumentKind::Material,
        content: EditData::Material(MaterialData::default()),
    }));
    app.state
        .selection
        .active_document
        .expect("opening a document selects it")
}

fn open_text(app: &mut App, text: &str) -> DocumentId {
    app.dispatch(Action::Document(DocumentAction::Open {
        path: "notes.txt".to_string(),
        kind: DocumentKind::Text,
        content: EditData::Text(text.to_string()),
    }));
    app.state
        .selection
        .active_document
        .expect("opening a document selects it")
}

fn dispatch_all(app: &mut App, actions: Vec<Action>) -> Vec<Effect> {
    let mut effects = Vec::new();
    for action in actions {
        effects.extend(app.dispatch(action));
    }
    effects
}

/// Render one project-panel frame, click a row by asset path (single or
/// double click), then dispatch every emitted Action through the real App.
fn click_project_row(
    app: &mut App,
    panel: &mut ProjectPanel,
    harness: &mut Harness,
    path: &str,
    double: bool,
) -> Vec<Effect> {
    let row_rects = Rc::new(RefCell::new(Vec::<(u64, Rect)>::new()));
    let actions = Rc::new(RefCell::new(Vec::<Action>::new()));
    {
        let row_rects = row_rects.clone();
        let actions = actions.clone();
        let state = &app.state;
        let mut draw = |ui: &mut Ui| {
            let frame = Frame::new(state);
            let mut out = UiOut::new();
            let output = panel.show(ui, &frame, &mut out);
            *row_rects.borrow_mut() = output.row_rects;
            *actions.borrow_mut() = out.take();
        };
        harness.frame(Vec::new(), &mut draw);
        let target = row_id(path);
        let rect = row_rects
            .borrow()
            .iter()
            .find(|(id, _)| *id == target)
            .map(|(_, rect)| *rect)
            .expect("project row rect");
        if double {
            harness.double_click(rect.center(), &mut draw);
        } else {
            harness.click(rect.center(), &mut draw);
        }
    }
    dispatch_all(app, actions.borrow().clone())
}

fn click_project_toolbar(
    app: &mut App,
    panel: &mut ProjectPanel,
    harness: &mut Harness,
    item: &'static str,
) -> Vec<Effect> {
    let toolbar_rects = Rc::new(RefCell::new(Vec::<(&'static str, Rect)>::new()));
    let actions = Rc::new(RefCell::new(Vec::<Action>::new()));
    {
        let toolbar_rects = toolbar_rects.clone();
        let actions = actions.clone();
        let state = &app.state;
        let mut draw = |ui: &mut Ui| {
            let frame = Frame::new(state);
            let mut out = UiOut::new();
            let output = panel.show(ui, &frame, &mut out);
            *toolbar_rects.borrow_mut() = output.toolbar_rects;
            *actions.borrow_mut() = out.take();
        };
        harness.frame(Vec::new(), &mut draw);
        let rect = toolbar_rects
            .borrow()
            .iter()
            .find(|(id, _)| *id == item)
            .map(|(_, rect)| *rect)
            .expect("project toolbar item rect");
        harness.click(rect.center(), &mut draw);
    }
    dispatch_all(app, actions.borrow().clone())
}

fn click_inspector_toolbar(
    app: &mut App,
    panel: &mut InspectorPanel,
    harness: &mut Harness,
    item: &'static str,
) -> Vec<Effect> {
    let toolbar_rects = Rc::new(RefCell::new(Vec::<(&'static str, Rect)>::new()));
    let actions = Rc::new(RefCell::new(Vec::<Action>::new()));
    {
        let toolbar_rects = toolbar_rects.clone();
        let actions = actions.clone();
        let state = &app.state;
        let mut draw = |ui: &mut Ui| {
            let frame = Frame::new(state);
            let mut out = UiOut::new();
            let output = panel.show(ui, &frame, &mut out);
            *toolbar_rects.borrow_mut() = output.toolbar_rects;
            *actions.borrow_mut() = out.take();
        };
        harness.frame(Vec::new(), &mut draw);
        let rect = toolbar_rects
            .borrow()
            .iter()
            .find(|(id, _)| *id == item)
            .map(|(_, rect)| *rect)
            .expect("inspector toolbar item rect");
        harness.click(rect.center(), &mut draw);
    }
    dispatch_all(app, actions.borrow().clone())
}

#[test]
fn project_row_click_round_trips_into_core_reducer() {
    let mut app = demo_app();
    let mut panel = ProjectPanel::expanded(&app.state.project);
    let mut harness = themed_harness();

    let effects = click_project_row(
        &mut app,
        &mut panel,
        &mut harness,
        "assets/models/crate.material",
        false,
    );

    assert_eq!(app.state.selection.selected_asset, Some(AssetId(2)));
    assert_eq!(app.state.layout.focus, PanelId::Inspector);
    assert!(
        app.state.project.loading_assets.contains(&AssetId(2)),
        "selection starts the async asset load through the real reducer"
    );
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::FocusPanel { panel: PanelId::Inspector }
    )));

    let ticket = effects
        .iter()
        .find_map(|effect| match effect {
            Effect::LoadAsset { ticket, asset, .. } if *asset == AssetId(2) => Some(*ticket),
            _ => None,
        })
        .expect("LoadAsset effect");

    let effects = app.complete(ticket, EffectOutcome::AssetLoaded);
    assert!(!app.state.project.loading_assets.contains(&AssetId(2)));
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, Effect::SpawnPreview { .. })),
        "completion is routed back through App::complete"
    );
}

#[test]
fn project_double_click_opens_material_document_via_core() {
    let mut app = demo_app();
    let mut panel = ProjectPanel::expanded(&app.state.project);
    let mut harness = themed_harness();

    click_project_row(
        &mut app,
        &mut panel,
        &mut harness,
        "assets/models/crate.material",
        true,
    );

    let document_id = app
        .state
        .selection
        .active_document
        .expect("double click opens a document through DocumentAction::Open");
    let document = app.state.documents.get(document_id).expect("document exists");
    assert_eq!(document.path, "assets/models/crate.material");
    assert_eq!(document.kind, DocumentKind::Material);
    assert!(matches!(document.edit, EditData::Material(_)));
    assert!(!document.dirty);
}

#[test]
fn project_toolbar_create_folder_round_trips_into_core_state() {
    let mut app = demo_app();
    let before = app.state.project.assets.len();
    let mut panel = ProjectPanel::expanded(&app.state.project);
    let mut harness = themed_harness();

    let effects = click_project_toolbar(&mut app, &mut panel, &mut harness, "create-folder");

    assert_eq!(app.state.project.assets.len(), before + 1);
    let created = app
        .state
        .project
        .assets
        .iter()
        .find(|asset| asset.path == "new_folder_9")
        .expect("created folder entry");
    assert_eq!(created.kind, AssetKind::Folder);
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, Effect::PersistProject)),
        "project mutations are persisted through an Effect"
    );
}

#[test]
fn material_slider_click_round_trips_set_material() {
    let mut app = demo_app();
    let document_id = open_material(&mut app);
    let mut panel = InspectorPanel::new();
    let mut harness = themed_harness();

    let field_rects = Rc::new(RefCell::new(Vec::<(String, Rect)>::new()));
    // Sliders are drag widgets: the value (and the emitted SetMaterial action)
    // changes on intermediate frames, so accumulate actions instead of keeping
    // only the last pass. (Duplicates are harmless: the reducer ignores an edit
    // whose content equals the current content.)
    let actions = Rc::new(RefCell::new(Vec::<Action>::new()));
    {
        let field_rects = field_rects.clone();
        let actions = actions.clone();
        let state = &app.state;
        let mut draw = |ui: &mut Ui| {
            let frame = Frame::new(state);
            let mut out = UiOut::new();
            let output = panel.show(ui, &frame, &mut out);
            *field_rects.borrow_mut() = output.field_rects;
            actions.borrow_mut().extend(out.take());
        };
        harness.frame(Vec::new(), &mut draw);
        let rect = field_rects
            .borrow()
            .iter()
            .find(|(key, _)| key == "metallic")
            .map(|(_, rect)| *rect)
            .expect("metallic property editor rect");
        // Drag along the slider rail to raise the value.
        harness.drag(
            pos2(rect.left() + 8.0, rect.center().y),
            pos2(rect.right() - 8.0, rect.center().y),
            &mut draw,
        );
    }

    let collected = actions.borrow().clone();
    assert!(!collected.is_empty(), "slider change must emit SetMaterial");
    dispatch_all(&mut app, collected);

    let document = app.state.documents.get(document_id).expect("document exists");
    match &document.edit {
        EditData::Material(material) => {
            assert!(
                material.metallic > 0.5,
                "click near the right edge raises metallic, got {}",
                material.metallic
            );
        }
        other => panic!("expected a material document, got {other:?}"),
    }
    assert!(document.dirty);
}

#[test]
fn inspector_save_click_round_trips_effect_then_completion() {
    let mut app = demo_app();
    let document_id = open_material(&mut app);
    app.dispatch(Action::Document(DocumentAction::SetMaterial {
        document: document_id,
        material: MaterialData { roughness: 0.9, ..Default::default() },
    }));
    assert!(app.state.documents.get(document_id).unwrap().dirty);

    let mut panel = InspectorPanel::new();
    let mut harness = themed_harness();
    let effects = click_inspector_toolbar(&mut app, &mut panel, &mut harness, "save");

    let document = app.state.documents.get(document_id).unwrap();
    assert!(document.dirty, "dirty until the host acknowledges the write");
    assert!(document.is_saving());

    let ticket = effects
        .iter()
        .find_map(|effect| match effect {
            Effect::SaveDocument { ticket, document, bytes } if *document == document_id => {
                assert!(!bytes.is_empty(), "save effect carries the serialized document");
                Some(*ticket)
            }
            _ => None,
        })
        .expect("SaveDocument effect");

    app.complete(ticket, EffectOutcome::DocumentSaved);
    let document = app.state.documents.get(document_id).unwrap();
    assert!(!document.dirty);
    assert!(!document.is_saving());
    assert_eq!(document.saved, document.edit);
    assert!(app.state.inflight.is_empty());
}

#[test]
fn inspector_text_typing_round_trips_set_text() {
    let mut app = App::new();
    let document_id = open_text(&mut app, "hello");
    let mut panel = InspectorPanel::new();
    let mut harness = themed_harness();

    let field_rects = Rc::new(RefCell::new(Vec::<(String, Rect)>::new()));
    let actions = Rc::new(RefCell::new(Vec::<Action>::new()));
    {
        let field_rects = field_rects.clone();
        let actions = actions.clone();
        let state = &app.state;
        let mut draw = |ui: &mut Ui| {
            let frame = Frame::new(state);
            let mut out = UiOut::new();
            let output = panel.show(ui, &frame, &mut out);
            *field_rects.borrow_mut() = output.field_rects;
            *actions.borrow_mut() = out.take();
        };
        harness.frame(Vec::new(), &mut draw);
        let rect = field_rects
            .borrow()
            .iter()
            .find(|(key, _)| key == "text")
            .map(|(_, rect)| *rect)
            .expect("text editor rect");
        // Click near the right edge to place the caret after "hello", then type.
        harness.click(pos2(rect.right() - 3.0, rect.center().y), &mut draw);
        harness.type_text(" world", &mut draw);
    }

    let collected = actions.borrow().clone();
    assert!(!collected.is_empty(), "typing must emit SetText");
    dispatch_all(&mut app, collected);

    let document = app.state.documents.get(document_id).expect("document exists");
    assert_eq!(document.edit.as_text(), Some("hello world"));
    assert!(document.dirty);
}

#[test]
fn inspector_undo_and_redo_toolbar_round_trip_core_history() {
    let mut app = App::new();
    let document_id = open_text(&mut app, "hello");
    app.dispatch(Action::Document(DocumentAction::SetText {
        document: document_id,
        text: "hello world".to_string(),
    }));
    assert!(app.state.documents.get(document_id).unwrap().dirty);

    let mut panel = InspectorPanel::new();
    let mut harness = themed_harness();

    let effects = click_inspector_toolbar(&mut app, &mut panel, &mut harness, "undo");
    assert!(effects.is_empty(), "undo is a state-only action");
    let document = app.state.documents.get(document_id).unwrap();
    assert_eq!(document.edit.as_text(), Some("hello"));
    assert!(!document.dirty, "undoing back to saved content is clean");
    assert!(document.can_redo());

    let _ = click_inspector_toolbar(&mut app, &mut panel, &mut harness, "redo");
    let document = app.state.documents.get(document_id).unwrap();
    assert_eq!(document.edit.as_text(), Some("hello world"));
    assert!(document.dirty);
}
