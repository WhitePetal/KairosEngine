//! Project/asset-browser feature component.
//!
//! Maps the real `editor_core_prototype::ProjectState` (flat `AssetEntry` list,
//! no folder structure) into themed widget props by **deriving the folder tree
//! from asset paths**, and maps widget Output data back into real core Actions.
//!
//! Everything the UI needs but the domain must not own (search text, expanded
//! folder paths) lives in [ProjectViewState] -- the stand-in for ui::ViewStates.
//! The component never mutates state and never does IO: it only pushes Actions.

use std::collections::{BTreeMap, BTreeSet};

use editor_core_prototype::{
    Action, AssetEntry, AssetId, AssetKind, DocumentAction, DocumentKind, EditData, LayoutAction,
    MaterialData, PanelId, ProjectAction, ProjectState, SelectionAction,
};
use egui::{Pos2, Rect, Ui};
use ui_widgets_prototype::{
    Breadcrumb, Expander, IconId, PanelChrome, SearchBox, StatusBar, Toolbar, ToolbarItem,
    ToolbarOutput, TreeOutput, TreeRow, TreeView,
};

use crate::frame::{Frame, UiOut};

/// UI-only state of the project panel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectViewState {
    pub search: String,
    /// Expanded folder paths. Folders are derived from asset paths, so the path
    /// is the only stable key (the core has no folder ids).
    pub expanded: BTreeSet<String>,
}

impl ProjectViewState {
    /// Expand every folder derivable from the project's asset paths.
    pub fn expanded_all(project: &ProjectState) -> Self {
        Self { search: String::new(), expanded: folder_paths(project) }
    }
}

/// One visible row of the path-derived tree, before widget conversion.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectNode {
    pub path: String,
    pub name: String,
    pub depth: u8,
    pub is_folder: bool,
    pub asset: Option<AssetId>,
    pub kind: Option<AssetKind>,
}

/// Non-action results of one frame (hit rects for tests/host overlays).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectPanelOutput {
    pub row_rects: Vec<(u64, Rect)>,
    pub toolbar_rects: Vec<(&'static str, Rect)>,
    pub context_menu: Option<(u64, Pos2)>,
}

#[derive(Default)]
pub struct ProjectPanel {
    pub view: ProjectViewState,
}

impl ProjectPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Demo/tests: start with every derived folder expanded.
    pub fn expanded(project: &ProjectState) -> Self {
        Self { view: ProjectViewState::expanded_all(project) }
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        frame: &Frame,
        out: &mut UiOut,
    ) -> ProjectPanelOutput {
        let state = frame.state;
        let mut panel = ProjectPanelOutput::default();

        let toolbar = Toolbar::new(vec![
            ToolbarItem::button("create-folder")
                .icon(IconId::Plus)
                .tooltip("New folder"),
            ToolbarItem::button("delete")
                .icon(IconId::Trash)
                .tooltip("Delete selected asset")
                .enabled(state.selection.selected_asset.is_some()),
            ToolbarItem::button("focus-inspector")
                .icon(IconId::Settings)
                .tooltip("Focus inspector"),
        ]);
        let mut chrome = PanelChrome::new("Project").toolbar(toolbar);

        let segments: Vec<&str> = match state
            .selection
            .selected_asset
            .and_then(|id| state.project.asset(id))
        {
            Some(asset) => asset.path.split('/').collect(),
            None => Vec::new(),
        };
        let nodes = project_nodes(&state.project, &self.view);
        let expanded = self.view.expanded.clone();
        let selected = state.selection.selected_asset;

        let chrome_output = chrome.show(ui, |ui| {
            let mut search = SearchBox::new("Search assets");
            search.show(ui, &mut self.view.search);

            if !segments.is_empty() {
                let mut crumbs = Breadcrumb::new(segments.clone());
                crumbs.show(ui);
            }

            let mut tree = TreeView::new("project-tree").rows(tree_rows(&nodes, &expanded, selected));
            let tree_output = tree.show(ui);
            panel.row_rects = tree_output.row_rects.clone();

            // Folder expand/collapse is *view* state, never a domain action.
            let toggle = tree_output.expander_toggled.or(tree_output.clicked);
            if let Some(id) = toggle {
                if let Some(node) = nodes.iter().find(|node| row_id(&node.path) == id) {
                    if node.is_folder {
                        if !self.view.expanded.remove(&node.path) {
                            self.view.expanded.insert(node.path.clone());
                        }
                    }
                }
            }
            if let Some(id) = tree_output.context_menu {
                panel.context_menu = tree_output.context_menu_pos.map(|pos| (id, pos));
            }

            out.extend(actions_from_tree(&nodes, &tree_output));

            let left = format!("{} assets", state.project.assets.len());
            let right = match selected.and_then(|id| state.project.asset(id)) {
                Some(asset) => format!("selected: {}", asset.path),
                None => "nothing selected".to_owned(),
            };
            let mut status = StatusBar::new(&left, &right);
            status.show(ui);
        });

        if let Some(toolbar_output) = &chrome_output.toolbar {
            panel.toolbar_rects = toolbar_output.rects.clone();
            out.extend(actions_from_toolbar(
                &state.project,
                state.selection.selected_asset,
                toolbar_output,
            ));
        }
        panel
    }
}

