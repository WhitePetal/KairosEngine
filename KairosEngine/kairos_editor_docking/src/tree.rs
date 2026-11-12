use std::{ops::{Index, IndexMut, Range}, slice::IterMut};

use egui::Rect;

use crate::{split::{Fraction, Split}, tab::{TabState}};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabIndex(pub usize);

#[derive(Clone)]
pub struct LeafNode {
    /// tab bar + body
    pub rect: Rect,
    /// body rect
    pub viewport: Rect,
    pub tabs: Vec<TabState>,
    pub active: TabIndex,
    pub scroll: f32,
}

impl LeafNode {
    pub fn new(tabs: Vec<TabState>) -> Self {
        Self { rect: Rect::NOTHING, viewport: Rect::NOTHING, tabs, active: TabIndex(0), scroll: 0.0 }
    }

    pub fn remove_tab(&mut self, tab_index: impl Into<TabIndex>) -> TabState {
        let index = tab_index.into();
        if index <= self.active {
            self.active.0 = self.active.0.saturating_sub(1)
        }
        self.tabs.remove(index.0)
    }

    pub fn insert_tab(&mut self, tab_index: impl Into<TabIndex>, tab: TabState) {
        let tab_index = tab_index.into();
        self.tabs.insert(tab_index.0, tab);
        self.active = tab_index;
    }

    pub fn append_tab(&mut self, tab: TabState) {
        self.active = TabIndex(self.tabs.len());
        self.tabs.push(tab);
    }

    pub fn set_rect(&mut self, new_rect: Rect) {
        self.rect = new_rect
    }
}

#[derive(Clone)]
pub struct SplitNode {
    pub rect: Rect,
    pub fraction: Fraction,
}

impl SplitNode {
    pub fn new(rect: Rect, fraction: Fraction) -> Self {
        Self { rect, fraction }
    }

    pub fn set_rect(&mut self, new_rect: Rect) {
        self.rect = new_rect
    }
}

#[derive(Clone)]
pub enum Node {
    /// Empty Node
    Empty,

    /// Contains the actual tabs.
    Leaf(LeafNode),

    /// Parent node in the vertical orientation.
    Vertical(SplitNode),

    /// Parent node in the horizontal orientation.
    Horizontal(SplitNode),
}

impl Node {
    pub fn leaf(tab: TabState) -> Self {
        Self::Leaf(LeafNode::new(vec![tab]))
    }

    pub fn leaf_with(tabs: Vec<TabState>) -> Self {
        Self::Leaf(LeafNode::new(tabs))
    }

