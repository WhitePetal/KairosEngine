//! Project slice reducer: the request/completion protocol around scanning.
//!
//! The reducer never walks the file system. It assigns the next generation,
//! marks the scan running and returns the host work; the app answers with
//! [`ProjectAction::Scanned`], and this is where the answer becomes state: the
//! entries are assembled into the tree, the view intent is pruned to paths that
//! still exist, a pending reveal is honoured, and the walk's failures land in
//! [`ProjectState::error`] where the status bar can render them.
//!
//! The panel's own intents (select, expand, navigate, split) land here too. They
//! are plain state transitions over paths already in the tree: no IO, and the
//! only host work any of them may ask for is [`ProjectEffect::Open`].
//!
//! Two rules keep a slow or duplicated answer from corrupting the tree:
//!
//! * a completion is accepted only while a scan is running **and** its
//!   generation is the current one — a stale result says nothing about the tree
//!   a newer request will produce, and a duplicate delivery of the current one
//!   is not news either;
//! * nothing here is optimistic. A rename or a create does not edit the tree;
//!   it asks for host work and lets the follow-up scan answer. Editing the tree
//!   in place would race the watcher for no benefit, since a rescan is cheap.

use crate::project::{
    Node, ProjectError, ProjectState, ProjectTree, ProjectView, RelPath, ScanState,
};
use crate::{EditorState, Effect, ProjectAction, ProjectEffect};

pub(super) fn reduce(state: &mut EditorState, action: ProjectAction) -> Vec<Effect> {
    match action {
        ProjectAction::PolicyLoaded { policy } => {
            state.project.policy = policy;
            Vec::new()
        }
        ProjectAction::ScanRequested { reason: _, focus } => {
            let project = &mut state.project;
            project.scan.generation = project.scan.generation.wrapping_add(1);
            project.scan.running = true;
            // `None` keeps a reveal that is still pending: a watcher rescan
            // must not cancel the path a rename just asked to show. The
            // completion that lands on it reveals the path and clears it.
            if focus.is_some() {
                project.scan.focus_after_scan = focus;
            }
            vec![Effect::Project(ProjectEffect::Scan {
                root: project.root.clone(),
                policy: project.policy.clone(),
                generation: project.scan.generation,
            })]
        }
        ProjectAction::Scanned {
            generation,
            entries,
            truncated,
            elapsed_ms,
            library_nodes: _,
            errors,
        } => {
            let project = &mut state.project;
            if !project.scan.running || generation != project.scan.generation {
                // Either an older scan finishing late (it says nothing about the
                // tree the newer request will produce) or a duplicate completion
                // for a scan this reducer already answered. Both are dropped
                // whole, never merged.
                return Vec::new();
            }

            let report = project
                .tree
                .assemble(&entries, &project.policy.system_dirs, truncated);
            project.scan.running = false;
            project.scan.last_ms = elapsed_ms;
            project.scan.truncated = truncated;

            let ProjectState {
                tree, view, scan, ..
            } = project;
            settle(tree, view, scan);

            project.error = visible_error(&errors, report.skipped);
            Vec::new()
        }
        ProjectAction::Select { path } => {
            let project = &mut state.project;
            // The selection is replaced, never merged: a click selects one row
            // (M3 turns this into modes). A path the tree does not have is not
            // selectable, so a stale click ends with an empty selection rather
            // than a dangling path.
            project.view.selection.clear();
            if let Some(path) = path
                && project.tree.find(&path).is_some()
            {
                project.view.selection.insert(path);
            }
            Vec::new()
        }
        ProjectAction::ToggleFolder { path } => {
            let project = &mut state.project;
            // Only a folder has a triangle to hit; a file arriving here is stale
            // UI input, not intent, and leaves the view untouched.
            let is_folder = project.tree.get(&path).is_some_and(Node::is_folder);
            if is_folder && !project.view.expanded.remove(&path) {
                project.view.expanded.insert(path);
            }
            Vec::new()
        }
        ProjectAction::Navigate { path } => {
            let project = &mut state.project;
            if project.tree.get(&path).is_some_and(Node::is_folder) {
                // Navigating into a folder shows its contents, so the path
                // itself is expanded too; its ancestors are expanded so the row
                // is actually visible. The root is the window, not a row, and
                // never belongs in `expanded` (the same rule `settle` keeps).
                let mut cursor = Some(path.clone());
                while let Some(ancestor) = cursor {
                    if ancestor.is_root() {
                        break;
                    }
                    if project.tree.get(&ancestor).is_some_and(Node::is_folder) {
                        project.view.expanded.insert(ancestor.clone());
                    }
                    cursor = ancestor.parent();
                }
                project.view.active = Some(path.clone());
                project.view.selection.clear();
                project.view.selection.insert(path);
            }
            Vec::new()
        }
        ProjectAction::OpenRequested { path } => {
            // A request, not a completion: nothing in the state changes here.
            // The app answers it with host work and reports a failure back as
            // `Failed`; an open of a path the tree no longer has is dropped.
            let project = &state.project;
            if project.tree.find(&path).is_none() {
                return Vec::new();
            }
            vec![Effect::Project(ProjectEffect::Open {
                root: project.root.clone(),
                path,
            })]
        }
        ProjectAction::SetShowHidden(show_hidden) => {
            state.project.view.show_hidden = show_hidden;
            Vec::new()
        }
        ProjectAction::CollapseAll => {
            // Rows are what the toolbar collapses: the active directory and the
            // selection are the user's place in the project and survive.
            state.project.view.expanded.clear();
            Vec::new()
        }
        ProjectAction::SetTreeSplit(tree_split) => {
            // NaN fails every comparison, so it would slip through `clamp` and
            // poison the splitter on every frame; a degenerate drag is not
            // intent. The range is the documented one (D1).
            if !tree_split.is_nan() {
                state.project.view.tree_split = tree_split.clamp(0.15, 0.6);
            }
            Vec::new()
        }
        ProjectAction::Failed { message } => {
            state.project.error = Some(ProjectError::new(message));
            Vec::new()
        }
    }
}

