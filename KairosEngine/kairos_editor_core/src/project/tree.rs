//! The tree's identity types.
//!
//! M0 carries only the generation stamp: the node table, `assemble` and the row
//! projection land in M1. The public shape of [`ProjectTree`] does not change
//! then — [`RelPath`] stays the tree's only identity (D4: the editor's GUID
//! registry is not replaced; identity is the relative path).

use std::fmt;

/// A node's path relative to the project root, always `/`-separated; the root
/// itself is the empty string.
///
/// This is the tree's identity: the same file gets the same `RelPath` before
/// and after a rescan, so selection/expansion survive a rebuild without a GUID.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath(String);

impl RelPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this is the project root (the empty path).
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for RelPath {
    fn from(path: &str) -> Self {
        Self::new(path)
    }
}

impl From<String> for RelPath {
    fn from(path: String) -> Self {
        Self(path)
    }
}

/// A snapshot of the project's file system at one point in time.
///
/// M0 carries the generation counter only: panels cache rows derived from the
/// tree and invalidate them when it changes. M1 adds the node table and
/// `assemble`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectTree {
    generation: u64,
}

impl ProjectTree {
    /// Incremented every time a scan result is assembled into the tree.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Record one rebuild. M0 completes every scan into an empty tree, but the
    /// stamp still moves so a cache cannot survive a changed result; M1 replaces
    /// this with `assemble(entries)`, which bumps it after building the nodes.
    pub(crate) fn assembled(&mut self) {
        self.generation += 1;
    }
}
