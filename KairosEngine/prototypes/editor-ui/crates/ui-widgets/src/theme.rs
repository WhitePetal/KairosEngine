//! Theme tokens: the *only* place colors, spacing, radii and font sizes come
//! from. No widget in this crate may hard-code a color.
//!
//! ## Delivery decision (explicit, documented)
//!
//! The host installs a `Theme` into `egui::Context` data **once per frame**
//! (`Theme::install`), and widgets read it through `Theme::get(ui)`. This keeps
//! atom signatures identical to plain egui widgets (no extra `&Theme` parameter
//! threading through every builders) while the authoritative style still lives
//! in `core::StyleState` — `ctx.data` here is only a render cache, refreshed
//! from domain state each frame, never written by widgets.
//!
//! Trade-off vs. an explicit `&Theme` parameter: an explicit parameter is
//! mechanically pure and impossible to forget; the cache can silently fall back
//! to `Theme::dark()` if the host forgets to install. We chose the cache because
//! threading `&Theme` through every `impl egui::Widget` (whose `ui(self, ui)`
//! signature is fixed) is impossible without storing the parameter in the
//! widget struct, which then infects every call site. See README "Open questions".

use egui::{Color32, Context, Id, Stroke, TextStyle, Ui, Visuals, vec2};

use crate::icon::{FALLBACK_ICONS, IconId, IconProvider};

fn theme_key() -> Id {
    Id::new("kairos-ui-widgets.theme")
}

/// Appearance tokens. All fields are plain data so the host can derive them
/// from `core::StyleState` (TOML mirror) without touching egui types.
#[derive(Clone, Copy)]
pub struct Theme {
    // colors
    pub bg: Color32,
    pub panel: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub border: Color32,
    pub error: Color32,
    pub success: Color32,

    // spacing
    pub space_xs: f32,
    pub space_sm: f32,
    pub space_md: f32,
    pub space_lg: f32,

    // corner radii
    pub radius_sm: f32,
    pub radius_md: f32,

    // font sizes
    pub font_subtitle: f32,
    pub font_body: f32,
    pub font_caption: f32,

    // metrics
    pub row_height: f32,
    pub icon_sm: f32,
    pub icon_md: f32,

    /// Pluggable icon resolver; `&'static` because themes are installed once
    /// per frame and must be `Copy` + `Send + Sync` for `ctx.data`.
    pub icons: &'static dyn IconProvider,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// The prototype's only built-in theme.
    pub fn dark() -> Self {
        Self {
            bg: Color32::from_rgb(24, 26, 31),
            panel: Color32::from_rgb(30, 33, 40),
            text: Color32::from_rgb(226, 229, 235),
            text_muted: Color32::from_rgb(140, 146, 158),
            accent: Color32::from_rgb(79, 156, 240),
            border: Color32::from_rgb(55, 60, 70),
            error: Color32::from_rgb(232, 92, 92),
            success: Color32::from_rgb(90, 190, 120),
            space_xs: 4.0,
            space_sm: 8.0,
            space_md: 12.0,
            space_lg: 16.0,
            radius_sm: 4.0,
            radius_md: 6.0,
            font_subtitle: 15.0,
            font_body: 13.0,
            font_caption: 11.0,
            row_height: 22.0,
            icon_sm: 12.0,
            icon_md: 16.0,
            icons: &FALLBACK_ICONS,
        }
    }

    /// Read the theme installed for this frame, falling back to the dark theme.
    pub fn get(ui: &Ui) -> Self {
        ui.ctx().data(|d| d.get_temp(theme_key())).unwrap_or_default()
    }

    /// Install (or refresh) the theme for subsequent frames. Call once per frame
    /// from the host, *before* running the UI, when `StyleState` changed.
    pub fn install(self, ctx: &Context) {
        ctx.data_mut(|d| d.insert_temp(theme_key(), self));
        self.apply_egui_style(ctx);
    }

    /// Resolve an icon glyph through the installed provider.
    pub fn icon(&self, id: IconId) -> &'static str {
        self.icons.glyph(id)
    }

    /// Map tokens onto egui's built-in `Visuals`/`Style` so wrapped widgets
    /// (TextEdit, Slider, ComboBox, Window, CollapsingHeader) look themed too.
    pub fn apply_egui_style(&self, ctx: &Context) {
        let mut v = Visuals::dark();
        v.panel_fill = self.panel;
        v.window_fill = self.panel;
        v.extreme_bg_color = self.bg;
        v.faint_bg_color = self.hover_fill();
        v.override_text_color = Some(self.text);
        v.selection.bg_fill = self.selected_fill();
        v.selection.stroke = Stroke::new(1.0, self.text);
        v.hyperlink_color = self.accent;
        v.window_stroke = Stroke::new(1.0, self.border);
        v.window_corner_radius = self.radius_md.into();
        v.menu_corner_radius = self.radius_sm.into();

        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = self.radius_sm.into();
        }
        v.widgets.noninteractive.bg_fill = self.panel;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.border);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.text_muted);

        v.widgets.inactive.bg_fill = self.bg;
        v.widgets.inactive.weak_bg_fill = self.bg;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, self.border);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text);

        v.widgets.hovered.bg_fill = self.hover_fill();
        v.widgets.hovered.weak_bg_fill = self.hover_fill();
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, self.border);
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, self.text);

        v.widgets.active.bg_fill = self.active_fill();
        v.widgets.active.weak_bg_fill = self.active_fill();
        v.widgets.active.bg_stroke = Stroke::new(1.0, self.accent);
        v.widgets.active.fg_stroke = Stroke::new(1.0, self.text);

        v.widgets.open.bg_fill = self.panel;
        v.widgets.open.weak_bg_fill = self.panel;
        v.widgets.open.bg_stroke = Stroke::new(1.0, self.border);
        v.widgets.open.fg_stroke = Stroke::new(1.0, self.text);

        // egui 0.35 keeps a Style per ColorTheme; mutate all of them so the
        // mapped tokens apply regardless of which theme egui picked.
        ctx.all_styles_mut(|style| {
            style.visuals = v.clone();
            style
                .text_styles
                .insert(TextStyle::Body, egui::FontId::proportional(self.font_body));
            style
                .text_styles
                .insert(TextStyle::Button, egui::FontId::proportional(self.font_body));
            style
                .text_styles
                .insert(TextStyle::Small, egui::FontId::proportional(self.font_caption));
            style
                .text_styles
                .insert(TextStyle::Heading, egui::FontId::proportional(self.font_subtitle + 4.0));
            style
                .text_styles
                .insert(TextStyle::Monospace, egui::FontId::monospace(self.font_body));
            style.spacing.item_spacing = vec2(self.space_sm, self.space_xs);
            style.spacing.button_padding = vec2(self.space_sm, self.space_xs);
        });
    }

    /// Background for a hovered row.
    pub fn hover_fill(&self) -> Color32 {
        mix(self.panel, self.text, 0.08)
    }

    /// Background for the selected row.
    pub fn selected_fill(&self) -> Color32 {
        mix(self.panel, self.accent, 0.22)
    }

    /// Background for an "active" toggle button.
    pub fn active_fill(&self) -> Color32 {
        mix(self.panel, self.accent, 0.32)
    }
}

/// Blend two colors in sRGB space (good enough for flat UI tokens; avoids
/// pulling a color crate into a supposedly dependency-light widgets crate).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}
