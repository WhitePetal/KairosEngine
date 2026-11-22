//! The project tree: a snapshot of the file system, identified by relative
//! paths only.
//!
//! A scan hands [`ProjectTree::assemble`] a flat, path-sorted list of entries and
//! the product is a node table plus a path index. Two decisions shape everything
//! here:
//!
//! * **`RelPath` is the identity** (D4): a rescan rebuilds every `NodeId`, but the
//!   same file gets the same `RelPath` before and after, so selection and
//!   expansion survive without the editor's retired GUID registry.
//! * **Nothing here touches the file system.** The walk is
//!   `kairos_editor_app::project::scanner`; this module is the pure half, which is
//!   what makes the ordering, visibility and truncation rules testable without a
//!   tempdir.
//!
//! Three things make a node invisible and they stay separate: **system roots**
//! (`Library`) are scanned normally and only ordered last; **ignored** directories
//! (`.kairosignore`) never enter the snapshot at all (see
//! [`IgnoreRules`](super::IgnoreRules)); **hidden** names are in the snapshot but
//! not drawn unless `ProjectView::show_hidden`.

use std::collections::{BTreeSet, HashMap};
use std::fmt;

use super::view::ProjectView;
use super::ScanEntry;

/// One node of the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u32);

impl NodeId {
    pub(crate) fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The node's index in the tree's table. Ids are only meaningful for the
    /// tree revision they came from (`assemble` rebuilds the table).
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// A node's path relative to the project root, always `/`-separated; the root
/// itself is the empty string.
///
/// This is the tree's identity: the same file gets the same `RelPath` before and
/// after a rescan, so selection/expansion survive a rebuild without a GUID.
/// The app converts OS paths to it at the IO boundary (`to_string_lossy`, `/`
/// separators); core never sees a `std::path::Path`.
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

    /// The last segment; empty for the root.
    pub fn name(&self) -> &str {
        match self.0.rfind('/') {
            Some(slash) => &self.0[slash + 1..],
            None => &self.0,
        }
    }

    /// The containing directory; `None` only for the root (`"a"`'s parent is the
    /// root, not `None`).
    pub fn parent(&self) -> Option<RelPath> {
        if self.is_root() {
            return None;
        }
        match self.0.rfind('/') {
            Some(slash) => Some(RelPath(self.0[..slash].to_owned())),
            None => Some(RelPath::default()),
        }
    }

    /// This path with one more segment below it.
    pub fn child(&self, name: &str) -> RelPath {
        if self.is_root() {
            RelPath(name.to_owned())
        } else {
            RelPath(format!("{}/{name}", self.0))
        }
    }

    /// Whether this path is `ancestor` itself or one of its descendants.
    ///
    /// The root is not "under" anything but itself: an empty ancestor cannot
    /// match a child prefix, which keeps `is_under(&RelPath::default())` a
    /// meaningful question instead of an always-true one.
    pub fn is_under(&self, ancestor: &RelPath) -> bool {
        if self == ancestor {
            return true;
        }
        let bytes = self.0.as_bytes();
        let ancestor = ancestor.0.as_bytes();
        bytes.len() > ancestor.len()
            && bytes.starts_with(ancestor)
            && bytes[ancestor.len()] == b'/'
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    Folder,
    File,
}

/// One row of the snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// The node's identity; [`Node::name`] is derived from it.
    pub path: RelPath,
    pub kind: NodeKind,
    pub parent: Option<NodeId>,
    /// Already ordered: folders first, then by case-insensitive name.
    pub children: Vec<NodeId>,
    /// `FNV-1a(path)`, computed once when the tree is built: egui row ids need
    /// to be stable across passes, and hashing a path per row per frame would
    /// cost more than the row.
    pub row_id: u64,
}

impl Node {
    /// The last path segment (row label, rename editor's initial text). Derived
    /// rather than stored so the two can never disagree.
    pub fn name(&self) -> &str {
        self.path.name()
    }

    pub fn is_folder(&self) -> bool {
        self.kind == NodeKind::Folder
    }
}

/// What [`ProjectTree::assemble`] had to drop.
///
/// Core cannot log (it would need an IO sink), so a lossy assemble reports
/// instead of hiding it; the reducer turns a non-zero report into the panel's
/// visible error, which is what keeps the legacy `read_dir`-swallows-errors
/// defect from coming back in a new shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AssembleReport {
    /// Entries dropped because their path was already taken (two names that
    /// collide after a lossy UTF-8 conversion) or because their parent was not
    /// in the snapshot (an entry from an ignored or truncated directory).
    pub skipped: usize,
}

