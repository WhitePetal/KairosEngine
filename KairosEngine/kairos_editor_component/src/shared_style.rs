//! Shared styles: appearance and global resources owned by the editor shell,
//! not by a panel.
//!
//! A panel owns its skin and loads it in [`Panel::init`](crate::Panel::init).
//! The toolbar is app chrome; the color picker, the value fields and the
//! reorderable list are widgets; the font list is a global resource. None has a
//! per-instance lifecycle to hang a skin on.
//! They declare themselves with
//! `#[SharedStyle(...)]` next to `#[derive(StyleFields)]`; the derive emits the
//! id constant and submits a [`SharedStyleDef`]. The app collects the
//! [`StyleCatalog`] once at boot and [`SharedStyles`] owns the live values.
//!
//! Delivery and editing are deliberately separate from panel skins:
//!
//! * [`Frame`](crate::Frame) hands the live styles to every panel each frame,
//!   so all call sites draw from one source;
//! * the Preferences panel edits them through the separate
//!   [`SharedStyleAction`](kairos_editor_core::SharedStyleAction) path, because
//!   there is no panel instance to route a `SetField` to.
//!
//! A widget reads its own runtime tokens, and the serde skin here projects onto
//! them at the call site: the color picker's [`ColorPickerSkin::tokens`] and
//! the font list's [`EditorFonts::config`] are the same move, which keeps the
//! widget kit out of the editor's file format.
//!
//! The type writes only [`SharedStyle::preload`]. The derive implements the
//! sealed erasure half and submits the definition; each half is what makes the
//! other mandatory:
//!
//! * `SharedStyle` needs a crate-private erasure trait that only the derive
//!   emits, so a hand-written shared style without `#[SharedStyle(...)]`
//!   does not compile;
//! * the generated [`SharedStyleDef::new`] needs `SharedStyle`, so the
//!   attribute without the `impl SharedStyle` does not compile either.
//!
//! `Style` is not a supertrait of [`SharedStyle`]: its `dark() -> Self` and
//! associated `FILE` make it non-object-safe, and the registry stores
//! `Box<dyn SharedStyle>`. The concrete type bound lives on
//! [`SharedStyleDef::new`] instead.

mod catalog;
mod fonts;
mod registry;

pub mod color_picker;
pub mod field;
pub mod list;

pub use crate::toolbar::style::TOOLBAR;
pub use catalog::{SharedStyleDef, StyleCatalog};
pub use color_picker::{COLOR_PICKER, ColorPickerSkin};
pub use field::{EDITOR_FIELD, EditorFieldStyle};
pub use fonts::{EditorFonts, FONTS};
pub use list::{LIST, ListStyle};
pub use registry::{SharedSkin, SharedStyles};

use egui::Context;

use crate::skin::StyleFields;

/// A style value the editor shell owns and the Preferences panel can edit.
///
/// Declare it with `#[SharedStyle(...)]` next to `#[derive(StyleFields)]`;
/// the type then implements only the optional [`preload`](SharedStyle::preload)
/// hook. The editable field table comes from [`StyleFields`].
///
/// The supertrait is a crate-private seal implemented by the derive, so this
/// trait cannot be implemented by hand (and therefore cannot be declared
/// without the registration).
pub trait SharedStyle:
    crate::__sealed::SharedStyleSealed + StyleFields + Send + Sync + 'static
{
    /// Warm runtime images this style points at. Called after load and reload;
    /// the widgets also load lazily, so this is a lifecycle hook, not a
    /// correctness requirement.
    fn preload(&self, _ctx: &Context) {}
}
