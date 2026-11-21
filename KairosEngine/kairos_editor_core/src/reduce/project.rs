//! Project slice reducer: the request/completion protocol around scanning.
//!
//! The reducer never walks the file system. It assigns the next generation,
//! marks the scan running and returns the host work; the app answers with
//! [`ProjectAction::Scanned`], which is discarded when it belongs to an older
//! generation. Everything a scan finds is assembled into the tree here (M1),
//! so the app's scanner stays a loop that only reads directories.

use crate::{EditorState, Effect, ProjectAction, ProjectEffect};

pub(super) fn reduce(state: &mut EditorState, action: ProjectAction) -> Vec<Effect> {
    match action {
        ProjectAction::ScanRequested { reason: _, focus } => {
            let project = &mut state.project;
            project.scan.generation = project.scan.generation.wrapping_add(1);
            project.scan.running = true;
            // `None` keeps a reveal that is still pending: a watcher rescan
            // must not cancel the path a rename just asked to show. The
            // completion that lands on it reveals the path and clears it (M1).
            if focus.is_some() {
                project.scan.focus_after_scan = focus;
            }
            vec![Effect::Project(ProjectEffect::Scan {
                root: project.root.clone(),
                generation: project.scan.generation,
            })]
        }
        ProjectAction::Scanned {
            generation,
            entries: _,
            truncated,
            elapsed_ms,
        } => {
            let project = &mut state.project;
            if !project.scan.running || generation != project.scan.generation {
                // Either an older scan finishing late (it says nothing about the
                // tree the newer request will produce) or a duplicate completion
                // for an idle scan. Both are dropped whole, never merged.
                return Vec::new();
            }
            // M1: assemble `entries` into the tree and prune `expanded` /
            // `selection` to paths that still exist. M0 records the completion
            // itself so the protocol's bookkeeping is already real.
            project.tree.assembled();
            project.scan.running = false;
            project.scan.last_ms = elapsed_ms;
            project.scan.truncated = truncated;
            Vec::new()
        }
    }
}
