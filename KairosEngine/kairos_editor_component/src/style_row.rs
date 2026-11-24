//! Styles that draw their own editor.
//!
//! A style is a plain Rust value. Its Preferences page is generated from the
//! same declaration (`#[derive(StyleRow)]` emits [`StyleRow`]): a token becomes
//! a row, a nested struct a group, an enum a menu row plus its payload, a
//! `Vec<T>` a list. The style walks itself into a [`RowSink`], which owns
//! everything the value cannot know: layout, the collapse and selection state,
//! the commit gesture.
//!
//! The sink in the editor is [`FormRows`](crate::form::FormRows); a test records
//! the calls instead, so the generated walk can be asserted token by token.
//!
//! The widgets borrow the value (`&mut N`, `&mut E`, `&mut Vec<T>`), so no
//! per-token value type and no dotted path is involved: the page draws the one
//! typed copy it holds and hands the whole value back. Because the page cannot
//! borrow another owner's style (`Frame` is read-only), the app keeps one
//! snapshot page per editable skin while Preferences is open and the panel
//! copies the page it shows — on a page switch, not per frame.

use kairos_color::Color;
use kairos_editor_widgets::color_picker::ColorProperty;
use kairos_editor_widgets::field::Number;

use crate::skin::StyleEnum;

#[doc(inline)]
pub use kairos_editor_component_macros::StyleRow;

/// The drawing side of the row protocol.
///
/// One method per leaf kind, plus the three scopes a style tree can open: a
/// group (a foldout band), an enum payload (rows laid out under the enum's
/// menu button) and a list (a header plus one block per element). The sink
/// keeps the current path, so the generated walk never joins strings.
pub trait RowSink {
    /// One number box, over the token's own type.
    fn number<N: Number>(&mut self, name: &str, value: &mut N);

    /// One single-line text box.
    fn text(&mut self, name: &str, value: &mut String);

    /// One checkbox.
    fn toggle(&mut self, name: &str, value: &mut bool);

    /// One color swatch. The token is an LDR color.
    fn color(&mut self, name: &str, value: &mut Color);

    /// One color that carries its own usage (HDR or LDR).
    fn color_property(&mut self, name: &str, value: &mut ColorProperty);

    /// One enum row: a menu over `E`'s variants, writing the chosen variant
    /// back into the value.
    fn variant<E: StyleEnum>(&mut self, name: &str, value: &mut E);

    /// One nested group, drawn as a foldout band.
    ///
    /// An empty `name` is a transparent scope: the root of a style, or a
    /// single-field tuple payload whose fields belong to the enum's own path.
    fn group(&mut self, name: &str, body: impl FnOnce(&mut Self));

    /// The active variant's payload rows, under the enum row `name`.
    ///
    /// Rows inside hang under the enum's menu button instead of starting a
    /// new indent level, and they are addressed below the enum's own path
    /// (`mode.radius`). Only variants with rows call in here, so a unit variant
    /// simply draws its menu row.
    fn payload(&mut self, name: &str, body: impl FnOnce(&mut Self));

    /// One list token: a header with the element count, then one block per
    /// element. The vector is resized, reordered and edited in place.
    fn list<T: StyleRow + Default>(&mut self, name: &str, values: &mut Vec<T>);
}

/// One style value that can draw itself into a [`RowSink`].
///
/// `#[derive(StyleRow)]` generates the implementation for a style struct or
/// enum. Hand-written implementations are the leaves (numbers, text, toggles,
/// colors), the enums of other crates projected into a local style enum, and
/// any type whose tokens are not its own fields.
pub trait StyleRow {
    /// Draw this token — or this group of tokens — under `name`.
    fn row(&mut self, name: &str, sink: &mut impl RowSink);
}

/// A style value a page can own and draw without knowing its type.
///
/// The Preferences page holds one of these per selected page (a typed copy of
/// the owner's style) and hands the whole value back when it changed; `dyn` is
/// what lets one page slot hold any style. Implemented for every [`StyleRow`],
/// so a style needs no extra declaration.
pub trait StylePage: kairos_editor_core::style::StyleBlob {
    /// Draw the whole page into `rows`.
    fn draw_page(&mut self, rows: &mut crate::form::FormRows<'_>);

    /// A copy of this page, for the write-back: the page keeps drawing its own
    /// copy while the owner receives the new value.
    fn clone_box(&self) -> Box<dyn StylePage>;
}

macro_rules! number_rows {
    ($($ty:ty),* $(,)?) => {
        $(
            impl StyleRow for $ty {
                fn row(&mut self, name: &str, sink: &mut impl RowSink) {
                    sink.number(name, self);
                }
            }
        )*
    };
}

// Each integer keeps its own type: the sink draws the concrete `NumberField<N>`,
// so no widened integer and no type-bound clamp is involved here.
number_rows!(f32, f64);
number_rows!(i8, i16, i32, i64, isize);
number_rows!(u8, u16, u32, u64, usize);

impl StyleRow for bool {
    fn row(&mut self, name: &str, sink: &mut impl RowSink) {
        sink.toggle(name, self);
    }
}

impl StyleRow for String {
    fn row(&mut self, name: &str, sink: &mut impl RowSink) {
        sink.text(name, self);
    }
}

impl StyleRow for Color {
    fn row(&mut self, name: &str, sink: &mut impl RowSink) {
        sink.color(name, self);
    }
}

impl StyleRow for ColorProperty {
    fn row(&mut self, name: &str, sink: &mut impl RowSink) {
        sink.color_property(name, self);
    }
}

/// A list token is one row (its header) plus one sub-walk per element; the
/// sink decides where the element blocks sit and how a drag reorders them.
impl<T: StyleRow + Default> StyleRow for Vec<T> {
    fn row(&mut self, name: &str, sink: &mut impl RowSink) {
        sink.list(name, self);
    }
}

#[cfg(test)]
mod tests {
    use kairos_color::Color;

