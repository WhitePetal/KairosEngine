//! The `embedded_watcher` backend: hot-reload for embedded assets.
//!
//! A port of `bevy_asset` 0.19.1's `io/embedded/embedded_watcher.rs`. The
//! watcher watches the asset base path (`get_base_path`), maps changed absolute
//! paths back to asset paths through the [`EmbeddedAssetRegistry`]'s recorded
//! `root_paths`, reads the changed file from disk, and overwrites the
//! compiled-in bytes in the in-memory [`Dir`]. Only an
//! [`AssetSourceEvent::ModifiedAsset`] triggers the byte overwrite, but — as
//! upstream — every event that maps back through `root_paths` is forwarded to
//! the source's channel. Embedded metadata is not hot-reloaded (a change to a
//! `.meta` sidecar is only warned about).
//!
//! [`EmbeddedAssetRegistry`]: super::EmbeddedAssetRegistry

use core::time::Duration;
use std::{
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::{Arc, PoisonError, RwLock},
};

use async_channel::Sender;
use kairos_collections::FixedHashMap as HashMap;
use notify_debouncer_full::{Debouncer, RecommendedCache, notify::RecommendedWatcher};
use tracing::warn;

use crate::io::file::file_watcher::{
    FilesystemEventHandler, get_asset_path, new_asset_event_debouncer,
};
use crate::io::file::get_base_path;
use crate::io::memory::Dir;
use crate::io::{AssetSourceEvent, AssetWatcher};

/// A watcher for assets stored in the `embedded` asset source.
///
/// Embedded assets are assets whose bytes have been embedded into the Rust binary
/// by the [`embedded_asset!`](crate::embedded_asset) macro. This watcher watches
/// the "source files", reads the contents of changed files from the filesystem,
/// and overwrites the initial static bytes embedded in the binary with the new
/// dynamically loaded bytes.
pub struct EmbeddedWatcher {
    _watcher: Debouncer<RecommendedWatcher, RecommendedCache>,
}

impl EmbeddedWatcher {
    /// Creates a new [`EmbeddedWatcher`] that watches for changes to the embedded
    /// assets in `dir`.
    ///
    /// Returns [`None`] when the underlying watcher cannot be created (for
    /// example, the base path is missing): watching is best-effort and must
    /// degrade rather than abort the process.
    pub fn new(
        dir: Dir,
        root_paths: Arc<RwLock<HashMap<Box<Path>, PathBuf>>>,
        sender: Sender<AssetSourceEvent>,
        debounce_wait_time: Duration,
    ) -> Option<Self> {
        let root = get_base_path();
        let handler = EmbeddedEventHandler {
            dir,
            root: root.clone(),
            sender,
            root_paths,
            last_event: None,
        };
        let watcher = new_asset_event_debouncer(root, debounce_wait_time, handler).ok()?;
        Some(Self { _watcher: watcher })
    }
}

impl AssetWatcher for EmbeddedWatcher {}

/// A [`FilesystemEventHandler`] that uses the
/// [`EmbeddedAssetRegistry`](super::EmbeddedAssetRegistry) to hot-reload
/// binary-embedded Rust source files.
///
/// This reads the contents of changed files from the filesystem and overwrites
/// the initial static bytes from the file embedded in the binary.
pub(crate) struct EmbeddedEventHandler {
    sender: Sender<AssetSourceEvent>,
    root_paths: Arc<RwLock<HashMap<Box<Path>, PathBuf>>>,
    root: PathBuf,
    dir: Dir,
    last_event: Option<AssetSourceEvent>,
}

impl FilesystemEventHandler for EmbeddedEventHandler {
    fn begin(&mut self) {
        self.last_event = None;
    }

    fn get_path(&self, absolute_path: &Path) -> Option<(PathBuf, bool)> {
        let (local_path, is_meta) = get_asset_path(&self.root, absolute_path)?;
        let final_path = self
            .root_paths
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(local_path.as_path())?
            .clone();
        if is_meta {
            warn!("Meta file asset hot-reloading is not supported yet: {final_path:?}");
        }
        Some((final_path, false))
    }

