

#[derive(Debug, Clone)]
pub struct Translations {
    /// Text overrides for buttons in tab context menus.
    pub tab_context_menu: TabContextMenuTranslations,
    /// Text overrides for buttons in windows
    pub leaf: LeafTranslations,
}


#[derive(Debug, Clone)]
pub struct TabContextMenuTranslations {
    /// Button that closes the tab.
    pub close_button: String,
    /// Button that undocks the tab into a new window.
    pub eject_button: String,
}

pub struct LeafTranslations {
    /// Message in the tooltip shown while hovering over a grayed out X button of a leaf
    /// containing non-closable tabs.
    pub close_button_disabled_tooltip: String,
    /// Button that closes the entire window.
    pub close_all_button: String,
    /// Message in the tooltip shown while hovering over an X button of a window.
    /// Used when the secondary buttons are accessible from the context menu.
    pub close_all_button_menu_hint: String,
    /// Message in the tooltip shown while hovering over an X button of a window.
    /// Used when the secondary buttons are accessible using modifiers.
    pub close_all_button_modifier_hint: String,
    /// Message in the tooltip shown while hovering over an X button of a window.
    /// Used when the secondary buttons are accessible using modifiers and from the context menu.
    pub close_all_button_modifier_menu_hint: String,
    /// Message in the tooltip shown while hovering over a grayed out close window button of a window
    /// containing non-closable tabs.
    pub close_all_button_disabled_tooltip: String,
    /// Button that minimizes the window.
    pub minimize_button: String,
    /// Message in the tooltip shown while hovering over a collapse button of a leaf.
    /// Used when the secondary buttons are accessible from the context menu.
    pub minimize_button_menu_hint: String,
    /// Message in the tooltip shown while hovering over a collapse button of a leaf.
    /// Used when the secondary buttons are accessible using modifiers.
    pub minimize_button_modifier_hint: String,
    /// Message in the tooltip shown while hovering over a collapse button of a leaf.
    /// Used when the secondary buttons are accessible using modifiers and from the context menu.
    pub minimize_button_modifier_menu_hint: String,
}