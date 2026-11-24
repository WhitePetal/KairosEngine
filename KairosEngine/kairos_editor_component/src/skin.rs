//! Style tokens the editor can edit: the vocabulary a style type and its
//! Preferences page share.
//!
//! A style is a plain Rust value that draws itself
//! ([`StyleRow`](crate::style_row::StyleRow) walking into
//! [`FormRows`](crate::form::FormRows)); the Preferences panel holds a typed copy
//! and hands the whole value back when it changed. This module keeps the pieces
//! *around* that value:
//!
//! * [`SkinId`] — which owner a page belongs to (a panel instance or a shared
//!   style), so one page list can hold both kinds;
//! * [`PageMeta`] — the owner id, the title and the file shown in the page
//!   header, without a style copy: what the page list is built from and what
//!   the app compares to keep its snapshot current;
//! * [`PageSkin`] — one page: the metadata plus the self-drawing copy of the
//!   style, which is what the panel clones when the user switches pages;
//! * [`StyleEnum`] — the token protocol an enum needs to be a variant menu;
//! * [`humanize`] and [`LIST_MAX`] — a row's label and a list's growth cap;
//! * [`save_skin`] — persist one live style into its RON file.
//!
//! Live apply and persistence are two separate steps: the page hands the value
//! over as it changes (live preview), and the app writes the file once per
//! gesture (`Commit` on pointer release, typed edit end, page switch, close).
//! This module only describes and persists; it does not know about the
//! Preferences panel.

use kairos_editor_core::{PanelId, SharedStyleId};
use kairos_editor_widgets::style::{Style, StyleFile};

/// Largest list the editor will grow a `Vec<T>` token to.
///
/// A list token is a hand-authored style value (a font fallback chain, the
/// profiler's category palette), not a user document, so a few dozen entries
/// is the whole range that exists. The cap keeps a dragged or typed count from
/// allocating an unbounded number of default entries; a file may still author
/// more, it just cannot be grown past this from the editor.
pub const LIST_MAX: u32 = 64;

/// An enum token: its variants, the active one, and how to rebuild a variant.
///
/// `#[derive(StyleRow)]` generates this for an enum, so the menu labels, the
/// row and the rebuild path cannot drift from the declaration. The enum also
/// derives `strum::VariantNames`: that is the one table of names the
/// `VariantMenu` reads. The id written to RON is the Rust variant ident; do not
/// override the strum name (`#[strum(serialize = ...)]`) on a style enum, or the
/// menu and the stored id would diverge.
///
/// A type from another crate cannot take the derive; if a style needs one,
/// declare a parallel enum in this crate and project it onto the runtime type
/// (the font list's `EditorFontPriority` -> `FontPriority` is the example).
///
/// A variant with named fields is a *payload*: `StyleRow` draws its fields under
/// the enum's menu row, in a payload scope. A single-field tuple variant is
/// transparent: its payload's fields are the enum's rows directly, with no
/// header for the unnamed field. Switching variants rebuilds the new one from
/// `Default`, which is the value semantics of a Rust enum.
///
/// A style enum that a typed editor should own also derives strum's `Display`
/// and `EnumIter`; the Preferences page needs neither, because `VariantMenu`
/// edits the id against [`StyleEnum::variants`].
pub trait StyleEnum: Sized + strum::VariantNames {
    /// Every variant name, in declaration order.
    fn variants() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// The active variant's stable id.
    fn variant_id(&self) -> &'static str;

    /// Rebuild the enum as the named variant, with a default payload.
    ///
    /// `None` for an id this enum does not have: an unknown choice leaves the
    /// value untouched rather than panicking on a stale menu.
    fn with_variant(id: &str) -> Option<Self>;
}

/// Which owner a typed page belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkinId {
    Panel(PanelId),
    Shared(SharedStyleId),
}

/// One page's owner, heading and file, without the style value.
///
/// Building this costs no style copy, so the app can rebuild the list every
/// frame and compare it with the snapshot it holds; only a changed list makes it
/// copy a value. The panel needs the same three pieces to draw the page list and
/// the heading, so the snapshot carries them next to the value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageMeta {
    pub id: SkinId,
    /// Page heading: the panel's `Panel::title`, or the shared style's title.
    pub title: String,
    /// File name under `Preferences/Styles/`.
    pub file: &'static str,
}

/// One page a style draws itself: its metadata and a typed copy of the owner's
/// style.
///
/// The app holds one of these per page while Preferences is open and hands the
/// list to the panel; the panel copies the one page the user selected and hands
/// a new value back when it changes (`StyleAction::Replace`). Nothing here
/// describes tokens.
pub struct PageSkin {
    pub id: SkinId,
    /// Page heading: the panel's `Panel::title`, or the shared style's title.
    pub title: String,
    /// File name under `Preferences/Styles/`.
    pub file: &'static str,
    /// The style value the page draws.
    pub page: Box<dyn crate::style_row::StylePage>,
}

/// Persist one panel's live style into its RON file.
///
/// The panel keeps only the skin it draws with. The other half is read back
/// from the file first, so editing the dark skin does not throw away a
/// hand-edited light one; a missing file is seeded from the code defaults.
pub fn save_skin<S: Style>(style: &S, theme: egui::Theme) -> Result<(), String> {
    let path = StyleFile::<S>::path();
    let mut file = if path.is_file() {
        StyleFile::<S>::load_from(&path)?
    } else {
        StyleFile::<S>::defaults()
    };
    match theme {
        egui::Theme::Light => file.light = style.clone(),
        egui::Theme::Dark => file.dark = style.clone(),
    }
    file.save_to(&path)
}

/// `toolbar_bg` -> `"Toolbar bg"`: the editor label for a token.
///
/// The label comes from the field name, so a row cannot be shown under a
/// hand-written label that drifted; this is the one place the naming rule lives.
pub fn humanize(path: &str) -> String {
    let leaf = path.rsplit('.').next().unwrap_or(path);
    let mut name = String::with_capacity(leaf.len());
    for (index, word) in leaf.split('_').filter(|word| !word.is_empty()).enumerate() {
        if index > 0 {
            name.push(' ');
        }
        if index == 0 {
            // Sentence case: only the first word is capitalized, because these
            // labels sit in a dense form where Title Case on every word reads
            // like an acronym.
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                name.extend(first.to_uppercase());
                name.push_str(chars.as_str());
            }
        } else {
            name.push_str(word);
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_label_is_derived_from_the_field_name() {
        assert_eq!(humanize("toolbar_bg"), "Toolbar bg");
        assert_eq!(humanize("icons.warning"), "Warning");
        assert_eq!(humanize("x"), "X");
    }
}
