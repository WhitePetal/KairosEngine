//! Host side of the project slice: everything about the project that touches the
//! file system.
//!
//! `EditorState::project` owns the data; this service owns the work. The app
//! hands it every `ProjectEffect` and, in the host phase of each frame
//! (`EditorApp::begin_frame`), takes back what completed as actions — which the
//! app applies through the same reducer that handles the UI's own actions.
//!
//! M0 is the seam, not the scanner: `execute` completes a scan request
//! synchronously with an empty result, so request → effect → completion → state
//! runs end to end before the panel exists. The app side is the shape M1 needs;
//! M1 only replaces this file's body with the background scanner (results arrive
//! through `poll`) and adds the watcher.
//!
//! Nothing here touches `EditorState`: the host owns the file system and the
//! threads, the reducer owns the state. That split is the reason a scan cannot
//! leave the tree half-updated.

use std::time::Instant;

use kairos_editor_core::{Action, ProjectAction, ProjectEffect};

/// Owns the project's host work: the scan thread and the watcher (M1), the file
/// operations (M3). In M0 it is the seam those land behind.
pub(crate) struct ProjectService;

impl ProjectService {
    pub(crate) fn new() -> Self {
        Self
    }

    /// Host work that completed since the last frame, as actions.
    ///
    /// M0 has no background work, so this is always empty. M1's scanner thread
    /// and watcher push their completions into a channel that this drains, which
    /// is why the seam is a `poll` and not a callback: the app pulls at a known
    /// point in the frame instead of mutating state from another thread.
    pub(crate) fn poll(&mut self, _now: Instant) -> Vec<Action> {
        Vec::new()
    }

    /// Execute one effect, returning the actions it produced.
    ///
    /// M0 answers a scan request synchronously with an empty tree — the same
    /// `Scanned` completion M1's scanner sends, without the thread. The action
    /// travels the normal inbox path and is applied at the next frame's host
    /// phase, like any other completion.
    pub(crate) fn execute(&mut self, effect: ProjectEffect, _now: Instant) -> Vec<Action> {
        match effect {
            ProjectEffect::Scan { root, generation } => {
                kairos_log::debug!(
                    target: "kairos_editor",
                    "project scan #{generation}: {} (M0 stub, empty result)",
                    root.display()
                );
                vec![Action::Project(ProjectAction::Scanned {
                    generation,
                    entries: Vec::new(),
                    truncated: false,
                    elapsed_ms: 0,
                })]
            }
        }
    }
}
