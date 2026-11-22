//! Project slice reducer: the request/completion protocol around scanning, the
//! view intents over the tree, and the file-operation protocol.
//!
//! The reducer never walks the file system and never renames a file. It assigns
//! the next generation to a scan or the next [`FsOpId`] to a file operation, marks
//! the work requested, and returns the host work; the app answers either with
//! [`ProjectAction::Scanned`] or [`ProjectAction::FsCompleted`] and this is where
//! the answer becomes state.
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
//!
//! Every mutation request runs the same entry checks before it becomes host
//! work: the [`name table`](crate::project::validate_name), sibling collisions,
//! moving a folder into its own subtree, and de-duplicating a selection that
//! names both a folder and something inside it. A failed check is a visible
//! error ([`ProjectState::error`]) and **no** effect — the file system is never
//! asked to do something the editor already knows is wrong.

use std::collections::BTreeSet;

use crate::project::{
    CreateKind, FsOp, FsOpId, FsOpKind, FsOutcome, Node, ProjectError, ProjectState, ProjectTree,
    ProjectView, RelPath, ScanReason, ScanState, SelectMode, name_collides, validate_name,
};
use crate::{EditorState, Effect, ProjectAction, ProjectEffect};

pub(super) fn reduce(state: &mut EditorState, action: ProjectAction) -> Vec<Effect> {
    match action {
        ProjectAction::PolicyLoaded { policy } => {
            state.project.policy = policy;
            Vec::new()
        }
        ProjectAction::ScanRequested { reason, focus } => {
            request_scan(&mut state.project, reason, focus)
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

            // A clean scan clears a *scan* error only. A host failure (a rename
            // the OS refused) must survive the rescan its own completion asks
            // for, or the message flashes for two frames.
            project.error = match visible_error(&errors, report.skipped) {
                Some(error) => Some(error),
                None => project.error.take().filter(|error| !error.clears_on_scan),
            };
            Vec::new()
        }
        ProjectAction::Select { paths, mode } => {
            select(&mut state.project, &paths, mode);
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
                project.view.selection.insert(path.clone());
                project.view.anchor = Some(path);
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
        ProjectAction::CreateRequested { parent, name, kind } => {
            create_requested(&mut state.project, parent, &name, kind)
        }
        ProjectAction::RenameRequested { path, name } => {
            rename_requested(&mut state.project, path, &name)
        }
        ProjectAction::MoveRequested { paths, target } => {
            move_requested(&mut state.project, &paths, target)
        }
        ProjectAction::DeleteRequested { paths } => delete_requested(&mut state.project, &paths),
        ProjectAction::FsCompleted { op, result } => fs_completed(&mut state.project, op, result),
        ProjectAction::RevealRequested { path } => {
            let project = &state.project;
            if project.tree.find(&path).is_none() {
                return Vec::new();
            }
            vec![Effect::Project(ProjectEffect::Reveal {
                root: project.root.clone(),
                path,
            })]
        }
    }
}

// ---------------------------------------------------------------------------
// scan protocol
// ---------------------------------------------------------------------------

/// Ask for a walk: assign the generation, mark it running, remember what to
/// reveal, and return the host work.
///
/// The reason is not part of the effect (the walk is identical); it exists so
/// the app's action log says *why* a scan happened, and so a future lazy scan
/// can branch on it.
fn request_scan(
    project: &mut ProjectState,
    _reason: ScanReason,
    focus: Option<RelPath>,
) -> Vec<Effect> {
    project.scan.generation = project.scan.generation.wrapping_add(1);
    project.scan.running = true;
    // `None` keeps a reveal that is still pending: a watcher rescan must not
    // cancel the path a rename just asked to show. The completion that lands on
    // it reveals the path and clears it.
    if focus.is_some() {
        project.scan.focus_after_scan = focus;
    }
    vec![Effect::Project(ProjectEffect::Scan {
        root: project.root.clone(),
        policy: project.policy.clone(),
        generation: project.scan.generation,
    })]
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
    // The anchor is a row of the selection, so it follows the same pruning. A
    // deleted or externally renamed anchor falls back to what is still selected:
    // the next Shift+click has to extend from a row that exists.
    if view
        .anchor
        .as_ref()
        .is_some_and(|anchor| !view.selection.contains(anchor))
    {
        view.anchor = view.selection.iter().next().cloned();
    }

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
        // The reveal must not eat a selection the gesture already rewrote. A
        // rename/move remaps the selection before the scan lands, so the focused
        // path is part of it and a multi-entry move stays multi-selected; only a
        // create (whose row cannot have been selected yet) replaces the selection.
        if !view.selection.contains(&focus) {
            view.selection.clear();
            view.selection.insert(focus.clone());
        }
        view.anchor = Some(focus.clone());
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
            parts.push(format!(
                "{} unreadable directories (first: {first})",
                errors.len()
            ));
        }
    }
    if skipped > 0 {
        parts.push(format!("{skipped} scan entries skipped"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(ProjectError::from_scan(parts.join("; ")))
    }
}

// ---------------------------------------------------------------------------
// selection
// ---------------------------------------------------------------------------

/// Apply one of the three desktop selection gestures.
///
/// Every gesture leaves [`ProjectView::anchor`] pointing at the row it was made
/// from: a plain click re-anchors on that row, a toggle re-anchors on the row it
/// kept, and a range keeps the anchor it was given so a second Shift+click still
/// extends from the same row.
fn select(project: &mut ProjectState, paths: &[RelPath], mode: SelectMode) {
    match mode {
        SelectMode::Replace => {
            // A multi-path Replace (Ctrl+A) is a selection gesture, not a click:
            // it keeps the anchor it already has while that row is still part of
            // the new selection, so Shift+click still extends from where the
            // user last clicked.
            let previous_anchor = project.view.anchor.clone();
            project.view.selection.clear();
            let mut last = None;
            for path in paths {
                if project.tree.find(path).is_some() {
                    project.view.selection.insert(path.clone());
                    last = Some(path.clone());
                }
            }
            let selection_len = project.view.selection.len();
            let first_selected = project.view.selection.iter().next().cloned();
            project.view.anchor = match (selection_len, last.clone()) {
                (1, Some(only)) => Some(only),
                _ => previous_anchor
                    .filter(|anchor| project.view.selection.contains(anchor))
                    .or(first_selected),
            };
            // Unity's browsing rule: the content pane follows what was picked
            // last (a folder shows itself, a file shows its parent). A
            // multi-path Replace (Ctrl+A) is a selection gesture, not a
            // navigation: it leaves the content pane where it was.
            if project.view.selection.len() == 1
                && let Some(directory) = last
                    .as_ref()
                    .and_then(|path| project.tree.content_dir(path))
            {
                project.view.active = Some(directory);
            }
        }
        SelectMode::Toggle => {
            // Extending a selection must not move the content pane: the user is
            // building a set, not navigating.
            let mut last = None;
            for path in paths {
                if project.tree.find(path).is_none() {
                    continue;
                }
                if !project.view.selection.remove(path) {
                    project.view.selection.insert(path.clone());
                }
                last = Some(path.clone());
            }
            // Anchoring on a row the toggle just removed would leave the next
            // Shift+click anchored nowhere: fall back to the surviving anchor,
            // then to the first selected row.
            project.view.anchor = last
                .filter(|path| project.view.selection.contains(path))
                .or_else(|| {
                    project
                        .view
                        .anchor
                        .clone()
                        .filter(|path| project.view.selection.contains(path))
                })
                .or_else(|| project.view.selection.iter().next().cloned());
        }
        SelectMode::Range => {
            // `paths` is exactly [anchor, focus]. The interval is over the rows
            // the user can see, which is why the row order lives in core.
            let Some(anchor) = paths.first() else {
                return;
            };
            let focus = paths.get(1).unwrap_or(anchor);
            let rows = project
                .tree
                .visible_rows(&project.view, &project.file_types);
            let mut from = None;
            let mut to = None;
            for (index, id) in rows.iter().enumerate() {
                let Some(node) = project.tree.node(*id) else {
                    continue;
                };
                if &node.path == anchor {
                    from = Some(index);
                }
                if &node.path == focus {
                    to = Some(index);
                }
            }
            // A focus that is not on screen is stale UI input (its row changed
            // between the click and the action): leave the selection alone.
            let Some(to) = to else {
                return;
            };
            // The anchor can be hidden without being gone: the user collapsed
            // the folder it lived in. The gesture is still a selection, so it
            // selects the clicked row and re-anchors there instead of doing
            // nothing at all.
            let Some(from) = from else {
                select(project, std::slice::from_ref(focus), SelectMode::Replace);
                return;
            };
            let (low, high) = if from <= to { (from, to) } else { (to, from) };
            project.view.selection.clear();
            for id in &rows[low..=high] {
                if let Some(node) = project.tree.node(*id) {
                    project.view.selection.insert(node.path.clone());
                }
            }
            // A range extends from its anchor and does not move it: repeated
            // Shift+clicks keep growing from the same row.
            project.view.anchor = Some(anchor.clone());
            if let Some(directory) = project.tree.content_dir(focus) {
                project.view.active = Some(directory);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// file operations
// ---------------------------------------------------------------------------

/// Remember one host failure and change nothing else.
fn fail(project: &mut ProjectState, message: impl Into<String>) -> Vec<Effect> {
    project.error = Some(ProjectError::new(message));
    Vec::new()
}

/// Turn a validated operation into host work.
fn push_fs(project: &mut ProjectState, kind: FsOpKind) -> Vec<Effect> {
    let id = project.next_fs_op;
    project.next_fs_op = id.next();
    vec![Effect::Project(ProjectEffect::Fs(FsOp {
        id,
        root: project.root.clone(),
        companions: project.file_types.companion_extensions().collect(),
        kind,
    }))]
}

/// The label the error messages use for a directory: the project name for the
/// root, the relative path otherwise.
fn directory_label(project: &ProjectState, directory: &RelPath) -> String {
    if directory.is_root() {
        project.name.clone()
    } else {
        directory.to_string()
    }
}

/// Whether `parent` has a child named `name`, case-insensitively, ignoring the
/// entry being renamed.
fn has_sister(tree: &ProjectTree, parent: &RelPath, name: &str, except: Option<&str>) -> bool {
    let Some(parent_id) = tree.find(parent) else {
        return false;
    };
    let sisters = tree
        .children(parent_id)
        .iter()
        .filter_map(|id| tree.node(*id))
        .map(Node::name);
    name_collides(name, sisters, except)
}

/// The entry name a create should use: the template's extension is appended
/// unless the user already typed it.
///
/// The panel passes the default stem ("New Document"), which has no extension;
/// a user who types "notes.md" must not be given "notes.md.md".
fn with_extension(name: &str, extension: &str) -> String {
    let suffix = format!(".{extension}");
    if name.len() >= suffix.len()
        && name.as_bytes()[name.len() - suffix.len()..].eq_ignore_ascii_case(suffix.as_bytes())
    {
        name.to_owned()
    } else {
        format!("{name}{suffix}")
    }
}

fn create_requested(
    project: &mut ProjectState,
    parent: RelPath,
    name: &str,
    kind: CreateKind,
) -> Vec<Effect> {
    // Stale UI input (the panel's active directory vanished) is dropped, not
    // reported: it is not intent the user can act on.
    if !project.tree.get(&parent).is_some_and(Node::is_folder) {
        return Vec::new();
    }
    let full_name = match kind {
        CreateKind::Folder => name.to_owned(),
        CreateKind::File { extension, .. } => with_extension(name, extension),
    };
    if let Err(error) = validate_name(&full_name) {
        return fail(project, error.message());
    }
    if has_sister(&project.tree, &parent, &full_name, None) {
        return fail(
            project,
            format!(
                "\"{full_name}\" already exists in {}",
                directory_label(project, &parent)
            ),
        );
    }
    push_fs(
        project,
        FsOpKind::Create {
            parent,
            name: full_name,
            kind,
        },
    )
}

fn rename_requested(project: &mut ProjectState, path: RelPath, name: &str) -> Vec<Effect> {
    let Some(node) = project.tree.get(&path) else {
        return Vec::new();
    };
    // The root is the window, not a row: it has no sister set and no row to
    // edit, so a rename of it is stale input rather than intent.
    if path.is_root() {
        return Vec::new();
    }
    if let Err(error) = validate_name(name) {
        return fail(project, error.message());
    }
    if name == node.name() {
        // Renaming to the name it already has is a no-op, not an error: the
        // inline editor is pre-filled, so Enter on an untouched name lands here.
        return Vec::new();
    }
    let Some(parent) = path.parent() else {
        return Vec::new();
    };
    // Case-only renames are allowed: `Node::name` is excluded from the sister
    // set, so `READ.png` -> `read.png` is not a collision with itself.
    if has_sister(&project.tree, &parent, name, Some(node.name())) {
        return fail(
            project,
            format!(
                "\"{name}\" already exists in {}",
                directory_label(project, &parent)
            ),
        );
    }
    push_fs(
        project,
        FsOpKind::Rename {
            from: path,
            to: parent.child(name),
        },
    )
}

fn move_requested(project: &mut ProjectState, paths: &[RelPath], target: RelPath) -> Vec<Effect> {
    if !project.tree.get(&target).is_some_and(Node::is_folder) {
        return Vec::new();
    }
    let selected = top_level(&project.tree, paths);
    if selected.is_empty() {
        return Vec::new();
    }
    // Moving a folder into its own subtree would destroy it: the OS reports
    // either an error or (on some platforms) succeeds halfway, and either way
    // the result is not what the user can have meant.
    for path in &selected {
        if *path == target || target.is_under(path) {
            return fail(
                project,
                format!("\"{}\" cannot be moved into itself", path.name()),
            );
        }
    }
    // Dropping every entry into the directory it already lives in is a no-op,
    // not a rescan request.
    if selected
        .iter()
        .all(|path| path.parent().as_ref() == Some(&target))
    {
        return Vec::new();
    }
    // The panel pre-checks this and shows a blocking dialog (T7); the reducer
    // repeats it because a stale snapshot must never let a move overwrite.
    for path in &selected {
        if has_sister(&project.tree, &target, path.name(), None) {
            return fail(
                project,
                format!(
                    "\"{}\" already exists in {}",
                    path.name(),
                    directory_label(project, &target)
                ),
            );
        }
    }
    push_fs(
        project,
        FsOpKind::Move {
            from: selected,
            to: target,
        },
    )
}

/// Ask the host to move the selection to the recycle bin.
///
/// By the time this runs the panel has already asked the user (D5's
/// confirmation), so there is nothing left to validate beyond what the tree can
/// answer: which of the named paths are still there, and which of them are
/// already inside another one. The file system — not this function — is the
/// only thing that knows whether the platform's trash works, so its failure
/// comes back through [`ProjectAction::FsCompleted`] as a visible error.
fn delete_requested(project: &mut ProjectState, paths: &[RelPath]) -> Vec<Effect> {
    // The root is the project itself: deleting it would leave the panel with no
    // project to show. It is not a row, so this is stale input rather than
    // intent, and is dropped like a path the tree no longer has.
    let paths: Vec<RelPath> = paths
        .iter()
        .filter(|path| !path.is_root())
        .cloned()
        .collect();
    let selected = top_level(&project.tree, &paths);
    if selected.is_empty() {
        return Vec::new();
    }
    push_fs(project, FsOpKind::Trash { paths: selected })
}

/// Drop the entries of a selection that another entry already contains, and any
/// path that is no longer in the tree.
///
/// Selecting a folder and a file inside it and moving both would move the file
/// twice (or race with itself); the folder is the operation the user means.
fn top_level(tree: &ProjectTree, paths: &[RelPath]) -> Vec<RelPath> {
    let mut kept: Vec<RelPath> = Vec::new();
    for path in paths {
        if tree.find(path).is_none() || kept.contains(path) {
            continue;
        }
        if kept.iter().any(|kept| path.is_under(kept)) {
            continue;
        }
        // A later path may be an ancestor of one already kept.
        kept.retain(|kept| !kept.is_under(path));
        kept.push(path.clone());
    }
    kept
}

/// Apply the result of one host operation.
fn fs_completed(
    project: &mut ProjectState,
    op: FsOpId,
    result: Result<FsOutcome, String>,
) -> Vec<Effect> {
    let _ = op;
    match result {
        Ok(outcome) => {
            // A successful host operation is the only thing that clears a host
            // error; a clean scan must not (its completion always follows).
            project.error = None;
            for (from, to) in &outcome.moved {
                rewrite_prefixes(&mut project.view, from, to);
            }
            request_scan(project, ScanReason::Manual, outcome.focus())
        }
        Err(message) => {
            project.error = Some(ProjectError::operation(message));
            // Rescan even on failure: a move of several entries can fail
            // halfway, and the tree must show what is actually on disk.
            request_scan(project, ScanReason::Manual, None)
        }
    }
}

/// Rewrite the view's prefixes after a rename so a renamed folder stays
/// expanded and its selection survives.
///
/// Without this the follow-up scan prunes the old paths and the folder the user
/// just renamed collapses — the "renaming a directory should not throw away
/// where you were" half of the M3 acceptance.
fn rewrite_prefixes(view: &mut ProjectView, from: &RelPath, to: &RelPath) {
    fn remap(path: &RelPath, from: &RelPath, to: &RelPath) -> Option<RelPath> {
        if path == from {
            return Some(to.clone());
        }
        if path.is_under(from) {
            return Some(to.child(&path.as_str()[from.as_str().len() + 1..]));
        }
        None
    }

    fn remap_set(set: &mut BTreeSet<RelPath>, from: &RelPath, to: &RelPath) {
        if !set.iter().any(|path| path == from || path.is_under(from)) {
            return;
        }
        *set = set
            .iter()
            .map(|path| remap(path, from, to).unwrap_or_else(|| path.clone()))
            .collect();
    }

    remap_set(&mut view.expanded, from, to);
    remap_set(&mut view.selection, from, to);
    if let Some(anchor) = &view.anchor
        && let Some(rewritten) = remap(anchor, from, to)
    {
        view.anchor = Some(rewritten);
    }
    if let Some(active) = &view.active
        && let Some(rewritten) = remap(active, from, to)
    {
        view.active = Some(rewritten);
    }
}
