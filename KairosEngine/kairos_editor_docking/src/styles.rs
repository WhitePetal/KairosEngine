use egui::{Color32, CornerRadius, Margin, Shadow, Stroke};

#[derive(Clone)]
pub struct TabBodyStyle {
    pub inner_margin: Margin,
    pub stroke: Stroke,
    pub corner_radius: CornerRadius,
    pub bg_fill: Color32,
}

impl Default for TabBodyStyle {
    fn default() -> Self {
        Self {
            inner_margin: Margin::same(0),
            stroke: Stroke::default(),
            corner_radius: CornerRadius::default(),
            bg_fill: Color32::from_rgb(54, 54, 54),
        }
    }
}

#[derive(Clone)]
pub struct TabInteractionStyle {
    pub headerline_color: Color32,
    /// Thickness of the coloured line on top of the tab.
    pub headerline_width: f32,
    pub outline_color: Color32,
    pub corner_radius: CornerRadius,
    pub bg_fill: Color32,
    pub text_color: Color32,
    /// Tint multiplied into the tab icon, so artwork can follow the tab state.
    /// The dark-skin icons are pre-coloured; white keeps them unchanged.
    pub icon_tint: Color32,
}

impl Default for TabInteractionStyle {
    fn default() -> Self {
        Self {
            headerline_color: Color32::TRANSPARENT,
            headerline_width: 2.0,
            outline_color: Color32::TRANSPARENT,
            corner_radius: CornerRadius {
                nw: 4,
                ne: 4,
                sw: 0,
                se: 0,
            },
            bg_fill: Color32::from_rgb(54, 54, 54),
            text_color: Color32::from_rgb(196, 196, 196),
            icon_tint: Color32::WHITE,
        }
    }
}

#[derive(Clone)]
pub struct TabStyle {
    pub active: TabInteractionStyle,
    pub inactive: TabInteractionStyle,
    pub focused: TabInteractionStyle,
    pub hovered: TabInteractionStyle,
    pub tab_body: TabBodyStyle,
    pub hline_below_active_tab_name: bool,
    pub spacing: f32,
    pub minimum_width: Option<f32>,
    /// Square size of a tab's icon in points. `0.0` hides icons.
    pub icon_size: f32,
    /// Gap between the icon and the title, in points.
    pub icon_spacing: f32,
    /// Padding between a tab's edge and its icon/title block. Live tabs and
    /// drag previews share the tab-header helpers in `docker.rs`, so they
    /// cannot drift.
    pub horizontal_padding: f32,
}

impl Default for TabStyle {
    fn default() -> Self {
        Self {
            active: TabInteractionStyle::default(),
            inactive: TabInteractionStyle {
                bg_fill: Color32::from_rgb(44, 44, 44),
                ..Default::default()
            },
            focused: TabInteractionStyle {
                headerline_color: Color32::from_rgb(51, 105, 146),
                ..Default::default()
            },
            hovered: TabInteractionStyle {
                headerline_color: Color32::from_rgb(51, 105, 146),
                ..Default::default()
            },
            tab_body: TabBodyStyle::default(),
            hline_below_active_tab_name: false,
            spacing: 0.0,
            minimum_width: None,
            icon_size: 16.0,
            icon_spacing: 6.0,
            horizontal_padding: 8.0,
        }
    }
}

#[derive(Clone)]
pub struct OverlayFeel {
    /// range is `0.0..=1.0`.
    pub window_drop_coverage: f32,
    /// range is `0.0..=1.0`.
    pub center_drop_coverage: f32,
    /// The amount of time windows should stay faded despite not needing to, prevents quick mouse movements from causing flashing.
    pub fade_hold_time: f32,
    /// Amount of time the overlay waits before dropping a preference it may have for a node.
    pub max_preference_time: f32,
}

impl Default for OverlayFeel {
    fn default() -> Self {
        Self {
            window_drop_coverage: 0.5,
            center_drop_coverage: 0.25,
            fade_hold_time: 0.2,
            max_preference_time: 0.3,
        }
    }
}

#[derive(Clone)]
pub struct OverlayStyle {
    pub selection_color: Color32,
    pub selection_stroke_width: f32,
    pub surface_fade_opacity: f32,
    pub feel: OverlayFeel,
}

impl Default for OverlayStyle {
    fn default() -> Self {
        Self {
            selection_color: Color32::from_rgb(0, 191, 255).linear_multiply(0.5),
            selection_stroke_width: 1.0,
            surface_fade_opacity: 0.1,
            feel: Default::default(),
        }
    }
}

