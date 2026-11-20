//! The value field's appearance tokens.

use egui::Color32;

use crate::text::TextStyle;

/// Visual tokens for one value field: its box, its name, and the wash that
/// marks a selected value.
///
/// Components build one from their serializable skin and reuse it for every
/// field on a page; a widget never reads the theme itself.
#[derive(Clone, Debug)]
pub struct FieldStyle {
    /// Value text.
    pub text: TextStyle,
    /// Row name at rest.
    pub label: Color32,
    /// Row name while the field is selected or being edited.
    pub label_active: Color32,
    /// Box fill at rest.
    pub fill: Color32,
    /// Box fill under the pointer, before the field is selected.
    pub hover_fill: Color32,
    /// Outline at rest.
    pub edge: Color32,
    /// Outline under the pointer, before the field is selected.
    pub edge_hover: Color32,
    /// Outline of an active field: selected, focused, or being edited.
    pub accent: Color32,
    /// Outline thickness. Zero paints no outline.
    pub border_width: f32,
    pub corner_radius: u8,
    /// Horizontal inset of the value text.
    pub padding: f32,
    /// Height of a value box; the row that owns the field centres it.
    pub height: f32,
    /// Side of a boolean checkbox.
    pub checkbox: f32,
}

impl Default for FieldStyle {
    fn default() -> Self {
        Self::new(TextStyle::sized(Color32::from_rgb(0xDC, 0xDC, 0xDC), 12.0))
    }
}

impl FieldStyle {
    /// A style with the given value text; colours start from the editor's dark
    /// chrome so a caller only overrides what its skin owns.
    pub fn new(text: TextStyle) -> Self {
        Self {
            text,
            label: Color32::from_rgb(0xC8, 0xC8, 0xC8),
            label_active: Color32::from_rgb(0x8F, 0xB4, 0xD9),
            fill: Color32::from_rgb(0x28, 0x28, 0x28),
            hover_fill: Color32::from_rgb(0x3E, 0x3E, 0x3E),
            edge: Color32::from_rgb(0x20, 0x20, 0x20),
            edge_hover: Color32::from_rgb(0x5A, 0x5A, 0x5A),
            accent: Color32::from_rgb(0x4A, 0x90, 0xD9),
            border_width: 1.0,
            corner_radius: 0,
            padding: 6.0,
            height: 18.0,
            checkbox: 15.0,
        }
    }

    /// Row name at rest and while the field is active.
    pub fn labels(mut self, label: Color32, label_active: Color32) -> Self {
        self.label = label;
        self.label_active = label_active;
        self
    }

    /// Box fill at rest and under the pointer.
    pub fn fills(mut self, fill: Color32, hover_fill: Color32) -> Self {
        self.fill = fill;
        self.hover_fill = hover_fill;
        self
    }

    /// Outline at rest, under the pointer, and while active.
    pub fn edges(mut self, edge: Color32, edge_hover: Color32, accent: Color32) -> Self {
        self.edge = edge;
        self.edge_hover = edge_hover;
        self.accent = accent;
        self
    }

    pub fn border_width(mut self, border_width: f32) -> Self {
        self.border_width = border_width;
        self
    }

    pub fn corner_radius(mut self, corner_radius: u8) -> Self {
        self.corner_radius = corner_radius;
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn checkbox(mut self, checkbox: f32) -> Self {
        self.checkbox = checkbox;
        self
    }

    /// Outline colour of a field in this state.
    ///
    /// Active wins over hover: a selected field keeps its accent edge while the
    /// pointer is over it.
    pub fn outline(&self, active: bool, hovered: bool) -> Color32 {
        if active {
            self.accent
        } else if hovered {
            self.edge_hover
        } else {
            self.edge
        }
    }

    /// Name colour: Unity turns the label blue while its field is active.
    pub fn label_color(&self, active: bool) -> Color32 {
        if active {
            self.label_active
        } else {
            self.label
        }
    }
}
