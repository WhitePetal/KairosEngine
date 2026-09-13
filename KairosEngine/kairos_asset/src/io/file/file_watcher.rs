//! The filesystem watcher: a port of `bevy_asset` 0.19.1's
//! `io/file/file_watcher.rs`.
//!
//! [`FileWatcher`] wraps [`notify_debouncer_full`] and turns raw OS filesystem
//! events into [`AssetSourceEvent`]s. Debouncing is delegated to the backend's
//! built-in 300 ms window (configured by the caller), so the bevy layer itself
//! only suppresses *adjacent duplicate* events within one debounced batch.
//!
//! Unlike upstream, a path that cannot be made relative to the watched root, and
//! a failed send to a torn-down event channel, are logged and dropped rather than
//! panicking: watching must degrade, not abort the process.

use core::time::Duration;
use std::path::{Path, PathBuf};

use async_channel::Sender;
use notify_debouncer_full::{
    DebounceEventResult, Debouncer, RecommendedCache, new_debouncer,
    notify::{
        self,
        event::{AccessKind, AccessMode, CreateKind, ModifyKind, RemoveKind, RenameMode},
        RecommendedWatcher, RecursiveMode,
    },
};
use tracing::warn;

use crate::io::{AssetSourceEvent, AssetWatcher};
use crate::path::normalize_path;

/// An [`AssetWatcher`] that watches the filesystem for changes to asset files
/// under a given root and emits an [`AssetSourceEvent`] for each relevant change.
///
/// This uses [`notify_debouncer_full`] to retrieve "debounced" events.
/// "Debouncing" holds events for a time window and removes duplicates that fall
/// inside it. That introduces a small delay, but it reduces event duplicates and
/// avoids processing a change event before the change has actually landed.
pub struct FileWatcher {
    _watcher: Debouncer<RecommendedWatcher, RecommendedCache>,
}

impl FileWatcher {
    /// Creates a [`FileWatcher`] that watches for changes to the asset files
    /// under `path`, forwarding them to `sender`. `debounce_wait_time` is the
    /// debounce window handed to the backend.
    pub fn new(
        path: PathBuf,
        sender: Sender<AssetSourceEvent>,
        debounce_wait_time: Duration,
    ) -> Result<Self, notify::Error> {
        let root = make_absolute_path(&path)?;
        let watcher = new_asset_event_debouncer(
            path,
            debounce_wait_time,
            FileEventHandler {
                root,
                sender,
                last_event: None,
            },
        )?;
        Ok(FileWatcher { _watcher: watcher })
    }
}

impl AssetWatcher for FileWatcher {}

/// Converts `path` into an absolute, normalized path.
///
/// `normalize` + `absolute` is used instead of `canonicalize` so the path is
/// resolved without reading the filesystem. That also means a path that no
/// longer exists (for example the "old" side of a rename) can still become
/// absolute.
fn make_absolute_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    Ok(normalize_path(&std::path::absolute(path)?))
}

/// Maps an absolute path back onto a source-relative asset path.
///
/// Returns the asset path and whether it names a `.meta` sidecar. The sidecar's
/// `.meta` suffix is stripped so both a source file and its sidecar address the
/// same asset. Returns [`None`] when the path does not sit under `root`, which
/// can only happen for an event the backend reported outside the watched tree.
pub(crate) fn get_asset_path(root: &Path, absolute_path: &Path) -> Option<(PathBuf, bool)> {
    let relative_path = match absolute_path.strip_prefix(root) {
        Ok(relative_path) => relative_path,
        Err(_) => {
            warn!(
                "FileWatcher dropped a change outside its watched root: absolute_path={}, root={}",
                absolute_path.display(),
                root.display()
            );
            return None;
        }
    };
    let is_meta = relative_path
        .extension()
        .is_some_and(|extension| extension == "meta");
    let asset_path = if is_meta {
        relative_path.with_extension("")
    } else {
        relative_path.to_owned()
    };
    Some((asset_path, is_meta))
}

