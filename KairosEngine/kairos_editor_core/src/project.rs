//! Project slice: the editor's asset workspace (one root, its file tree and the
//! user's intent over it) plus the request/completion protocol that keeps file
//! system work out of the state tree.
//!
//! This is the slice M0 of `docs/plan/project-panel` establishes. The tree
//! algorithms, the background scanner and the watcher land in M1; what exists
//! here is the data the state tree owns ([`ProjectState`]), the actions the UI and
//! the host feed it ([`ProjectAction`]), and the effects that ask the app to
//! touch the file system ([`ProjectEffect`]).
//!
//! Everything in this module is pure data: no `std::fs`, no threads, no egui
//! (see `docs/adr/0008-editor-ui-architecture.md`).
//!
//! Three things stay separate even though they all make a node invisible:
//!
//! * **system dirs** (`Library`): scanned normally, ordered last at the root,
//!   own node budget (M1's `ScanPolicy`);
//! * **ignored** (`.kairosignore`): not in the snapshot at all (M1);
//! * **hidden** (dot files, `.meta` companions): in the snapshot, not drawn by
//!   default (`ProjectView::show_hidden`, M2/M5).
//!
//! Requests and completions are named apart on purpose. A `*Requested` action
//! comes from the UI (or from boot); the app answers it with host work; the
//! host's answer is a completion named after what happened (`Scanned`). The UI
//! never fabricates a completion.

mod scan;
mod tree;
mod view;

pub use scan::{ScanEntry, ScanReason, ScanState};
pub use tree::{ProjectTree, RelPath};
pub use view::{ProjectView, SortKey};

use std::path::PathBuf;

/// The project slice of [`EditorState`](crate::EditorState).
///
/// `root` is the only absolute path the slice stores: every other path is a
/// [`RelPath`] below it, so "open another project" is a data change rather than
/// a rewrite of the tree code (D9 in `docs/plan/project-panel`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectState {
    /// Panel title / top bar. Derived from `root`'s last component at boot; it
    /// replaces the old `EditorState::project_name`.
    pub name: String,
    /// Absolute project root. Comes from the app's IO boundary (the working
    /// directory today, a project chooser later); core never calls `current_dir`.
    pub root: PathBuf,
    /// Snapshot of the file system (M1 fills the node table).
    pub tree: ProjectTree,
    /// Persistable user intent: expanded, active directory, selection, sort,
    /// split ratio (M2/M4).
    pub view: ProjectView,
    /// Progress of the scan request/completion protocol.
    pub scan: ScanState,
    /// The most recent visible failure. A failure is state, not a log line:
    /// the status bar renders it, so it cannot disappear the way the legacy
    /// tree's `read_dir` errors did.
    pub error: Option<ProjectError>,
}

impl ProjectState {
    /// The slice for a project rooted at `root`.
    ///
    /// `root` is absolute (the app's IO boundary supplies it); the last path
    /// component names the project for the panel title. An empty root — a
    /// failed `current_dir` at boot — falls back to `"project"`.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let name = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| String::from("project"));
        Self {
            name,
            root,
            ..Self::default()
        }
    }
}

/// A visible project failure (the last one wins).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectError {
    /// Message shown by the panel's status bar. The app formats it; core only
    /// carries it.
    pub message: String,
}

impl ProjectError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Intents the project panel and the app's host work produce.
#[derive(Clone, Debug, PartialEq)]
pub enum ProjectAction {
    /// Ask the host to scan the project. The generation is assigned by the
    /// reducer, not by the caller, so two requests cannot share one id.
    ScanRequested {
        reason: ScanReason,
        /// Path to reveal (and expand to) once the result lands, e.g. after a
        /// rename or a create. `None` leaves a reveal that is still pending
        /// alone: a watcher rescan must not cancel the path a rename asked to
        /// show.
        focus: Option<RelPath>,
    },
    /// A scan finished. A completion whose generation is not the current one
    /// is stale and is discarded whole — a slow scan must never overwrite a
    /// newer tree.
    Scanned {
        generation: u64,
        entries: Vec<ScanEntry>,
        truncated: bool,
        elapsed_ms: u64,
    },
}

/// Host work for the project slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectEffect {
    /// Walk `root` and answer with [`ProjectAction::Scanned`]. The effect carries
    /// the root because the executor has no access to the state tree.
    Scan { root: PathBuf, generation: u64 },
}
