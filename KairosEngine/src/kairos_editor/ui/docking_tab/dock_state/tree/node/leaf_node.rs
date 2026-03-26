use eframe::egui::Rect;

use super::super::TabIndex;


#[derive(Clone, Debug)]
pub struct LeafNode<Drawer> {
    /// The full rectangle - tab bar plus tab body.
    pub rect: Rect,

    /// The tab body rectangle
    pub viewport: Rect,

    /// All the tabs in this node.
    pub tabs: Vec<Drawer>,

    /// The opened Tab
    pub active: TabIndex,

    /// Scroll amount of the tab bar
    pub scroll: f32,

    /// Whether the leaf is collapsed.
    pub collapsed: bool,
}

impl<Drawer> LeafNode<Drawer> {
    /// Create New LeafNode with specified ``tabs``, all other internal values wiil be filled by "nothing" defaults.
    pub fn new(tabs: Vec<Drawer>) -> Self {
        LeafNode {
            rect: Rect::NOTHING,
            viewport: Rect::NOTHING,
            tabs,
            active: TabIndex(0),
            scroll: 0.0,
            collapsed: false
        }
    }

    /// Set the active tab of this [`LeafNode`]
    #[inline]
    pub fn set_active_tab(&mut self, active_tab: impl Into<TabIndex>) {
        let index = active_tab.into();
        self.active = index
    }

    
}