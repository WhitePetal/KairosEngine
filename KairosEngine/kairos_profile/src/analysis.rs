//! Derived views over a captured frame.
//!
//! The Profiler panel's Hierarchy tab is the Unity-style aggregate: samples with
//! the same marker (and the same per-invocation data) under the same parent path
//! are merged, totals and self time are accumulated, and children are sorted by
//! total time. This module owns that reduction so the panel stays a renderer and
//! the drawing phase can reuse the exact same numbers.

use crate::model::FrameSnapshot;

/// One row of the aggregated hierarchy.
///
/// Rows are returned in display order: parents before children, siblings sorted
/// by total duration descending. `depth` is the indentation level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HierarchyRow {
    /// Marker id, resolvable through the session's scope table.
    pub scope: u32,
    /// Per-invocation data when every merged sample carried the same value.
    /// Samples with different data are separate rows.
    pub data: Option<String>,
    /// Indentation level; roots are `0`.
    pub depth: u32,
    /// Sum of the durations of every merged sample.
    pub total_ns: u64,
    /// `total_ns` minus the sum of the children's totals; the time spent in this
    /// marker itself. Saturates at zero when threads overlap.
    pub self_ns: u64,
    /// Number of merged samples.
    pub calls: u32,
    /// Longest single sample.
    pub max_ns: u64,
}

/// One aggregate tree node while the frame is being folded.
struct Node {
    scope: u32,
    data: Option<String>,
    total_ns: u64,
    self_ns: u64,
    calls: u32,
    max_ns: u64,
    children: Vec<usize>,
}

/// Aggregate `frame` into display rows.
///
/// Passing `Some(thread_id)` restricts the view to one thread; `None` merges
/// every thread. Parent paths are preserved in both cases, so the same marker
/// reached from different call paths stays on separate rows.
///
/// Multi-thread merging can make children overlap in wall time; `self_ns`
/// saturates rather than reporting a negative value. For one thread it is exact.
pub fn hierarchy(frame: &FrameSnapshot, thread: Option<u32>) -> Vec<HierarchyRow> {
    let mut nodes: Vec<Node> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();

    for thread_snapshot in frame
        .threads
        .iter()
        .filter(|candidate| thread.is_none_or(|id| id == candidate.id))
    {
        // `node_of_sample[i]` is the aggregate node for sample `i`. Parent links
        // always point backwards, so a single forward pass is enough.
        let mut node_of_sample: Vec<Option<usize>> = vec![None; thread_snapshot.samples.len()];

        for (index, sample) in thread_snapshot.samples.iter().enumerate() {
            let parent = sample
                .parent
                .and_then(|parent| node_of_sample.get(parent as usize).copied().flatten());
            let node = find_or_create(&mut nodes, &mut roots, parent, sample.scope, &sample.data);

            let aggregate = &mut nodes[node];
            aggregate.total_ns = aggregate.total_ns.saturating_add(sample.duration_ns);
            aggregate.calls = aggregate.calls.saturating_add(1);
            aggregate.max_ns = aggregate.max_ns.max(sample.duration_ns);
            node_of_sample[index] = Some(node);
        }
    }

    // Self time is computed after every thread has contributed, because a node
    // can gain children from a later thread.
    let totals: Vec<u64> = nodes.iter().map(|node| node.total_ns).collect();
    for node in &mut nodes {
        let child_total: u64 = node
            .children
            .iter()
            .map(|&child| totals[child])
            .fold(0u64, u64::saturating_add);
        node.self_ns = node.total_ns.saturating_sub(child_total);
    }

    // Sort by total duration, then marker id for a deterministic tie-break.
    let sort_keys: Vec<(u64, u32)> = nodes
        .iter()
        .map(|node| (node.total_ns, node.scope))
        .collect();
    roots.sort_by_key(|&root| (std::cmp::Reverse(sort_keys[root].0), sort_keys[root].1));
    for node in &mut nodes {
        node.children
            .sort_by_key(|&child| (std::cmp::Reverse(sort_keys[child].0), sort_keys[child].1));
    }

    let mut rows = Vec::with_capacity(nodes.len());
    // Reverse-push so the first root/child is popped first.
    let mut stack: Vec<(usize, u32)> = roots.iter().rev().map(|&root| (root, 0)).collect();
    while let Some((index, depth)) = stack.pop() {
        let node = &nodes[index];
        rows.push(HierarchyRow {
            scope: node.scope,
            data: node.data.clone(),
            depth,
            total_ns: node.total_ns,
            self_ns: node.self_ns,
            calls: node.calls,
            max_ns: node.max_ns,
        });
        for &child in node.children.iter().rev() {
            stack.push((child, depth + 1));
        }
    }

    rows
}

fn find_or_create(
    nodes: &mut Vec<Node>,
    roots: &mut Vec<usize>,
    parent: Option<usize>,
    scope: u32,
    data: &Option<String>,
) -> usize {
    let siblings: &[usize] = match parent {
        Some(parent) => &nodes[parent].children,
        None => roots,
    };
    if let Some(&existing) = siblings.iter().find(|&&index| {
        nodes[index].scope == scope && nodes[index].data.as_deref() == data.as_deref()
    }) {
        return existing;
    }

    let index = nodes.len();
    nodes.push(Node {
        scope,
        data: data.clone(),
        total_ns: 0,
        self_ns: 0,
        calls: 0,
        max_ns: 0,
        children: Vec::new(),
    });
    match parent {
        Some(parent) => nodes[parent].children.push(index),
        None => roots.push(index),
    }
    index
}