/// Builds a debouncer over the platform-recommended backend, turning its events
/// into [`AssetSourceEvent`]s through `handler`.
///
/// This abstracts the event-management logic so each filesystem-driven
/// [`AssetWatcher`] implementation does not repeat it: every operating system
/// behaves a little differently and this balancing act should only be performed
/// once.
pub(crate) fn new_asset_event_debouncer(
    root: PathBuf,
    debounce_wait_time: Duration,
    mut handler: impl FilesystemEventHandler,
) -> Result<Debouncer<RecommendedWatcher, RecommendedCache>, notify::Error> {
    let root = super::get_base_path().join(root);
    let mut debouncer = new_debouncer(
        debounce_wait_time,
        // `None` lets the backend pick its tick rate, which is the debounce
        // window divided by four.
        None,
        move |result: DebounceEventResult| match result {
            Ok(events) => {
                handler.begin();
                for event in events.iter() {
                    // Make every path absolute here so individual handlers do
                    // not have to.
                    let paths = event
                        .paths
                        .iter()
                        .map(PathBuf::as_path)
                        .filter_map(|path| make_absolute_path(path).ok())
                        .collect::<Vec<_>>();
                    if paths.is_empty() {
                        continue;
                    }

                    match event.kind {
                        notify::EventKind::Create(CreateKind::File) => {
                            if let Some((path, is_meta)) = handler.get_path(&paths[0]) {
                                if is_meta {
                                    handler.handle(&paths, AssetSourceEvent::AddedMeta(path));
                                } else {
                                    handler.handle(&paths, AssetSourceEvent::AddedAsset(path));
                                }
                            }
                        }
                        notify::EventKind::Create(CreateKind::Folder) => {
                            if let Some((path, _)) = handler.get_path(&paths[0]) {
                                handler.handle(&paths, AssetSourceEvent::AddedFolder(path));
                            }
                        }
                        notify::EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
                            if let Some((path, is_meta)) = handler.get_path(&paths[0]) {
                                if is_meta {
                                    handler.handle(&paths, AssetSourceEvent::ModifiedMeta(path));
                                } else {
                                    handler.handle(&paths, AssetSourceEvent::ModifiedAsset(path));
                                }
                            }
                        }
                        // Because this is debounced over a reasonable period of
                        // time, a `RenameMode::From` event is assumed to be
                        // "dangling" without a follow-up "To" event. Without
                        // debouncing, "From" -> "To" -> "Both" events are emitted
                        // for renames. A dangling "From" is treated as a removal.
                        notify::EventKind::Remove(RemoveKind::Any)
                        | notify::EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                            if let Some((path, is_meta)) = handler.get_path(&paths[0]) {
                                handler.handle(
                                    &paths,
                                    AssetSourceEvent::RemovedUnknown { path, is_meta },
                                );
                            }
                        }
                        notify::EventKind::Create(CreateKind::Any)
                        | notify::EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
                            if let Some((path, is_meta)) = handler.get_path(&paths[0]) {
                                let asset_event = if paths[0].is_dir() {
                                    AssetSourceEvent::AddedFolder(path)
                                } else if is_meta {
                                    AssetSourceEvent::AddedMeta(path)
                                } else {
                                    AssetSourceEvent::AddedAsset(path)
                                };
                                handler.handle(&paths, asset_event);
                            }
                        }
                        notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                            let Some((old_path, old_is_meta)) = handler.get_path(&paths[0]) else {
                                continue;
                            };
                            let Some((new_path, new_is_meta)) =
                                paths.get(1).and_then(|path| handler.get_path(path))
                            else {
                                continue;
                            };
                            // Only the new ("real") path can be a directory.
                            if paths[1].is_dir() {
                                handler.handle(
                                    &paths,
                                    AssetSourceEvent::RenamedFolder {
                                        old: old_path,
                                        new: new_path,
                                    },
                                );
                            } else {
                                match (old_is_meta, new_is_meta) {
                                    (true, true) => {
                                        handler.handle(
                                            &paths,
                                            AssetSourceEvent::RenamedMeta {
                                                old: old_path,
                                                new: new_path,
                                            },
                                        );
                                    }
                                    (false, false) => {
                                        handler.handle(
                                            &paths,
                                            AssetSourceEvent::RenamedAsset {
                                                old: old_path,
                                                new: new_path,
                                            },
                                        );
                                    }
                                    (true, false) => {
                                        warn!(
                                            "Asset metafile {old_path:?} was changed to asset file \
                                             {new_path:?}, which is not supported. Try restarting \
                                             your app to see if configuration is still valid"
                                        );
                                    }
                                    (false, true) => {
                                        warn!(
                                            "Asset file {old_path:?} was changed to meta file \
                                             {new_path:?}, which is not supported. Try restarting \
                                             your app to see if configuration is still valid"
                                        );
                                    }
                                }
                            }
                        }
                        notify::EventKind::Modify(_) => {
                            let Some((path, is_meta)) = handler.get_path(&paths[0]) else {
                                continue;
                            };
                            if paths[0].is_dir() {
                                // A modified folder means nothing here.
                            } else if is_meta {
                                handler.handle(&paths, AssetSourceEvent::ModifiedMeta(path));
                            } else {
                                handler.handle(&paths, AssetSourceEvent::ModifiedAsset(path));
                            }
                        }
                        notify::EventKind::Remove(RemoveKind::File) => {
                            let Some((path, is_meta)) = handler.get_path(&paths[0]) else {
                                continue;
                            };
                            if is_meta {
                                handler.handle(&paths, AssetSourceEvent::RemovedMeta(path));
                            } else {
                                handler.handle(&paths, AssetSourceEvent::RemovedAsset(path));
                            }
                        }
                        notify::EventKind::Remove(RemoveKind::Folder) => {
                            let Some((path, _)) = handler.get_path(&paths[0]) else {
                                continue;
                            };
                            handler.handle(&paths, AssetSourceEvent::RemovedFolder(path));
                        }
                        _ => {}
                    }
                }
            }
            Err(errors) => errors
                .iter()
                .for_each(|error| warn!("Encountered a filesystem watcher error {error:?}")),
        },
    )?;
    debouncer.watch(&root, RecursiveMode::Recursive)?;
    Ok(debouncer)
}

