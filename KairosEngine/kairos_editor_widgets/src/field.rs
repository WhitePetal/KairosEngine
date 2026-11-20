//! Boxed value fields: the right-hand column of an inspector.
//!
//! Unity's inspector is a list of properties: a name on the left, and a value
//! control drawn as a fixed-height box. The Preferences editor reproduces that
//! layout, and it is not the last place that will — an inspector panel for
//! scene objects draws the same shapes. So the controls live here, over plain
//! values (`f32`, `u32`, `String`, `bool`), and know nothing about style
//! tokens, dotted paths or domain actions. A component maps its own field table
//! onto them and decides what an edit means.
//!
//! One file per control:
//!
//! * `style`: the appearance tokens ([FieldStyle]);
//! * `paint`: the box fill, outline and checkbox glyph they share;
//! * `edit`: the inline text editor, with Unity's selected-text wash;
//! * `number`: [NumberField], over any [Number] primitive;
//! * `text`: [TextField];
//! * `toggle`: [ToggleField];
//! * `enum_field`: [EnumField], a popup field over an enum, and [VariantMenu]
//!   for a style token's static variant table;
//! * `label`: [FieldLabel], the row name that scrubs a number.
//!
//! Every control has the same shape:
//!
//!     let outcome = NumberField::new(id, &mut value, &style)
//!         .label("Width")
//!         .show(ui, rect, &mut edit);
//!     if outcome.changed() { .. }
//!
//! [EnumField] borrows the enum mutably, like the other controls borrow their
//! values: choosing a row writes the new variant into the source enum.
//! [VariantMenu] is the style-token view used when the caller has a variant id
//! and a static table rather than the typed value.
//!
//! `show` takes the rect the caller laid out (an inspector's two columns
//! belong to the panel, not to egui's layout) and returns a [FieldOutcome]: the
//! egui [Response] plus whether a typed edit ended. That flag is what lets a
//! panel write its RON file when the user presses Enter, while a drag is
//! written when the pointer comes up.
//!
//! # State the caller owns
//!
//! A number box switches into a text editor on click. The editor's buffer is
//! [NumberEdit], which the caller keeps next to its own field state and hands
//! back every pass. That way a panel can drop a half-typed edit when the row
//! disappears, and a test can drive the control without knowing egui's memory
//! keys. Everything else — cursor, selection, focus — is the TextEdit's own
//! state.

mod edit;
mod enum_field;
mod label;
mod number;
mod paint;
mod style;
mod text;
mod toggle;

#[cfg(test)]
mod testing;

pub use enum_field::{EnumField, VariantMenu, enum_item_id, enum_popup_id};
pub use label::{FieldLabel, FieldLabelResponse};
pub use number::{Number, NumberEdit, NumberField};
pub use style::FieldStyle;
pub use text::TextField;
pub use toggle::ToggleField;

use egui::Response;

/// One frame of Unity's scrub gesture: the pointer moved while held on a
/// field's name.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scrub {
    /// Horizontal pointer delta this pass.
    pub delta: f32,
    /// This pass ends the drag.
    pub stopped: bool,
}

/// What a field control did this pass.
#[derive(Debug)]
pub struct FieldOutcome {
    /// The control's own interaction response; its rect is the field's rect.
    pub response: Response,
    /// A typed edit or a picker gesture ended. A caller that batches writes
    /// should flush now instead of waiting for the pointer to come up.
    pub finished: bool,
}

impl FieldOutcome {
    pub fn changed(&self) -> bool {
        self.response.changed()
    }

    pub fn clicked(&self) -> bool {
        self.response.clicked()
    }
}
