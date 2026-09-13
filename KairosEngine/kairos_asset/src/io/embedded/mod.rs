//! The embedded asset backend: assets compiled into the binary.
//!
//! The [`embedded_asset!`](crate::embedded_asset) macro bakes a file's bytes into
//! the executable and registers them with an [`EmbeddedAssetRegistry`], which
//! backs the `embedded://` [`AssetSource`](crate::io::AssetSource). Loads read
//! through an in-memory [`Dir`](crate::io::memory::Dir), so nothing touches the
//! filesystem at runtime.
//!
//! With the `embedded_watcher` feature the registry also remembers where each
//! asset came from on disk ([`EmbeddedWatcher`]), so editing the source file
//! replaces the compiled-in bytes and hot-reloads the asset.
//!
//! This is a port of `bevy_asset` 0.19.1's `io/embedded/mod.rs`, except that the
//! macros take a `&mut World` rather than an `App` (ADR 0003, ADR 0006).

#[cfg(feature = "embedded_watcher")]
mod embedded_watcher;

#[cfg(feature = "embedded_watcher")]
pub use embedded_watcher::EmbeddedWatcher;

use std::path::{Path, PathBuf};

#[cfg(feature = "embedded_watcher")]
use std::sync::{Arc, PoisonError, RwLock};

#[cfg(feature = "embedded_watcher")]
use kairos_collections::FixedHashMap as HashMap;
use kairos_ecs::resource::Resource;
use kairos_ecs::world::World;

use crate::io::memory::{Dir, MemoryAssetReader, Value};
use crate::io::{AssetSourceBuilder, AssetSourceBuilders};
use crate::server::AssetServer;

/// The name of the `embedded` [`AssetSource`](crate::io::AssetSource), as stored
/// in the [`AssetSourceBuilders`] resource.
pub const EMBEDDED: &str = "embedded";

/// A [`Resource`] that manages "rust source files" in a virtual in-memory
/// [`Dir`], which is shared with the [`MemoryAssetReader`] the `embedded` source
/// reads through.
///
/// Generally this should not be interacted with directly: the
/// [`embedded_asset!`](crate::embedded_asset) macro populates it.
#[derive(Resource, Clone, Default)]
pub struct EmbeddedAssetRegistry {
    /// The in-memory filesystem the `embedded` source reads through. Exposed to
    /// the crate so tests can build a reader over it.
    pub(crate) dir: Dir,
    #[cfg(feature = "embedded_watcher")]
    root_paths: Arc<RwLock<HashMap<Box<Path>, PathBuf>>>,
}

impl EmbeddedAssetRegistry {
    /// Inserts a new asset.
    ///
    /// `full_path` is the path the watcher sees (as `file!` would return for that
    /// file, if it could run outside a Rust build). `asset_path` is the path that
    /// identifies the asset in the `embedded` source. `value` is the bytes
    /// returned for the asset: either a `&'static [u8]` or a `Vec<u8>`.
    pub fn insert_asset(&self, full_path: PathBuf, asset_path: &Path, value: impl Into<Value>) {
        self.insert_asset_internal(full_path, asset_path, value.into());
    }

    // Implements `insert_asset`, but with a non-generic `value` parameter. This
    // stops the function from being duplicated many times by monomorphization.
    #[cfg_attr(
        not(feature = "embedded_watcher"),
        expect(
            unused_variables,
            reason = "`full_path` is unused when `embedded_watcher` is disabled"
        )
    )]
    fn insert_asset_internal(&self, full_path: PathBuf, asset_path: &Path, value: Value) {
        #[cfg(feature = "embedded_watcher")]
        self.root_paths
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(full_path.into(), asset_path.to_owned());
        self.dir.insert_asset(asset_path, value);
    }

    /// Inserts new asset metadata.
    ///
    /// `full_path` is the path the watcher sees, and `asset_path` is the path that
    /// identifies the asset in the `embedded` source. `value` is either a
    /// `&'static [u8]` or a `Vec<u8>`.
    #[cfg_attr(
        not(feature = "embedded_watcher"),
        expect(
            unused_variables,
            reason = "`full_path` is unused when `embedded_watcher` is disabled"
        )
    )]
    pub fn insert_meta(&self, full_path: &Path, asset_path: &Path, value: impl Into<Value>) {
        #[cfg(feature = "embedded_watcher")]
        self.root_paths
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(full_path.into(), asset_path.to_owned());
        self.dir.insert_meta(asset_path, value.into());
    }

    /// Removes the asset stored at `asset_path` (the same path passed as
    /// `asset_path` to [`insert_asset`](Self::insert_asset)).
    ///
    /// Returns the removed [`Data`](crate::io::memory::Data), or [`None`] if no
    /// asset was stored there.
    pub fn remove_asset(&self, asset_path: &Path) -> Option<crate::io::memory::Data> {
        self.dir.remove_asset(asset_path)
    }

    /// Registers the [`EMBEDDED`] [`AssetSource`](crate::io::AssetSource) with
    /// the given [`AssetSourceBuilders`].
    pub fn register_source(&self, sources: &mut AssetSourceBuilders) {
        let dir = self.dir.clone();
        let processed_dir = self.dir.clone();

        #[cfg_attr(
            not(feature = "embedded_watcher"),
            expect(
                unused_mut,
                reason = "`source` is only reassigned when `embedded_watcher` is enabled"
            )
        )]
        let mut source =
            AssetSourceBuilder::new(move || Box::new(MemoryAssetReader { root: dir.clone() }))
                .with_processed_reader(move || {
                    Box::new(MemoryAssetReader {
                        root: processed_dir.clone(),
                    })
                })
                // Only a processed watch warning: warning noisily about embedded
                // watching (which is niche) when a host enables file watching
                // would be more confusing than helpful.
                .with_processed_watch_warning(
                    "Consider enabling the `embedded_watcher` cargo feature.",
                );

        #[cfg(feature = "embedded_watcher")]
        {
            let root_paths = self.root_paths.clone();
            let dir = self.dir.clone();
            let processed_root_paths = self.root_paths.clone();
            let processed_dir = self.dir.clone();
            source = source
                .with_watcher(move |sender| {
                    EmbeddedWatcher::new(
                        dir.clone(),
                        root_paths.clone(),
                        sender,
                        std::time::Duration::from_millis(300),
                    )
                    .map(|watcher| Box::new(watcher) as Box<dyn crate::io::AssetWatcher>)
                })
                .with_processed_watcher(move |sender| {
                    EmbeddedWatcher::new(
                        processed_dir.clone(),
                        processed_root_paths.clone(),
                        sender,
                        std::time::Duration::from_millis(300),
                    )
                    .map(|watcher| Box::new(watcher) as Box<dyn crate::io::AssetWatcher>)
                });
        }
        sources.insert(EMBEDDED, source);
    }
}

