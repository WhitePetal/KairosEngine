use std::{
    cmp::max,
    collections::HashSet,
    fmt,
    ops::{Index, IndexMut, Range},
    slice::{Iter, IterMut},
};

use egui::Rect;

use crate::kairos_editor::ui::docking_tab::{dock_state::tree::node::Node, surfaces::SurfaceIndex};

pub mod node;

/// Direction in which a new node is created relatively to the parent node at which the split occurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Split {
    Left,
    Right,
    Above,
    Below,
}
impl Split {
    /// Returns whether the split is vertical.
    pub const fn is_top_bottom(self) -> bool {
        matches!(self, Split::Above | Split::Below)
    }

    /// Returns whether the split is horizontal.
    pub const fn is_left_right(self) -> bool {
        matches!(self, Split::Left | Split::Right)
    }
}

/// Specify how a tab should be added to a Node.
pub enum TabInsert {
    /// Split the node in the given direction.
    Split(Split),

    /// Insert the tab at the given index.
    Insert(TabIndex),

    /// Append the tab to the node.
    Append,
}

/// The destination for a tab which is being moved.
pub enum TabDestination {
    /// Move to a new window with this rect.
    Window(Rect),

    /// Move to a an existing node with this insertion.
    Node(SurfaceIndex, NodeIndex, TabInsert),

    /// Move to an empty surface.
    EmptySurface(SurfaceIndex),
}

impl From<(SurfaceIndex, NodeIndex, TabInsert)> for TabDestination {
    fn from(value: (SurfaceIndex, NodeIndex, TabInsert)) -> Self {
        TabDestination::Node(value.0, value.1, value.2)
    }
}
impl From<SurfaceIndex> for TabDestination {
    fn from(value: SurfaceIndex) -> Self {
        TabDestination::EmptySurface(value)
    }
}
impl TabDestination {
    /// Returns if this tab destination is a [`Window`](TabDestination::Window).
    pub fn is_window(&self) -> bool {
        matches!(self, Self::Window(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabIndex(pub usize);

impl From<usize> for TabIndex {
    #[inline(always)]
    fn from(index: usize) -> Self {
        TabIndex(index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeIndex(pub usize);

impl From<usize> for NodeIndex {
    #[inline(always)]
    fn from(value: usize) -> Self {
        NodeIndex(value)
    }
}

impl NodeIndex {
    /// Returns the index of the root node.
    pub const fn root() -> Self {
        Self(0)
    }

    /// Returns the index of the node to the left of the current one.
    #[inline(always)]
    pub fn left(self) -> Self {
        Self(self.0 * 2 + 1)
    }

    /// Returns the index of the node to the right of the current one.
    #[inline(always)]
    pub fn right(self) -> Self {
        Self(self.0 * 2 + 2)
    }

    /// Returns the index of the parent node or `None` if current node is the root.
    #[inline]
    pub fn parent(self) -> Option<Self> {
        if self.0 > 0 {
            Some(Self((self.0 - 1) / 2))
        } else {
            None
        }
    }

    /// Returns the number of nodes leading from the root to the current node, including `self`.
    #[inline(always)]
    pub fn level(self) -> usize {
        (usize::BITS - (self.0 + 1).leading_zeros()) as usize
    }

    /// Returns `true` if current node is the left child of its parent, otherwise `false`.
    #[inline(always)]
    pub const fn is_left(self) -> bool {
        self.0 % 2 != 0
    }

    /// Returns `true` if current node is the right child of its parent, otherwise `false`.
    #[inline(always)]
    pub const fn is_right(self) -> bool {
        self.0 % 2 == 0
    }

    #[inline]
    pub(super) const fn children_at(self, level: usize) -> Range<usize> {
        let base = 1 << level;
        let s = (self.0 + 1) * base - 1;
        let e = (self.0 + 2) * base - 1;
        s..e
    }

    #[inline]
    pub(super) const fn children_left(self, level: usize) -> Range<usize> {
        let base = 1 << level;
        let s = (self.0 + 1) * base - 1;
        let e = (self.0 + 1) * base + (base / 2) - 1;
        s..e
    }

    #[inline]
    pub(super) const fn children_right(self, level: usize) -> Range<usize> {
        let base = 1 << level;
        let s = (self.0 + 1) * base + (base / 2) - 1;
        let e = (self.0 + 2) * base - 1;
        s..e
    }
}

/// A full path to locate a node in an entire dock state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodePath {
    /// Index of the surface owning the node.
    pub surface: SurfaceIndex,
    /// Index of the node in the surface tree.
    pub node: NodeIndex,
}

/// Iterates over all tabs in a [`Tree`].
pub struct DrawerIter<'a, Drawer> {
    tree: &'a Tree<Drawer>,
    node_idx: usize,
    tab_idx: usize,
}

impl<'a, Drawer> DrawerIter<'a, Drawer> {
    pub fn new(tree: &'a Tree<Drawer>) -> Self {
        Self {
            tree,
            node_idx: 0,
            tab_idx: 0,
        }
    }
}

impl<'a, Drawer> Iterator for DrawerIter<'a, Drawer> {
    type Item = &'a Drawer;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.tree.nodes.get(self.node_idx)?.drawers() {
                Some(tabs) => match tabs.get(self.tab_idx) {
                    Some(tab) => {
                        self.tab_idx += 1;
                        return Some(tab);
                    }
                    None => {
                        self.node_idx += 1;
                        self.tab_idx = 0;
                    }
                },
                None => {
                    self.node_idx += 1;
                    self.tab_idx = 0;
                }
            }
        }
    }
}

impl<Drawer> std::fmt::Debug for DrawerIter<'_, Drawer> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DrawerIter").finish_non_exhaustive()
    }
}



