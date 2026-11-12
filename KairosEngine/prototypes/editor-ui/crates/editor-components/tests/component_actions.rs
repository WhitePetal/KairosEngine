//! Pure mapping tests for the merged components (no egui frame needed):
//! real editor-core ProjectState -> path-derived tree -> widget props, and
//! widget Output -> real core Actions.

use editor_components_prototype::*;
use editor_core_prototype::{
    Action, AssetEntry, AssetId, AssetKind, DocumentAction, DocumentKind, EditData, EditorState,
    LayoutAction, MaterialData, PanelId, ProjectAction, ProjectState, SelectionAction,
};
use ui_widgets_prototype::{Expander, ToolbarOutput, TreeOutput};

fn demo_project() -> ProjectState {
    ProjectState::demo()
}

fn expanded_view(project: &ProjectState) -> ProjectViewState {
    ProjectViewState::expanded_all(project)
}

fn node<'a>(nodes: &'a [ProjectNode], path: &str) -> &'a ProjectNode {
    nodes
        .iter()
        .find(|node| node.path == path)
        .unwrap_or_else(|| panic!("missing project node {path}"))
}

#[test]
fn path_tree_derives_folders_with_stable_ids() {
    let project = demo_project();
    let view = expanded_view(&project);
    let nodes = project_nodes(&project, &view);

    let names: Vec<&str> = nodes.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "assets",
            "audio",
            "theme.ogg",
            "config",
            "settings.toml",
            "models",
            "broken.bin",
            "crate.material",
            "hero.scene",
            "scenes",
            "level01.kairos",
            "textures",
            "hero.png",
            "noise.texture",
        ]
    );
    assert_eq!(nodes.len(), 14);

    let assets = node(&nodes, "assets");
    assert!(assets.is_folder);
    assert_eq!(assets.asset, None, "derived folders have no AssetId");
    assert_eq!(assets.depth, 0);
    assert_eq!(node(&nodes, "assets/models").depth, 1);
    assert_eq!(node(&nodes, "assets/models/crate.material").depth, 2);

    // Row ids are derived from paths, so they are stable across frames/filters.
    let again = project_nodes(&project, &expanded_view(&project));
    assert_eq!(row_id(&assets.path), row_id(&again[0].path));
}

#[test]
fn collapsed_folders_hide_children() {
    let project = demo_project();

    let collapsed = project_nodes(&project, &ProjectViewState::default());
    assert_eq!(collapsed.len(), 1, "only the root folder is visible");
    assert_eq!(collapsed[0].name, "assets");
    assert!(collapsed[0].is_folder);

    let mut view = ProjectViewState::default();
    view.expanded.insert("assets".to_string());
    let expanded_root = project_nodes(&project, &view);
    let names: Vec<&str> = expanded_root
        .iter()
        .map(|node| node.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["assets", "audio", "config", "models", "scenes", "textures"]
    );
}

#[test]
fn search_shows_matching_assets_and_their_ancestors() {
    let project = demo_project();
    let view = ProjectViewState {
        search: "hero".to_string(),
        expanded: Default::default(),
    };
    let nodes = project_nodes(&project, &view);
    let names: Vec<&str> = nodes.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["assets", "models", "hero.scene", "textures", "hero.png"],
        "search ignores collapse state and keeps ancestors of matches"
    );
}

#[test]
fn tree_rows_carry_labels_indent_expander_and_selection() {
    let project = demo_project();
    let view = expanded_view(&project);
    let nodes = project_nodes(&project, &view);
    let rows = tree_rows(&nodes, &view.expanded, Some(AssetId(2)));

    assert_eq!(rows.len(), nodes.len());

    let crate_row = rows
        .iter()
        .find(|row| row.label == "crate.material")
        .unwrap();
    assert!(crate_row.selected, "crate.material is the selected asset");
    assert_eq!(crate_row.indent, 2);
    assert_eq!(crate_row.suffix, Some("mat"));
    assert_eq!(crate_row.expander, Expander::None);

    let models_row = rows.iter().find(|row| row.label == "models").unwrap();
    assert_eq!(models_row.expander, Expander::Expanded);
    assert!(!models_row.selected);

    let collapsed_rows = tree_rows(&nodes, &Default::default(), None);
    let collapsed_models = collapsed_rows
        .iter()
        .find(|row| row.label == "models")
        .unwrap();
    assert_eq!(collapsed_models.expander, Expander::Collapsed);
}