// ---------------------------------------------------------------------------
// pure derivation: ProjectState -> tree nodes
// ---------------------------------------------------------------------------

/// Stable egui row id for a path (FNV-1a). Folder rows have no AssetId, so the
/// widget's u64 row id is derived from the path; this keeps ids stable across
/// frames and independent of the current filter/expansion.
pub fn row_id(path: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// All folder paths implied by asset paths (plus explicit folder entries).
pub fn folder_paths(project: &ProjectState) -> BTreeSet<String> {
    let mut folders = BTreeSet::new();
    for asset in &project.assets {
        let segments: Vec<&str> = asset.path.split('/').collect();
        let mut prefix = String::new();
        for segment in &segments[..segments.len().saturating_sub(1)] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            folders.insert(prefix.clone());
        }
        if asset.kind == AssetKind::Folder {
            folders.insert(asset.path.clone());
        }
    }
    folders
}

/// Flattens the path-derived tree into the visible rows for the current
/// search/expansion view state.
pub fn project_nodes(project: &ProjectState, view: &ProjectViewState) -> Vec<ProjectNode> {
    let folders = folder_paths(project);

    // path -> (is_folder, asset entry if any)
    let mut entries: BTreeMap<String, (bool, Option<&AssetEntry>)> = BTreeMap::new();
    for folder in &folders {
        entries.insert(folder.clone(), (true, None));
    }
    for asset in &project.assets {
        entries.insert(
            asset.path.clone(),
            (asset.kind == AssetKind::Folder, Some(asset)),
        );
    }

    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in entries.keys() {
        let parent = parent_of(path).to_string();
        children.entry(parent).or_default().push(path.clone());
    }
    for list in children.values_mut() {
        list.sort();
    }

    let query = view.search.trim().to_ascii_lowercase();
    let mut nodes = Vec::new();
    walk(
        &children, &entries, "", 0, view, &query, &mut nodes,
    );
    nodes
}

fn walk(
    children: &BTreeMap<String, Vec<String>>,
    entries: &BTreeMap<String, (bool, Option<&AssetEntry>)>,
    parent: &str,
    depth: u8,
    view: &ProjectViewState,
    query: &str,
    nodes: &mut Vec<ProjectNode>,
) {
    let Some(list) = children.get(parent) else {
        return;
    };
    for path in list {
        let (is_folder, asset) = entries[path];
        let name = name_of(path);
        let self_matches = query.is_empty() || name.to_ascii_lowercase().contains(query);
        if !self_matches && !has_matching_descendant(children, path, query) {
            continue;
        }
        nodes.push(ProjectNode {
            path: path.clone(),
            name: name.to_string(),
            depth,
            is_folder,
            asset: asset.map(|entry| entry.id),
            kind: asset.map(|entry| entry.kind),
        });
        if is_folder && (view.expanded.contains(path) || !query.is_empty()) {
            walk(children, entries, path, depth + 1, view, query, nodes);
        }
    }
}

fn has_matching_descendant(
    children: &BTreeMap<String, Vec<String>>,
    parent: &str,
    query: &str,
) -> bool {
    let Some(list) = children.get(parent) else {
        return false;
    };
    list.iter().any(|path| {
        name_of(path).to_ascii_lowercase().contains(query)
            || has_matching_descendant(children, path, query)
    })
}

fn parent_of(path: &str) -> &str {
    match path.rfind('/') {
        Some(index) => &path[..index],
        None => "",
    }
}