    fn handle(&mut self, absolute_paths: &[PathBuf], event: AssetSourceEvent) {
        if self.last_event.as_ref() != Some(&event) {
            if let AssetSourceEvent::ModifiedAsset(path) = &event
                && let Ok(file) = File::open(&absolute_paths[0])
            {
                let mut reader = BufReader::new(file);
                let mut buffer = Vec::new();

                // Read the file into the buffer and overwrite the static bytes.
                if reader.read_to_end(&mut buffer).is_ok() {
                    self.dir.insert_asset(path, buffer);
                }
            }
            self.last_event = Some(event.clone());
            if self.sender.send_blocking(event).is_err() {
                // The receiver is gone: the source is being torn down. Nothing to
                // do but stop forwarding. (Upstream unwraps here; like
                // `FileEventHandler`, watching degrades rather than aborts.)
                warn!("EmbeddedWatcher event channel closed; stopping event forwarding");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EmbeddedEventHandler;
    use crate::io::AssetSourceEvent;
    use crate::io::file::file_watcher::FilesystemEventHandler;
    use crate::io::memory::Dir;
    use kairos_collections::FixedHashMap as HashMap;
    use std::{
        path::{Path, PathBuf},
        sync::{Arc, RwLock},
    };

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kairos_asset_embedded_watcher_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The handler reads the changed file from disk, overwrites the in-memory
    /// bytes, maps the absolute path back to the asset path through
    /// `root_paths`, and forwards a `ModifiedAsset` event.
    #[test]
    fn handler_overwrites_bytes_and_forwards_modified() {
        let root = temp_dir("overwrite");
        let asset_path = "my_crate/thing.bytes";
        let watched = root.join("src").join("thing.bytes");
        std::fs::create_dir_all(watched.parent().unwrap()).unwrap();
        std::fs::write(&watched, b"two, longer").unwrap();

        let dir = Dir::default();
        dir.insert_asset(Path::new(asset_path), b"one".to_vec());

        let mut root_paths = HashMap::default();
        root_paths.insert(
            PathBuf::from("src").join("thing.bytes").into_boxed_path(),
            PathBuf::from(asset_path),
        );

        let (sender, receiver) = async_channel::unbounded();
        let mut handler = EmbeddedEventHandler {
            sender,
            root_paths: Arc::new(RwLock::new(root_paths)),
            root: root.clone(),
            dir: dir.clone(),
            last_event: None,
        };

        handler.begin();
        handler.handle(
            std::slice::from_ref(&watched),
            AssetSourceEvent::ModifiedAsset(PathBuf::from(asset_path)),
        );

        assert_eq!(
            dir.get_asset(Path::new(asset_path)).unwrap().value(),
            b"two, longer"
        );
        assert_eq!(
            receiver.try_recv().unwrap(),
            AssetSourceEvent::ModifiedAsset(PathBuf::from(asset_path))
        );
    }

    /// A meta-file change is mapped back to the asset path and warned about, but
    /// still reported as a modification of the asset (never as meta).
    #[test]
    fn handler_maps_meta_changes_to_the_asset() {
        let root = temp_dir("meta");
        let asset_path = "my_crate/thing.bytes";
        let watched = root.join("src").join("thing.bytes");

        let dir = Dir::default();
        dir.insert_asset(Path::new(asset_path), b"one".to_vec());

        let mut root_paths = HashMap::default();
        root_paths.insert(
            PathBuf::from("src").join("thing.bytes").into_boxed_path(),
            PathBuf::from(asset_path),
        );

        let (sender, receiver) = async_channel::unbounded();
        let handler = EmbeddedEventHandler {
            sender,
            root_paths: Arc::new(RwLock::new(root_paths)),
            root: root.clone(),
            dir,
            last_event: None,
        };

        let path = handler
            .get_path(&watched.with_extension("bytes.meta"))
            .unwrap();
        assert_eq!(path, (PathBuf::from(asset_path), false));
        let _ = receiver;
    }

    /// Upstream forwards every event that maps back through `root_paths`, even
    /// though only `ModifiedAsset` overwrites the compiled-in bytes.
    #[test]
    fn handler_forwards_non_modification_events_without_overwriting_bytes() {
        let root = temp_dir("forward");
        let asset_path = "my_crate/thing.bytes";
        let watched = root.join("src").join("thing.bytes");
        std::fs::create_dir_all(watched.parent().unwrap()).unwrap();
        std::fs::write(&watched, b"on disk").unwrap();

        let dir = Dir::default();
        dir.insert_asset(Path::new(asset_path), b"compiled".to_vec());

        let mut root_paths = HashMap::default();
        root_paths.insert(
            PathBuf::from("src").join("thing.bytes").into_boxed_path(),
            PathBuf::from(asset_path),
        );

        let (sender, receiver) = async_channel::unbounded();
        let mut handler = EmbeddedEventHandler {
            sender,
            root_paths: Arc::new(RwLock::new(root_paths)),
            root: root.clone(),
            dir: dir.clone(),
            last_event: None,
        };

        handler.begin();
        handler.handle(
            std::slice::from_ref(&watched),
            AssetSourceEvent::AddedAsset(PathBuf::from(asset_path)),
        );

        // The event is forwarded...
        assert_eq!(
            receiver.try_recv().unwrap(),
            AssetSourceEvent::AddedAsset(PathBuf::from(asset_path))
        );
        // ...but the compiled-in bytes are untouched; only `ModifiedAsset`
        // rereads the file.
        assert_eq!(
            dir.get_asset(Path::new(asset_path)).unwrap().value(),
            b"compiled"
        );
    }
}
