use egui::Rect;

use super::super::TabIndex;

#[derive(Clone, Debug)]
pub struct LeafNode<Drawer> {
    /// The full rectangle - tab bar plus tab body.
    pub rect: Rect,

    /// The tab body rectangle
    pub viewport: Rect,

    /// All the tabs in this node.
    pub drawers: Vec<Drawer>,

    /// The opened Tab
    pub active: TabIndex,

    /// Scroll amount of the tab bar
    pub scroll: f32,

    /// Whether the leaf is collapsed.
    pub collapsed: bool,
}

impl<Drawer> LeafNode<Drawer> {
    /// Create New LeafNode with specified ``tabs``, all other internal values wiil be filled by "nothing" defaults.
    pub fn new(drawers: Vec<Drawer>) -> Self {
        LeafNode {
            rect: Rect::NOTHING,
            viewport: Rect::NOTHING,
            drawers,
            active: TabIndex(0),
            scroll: 0.0,
            collapsed: false,
        }
    }

    /// Set the active tab of this [`LeafNode`]
    #[inline]
    pub fn set_active_tab(&mut self, active_tab: impl Into<TabIndex>) {
        let index = active_tab.into();
        self.active = index
    }

    /// Set the area this [`LeafNode`] Occupies on screen.
    pub fn set_rect(&mut self, new_rect: Rect) {
        self.rect = new_rect;
    }

    /// Get the length of tab list in this [`LeafNode`]
    pub fn len(&self) -> usize {
        self.drawers.len()
    }

    /// Returns `true` wehn the [`LeafNode`] contains no tabs
    pub fn is_empty(&self) -> bool {
        self.drawers.is_empty()
    }

    /// Get a [`Rect`] representing the area this [`LeafNode`] occupies on screen.
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// Get immutable access to the ``Tab``s of this [`LeafNode`]
    #[inline]
    pub fn drawers(&self) -> &[Drawer] {
        &self.drawers
    }

    #[inline]
    pub fn drawers_count(&self) -> usize {
        self.drawers.len()
    }

    /// Get mutable access to the ``Tab``s of this [`LeafNode`]
    #[inline]
    pub fn drawers_mut(&mut self) -> &mut [Drawer] {
        &mut self.drawers
    }

    /// Append a ``Tab`` to the end of this [`LeafNode`]s tab list
    ///
    /// This will also focus the added tab.
    #[track_caller]
    #[inline]
    pub fn append_drawer(&mut self, tab: Drawer) {
        self.active = TabIndex(self.drawers.len());
        self.drawers.push(tab);
    }

    /// Insert a ``Tab`` to this [`LeafNode`]s tab list at the specified [`TabIndex`]
    ///
    /// This will also focus the added tab.
    ///
    /// # Paincs
    ///
    /// if ``tab_index`` exceeds the leaf's tab list length
    #[track_caller]
    #[inline]
    pub fn insert_drawer(&mut self, tab_index: impl Into<TabIndex>, tab: Drawer) {
        let tab_index = tab_index.into();
        self.drawers.insert(tab_index.0, tab);
        self.active = tab_index;
    }

    /// Remove a ``Tab`` to this [`LeafNode`]s tab list at the specified [`TabIndex`].
    ///
    /// This will also focus the added tab.
    ///
    /// # Paincs
    ///
    /// if ``tab_index`` is out of bounds for the tab list
    #[inline]
    pub fn remove_drawer(&mut self, tab_index: impl Into<TabIndex>) -> Option<Drawer> {
        let index = tab_index.into();
        if index <= self.active {
            self.active.0 = self.active.0.saturating_sub(1)
        }
        Some(self.drawers.remove(index.0))
    }

    /// Removes all tabs for which `predicate` returns `false`
    pub fn retain_drawers<F>(&mut self, predicated: F)
    where
        F: FnMut(&mut Drawer) -> bool,
    {
        self.drawers.retain_mut(predicated);
    }

    /// Return the area and tab which is currently representing this [`LeafNode`]
    ///
    /// This may return ``None`` if the leaf conmtains 0 tabs.
    #[inline]
    pub fn active_focused(&mut self) -> Option<(Rect, &mut Drawer)> {
        self.drawers
            .get_mut(self.active.0)
            .map(|tab| (self.viewport, tab))
    }
}