fn name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn icon_for(kind: Option<AssetKind>, is_folder: bool, expanded: bool) -> IconId {
    if is_folder {
        return if expanded { IconId::FolderOpen } else { IconId::Folder };
    }
    match kind {
        Some(AssetKind::Texture) => IconId::Texture,
        Some(AssetKind::Scene) => IconId::Scene,
        Some(AssetKind::Material) => IconId::Material,
        Some(AssetKind::Audio) => IconId::Info,
        Some(AssetKind::Toml) => IconId::Script,
        Some(AssetKind::Binary) | None => IconId::File,
        Some(AssetKind::Folder) => IconId::Folder,
    }
}

fn kind_label(kind: Option<AssetKind>) -> Option<&'static str> {
    match kind? {
        AssetKind::Texture => Some("tex"),
        AssetKind::Scene => Some("scene"),
        AssetKind::Material => Some("mat"),
        AssetKind::Toml => Some("toml"),
        AssetKind::Audio => Some("audio"),
        AssetKind::Binary => Some("bin"),
        AssetKind::Folder => None,
    }
}

/// Widget props: tree nodes -> themed rows.
pub fn tree_rows<'a>(
    nodes: &'a [ProjectNode],
    expanded: &BTreeSet<String>,
    selected: Option<AssetId>,
) -> Vec<TreeRow<'a>> {
    nodes
        .iter()
        .map(|node| {
            let is_expanded = node.is_folder && expanded.contains(&node.path);
            let mut row = TreeRow::new(row_id(&node.path), &node.name)
                .indent(node.depth)
                .icon(icon_for(node.kind, node.is_folder, is_expanded))
                .selected(!node.is_folder && node.asset.is_some() && node.asset == selected);
            if node.is_folder {
                row = row.expander(if is_expanded {
                    Expander::Expanded
                } else {
                    Expander::Collapsed
                });
            }
            if let Some(label) = kind_label(node.kind) {
                row = row.suffix(label);
            }
            row
        })
        .collect()
}

// ---------------------------------------------------------------------------
// pure mapping: widget Output -> core Action
// ---------------------------------------------------------------------------

pub fn actions_from_tree(nodes: &[ProjectNode], output: &TreeOutput) -> Vec<Action> {
    let mut actions = Vec::new();
    let find = |id: u64| nodes.iter().find(|node| row_id(&node.path) == id);

    if let Some(id) = output.clicked {
        if let Some(node) = find(id) {
            if let Some(asset) = node.asset.filter(|_| !node.is_folder) {
                actions.push(Action::Selection(SelectionAction::SelectAsset { asset }));
            }
        }
    }
    if let Some(id) = output.double_clicked {
        if let Some(node) = find(id) {
            if !node.is_folder {
                if let Some(action) = open_action(&node.path, node.kind) {
                    actions.push(action);
                }
            }
        }
    }
    actions
}

/// Double-click mapping: the prototype fabricates placeholder content by kind.
/// The real host would emit `Effect::LoadDocument` and open on completion.
pub fn open_action(path: &str, kind: Option<AssetKind>) -> Option<Action> {
    let (doc_kind, content) = match kind? {
        AssetKind::Material => (
            DocumentKind::Material,
            EditData::Material(MaterialData::default()),
        ),
        AssetKind::Toml => (DocumentKind::Toml, EditData::Toml(String::new())),
        _ => (DocumentKind::Text, EditData::Text(String::new())),
    };
    Some(Action::Document(DocumentAction::Open {
        path: path.to_string(),
        kind: doc_kind,
        content,
    }))
}

pub fn actions_from_toolbar(
    project: &ProjectState,
    selected: Option<AssetId>,
    output: &ToolbarOutput,
) -> Vec<Action> {
    match output.clicked {
        Some("create-folder") => {
            let parent = selected
                .and_then(|id| project.asset(id))
                .filter(|asset| asset.kind == AssetKind::Folder)
                .map(|asset| asset.id);
            let name = format!("new_folder_{}", project.assets.len() + 1);
            vec![Action::Project(ProjectAction::CreateAsset {
                parent,
                name,
                kind: AssetKind::Folder,
            })]
        }
        Some("delete") => selected
            .map(|asset| vec![Action::Project(ProjectAction::DeleteAsset { asset })])
            .unwrap_or_default(),
        Some("focus-inspector") => vec![Action::Layout(LayoutAction::FocusPanel {
            panel: PanelId::Inspector,
        })],
        _ => Vec::new(),
    }
}
