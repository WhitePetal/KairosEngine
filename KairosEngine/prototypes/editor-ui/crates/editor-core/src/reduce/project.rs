//! Project/asset reducers: async scan results, asset loading, preview spawning,
//! and the plain project mutations the asset-browser toolbar needs.
//!
//! Mutations are "data in, effects out": they edit `ProjectState` and return
//! `Effect::PersistProject` so persistence remains a host concern.

use crate::action::ProjectAction;
use crate::effect::{Effect, RequestKind};
use crate::state::{AssetEntry, AssetId, AssetKind, EditorState};

pub fn reduce(state: &mut EditorState, action: ProjectAction) -> Vec<Effect> {
    match action {
        ProjectAction::AssetsDiscovered { assets } => {
            state.project.assets = assets;
            Vec::new()
        }
        ProjectAction::AssetLoadCompleted { asset, ok, error } => {
            state.project.loading_assets.remove(&asset);
            if !ok {
                let message = error.unwrap_or_else(|| "unknown error".to_string());
                return vec![Effect::ShowNativeDialog {
                    title: "Asset load failed".to_string(),
                    message,
                }];
            }
            if state.project.preview_ready.contains(&asset) {
                return Vec::new();
            }
            let ticket = state.begin_request(RequestKind::SpawnPreview { asset });
            vec![Effect::SpawnPreview { ticket, asset }]
        }
        ProjectAction::PreviewSpawned { asset } => {
            state.project.preview_ready.insert(asset);
            Vec::new()
        }
        ProjectAction::CreateAsset { parent, name, kind } => {
            create_asset(state, parent, name, kind)
        }
        ProjectAction::DeleteAsset { asset } => delete_asset(state, asset),
        ProjectAction::RenameAsset { asset, name } => rename_asset(state, asset, name),
    }
}

/// Root path of the addressed parent folder; `None` if the id is not a folder.
fn folder_path(state: &EditorState, parent: Option<AssetId>) -> Option<String> {
    match parent {
        None => Some(String::new()),
        Some(asset) => state
            .project
            .asset(asset)
            .filter(|entry| entry.kind == AssetKind::Folder)
            .map(|entry| entry.path.trim_end_matches('/').to_string()),
    }
}

fn create_asset(
    state: &mut EditorState,
    parent: Option<AssetId>,
    name: String,
    kind: AssetKind,
) -> Vec<Effect> {
    let name = name.trim();
    if name.is_empty() {
        return Vec::new();
    }
    let Some(parent_path) = folder_path(state, parent) else {
        return Vec::new();
    };
    let path = if parent_path.is_empty() {
        name.to_string()
    } else {
        format!("{parent_path}/{name}")
    };
    // Unique paths: creating an existing row is a no-op (the real host reports
    // an error; keeping it silent keeps the prototype's reducer total).
    if state.project.assets.iter().any(|entry| entry.path == path) {
        return Vec::new();
    }
    let id = state.project.next_asset_id();
    state.project.assets.push(AssetEntry { id, path, kind });
    state.project.assets.sort_by(|a, b| a.path.cmp(&b.path));
    vec![Effect::PersistProject]
}

fn delete_asset(state: &mut EditorState, asset: AssetId) -> Vec<Effect> {
    let before = state.project.assets.len();
    state.project.assets.retain(|entry| entry.id != asset);
    if state.project.assets.len() == before {
        return Vec::new();
    }
    if state.selection.selected_asset == Some(asset) {
        state.selection.selected_asset = None;
    }
    state.project.loading_assets.remove(&asset);
    state.project.preview_ready.remove(&asset);
    vec![Effect::PersistProject]
}

fn rename_asset(state: &mut EditorState, asset: AssetId, name: String) -> Vec<Effect> {
    let name = name.trim();
    if name.is_empty() {
        return Vec::new();
    }
    let Some(entry) = state.project.asset(asset).cloned() else {
        return Vec::new();
    };
    let (parent, _) = split_path(&entry.path);
    let new_path = if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    };
    if new_path == entry.path {
        return Vec::new();
    }
    if state
        .project
        .assets
        .iter()
        .any(|other| other.id != asset && other.path == new_path)
    {
        return Vec::new();
    }
    // Renaming a folder entry rewrites every descendant path, so the
    // path-derived view tree stays consistent.
    let old_prefix = format!("{}/", entry.path);
    for other in &mut state.project.assets {
        if other.id == asset {
            other.path = new_path.clone();
        } else if entry.kind == AssetKind::Folder && other.path.starts_with(&old_prefix) {
            other.path = format!("{new_path}/{}", &other.path[old_prefix.len()..]);
        }
    }
    state.project.assets.sort_by(|a, b| a.path.cmp(&b.path));
    vec![Effect::PersistProject]
}

fn split_path(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(index) => (&path[..index], &path[index + 1..]),
        None => ("", path),
    }
}