impl AssembleReport {
    pub fn is_clean(&self) -> bool {
        self.skipped == 0
    }
}

/// A snapshot of the project's file system at one point in time.
///
/// The root always exists (`nodes[0]`, `path == ""`); an unscanned tree is a root
/// with no children, not an absent tree, so every consumer can ask the tree
/// questions without an `Option` around it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectTree {
    nodes: Vec<Node>,
    by_path: HashMap<RelPath, NodeId>,
    system_roots: BTreeSet<RelPath>,
    system_nodes: usize,
    revision: u64,
    truncated: bool,
}

impl Default for ProjectTree {
    fn default() -> Self {
        let root = RelPath::default();
        let node = Node {
            row_id: row_id(&root),
            path: root.clone(),
            kind: NodeKind::Folder,
            parent: None,
            children: Vec::new(),
        };
        let mut by_path = HashMap::new();
        by_path.insert(root, NodeId(0));
        Self {
            nodes: vec![node],
            by_path,
            system_roots: BTreeSet::new(),
            system_nodes: 0,
            revision: 0,
            truncated: false,
        }
    }
}

impl ProjectTree {
    /// The one node that is always present.
    pub fn root(&self) -> NodeId {
        NodeId(0)
    }

    /// Bumped by every `assemble`. This is a *content* stamp: caches derived from
    /// the tree (visible rows, row layouts) invalidate on it. It is not the scan
    /// request id (`ScanState::generation`), which orders requests.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Whether the scan that produced this tree stopped at a budget.
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// Node count including the root.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the project has nothing in it (a root and no entries).
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// Nodes below a system root, including the system root itself.
    pub fn system_nodes(&self) -> usize {
        self.system_nodes
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.index())
    }

    pub fn get(&self, path: &RelPath) -> Option<&Node> {
        self.find(path).and_then(|id| self.node(id))
    }

    pub fn find(&self, path: &RelPath) -> Option<NodeId> {
        self.by_path.get(path).copied()
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.nodes.get(id.index()).map_or(&[], |node| &node.children)
    }

    /// The snapshot's paths, in node-table order (root first).
    pub fn paths(&self) -> impl Iterator<Item = &RelPath> {
        self.nodes.iter().map(|node| &node.path)
    }

    /// Whether `path` is exactly one of the system roots (the panel draws its
    /// separator before this row).
    pub fn is_system_dir(&self, path: &RelPath) -> bool {
        self.system_roots.contains(path)
    }

    /// Whether `path` is a system root or below one (M6's `In Library` search
    /// scope).
    pub fn is_under_system(&self, path: &RelPath) -> bool {
        self.system_roots.iter().any(|root| path.is_under(root))
    }

    pub fn system_roots(&self) -> impl Iterator<Item = &RelPath> {
        self.system_roots.iter()
    }

    /// The rows the panel draws, in pre-order: a folder's children follow it
    /// when it is expanded in `view`.
    ///
    /// Hidden names (dot entries and `.meta` companions) are in the tree but
    /// skipped unless `view.show_hidden`. They are deliberately not removed from
    /// the snapshot: revealing them must not cost a rescan, and the type registry
    /// (M5) will take over the companion rule.
    ///
    /// O(visible rows) — it never touches a collapsed subtree, which is what lets
    /// a 100k-node project draw at frame rate.
    pub fn visible_rows(&self, view: &ProjectView) -> Vec<NodeId> {
        let mut rows = Vec::new();
        let mut stack: Vec<NodeId> = self
            .children(self.root())
            .iter()
            .rev()
            .copied()
            .collect();
        while let Some(id) = stack.pop() {
            let Some(node) = self.node(id) else { continue };
            if !view.show_hidden && is_hidden_name(node.name()) {
                continue;
            }
            rows.push(id);
            if node.is_folder() && view.expanded.contains(&node.path) {
                stack.extend(node.children.iter().rev().copied());
            }
        }
        rows
    }

    /// Rebuild the table from a scan result.
    ///
    /// `entries` is what the scanner saw: directories and files, any order (a
    /// sorted list is used as-is, so the scanner's byte-sorted output makes the
    /// walk O(n); an unsorted list is sorted first, which keeps this function
    /// honest when a test builds entries by hand).
    ///
    /// The root is implicit: an entry for the empty path is skipped, and an entry
    /// whose parent is absent is skipped too (it belonged to an ignored or
    /// truncated directory). Skipped entries are reported, never hidden.
    pub fn assemble(
        &mut self,
        entries: &[ScanEntry],
        system_dirs: &[String],
        truncated: bool,
    ) -> AssembleReport {
        self.nodes.clear();
        self.by_path.clear();
        self.system_roots.clear();
        self.system_nodes = 0;
        self.truncated = truncated;

        let root = RelPath::default();
        self.nodes.push(Node {
            row_id: row_id(&root),
            path: root.clone(),
            kind: NodeKind::Folder,
            parent: None,
            children: Vec::new(),
        });
        self.by_path.insert(root, NodeId(0));

        // A byte-sorted list is the scanner's normal output: reusing it keeps the
        // walk single-pass (a parent always precedes its children). Anything else
        // is copied and sorted rather than trusted.
        let ordered: Vec<&ScanEntry> = if entries
            .windows(2)
            .all(|pair| pair[0].path <= pair[1].path)
        {
            entries.iter().collect()
        } else {
            let mut sorted = entries.iter().collect::<Vec<_>>();
            sorted.sort_by(|a, b| a.path.cmp(&b.path));
            sorted
        };

        let mut report = AssembleReport::default();
        for entry in ordered {
            let path = &entry.path;
            if path.is_root() {
                report.skipped += 1;
                continue;
            }
            let Some(parent_path) = path.parent() else {
                report.skipped += 1;
                continue;
            };
            let Some(parent) = self.by_path.get(&parent_path).copied() else {
                report.skipped += 1;
                continue;
            };
            if self.by_path.contains_key(path) {
                report.skipped += 1;
                continue;
            }
            let id = NodeId::from_index(self.nodes.len());
            let kind = if entry.is_dir {
                NodeKind::Folder
            } else {
                NodeKind::File
            };
            self.nodes.push(Node {
                row_id: row_id(path),
                path: path.clone(),
                kind,
                parent: Some(parent),
                children: Vec::new(),
            });
            self.nodes[parent.index()].children.push(id);
            self.by_path.insert(path.clone(), id);
            if parent == self.root()
                && kind == NodeKind::Folder
                && system_dirs
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(path.name()))
            {
                self.system_roots.insert(path.clone());
            }
            if !self.system_roots.is_empty()
                && self.system_roots.iter().any(|root| path.is_under(root))
            {
                self.system_nodes += 1;
            }
        }

        self.sort_children();
        self.revision = self.revision.wrapping_add(1);
        report
    }

    /// Order every node's children: folders first, then case-insensitive name.
    ///
    /// At the root the system directories ( `Library` ) sink below everything,
    /// including files — that placement is what lets the panel draw its separator
    /// without knowing anything about the policy.
    fn sort_children(&mut self) {
        let root = self.root();
        for index in 0..self.nodes.len() {
            let children = std::mem::take(&mut self.nodes[index].children);
            let mut keyed: Vec<(SortKey, NodeId)> = children
                .iter()
                .filter_map(|&child| {
                    let node = self.node(child)?;
                    Some((
                        SortKey {
                            system: index == root.index() && self.system_roots.contains(&node.path),
                            file: !node.is_folder(),
                            lower: node.name().to_lowercase(),
                            name: node.name().to_owned(),
                        },
                        child,
                    ))
                })
                .collect();
            keyed.sort();
            self.nodes[index].children = keyed.into_iter().map(|(_, child)| child).collect();
        }
    }
}

/// The sort key for one child slot.
///
/// `sort()` on this tuple gives the tree's whole ordering rule a name: system
/// roots last at the root level, folders before files everywhere, names
/// case-insensitively (raw name breaks ties so `A` and `a` keep a stable order).
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct SortKey {
    system: bool,
    file: bool,
    lower: String,
    name: String,
}

/// Whether a name is hidden by default: dot entries and `.meta` companions.
///
/// The companion half is a placeholder for M5's `FileTypeRegistry`: a single
/// documented rule now beats a type table that does not exist yet, and moving it
/// later is one function.
pub fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.') || name.ends_with(".meta")
}

/// `FNV-1a` over the path's UTF-8 bytes.
fn row_id(path: &RelPath) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in path.as_str().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}