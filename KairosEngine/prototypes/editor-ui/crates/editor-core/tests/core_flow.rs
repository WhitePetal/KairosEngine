//! Integration tests: the whole editor core is exercised without egui, GPU,
//! window or engine runtime. Every assertion is plain state inspection.

use std::collections::BTreeSet;

use editor_core_prototype::*;

fn demo_assets() -> Vec<AssetEntry> {
    vec![
        AssetEntry {
            id: AssetId(1),
            path: "assets/textures/hero.png".to_string(),
            kind: AssetKind::Texture,
        },
        AssetEntry {
            id: AssetId(2),
            path: "assets/scenes/level01.kairos".to_string(),
            kind: AssetKind::Scene,
        },
    ]
}

fn app_with_assets() -> App {
    let mut app = App::new();
    app.dispatch(Action::Project(ProjectAction::AssetsDiscovered {
        assets: demo_assets(),
    }));
    app
}

fn open_text(app: &mut App, path: &str, text: &str) -> DocumentId {
    app.dispatch(Action::Document(DocumentAction::Open {
        path: path.to_string(),
        kind: DocumentKind::Text,
        content: EditData::Text(text.to_string()),
    }));
    app.state
        .selection
        .active_document
        .expect("open selects the document")
}

fn text_of(app: &App, id: DocumentId) -> String {
    app.state
        .documents
        .get(id)
        .expect("document exists")
        .edit
        .as_text()
        .expect("text document")
        .to_string()
}

fn save_ticket(effects: &[Effect]) -> EffectTicket {
    effects
        .iter()
        .find_map(|effect| match effect {
            Effect::SaveDocument { ticket, .. } => Some(*ticket),
            _ => None,
        })
        .expect("expected a SaveDocument effect")
}

fn load_ticket(effects: &[Effect]) -> EffectTicket {
    effects
        .iter()
        .find_map(|effect| match effect {
            Effect::LoadAsset { ticket, .. } => Some(*ticket),
            _ => None,
        })
        .expect("expected a LoadAsset effect")
}

#[test]
fn selecting_asset_focuses_inspector_and_starts_load() {
    let mut app = app_with_assets();
    app.dispatch(Action::Layout(LayoutAction::TogglePanel {
        panel: PanelId::INSPECTOR,
    }));
    assert!(!app.state.layout.is_open(PanelId::INSPECTOR));

    let effects = app.dispatch(Action::Selection(SelectionAction::SelectAsset {
        asset: AssetId(1),
    }));

    assert_eq!(app.state.selection.selected_asset, Some(AssetId(1)));
    assert_eq!(app.state.layout.focus, PanelId::INSPECTOR);
    assert!(app.state.layout.is_open(PanelId::INSPECTOR));
    assert!(app.state.project.loading_assets.contains(&AssetId(1)));
    assert!(effects.contains(&Effect::FocusPanel {
        panel: PanelId::INSPECTOR
    }));

    let ticket = load_ticket(&effects);
    assert_eq!(
        app.state.inflight.get(&ticket),
        Some(&RequestKind::LoadAsset { asset: AssetId(1) })
    );
}

#[test]
fn editing_marks_document_dirty_and_records_history() {
    let mut app = App::new();
    let id = open_text(&mut app, "notes/todo.txt", "hello");
    assert!(!app.state.documents.get(id).unwrap().dirty);

    app.dispatch(Action::Document(DocumentAction::EditText {
        document: id,
        insert: " world".to_string(),
    }));

    let doc = app.state.documents.get(id).unwrap();
    assert!(doc.dirty);
    assert_eq!(doc.edit, EditData::Text("hello world".to_string()));
    assert_eq!(doc.history.len(), 1);
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
}

