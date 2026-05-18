use eframe::egui::{self, Id, Ui, WidgetText};

use crate::kairos_editor::ui::{Drawer, Messager, docking_tab::{dock_state::tree::NodeIndex, styles::TabStyle, surfaces::SurfaceIndex}};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnCloseResponse {
    /// Closes the tab.
    Close,
    /// Focuses on the tab.
    Focus,
    /// Ignores the close request.
    Ignore,
}

pub trait TabDrawer {
    type Tab;

    fn title(&self, tab: &mut Self::Tab, drawers: &Vec<Box<dyn Drawer>>) -> WidgetText;

    fn ui(
        &mut self, 
        ui: &mut Ui, 
        tab: &mut Self::Tab, 
        ctx: &eframe::egui::Context, 
        frame: &mut eframe::Frame, 
        messager: &mut Messager,
        drawers: &Vec<Box<dyn Drawer>>
    );

    /// Content inside the context menu shown when the tab is right-clicked.
    ///
    /// `_surface` and `_node` specify which [`Surface`](super::surfaces::Surface) and [`Node`](super::dock_state::tree::node::Node)
    /// that this particular context menu belongs to.
    fn context_menu(
        &mut self,
        _ui: &mut Ui,
        _tab: &mut Self::Tab,
        _surface: SurfaceIndex,
        _node: NodeIndex
    ) {
    }

    /// Unique ID for this tab.
    ///
    /// If not implemented, uses tab title text as an ID source.
    fn id(&self, tab: &mut Self::Tab, drawers: &Vec<Box<dyn Drawer>>) -> Id {
        Id::new(self.title(tab, drawers).text())
    }

    /// Called after each tab button is shown, so you can add a tooltip, check for clicks, etc.
    fn on_tab_button(&mut self, _tab: &mut Self::Tab, _response: &egui::Response) {
    }

    fn on_close(&mut self, _tab: &mut Self::Tab, _messager: &mut Messager, _drawers: &Vec<Box<dyn Drawer>>) -> OnCloseResponse {
        OnCloseResponse::Close
    }

    /// Returns `true` if the user of your app should be able to close a given `_tab`.
    ///
    /// By default, `true` is always returned.
    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        true
    }

    /// Returns `true` if the user of your app should be able to close a given `_tab`.
    ///
    /// By default, `true` is always returned.
    #[deprecated = "Use the `TabViewer::is_closeable` function instead."]
    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        true
    }

    /// This is called every frame after [`ui`](Self::ui) is called, if the `_tab` is active.
    ///
    /// Returns `true` if the tab should be forced to close, `false` otherwise.
    ///
    /// In the event this function returns true the tab will be removed without calling `on_close`.
    fn force_close(&mut self, _tab: &mut Self::Tab) -> bool {
        false
    }

    /// This is called when the add button is pressed.
    ///
    /// `_surface` and `_node` specify which [`Surface`](super::surfaces::Surface) and on which
    /// [`Node`](super::dock_state::tree::node::Node) this particular add button was pressed.
    fn on_add(&mut self, _surface: SurfaceIndex, _node: NodeIndex) {
    }

    /// Called when the rectangle of the tab content changes.
    ///
    /// This can happen when the window is resized, panels are docked or undocked,
    /// or when the layout of the dock area is changed in any way that affects
    /// the available space for the tab content.
    ///
    /// This is useful for tabs that need to adjust their content based on the
    /// available space.
    fn on_rect_changed(&mut self, _tab: &mut Self::Tab) {
    }

    /// Content of the popup under the add button. Useful for selecting what type of tab to add.
    ///
    /// This requires that [`DockArea::show_add_buttons`](super::DockArea::show_add_buttons) and
    /// [`DockArea::show_add_popup`](crate::DockArea::show_add_popup) are set to `true`.
    fn add_popup(&mut self, _ui: &mut Ui, _surface: SurfaceIndex, _node: NodeIndex) {
    }

    /// Sets custom style for given tab.
    fn tab_style_override(&self, _tab: &Self::Tab, _global_style: &TabStyle) -> Option<TabStyle> {
        None
    }

    /// Specifies a tab's ability to be shown in a window.
    ///
    /// Returns `false` if this tab should never be turned into a window.
    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        true
    }

    /// Whether the tab body will be cleared with the color specified in
    /// [`TabBarStyle::bg_fill`](super::styles::TabBarStyle::bg_fill).
    fn clear_background(&self, _tab: &Self::Tab) -> bool {
        true
    }

    /// Returns `true` if the horizontal and vertical scroll bars will be shown for `tab`.
    ///
    /// By default, both scroll bars are shown.
    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [true, true]
    }
}