use egui::{Color32, CornerRadius, Margin, Stroke};

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
            inner_margin: Margin::same(4),
            stroke: Stroke::default(),
            corner_radius: CornerRadius::default(),
            bg_fill: Color32::from_rgb(67, 67, 67),
        }
    }
}

#[derive(Clone)]
pub struct TabInteractionStyle {
    pub headerline_color: Color32,
    pub outline_color: Color32,
    pub corner_radius: CornerRadius,
    pub bg_fill: Color32,
    pub text_color: Color32,
}

impl Default for TabInteractionStyle {
    fn default() -> Self {
        Self {
            headerline_color: Color32::TRANSPARENT,
            outline_color: Color32::TRANSPARENT,
            corner_radius: CornerRadius {
                nw: 4,
                ne: 4,
                sw: 0,
                se: 0,
            },
            bg_fill: Color32::from_rgb(67, 67, 67),
            text_color: Color32::from_rgb(196, 196, 196)
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
}

impl Default for TabStyle {
    fn default() -> Self {
        Self {
            active: TabInteractionStyle::default(),
            inactive: TabInteractionStyle::default(),
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
            minimum_width: None
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
            feel: Default::default()
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
}

impl Default for SeparatorStyle {
    fn default() -> Self {
        Self {
            width: 1.0,
            extra_interact_width: 2.0,
            extra: 175.0,
            color_idle: Color32::BLACK,
            color_hovered: Color32::from_rgb(67, 67, 67),
            color_dragged: Color32::from_rgb(126, 126, 126),
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
    pub hline_color: Color32,
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
            hline_color: Color32::BLACK,
            fill_tab_bar: false,
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
        }
    }
}