/// Trait for the [`load_embedded_asset!`] macro, to access an [`AssetServer`]
/// from arbitrary things.
///
/// [`load_embedded_asset!`]: crate::load_embedded_asset
pub trait GetAssetServer {
    /// The [`AssetServer`] this value can reach.
    fn get_asset_server(&self) -> &AssetServer;
}

impl GetAssetServer for World {
    fn get_asset_server(&self) -> &AssetServer {
        self.resource()
    }
}

impl GetAssetServer for AssetServer {
    fn get_asset_server(&self) -> &AssetServer {
        self
    }
}

/// Returns the [`Path`] for a given `embedded` asset.
///
/// Used internally by [`embedded_asset!`](crate::embedded_asset) and to get a
/// [`Path`] that matches the [`AssetPath`](crate::AssetPath) used by that asset.
#[macro_export]
macro_rules! embedded_path {
    ($path_str: expr) => {{
        $crate::embedded_path!("src", $path_str)
    }};

    ($source_path: expr, $path_str: expr) => {{
        let crate_name = module_path!().split(':').next().unwrap();
        $crate::io::embedded::_embedded_asset_path(
            crate_name,
            $source_path.as_ref(),
            file!().as_ref(),
            $path_str.as_ref(),
        )
    }};
}

/// Implementation detail of `embedded_path`, do not use this!
///
/// Returns an embedded asset path, given:
///   - `crate_name`: name of the crate where the asset is embedded
///   - `src_prefix`: path prefix of the crate's source directory, relative to the
///     workspace root
///   - `file_path`: `std::file!()` path of the source file where `embedded_path!`
///     is called
///   - `asset_path`: path of the embedded asset relative to `file_path`
#[doc(hidden)]
pub fn _embedded_asset_path(
    crate_name: &str,
    src_prefix: &Path,
    file_path: &Path,
    asset_path: &Path,
) -> PathBuf {
    let file_path = if cfg!(not(target_family = "windows")) {
        // Work around https://github.com/bevyengine/bevy/issues/14246. This breaks
        // any paths on Linux/macOS containing "\", which are already broken there.
        PathBuf::from(file_path.to_str().unwrap().replace('\\', "/"))
    } else {
        PathBuf::from(file_path)
    };
    let mut maybe_parent = file_path.parent();
    let after_src = loop {
        let Some(parent) = maybe_parent else {
            panic!("Failed to find src_prefix {src_prefix:?} in {file_path:?}")
        };
        if parent.ends_with(src_prefix) {
            break file_path.strip_prefix(parent).unwrap();
        }
        maybe_parent = parent.parent();
    };
    let asset_path = after_src.parent().unwrap().join(asset_path);
    Path::new(crate_name).join(asset_path)
}

/// Returns the path the watcher uses for `asset_path`.
#[doc(hidden)]
#[cfg(feature = "embedded_watcher")]
pub fn watched_path(source_file_path: &'static str, asset_path: &'static str) -> PathBuf {
    PathBuf::from(source_file_path)
        .parent()
        .unwrap()
        .join(asset_path)
}