#[test]
fn save_emits_effect_and_completion_clears_dirty() {
    let mut app = App::new();
    let id = open_text(&mut app, "notes/todo.txt", "hello");
    app.dispatch(Action::Document(DocumentAction::EditText {
        document: id,
        insert: " world".into(),
    }));

    let effects = app.dispatch(Action::Document(DocumentAction::Save { document: id }));
    let ticket = save_ticket(&effects);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::SaveDocument { bytes, .. } if bytes == b"hello world"
    )));

    let doc = app.state.documents.get(id).unwrap();
    assert!(doc.dirty, "dirty until the host acknowledges the write");
    assert!(doc.is_saving());

    // A second save request while one is in flight is a no-op.
    assert!(
        app.dispatch(Action::Document(DocumentAction::Save { document: id }))
            .is_empty()
    );

    app.complete(ticket, EffectOutcome::DocumentSaved);

    let doc = app.state.documents.get(id).unwrap();
    assert!(!doc.dirty);
    assert!(!doc.is_saving());
    assert_eq!(doc.saved, EditData::Text("hello world".to_string()));
    assert!(app.state.inflight.is_empty());
}

#[test]
fn closing_dirty_document_opens_confirm_modal() {
    let mut app = App::new();
    let id = open_text(&mut app, "notes/todo.txt", "hello");
    app.dispatch(Action::Document(DocumentAction::EditText {
        document: id,
        insert: "!".into(),
    }));

    let effects = app.dispatch(Action::Document(DocumentAction::Close { document: id }));
    assert!(effects.is_empty());
    assert!(
        app.state.documents.get(id).is_some(),
        "must not close before confirmation"
    );

    let dialog = app.state.modal.confirm_dialog().expect("modal is open");
    assert_eq!(
        dialog.confirm,
        Action::Document(DocumentAction::SaveThenClose { document: id })
    );
    assert_eq!(dialog.cancel, Action::Modal(ModalAction::Dismiss));
}

#[test]
fn confirming_unsaved_close_saves_then_closes() {
    let mut app = App::new();
    let id = open_text(&mut app, "notes/todo.txt", "hello");
    app.dispatch(Action::Document(DocumentAction::EditText {
        document: id,
        insert: "!".into(),
    }));
    app.dispatch(Action::Document(DocumentAction::Close { document: id }));

    let effects = app.dispatch(Action::Modal(ModalAction::Confirm));
    assert!(!app.state.modal.is_open());
    let ticket = save_ticket(&effects);
    let doc = app
        .state
        .documents
        .get(id)
        .expect("closes only after the save completes");
    assert!(doc.close_after_save);

    app.complete(ticket, EffectOutcome::DocumentSaved);
    assert!(app.state.documents.get(id).is_none());
    assert_eq!(app.state.selection.active_document, None);
}

#[test]
fn cancelling_unsaved_close_keeps_document_dirty() {
    let mut app = App::new();
    let id = open_text(&mut app, "notes/todo.txt", "hello");
    app.dispatch(Action::Document(DocumentAction::EditText {
        document: id,
        insert: "!".into(),
    }));
    app.dispatch(Action::Document(DocumentAction::Close { document: id }));

    let effects = app.dispatch(Action::Modal(ModalAction::Cancel));
    assert!(effects.is_empty(), "cancel does nothing else");
    assert!(!app.state.modal.is_open());
    let doc = app.state.documents.get(id).expect("document still open");
    assert!(doc.dirty);
}

#[test]
fn undo_and_redo_restore_text_and_dirty_flag() {
    let mut app = App::new();
    let id = open_text(&mut app, "notes/todo.txt", "hello");
    app.dispatch(Action::Document(DocumentAction::EditText {
        document: id,
        insert: " world".into(),
    }));

    let effects = app.dispatch(Action::Document(DocumentAction::Save { document: id }));
    app.complete(save_ticket(&effects), EffectOutcome::DocumentSaved);
    assert!(!app.state.documents.get(id).unwrap().dirty);

    app.dispatch(Action::Document(DocumentAction::EditText {
        document: id,
        insert: "!".into(),
    }));
    assert_eq!(text_of(&app, id), "hello world!");
    assert!(app.state.documents.get(id).unwrap().dirty);

    app.dispatch(Action::Document(DocumentAction::Undo { document: id }));
    assert_eq!(text_of(&app, id), "hello world");
    assert!(
        !app.state.documents.get(id).unwrap().dirty,
        "undoing back to saved content is clean"
    );
    assert!(app.state.documents.get(id).unwrap().can_redo());

    app.dispatch(Action::Document(DocumentAction::Redo { document: id }));
    assert_eq!(text_of(&app, id), "hello world!");
    assert!(app.state.documents.get(id).unwrap().dirty);

    app.dispatch(Action::Document(DocumentAction::Undo { document: id }));
    app.dispatch(Action::Document(DocumentAction::Undo { document: id }));
    assert_eq!(text_of(&app, id), "hello");
    assert!(app.state.documents.get(id).unwrap().dirty);
    assert!(!app.state.documents.get(id).unwrap().can_undo());
}

