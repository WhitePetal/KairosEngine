//! About panel: the editor's identity card.
//!
//! This is the 0.2.0 rewrite of the legacy `kairos_editor` `AboutWindow`
//! (`kairos_editor/src/ui/about_window.rs`), not a port: the old window was five
//! `ui.label` lines whose style was a TOML read with `fs` and written back
//! every frame the Preferences page was open. The review that produced this
//! shape — every gap and every defect of the old window, with `file:line`
//! evidence — is `docs/plan/editor-ui-architecture/about-panel.md`.
//!
//! # What it is
//!
//! A normal `Panel`: the dock owns the tab or floating window, the app decides
//! where a new one opens (`LayoutAction::ShowPanel`), and the body only reads
//! `Frame` and pushes `Action`s into `UiOut`. About opens as a floating
//! window because `ShowPanel` floats a panel that has no tab yet; dragging it
//! into the dock makes it a tab, and both close paths
//! (`LayoutAction::ClosePanel`, the tab's x) converge on the same action.
//!
//! # What it draws
//!
//! Product identity (name, version, description, license, links) plus the
//! properties of the running binary (Debug/Release, arch-os). The identity is
//! **injected** (`EditorInfo`) by the composition root: this crate must not
//! depend on `kairos_editor_app`, and a version literal compiled into a
//! component is exactly how the old window ended up showing a different version
//! than the title bar.
//!
//! Links and the copy button use egui's platform-output commands
//! (`OutputCommand::{OpenUrl, CopyText}`); the host already routes those to
//! `egui_winit` (`handle_platform_output`), so no `Action`/`Effect` is added
//! for "open a browser". The body itself performs no IO and logs nothing: it may
//! run twice per frame (`max_passes = 2`) and a discarded pass must not have
//! side effects.

pub mod style;

use egui::{
    Align, Color32, Context, CornerRadius, Hyperlink, Id, Label, Layout, Margin, Sense, Ui,
    WidgetText, vec2,
};
use kairos_editor_core::{Action, LayoutAction, PanelId, StyleValue};
use kairos_editor_widgets::button::Button;
use kairos_editor_widgets::color::Color32Ext;
use kairos_editor_widgets::divider::Divider;
use kairos_editor_widgets::style::{Style, StyleFile};
use kairos_editor_widgets::text::TextStyle;
use kairos_editor_widgets::ui_image;

use crate::frame::{Frame, UiOut};
use crate::panel::{EditorInfo, Panel, PanelContext, PanelDef};
use crate::skin::{PanelSkin, StyleFields};

use self::style::AboutPanelStyle;
use super::ABOUT;

/// Stable id of the Close button.
///
/// Fixed rather than `Ui`-scoped so a headless test can click the real widget
/// (`Context::read_response`) instead of a guessed coordinate. About is a
/// singleton in practice (`ShowPanel` focuses the existing instance), so two
/// live instances sharing this id is not reachable from the UI.
fn close_button_id() -> Id {
    Id::new("kairos.about.close")
}

/// Stable id of the Copy button; see [`close_button_id`].
fn copy_button_id() -> Id {
    Id::new("kairos.about.copy")
}

/// The About panel. Holds the injected identity and the loaded skin; no other
/// state, so nothing to persist and nothing to reset on reopen.
pub struct AboutPanel {
    info: EditorInfo,
    style: AboutPanelStyle,
}

/// Catalog entry: the host builds the panel through this when it opens, with
/// the identity the composition root injected.
pub(super) const DEF: PanelDef = PanelDef::new(ABOUT, create);

fn create(context: &PanelContext) -> Box<dyn Panel> {
    Box::new(AboutPanel::new(context.editor))
}

impl AboutPanel {
    /// Build the panel for one product identity. The catalog factory above is
    /// the only caller; nothing else constructs it.
    pub fn new(info: EditorInfo) -> Self {
        Self {
            info,
            style: AboutPanelStyle::default(),
        }
    }
}

