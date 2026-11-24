//! Persisting the editor layout (M4, D13).
//!
//! What survives a restart: which panels are open (derived from the dock), the
//! dock tree and fractions, each leaf's tab order and active tab, the focused
//! tab, the floating windows (position/size/minimized/maximized), and the
//! Project panel's tree/content split. Panel **session** state — expanded
//! folders, selection, scroll, search — is deliberately *not* persisted: it is
//! low value per byte and owned by panel instances that drop with the tab (D13
//! in docs/plan/project-panel).
//!
//! The dock tree is host state: kairos_editor_core::LayoutState owns only
//! open/focus, and dragging/splitting mutates DockState inside Docker::show
//! without producing an action. So this module is a host-side observer:
//! EditorApp::begin_frame captures the persistable subset, and a changed
//! snapshot is written after a 500 ms debounce (plus a flush from
//! Layer::on_exit). docs/plan/project-panel M4 L4 records why this is not an
//! Effect::PersistLayout.
//!
//! File: Library/editor_layout.toml — versioned and disposable. Deleting it
//! (or writing garbage into it) resets the layout to default_dock; it is in
//! .gitignore because it is per-user state, not project data.

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Rect, pos2, vec2};
use kairos_editor_component::{PanelHost, PanelInstanceId};
use kairos_editor_core::PanelId;
use kairos_editor_docking::{
    docker::DockState,
    split::Fraction,
    surface::{Surface, WindowState},
    tree::{LeafNode, Node, SplitNode, TabIndex, Tree},
};
use serde::{Deserialize, Serialize};

/// Where the layout file lives, relative to the project root.
pub(crate) const LAYOUT_FILE: &str = "Library/editor_layout.toml";

/// Bumped when the file format changes in a way an older reader cannot absorb.
const VERSION: u32 = 1;
/// A layout is written after the last change; a drag/frame storm coalesces.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);
/// Defensive read cap: a layout is a few KB. Anything larger is not this file.
const MAX_FILE_BYTES: u64 = 64 * 1024;
/// Node cap per surface (main tree or one floating-window tree).
const MAX_NODES: usize = 1024;
/// Floating-window cap; each one is a dock surface.
const MAX_WINDOWS: usize = 32;
/// The Project panel split range, mirroring ProjectView::tree_split's reducer
/// clamp (0.15..=0.6).
const MIN_TREE_SPLIT: f32 = 0.15;
const MAX_TREE_SPLIT: f32 = 0.60;
/// A restored dock split may not collapse a pane: the divisor is a UI
/// arrangement, so a hand-edited `fraction = 0.0` is clamped instead of hidden.
const MIN_SPLIT_FRACTION: f32 = 0.05;

/// The header written above the TOML body. Comments are ignored on read.
const FILE_HEADER: &str =
    "# Editor layout written by Kairos. Delete this file to reset the layout.\n";

/// The path of the layout file for a project root.
pub(crate) fn layout_path(root: &Path) -> PathBuf {
    root.join(LAYOUT_FILE)
}

/// Read the layout file, or None when it is missing or unusable.
///
/// A corrupt file is a warning, never a failure: the editor boots with the
/// default layout and the next change rewrites a valid file.
pub(crate) fn load(root: &Path) -> Option<LayoutFile> {
    let path = layout_path(root);
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        Err(error) => {
            kairos_log::warn!(
                target: "kairos_editor",
                "editor layout: cannot read {}: {error}; using the default layout",
                path.display()
            );
            return None;
        }
    };
    if metadata.len() > MAX_FILE_BYTES {
        kairos_log::warn!(
            target: "kairos_editor",
            "editor layout: {} is {} bytes (limit {MAX_FILE_BYTES}); using the default layout",
            path.display(),
            metadata.len()
        );
        return None;
    }
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            kairos_log::warn!(
                target: "kairos_editor",
                "editor layout: cannot read {}: {error}; using the default layout",
                path.display()
            );
            return None;
        }
    };
    let file: LayoutFile = match toml::from_str(&text) {
        Ok(file) => file,
        Err(error) => {
            kairos_log::warn!(
                target: "kairos_editor",
                "editor layout: {} is not valid: {error}; using the default layout",
                path.display()
            );
            return None;
        }
    };
    if file.version != VERSION {
        kairos_log::warn!(
            target: "kairos_editor",
            "editor layout: {} has version {} (this build reads {VERSION}); using the default layout",
            path.display(),
            file.version
        );
        return None;
    }
    Some(file)
}

