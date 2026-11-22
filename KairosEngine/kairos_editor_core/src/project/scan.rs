//! The scan protocol's data: what the scanner reports, why a scan was
//! requested, how far the current one got, and the policy it walks with.
//!
//! The walk itself lives in `kairos_editor_app::project`: core has no `std::fs`
//! (M1). This module is the contract between the two, and [`ScanPolicy`] is the
//! part of it that is data rather than code: skipping rules, budgets and system
//! directories are values a test can inject and a future "open project" can
//! replace without touching the walk.

use super::ignore::IgnoreRules;
use super::RelPath;

/// One directory entry as the scanner saw it.
///
/// The scanner walks directories only (one `read_dir` per directory); turning
/// this flat, path-sorted list into a tree is `ProjectTree`'s job, which keeps the
/// walk testable as a pure loop and the tree rules as pure functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanEntry {
    pub path: RelPath,
    pub is_dir: bool,
}

/// Why a scan was requested.
///
/// The walk is the same for every reason; the reason keeps the log meaningful
/// and lets the app skip needless work later (a focus-only rescan, a lazy
/// per-directory scan).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanReason {
    /// The editor just opened the project.
    Startup,
    /// The file-system watcher saw a change made outside the editor.
    ExternalChange,
    /// The user asked for it: the toolbar's refresh, or a file operation that
    /// the panel just performed.
    Manual,
}

/// How the scanner walks the project root.
///
/// Every field is data, not a branch: the scanner is a loop over this value, so
/// a test can shrink the budget to one node and still exercise the real code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanPolicy {
    /// Directories that never enter the snapshot; see
    /// [`IgnoreRules`]. Read from the project root's `.kairosignore` by the app.
    pub ignore: IgnoreRules,
    /// Directories that are scanned normally but treated as *system* output:
    /// ordered last at the root (the panel draws a separator before them), exempt
    /// from `ignore`, and given their own node budget. Today only `Library`.
    pub system_dirs: Vec<String>,
    /// The node budget of the system subtree, so build output under `Library`
    /// cannot eat the main walk's budget.
    pub system_dir_budget: usize,
    /// The node budget outside the system subtree. Crossing it stops the walk
    /// and marks the tree truncated.
    pub max_nodes: usize,
    /// How deep the walk descends (the root is depth 0).
    pub max_depth: u32,
    /// Whether a symlink to a directory is followed. Off by default: following
    /// one can loop, and the project root may contain links the editor did not
    /// create.
    pub follow_dir_symlinks: bool,
}

impl ScanPolicy {
    /// Whether the root-level directory `name` is a system directory.
    ///
    /// Case-insensitive on purpose: the check exists to recognise `Library` on
    /// every platform, not to spell it.
    pub fn is_system_dir_name(&self, name: &str) -> bool {
        self.system_dirs
            .iter()
            .any(|system| system.eq_ignore_ascii_case(name))
    }
}

impl Default for ScanPolicy {
    fn default() -> Self {
        Self {
            ignore: IgnoreRules::defaults(),
            system_dirs: vec![String::from("Library")],
            system_dir_budget: 20_000,
            max_nodes: 200_000,
            max_depth: 64,
            follow_dir_symlinks: false,
        }
    }
}

/// Progress of the scan request/completion protocol.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanState {
    /// Id of the newest request. A completion with an older id is discarded, so
    /// an in-flight scan cannot overwrite the result of a newer one. This is a
    /// *request* id, not the tree's content revision (`ProjectTree::revision`).
    pub generation: u64,
    /// True from the moment the request is applied until its completion lands.
    pub running: bool,
    /// Wall time of the last completed scan, for the status bar and the log.
    pub last_ms: u64,
    /// The last scan hit a node/depth/system-dir budget and stopped early.
    pub truncated: bool,
    /// Path to reveal once the result lands (after a rename or a create).
    pub focus_after_scan: Option<RelPath>,
}