#[test]
fn revert_restores_last_saved_content_and_is_undoable() {
    let mut app = App::new();
    let id = open_text(&mut app, "notes/todo.txt", "hello");
    app.dispatch(Action::Document(DocumentAction::EditText {
        document: id,
        insert: " changed".into(),
    }));

    app.dispatch(Action::Document(DocumentAction::Revert { document: id }));

    let doc = app.state.documents.get(id).unwrap();
    assert_eq!(doc.edit, EditData::Text("hello".to_string()));
    assert!(!doc.dirty);
    assert!(doc.can_undo(), "revert is itself an undoable transaction");
    assert_eq!(doc.history.last().unwrap().label, "Revert to saved");
}

#[test]
fn command_registry_reports_enabled_state_and_dispatches() {
    let mut app = App::new();
    assert!(
        !app.commands
            .get(CommandId::SAVE_DOCUMENT)
            .unwrap()
            .is_enabled(&app.state)
    );
    assert!(
        app.dispatch_command(CommandId::SAVE_DOCUMENT).is_empty(),
        "disabled commands are inert"
    );

    let id = open_text(&mut app, "notes/todo.txt", "hello");
    assert!(
        !app.commands
            .get(CommandId::UNDO)
            .unwrap()
            .is_enabled(&app.state)
    );
    assert!(
        !app.commands
            .get(CommandId::SAVE_DOCUMENT)
            .unwrap()
            .is_enabled(&app.state)
    );

    app.dispatch(Action::Document(DocumentAction::EditText {
        document: id,
        insert: "!".into(),
    }));
    assert!(
        app.commands
            .get(CommandId::UNDO)
            .unwrap()
            .is_enabled(&app.state)
    );
    assert!(
        app.commands
            .get(CommandId::SAVE_DOCUMENT)
            .unwrap()
            .is_enabled(&app.state)
    );

    let effects = app.dispatch_command(CommandId::SAVE_DOCUMENT);
    let ticket = save_ticket(&effects);
    app.complete(ticket, EffectOutcome::DocumentSaved);
    assert!(
        !app.commands
            .get(CommandId::SAVE_DOCUMENT)
            .unwrap()
            .is_enabled(&app.state)
    );

    // Toolbar/menu/palette/MCP all consume the same registry with stable ids.
    let ids: BTreeSet<CommandId> = app.commands.commands().map(|command| command.id).collect();
    assert_eq!(ids.len(), app.commands.commands().count());
    assert!(
        app.commands
            .commands()
            .all(|command| !command.label.is_empty())
    );
}

#[test]
fn async_completion_clears_loading_state_and_spawns_preview() {
    let mut app = app_with_assets();
    let effects = app.dispatch(Action::Selection(SelectionAction::SelectAsset {
        asset: AssetId(2),
    }));
    let load = load_ticket(&effects);
    assert!(app.state.project.loading_assets.contains(&AssetId(2)));

    let effects = app.complete(load, EffectOutcome::AssetLoaded);
    assert!(
        !app.state.project.loading_assets.contains(&AssetId(2)),
        "loading flag cleared by the completion action"
    );
    let spawn = effects
        .iter()
        .find_map(|effect| match effect {
            Effect::SpawnPreview { ticket, .. } => Some(*ticket),
            _ => None,
        })
        .expect("loaded asset spawns a preview request");
    assert!(app.state.is_inflight(spawn));

    app.complete(spawn, EffectOutcome::PreviewSpawned);
    assert!(app.state.project.preview_ready.contains(&AssetId(2)));
    assert!(app.state.inflight.is_empty());

    // Duplicate/stale completions are ignored instead of double-editing state.
    assert!(app.complete(load, EffectOutcome::AssetLoaded).is_empty());
}