/// Write the layout atomically: the full file is written to a sibling temp file
/// and renamed over the target, so a crash can never leave a half file behind.
pub(crate) fn save(path: &Path, file: &LayoutFile) -> Result<(), String> {
    let body = toml::to_string_pretty(file).map_err(|error| error.to_string())?;
    let mut text = String::with_capacity(FILE_HEADER.len() + body.len());
    text.push_str(FILE_HEADER);
    text.push_str(&body);

    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has no file name", path.display()))?;
    let temp = path.with_file_name(format!(".{name}.tmp-{}", std::process::id()));
    fs::write(&temp, text.as_bytes()).map_err(|error| format!("{}: {error}", temp.display()))?;
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!("{}: {error}", path.display())
    })
}

/// One parsed layout file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct LayoutFile {
    pub version: u32,
    /// The main dock surface's tree (empty = no tab in the main surface).
    #[serde(default)]
    pub main: SurfaceFile,
    /// Floating windows, in creation order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<WindowFile>,
    /// The focused tab, by panel kind. None means no keyboard focus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
    /// Panel-level layout values that are not dock structure.
    #[serde(default)]
    pub panels: PanelsFile,
}

/// One surface's tree: a preorder list (a split's left subtree precedes its
/// right subtree, both directly after the split node).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct SurfaceFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<NodeFile>,
}

/// One tree node, in the file's own vocabulary: the app owns this format, the
/// dock crate owns the in-memory tree (Tree::from_nodes validates the decoded
/// shape again).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct NodeFile {
    pub kind: NodeKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tabs: Vec<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub active: usize,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub fraction: f32,
}

/// What a node is. A leaf holds tabs; horizontal/vertical split the space
/// between two children.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum NodeKind {
    Leaf,
    Horizontal,
    Vertical,
}

impl NodeFile {
    #[cfg(test)]
    fn leaf(tabs: &[&str], active: usize) -> Self {
        Self {
            kind: NodeKind::Leaf,
            tabs: tabs.iter().map(|tab| (*tab).to_owned()).collect(),
            active,
            fraction: 0.0,
        }
    }

    fn split(kind: NodeKind, fraction: f32) -> Self {
        Self {
            kind,
            tabs: Vec::new(),
            active: 0,
            fraction,
        }
    }
}

/// One floating window: its tree plus the geometry egui restores from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct WindowFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<NodeFile>,
    #[serde(default)]
    pub position: [f32; 2],
    #[serde(default)]
    pub size: [f32; 2],
    #[serde(default, skip_serializing_if = "is_false")]
    pub minimized: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub maximized: bool,
}

/// Layout values of individual panels that are not part of the dock tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct PanelsFile {
    #[serde(default)]
    pub project: ProjectPanelFile,
}

impl Default for PanelsFile {
    fn default() -> Self {
        Self {
            project: ProjectPanelFile::default(),
        }
    }
}

/// The Project panel's layout values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ProjectPanelFile {
    /// The project tree's fraction of the panel width (D1).
    #[serde(default = "default_tree_split")]
    pub tree_split: f32,
}

impl Default for ProjectPanelFile {
    fn default() -> Self {
        Self {
            tree_split: default_tree_split(),
        }
    }
}

/// The Project panel's default split ratio. Kept in sync with
/// ProjectView::default().tree_split instead of repeating the number.
fn default_tree_split() -> f32 {
    kairos_editor_core::project::ProjectView::default().tree_split
}

pub(crate) fn clamp_tree_split(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(MIN_TREE_SPLIT, MAX_TREE_SPLIT)
    } else {
        default_tree_split()
    }
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

