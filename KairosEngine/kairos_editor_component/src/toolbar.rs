//! The editor's top menu bar: host chrome, not a docked panel.
//!
//! It is the production counterpart of the prototype's `toolbar` component
//! (`prototypes/editor-ui/crates/editor-components`): it reads the read-only
//! [`Frame`], draws with the widget kit, and pushes [`Action`]s into [`UiOut`].
//! It never mutates `EditorState` and never performs IO at draw time.
//!
//! Appearance comes from [`ToolBarStyle`], a shared style the app loads once
//! from `Preferences/Styles/ToolBar.ron` and hands down through [`Frame`]. The
//! toolbar never owns the skin, so a Preferences edit is visible on the next
//! pass. The tokens are projected onto reusable widget styles
//! (`ButtonStyle` / `MenuStyle`); the drawing itself is done by
//! `kairos_editor_widgets::button` and `kairos_editor_widgets::menu`, so this
//! file only wires labels, actions and the two style projections.

use egui::{Panel, Ui};
use kairos_editor_core::{Action, AppAction, LayoutAction, PanelId};
use kairos_editor_widgets::color::ColorExt;
use kairos_editor_widgets::menu;
use kairos_profile::Category;

use crate::{Frame, UiOut, panels};

pub mod style;

#[derive(Default)]
pub struct ToolBar;

impl ToolBar {
    pub fn new() -> Self {
        Self
    }

    pub fn ui(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut) {
        kairos_profile::scope!(Category::Editor, "Kairos.Editor.ToolBar");

        // The style lives in the app's shared-style registry, so a Preferences
        // edit is visible on the next pass without telling the toolbar to
        // reload anything.
        let style = frame.toolbar_style();
        let button = style.button_style();
        let menu_style = style.menu_style();

        Panel::top("toolbar")
            .exact_size(style.height)
            .frame(egui::Frame::NONE.fill(style.background_color.to_egui()))
            .show(ui, |ui| {
                // Unity's menu bar is a row of adjacent buttons: the button's
                // own padding creates the gap, so no item spacing is added.
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                ui.horizontal(|ui| {
                    menu::image_menu_button(
                        ui,
                        &menu_style,
                        &button,
                        &style.icons.about,
                        style.icon_size,
                        "Kairos",
                        |ui| {
                            if menu::menu_item(ui, "About Kairos").clicked() {
                                // An ordinary panel toggle: the layout reducer
                                // opens it, and `ShowPanel` floats a panel that
                                // has no dock tab yet. About is not special-cased
                                // anywhere (that is the point of the panel API).
                                out.push(Action::Layout(LayoutAction::ShowPanel {
                                    panel: panels::ABOUT,
                                }));
                            }
                            menu::menu_separator(ui);
                            if menu::menu_item(ui, "Quit").clicked() {
                                // Host work, not a panel toggle: the reducer
                                // answers this request with an app effect and the
                                // app closes the window at the next frame
                                // boundary. The toolbar never touches the event
                                // loop, so the exit protocol has one home.
                                out.push(Action::App(AppAction::QuitRequested));
                            }
                        },
                    );

                    menu::menu_button(ui, &menu_style, &button, "Edit", |ui| {
                        if menu::menu_item(ui, "Preferences").clicked() {
                            // A panel toggle like About: the layout reducer opens
                            // it, and `ShowPanel` floats a panel that has no dock
                            // tab yet. Preferences is not special-cased.
                            out.push(Action::Layout(LayoutAction::ShowPanel {
                                panel: panels::PREFERENCES,
                            }));
                        }
                    });

                    menu::menu_button(ui, &menu_style, &button, "World", |ui| {
                        if menu::menu_item(ui, "New World").clicked() {
                            todo!("需要实现 UiOut 发送 创建新世界/场景逻辑")
                        }
                    });

                    menu::menu_button(ui, &menu_style, &button, "Window", |ui| {
                        menu::submenu(ui, "General", |ui| {
                            window_item(ui, out, "Inspector", panels::INSPECTOR);
                            window_item(ui, out, "Hierarchy", panels::HIERARCHY);
                            window_item(ui, out, "Project", panels::PROJECT);
                            window_item(ui, out, "Console", panels::CONSOLE);
                            window_item(ui, out, "Scene", panels::SCENE);
                            if menu::menu_item(ui, "Game").clicked() {
                                todo!("需要实现 UiOut 发送 创建 Game窗口逻辑")
                            }
                        });
                        menu::submenu(ui, "Analysis", |ui| {
                            window_item(ui, out, "Profiler", panels::PROFILER);
                        });
                    });
                });
            });
    }
}

/// A Window-menu entry for a built-in panel: shows it through the reducer. The
/// app focuses an existing tab or opens a floating window when the panel has no
/// tab yet, so the toolbar never manipulates the dock itself.
fn window_item(ui: &mut Ui, out: &mut UiOut, label: &str, panel: PanelId) {
    if menu::menu_item(ui, label).clicked() {
        out.push(Action::Layout(LayoutAction::ShowPanel { panel }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Context, Pos2, RawInput, Rect, vec2};
    use kairos_editor_core::EditorState;

    use crate::frame::HostView;

    struct EmptyHost;

    impl HostView for EmptyHost {}

    fn raw_input() -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(960.0, 240.0))),
            ..Default::default()
        }
    }

    #[test]
    fn toolbar_draws_the_menu_bar_without_panicking() {
        let ctx = Context::default();
        let mut toolbar = ToolBar::new();
        let state = EditorState::new();
        let host = EmptyHost;
        let frame = Frame::with_host(&state, &host);
        let mut out = UiOut::new();

        let output = ctx.run_ui(raw_input(), |ui| toolbar.ui(ui, &frame, &mut out));

        // The bar paints at least the menu buttons' labels and the engine logo.
        assert!(!output.shapes.is_empty());
        assert!(out.is_empty(), "the toolbar only emits actions on click");
    }
}
