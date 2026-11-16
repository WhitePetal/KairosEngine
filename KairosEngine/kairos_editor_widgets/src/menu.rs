//! Unity/Tuanjie-style menus: a flat menu bar button plus its dropdown.
//!
//! egui's default menu popup is rounded, has generous margins, and inherits the
//! app's widget colours. The editor's reference skin (Unity/Tuanjie 2022.3) is
//! square, dense and dark: 22 px rows, full-width 1 px separators, and a blue
//! hover highlight. [`MenuStyle`] carries those tokens; the helpers here
//! translate them once into egui's [`Style`] for the popup and then let the
//! standard `ui.button` / `SubMenuButton` widgets do the interaction.
//!
//! Components own the menu *contents* (labels, actions, icons); this module only
//! knows how a menu looks and how it anchors to its trigger.

use egui::{
    Color32, CornerRadius, FontFamily, FontId, Margin, Popup, Response, Shadow, Stroke, Style, Ui,
    UiKind, UiStackInfo, Vec2,
    containers::menu::{MenuConfig, SubMenuButton},
    style::StyleModifier,
};

use crate::button::Button;

/// The right-arrow glyph Unity uses for submenu entries.
pub const SUBMENU_ARROW: &str = "›";

/// Appearance of a menu dropdown. The trigger button's own look is a
/// [`crate::button::ButtonStyle`]; this struct is only the popup.
#[derive(Clone, Debug)]
pub struct MenuStyle {
    pub popup_fill: Color32,
    pub popup_stroke: Stroke,
    pub popup_corner_radius: CornerRadius,
    pub popup_shadow: Shadow,
    /// Margin around the item column. Unity menus keep the separators
    /// full-width, so the horizontal margins are usually 0.
    pub popup_margin: Margin,
    pub popup_min_width: f32,

    pub text: Color32,
    pub text_size: f32,

    pub item_fill: Color32,
    pub item_hover_fill: Color32,
    pub item_active_fill: Color32,
    pub item_corner_radius: CornerRadius,
    /// Horizontal text padding and vertical breathing room per item.
    pub item_padding: Vec2,
    pub item_height: f32,

    pub separator: Color32,
}

impl Default for MenuStyle {
    fn default() -> Self {
        Self {
            // Sampled from the Unity/Tuanjie 2022.3 dark menu in the reference
            // capture: #2d2d2d fill, #4f4f4f separators, ~12 px text.
            popup_fill: Color32::from_rgb(0x2d, 0x2d, 0x2d),
            popup_stroke: Stroke::NONE,
            popup_corner_radius: CornerRadius::ZERO,
            popup_shadow: Shadow {
                offset: [1, 2],
                blur: 6,
                spread: 0,
                color: Color32::from_black_alpha(128),
            },
            popup_margin: Margin {
                left: 0,
                right: 0,
                top: 4,
                bottom: 4,
            },
            popup_min_width: 160.0,
            text: Color32::from_rgb(0xde, 0xde, 0xde),
            text_size: 12.0,
            item_fill: Color32::TRANSPARENT,
            // Unity's dark-skin highlight (--unity-colors-highlight-background).
            item_hover_fill: Color32::from_rgb(0x2c, 0x5d, 0x87),
            item_active_fill: Color32::from_rgb(0x2c, 0x5d, 0x87),
            item_corner_radius: CornerRadius::ZERO,
            item_padding: Vec2::new(11.0, 3.0),
            item_height: 22.0,
            separator: Color32::from_rgb(0x4f, 0x4f, 0x4f),
        }
    }
}

impl MenuStyle {
    /// Apply the menu look to an egui style. Public so a component can also
    /// style its own context menus through `ui.scope(|ui| ..)`.
    pub fn apply(&self, style: &mut Style) {
        style.visuals.window_fill = self.popup_fill;
        style.visuals.window_stroke = self.popup_stroke;
        style.visuals.menu_corner_radius = self.popup_corner_radius;
        style.visuals.popup_shadow = self.popup_shadow;
        style.visuals.button_frame = true;

        style.spacing.menu_margin = self.popup_margin;
        style.spacing.button_padding = self.item_padding;
        style.spacing.item_spacing = Vec2::ZERO;
        style.spacing.interact_size.y = self.item_height;

        style.override_font_id = Some(FontId::new(self.text_size, FontFamily::Proportional));
        style.visuals.override_text_color = Some(self.text);

        for widget in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.fg_stroke = Stroke::new(1.0, self.text);
            widget.bg_stroke = Stroke::NONE;
            widget.corner_radius = self.item_corner_radius;
        }
        style.visuals.widgets.inactive.weak_bg_fill = self.item_fill;
        style.visuals.widgets.hovered.weak_bg_fill = self.item_hover_fill;
        style.visuals.widgets.active.weak_bg_fill = self.item_active_fill;
        style.visuals.widgets.open.weak_bg_fill = self.item_active_fill;