impl Panel for AboutPanel {
    fn kind(&self) -> PanelId {
        ABOUT
    }

    fn icon(&self) -> Option<&str> {
        Some(self.style.icons.tab.as_str())
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        format!("About {}", self.info.name).into()
    }

    /// The About skin, editable from Preferences like any other panel's.
    fn skin(&self) -> Option<PanelSkin> {
        Some(PanelSkin {
            panel: ABOUT,
            title: "About",
            file: AboutPanelStyle::FILE,
            fields: self.style.style_fields(),
        })
    }

    fn set_skin_field(&mut self, path: &str, value: &StyleValue) -> bool {
        self.style.set_style_field(path, value)
    }

    fn save_skin(&self, ctx: &Context) -> Result<(), String> {
        crate::skin::save_skin::<AboutPanelStyle>(&self.style, ctx.theme())
    }

    /// Load the skin and warm the two images. A missing or broken skin file is
    /// logged and falls back to the code defaults: the old window treated it as
    /// a fatal error and took the editor down with it.
    fn init(&mut self, ctx: &Context) {
        match StyleFile::<AboutPanelStyle>::load_or_default() {
            Ok(file) => {
                self.style = if ctx.theme() == egui::Theme::Light {
                    file.light
                } else {
                    file.dark
                };
            }
            Err(error) => {
                kairos_log::warn!(target: "kairos_editor", "about style: {error}");
            }
        }

        for path in self.style.icons.all() {
            ui_image::preload(ctx, path);
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &Frame, out: &mut UiOut) {
        let style = &self.style;

        // The app switches text selection off for editor chrome, but the dock
        // builds each tab body's `Ui` from the context's style
        // (`Docker::show_body`), so that switch does not reach a panel. This body
        // is chrome too: a stray drag must not select "Credits" or the version.
        ui.style_mut().interaction.selectable_labels = false;

        // The dock paints a body colour behind every tab; a floating window uses
        // the panel's own. Painting it here keeps the two cases identical.
        ui.painter()
            .rect_filled(ui.max_rect(), CornerRadius::ZERO, style.background.to_egui());

        egui::Frame::NONE
            .inner_margin(Margin::same(style.padding.round() as i8))
            .show(ui, |ui| {
                header(ui, style, &self.info);
                rule(ui, style);

                let has_license = !self.info.license.is_empty();
                if has_license {
                    row(ui, style, "License", self.info.license);
                }

                if has_links(&self.info) {
                    // A rule with nothing above it would just double the header
                    // rule; only separate the two blocks when both exist.
                    if has_license {
                        rule(ui, style);
                    }
                    link_row(
                        ui,
                        style,
                        "Repository",
                        "Project page",
                        self.info.repository,
                    );
                    link_row(ui, style, "Discussions", "Discussions", self.info.discussions);
                    link_row(ui, style, "Credits", "Thanks", self.info.credits);
                }

                ui.add_space(style.spacing);
                footer(ui, style, &self.info, out);
            });
    }
}

fn has_links(info: &EditorInfo) -> bool {
    !(info.repository.is_empty() && info.discussions.is_empty() && info.credits.is_empty())
}

/// Product mark, name, version, build and the one-line description.
fn header(ui: &mut Ui, style: &AboutPanelStyle, info: &EditorInfo) {
    ui.horizontal_top(|ui| {
        let (rect, _) =
            ui.allocate_exact_size(vec2(style.logo_size, style.logo_size), Sense::hover());
        // The artwork is pre-coloured for its skin, so the tint only has to
        // leave it alone.
        ui_image::paint(ui, rect, &style.icons.logo, Color32::WHITE);
        ui.add_space(style.logo_gap);

        ui.vertical(|ui| {
            TextStyle::sized(style.title.to_egui(), style.title_size)
                .strong()
                .label(ui, info.name);
            if !info.version.is_empty() {
                TextStyle::sized(style.subtitle.to_egui(), style.subtitle_size)
                    .label(ui, format!("Version {}", info.version));
            }
            // The running binary's properties, dimmer than the version: a bug
            // report needs them, the headline does not.
            TextStyle::sized(style.label.to_egui(), style.body_size - 1.0)
                .label(ui, EditorInfo::build_label());
        });
    });

    if !info.description.is_empty() {
        ui.add_space(style.spacing);
        TextStyle::sized(style.body.to_egui(), style.body_size).label(ui, info.description);
    }
}

/// A hairline rule between sections, with the style's breathing room.
fn rule(ui: &mut Ui, style: &AboutPanelStyle) {
    ui.add_space(style.section_spacing);
    ui.add(Divider::horizontal().color(style.divider.to_egui()));
    ui.add_space(style.section_spacing);
}

/// One key/value identity row. The key column is a fixed width so the values
/// line up without a grid.
fn row(ui: &mut Ui, style: &AboutPanelStyle, key: &str, value: &str) {
    let key_style = TextStyle::sized(style.label.to_egui(), style.body_size);
    let value_style = TextStyle::sized(style.value.to_egui(), style.body_size);

    ui.horizontal(|ui| {
        ui.add_sized(
            vec2(style.label_width, style.row_height),
            Label::new(key_style.rich(key)),
        );
        ui.add(Label::new(value_style.rich(value)).truncate());
    });
}

/// One link row. An empty URL hides the whole row, so a host that knows fewer
/// URLs does not draw empty labels.
fn link_row(ui: &mut Ui, style: &AboutPanelStyle, key: &str, label: &str, url: &str) {
    if url.is_empty() {
        return;
    }

    let key_style = TextStyle::sized(style.label.to_egui(), style.body_size);
    let link_style = TextStyle::sized(style.link.to_egui(), style.body_size);

    ui.horizontal(|ui| {
        ui.add_sized(
            vec2(style.label_width, style.row_height),
            Label::new(key_style.rich(key)),
        );
        // `Hyperlink` paints with the visuals' link colour and emits
        // `OutputCommand::OpenUrl` on click (handled by the host's winit glue).
        ui.scope(|ui| {
            ui.visuals_mut().hyperlink_color = style.link.to_egui();
            ui.add(Hyperlink::from_label_and_url(link_style.rich(label), url));
        });
    });
}

/// Copy + Close, right-aligned at the end of the body.
fn footer(ui: &mut Ui, style: &AboutPanelStyle, info: &EditorInfo, out: &mut UiOut) {
    let button = style.button_style();

    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        // Right-to-left layout: the first widget added sits on the right edge.
        if ui
            .add(Button::new("Close", &button).id(close_button_id()))
            .clicked()
        {
            out.push(Action::Layout(LayoutAction::ClosePanel { panel: ABOUT }));
        }

        if ui
            .add(Button::new("Copy version info", &button).id(copy_button_id()))
            .clicked()
        {
            ui.ctx().copy_text(info.banner());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::HostView;
    use kairos_editor_core::EditorState;

    struct NoHost;

    impl HostView for NoHost {}

    fn info() -> EditorInfo {
        EditorInfo {
            name: "KairosEngine",
            version: "9.9.9-test",
            description: "a test engine",
            license: "MIT OR Apache-2.0",
            repository: "https://example.invalid/kairos",
            discussions: "",
            credits: "",
        }
    }

    fn panel() -> AboutPanel {
        AboutPanel::new(info())
    }

    fn raw_input(events: Vec<egui::Event>) -> egui::RawInput {
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            vec2(520.0, 380.0),
        ));
        input.events = events;
        input
    }