/// Bring the view intent back in line with the new tree.
///
/// A path that vanished (renamed outside the editor, deleted by another tool)
/// cannot stay expanded or selected; the active directory falls back to its
/// nearest surviving ancestor rather than resetting to the root, so a renamed
/// directory does not throw the breadcrumb back to the project.
fn settle(tree: &ProjectTree, view: &mut ProjectView, scan: &mut ScanState) {
    // The root is always shown: its children are the first rows, so it never
    // belongs in `expanded` (a persisted view may still carry it).
    view.expanded
        .retain(|path| !path.is_root() && tree.get(path).is_some_and(Node::is_folder));
    view.selection.retain(|path| tree.find(path).is_some());

    if let Some(active) = view.active.clone()
        && tree.find(&active).is_none()
    {
        view.active = nearest_folder(tree, &active);
    }

    // A rename/create asked for its result to be revealed. The path can be gone
    // again (a failed operation, a watcher race): revealing nothing is correct,
    // and the pending request is cleared either way so it cannot fire later
    // against an unrelated tree.
    if let Some(focus) = scan.focus_after_scan.take()
        && tree.find(&focus).is_some()
    {
        let mut cursor = focus.parent();
        while let Some(path) = cursor {
            if path.is_root() {
                break;
            }
            if tree.get(&path).is_some_and(Node::is_folder) {
                view.expanded.insert(path.clone());
            }
            cursor = path.parent();
        }
        view.selection.clear();
        view.selection.insert(focus.clone());
        view.active = focus.parent();
    }
}

/// The nearest directory at or above `path` that still exists. The root always
/// does, so this never returns `None`.
fn nearest_folder(tree: &ProjectTree, path: &RelPath) -> Option<RelPath> {
    let mut cursor = Some(path.clone());
    while let Some(path) = cursor {
        if tree.get(&path).is_some_and(Node::is_folder) {
            return Some(path);
        }
        cursor = path.parent();
    }
    Some(RelPath::default())
}

/// The scan's visible failure, if it had one.
///
/// Both halves matter: a directory the walk could not read, and entries that
/// were dropped (a lossy-name collision, a parent that is not in the snapshot).
/// The message is one line because the status bar has one line.
fn visible_error(errors: &[String], skipped: usize) -> Option<ProjectError> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(first) = errors.first() {
        if errors.len() == 1 {
            parts.push(format!("unreadable directory: {first}"));
        } else {
            parts.push(format!("{} unreadable directories (first: {first})", errors.len()));
        }
    }
    if skipped > 0 {
        parts.push(format!("{skipped} scan entries skipped"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(ProjectError::new(parts.join("; ")))
    }
}