#[test]
fn tree_click_maps_to_core_selection_action() {
    let project = demo_project();
    let view = expanded_view(&project);
    let nodes = project_nodes(&project, &view);

    let crate_node = node(&nodes, "assets/models/crate.material");
    let output = TreeOutput {
        clicked: Some(row_id(&crate_node.path)),
        ..Default::default()
    };
    assert_eq!(
        project_actions_from_tree(&nodes, &output),
        vec![Action::Selection(SelectionAction::SelectAsset {
            asset: AssetId(2)
        })]
    );

    // Clicking a folder is a view toggle, never a domain action.
    let models = node(&nodes, "assets/models");
    let output = TreeOutput {
        clicked: Some(row_id(&models.path)),
        ..Default::default()
    };
    assert!(project_actions_from_tree(&nodes, &output).is_empty());
}

#[test]
fn double_click_maps_to_open_with_content_by_kind() {
    let project = demo_project();
    let view = expanded_view(&project);
    let nodes = project_nodes(&project, &view);

    let open_for = |path: &str| {
        let node = node(&nodes, path);
        project_actions_from_tree(
            &nodes,
            &TreeOutput {
                double_clicked: Some(row_id(&node.path)),
                ..Default::default()
            },
        )
    };

    assert_eq!(
        open_for("assets/models/crate.material"),
        vec![Action::Document(DocumentAction::Open {
            path: "assets/models/crate.material".to_string(),
            kind: DocumentKind::Material,
            content: EditData::Material(MaterialData::default()),
        })]
    );
    assert_eq!(
        open_for("assets/config/settings.toml"),
        vec![Action::Document(DocumentAction::Open {
            path: "assets/config/settings.toml".to_string(),
            kind: DocumentKind::Toml,
            content: EditData::Toml(String::new()),
        })]
    );
    assert_eq!(
        open_for("assets/models/hero.scene"),
        vec![Action::Document(DocumentAction::Open {
            path: "assets/models/hero.scene".to_string(),
            kind: DocumentKind::Text,
            content: EditData::Text(String::new()),
        })]
    );
    assert!(
        open_for("assets/models").is_empty(),
        "double-clicking a folder opens nothing"
    );
}

#[test]
fn project_toolbar_maps_to_core_project_and_layout_actions() {
    let project = demo_project();
    let mut output = ToolbarOutput {
        clicked: Some("create-folder"),
        ..Default::default()
    };

    assert_eq!(
        project_actions_from_toolbar(&project, None, &output),
        vec![Action::Project(ProjectAction::CreateAsset {
            parent: None,
            name: "new_folder_9".to_string(),
            kind: AssetKind::Folder,
        })]
    );

    output.clicked = Some("delete");
    assert_eq!(
        project_actions_from_toolbar(&project, Some(AssetId(3)), &output),
        vec![Action::Project(ProjectAction::DeleteAsset {
            asset: AssetId(3)
        })]
    );

    output.clicked = Some("focus-inspector");
    assert_eq!(
        project_actions_from_toolbar(&project, None, &output),
        vec![Action::Layout(LayoutAction::FocusPanel {
            panel: PanelId::INSPECTOR
        })]
    );

    // A selected folder *entry* (only explicit folder assets have ids) is used
    // as the parent for create-folder; path-derived folders have no id.
    let folder_project = ProjectState {
        assets: vec![AssetEntry {
            id: AssetId(1),
            path: "assets".to_string(),
            kind: AssetKind::Folder,
        }],
        ..Default::default()
    };
    output.clicked = Some("create-folder");
    assert_eq!(
        project_actions_from_toolbar(&folder_project, Some(AssetId(1)), &output),
        vec![Action::Project(ProjectAction::CreateAsset {
            parent: Some(AssetId(1)),
            name: "new_folder_2".to_string(),
            kind: AssetKind::Folder,
        })]
    );
}

#[test]
fn inspector_toolbar_maps_to_core_document_actions() {
    let mut state = EditorState::new();
    let document = state.documents.demo_material();
    state.selection.active_document = Some(document);
    let document_ref = state.active_document().expect("document exists");
    let mut output = ToolbarOutput {
        clicked: Some("save"),
        ..Default::default()
    };

    assert_eq!(
        inspector_actions_from_toolbar(document_ref, &output),
        vec![Action::Document(DocumentAction::Save { document })]
    );
    output.clicked = Some("undo");
    assert_eq!(
        inspector_actions_from_toolbar(document_ref, &output),
        vec![Action::Document(DocumentAction::Undo { document })]
    );
    output.clicked = Some("redo");
    assert_eq!(
        inspector_actions_from_toolbar(document_ref, &output),
        vec![Action::Document(DocumentAction::Redo { document })]
    );
    output.clicked = Some("revert");
    assert_eq!(
        inspector_actions_from_toolbar(document_ref, &output),
        vec![Action::Document(DocumentAction::Revert { document })]
    );

    let material = MaterialData {
        roughness: 0.9,
        metallic: 1.0,
        ..Default::default()
    };
    assert_eq!(
        set_material_action(document_ref, material.clone()),
        Action::Document(DocumentAction::SetMaterial { document, material })
    );
}
