use eframe::{egui::{Context, Pos2, Rect}};

use crate::kairos_editor::ui::docking_tab::{dock_state::tree::{NodeIndex, TabIndex}, styles::Style, surfaces::SurfaceIndex};


#[derive(Debug, Clone)]
pub(super) enum TreeComponent {
    Surface(SurfaceIndex),
    Node(SurfaceIndex, NodeIndex),
    Tab(SurfaceIndex, NodeIndex, TabIndex)
}

#[derive(Debug, Clone)]
pub(super) struct HoverData {
    pub rect: Rect,

    pub dst: TreeComponent,

    pub tab: Option<Rect>,
}

/// Specifies the location of a tab on the tree, used when moving tabs.
#[derive(Debug, Clone)]
pub(super) struct DragData {
    pub src: TreeComponent,
    pub rect: Rect
}

#[derive(Debug, Clone)]
pub(super) struct DragDropState {
    pub hover: HoverData, 
    pub drag: DragData,
    pub pointer: Pos2,
    /// Is some when the pointer is over rect, f64 holds the time when the lock was last active.
    pub locked: Option<f64>,
}

impl DragDropState {
    pub(super) fn is_locked(&self, style: &Style, ctx: &Context) -> bool {
        match self.locked.as_ref() {
            Some(lock_time) => {
                let elapsed = ctx.input(|i| (i.time - lock_time) as f32);
                ctx.request_repaint();
                elapsed < style.overlay.feel.max_preference_time
            }
            None => false
        }
    }
}

impl TreeComponent {
    pub(super) fn node_address(&self) -> (SurfaceIndex, Option<NodeIndex>) {
        match *self {
            TreeComponent::Surface(surface_index) => (surface_index, None),
            TreeComponent::Node(surface_index, node_index) => (surface_index, Some(node_index)),
            TreeComponent::Tab(surface_index, node_index, tab_index) => (surface_index, Some(node_index)),
        }
    }
}