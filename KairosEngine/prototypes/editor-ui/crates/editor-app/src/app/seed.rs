//! Demo state for the runnable prototype: a project with assets and one open
//! material document, so the first frame shows all panels doing real work.

use editor_core_prototype::{
    Action, App, DocumentAction, DocumentKind, EditData, MaterialData, ProjectAction, ProjectState,
    SelectionAction,
};

/// Seed the demo project/document so the prototype opens with something to look at.
pub(super) fn seed_demo_state(core: &mut App) {
    let demo = ProjectState::demo();
    core.dispatch(Action::Project(ProjectAction::AssetsDiscovered {
        assets: demo.assets,
    }));
    if let Some(asset) = core
        .state
        .project
        .assets
        .iter()
        .find(|asset| asset.path.ends_with(".material"))
        .cloned()
    {
        core.dispatch(Action::Selection(SelectionAction::SelectAsset {
            asset: asset.id,
        }));
        core.dispatch(Action::Document(DocumentAction::Open {
            path: asset.path,
            kind: DocumentKind::Material,
            content: EditData::Material(MaterialData::default()),
        }));
    }
}