#[derive(Clone)]
pub struct SeparatorStyle {
    pub width: f32,
    pub extra_interact_width: f32,
    pub extra: f32,
    pub color_idle: Color32,
    pub color_hovered: Color32,
    pub color_dragged: Color32,
    /// Width of the soft shadow cast into the panels on both sides of the
    /// split, so the divider reads as a shadowed gutter instead of a line.
    pub shadow_width: f32,
    /// Shadow colour at its densest (next to the divider); it fades outwards.
    pub shadow_color: Color32,
}

impl Default for SeparatorStyle {
    fn default() -> Self {
        Self {
            width: 2.0,
            extra_interact_width: 2.0,
            extra: 175.0,
            color_idle: Color32::BLACK,
            color_hovered: Color32::BLACK,
            color_dragged: Color32::from_rgb(126, 126, 126),
            shadow_width: 4.0,
            shadow_color: Color32::TRANSPARENT,
        }
    }
}

/// The vertical divider drawn between two adjacent tabs.
///
/// Unity-like tabs are separated by a 1px dark line plus a soft drop shadow
/// that fades over the next tab, so the bar reads as layered tabs instead of
/// one flat strip.
#[derive(Clone)]
pub struct TabSeparatorStyle {
    /// Width of the dark divider line.
    pub line_width: f32,
    /// Width of the soft shadow cast to the right of the divider line.
    pub shadow_width: f32,
    /// Colour of the divider line itself.
    pub line_color: Color32,
    /// Shadow colour at its densest (right next to the line); it fades out
    /// towards `shadow_width`.
    pub shadow_color: Color32,
}

impl Default for TabSeparatorStyle {
    fn default() -> Self {
        Self {
            line_width: 1.0,
            shadow_width: 3.0,
            line_color: Color32::from_rgb(16, 16, 16),
            shadow_color: Color32::from_black_alpha(70),
        }
    }
}

#[derive(Clone)]
pub struct TabBarStyle {
    pub bg_fill: Color32,
    pub height: f32,
    pub inner_margin: Margin,
    pub show_scroll_bar_on_overflow: bool,
    pub corner_radius: CornerRadius,
    pub separator: TabSeparatorStyle,
    pub fill_tab_bar: bool,
}

impl Default for TabBarStyle {
    fn default() -> Self {
        Self {
            bg_fill: Color32::from_rgb(44, 44, 44),
            height: 24.0,
            inner_margin: Margin::ZERO,
            show_scroll_bar_on_overflow: true,
            corner_radius: CornerRadius::default(),
            separator: TabSeparatorStyle::default(),
            fill_tab_bar: false,
        }
    }
}

/// Chrome of a detached/floating dock window.
///
/// The window is drawn by `Docker` itself (not egui's default title bar) so the
/// title bar, shadow and system buttons are consistent on every platform.
#[derive(Clone)]
pub struct WindowStyle {
    pub bg_fill: Color32,
    pub title_bg_fill: Color32,
    pub border: Stroke,
    pub shadow: Shadow,
    pub title_height: f32,
    /// macOS-style traffic lights: circle radius, center spacing, left margin.
    pub traffic_light_radius: f32,
    pub traffic_light_spacing: f32,
    pub traffic_light_margin: f32,
    pub close_color: Color32,
    pub minimize_color: Color32,
    pub maximize_color: Color32,
    pub glyph_color: Color32,
}

impl Default for WindowStyle {
    fn default() -> Self {
        Self {
            bg_fill: Color32::from_rgb(49, 49, 49),
            title_bg_fill: Color32::from_rgb(57, 57, 57),
            border: Stroke::new(1.0, Color32::from_rgb(24, 24, 24)),
            shadow: Shadow {
                offset: [3, 3],
                blur: 10,
                spread: 1,
                color: Color32::from_black_alpha(120),
            },
            title_height: 28.0,
            traffic_light_radius: 6.0,
            traffic_light_spacing: 20.0,
            traffic_light_margin: 12.0,
            close_color: Color32::from_rgb(255, 95, 86),
            minimize_color: Color32::from_rgb(255, 189, 46),
            maximize_color: Color32::from_rgb(40, 200, 64),
            glyph_color: Color32::from_black_alpha(170),
        }
    }
}

#[derive(Clone)]
pub struct Style {
    pub dock_area_padding: Option<Margin>,
    pub main_surface_border_stroke: Stroke,
    pub main_surface_border_rounding: CornerRadius,
    pub separator: SeparatorStyle,
    pub tab_bar: TabBarStyle,
    pub tab: TabStyle,
    pub overlay: OverlayStyle,
    pub window: WindowStyle,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            dock_area_padding: None,
            main_surface_border_stroke: Stroke::new(0.0, Color32::BLACK),
            main_surface_border_rounding: CornerRadius::default(),
            separator: SeparatorStyle::default(),
            tab_bar: TabBarStyle::default(),
            tab: TabStyle::default(),
            overlay: OverlayStyle::default(),
            window: WindowStyle::default(),
        }
    }
}
