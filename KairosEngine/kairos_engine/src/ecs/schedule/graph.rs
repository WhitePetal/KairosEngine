mod dag;
mod graph_map;
mod tarjan_scc;

pub use dag::*;
pub use graph_map::{DiGraph, DiGraphToposortError, Direction, GraphNodeId, UnGraph};

/// Specifies what kind of edge should be added to the dependency graph.
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub(crate) enum DependencyKind {
    /// A node that should be preceded.
    Before,
    /// A node that should be succeeded.
    After,
}

/// Converts 2D row-major pair of indices into a 1D array index.
pub(crate) fn index(row: usize, col: usize, num_cols: usize) -> usize {
    debug_assert!(col < num_cols);
    (row * num_cols) + col
}

/// Converts a 1D array index into a 2D row-major pair of indices.
pub(crate) fn row_col(index: usize, num_cols: usize) -> (usize, usize) {
    (index / num_cols, index % num_cols)
}
