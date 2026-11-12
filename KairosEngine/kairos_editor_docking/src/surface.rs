use egui::{Pos2, Rect, Vec2};

use crate::{tree::{NodeIndex, TabIndex, Tree}};

#[derive(Debug, Clone)]
pub struct WindowState {
    rect: Option<Rect>,
    dragged: bool,
    position: Option<Pos2>,
    size: Option<Vec2>,
    minimized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            rect: None,
            dragged: false,
            position: None,
            size: None,
            minimized: false,
        }
    }
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
pub enum Surface {
    Main(Tree),
    Window(Tree, WindowState)
}

impl Surface {
    pub fn get_tree(&self) -> &Tree {
        match self {
            Surface::Main(tree) => tree,
            Surface::Window(tree, _) => tree,
        }
    }

    pub fn get_tree_mut(&mut self) -> &mut Tree {
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
