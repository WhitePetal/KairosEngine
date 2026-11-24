//! Toolbar appearance tokens.
//!
//! Kept as pure data so it can be persisted to
//! `Preferences/Styles/ToolBar.ron` (dark + light) and edited from the
//! Preferences panel's shared-style page. The component projects these tokens
//! onto the widget-layer [`kairos_editor_widgets::button::ButtonStyle`] and
//! [`kairos_editor_widgets::menu::MenuStyle`] at draw time; the widgets never
//! read this file themselves.

use egui::{CornerRadius, Margin, Vec2};
use kairos_color::Color;
use kairos_editor_widgets::button::ButtonStyle;
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::menu::MenuStyle;
use kairos_editor_widgets::style::Style;
use kairos_editor_widgets::text::TextStyle;
use kairos_editor_widgets::ui_image;
use serde::{Deserialize, Serialize};

use crate::shared_style::SharedStyle;
use crate::style_row::StyleRow;

/// Appearance of the editor's top menu bar and its dropdown menus.
#[derive(Clone, PartialEq, Serialize, Deserialize, StyleRow)]
#[SharedStyle(TOOLBAR = "toolbar", title = "Toolbar")]
pub struct ToolBarStyle {
    pub icons: ToolBarIconPaths,

    pub background_color: Color,
    pub text_color: Color,
    /// Hover / pressed fill of a menu bar button. Matches the Console panel's
    /// toolbar hover (`ConsoleStyle::toggle_bg`) so both bars read as one skin.
    #[serde(default = "default_hover_color")]
    pub hover_color: Color,

    #[serde(default = "default_text_size")]
    pub text_size: f32,
    pub height: f32,
    /// Corner radius of the toolbar buttons. The reference skin is square.
    pub corner_radius: f32,
    /// Horizontal text padding of a menu bar button.
    #[serde(default = "default_button_padding")]
    pub button_padding: f32,
    /// Size of the engine logo in the About button.
    #[serde(default = "default_icon_size")]
    pub icon_size: f32,

    #[serde(default)]
    pub menu: ToolBarMenuStyle,
}

/// Dropdown appearance. Values mirror Unity/Tuanjie 2022.3 dark: a #2d2d2d
/// square popup, 22 px rows, 1 px #4f4f4f separators and the Unity blue
/// selection highlight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, StyleRow)]
pub struct ToolBarMenuStyle {
    pub background_color: Color,
    pub item_hover_color: Color,
    pub text_color: Color,
    pub separator_color: Color,
    pub item_height: f32,
    /// Vertical padding between the popup edge and the item column.
    pub padding: f32,
    /// Left/right padding of one menu item.
    pub item_padding: f32,
    pub min_width: f32,
}

impl Default for ToolBarMenuStyle {
    fn default() -> Self {
        Self::dark()
    }
}

impl ToolBarMenuStyle {
    pub fn dark() -> Self {
        Self {
            background_color: Color::from_srgb8([0x2d, 0x2d, 0x2d, 255]),
            item_hover_color: Color::from_srgb8([0x2c, 0x5d, 0x87, 255]),
            text_color: Color::from_srgb8([0xde, 0xde, 0xde, 255]),
            separator_color: Color::from_srgb8([0x4f, 0x4f, 0x4f, 255]),
            item_height: 22.0,
            padding: 4.0,
            item_padding: 11.0,
            min_width: 160.0,
        }
    }

    pub fn light() -> Self {
        Self {
            background_color: Color::from_srgb8([0xf0, 0xf0, 0xf0, 255]),
            item_hover_color: Color::from_srgb8([0x3a, 0x72, 0xb0, 255]),
            text_color: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            separator_color: Color::from_srgb8([0xc0, 0xc0, 0xc0, 255]),
            ..Self::dark()
        }
    }
}

impl Style for ToolBarStyle {
    const FILE: &'static str = "ToolBar.ron";

    fn dark() -> Self {
        Self {
            icons: ToolBarIconPaths::dark(),

            background_color: Color::from_srgb8([0x19, 0x19, 0x19, 255]),
            text_color: Color::from_srgb8([0xfe, 0xfe, 0xfe, 255]),
            hover_color: Color::from_srgb8([0x46, 0x46, 0x46, 255]),

            text_size: 12.0,
            height: 23.0,
            corner_radius: 0.0,
            button_padding: 10.0,
            icon_size: 16.0,

            menu: ToolBarMenuStyle::dark(),
        }
    }

    fn light() -> Self {
        Self {
            icons: ToolBarIconPaths::light(),

            background_color: Color::from_srgb8([0xc8, 0xc8, 0xc8, 255]),
            text_color: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            hover_color: Color::from_srgb8([0xb4, 0xb4, 0xb4, 255]),

            text_size: 12.0,
            height: 23.0,
            corner_radius: 0.0,
            button_padding: 10.0,
            icon_size: 16.0,

            menu: ToolBarMenuStyle::light(),
        }
    }
}

