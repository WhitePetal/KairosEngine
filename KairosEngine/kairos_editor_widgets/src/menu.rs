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

use std::hash::{DefaultHasher, Hash, Hasher};

use egui::{
    Align, Color32, CornerRadius, FontFamily, FontId, LayerId, Layout, Margin, Order, Popup,
    PopupAnchor, PopupCloseBehavior, PopupKind, Pos2, Response, Shadow, Stroke, Style, Ui, UiKind,
    UiStackInfo, Vec2,
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

/// Attach the menu skin to a popup before it is shown.
///
/// egui reads the look from three places that are set independently — the
/// popup's [`StyleModifier`], the [`MenuConfig`] parked in the [`UiStackInfo`]
/// (submenus inherit their style and close behavior from it), and the close
/// behavior on the popup itself — so both entry points have to assemble all
/// three. Assembling them here is what keeps a context menu from growing into a
/// second menu look next to the menu bar dropdown.
fn menu_popup<'a>(
    popup: Popup<'a>,
    style: &MenuStyle,
    close_behavior: PopupCloseBehavior,
) -> Popup<'a> {
    let config = style.config().close_behavior(close_behavior);
    popup
        .close_behavior(close_behavior)
        .style(style.style_modifier())
        .info(UiStackInfo::new(UiKind::Menu).with_tag_value(MenuConfig::MENU_CONFIG_TAG, config))
}

fn show_popup<R>(response: &Response, style: &MenuStyle, content: impl FnOnce(&mut Ui) -> R) {
    // A menu bar dropdown has always closed on the click that picks an item.
    menu_popup(
        Popup::menu(response),
        style,
        PopupCloseBehavior::CloseOnClick,
    )
    .show(|ui| {
        ui.set_min_width(style.popup_min_width);
        content(ui);
    });
}

