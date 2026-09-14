//! The editor's own watcher over the project source root.
//!
//! ADR 0006 deviation 8: the project tree gets its own watcher rather than
//! riding the asset sources' `AssetSourceEvent` stream, which stays internal to
//! `kairos_asset`. The two answer different questions — the source watcher
//! reloads *loaded assets*, this one tells the *directory listing* that it is
//! stale — so only the model is shared, not the event flow.
//!
//! The watcher is deliberately coarse: it reports "something under the project
//! root changed" once per debounce window, and the tree answers with a rescan.
//! The tree is a directory listing, not a set of assets, so there is no event
//! mapping worth preserving; a full rescan is also what already handles the
//! editor's own create / rename / delete.
//!
//! The backend and the debounce window match the asset watcher (ADR 0006
//! deviation 7: `notify-debouncer-full`, 300 ms, hard-coded), so a file another
//! program writes is reported once, after it has landed.

use std::path::Path;
use std::time::Duration;

use crossbeam_channel::{Receiver, TryRecvError};
use notify_debouncer_full::{
    DebounceEventResult, Debouncer, RecommendedCache, new_debouncer,
    notify::{Error, RecommendedWatcher, RecursiveMode},
};

#[cfg(test)]
mod test;

/// The debounce window handed to the backend, matching the asset source's
/// watcher (ADR 0006 deviation 7).
const DEBOUNCE_WAIT: Duration = Duration::from_millis(300);

/// Watches the project root for changes made outside the editor.
///
/// Dropping the watcher stops the watching; the notifications themselves arrive
/// on the bundled channel and are drained by
/// [`take_change`](ProjectTreeWatcher::take_change).
pub struct ProjectTreeWatcher {
    /// The backend handle. It is never read, but dropping it stops the watch.
    _watcher: Debouncer<RecommendedWatcher, RecommendedCache>,
    changed: Receiver<()>,
}

impl ProjectTreeWatcher {
    /// Starts watching `root` and everything below it.
    ///
    /// # Errors
    ///
    /// If the backend could not be created or `root` could not be watched. The
    /// caller degrades to a tree that only follows the editor's own edits.
    pub fn new(root: &Path) -> Result<Self, Error> {
        let (sender, changed) = crossbeam_channel::unbounded();
        let mut watcher = new_debouncer(
            DEBOUNCE_WAIT,
            // `None` lets the backend pick its tick rate (the debounce window
            // divided by four), as the asset watcher does.
            None,
            move |result: DebounceEventResult| match result {
                // One notification per debounced batch. An empty batch changes
                // nothing, so it is not worth a rescan.
                Ok(events) => {
                    if !events.is_empty() {
                        // `try_send` cannot block the backend's thread, and the
                        // channel is unbounded; a failure only means the tree
                        // (and with it this receiver) is gone.
                        let _ = sender.try_send(());
                    }
                }
                // Watching must degrade, not abort the editor: a backend error
                // costs the tree its automatic refresh and nothing more.
                Err(errors) => {
                    for error in errors {
                        log::warn!("project tree watcher: {error}");
                    }
                }
            },
        )?;
        watcher.watch(root, RecursiveMode::Recursive)?;
        Ok(Self {
            _watcher: watcher,
            changed,
        })
    }

    /// Drains the pending notifications, reporting whether anything under the
    /// watched root changed since the last call.
    pub fn take_change(&self) -> bool {
        let mut changed = false;
        loop {
            match self.changed.try_recv() {
                Ok(()) => changed = true,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return changed,
            }
        }
    }
}