fn is_zero_f32(value: &f32) -> bool {
    *value == 0.0
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl LayoutFile {
    /// Snapshot the persistable subset of dock (plus the Project split).
    ///
    /// Returns None when the dock holds a state that has no faithful file
    /// representation (a node layout the restore path would reject). The caller
    /// skips this save; the next change tries again.
    pub(crate) fn capture(dock: &DockState<PanelInstanceId>, tree_split: f32) -> Option<Self> {
        let surfaces = dock.surfaces();
        let Some(Surface::Main(tree)) = surfaces.first() else {
            return None;
        };
        let main = SurfaceFile {
            nodes: capture_tree(tree)?,
        };

        let mut windows = Vec::with_capacity(surfaces.len().saturating_sub(1));
        for surface in surfaces.iter().skip(1) {
            let Surface::Window(tree, state) = surface else {
                continue;
            };
            let nodes = capture_tree(tree)?;
            if nodes.is_empty() {
                // A window without tabs is not restorable; it is dropped as the
                // dock would drop it, not saved as an invalid surface.
                continue;
            }
            let (position, size) = (state.position()?, state.size()?);
            if !position.x.is_finite()
                || !position.y.is_finite()
                || !size.x.is_finite()
                || !size.y.is_finite()
            {
                return None;
            }
            windows.push(WindowFile {
                nodes,
                position: [position.x, position.y],
                size: [size.x, size.y],
                minimized: state.is_minimized(),
                maximized: state.is_maximized(),
            });
        }
        if windows.len() > MAX_WINDOWS {
            return None;
        }

        Some(Self {
            version: VERSION,
            main,
            windows,
            focus: dock.focused_tab().map(|id| id.kind.0.to_owned()),
            panels: PanelsFile {
                project: ProjectPanelFile {
                    tree_split: clamp_tree_split(tree_split),
                },
            },
        })
    }

    /// Check a parsed file against the panels this build knows and normalise it
    /// into a RestoredLayout.
    ///
    /// A single unknown panel kind invalidates the whole file: v1 does not
    /// rewrite the tree around a dropped tab (that would silently change the
    /// layout); the caller falls back to default_dock instead.
    pub(crate) fn restore(&self, known: &[PanelId]) -> Result<RestoredLayout, String> {
        if self.version != VERSION {
            return Err(format!(
                "unsupported layout version {} (this build writes {VERSION})",
                self.version
            ));
        }
        if self.main.nodes.len() > MAX_NODES {
            return Err(format!(
                "the main surface has {} nodes (limit {MAX_NODES})",
                self.main.nodes.len()
            ));
        }
        if self.windows.len() > MAX_WINDOWS {
            return Err(format!(
                "the layout has {} floating windows (limit {MAX_WINDOWS})",
                self.windows.len()
            ));
        }
        if !is_preorder_tree(&self.main.nodes) {
            return Err(String::from("the main surface's node list is not a tree"));
        }

        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut open: BTreeSet<PanelId> = BTreeSet::new();
        collect_panels(&self.main.nodes, known, &mut seen, &mut open)?;
        for window in &self.windows {
            if window.nodes.is_empty() {
                return Err(String::from("a floating window has no tab"));
            }
            if window.nodes.len() > MAX_NODES {
                return Err(format!(
                    "a floating window has {} nodes (limit {MAX_NODES})",
                    window.nodes.len()
                ));
            }
            if !is_preorder_tree(&window.nodes) {
                return Err(String::from("a floating window's node list is not a tree"));
            }
            if !(window.position.iter().all(|value| value.is_finite())
                && window.size.iter().all(|value| value.is_finite())
                && window.size.iter().all(|value| *value > 0.0))
            {
                return Err(String::from("a floating window has invalid geometry"));
            }
            collect_panels(&window.nodes, known, &mut seen, &mut open)?;
        }

        Ok(RestoredLayout {
            file: self.clone(),
            open: open.into_iter().collect(),
            tree_split: clamp_tree_split(self.panels.project.tree_split),
        })
    }
}

/// A validated layout file plus what the boot path needs from it.
#[derive(Debug)]
pub(crate) struct RestoredLayout {
    file: LayoutFile,
    open: Vec<PanelId>,
    tree_split: f32,
}

impl RestoredLayout {
    /// The panels to open, derived from the file's tabs (the dock tree is the
    /// only truth; no separate open list is stored).
    pub(crate) fn open(&self) -> &[PanelId] {
        &self.open
    }

    pub(crate) fn tree_split(&self) -> f32 {
        self.tree_split
    }

    /// Rebuild the dock against the live panel instances.
    ///
    /// Returns None when a tab of a known kind has no live instance (the host
    /// could not build that panel); the caller falls back to the default layout.
    pub(crate) fn dock(&self, host: &PanelHost) -> Option<DockState<PanelInstanceId>> {
        self.dock_with(&|name: &str| {
            let kind = host.kinds().find(|kind| kind.0 == name)?;
            host.id_of(kind)
        })
    }

    fn dock_with(
        &self,
        resolve: &dyn Fn(&str) -> Option<PanelInstanceId>,
    ) -> Option<DockState<PanelInstanceId>> {
        let mut surfaces = Vec::with_capacity(1 + self.file.windows.len());
        surfaces.push(Surface::Main(build_tree(&self.file.main.nodes, resolve)?));
        for window in &self.file.windows {
            let tree = build_tree(&window.nodes, resolve)?;
            let mut state = WindowState::new();
            state.set_position(pos2(window.position[0], window.position[1]));
            state.set_size(vec2(window.size[0], window.size[1]));
            state.set_minimized(window.minimized);
            state.set_maximized(window.maximized);
            surfaces.push(Surface::Window(tree, state));
        }
        let mut dock = DockState::from_parts(surfaces)?;
        if let Some(focus) = &self.file.focus
            && let Some(id) = resolve(focus)
        {
            dock.focus_tab(id);
        }
        Some(dock)
    }
}

/// Serialize one surface tree into the file's preorder node list.
fn capture_tree(tree: &Tree<PanelInstanceId>) -> Option<Vec<NodeFile>> {
    // A dock with no tabs is stored either as an empty node list or, before the
    // first tab, as a single empty leaf (DockState::new). Both mean "empty".
    if tree.nodes.is_empty() {
        return Some(Vec::new());
    }
    if tree.nodes.len() == 1 && matches!(&tree.nodes[0], Node::Leaf(leaf) if leaf.tabs.is_empty()) {
        return Some(Vec::new());
    }

    fn walk(nodes: &[Node<PanelInstanceId>], index: usize, out: &mut Vec<NodeFile>) -> Option<()> {
        let node = nodes.get(index)?;
        match node {
            Node::Empty => None,
            Node::Leaf(leaf) => {
                if leaf.tabs.is_empty() || leaf.active.0 >= leaf.tabs.len() {
                    return None;
                }
                out.push(NodeFile {
                    kind: NodeKind::Leaf,
                    tabs: leaf.tabs.iter().map(|id| id.kind.0.to_owned()).collect(),
                    active: leaf.active.0,
                    fraction: 0.0,
                });
                Some(())
            }
            Node::Horizontal(split) => {
                out.push(NodeFile::split(
                    NodeKind::Horizontal,
                    split.fraction.value(),
                ));
                walk(nodes, index * 2 + 1, out)?;
                walk(nodes, index * 2 + 2, out)
            }
            Node::Vertical(split) => {
                out.push(NodeFile::split(NodeKind::Vertical, split.fraction.value()));
                walk(nodes, index * 2 + 1, out)?;
                walk(nodes, index * 2 + 2, out)
            }
        }
    }

    let mut out = Vec::new();
    walk(&tree.nodes, 0, &mut out)?;
    if out.len() > MAX_NODES {
        return None;
    }
    // Every non-empty node must have been visited; an unreachable one means the
    // live dock is in a state the restore path would reject.
    let non_empty = tree.nodes.iter().filter(|node| !node.is_empty()).count();
    (out.len() == non_empty).then_some(out)
}

/// Decode the file's preorder node list into the dock's heap storage.
fn build_tree(
    nodes: &[NodeFile],
    resolve: &dyn Fn(&str) -> Option<PanelInstanceId>,
) -> Option<Tree<PanelInstanceId>> {
    if nodes.is_empty() {
        return Tree::from_nodes(Vec::new());
    }

    fn place(
        nodes: &[NodeFile],
        cursor: &mut usize,
        storage: &mut Vec<Node<PanelInstanceId>>,
        resolve: &dyn Fn(&str) -> Option<PanelInstanceId>,
        index: usize,
    ) -> Option<()> {
        let spec = nodes.get(*cursor)?;
        *cursor += 1;
        while storage.len() <= index {
            storage.push(Node::Empty);
        }
        match spec.kind {
            NodeKind::Leaf => {
                let mut tabs = Vec::with_capacity(spec.tabs.len());
                for tab in &spec.tabs {
                    tabs.push(resolve(tab)?);
                }
                let mut leaf = LeafNode::new(tabs);
                leaf.active = TabIndex(spec.active);
                storage[index] = Node::Leaf(leaf);
            }
            NodeKind::Horizontal | NodeKind::Vertical => {
                let fraction = spec
                    .fraction
                    .clamp(MIN_SPLIT_FRACTION, 1.0 - MIN_SPLIT_FRACTION);
                let split = SplitNode::new(Rect::NOTHING, Fraction::new(fraction));
                storage[index] = if spec.kind == NodeKind::Horizontal {
                    Node::Horizontal(split)
                } else {
                    Node::Vertical(split)
                };
                place(nodes, cursor, storage, resolve, index * 2 + 1)?;
                place(nodes, cursor, storage, resolve, index * 2 + 2)?;
            }
        }
        Some(())
    }

    let mut storage = Vec::with_capacity(nodes.len());
    let mut cursor = 0;
    place(nodes, &mut cursor, &mut storage, resolve, 0)?;
    if cursor != nodes.len() {
        return None;
    }
    Tree::from_nodes(storage)
}

/// Whether nodes is a complete preorder encoding of a tree.
fn is_preorder_tree(nodes: &[NodeFile]) -> bool {
    fn check(nodes: &[NodeFile], cursor: usize) -> Option<usize> {
        let node = nodes.get(cursor)?;
        match node.kind {
            NodeKind::Leaf => {
                if node.tabs.is_empty() || node.active >= node.tabs.len() {
                    return None;
                }
                Some(cursor + 1)
            }
            NodeKind::Horizontal | NodeKind::Vertical => {
                if !node.fraction.is_finite() {
                    return None;
                }
                let after_left = check(nodes, cursor + 1)?;
                check(nodes, after_left)
            }
        }
    }
    nodes.is_empty() || check(nodes, 0) == Some(nodes.len())
}

/// Collect the tabs of one surface, rejecting unknown or duplicated panels.
fn collect_panels(
    nodes: &[NodeFile],
    known: &[PanelId],
    seen: &mut BTreeSet<String>,
    open: &mut BTreeSet<PanelId>,
) -> Result<(), String> {
    for node in nodes {
        for tab in &node.tabs {
            if !seen.insert(tab.clone()) {
                return Err(format!("\"{tab}\" appears more than once in the layout"));
            }
            let kind = known
                .iter()
                .copied()
                .find(|kind| kind.0 == tab)
                .ok_or_else(|| format!("unknown panel \"{tab}\""));
            open.insert(kind?);
        }
    }
    Ok(())
}

/// Debounced writer for the layout file.
///
/// observe is cheap (one comparison against the last snapshot) and runs every
/// frame from EditorApp::begin_frame; the file is written once the dock has
/// been still for SAVE_DEBOUNCE. flush is the on_exit path: a change made in
/// the last half second before the window closes is not lost.
pub(crate) struct LayoutPersist {
    /// None for tests that do not do file IO.
    path: Option<PathBuf>,
    pending: Option<LayoutFile>,
    deadline: Option<Instant>,
    last_seen: Option<LayoutFile>,
}

impl LayoutPersist {
    pub(crate) fn new(path: PathBuf, baseline: Option<LayoutFile>) -> Self {
        Self {
            path: Some(path),
            pending: None,
            deadline: None,
            last_seen: baseline,
        }
    }

    /// A writer that never touches the disk (unit tests of the app frame).
    #[cfg(test)]
    pub(crate) fn disabled() -> Self {
        Self {
            path: None,
            pending: None,
            deadline: None,
            last_seen: None,
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.path.is_some()
    }

    /// Record this frame's snapshot. A changed layout (re)starts the debounce;
    /// an unchanged one costs one comparison.
    pub(crate) fn observe(&mut self, layout: Option<LayoutFile>, now: Instant) {
        if !self.enabled() {
            return;
        }
        let Some(layout) = layout else {
            return;
        };
        if self.last_seen.as_ref() == Some(&layout) {
            return;
        }
        self.last_seen = Some(layout.clone());
        self.pending = Some(layout);
        self.deadline = Some(now + SAVE_DEBOUNCE);
    }

    /// Write when the debounce elapsed. Called once per frame.
    pub(crate) fn poll(&mut self, now: Instant) {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.flush(now);
        }
    }

    /// Write the pending layout immediately (exit path).
    pub(crate) fn flush(&mut self, _now: Instant) {
        self.deadline = None;
        let (Some(path), Some(layout)) = (self.path.as_ref(), self.pending.take()) else {
            return;
        };
        if let Err(error) = save(path, &layout) {
            kairos_log::warn!(
                target: "kairos_editor",
                "editor layout: cannot save {}: {error}",
                path.display()
            );
        }
    }

    #[cfg(test)]
    pub(crate) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
}

#[cfg(test)]
mod test;
