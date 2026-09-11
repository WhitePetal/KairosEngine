//! The embedded asset backend: assets compiled into the binary.
//!
//! bevy lets `#[embedded_asset!]` macros and build scripts bake assets into the
//! executable and read them back through a `embedded://` source. Kairos has no
//! such macros yet, so the registry is deliberately empty: the types exist so
//! the source has a home, and a read always reports
//! [`NotFound`](AssetReaderError::NotFound).

use std::path::Path;

use crate::io::{
    AssetReader, AssetReaderError, PathStream, Reader, VecReader, empty_path_stream,
};

/// The registry of assets compiled into the binary.
///
/// Nothing registers embedded assets yet, so this carries no entries; it exists
/// so the `embedded://` source has a home and can grow a registration API when
/// embedding lands.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmbeddedAssetRegistry;

impl EmbeddedAssetRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self
    }
}

/// Reads from the embedded asset registry, which is empty.
///
/// Every read misses with [`AssetReaderError::NotFound`]; directory reads
/// return an empty stream.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmbeddedAssetReader;

impl EmbeddedAssetReader {
    /// Creates a reader over the (empty) embedded registry.
    pub fn new() -> Self {
        Self
    }
}

impl AssetReader for EmbeddedAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        Err::<VecReader, _>(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        Err::<VecReader, _>(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        _path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        Ok(empty_path_stream())
    }

    async fn is_directory<'a>(&'a self, _path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(false)
    }
}