#[derive(Clone)]
pub struct Tree<Drawer> {
    // Binary tree vector
    pub nodes: Vec<Node<Drawer>>,
    focused_node: Option<NodeIndex>,
    // Whether all subnodes of the tree is collapsed
    collapsed: bool,
    collapsed_leaf_count: i32,
}

impl<Drawer> fmt::Debug for Tree<Drawer> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tree").finish_non_exhaustive()
    }
}

impl<Drawer> Default for Tree<Drawer> {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            focused_node: None,
            collapsed: false,
            collapsed_leaf_count: 0,
        }
    }
}

impl<Drawer> Index<NodeIndex> for Tree<Drawer> {
    type Output = Node<Drawer>;

    #[inline(always)]
    fn index(&self, index: NodeIndex) -> &Self::Output {
        &self.nodes[index.0]
    }
}

impl<Drawer> IndexMut<NodeIndex> for Tree<Drawer> {
    #[inline(always)]
    fn index_mut(&mut self, index: NodeIndex) -> &mut Self::Output {
        &mut self.nodes[index.0]
    }
}

impl<Drawer> Tree<Drawer> {
    /// Create a new ['Tree'] with a given 'Vec' of 'Tab's in its root node.
    #[inline(always)]
    pub fn new(tabs: Vec<Drawer>) -> Self {
        let root = Node::leaf_with(tabs);
        Self {
            nodes: vec![root],
            focused_node: None,
            collapsed: false,
            collapsed_leaf_count: 0,
        }
    }