    use super::*;

    #[derive(Clone, Debug, PartialEq, StyleRow)]
    struct Icons {
        warning: String,
        size: f32,
    }

    #[derive(Clone, Debug, PartialEq, StyleRow)]
    struct TestStyle {
        height: f32,
        enabled: bool,
        background: Color,
        #[style(skip)]
        cache: u32,
        icons: Icons,
        categories: Vec<f32>,
    }

    #[derive(Clone, Debug, Default, PartialEq, StyleRow)]
    struct Payload {
        fvalue: f32,
    }

    #[derive(Clone, Debug, PartialEq, StyleRow, strum::VariantNames)]
    enum Mode {
        Off,
        Soft { radius: f32, samples: u32 },
        Push(Payload),
    }

    fn style() -> TestStyle {
        TestStyle {
            height: 20.0,
            enabled: true,
            background: Color::from_srgb8([1, 2, 3, 255]),
            cache: 7,
            icons: Icons {
                warning: "w.png".to_owned(),
                size: 12.0,
            },
            categories: vec![1.0, 2.0],
        }
    }

    /// Records the walk as dotted paths, the same addresses the field table
    /// uses, so the two can be compared directly.
    #[derive(Debug, Default)]
    struct Recorder {
        rows: Vec<String>,
        path: Vec<String>,
    }

    impl Recorder {
        fn path_of(&self, name: &str) -> String {
            match (self.path.is_empty(), name.is_empty()) {
                (true, _) => name.to_owned(),
                (false, true) => self.path.join("."),
                (false, false) => format!("{}.{}", self.path.join("."), name),
            }
        }

        fn push(&mut self, name: &str) {
            let path = self.path_of(name);
            self.rows.push(path);
        }
    }

    impl RowSink for Recorder {
        fn number<N: Number>(&mut self, name: &str, _value: &mut N) {
            self.push(name);
        }

        fn text(&mut self, name: &str, _value: &mut String) {
            self.push(name);
        }

        fn toggle(&mut self, name: &str, _value: &mut bool) {
            self.push(name);
        }

        fn color(&mut self, name: &str, _value: &mut Color) {
            self.push(name);
        }

        fn color_property(&mut self, name: &str, _value: &mut ColorProperty) {
            self.push(name);
        }

        fn variant<E: StyleEnum>(&mut self, name: &str, _value: &mut E) {
            self.push(name);
        }

        fn group(&mut self, name: &str, body: impl FnOnce(&mut Self)) {
            // A group is layout, not a token: the field table has no row for
            // it either. Only the path changes.
            if name.is_empty() {
                return body(self);
            }
            self.path.push(name.to_owned());
            body(self);
            self.path.pop();
        }

        fn payload(&mut self, name: &str, body: impl FnOnce(&mut Self)) {
            self.path.push(name.to_owned());
            body(self);
            self.path.pop();
        }

        fn list<T: StyleRow + Default>(&mut self, name: &str, values: &mut Vec<T>) {
            // The list node is a token (its count is editable), so it keeps
            // its own row; the elements are ordinary sub-walks.
            self.push(name);
            self.path.push(name.to_owned());
            for (index, item) in values.iter_mut().enumerate() {
                StyleRow::row(item, &index.to_string(), self);
            }
            self.path.pop();
        }
    }

    #[test]
    fn a_generated_walk_hits_every_token_in_declaration_order() {
        let mut style = style();
        let mut recorder = Recorder::default();
        style.row("", &mut recorder);

        assert_eq!(
            recorder.rows,
            [
                "height",
                "enabled",
                "background",
                "icons.warning",
                "icons.size",
                "categories",
                "categories.0",
                "categories.1",
            ],
            "nested structs are groups, a list keeps its own row before its entries"
        );
    }

    #[test]
    fn a_skipped_field_draws_no_row() {
        let mut style = style();
        let mut recorder = Recorder::default();
        style.row("", &mut recorder);

        assert!(
            recorder.rows.iter().all(|path| path != "cache"),
            "{recorder:?}"
        );
    }

    /// A payload's rows hang under the enum's own path: the menu row names
    /// them, and the block layout is the sink's business (the `payload`
    /// scope), so the token paths are the same as the field table's.
    #[test]
    fn an_enum_draws_its_row_then_the_active_variants_payload() {
        let mut mode = Mode::Soft {
            radius: 4.0,
            samples: 2,
        };
        let mut recorder = Recorder::default();
        mode.row("mode", &mut recorder);
        assert_eq!(recorder.rows, ["mode", "mode.radius", "mode.samples"]);

        let mut unit = Mode::Off;
        let mut recorder = Recorder::default();
        unit.row("mode", &mut recorder);
        assert_eq!(
            recorder.rows,
            ["mode"],
            "a unit variant has no payload rows"
        );
    }

    /// A single-field tuple payload is transparent: it enters with an empty
    /// name, so its fields are the enum's own payload rows.
    #[test]
    fn a_tuple_payload_is_transparent() {
        let mut mode = Mode::Push(Payload { fvalue: 0.5 });
        let mut recorder = Recorder::default();
        mode.row("mode", &mut recorder);

        assert_eq!(recorder.rows, ["mode", "mode.fvalue"]);
    }
}
