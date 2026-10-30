//! Selection reducers.

use crate::action::SelectionAction;
use crate::effect::{Effect, RequestKind};
use crate::state::{AssetId, EditorState, PanelId};

pub fn reduce(state: &mut EditorState, action: SelectionAction) -> Vec<Effect> {
    match action {
        SelectionAction::SelectAsset { asset } => select_asset(state, asset),
        SelectionAction::SelectDocument { document } => {
            if state.documents.get(document).is_some() {
                state.selection.active_document = Some(document);
            }
            Vec::new()
        }
        SelectionAction::ClearAssetSelection => {
            state.selection.selected_asset = None;
            Vec::new()
        }
    }
}

/// Cross-slice rule: selecting an asset also opens/focuses the inspector and
/// starts an async load; the inspector gets its data from effects, not from a
/// view reaching into another view.
fn select_asset(state: &mut EditorState, asset: AssetId) -> Vec<Effect> {
    if state.selection.selected_asset == Some(asset)
        && state.project.loading_assets.contains(&asset)
    {
        return Vec::new();
    }
    let path = state
        .project
        .asset(asset)
        .map(|entry| entry.path.clone())
        .unwrap_or_default();
    let ticket = state.begin_request(RequestKind::LoadAsset { asset });
    state.selection.selected_asset = Some(asset);
    state.project.loading_assets.insert(asset);
    state.layout.open(PanelId::Inspector);
    state.layout.focus = PanelId::Inspector;
    vec![
        Effect::FocusPanel {
            panel: PanelId::Inspector,
        },
        Effect::LoadAsset {
            ticket,
            asset,
            path,
        },
    ]
}