    /// Run one pass of the panel body, exactly as the dock's viewer does.
    fn draw(
        ctx: &Context,
        panel: &mut AboutPanel,
        events: Vec<egui::Event>,
    ) -> (egui::FullOutput, UiOut) {
        let state = EditorState::new();
        let host = NoHost;
        let frame = Frame::with_host(&state, &host);
        let mut out = UiOut::new();
        let output = ctx.run_ui(raw_input(events), |ui| panel.ui(ui, &frame, &mut out));
        (output, out)
    }

    /// Every text run the pass painted, flattened out of the shape tree.
    fn painted_text(output: &egui::FullOutput) -> Vec<String> {
        fn flatten(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => out.push(text.galley.text().to_owned()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        flatten(shape, out);
                    }
                }
                _ => {}
            }
        }

        let mut out = Vec::new();
        for clipped in &output.shapes {
            flatten(&clipped.shape, &mut out);
        }
        out
    }

    /// Rect of a painted run of text, for clicking a widget egui gives an
    /// auto-generated id (the hyperlink).
    fn text_rect(output: &egui::FullOutput, needle: &str) -> Option<egui::Rect> {
        fn find(shape: &egui::Shape, needle: &str, out: &mut Option<egui::Rect>) {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == needle => {
                    *out = Some(egui::Rect::from_min_size(text.pos, text.galley.size()));
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        find(shape, needle, out);
                    }
                }
                _ => {}
            }
        }

        let mut rect = None;
        for clipped in &output.shapes {
            find(&clipped.shape, needle, &mut rect);
        }
        rect
    }

    fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Press and release on a widget across passes, the way a real click arrives.
    fn click(ctx: &Context, panel: &mut AboutPanel, id: Id) -> UiOut {
        let rect = ctx
            .read_response(id)
            .expect("the widget was laid out in a previous pass")
            .rect;

        draw(
            ctx,
            panel,
            vec![egui::Event::PointerMoved(rect.center())],
        );
        draw(ctx, panel, vec![pointer(rect.center(), true)]);
        let (_, out) = draw(ctx, panel, vec![pointer(rect.center(), false)]);
        out
    }

    /// The footer actions must read as buttons, not labels: filled and
    /// outlined. They sit on the panel background, where a text-only button is
    /// indistinguishable from the "License" row's value.
    #[test]
    fn the_footer_buttons_are_filled_and_outlined() {
        let ctx = Context::default();
        let mut panel = panel();
        let (output, _) = draw(&ctx, &mut panel, Vec::new());

        // The panel starts on the dark skin (code default) until `init` loads a
        // file; that is the skin these shapes were painted with.
        let style = AboutPanelStyle::default();
        let fill = style.button_fill.to_egui();
        let stroke = egui::Stroke::new(style.button_border_width, style.button_border.to_egui());

        let mut filled = 0;
        let mut outlined = 0;
        for clipped in &output.shapes {
            if let egui::Shape::Rect(shape) = &clipped.shape {
                if shape.fill == fill {
                    filled += 1;
                }
                if shape.stroke == stroke {
                    outlined += 1;
                }
            }
        }

        assert!(filled >= 2, "Copy and Close paint their fill: {filled}");
        assert!(outlined >= 2, "Copy and Close paint their border: {outlined}");
    }

    #[test]
    fn the_body_shows_the_injected_identity() {
        let ctx = Context::default();
        let mut panel = panel();
        let (output, out) = draw(&ctx, &mut panel, Vec::new());
        let text = painted_text(&output).join("\n");

        assert!(text.contains("KairosEngine"), "{text}");
        assert!(text.contains("Version 9.9.9-test"), "{text}");
        assert!(text.contains("MIT OR Apache-2.0"), "{text}");
        assert!(text.contains(&EditorInfo::build_label()), "{text}");
        assert!(text.contains("Project page"), "{text}");
        // An empty URL hides its row instead of drawing a dead label.
        assert!(!text.contains("Discussions"), "{text}");
        assert!(out.is_empty(), "a plain pass emits nothing");
    }

    /// The body's natural size, measured through the space it asks egui for.
    ///
    /// `Ui::min_rect` cannot be used: a fresh `Ui` starts out the size of its
    /// parent, so it reports the canvas, not the content.
    fn body_size(panel: &mut AboutPanel, width: f32) -> egui::Vec2 {
        let ctx = Context::default();
        let state = EditorState::new();
        let host = NoHost;
        let frame = Frame::with_host(&state, &host);
        let mut out = UiOut::new();
        let mut size = egui::Vec2::ZERO;

        let _ = ctx.run_ui(raw_input(Vec::new()), |ui| {
            let allocated = ui.allocate_ui(vec2(width, 1.0), |ui| {
                panel.ui(ui, &frame, &mut out);
            });
            size = allocated.response.rect.size();
        });

        size
    }

    /// The default floating rect is 42% of the editor (537x344 for a 1280x820
    /// window) and the dock's title bar takes 28 px, leaving 316 px of body. The
    /// body must fit: a body taller than its window opens on a scrollbar, and the
    /// first thing it hides is the Close button.
    #[test]
    fn the_body_fits_the_default_floating_window() {
        // Measure at the narrowest default width (a small editor) and at the
        // typical one; the body must fit both.
        for width in [460.0, 537.0] {
            let size = body_size(&mut panel(), width);
            assert!(
                size.y <= 316.0,
                "About body is {} px tall at {width} px wide, the default floating window has 316",
                size.y
            );
        }
    }

    #[test]
    fn the_title_carries_the_product_name() {
        let panel = panel();
        let state = EditorState::new();
        let host = NoHost;
        let frame = Frame::with_host(&state, &host);

        assert_eq!(panel.title(&frame).text(), "About KairosEngine");
    }

    #[test]
    fn close_asks_the_layout_to_close_the_panel() {
        let ctx = Context::default();
        let mut panel = panel();
        let _ = draw(&ctx, &mut panel, Vec::new());

        let out = click(&ctx, &mut panel, close_button_id());

        assert_eq!(
            out.as_slice(),
            &[Action::Layout(LayoutAction::ClosePanel { panel: ABOUT })]
        );
    }

    /// The copy button is host work through egui's platform output, not a new
    /// `Action`: the app already forwards `platform_output` to egui-winit.
    #[test]
    fn copy_puts_the_version_banner_on_the_clipboard() {
        let ctx = Context::default();
        let mut panel = panel();
        let _ = draw(&ctx, &mut panel, Vec::new());

        let rect = ctx
            .read_response(copy_button_id())
            .expect("copy button laid out")
            .rect;
        draw(
            &ctx,
            &mut panel,
            vec![egui::Event::PointerMoved(rect.center())],
        );
        draw(&ctx, &mut panel, vec![pointer(rect.center(), true)]);
        let (output, out) = draw(&ctx, &mut panel, vec![pointer(rect.center(), false)]);

        let copied = output
            .platform_output
            .commands
            .iter()
            .find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .expect("CopyText command");

        assert_eq!(copied, info().banner());
        assert!(copied.contains("KairosEngine 9.9.9-test"), "{copied}");
        assert!(out.is_empty(), "copying is not a domain action");
    }

    /// Links are host work through egui's platform output as well: the panel
    /// never opens a browser itself, it asks the platform-output channel the
    /// host already forwards.
    #[test]
    fn a_link_opens_through_the_platform_output() {
        let ctx = Context::default();
        let mut panel = panel();
        let (output, _) = draw(&ctx, &mut panel, Vec::new());
        let link = text_rect(&output, "Project page").expect("repository link painted");

        draw(
            &ctx,
            &mut panel,
            vec![egui::Event::PointerMoved(link.center())],
        );
        draw(&ctx, &mut panel, vec![pointer(link.center(), true)]);
        let (output, out) = draw(&ctx, &mut panel, vec![pointer(link.center(), false)]);

        let opened = output
            .platform_output
            .commands
            .iter()
            .find_map(|command| match command {
                egui::OutputCommand::OpenUrl(open) => Some(open.url.clone()),
                _ => None,
            });

        assert_eq!(opened.as_deref(), Some("https://example.invalid/kairos"));
        assert!(out.is_empty(), "opening a link is not a domain action");
    }
}
