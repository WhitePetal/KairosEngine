//! Styles that draw their own editor page.
//!
//! A style is a plain Rust value. Its Preferences page is generated from the
//! same declaration (`#[derive(StyleRow)]` emits [`StyleRow`]): a token becomes
//! a row, a nested struct a foldout band, an enum a menu row plus its payload,
//! a `Vec<T>` a list. The style walks itself into a
//! [`RowSink`](kairos_editor_widgets::form::RowSink), which owns everything the
//! value cannot know: layout, the collapse and selection state, the commit
//! gesture.
//!
//! The row protocol and the widget that draws it are the widget kit's: how a
//! property row is laid out, striped and selected is not a style concept, and an
//! object inspector (a form over `&mut` properties) will want the same rows. So
//! [`RowSink`]/[`StyleRow`] live in `kairos_editor_widgets::form` and are
//! re-exported here under the names the derive already generates; the derived
//! `StyleRow::row` walks into the widget's `FormRows`. This module keeps the
//! half that needs the editor's page machinery: [`StylePage`], the erased page
//! value that has no type at the call site. The trait needs
//! `kairos_editor_core::style::StyleBlob` for the whole-value write, which the
//! widget kit must not know.
//!
//! The widgets borrow the value (`&mut N`, `&mut E`, `&mut Vec<T>`), so no
//! per-token value type and no dotted path is involved: the page draws the one
//! typed copy it holds and hands the whole value back. Because the page cannot
//! borrow another owner's style (`Frame` is read-only), the app keeps one
//! snapshot page per editable skin while Preferences is open and the panel
//! copies the page it shows — on a page switch, not per frame.

// The derive macro stays in the component crate: it also emits the page half
// (`StyleBlob`/`StylePage`/`StyleEnum`) and the `#[SharedStyle(...)]`
// registration, none of which belongs to the widget kit.
#[doc(inline)]
pub use kairos_editor_component_macros::StyleRow;
// The row protocol itself is a widget-kit concept; re-exported so generated
// code (and this crate's own call sites) keep one path for the trait.
pub use kairos_editor_widgets::form::{RowSink, StyleRow};

/// A style value a page can own and draw without knowing its type.
///
/// The Preferences page holds one of these per selected page (a typed copy of
/// the owner's style) and hands the whole value back when it changed; `dyn` is
/// what lets one page slot hold any style. Implemented for every [`StyleRow`],
/// so a style needs no extra declaration.
pub trait StylePage: kairos_editor_core::style::StyleBlob {
    /// Draw the whole page into the form rows.
    fn draw_page(&mut self, rows: &mut kairos_editor_widgets::form::FormRows<'_>);

    /// A copy of this page, for the write-back: the page keeps drawing its own
    /// copy while the owner receives the new value.
    fn clone_box(&self) -> Box<dyn StylePage>;
}

#[cfg(test)]
mod tests {
    use kairos_color::Color;
    use kairos_editor_widgets::color_picker::ColorProperty;
    use kairos_editor_widgets::field::Number;

    use super::*;
    use crate::skin::StyleEnum;

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

    /// The page is a `dyn` value drawn by the same walk: the widget takes the
    /// erased page and draws it, so a page slot needs no type at the call site.
    #[test]
    fn a_boxed_page_draws_itself_into_the_form_widget() {
        use egui::{Id, Pos2, RawInput, Rect, vec2};
        use kairos_editor_widgets::color_picker::ColorPickerStyle;
        use kairos_editor_widgets::field::{FieldStyle, ListChromeStyle};
        use kairos_editor_widgets::form::{FormRows, FormState, FormStyle};

        let ctx = egui::Context::default();
        let mut page: Box<dyn StylePage> = Box::new(style());
        let form = FormStyle::dark();
        let field = FieldStyle::default();
        let chrome = ListChromeStyle::from_field(&field);
        let picker = ColorPickerStyle::default();
        let mut state = FormState::default();
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(420.0, 620.0))),
            ..Default::default()
        };
        let _ = ctx.run_ui(input, |ui| {
            let mut rows = FormRows::new(
                ui,
                Id::new("kairos.style-page.test"),
                &form,
                &field,
                &chrome,
                &picker,
                &mut state,
                360.0,
            );
            page.draw_page(&mut rows);
            assert!(!rows.changed(), "drawing alone writes nothing");
        });
    }
}