    fn split(&mut self, split: Split, fraction: Fraction) -> Self {
        let rect = Rect::NOTHING;
        let src = match split {
            Split::Left | Split::Right => Node::Horizontal(SplitNode::new(rect, fraction)),
            Split::Top | Split::Bottom => Node::Vertical(SplitNode::new(rect, fraction))
        };
        std::mem::replace(self, src)
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    pub fn is_horizontal(&self) -> bool {
        matches!(self, Self::Horizontal(..))
    }

    pub fn is_vertical(&self) -> bool {
        matches!(self, Self::Vertical(..))
    }

    pub fn is_parent(&self) -> bool {
        self.is_horizontal() | self.is_vertical()
    }

    pub fn is_leaf(&self) -> bool {
        matches!(self, Self::Leaf(_))
    }

    pub fn tabs_count(&self) -> usize {
        match self {
            Node::Leaf(leaf_node) => leaf_node.tabs.len(),
            _ => 0
        }
    }

    pub fn remove_tab(&mut self, tab_index: TabIndex) -> Option<TabState> {
        match self {
            Node::Leaf(leaf_node) => Some(leaf_node.remove_tab(tab_index)),
            _ => None,
        }
    }

    pub fn insert_tab(&mut self, index: TabIndex, tab: TabState) {
        match self {
            Node::Leaf(leaf_node) => leaf_node.insert_tab(index, tab),
            _ => ()
        }
    }

    pub fn append_tab(&mut self, tab: TabState) {
        match self {
            Node::Leaf(leaf_node) => leaf_node.append_tab(tab),
            _ => ()
        }
    }

    pub fn set_rect(&mut self, new_rect: Rect) {
        match self {
            Node::Empty => (),
            Node::Leaf(leaf_node) => leaf_node.set_rect(new_rect),
            Node::Vertical(split_node) | Node::Horizontal(split_node) => split_node.set_rect(new_rect),
        }
    }

    pub fn rect(&self) -> Option<Rect> {
        match self {
            Node::Empty => None,
            Node::Leaf(leaf_node) => Some(leaf_node.rect),
            Node::Vertical(split_node) | Node::Horizontal(split_node) => Some(split_node.rect),
        }
    }

    pub fn get_leaf_mut(&mut self) -> Option<&mut LeafNode> {
        match self {
            Node::Leaf(leaf_node) => Some(leaf_node),
            _ => None,
        }
    }

    pub fn tabs_mut(&mut self) -> Option<&mut Vec<TabState>> {
        match self {
            Node::Leaf(leaf_node) => Some(&mut leaf_node.tabs),
            _ => None
        }
    }

    pub fn iter_tabs_mut(&mut self) -> impl Iterator<Item = &mut TabState> {
        match self.tabs_mut() {
            Some(tabs) => tabs.iter_mut(),
            None => IterMut::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeIndex(pub usize);

impl From<usize> for NodeIndex {
    #[inline(always)]
    fn from(value: usize) -> Self {
        NodeIndex(value)
    }
}

impl NodeIndex {
    pub fn root() -> Self {
        Self(0)
    }

    #[inline(always)]
    pub fn level(self) -> usize {
        (usize::BITS - (self.0 + 1).leading_zeros()) as usize
    }

    #[inline(always)]
    pub fn left(self) -> Self {
        Self(self.0 * 2 + 1)
    }

    #[inline(always)]
    pub fn right(self) -> Self {
        Self(self.0 * 2 + 2)
    }

    #[inline]
    pub fn parent(self) -> Option<Self> {
        if self.0 > 0 {
            Some(Self((self.0 - 1) / 2))
        } else {
            None
        }
    }

    pub fn is_left(self) -> bool {
        self.0 % 2 != 0
    }

    pub fn is_right(self) -> bool {
        self.0 % 2 == 0
    }

    pub fn children_at(self, level: usize) -> Range<usize> {
        let base = 1 << level;
        let s = (self.0 + 1) * base - 1;
        let e = (self.0 + 2) * base - 1;
        s..e
    }

    pub fn children_right(self, level: usize) -> Range<usize> {
        let base = 1 << level;
        let s = (self.0 + 1) * base + (base / 2) - 1;
        let e = (self.0 + 2) * base - 1;
        s..e
    }

    pub fn children_left(self, level: usize) -> Range<usize> {
        let base = 1 << level;
        let s = (self.0 + 1) * base - 1;
        let e = (self.0 + 1) * base + (base / 2) - 1;
        s..e
    }
}

#[derive(Clone)]
pub struct Tree {
    pub nodes: Vec<Node>,
    focused_node: Option<NodeIndex>,
}

impl Index<NodeIndex> for Tree {
    type Output = Node;

    fn index(&self, index: NodeIndex) -> &Self::Output {
        &self.nodes[index.0]
    }
}

impl IndexMut<NodeIndex> for Tree {
    fn index_mut(&mut self, index: NodeIndex) -> &mut Self::Output {
        &mut self.nodes[index.0]
    }
}

impl Tree {
    pub fn new(tabs: Vec<TabState>) -> Self {
        let root = Node::leaf_with(tabs);
        Self { nodes: vec![root], focused_node: None }
    }

    pub fn root_node_mut(&mut self) -> Option<&mut Node> {
        self.nodes.first_mut()
    }

    pub fn split(&mut self, parent: NodeIndex, split: Split, fraction: Fraction, new: Node) -> [NodeIndex; 2] {
        let old = self.nodes[parent.0].split(split, fraction);

        let index = self.nodes.iter().rposition(|n| !n.is_empty()).unwrap_or(0);
        let level = NodeIndex(index).level();
        self.nodes.resize_with((1 << (level + 1)) - 1, || Node::Empty);

        let index = match split {
            Split::Left | Split::Top => [parent.right(), parent.left()],
            Split::Right | Split::Bottom => [parent.left(), parent.right()]
        };

        if old.is_parent() {
            let levels_to_move = NodeIndex(self.nodes.len()).level() - index[0].level();

            for level in (1..levels_to_move).rev() {
                let old_start = parent.children_at(level).start;
                let new_start = index[0].children_at(level).start;

                let len = 1 << level;

                let (old_range, new_range) = {
                    let (first_part, second_part) = self.nodes.split_at_mut(new_start);
                    (
                        &mut first_part[old_start..old_start + len],
                        &mut second_part[..len],
                    )
                };
                old_range.swap_with_slice(new_range);
            }
        }

        self.nodes[index[0].0] = old;
        self.nodes[index[1].0] = new;

        self.focused_node = Some(index[1]);

        index
    }

    pub fn split_right(
        &mut self,
        parent: NodeIndex,
        fraction: Fraction,
        tabs: Vec<TabState>,
    ) -> [NodeIndex; 2] {
        self.split(parent, Split::Right, fraction, Node::leaf_with(tabs))
    }

    #[inline(always)]
    pub fn split_left(
        &mut self,
        parent: NodeIndex,
        fraction: Fraction,
        tabs: Vec<TabState>,
    ) -> [NodeIndex; 2] {
        self.split(parent, Split::Left, fraction, Node::leaf_with(tabs))
    }

    pub fn split_bottom(
        &mut self,
        parent: NodeIndex,
        fraction: Fraction,
        tabs: Vec<TabState>,
    ) -> [NodeIndex; 2] {
        self.split(parent, Split::Bottom, fraction, Node::leaf_with(tabs))
    }

    pub fn split_top(
        &mut self,
        parent: NodeIndex,
        fraction: Fraction,
        tabs: Vec<TabState>,
    ) -> [NodeIndex; 2] {
        self.split(parent, Split::Top, fraction, Node::leaf_with(tabs))
    }

    pub fn split_with_tabs(
        &mut self,
        parent: NodeIndex,
        split: Split,
        fraction: Fraction,
        tabs: Vec<TabState>,
    ) -> [NodeIndex; 2] {
        self.split(parent, split, fraction, Node::leaf_with(tabs))
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn remove_leaf(&mut self, node_index: NodeIndex) {
        if self.is_empty() || !self[node_index].is_leaf() {
            return
        }

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
                if self.nodes.get(node_index.0).is_some_and(|node_index| node_index.is_leaf()) {
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
                    self.nodes[dst] = std::mem::replace(&mut self.nodes[src], Node::Empty)
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

        while let Some(last_index) = self.nodes.len().checked_sub(1).map(NodeIndex) {
            if self[last_index].is_empty() && last_index.parent().is_some_and(|pi| !self[pi].is_parent()) {
                self.nodes.pop();
            } else {
                break;
            }
        }
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

    pub fn breadth_first_index_iter(&self) -> std::iter::Map<Range<usize>, fn (usize) -> NodeIndex> {
        (0..self.nodes.len()).map(NodeIndex)
    }

    pub fn focused_leaf(&self) -> Option<NodeIndex> {
        self.focused_node
    }

    pub fn tabs_count(&self) -> usize {
        let mut count = 0;
        for node in self.nodes.iter() {
            if let Node::Leaf(leaf) = node {
                count += leaf.tabs.iter().len();
            }
        }
        count
    }

    pub fn remove_tab(&mut self, (node_index, tab_index): (NodeIndex, TabIndex)) -> Option<TabState> {
        let node = &mut self[node_index];
        let tab = node.remove_tab(tab_index);
        if node.tabs_count() == 0 {
            self.remove_leaf(node_index);
        }
        tab
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn set_focused_node(&mut self, node_index: NodeIndex) {
        self.focused_node = self.nodes.get(node_index.0).filter(|node| node.is_leaf()).map(|_| node_index)
    }
}
