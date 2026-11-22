//! Project slice: the editor's asset workspace (one root, its file tree and the
//! user's intent over it) plus the request/completion protocol that keeps file
//! system work out of the state tree.
//!
//! The model is M1's: [`ProjectTree`] is a snapshot of the root, [`ProjectView`]
//! is the persistable intent over it, and [`ScanPolicy`] is the data the walk is
//! configured with. What the reducer owns is the protocol: a request gets a
//! generation, the app walks in the background, and the completion is assembled
//! here — or discarded whole when it is stale.
//!
//! Everything in this module is pure data: no `std::fs`, no threads, no egui
//! (see `docs/adr/0008-editor-ui-architecture.md`).
//!
//! Three things stay separate even though they all make a node invisible:
//!
//! * **system dirs** (`Library`): scanned normally, ordered last at the root with
//!   their own node budget ([`ScanPolicy::system_dirs`]);
//! * **ignored** (`.kairosignore`): not in the snapshot at all ([`IgnoreRules`]);
//! * **hidden** (dot files, `.meta` companions): in the snapshot, not drawn by
//!   default ([`ProjectView::show_hidden`]).
//!
//! Requests and completions are named apart on purpose. A `*Requested` action
//! comes from the UI (or from boot); the app answers it with host work; the
//! host's answer is a completion named after what happened (`Scanned`). The UI
//! never fabricates a completion.

mod ignore;
mod scan;
mod tree;
mod view;

pub use ignore::{DEFAULT_IGNORE_LINES, IgnoreRules};
pub use scan::{ScanEntry, ScanPolicy, ScanReason, ScanState};
pub use tree::{
    AssembleReport, Node, NodeId, NodeKind, ProjectTree, RelPath, is_hidden_name,
};
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
    /// The walk's configuration: ignore rules, system directories, budgets.
    ///
    /// Reading `.kairosignore` is the app's IO, but the policy is core's data:
    /// the app sends [`ProjectAction::PolicyLoaded`] and the reducer puts the
    /// value into [`ProjectEffect::Scan`]. Keeping it here means a test can scan
    /// with a one-node budget, and a future project switcher changes data instead
    /// of threading a policy through every call.
    pub policy: ScanPolicy,
    /// Snapshot of the file system, rebuilt by every accepted scan.
    pub tree: ProjectTree,
    /// Persistable user intent: expanded, active directory, selection, sort,
    /// split ratio (M2/M4).
    pub view: ProjectView,
    /// Progress of the scan request/completion protocol.
    pub scan: ScanState,
    /// The most recent visible failure. A failure is state, not a log line: the
    /// status bar renders it, so it cannot disappear the way the legacy tree's
    /// `read_dir` errors did. Written by [`ProjectAction::Scanned`].
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
    /// The app read the project root's `.kairosignore` (and seeded it when it
    /// was missing). Must be applied before the first [`Self::ScanRequested`].
    PolicyLoaded { policy: ScanPolicy },
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
        /// Nodes below the system roots, counted by the walker for the log
        /// line (the tree recomputes it for the status bar; one integer is
        /// cheaper than making the log wait for the reducer).
        library_nodes: usize,
        /// Directories the walk could not read, aggregated by the app (one line
        /// per directory, capped). Empty on a clean scan; anything here is shown
        /// by the status bar rather than only logged.
        errors: Vec<String>,
    },
}

/// Host work for the project slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectEffect {
    /// Walk `root` with `policy` and answer with [`ProjectAction::Scanned`]. The
    /// effect carries the root and the policy because the executor has no access
    /// to the state tree.
    Scan {
        root: PathBuf,
        policy: ScanPolicy,
        generation: u64,
    },
}