/// The handler that forwards filesystem events to the asset server's channel.
pub(crate) struct FileEventHandler {
    sender: Sender<AssetSourceEvent>,
    root: PathBuf,
    last_event: Option<AssetSourceEvent>,
}

impl FilesystemEventHandler for FileEventHandler {
    fn begin(&mut self) {
        self.last_event = None;
    }

    fn get_path(&self, absolute_path: &Path) -> Option<(PathBuf, bool)> {
        get_asset_path(&self.root, absolute_path)
    }

    fn handle(&mut self, _absolute_paths: &[PathBuf], event: AssetSourceEvent) {
        // Adjacent duplicate suppression within one debounced batch.
        if self.last_event.as_ref() != Some(&event) {
            self.last_event = Some(event.clone());
            if self.sender.send_blocking(event).is_err() {
                // The receiver is gone: the source is being torn down. Nothing
                // to do but stop forwarding.
                warn!("FileWatcher event channel closed; stopping event forwarding");
            }
        }
    }
}

/// The event-management seam shared by filesystem-driven [`AssetWatcher`]s.
pub(crate) trait FilesystemEventHandler: Send + Sync + 'static {
    /// Called once before each debounced batch of events.
    fn begin(&mut self);
    /// Returns the source-relative asset path for `absolute_path`, if it lives
    /// under the watched root, together with whether it names a `.meta` sidecar.
    fn get_path(&self, absolute_path: &Path) -> Option<(PathBuf, bool)>;
    /// Handles one `event`.
    fn handle(&mut self, absolute_paths: &[PathBuf], event: AssetSourceEvent);
}