/// A context menu opened at a screen position instead of at a widget response.
///
/// [`Popup::context_menu`] anchors to a [`Response`], but a tree row's response
/// is consumed while the row is hit-tested, so its caller only has the position
/// of the secondary click left. This opener takes that position and the
/// caller's own `open` flag, and draws the menu with the same [`MenuStyle`] /
/// [`MenuConfig`] as the menu bar dropdowns.
///
/// `salt` must be stable across frames for one menu and different between
/// menus: the derived id is what egui keys the popup's area state by, so a
/// per-frame salt would move and fade the menu in again every frame. Call it
/// from the same `ui` for the same reason: the id is that `ui`'s id salted
/// with `salt`, so a per-row child ui would make a new menu every frame.
///
/// `open` is owned by the caller: the caller sets it to `true` when the
/// secondary click is reported and reads it to keep its trigger state in sync.
/// egui resets it to `false` when the click lands outside the menu or on
/// Escape; a click on one of the items is reported to `content` and leaves the
/// menu open, so the caller also resets `open` when the item it drew performs
/// an action (item activation is the caller's decision, dismissal is not).
///
/// Returns the popup area's response for the frames in which it was shown, so
/// a caller can react to the menu itself without guessing its rect.
pub fn context_menu<R>(
    ui: &mut Ui,
    style: &MenuStyle,
    salt: impl Hash,
    pos: Pos2,
    open: &mut bool,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<Response> {
    // Fold the salt into a number first: egui wants a hashable and debuggable
    // id source, and callers (a row id, a path) should not have to care. Same
    // folding as `TreeView`.
    let mut hasher = DefaultHasher::new();
    salt.hash(&mut hasher);
    let id = ui.make_persistent_id(("kairos-menu-context", hasher.finish()));
    menu_popup(
        Popup::new(
            id,
            ui.ctx().clone(),
            PopupAnchor::Position(pos),
            // The popup area allocates exactly this layer (same order, same id),
            // so the popup bookkeeping lands on the layer it paints into.
            LayerId::new(Order::Foreground, id),
        )
        .kind(PopupKind::Menu)
        .layout(Layout::top_down_justified(Align::Min))
        .gap(0.0)
        .open_bool(open),
        style,
        PopupCloseBehavior::CloseOnClickOutside,
    )
    .show(|ui| {
        ui.set_min_width(style.popup_min_width);
        content(ui);
    })
    .map(|inner| inner.response)
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

    /// The position a row reports for its context menu; the salts below keep
    /// that row's popup id stable across frames.
    fn context_menu_pos() -> Pos2 {
        Pos2::new(200.0, 100.0)
    }

    /// One frame of a caller that owns the open flag and draws a one-item menu.
    ///
    /// Returns the frame output plus the popup and item responses; both are
    /// `None` when the menu is closed, which is also how the tests assert that
    /// no content was built.
    fn context_menu_frame(
        ctx: &Context,
        raw: RawInput,
        menu: &MenuStyle,
        open: &mut bool,
    ) -> (egui::FullOutput, Option<Response>, Option<Response>) {
        let mut popup = None;
        let mut item = None;
        let output = ctx.run_ui(raw, |ui| {
            popup = None;
            item = None;
            popup = context_menu(ui, menu, "row", context_menu_pos(), open, |ui| {
                item = Some(menu_item(ui, "Rename"));
            });
        });
        (output, popup, item)
    }

    #[test]
    fn a_closed_context_menu_paints_nothing() {
        let ctx = Context::default();
        let menu = MenuStyle::default();
        let mut open = false;

        let (output, popup, item) = context_menu_frame(&ctx, input(10.0), &menu, &mut open);

        assert!(popup.is_none(), "a closed menu has no popup response");
        assert!(item.is_none(), "a closed menu must not build its items");
        assert!(
            !output
                .shapes
                .iter()
                .any(|clipped| painted_anywhere(&clipped.shape, menu.popup_fill)),
            "a closed menu must not paint the menu fill"
        );
    }

    #[test]
    fn an_open_context_menu_paints_at_the_reported_position() {
        let ctx = Context::default();
        let menu = MenuStyle::default();
        let mut open = true;

        // egui sizes a new Area invisibly on its first frame and fades it in over
        // `animation_time`; frame at 1.0, then jump the clock so the assertion
        // frame paints the settled, opaque colour.
        let _ = context_menu_frame(&ctx, input(1.0), &menu, &mut open);
        let (output, popup, item) = context_menu_frame(&ctx, input(10.0), &menu, &mut open);

        let popup = popup.expect("an open menu has a popup response");
        let item = item.expect("an open menu lays out its items");
        let pos = context_menu_pos();
        assert!(
            popup.rect.min.distance(pos) < 1.0,
            "the menu should open at the reported position, got {:?} for {pos:?}",
            popup.rect
        );
        assert!(
            popup.rect.contains(item.rect.center()),
            "the item should be laid out inside the popup"
        );
        assert!(
            output.shapes.iter().any(|clipped| painted_fill(
                &clipped.shape,
                menu.popup_fill,
                pos + vec2(1.0, 1.0)
            )),
            "the context menu should paint the Unity menu fill at the click position"
        );
    }

    #[test]
    fn clicking_an_item_reports_it_but_only_an_outside_click_closes_the_menu() {
        let ctx = Context::default();
        let menu = MenuStyle::default();
        let mut open = true;
        let _ = context_menu_frame(&ctx, input(1.0), &menu, &mut open);
        let (_, _, item) = context_menu_frame(&ctx, input(10.0), &menu, &mut open);
        let item_rect = item.expect("item laid out while the menu is open").rect;

        // Press and release on the item. This menu closes on outside clicks only
        // (see `context_menu`), so the click is reported to the content and the
        // flag stays set for the caller to turn the click into an action.
        let mut press = input(10.1);
        press.events = vec![
            Event::PointerMoved(item_rect.center()),
            pointer(item_rect.center(), true),
        ];
        let _ = context_menu_frame(&ctx, press, &menu, &mut open);
        let mut release = input(10.2);
        release.events = vec![
            Event::PointerMoved(item_rect.center()),
            pointer(item_rect.center(), false),
        ];
        let (_, popup, item) = context_menu_frame(&ctx, release, &menu, &mut open);
        assert!(popup.is_some(), "the popup is shown on the click frame");
        assert!(
            item.expect("item laid out on the click frame").clicked(),
            "the item should report the click"
        );
        assert!(open, "an item click must leave the menu open");

        // The next click outside the menu dismisses it. egui writes the
        // caller-owned flag, so nobody else has to reset it.
        let outside = Pos2::new(600.0, 300.0);
        let mut press = input(10.3);
        press.events = vec![Event::PointerMoved(outside), pointer(outside, true)];
        let _ = context_menu_frame(&ctx, press, &menu, &mut open);
        let mut release = input(10.4);
        release.events = vec![Event::PointerMoved(outside), pointer(outside, false)];
        let _ = context_menu_frame(&ctx, release, &menu, &mut open);
        assert!(!open, "an outside click closes the menu and resets open");
    }

    #[test]
    fn a_context_menu_closed_by_its_owner_paints_nothing_on_the_next_frame() {
        let ctx = Context::default();
        let menu = MenuStyle::default();
        let mut open = true;
        let _ = context_menu_frame(&ctx, input(1.0), &menu, &mut open);
        let (open_frame, ..) = context_menu_frame(&ctx, input(10.0), &menu, &mut open);
        assert!(
            open_frame
                .shapes
                .iter()
                .any(|clipped| painted_anywhere(&clipped.shape, menu.popup_fill)),
            "the menu should be painted while it is open"
        );

        // The caller clears the flag itself when an item's action dismisses the
        // menu; the next frame must not paint a stale copy.
        open = false;
        let (closed, popup, item) = context_menu_frame(&ctx, input(11.0), &menu, &mut open);
        assert!(popup.is_none());
        assert!(item.is_none());
        assert!(
            !closed
                .shapes
                .iter()
                .any(|clipped| painted_anywhere(&clipped.shape, menu.popup_fill)),
            "a reopened-from-closed frame must not paint the menu fill"
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

    /// Whether this frame painted any rectangle with `fill`, wherever it lands.
    /// Used to assert the absence of a menu, where there is no position to probe.
    fn painted_anywhere(shape: &egui::Shape, fill: Color32) -> bool {
        match shape {
            egui::Shape::Rect(rect) => rect.fill == fill,
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| painted_anywhere(shape, fill)),
            _ => false,
        }
    }
}
