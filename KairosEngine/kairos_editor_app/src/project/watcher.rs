//! The project tree's watcher: "something under the root changed, rescan".
//!
//! Moved from the legacy editor's `project_watcher.rs` (ADR 0006 deviation 8):
//! the same `notify-debouncer-full` backend and the same 300 ms window as the
//! asset source's watcher, so a change made by another program is reported once,
//! after it has landed.
//!
//! The one thing this version adds is a filter: the backend reports every path
//! under the root, including the ones the walk will never enter. That matters —
//! a `cargo build` touches tens of thousands of files under `target/`, and a
//! rescan per debounce window would keep the UI recalculating a tree that did not
//! change. Events are therefore dropped when their path is ignored by the same
//! rules the walk uses, or when it lives in a system directory (the editor's own
//! logs and caches write there continuously).

#[cfg(test)]
mod test;

use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use kairos_editor_core::project::{IgnoreRules, RelPath, ScanPolicy};
use notify_debouncer_full::{
    DebounceEventResult, Debouncer, RecommendedCache, new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
};

/// The debounce window handed to the backend, matching the asset watcher.
const DEBOUNCE_WAIT: Duration = Duration::from_millis(300);

/// Watches the project root for changes made outside the editor.
///
/// Dropping the watcher stops the watching; the notifications arrive on the
/// bundled channel and are drained by [`Watcher::take_change`].
pub(crate) struct Watcher {
    _backend: Debouncer<RecommendedWatcher, RecommendedCache>,
    changed: Receiver<()>,
}

impl Watcher {
    /// Start watching `root`. A failure degrades to "no automatic refresh" — the
    /// caller logs it and the tree then only follows the editor's own edits.
    pub(crate) fn new(root: &Path, policy: &ScanPolicy) -> Result<Self, String> {
        let (sender, changed) = std::sync::mpsc::channel();
        // Resolve symlinks once so the filter can compare event paths with the
        // watched root. macOS reports FSEvents paths under /private/var while
        // TempDir hands out /var, and Windows canonical paths carry a \\?\
        // prefix; strip_prefix on the unresolved forms would classify every
        // event as "outside the root" and rescan even ignored directories.
        let watched_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let filter_root = watched_root.clone();
        let ignore = policy.ignore.clone();
        let system_dirs = policy.system_dirs.clone();
        let mut backend = new_debouncer(
            DEBOUNCE_WAIT,
            // `None` lets the backend pick its own tick rate (the debounce
            // window divided by four), as the asset watcher does.
            None,
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    // One notification per batch that contains at least one path
                    // worth rescanning. A batch of only ignored changes is not.
                    let worth = events.iter().any(|event| {
                        event
                            .paths
                            .iter()
                            .any(|path| worth_a_rescan(path, &filter_root, &ignore, &system_dirs))
                    });
                    if worth {
                        // `send` cannot block (unbounded) and a failure only means
                        // the watcher was dropped.
                        let _ = sender.send(());
                    }
                }
                Err(errors) => {
                    for error in errors {
                        kairos_log::warn!(target: "kairos_editor", "project watcher: {error}");
                    }
                }
            },
        )
        .map_err(|error| error.to_string())?;
        backend
            .watch(&watched_root, RecursiveMode::Recursive)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            _backend: backend,
            changed,
        })
    }

    /// Drain pending notifications, reporting whether anything under the watched
    /// root changed since the last call.
    pub(crate) fn take_change(&self) -> bool {
        let mut changed = false;
        loop {
            match self.changed.try_recv() {
                Ok(()) => changed = true,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        changed
    }
}

/// Whether an event path is worth a rescan.
///
/// Unfamiliar paths rescan: a change the editor cannot classify is still a change
/// it cannot rule out. Only paths it can prove irrelevant are dropped.
fn worth_a_rescan(path: &Path, root: &Path, ignore: &IgnoreRules, system_dirs: &[String]) -> bool {
    let Some(relative) = relative_to_root(path, root) else {
        return true;
    };
    let relative = relative.to_string_lossy().replace('\\', "/");
    if relative.is_empty() {
        return true;
    }
    let relative = RelPath::new(relative);
    if ignore.is_ignored_path(&relative) {
        return false;
    }
    if is_in_system(&relative, system_dirs) {
        return false;
    }
    true
}

/// The event path relative to the watched root.
///
/// The direct strip is the common case: the root was canonicalized once and the
/// backend reports paths in the same form. The fallback resolves the event path
/// when a backend hands out the watched form for one and a canonical form for
/// the other; if that fails too, the change is conservatively worth a rescan.
fn relative_to_root(path: &Path, root: &Path) -> Option<std::path::PathBuf> {
    if let Ok(relative) = path.strip_prefix(root) {
        return Some(relative.to_path_buf());
    }
    let path = std::fs::canonicalize(path).ok()?;
    path.strip_prefix(root).ok().map(Path::to_path_buf)
}

/// First path segment names a system directory.
fn is_in_system(rel: &RelPath, system_dirs: &[String]) -> bool {
    let first = rel.as_str().split('/').next().unwrap_or_default();
    system_dirs
        .iter()
        .any(|system| system.eq_ignore_ascii_case(first))
}
