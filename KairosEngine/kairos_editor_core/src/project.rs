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

mod file_type;
mod fs;
mod ignore;
mod name;
mod scan;
mod tree;
mod view;

pub use file_type::{CreateEntry, FileType, FileTypeRegistry, extension as file_extension};
pub use fs::{CreateKind, FsOp, FsOpId, FsOpKind, FsOutcome};
pub use ignore::{DEFAULT_IGNORE_LINES, IgnoreRules};
pub use name::{MAX_NAME_BYTES, NameError, collides as name_collides, validate as validate_name};
pub use scan::{ScanEntry, ScanPolicy, ScanReason, ScanState};
pub use tree::{AssembleReport, Node, NodeId, NodeKind, ProjectTree, RelPath};
pub use view::{ProjectView, SelectMode, SortKey};

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
    /// The file-type table: row labels and icons, which names hide, which move
    /// with a sidecar, and what the Create menu offers.
    ///
    /// Data, like `policy`: the M5 seam lives here so a rename's cluster rule and
    /// a delete's confirmation are table lookups rather than branches. A fake
    /// table drives the M5 acceptance test, and nothing outside `core` reads the
    /// built-in constants directly.
    pub file_types: FileTypeRegistry,
    /// Persistable user intent: expanded, active directory, selection, sort,
    /// split ratio (M2/M4).
    pub view: ProjectView,
    /// Progress of the scan request/completion protocol.
    pub scan: ScanState,
    /// Id of the next file operation this slice will ask for.
    ///
    /// The reducer assigns it (the request names never come from the UI), the
    /// app echoes it back on completion, and it is what makes a log line
    /// attributable when the user rattles off several operations.
    pub next_fs_op: FsOpId,
    /// The most recent visible failure. A failure is state, not a log line: the
    /// status bar renders it, so it cannot disappear the way the legacy tree's
    /// `read_dir` errors did. Written by [`ProjectAction::Scanned`] (scan
    /// failures) and [`ProjectAction::Failed`] (a host operation the user
    /// asked for).
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
    /// Whether the next clean scan may clear it.
    ///
    /// Almost every failure may be superseded by a clean scan: the walk's own
    /// verdict, and a one-off host failure such as "cannot open this file" (the
    /// user asked once, and the message must not outlive the reason it was
    /// shown). A **file operation** failure is the exception: its completion
    /// always requests a rescan, so clearing it there would flash the message
    /// for two frames and leave the user without the reason nothing happened.
    /// It is cleared by the next successful operation instead.
    pub clears_on_scan: bool,
}

impl ProjectError {
    /// A one-off host failure (an open that failed): the next clean scan clears
    /// it.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            clears_on_scan: true,
        }
    }

    /// A scan failure: the next clean scan clears it.
    pub fn from_scan(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            clears_on_scan: true,
        }
    }

    /// A failed file operation: survives the rescan its completion asks for.
    pub fn operation(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            clears_on_scan: false,
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
    /// Change the selection.
    ///
    /// `Replace` is the plain click; `Toggle` is Ctrl+click; `Range` is
    /// Shift+click, where `paths` is exactly `[anchor, focus]` and the reducer
    /// resolves the interval over the visible rows (the row order is the tree's,
    /// which is why this stays a core rule and is testable without egui).
    /// Paths the tree does not have are ignored, so a stale click cannot leave a
    /// selection pointing at nothing.
    Select {
        paths: Vec<RelPath>,
        mode: SelectMode,
    },
    /// Expand or collapse one folder: the triangle's half of the tree's
    /// two-hit-area rule (N1). Files are not expandable, and a path that is not
    /// in the tree is ignored.
    ToggleFolder { path: RelPath },
    /// Make `path` the active directory: the double-click on a folder and the
    /// breadcrumb's forward navigation (N1/N6). Sets `active`, expands the path
    /// and its ancestors, and selects it. A file, or a path that is not in the
    /// tree, is ignored.
    Navigate { path: RelPath },
    /// Open `path` with the system default program (M2; M5 replaces it with the
    /// typed `OpenWith` dispatch). A pure request: the reducer changes no state
    /// and answers with [`ProjectEffect::Open`]; a host failure comes back as
    /// [`Self::Failed`].
    OpenRequested { path: RelPath },
    /// Show or hide dot files and `.meta` companions. They are in the tree
    /// either way; this only decides whether the panel draws them.
    SetShowHidden(bool),
    /// Collapse every folder (the toolbar's "collapse all"). `active` and
    /// `selection` survive: the user collapsed rows, not their place in the
    /// project.
    CollapseAll,
    /// Set the left column's fraction of the panel width (the splitter's drag
    /// end). Clamped to `0.15..=0.6`; NaN is ignored so a degenerate drag cannot
    /// poison the layout.
    SetTreeSplit(f32),
    /// A host-side project failure the user should see (M2: opening a file
    /// failed). Writes [`ProjectState::error`]; the status bar renders it.
    Failed { message: String },

    // --- File operations: request -> effect -> completion (M3) ---
    /// Create one entry below `parent` (M3). `name` is the user's text without
    /// the template's extension; the reducer validates, appends the extension,
    /// and answers with [`ProjectEffect::Fs`]. A request that fails validation
    /// answers with [`ProjectState::error`] and no host work.
    CreateRequested {
        parent: RelPath,
        name: String,
        kind: CreateKind,
    },
    /// Rename one entry in place (M3). Only the name changes: moving is
    /// [`Self::MoveRequested`].
    RenameRequested { path: RelPath, name: String },
    /// Move entries below another directory (M3). The reducer de-duplicates the
    /// selection, rejects moving a folder into its own subtree, and leaves a
    /// same-name target to [`Self::Failed`]-style validation: nothing moves.
    MoveRequested {
        paths: Vec<RelPath>,
        target: RelPath,
    },
    /// Move entries to the recycle bin (M3, D5).
    ///
    /// The panel asks for this only after its confirmation dialog said yes (the
    /// dialog is panel state, T7). The reducer repeats the entry checks every
    /// mutation runs — de-duplicating a selection that names a folder and
    /// something inside it, dropping paths the tree no longer has — and never
    /// asks the host to delete the root, which is the project itself.
    DeleteRequested { paths: Vec<RelPath> },
    /// A file operation finished. The outcome says what changed, so the reducer
    /// can reveal the result and rewrite the view's prefixes; a failure is a
    /// visible error, and either way a rescan follows.
    FsCompleted {
        op: FsOpId,
        result: Result<FsOutcome, String>,
    },
    /// Show `path` in the OS file manager (M3).
    RevealRequested { path: RelPath },
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
    /// Open `path` under `root` with the system default program (M2; M5 adds
    /// the IDE dispatch). The effect carries the root because the executor has
    /// no access to the state tree. A host failure is reported as
    /// [`ProjectAction::Failed`].
    Open { root: PathBuf, path: RelPath },
    /// Execute one file operation (M3); the answer is
    /// [`ProjectAction::FsCompleted`]. The effect carries the root for the same
    /// reason `Open` does.
    Fs(FsOp),
    /// Show `path` in the OS file manager (M3).
    Reveal { root: PathBuf, path: RelPath },
}
