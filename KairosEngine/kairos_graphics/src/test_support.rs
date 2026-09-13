//! Helpers shared by the crate's processor tests.

use std::path::PathBuf;

/// A unique directory under the system temp dir, removed first so a rerun starts
/// clean.
pub(crate) fn file_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kairos_graphics_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the temp dir");
    dir
}
