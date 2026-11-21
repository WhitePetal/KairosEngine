//! The scan protocol's data: what the scanner reports, why a scan was
//! requested, and how far the current one got.
//!
//! The walk itself lives in `kairos_editor_app::project`: core has no
//! `std::fs` (M1). This module is the contract between the two.

use super::RelPath;

/// One directory entry as the scanner saw it.
///
/// The scanner walks directories only (one `read_dir` per directory); turning
/// this flat, path-sorted list into a tree is `ProjectTree`'s job (M1), which
/// keeps the walk testable as a pure loop and the tree rules as pure functions.
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

/// Progress of the scan request/completion protocol.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanState {
    /// Id of the newest request. A completion with an older id is discarded, so
    /// an in-flight scan cannot overwrite the result of a newer one.
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