#[test]
fn quitting_with_unsaved_changes_requires_confirmation() {
    let mut app = App::new();
    assert_eq!(
        app.dispatch(Action::App(AppAction::QuitRequested)),
        vec![Effect::Quit]
    );

    let id = open_text(&mut app, "notes/todo.txt", "hello");
    app.dispatch(Action::Document(DocumentAction::EditText {
        document: id,
        insert: "!".into(),
    }));
    assert!(
        app.dispatch(Action::App(AppAction::QuitRequested))
            .is_empty()
    );
    assert!(app.state.modal.confirm_dialog().is_some());

    let effects = app.dispatch(Action::Modal(ModalAction::Confirm));
    assert_eq!(effects, vec![Effect::Quit]);
    assert!(!app.state.modal.is_open());
}

#[test]
fn layout_toggle_and_persist_are_state_plus_effects() {
    let mut app = App::new();
    assert!(app.state.layout.is_open(PanelId::CONSOLE));

    app.dispatch(Action::Layout(LayoutAction::TogglePanel {
        panel: PanelId::CONSOLE,
    }));
    assert!(!app.state.layout.is_open(PanelId::CONSOLE));

    let effects = app.dispatch(Action::Layout(LayoutAction::PersistLayout));
    match &effects[0] {
        Effect::PersistLayout { layout } => {
            assert!(!layout.is_open(PanelId::CONSOLE));
            assert_eq!(layout.focus, app.state.layout.focus);
        }
        other => panic!("unexpected effect: {other:?}"),
    }
}

#[test]
fn save_failure_keeps_document_dirty_and_reports_native_dialog() {
    let mut app = App::new();
    let id = open_text(&mut app, "notes/todo.txt", "hello");
    app.dispatch(Action::Document(DocumentAction::EditText {
        document: id,
        insert: "!".into(),
    }));
    let effects = app.dispatch(Action::Document(DocumentAction::Save { document: id }));
    let ticket = save_ticket(&effects);

    let effects = app.complete(
        ticket,
        EffectOutcome::DocumentSaveFailed {
            error: "disk full".to_string(),
        },
    );

    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::ShowNativeDialog { message, .. } if message == "disk full"
    )));
    let doc = app.state.documents.get(id).unwrap();
    assert!(doc.dirty, "failed save must keep the document dirty");
    assert!(!doc.is_saving());
    assert!(app.state.inflight.is_empty());
}

// ---------------------------------------------------------------------------
// Merged prototype additions: demo project + plain project mutations that the
// asset-browser toolbar (crates/editor-components) dispatches.
// ---------------------------------------------------------------------------

#[test]
fn demo_project_has_nested_paths_and_stable_ids() {
    let state = ProjectState::demo();
    assert_eq!(state.assets.len(), 8);
    assert_eq!(state.assets[0].id, AssetId(1));
    assert_eq!(state.next_asset_id(), AssetId(9));
    assert!(state.assets.iter().any(|asset| {
        asset.path == "assets/models/crate.material" && asset.kind == AssetKind::Material
    }));
    assert!(
        state
            .assets
            .iter()
            .all(|asset| asset.path.starts_with("assets/")),
        "demo paths are nested so the view can derive folders"
    );
}

#[test]
fn create_asset_adds_row_and_requests_persist() {
    let mut app = App::new();
    app.dispatch(Action::Project(ProjectAction::AssetsDiscovered {
        assets: ProjectState::demo().assets,
    }));

    let effects = app.dispatch(Action::Project(ProjectAction::CreateAsset {
        parent: None,
        name: "imports".to_string(),
        kind: AssetKind::Folder,
    }));
    assert_eq!(effects, vec![Effect::PersistProject]);
    assert!(
        app.state
            .project
            .assets
            .iter()
            .any(|asset| { asset.path == "imports" && asset.kind == AssetKind::Folder })
    );
    assert_eq!(app.state.project.next_asset_id(), AssetId(10));

    // Duplicate paths are rejected instead of creating ambiguous rows.
    let duplicate = app.dispatch(Action::Project(ProjectAction::CreateAsset {
        parent: None,
        name: "imports".to_string(),
        kind: AssetKind::Folder,
    }));
    assert!(duplicate.is_empty());
}