        // The separator widget derives its stroke from the non-interactive
        // widgets; Unity draws it as a 1 px, full-width chrome line.
        style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.separator);

        style.visuals.selection.bg_fill = self.item_active_fill;
        style.visuals.selection.stroke = Stroke::new(1.0, self.text);
    }

    pub fn style_modifier(&self) -> StyleModifier {
        let style = self.clone();
        StyleModifier::new(move |egui_style| style.apply(egui_style))
    }

    pub fn config(&self) -> MenuConfig {
        MenuConfig::new().style(self.style_modifier())
    }
}

/// A menu bar trigger with a text label.
pub fn menu_button<R>(
    ui: &mut Ui,
    style: &MenuStyle,
    button: &crate::button::ButtonStyle,
    label: &str,
    content: impl FnOnce(&mut Ui) -> R,
) -> Response {
    let id = ui.id().with(("kairos-menu-button", label));
    let open = Popup::is_id_open(ui.ctx(), id.with("popup"));
    let response = ui.add(Button::new(label, button).id(id).selected(open));
    show_popup(&response, style, content);
    response
}

/// A menu bar trigger whose label is a runtime image (the engine logo).
pub fn image_menu_button<R>(
    ui: &mut Ui,
    style: &MenuStyle,
    button: &crate::button::ButtonStyle,
    image: &str,
    icon_size: f32,
    label: &str,
    content: impl FnOnce(&mut Ui) -> R,
) -> Response {
    let id = ui.id().with(("kairos-menu-image", label));
    let open = Popup::is_id_open(ui.ctx(), id.with("popup"));
    let response = ui.add(
        Button::new("", button)
            .icon(image, icon_size)
            .id(id)
            .selected(open)
            .tooltip(label),
    );
    show_popup(&response, style, content);
    response
}

/// One dropdown entry.
pub fn menu_item(ui: &mut Ui, label: &str) -> Response {
    ui.button(label)
}

/// One dropdown entry that can be disabled (Unity greys it out).
pub fn menu_item_enabled(ui: &mut Ui, label: &str, enabled: bool) -> Response {
    ui.add_enabled(enabled, egui::Button::new(label))
}

/// A dropdown entry that opens a nested menu.
pub fn submenu<R>(ui: &mut Ui, label: &str, content: impl FnOnce(&mut Ui) -> R) -> Response {
    let button = egui::Button::new(label).right_text(SUBMENU_ARROW);
    SubMenuButton::from_button(button).ui(ui, content).0
}

/// A full-width separator between menu sections.
pub fn menu_separator(ui: &mut Ui) {
    ui.separator();
}