    /// Returns the number of nodes in the [`Tree`].
    ///
    /// This includes [`Empty`](Node::Empty) nodes.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` if the number of nodes in the tree is 0, otherwise `false`.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns an [`Iterator`] of the underlying collection of nodes.
    ///
    /// This includes [`Empty`](Node::Empty) nodes.
    #[inline(always)]
    pub fn iter(&self) -> Iter<'_, Node<Drawer>> {
        self.nodes.iter()
    }

    /// Returns [`IterMut`] of the underlying collection of nodes.
    ///
    /// This includes [`Empty`](Node::Empty) nodes.
    #[inline(always)]
    pub fn iter_mut(&mut self) -> IterMut<'_, Node<Drawer>> {
        self.nodes.iter_mut()
    }

    /// Returns an [`Iterator`] of [`NodeIndex`] ordered in a breadth first manner.
    #[inline(always)]
    pub fn breadth_first_index_iter(&self) -> impl Iterator<Item = NodeIndex> + use<Drawer> {
        (0..self.nodes.len()).map(NodeIndex)
    }

    /// Returns an iterator over all tabs in arbitrary order.
    #[inline(always)]
    pub fn drawers(&self) -> DrawerIter<'_, Drawer> {
        DrawerIter::new(self)
    }

    /// Counts and returns the number of tabs in the whole tree.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use egui_dock::{DockState, NodeIndex, TabIndex};
    /// let mut dock_state = DockState::new(vec!["node 1", "node 2", "node 3"]);
    /// assert_eq!(dock_state.main_surface().num_tabs(), 3);
    ///
    /// let [a, b] = dock_state.main_surface_mut().split_left(NodeIndex::root(), 0.5, vec!["tab 4", "tab 5"]);
    /// assert_eq!(dock_state.main_surface().num_tabs(), 5);
    ///
    /// dock_state.main_surface_mut().remove_leaf(a);
    /// assert_eq!(dock_state.main_surface().num_tabs(), 2);
    /// ```
    #[inline]
    pub fn num_drawers(&self) -> usize {
        let mut count = 0;
        for node in self.nodes.iter() {
            if let Node::Leaf(leaf) = node {
                count += leaf.drawers.len();
            }
        }
        count
    }

    /// Acquire a immutable borrow to the [`Node`] at the root of the tree.
    /// Returns [`None`] if the tree is empty.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use egui_dock::DockState;
    /// let mut dock_state = DockState::new(vec!["single tab"]);
    /// let root_node = dock_state.main_surface().root_node().unwrap();
    ///
    /// assert_eq!(root_node.tabs(), Some(["single tab"].as_slice()));
    /// ```
    pub fn root_node(&self) -> Option<&Node<Drawer>> {
        self.nodes.first()
    }

    /// Acquire a mutable borrow to the [`Node`] at the root of the tree.
    /// Returns [`None`] if the tree is empty.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use egui_dock::{DockState, LeafNode};
    /// let mut dock_state = DockState::new(vec!["single tab"]);
    /// let root_node = dock_state.main_surface_mut().root_node_mut().unwrap();
    /// let root_as_leaf = root_node.get_leaf_mut().unwrap();
    /// root_as_leaf.tabs.push("partner tab");
    ///
    /// assert_eq!(root_node.tabs(), Some(["single tab", "partner tab"].as_slice()));
    /// ```
    pub fn root_node_mut(&mut self) -> Option<&mut Node<Drawer>> {
        self.nodes.first_mut()
    }

    /// Creates two new nodes by splitting a given `parent` node and assigns them as its children. The first (old) node
    /// inherits content of the `parent` from before the split, and the second (new) gets the `tabs`.
    ///
    /// `fraction` (in range 0..=1) specifies how much of the `parent` node's area the old node will attempt to occupy
    /// after the split.
    ///
    /// The new node is placed relatively to the old node, in the direction specified by `split`.
    ///
    /// Returns the indices of the old node and the new node.
    ///
    /// # Panics
    ///
    /// If `fraction` isn't in range 0..=1.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use egui_dock::{DockState, SurfaceIndex, NodeIndex, Split};
    /// let mut dock_state = DockState::new(vec!["tab 1", "tab 2"]);
    ///
    /// // At this point, the main surface only contains the leaf with tab 1 and 2.
    /// assert!(dock_state.main_surface().root_node().unwrap().is_leaf());
    ///
    /// // Split the node, giving 50% of the space to the new nodes and 50% to the old ones.
    /// let [old, new] = dock_state.main_surface_mut()
    ///     .split_tabs(NodeIndex::root(), Split::Below, 0.5, vec!["tab 3"]);
    ///
    /// assert!(dock_state.main_surface().root_node().unwrap().is_parent());
    /// assert!(dock_state[SurfaceIndex::main()][old].is_leaf());
    /// assert!(dock_state[SurfaceIndex::main()][new].is_leaf());
    /// ```
    #[inline(always)]
    pub fn split_drawers(
        &mut self,
        parent: NodeIndex,
        split: Split,
        fraction: f32,
        tabs: Vec<Drawer>,
    ) -> [NodeIndex; 2] {
        self.split(parent, split, fraction, Node::leaf_with(tabs))
    }

    /// Creates two new nodes by splitting a given `parent` node and assigns them as its children. The first (old) node
    /// inherits content of the `parent` from before the split, and the second (new) gets the `tabs`.
    ///
    /// This is a shorthand for using `split_tabs` with [`Split::Above`].
    ///
    /// `fraction` (in range 0..=1) specifies how much of the `parent` node's area the old node will attempt to occupy
    /// after the split.
    ///
    /// The new node is placed *above* the old node.
    ///
    /// Returns the indices of the old node and the new node.
    ///
    /// # Panics
    ///
    /// If `fraction` isn't in range 0..=1.
    #[inline(always)]
    pub fn split_above(
        &mut self,
        parent: NodeIndex,
        fraction: f32,
        tabs: Vec<Drawer>,
    ) -> [NodeIndex; 2] {
        self.split(parent, Split::Above, fraction, Node::leaf_with(tabs))
    }

    /// Creates two new nodes by splitting a given `parent` node and assigns them as its children. The first (old) node
    /// inherits content of the `parent` from before the split, and the second (new) gets the `tabs`.
    ///
    /// This is a shorthand for using `split_tabs` with [`Split::Below`].
    ///
    /// `fraction` (in range 0..=1) specifies how much of the `parent` node's area the old node will attempt to occupy
    /// after the split.
    ///
    /// The new node is placed *below* the old node.
    ///
    /// Returns the indices of the old node and the new node.
    ///
    /// # Panics
    ///
    /// If `fraction` isn't in range 0..=1.
    #[inline(always)]
    pub fn split_below(
        &mut self,
        parent: NodeIndex,
        fraction: f32,
        tabs: Vec<Drawer>,
    ) -> [NodeIndex; 2] {
        self.split(parent, Split::Below, fraction, Node::leaf_with(tabs))
    }

    /// Creates two new nodes by splitting a given `parent` node and assigns them as its children. The first (old) node
    /// inherits content of the `parent` from before the split, and the second (new) gets the `tabs`.
    ///
    /// This is a shorthand for using `split_tabs` with [`Split::Left`].
    ///
    /// `fraction` (in range 0..=1) specifies how much of the `parent` node's area the old node will attempt to occupy
    /// after the split.
    ///
    /// The new node is placed to the *left* of the old node.
    ///
    /// Returns the indices of the old node and the new node.
    ///
    /// # Panics
    ///
    /// If `fraction` isn't in range 0..=1.
    #[inline(always)]
    pub fn split_left(
        &mut self,
        parent: NodeIndex,
        fraction: f32,
        tabs: Vec<Drawer>,
    ) -> [NodeIndex; 2] {
        self.split(parent, Split::Left, fraction, Node::leaf_with(tabs))
    }

    /// Creates two new nodes by splitting a given `parent` node and assigns them as its children. The first (old) node
    /// inherits content of the `parent` from before the split, and the second (new) gets the `tabs`.
    ///
    /// This is a shorthand for using `split_tabs` with [`Split::Right`].
    ///
    /// `fraction` (in range 0..=1) specifies how much of the `parent` node's area the old node will attempt to occupy
    /// after the split.
    ///
    /// The new node is placed to the *right* of the old node.
    ///
    /// Returns the indices of the old node and the new node.
    ///
    /// # Panics
    ///
    /// If `fraction` isn't in range 0..=1.
    #[inline(always)]
    pub fn split_right(
        &mut self,
        parent: NodeIndex,
        fraction: f32,
        tabs: Vec<Drawer>,
    ) -> [NodeIndex; 2] {
        self.split(parent, Split::Right, fraction, Node::leaf_with(tabs))
    }

    /// Creates two new nodes by splitting a given `parent` node and assigns them as its children. The first (old) node
    /// inherits content of the `parent` from before the split, and the second (new) uses `new`.
    ///
    /// `fraction` (in range 0..=1) specifies how much of the `parent` node's area the old node will attempt to occupy
    /// after the split.
    ///
    /// The new node is placed relatively to the old node, in the direction specified by `split`.
    ///
    /// Returns the indices of the old node and the new node.
    ///
    /// # Panics
    ///
    /// If `fraction` isn't in range 0..=1.
    ///
    /// If `new` is an [`Empty`](Node::Empty), [`Horizontal`](Node::Horizontal) or [`Vertical`](Node::Vertical) node.
    ///
    /// If `new` is a [`Leaf`](Node::Leaf) node without any tabs.
    ///
    /// If `parent` points to an [`Empty`](Node::Empty) node.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use egui_dock::{DockState, SurfaceIndex, NodeIndex, Split, Node};
    /// let mut dock_state = DockState::new(vec!["tab 1", "tab 2"]);
    ///
    /// // At this point, the main surface only contains the leaf with tab 1 and 2.
    /// assert!(dock_state.main_surface().root_node().unwrap().is_leaf());
    ///
    /// // Splits the node, giving 50% of the space to the new nodes and 50% to the old ones.
    /// let [old, new] = dock_state.main_surface_mut()
    ///     .split(NodeIndex::root(), Split::Below, 0.5, Node::leaf_with(vec!["tab 3"]));
    ///
    /// assert!(dock_state.main_surface().root_node().unwrap().is_parent());
    /// assert!(dock_state[SurfaceIndex::main()][old].is_leaf());
    /// assert!(dock_state[SurfaceIndex::main()][new].is_leaf());
    /// ```
    pub fn split(
        &mut self,
        parent: NodeIndex,
        split: Split,
        fraction: f32,
        new: Node<Drawer>,
    ) -> [NodeIndex; 2] {
        let old = self[parent].split(split, fraction);
        assert!(old.is_leaf() || old.is_parent());
        assert_ne!(new.drawers_count(), 0);
        // Resize vector to fit the new size of the binary tree.
        {
            let index = self.nodes.iter().rposition(|n| !n.is_empty()).unwrap_or(0);
            let level = NodeIndex(index).level();
            self.nodes
                .resize_with((1 << (level + 1)) - 1, || Node::Empty);
        }

        let index = match split {
            Split::Left | Split::Above => [parent.right(), parent.left()],
            Split::Right | Split::Below => [parent.left(), parent.right()],
        };

        // If the node were splitting is a parent, all it's children need to be moved.
        if old.is_parent() {
            let levels_to_move = NodeIndex(self.nodes.len()).level() - index[0].level();

            // Level 0 is ourself, which is done when we assign self[index[0]] = old, so start at 1.
            for level in (1..levels_to_move).rev() {
                // Old child indices for this level
                let old_start = parent.children_at(level).start;
                // New child indices for this level
                let new_start = index[0].children_at(level).start;

                // Children to be moved this level change
                let len = 1 << level;

                // Swap self[old_start..(old_start+len)] with self[new_start..(new_start+len)]
                // (the new part will only contain empty entries).
                let (old_range, new_range) = {
                    let (first_part, second_part) = self.nodes.split_at_mut(new_start);
                    // Cut to length.
                    (
                        &mut first_part[old_start..old_start + len],
                        &mut second_part[..len],
                    )
                };
                old_range.swap_with_slice(new_range);
            }
        }

        self[index[0]] = old;
        self[index[1]] = new;

        self.focused_node = Some(index[1]);
        self.node_update_collapsed(index[1]);

        index
    }

    fn first_leaf(&self, top: NodeIndex) -> Option<NodeIndex> {
        let left = top.left();
        let right = top.right();
        match (self.nodes.get(left.0), self.nodes.get(right.0)) {
            (Some(&Node::Leaf { .. }), _) => Some(left),
            (_, Some(&Node::Leaf { .. })) => Some(right),

            (
                Some(Node::Horizontal { .. } | Node::Vertical { .. }),
                Some(Node::Horizontal { .. } | Node::Vertical { .. }),
            ) => self.first_leaf(left).or(self.first_leaf(right)),
            (Some(Node::Horizontal { .. } | Node::Vertical { .. }), _) => self.first_leaf(left),
            (_, Some(Node::Horizontal { .. } | Node::Vertical { .. })) => self.first_leaf(right),

            (None, None)
            | (Some(&Node::Empty), None)
            | (None, Some(&Node::Empty))
            | (Some(&Node::Empty), Some(&Node::Empty)) => None,
        }
    }

    /// Returns the vieport [`Rect`] and the `Drawer` inside the focused leaf node or [`None`] if it dones not exist.
    pub fn find_active_focused(&mut self) -> Option<(Rect, &mut Drawer)> {
        match self.focused_node.and_then(|idx| self.nodes.get_mut(idx.0)) {
            Some(Node::Leaf(leaf)) => leaf.active_focused(),
            _ => None,
        }
    }

    /// Gets the node index of currently focused leaf node; returns [`None`] when no leaf is focused.
    #[inline]
    pub fn focused_leaf(&self) -> Option<NodeIndex> {
        self.focused_node
    }

    /// Sets the currently focused leaf to `node_index` if the node at `node_index` is a leaf.
    ///
    /// This method will not never panic and instead removes focus from all nodes when given an invalid index.
    #[inline]
    pub fn set_focused_node(&mut self, node_index: NodeIndex) {
        self.focused_node = self
            .nodes
            .get(node_index.0)
            .filter(|node| node.is_leaf())
            .map(|_| node_index);
    }

    /// Removes the given node from the [`Tree`].
    ///
    /// # Panics
    ///
    /// - If the tree is empty.
    /// - If the node at index `node` is not a [`Leaf`](Node::Leaf).
    pub fn remove_leaf(&mut self, node_index: NodeIndex) {
        assert!(!self.is_empty());
        assert!(self[node_index].is_leaf());

        let Some(parent) = node_index.parent() else {
            self.nodes.clear();
            return;
        };

        if Some(node_index) == self.focused_node {
            self.focused_node = None;
            let mut node_index = node_index;
            while let Some(parent) = node_index.parent() {
                let next = if node_index.is_left() {
                    parent.right()
                } else {
                    parent.left()
                };
                if self
                    .nodes
                    .get(node_index.0)
                    .is_some_and(|node_index| node_index.is_leaf())
                {
                    self.focused_node = Some(next);
                    break;
                }
                if let Some(node_index) = self.first_leaf(next) {
                    self.focused_node = Some(node_index);
                    break;
                }
                node_index = parent;
            }
        }

        self[parent] = Node::Empty;
        self[node_index] = Node::Empty;

        let mut level = 0;

        if node_index.is_left() {
            'left_end: loop {
                let dst = parent.children_at(level);
                let src = parent.children_right(level + 1);
                for (dst, src) in dst.zip(src) {
                    if src >= self.nodes.len() {
                        break 'left_end;
                    }
                    if Some(NodeIndex(src)) == self.focused_node {
                        self.focused_node = Some(NodeIndex(dst));
                    }
                    self.nodes[dst] = std::mem::replace(&mut self.nodes[src], Node::Empty);
                }
                level += 1;
            }
        } else {
            'right_end: loop {
                let dst = parent.children_at(level);
                let src = parent.children_left(level + 1);
                for (dst, src) in dst.zip(src) {
                    if src >= self.nodes.len() {
                        break 'right_end;
                    }
                    if Some(NodeIndex(src)) == self.focused_node {
                        self.focused_node = Some(NodeIndex(dst));
                    }
                    self.nodes[dst] = std::mem::replace(&mut self.nodes[src], Node::Empty);
                }
                level += 1;
            }
        }
        // Ensure that there are no trailing `Node::Empty` items
        while let Some(last_index) = self.nodes.len().checked_sub(1).map(NodeIndex) {
            if self[last_index].is_empty()
                && last_index.parent().is_some_and(|pi| !self[pi].is_parent())
            {
                self.nodes.pop();
            } else {
                break;
            }
        }
    }

    /// Pushes a tab to the first `Leaf` it finds or create a new leaf if an `Empty` node is encountered.
    pub fn push_to_first_leaf(&mut self, tab: Drawer) {
        for (index, node) in &mut self.nodes.iter_mut().enumerate() {
            match node {
                Node::Leaf(leaf) => {
                    leaf.active = TabIndex(leaf.drawers.len());
                    leaf.drawers.push(tab);
                    self.focused_node = Some(NodeIndex(index));
                    return;
                }
                Node::Empty => {
                    *node = Node::leaf(tab);
                    self.focused_node = Some(NodeIndex(index));
                    return;
                }
                _ => {}
            }
        }
        assert!(self.nodes.is_empty());
        self.nodes.push(Node::leaf_with(vec![tab]));
        self.focused_node = Some(NodeIndex(0));
    }

    /// Sets which is the active tab within a specific node.
    #[inline]
    pub fn set_active_tab(
        &mut self,
        node_index: impl Into<NodeIndex>,
        tab_index: impl Into<TabIndex>,
    ) {
        if let Some(Node::Leaf(leaf)) = self.nodes.get_mut(node_index.into().0) {
            leaf.set_active_tab(tab_index);
        };
    }

    /// Pushes `tab` to the currently focused leaf.
    ///
    /// If no leaf is focused it will be pushed to the first available leaf.
    ///
    /// If no leaf is available then a new leaf will be created.
    pub fn push_to_focused_leaf(&mut self, tab: Drawer) {
        match self.focused_node {
            Some(node) => {
                if self.nodes.is_empty() {
                    self.nodes.push(Node::leaf(tab));
                    self.focused_node = Some(NodeIndex::root());
                } else {
                    match &mut self[node] {
                        Node::Empty => {
                            self[node] = Node::leaf(tab);
                            self.focused_node = Some(node);
                        }
                        Node::Leaf(leaf) => {
                            leaf.append_drawer(tab);
                            self.focused_node = Some(node);
                        }
                        _ => {
                            self.push_to_first_leaf(tab);
                        }
                    }
                }
            }
            None => {
                if self.nodes.is_empty() {
                    self.nodes.push(Node::leaf(tab));
                    self.focused_node = Some(NodeIndex::root());
                } else {
                    self.push_to_first_leaf(tab);
                }
            }
        }
    }

    /// Removes the tab at the given ([`NodeIndex`], [`TabIndex`]) pair.
    ///
    /// If the node is emptied after the tab is removed, the node will also be removed.
    ///
    /// Returns the removed tab if it exists, or `None` otherwise.
    pub fn remove_drawer(
        &mut self,
        (node_index, tab_index): (NodeIndex, TabIndex),
    ) -> Option<Drawer> {
        let node = &mut self[node_index];
        let tab = node.remove_drawer(tab_index);
        if node.drawers_count() == 0 {
            self.remove_leaf(node_index);
        }
        tab
    }

    /// Returns a new [`Tree`] while mapping and filtering the tab type.
    /// Any remaining empty [`Node`]s are removed.
    pub fn filter_map_drawers<F, NewTab>(&self, mut function: F) -> Tree<NewTab>
    where
        F: FnMut(&Drawer) -> Option<NewTab>,
    {
        let Tree {
            focused_node,
            nodes,
            collapsed,
            collapsed_leaf_count,
        } = self;
        let mut emptied_nodes = HashSet::default();
        let nodes = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let filtered_node = node.filter_map_drawers(&mut function);
                if filtered_node.is_empty() && !node.is_empty() {
                    emptied_nodes.insert(NodeIndex(index));
                }
                filtered_node
            })
            .collect();
        let mut new_tree = Tree {
            nodes,
            focused_node: *focused_node,
            collapsed: *collapsed,
            collapsed_leaf_count: *collapsed_leaf_count,
        };
        new_tree.balance(emptied_nodes);
        new_tree
    }

    /// Returns a new [`Tree`] while mapping the tab type.
    pub fn map_drawers<F, NewTab>(&self, mut function: F) -> Tree<NewTab>
    where
        F: FnMut(&Drawer) -> NewTab,
    {
        self.filter_map_drawers(move |tab| Some(function(tab)))
    }

    /// Returns a new [`Tree`] while filtering the tab type.
    /// Any remaining empty [`Node`]s are removed.
    pub fn filter_drawers<F>(&self, mut predicate: F) -> Tree<Drawer>
    where
        F: FnMut(&Drawer) -> bool,
        Drawer: Clone,
    {
        self.filter_map_drawers(move |tab| predicate(tab).then(|| tab.clone()))
    }

    /// Removes all tabs for which `predicate` returns `false`.
    /// Any remaining empty [`Node`]s are also removed.
    pub fn retain_tabs<F>(&mut self, mut predicate: F)
    where
        F: FnMut(&mut Drawer) -> bool,
    {
        let mut emptied_nodes = HashSet::default();
        for (index, node) in self.nodes.iter_mut().enumerate() {
            node.retain_drawers(&mut predicate);
            if node.is_empty() {
                emptied_nodes.insert(NodeIndex(index));
            }
        }
        self.balance(emptied_nodes);
    }

    /// Sets the collapsing state of the [`Tree`].
    pub fn set_collapsed(&mut self, collapsed: bool) {
        self.collapsed = collapsed;
    }

    /// Returns whether the [`Tree`] is collapsed.
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Sets the number of collapsed layers of leaf subnodes in the [`Tree`].
    pub fn set_collapsed_leaf_count(&mut self, collapsed_leaf_count: i32) {
        self.collapsed_leaf_count = collapsed_leaf_count;
    }

    /// Returns the number of collapsed layers of leaf subnodes in the [`Tree`].
    pub fn collapsed_leaf_count(&self) -> i32 {
        self.collapsed_leaf_count
    }

    fn balance(&mut self, emptied_nodes: HashSet<NodeIndex>) {
        let mut emptied_parents = HashSet::default();
        for parent_index in emptied_nodes.into_iter().filter_map(|ni| ni.parent()) {
            if !self[parent_index].is_parent() {
                continue;
            } else if self[parent_index.left()].is_empty() && self[parent_index.right()].is_empty()
            {
                self[parent_index] = Node::Empty;
                emptied_parents.insert(parent_index);
            } else if self[parent_index.left()].is_empty() {
                self.nodes.swap(parent_index.0, parent_index.right().0);
                self[parent_index.right()] = Node::Empty;
            } else if self[parent_index.right()].is_empty() {
                self.nodes.swap(parent_index.0, parent_index.left().0);
                self[parent_index.left()] = Node::Empty;
            }
        }
        if !emptied_parents.is_empty() {
            self.balance(emptied_parents);
        }
    }

    /// Updates the collapsed state of the node and its parents.
    pub fn node_update_collapsed(&mut self, node_index: NodeIndex) {
        let collapsed = self[node_index].is_collapsed();
        if !collapsed {
            // Recursively notify parent nodes that the leaf has expanded
            let mut parent_index_option = node_index.parent();
            while let Some(parent_index) = parent_index_option {
                parent_index_option = parent_index.parent();

                // Update collapsed leaf count and collapse status
                let left_count = self[parent_index.left()].collapsed_leaf_count();
                let right_count = self[parent_index.right()].collapsed_leaf_count();
                self[parent_index].set_collapsed(false);

                if self[parent_index].is_horizontal() {
                    self[parent_index].set_collapsed_leaf_count(max(left_count, right_count));
                } else {
                    self[parent_index].set_collapsed_leaf_count(left_count + right_count);
                }
            }
            self.set_collapsed(false);
            let root_index = NodeIndex::root();
            self.set_collapsed_leaf_count(self[root_index].collapsed_leaf_count());
        } else {
            // Recursively notify parent nodes that the leaf has collapsed
            let mut parent_index_option = node_index.parent();
            while let Some(parent_index) = parent_index_option {
                parent_index_option = parent_index.parent();

                // Update collapsed leaf count and collapse status
                let left_count = self[parent_index.left()].collapsed_leaf_count();
                let right_count = self[parent_index.right()].collapsed_leaf_count();

                if self[parent_index].is_horizontal() {
                    self[parent_index].set_collapsed_leaf_count(max(left_count, right_count));
                } else {
                    self[parent_index].set_collapsed_leaf_count(left_count + right_count);
                }

                if self[parent_index.left()].is_collapsed()
                    && self[parent_index.right()].is_collapsed()
                {
                    self[parent_index].set_collapsed(true);
                }
            }
            if self.root_node().is_some_and(|root| root.is_collapsed()) {
                self.set_collapsed(true);
                let root_index = NodeIndex::root();
                self.set_collapsed_leaf_count(self[root_index].collapsed_leaf_count());
            }
        }
    }

    /// Find a given tab based on ``predicate``.
    ///
    /// Returns the indices in where that node and tab is in this surface.
    ///
    /// The returned [`NodeIndex`] will always point to a [`Node::Leaf`].
    ///
    /// In case there are several hits, only the first is returned.
    pub fn find_drawer_from(
        &self,
        predicate: impl Fn(&Drawer) -> bool,
    ) -> Option<(NodeIndex, TabIndex)> {
        for (node_index, node) in self.nodes.iter().enumerate() {
            if let Some(tabs) = node.drawers() {
                for (tab_index, tab) in tabs.iter().enumerate() {
                    if predicate(tab) {
                        return Some((node_index.into(), tab_index.into()));
                    }
                }
            };
        }
        None
    }
}

impl<Drawer> Tree<Drawer>
where
    Drawer: PartialEq,
{
    /// Find the given tab.
    ///
    /// Returns in which node and where in that node the tab is.
    ///
    /// The returned [`NodeIndex`] will always point to a [`Node::Leaf`].
    ///
    /// In case there are several hits, only the first is returned.
    pub fn find_drawer(&self, needle_tab: &Drawer) -> Option<(NodeIndex, TabIndex)> {
        self.find_drawer_from(|tab| tab == needle_tab)
    }
}