impl ToolBarStyle {
    /// Project the toolbar tokens onto the reusable button widget.
    pub fn button_style(&self) -> ButtonStyle {
        ButtonStyle::new(TextStyle::sized(self.text_color.to_egui(), self.text_size))
            .fills(
                egui::Color32::TRANSPARENT,
                self.hover_color.to_egui(),
                self.hover_color.to_egui(),
            )
            .corner_radius(CornerRadius::same(self.corner_radius as u8))
            .padding(Vec2::new(self.button_padding, 0.0))
            .height(self.height)
    }

    /// Project the dropdown tokens onto the reusable menu widget.
    pub fn menu_style(&self) -> MenuStyle {
        MenuStyle {
            popup_fill: self.menu.background_color.to_egui(),
            popup_margin: Margin {
                left: 0,
                right: 0,
                top: self.menu.padding as i8,
                bottom: self.menu.padding as i8,
            },
            popup_min_width: self.menu.min_width,
            text: self.menu.text_color.to_egui(),
            text_size: self.text_size,
            item_hover_fill: self.menu.item_hover_color.to_egui(),
            item_active_fill: self.menu.item_hover_color.to_egui(),
            item_corner_radius: CornerRadius::ZERO,
            item_padding: Vec2::new(self.menu.item_padding, 3.0),
            item_height: self.menu.item_height,
            separator: self.menu.separator_color.to_egui(),
            ..MenuStyle::default()
        }
    }
}

impl SharedStyle for ToolBarStyle {
    fn preload(&self, ctx: &egui::Context) {
        for path in self.icons.all() {
            ui_image::preload(ctx, path);
        }
    }
}

fn default_hover_color() -> Color {
    Color::from_srgb8([0x46, 0x46, 0x46, 255])
}

fn default_text_size() -> f32 {
    12.0
}

fn default_button_padding() -> f32 {
    10.0
}

fn default_icon_size() -> f32 {
    16.0
}

#[derive(Clone, PartialEq, Serialize, Deserialize, StyleRow)]
pub struct ToolBarIconPaths {
    pub about: String,
}

impl ToolBarIconPaths {
    pub fn dark() -> Self {
        Self {
            about: "Preferences/Textures/engine_icon.png".to_owned(),
        }
    }

    pub fn light() -> Self {
        Self {
            about: "Preferences/Textures/engine_icon.png".to_owned(),
        }
    }

    pub fn all(&self) -> [&str; 1] {
        [&self.about]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_style_projects_square_hover_buttons() {
        let style = ToolBarStyle::dark();
        let button = style.button_style();
        assert_eq!(button.corner_radius, CornerRadius::ZERO);
        assert_eq!(button.hover_fill, style.hover_color.to_egui());
        assert_eq!(button.height, style.height);
        assert_eq!(button.text.text_color(), style.text_color.to_egui());
        assert_eq!(
            button.text.font_id(&egui::Style::default()).size,
            style.text_size
        );
    }

    #[test]
    fn toolbar_style_projects_a_unity_menu() {
        let style = ToolBarStyle::dark();
        let menu = style.menu_style();
        assert_eq!(menu.popup_fill, style.menu.background_color.to_egui());
        assert_eq!(menu.popup_corner_radius, CornerRadius::ZERO);
        assert_eq!(menu.item_height, style.menu.item_height);
        assert_eq!(menu.item_hover_fill, style.menu.item_hover_color.to_egui());
        assert_eq!(menu.separator, style.menu.separator_color.to_egui());
        assert_eq!(menu.popup_margin.left, 0);
        assert_eq!(menu.popup_margin.right, 0);
    }

    #[test]
    fn old_style_files_still_load_with_defaults() {
        // The file written before the menu/hover tokens existed.
        let old = r##"(
            dark: (
                icons: (about: "Preferences/Textures/engine_icon.png"),
                background_color: "#000000",
                text_color: "#fefefe",
                height: 23.0,
                corner_radius: 0.0,
            ),
            light: (
                icons: (about: "Preferences/Textures/engine_icon.png"),
                background_color: "#000000",
                text_color: "#fefefe",
                height: 23.0,
                corner_radius: 0.0,
            ),
        )"##;
        let dir = std::env::temp_dir().join(format!(
            "kairos-toolbar-style-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("ToolBar.ron");
        std::fs::write(&path, old).expect("old style file");
        let file = kairos_editor_widgets::style::StyleFile::<ToolBarStyle>::load_from(&path)
            .expect("old style file should stay loadable");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(file.dark.hover_color, default_hover_color());
        assert_eq!(file.dark.menu, ToolBarMenuStyle::dark());
        assert_eq!(file.light.text_size, default_text_size());
    }
}
