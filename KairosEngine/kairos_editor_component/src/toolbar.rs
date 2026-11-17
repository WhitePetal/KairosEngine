//! The editor's top menu bar: host chrome, not a docked panel.
//!
//! It is the production counterpart of the prototype's `toolbar` component
//! (`prototypes/editor-ui/crates/editor-components`): it reads the read-only
//! [`Frame`], draws with the widget kit, and pushes [`Action`]s into [`UiOut`].
//! It never mutates `EditorState` and never performs IO at draw time.
//!
//! Appearance comes from [`ToolBarStyle`] (loaded from
//! `Preferences/Styles/ToolBar.ron` once, when the app builds the toolbar). The
//! tokens are projected onto reusable widget styles
//! (`ButtonStyle` / `MenuStyle`); the drawing itself is done by
//! `kairos_editor_widgets::button` and `kairos_editor_widgets::menu`, so this
//! file only wires labels, actions and the two style projections.

use egui::{Panel, Ui};
use kairos_editor_core::{Action, LayoutAction, PanelId};
use kairos_editor_widgets::color::Color32Ext;
use kairos_editor_widgets::menu;
use kairos_editor_widgets::style::{Style, StyleFile};
use kairos_editor_widgets::ui_image;
use kairos_profile::Category;

use crate::toolbar::style::ToolBarStyle;
use crate::{Frame, UiOut, panels};

pub mod style;

pub struct ToolBar {
    style: ToolBarStyle,
}

impl ToolBar {
    pub fn new(ctx: &egui::Context) -> Self {
        let style = match StyleFile::<ToolBarStyle>::load_or_default() {
            Ok(file) => {
                if ctx.theme() == egui::Theme::Light {
                    file.light
                } else {
                    file.dark
                }
            }
            Err(error) => {
                kairos_log::warn!(target: "kairos_editor", "toolbar style: {error}");
                ToolBarStyle::dark()
            }
        };

        // Runtime images are preloaded once, before the first frame, so the
        // first paint does not show a missing logo.
        for path in style.icons.all() {
            ui_image::preload(ctx, path);
        }

        Self { style }
    }

    pub fn ui(&mut self, ui: &mut Ui, _frame: &Frame, out: &mut UiOut) {
        kairos_profile::scope!(Category::Editor, "Kairos.Editor.ToolBar");

        let style = &self.style;
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
                        "About Kairos",
                        |ui| {
                            if menu::menu_item(ui, "About Kairos").clicked() {
                                todo!("需要实现 UiOut 发送 About 窗口创建逻辑")
                            }
                            menu::menu_separator(ui);
                            if menu::menu_item(ui, "Quit").clicked() {
                                todo!("需要实现 UiOut 发送 Editor程序退出逻辑")
                            }
                        },
                    );

                    menu::menu_button(ui, &menu_style, &button, "Edit", |ui| {
                        if menu::menu_item(ui, "Preferences").clicked() {
                            todo!("需要实现 UiOut 发送 创建Preferences窗口逻辑")
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

    /// `ToolBar::new` seeds the style file on first load; point it at a scratch
    /// directory so the test does not create `Preferences/` in the crate.
    fn use_scratch_styles() {
        static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
        let root = DIR.get_or_init(|| {
            let root = std::env::temp_dir().join(format!(
                "kairos-component-styles-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            let _ = std::fs::create_dir_all(&root);
            root
        });
        kairos_editor_widgets::style::set_style_root(root);
    }

    #[test]
    fn toolbar_draws_the_menu_bar_without_panicking() {
        use_scratch_styles();
        let ctx = Context::default();
        let mut toolbar = ToolBar::new(&ctx);
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