fn show_popup<R>(response: &Response, style: &MenuStyle, content: impl FnOnce(&mut Ui) -> R) {
    let config = style.config();
    Popup::menu(response)
        .style(style.style_modifier())
        .info(UiStackInfo::new(UiKind::Menu).with_tag_value(MenuConfig::MENU_CONFIG_TAG, config))
        .show(|ui| {
            ui.set_min_width(style.popup_min_width);
            content(ui);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_modifier_installs_the_unity_menu_tokens() {
        let menu = MenuStyle::default();
        let mut style = Style::default();
        menu.apply(&mut style);

        assert_eq!(style.visuals.window_fill, menu.popup_fill);
        assert_eq!(style.visuals.menu_corner_radius, CornerRadius::ZERO);
        assert_eq!(style.visuals.popup_shadow, menu.popup_shadow);
        assert_eq!(style.spacing.interact_size.y, menu.item_height);
        assert_eq!(style.spacing.item_spacing, Vec2::ZERO);
        assert_eq!(
            style.visuals.widgets.hovered.weak_bg_fill,
            menu.item_hover_fill
        );
        assert_eq!(
            style.visuals.widgets.noninteractive.bg_stroke.color,
            menu.separator
        );
        assert_eq!(style.visuals.override_text_color, Some(menu.text));
        assert_eq!(
            style.override_font_id.expect("menu font").size,
            menu.text_size
        );
    }

    #[test]
    fn every_menu_entry_reuses_one_style() {
        // The point of the style struct: N items, one instance.
        let menu = MenuStyle::default();
        let mut style = Style::default();
        menu.apply(&mut style);
        for _ in 0..3 {
            assert_eq!(
                style.visuals.widgets.hovered.weak_bg_fill,
                menu.item_hover_fill
            );
        }
    }

    use egui::{Context, Event, PointerButton, Pos2, RawInput, Rect, vec2};

    fn input(time: f64) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 320.0))),
            time: Some(time),
            ..Default::default()
        }
    }

    fn trigger_style() -> crate::button::ButtonStyle {
        crate::button::ButtonStyle::new(crate::text::TextStyle::sized(egui::Color32::WHITE, 12.0))
            .fills(
                egui::Color32::TRANSPARENT,
                egui::Color32::from_rgb(70, 70, 70),
                egui::Color32::from_rgb(70, 70, 70),
            )
            .height(23.0)
    }

    fn pointer(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn clicking_the_bar_opens_the_styled_dropdown_and_hovering_highlights_an_item() {
        let ctx = Context::default();
        let menu = MenuStyle::default();
        let button = trigger_style();
        let mut item_rect = None;

        let mut trigger = None;
        let _ = ctx.run_ui(input(0.0), |ui| {
            trigger = Some(menu_button(ui, &menu, &button, "Window", |ui| {
                item_rect = Some(menu_item(ui, "Console").rect);
            }));
        });
        let trigger_rect = trigger.expect("trigger response").rect;

        let mut press = input(0.016);
        press.events = vec![
            Event::PointerMoved(trigger_rect.center()),
            pointer(trigger_rect.center(), true),
        ];
        let _ = ctx.run_ui(press, |ui| {
            menu_button(ui, &menu, &button, "Window", |ui| {
                item_rect = Some(menu_item(ui, "Console").rect);
            });
        });

        let mut release = input(0.032);
        release.events = vec![
            Event::PointerMoved(trigger_rect.center()),
            pointer(trigger_rect.center(), false),
        ];
        let _ = ctx.run_ui(release, |ui| {
            menu_button(ui, &menu, &button, "Window", |ui| {
                item_rect = Some(menu_item(ui, "Console").rect);
            });
        });

        // egui fades popup areas in over `animation_time`; jump past the fade so
        // the assertion sees the settled, opaque menu colour.
        let opened = ctx.run_ui(input(2.0), |ui| {
            menu_button(ui, &menu, &button, "Window", |ui| {
                item_rect = Some(menu_item(ui, "Console").rect);
            });
        });
        let item = item_rect.expect("menu item laid out while the popup is open");
        assert!(
            opened.shapes.iter().any(|clipped| painted_fill(
                &clipped.shape,
                menu.popup_fill,
                item.center()
            )),
            "the dropdown should paint the Unity menu fill"
        );

        let mut hover = input(2.1);
        hover.events = vec![Event::PointerMoved(item.center())];
        let hovered = ctx.run_ui(hover, |ui| {
            menu_button(ui, &menu, &button, "Window", |ui| {
                item_rect = Some(menu_item(ui, "Console").rect);
            });
        });
        assert!(
            hovered.shapes.iter().any(|clipped| painted_fill(
                &clipped.shape,
                menu.item_hover_fill,
                item.center()
            )),
            "the hovered item should paint the Unity highlight"
        );
    }

    /// egui paints a frame with a shadow as one `Shape::Vec`; look through it.
    fn painted_fill(shape: &egui::Shape, fill: Color32, at: Pos2) -> bool {
        match shape {
            egui::Shape::Rect(rect) => rect.fill == fill && rect.rect.contains(at),
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| painted_fill(shape, fill, at)),
            _ => false,
        }
    }
}
