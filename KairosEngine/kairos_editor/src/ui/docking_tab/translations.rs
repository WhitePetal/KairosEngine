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

#[derive(Debug, Clone)]
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

impl Translations {
    /// Default English translations.
    pub fn english() -> Self {
        Self {
            tab_context_menu: TabContextMenuTranslations::english(),
            leaf: LeafTranslations::english(),
        }
    }

    /// Default Chinese translations.
    pub fn chinese() -> Self {
        Self {
            tab_context_menu: TabContextMenuTranslations::chinese(),
            leaf: LeafTranslations::chinese(),
        }
    }
}

impl TabContextMenuTranslations {
    /// Default English translations.
    pub fn english() -> Self {
        Self {
            close_button: String::from("Close"),
            eject_button: String::from("Eject"),
        }
    }

    // Default Chinese translations.
    pub fn chinese() -> Self {
        Self {
            close_button: String::from("关闭"),
            eject_button: String::from("弹出"),
        }
    }
}

impl LeafTranslations {
    /// Default English translations.
    pub fn english() -> Self {
        Self {
            close_button_disabled_tooltip: String::from("This leaf contains non-closable tabs."),
            close_all_button: String::from("Close window"),
            close_all_button_menu_hint: String::from("Right click to close this window."),
            close_all_button_modifier_hint: String::from(
                "Press modifier keys (Shift by default) to close this window.",
            ),
            close_all_button_modifier_menu_hint: String::from(
                "Press modifier keys (Shift by default) or right click to close this window.",
            ),
            close_all_button_disabled_tooltip: String::from(
                "This window contains non-closable tabs.",
            ),
            minimize_button: String::from("Minimize window"),
            minimize_button_menu_hint: String::from("Right click to minimize this window."),
            minimize_button_modifier_hint: String::from(
                "Press modifier keys (Shift by default) tho minimize this window.",
            ),
            minimize_button_modifier_menu_hint: String::from(
                "Press modifier keys (Shift by default) or right click to minimize this window.",
            ),
        }
    }

    /// Default Chinese translations.
    pub fn chinese() -> Self {
        Self {
            close_button_disabled_tooltip: String::from("此叶节点包含不可关闭的标签。"),
            close_all_button: String::from("关闭窗口"),
            close_all_button_menu_hint: String::from("右键单击以关闭此窗口。"),
            close_all_button_modifier_hint: String::from("按修饰键（默认Shift）关闭此窗口。"),
            close_all_button_modifier_menu_hint: String::from(
                "按修饰键（默认Shift）或右键单击以关闭此窗口。",
            ),
            close_all_button_disabled_tooltip: String::from("此窗口包含不可关闭的标签。"),
            minimize_button: String::from("最小化窗口"),
            minimize_button_menu_hint: String::from("右键单击以最小化此窗口。"),
            minimize_button_modifier_hint: String::from("按修饰键（默认Shift）最小化此窗口。"),
            minimize_button_modifier_menu_hint: String::from(
                "按修饰键（默认Shift）或右键单击以最小化此窗口。",
            ),
        }
    }
}
