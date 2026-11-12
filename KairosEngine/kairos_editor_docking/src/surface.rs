use egui::{Pos2, Rect, Vec2};

use crate::{
    tree::{NodeIndex, TabIndex, Tree},
    viewer::TabId,
};

#[derive(Debug, Clone, Default)]
pub struct WindowState {
    /// Last known window position (also the initial position after detaching).
    position: Option<Pos2>,
    /// Last known window size (also the initial size after detaching).
    size: Option<Vec2>,
    minimized: bool,
    maximized: bool,
    /// Rect to restore after un-maximizing/un-minimizing.
    restore: Option<(Pos2, Vec2)>,
    /// Rect to force for exactly one frame (window resize/restore).
    pending_rect: Option<Rect>,
}

impl WindowState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_position(&mut self, position: Pos2) -> &mut Self {
        self.position = Some(position);
        self
    }

    pub fn set_size(&mut self, size: Vec2) -> &mut Self {
        self.size = Some(size);
        self
    }

    pub fn position(&self) -> Option<Pos2> {
        self.position
    }

    pub fn size(&self) -> Option<Vec2> {
        self.size
    }

    pub fn set_rect(&mut self, rect: Rect) {
        self.position = Some(rect.min);
        self.size = Some(rect.size());
        self.restore = None;
    }

    pub fn take_position(&mut self) -> Option<Pos2> {
        self.position.take()
    }

    pub fn take_size(&mut self) -> Option<Vec2> {
        self.size.take()
    }

    pub fn is_minimized(&self) -> bool {
        self.minimized
    }

    pub fn set_minimized(&mut self, minimized: bool) {
        self.minimized = minimized;
    }

    pub fn is_maximized(&self) -> bool {
        self.maximized
    }

    pub fn set_maximized(&mut self, maximized: bool) {
        self.maximized = maximized;
    }

    pub fn restore_rect(&self) -> Option<(Pos2, Vec2)> {
        self.restore
    }

    pub fn set_restore_rect(&mut self, position: Pos2, size: Vec2) {
        self.restore = Some((position, size));
    }

    pub fn clear_restore(&mut self) {
        self.restore = None;
    }

    pub fn set_pending_rect(&mut self, rect: Rect) {
        self.pending_rect = Some(rect);
    }

    pub fn take_pending_rect(&mut self) -> Option<Rect> {
        self.pending_rect.take()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceIndex(pub usize);

impl SurfaceIndex {
    pub const fn main() -> Self {
        Self(0)
    }

    pub fn is_main(self) -> bool {
        self.0 == Self::main().0
    }
}

#[derive(Clone)]
pub enum Surface<Id: TabId> {
    Main(Tree<Id>),
    Window(Tree<Id>, WindowState),
}

impl<Id: TabId> Surface<Id> {
    pub fn get_tree(&self) -> &Tree<Id> {
        match self {
            Surface::Main(tree) => tree,
            Surface::Window(tree, _) => tree,
        }
    }

    pub fn get_tree_mut(&mut self) -> &mut Tree<Id> {
        match self {
            Surface::Main(tree) => tree,
            Surface::Window(tree, _) => tree,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.get_tree().is_empty()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodePath {
    pub surface: SurfaceIndex,
    pub node: NodeIndex,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabPath {
    pub node: NodePath,
    pub tab: TabIndex,
}