#[test]
fn create_asset_under_existing_folder_asset() {
    let mut app = App::new();
    app.dispatch(Action::Project(ProjectAction::AssetsDiscovered {
        assets: vec![AssetEntry {
            id: AssetId(1),
            path: "assets".to_string(),
            kind: AssetKind::Folder,
        }],
    }));

    app.dispatch(Action::Project(ProjectAction::CreateAsset {
        parent: Some(AssetId(1)),
        name: "new_folder".to_string(),
        kind: AssetKind::Folder,
    }));
    assert_eq!(
        app.state
            .project
            .asset(AssetId(2))
            .map(|asset| asset.path.as_str()),
        Some("assets/new_folder")
    );

    // A parent that is not a folder is rejected (no silent wrong path).
    let effects = app.dispatch(Action::Project(ProjectAction::CreateAsset {
        parent: Some(AssetId(2)),
        name: "child".to_string(),
        kind: AssetKind::Toml,
    }));
    assert_eq!(effects, vec![Effect::PersistProject]);
    assert_eq!(
        app.state
            .project
            .asset(AssetId(3))
            .map(|asset| asset.path.as_str()),
        Some("assets/new_folder/child")
    );
}

#[test]
fn delete_asset_removes_row_and_clears_selection() {
    let mut app = App::new();
    app.dispatch(Action::Project(ProjectAction::AssetsDiscovered {
        assets: ProjectState::demo().assets,
    }));
    app.dispatch(Action::Selection(SelectionAction::SelectAsset {
        asset: AssetId(2),
    }));
    assert_eq!(app.state.selection.selected_asset, Some(AssetId(2)));

    let effects = app.dispatch(Action::Project(ProjectAction::DeleteAsset {
        asset: AssetId(2),
    }));
    assert_eq!(effects, vec![Effect::PersistProject]);
    assert!(app.state.project.asset(AssetId(2)).is_none());
    assert_eq!(app.state.selection.selected_asset, None);
}

#[test]
fn rename_folder_rewrites_descendant_paths() {
    let mut app = App::new();
    app.dispatch(Action::Project(ProjectAction::AssetsDiscovered {
        assets: vec![
            AssetEntry {
                id: AssetId(1),
                path: "assets/models".to_string(),
                kind: AssetKind::Folder,
            },
            AssetEntry {
                id: AssetId(2),
                path: "assets/models/hero.scene".to_string(),
                kind: AssetKind::Scene,
            },
        ],
    }));

    let effects = app.dispatch(Action::Project(ProjectAction::RenameAsset {
        asset: AssetId(1),
        name: "meshes".to_string(),
    }));
    assert_eq!(effects, vec![Effect::PersistProject]);
    assert_eq!(
        app.state
            .project
            .asset(AssetId(1))
            .map(|asset| asset.path.as_str()),
        Some("assets/meshes")
    );
    assert_eq!(
        app.state
            .project
            .asset(AssetId(2))
            .map(|asset| asset.path.as_str()),
        Some("assets/meshes/hero.scene"),
        "descendants are rewritten so the path-derived tree stays consistent"
    );
}

#[test]
fn documents_demo_helpers_create_material_and_text_docs() {
    let mut state = EditorState::new();
    let material = state.documents.demo_material();
    let text = state.documents.demo_text();

    assert_eq!(state.documents.len(), 2);
    assert!(matches!(
        state.documents.get(material).map(|doc| &doc.edit),
        Some(EditData::Material(_))
    ));
    assert!(matches!(
        state.documents.get(text).map(|doc| &doc.edit),
        Some(EditData::Toml(_))
    ));
}