/// Returns an empty [`PathBuf`]; nothing watches embedded assets without the
/// `embedded_watcher` feature.
#[doc(hidden)]
#[cfg(not(feature = "embedded_watcher"))]
pub fn watched_path(_source_file_path: &'static str, _asset_path: &'static str) -> PathBuf {
    PathBuf::from("")
}

/// Loads an [`embedded`](crate::embedded_asset) asset by path.
///
/// Useful when the embedded asset is not publicly exposed but must be used
/// internally.
///
/// # Syntax
///
/// This macro takes two arguments and an optional third:
/// 1. The asset source: an [`AssetServer`] or a `&mut World`.
/// 2. The path to the asset to embed, as a string literal.
/// 3. Optionally, a closure of the same type as in
///    [`LoadBuilder::with_settings`](crate::LoadBuilder::with_settings).
///
/// The advantage over [`AssetServer::load`] is that it accepts a `World` too, and
/// uses the same path as [`embedded_asset!`](crate::embedded_asset), so the two
/// stay consistent.
#[macro_export]
macro_rules! load_embedded_asset {
    (@get: $path: literal, $provider: expr) => {{
        let path = $crate::embedded_path!($path);
        let path = $crate::AssetPath::from_path_buf(path).with_source("embedded");
        let asset_server = $crate::io::embedded::GetAssetServer::get_asset_server($provider);
        (path, asset_server)
    }};
    ($provider: expr, $path: literal, $settings: expr) => {{
        let (path, asset_server) = $crate::load_embedded_asset!(@get: $path, $provider);
        asset_server.load_builder().with_settings($settings).load(path)
    }};
    ($provider: expr, $path: literal) => {{
        let (path, asset_server) = $crate::load_embedded_asset!(@get: $path, $provider);
        asset_server.load(path)
    }};
}

/// Creates a new `embedded` asset by embedding the bytes of `path` into the
/// current binary and registering them with the `embedded`
/// [`AssetSource`](crate::io::AssetSource).
///
/// This accepts a `&mut World` as the first argument and a path `&str` (relative
/// to the current file) as the second. By default the generated
/// [`AssetPath`](crate::AssetPath) trims everything up to and including the first
/// `$crate_name/src/` in `file!()`, then re-adds the crate name: a file
/// `bevy_rock/src/render/rock.wgsl` embedded from `bevy_rock/src/render/mod.rs`
/// is loadable as `embedded://bevy_rock/render/rock.wgsl`.
///
/// For crate layouts without a `src` directory (cargo examples), pass the source
/// prefix explicitly as the second argument:
/// `embedded_asset!(world, "/examples/rock_stuff/", "rock.wgsl")`.
///
/// This macro uses [`include_bytes`] internally and does not reallocate the
/// bytes. Hot-reloading `embedded` assets is supported when the
/// `embedded_watcher` cargo feature is enabled.
#[macro_export]
macro_rules! embedded_asset {
    ($world: expr, $path: expr) => {{
        $crate::embedded_asset!($world, "src", $path)
    }};

    ($world: expr, $source_path: expr, $path: expr) => {{
        let embedded = $world
            .resource_mut::<$crate::io::embedded::EmbeddedAssetRegistry>();
        let path = $crate::embedded_path!($source_path, $path);
        let watched_path = $crate::io::embedded::watched_path(file!(), $path);
        embedded.insert_asset(watched_path, &path, include_bytes!($path));
    }};
}

/// Loads an "internal" asset by embedding the string stored in `path_str` and
/// associating it with `handle`.
#[macro_export]
macro_rules! load_internal_asset {
    ($world: ident, $handle: expr, $path_str: expr, $loader: expr) => {{
        let mut assets = $world.resource_mut::<$crate::Assets<_>>();
        assets.insert($handle.id(), ($loader)(
            include_str!($path_str),
            std::path::Path::new(file!())
                .parent()
                .unwrap()
                .join($path_str)
                .to_string_lossy()
        )).unwrap();
    }};
    // We cannot support params without variadic arguments, so internal assets
    // with additional params cannot be hot-reloaded.
    ($world: ident, $handle: ident, $path_str: expr, $loader: expr $(, $param:expr)+) => {{
        let mut assets = $world.resource_mut::<$crate::Assets<_>>();
        assets.insert($handle.id(), ($loader)(
            include_str!($path_str),
            std::path::Path::new(file!())
                .parent()
                .unwrap()
                .join($path_str)
                .to_string_lossy(),
            $($param),+
        )).unwrap();
    }};
}

/// Loads an "internal" binary asset by embedding the bytes stored in `path_str`
/// and associating them with `handle`.
#[macro_export]
macro_rules! load_internal_binary_asset {
    ($world: ident, $handle: expr, $path_str: expr, $loader: expr) => {{
        let mut assets = $world.resource_mut::<$crate::Assets<_>>();
        assets
            .insert(
                $handle.id(),
                ($loader)(
                    include_bytes!($path_str).as_ref(),
                    std::path::Path::new(file!())
                        .parent()
                        .unwrap()
                        .join($path_str)
                        .to_string_lossy()
                        .into(),
                ),
            )
            .unwrap();
    }};
}
